//! Брандмауэр: профили вкл/выкл и правила. Наши правила помечены группой «winsetup».

use super::{ps, q};
use crate::config::{Action, Direction, Firewall, FirewallRule, Protocol};
use crate::step::Step;

pub struct ProfilesStep {
    /// (имя профиля для PowerShell, включить?)
    profiles: Vec<(&'static str, bool)>,
}

fn on_off(b: bool) -> &'static str { if b { t!("вкл") } else { t!("выкл") } }

impl Step for ProfilesStep {
    fn describe(&self) -> String {
        let names = |p: &str| match p { "Domain" => t!("доменный"), "Private" => t!("частный"), _ => t!("общий") };
        let parts: Vec<String> = self.profiles.iter().map(|(p, on)| format!("{} {}", names(p), on_off(*on))).collect();
        t!("Брандмауэр: профили — {list}", list = parts.join(", "))
    }

    fn check(&self) -> Result<bool, String> {
        for (p, on) in &self.profiles {
            let cur = ps(&format!("(Get-NetFirewallProfile -Name {p}).Enabled"))?;
            if cur.eq_ignore_ascii_case("true") != *on { return Ok(false); }
        }
        Ok(true)
    }

    fn apply(&self) -> Result<(), String> {
        let script: String = self.profiles.iter()
            .map(|(p, on)| format!("Set-NetFirewallProfile -Profile {p} -Enabled {}; ", if *on { "True" } else { "False" }))
            .collect();
        ps(&script).map(|_| ())
    }
}

pub struct RuleStep<'a>(pub &'a FirewallRule);

impl RuleStep<'_> {
    fn dir(&self) -> &'static str { if self.0.direction == Direction::In { "Inbound" } else { "Outbound" } }
    fn action(&self) -> &'static str { if self.0.action == Action::Allow { "Allow" } else { "Block" } }
    fn proto(&self) -> &'static str {
        match self.0.protocol { Protocol::Tcp => "TCP", Protocol::Udp => "UDP", Protocol::Icmp => "ICMPv4", Protocol::Any => "Any" }
    }
    /// Для входящих — локальный порт, для исходящих — удалённый.
    fn port_param(&self) -> &'static str { if self.0.direction == Direction::In { "LocalPort" } else { "RemotePort" } }

    /// Строка, с которой сравнивается состояние: `Inbound|Allow|TCP|3389|True`.
    fn expected(&self) -> String {
        let port = self.0.port.map(|p| p.to_string()).unwrap_or_else(|| "Any".into());
        format!("{}|{}|{}|{}|True", self.dir(), self.action(), self.proto(), port)
    }
}

impl Step for RuleStep<'_> {
    fn describe(&self) -> String {
        let r = self.0;
        let dir = if r.direction == Direction::In { t!("входящие") } else { t!("исходящие") };
        let act = if r.action == Action::Allow { t!("разрешить") } else { t!("блокировать") };
        let port = r.port.map(|p| t!(" порт {p}", p = p)).unwrap_or_default();
        t!("Брандмауэр: правило «{name}» — {act} {dir} {proto}{port}", name = r.name, act = act, dir = dir, proto = self.proto(), port = port)
    }

    fn check(&self) -> Result<bool, String> {
        let n = q(&self.0.name);
        let pp = self.port_param();
        let out = ps(&format!(
            "$r = Get-NetFirewallRule -DisplayName {n} -ErrorAction SilentlyContinue; \
             if (@($r).Count -ne 1) {{ 'missing'; exit }}; \
             $f = $r | Get-NetFirewallPortFilter; \
             \"$($r.Direction)|$($r.Action)|$($f.Protocol)|$($f.{pp})|$($r.Enabled)\""
        ))?;
        Ok(out == self.expected())
    }

    fn apply(&self) -> Result<(), String> {
        let n = q(&self.0.name);
        // пересоздаём: проще и надёжнее, чем менять поля по одному
        let mut script = format!(
            "Get-NetFirewallRule -DisplayName {n} -ErrorAction SilentlyContinue | Remove-NetFirewallRule; \
             $null = New-NetFirewallRule -DisplayName {n} -Group 'winsetup' -Direction {} -Action {} -Profile Any",
            self.dir(), self.action()
        );
        if self.0.protocol != Protocol::Any { script += &format!(" -Protocol {}", self.proto()); }
        if self.0.protocol == Protocol::Icmp { script += " -IcmpType 8"; } // 8 = echo request (ping)
        if let Some(p) = self.0.port { script += &format!(" -{} {p}", self.port_param()); }
        ps(&script).map(|_| ())
    }
}

pub fn steps(fw: &Firewall) -> Vec<Box<dyn Step + '_>> {
    let mut s: Vec<Box<dyn Step + '_>> = Vec::new();
    let profiles: Vec<_> = [("Domain", fw.domain), ("Private", fw.private), ("Public", fw.public)]
        .into_iter().filter_map(|(p, v)| v.map(|on| (p, on))).collect();
    if !profiles.is_empty() { s.push(Box::new(ProfilesStep { profiles })); }
    s.extend(fw.rule.iter().map(|r| Box::new(RuleStep(r)) as Box<dyn Step>));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_state() {
        let r = FirewallRule { name: "RDP".into(), direction: Direction::In, action: Action::Allow, protocol: Protocol::Tcp, port: Some(3389) };
        assert_eq!(RuleStep(&r).expected(), "Inbound|Allow|TCP|3389|True");
        let any = FirewallRule { name: "x".into(), direction: Direction::Out, action: Action::Block, protocol: Protocol::Any, port: None };
        assert_eq!(RuleStep(&any).expected(), "Outbound|Block|Any|Any|True");
    }
}
