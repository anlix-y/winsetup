//! Сеть: IPv4-адрес, маска, шлюз, DNS, DHCP. Меняется только то, что отличается.

use super::{ps, q};
use crate::config::Network;
use crate::step::Step;

pub struct NetworkStep<'a>(pub &'a Network);

/// Текущее состояние адаптера.
#[derive(Debug, Default, PartialEq)]
struct State {
    dhcp: bool,
    /// Ручные (не от DHCP) адреса: "ip/prefix".
    addrs: Vec<String>,
    gateway: Option<String>,
    dns: Vec<String>,
}

/// Ответ скрипта: `dhcp|addr1,addr2|gateway|dns1,dns2`.
fn parse_state(s: &str) -> Option<State> {
    let mut p = s.trim().split('|');
    let list = |x: &str| x.split(',').filter(|v| !v.is_empty()).map(String::from).collect::<Vec<_>>();
    Some(State {
        dhcp: p.next()? == "Enabled",
        addrs: list(p.next()?),
        gateway: Some(p.next()?).filter(|g| !g.is_empty()).map(String::from),
        dns: list(p.next()?),
    })
}

fn query(adapter: &str) -> Result<State, String> {
    let a = q(adapter);
    let out = ps(&format!(
        "$null = Get-NetAdapter -Name {a}; \
         $i = Get-NetIPInterface -InterfaceAlias {a} -AddressFamily IPv4; \
         $ad = (Get-NetIPAddress -InterfaceAlias {a} -AddressFamily IPv4 -ErrorAction SilentlyContinue | \
               Where-Object PrefixOrigin -eq 'Manual' | ForEach-Object {{ \"$($_.IPAddress)/$($_.PrefixLength)\" }}) -join ','; \
         $gw = (Get-NetRoute -InterfaceAlias {a} -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Select-Object -First 1).NextHop; \
         $dns = (Get-DnsClientServerAddress -InterfaceAlias {a} -AddressFamily IPv4).ServerAddresses -join ','; \
         \"$($i.Dhcp)|$ad|$gw|$dns\""
    ))?;
    parse_state(&out).ok_or_else(|| t!("не удалось разобрать состояние адаптера: {out}", out = out))
}

/// Нужно ли менять адресацию (DHCP / IP / маска / шлюз).
fn addressing_differs(n: &Network, cur: &State) -> bool {
    if n.dhcp {
        return !cur.dhcp;
    }
    let want = format!("{}/{}", n.ip.unwrap(), n.prefix.unwrap()); // проверено в config::validate
    cur.dhcp
        || cur.addrs != [want]
        || n.gateway.is_some_and(|g| cur.gateway.as_deref() != Some(g.to_string().as_str()))
}

/// Нужно ли менять DNS. Пустой список при статике — DNS не трогаем; при DHCP — DNS «от DHCP».
fn dns_differs(n: &Network, cur: &State) -> bool {
    let want: Vec<String> = n.dns.iter().map(|d| d.to_string()).collect();
    !want.is_empty() && cur.dns != want
}

impl Step for NetworkStep<'_> {
    fn describe(&self) -> String {
        let n = self.0;
        let mut s = t!("Сеть «{name}»: ", name = n.adapter);
        if n.dhcp {
            s += "DHCP";
        } else {
            s += &format!("{}/{}", n.ip.unwrap(), n.prefix.unwrap());
            if let Some(g) = n.gateway { s += &t!(", шлюз {g}", g = g); }
        }
        if !n.dns.is_empty() {
            s += &format!(", DNS {}", n.dns.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(", "));
        }
        s
    }

    fn check(&self) -> Result<bool, String> {
        let cur = query(&self.0.adapter)?;
        Ok(!addressing_differs(self.0, &cur) && !dns_differs(self.0, &cur))
    }

    fn apply(&self) -> Result<(), String> {
        let n = self.0;
        let a = q(&n.adapter);
        let cur = query(&n.adapter)?;
        let mut script = String::new();
        if addressing_differs(n, &cur) {
            // старые ручные адреса и шлюз удаляем, иначе New-NetIPAddress упадёт или останется два шлюза
            script += &format!(
                "Get-NetRoute -InterfaceAlias {a} -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Remove-NetRoute -Confirm:$false; "
            );
            if n.dhcp {
                script += &format!("Set-NetIPInterface -InterfaceAlias {a} -Dhcp Enabled; ");
                if n.dns.is_empty() {
                    script += &format!("Set-DnsClientServerAddress -InterfaceAlias {a} -ResetServerAddresses; ");
                }
            } else {
                script += &format!(
                    "Set-NetIPInterface -InterfaceAlias {a} -Dhcp Disabled; \
                     Get-NetIPAddress -InterfaceAlias {a} -AddressFamily IPv4 -ErrorAction SilentlyContinue | Remove-NetIPAddress -Confirm:$false; \
                     $null = New-NetIPAddress -InterfaceAlias {a} -IPAddress {} -PrefixLength {}",
                    q(&n.ip.unwrap().to_string()), n.prefix.unwrap()
                );
                if let Some(g) = n.gateway { script += &format!(" -DefaultGateway {}", q(&g.to_string())); }
                script += "; ";
            }
        }
        if dns_differs(n, &cur) {
            let list: Vec<String> = n.dns.iter().map(|d| q(&d.to_string())).collect();
            script += &format!("Set-DnsClientServerAddress -InterfaceAlias {a} -ServerAddresses @({}); ", list.join(","));
        }
        ps(&script).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(dhcp: bool, dns: &[&str]) -> Network {
        Network {
            adapter: "Ethernet".into(),
            dhcp,
            ip: (!dhcp).then(|| "192.168.1.50".parse().unwrap()),
            prefix: (!dhcp).then_some(24),
            gateway: (!dhcp).then(|| "192.168.1.1".parse().unwrap()),
            dns: dns.iter().map(|d| d.parse().unwrap()).collect(),
        }
    }

    #[test]
    fn parse() {
        let s = parse_state("Enabled||192.168.122.1|192.168.122.1\r\n").unwrap();
        assert_eq!(s, State { dhcp: true, addrs: vec![], gateway: Some("192.168.122.1".into()), dns: vec!["192.168.122.1".into()] });
    }

    #[test]
    fn dns_only_change_keeps_addressing() {
        // DHCP уже включён, меняется только DNS — адресацию (и шлюз) не трогаем
        let cur = parse_state("Enabled||192.168.122.1|192.168.122.1").unwrap();
        let n = net(true, &["1.1.1.1", "8.8.8.8"]);
        assert!(!addressing_differs(&n, &cur));
        assert!(dns_differs(&n, &cur));
    }

    #[test]
    fn static_matches() {
        let cur = parse_state("Disabled|192.168.1.50/24|192.168.1.1|8.8.8.8").unwrap();
        assert!(!addressing_differs(&net(false, &[]), &cur));
        assert!(!dns_differs(&net(false, &[]), &cur)); // DNS не указан — не трогаем
        let other = parse_state("Disabled|192.168.1.51/24|192.168.1.1|").unwrap();
        assert!(addressing_differs(&net(false, &[]), &other));
    }
}
