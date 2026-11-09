//! Локальные пользователи и группы.

use super::{ps_env, q};
use crate::config::User;
use crate::step::Step;

pub struct UserStep<'a>(pub &'a User);

/// Встроенные группы по SID — их имена переведены в локализованных Windows,
/// а SID одинаковый везде. Неизвестное имя считаем своей локальной группой.
pub fn group_sid(name: &str) -> Option<&'static str> {
    Some(match name.to_ascii_lowercase().as_str() {
        "administrators" => "S-1-5-32-544",
        "users" => "S-1-5-32-545",
        "guests" => "S-1-5-32-546",
        "power users" => "S-1-5-32-547",
        "backup operators" => "S-1-5-32-551",
        "remote desktop users" => "S-1-5-32-555",
        "network configuration operators" => "S-1-5-32-556",
        "performance monitor users" => "S-1-5-32-558",
        "performance log users" => "S-1-5-32-559",
        "distributed com users" => "S-1-5-32-562",
        "event log readers" => "S-1-5-32-573",
        "hyper-v administrators" => "S-1-5-32-578",
        "remote management users" => "S-1-5-32-580",
        _ => return None,
    })
}

/// PowerShell-выражение, возвращающее группу.
fn group_expr(name: &str) -> String {
    match group_sid(name) {
        Some(sid) => format!("(Get-LocalGroup -SID {})", q(sid)),
        None => format!("(Get-LocalGroup -Name {})", q(name)),
    }
}

/// Скрипт выводит первое несовпадение: missing / password / group:<имя>, либо ok.
fn check_script(u: &User) -> String {
    let n = q(&u.name);
    let mut s = format!("$u = Get-LocalUser -Name {n} -ErrorAction SilentlyContinue; if (-not $u) {{ 'missing'; exit }}; ");
    if u.password.is_some() {
        s += "Add-Type -AssemblyName System.DirectoryServices.AccountManagement; \
              $ctx = New-Object System.DirectoryServices.AccountManagement.PrincipalContext('Machine'); ";
        s += &format!("if (-not $ctx.ValidateCredentials({n}, $env:WINSETUP_PWD)) {{ 'password'; exit }}; ");
    }
    for g in &u.groups {
        s += &format!(
            "$g = {}; if (-not (Get-LocalGroupMember -Group $g | Where-Object {{ $_.SID -eq $u.SID }})) {{ 'group:' + {}; exit }}; ",
            group_expr(g), q(g)
        );
    }
    s + "'ok'"
}

fn apply_script(u: &User) -> String {
    let n = q(&u.name);
    let mut s = String::new();
    if u.password.is_some() {
        s += "$p = ConvertTo-SecureString $env:WINSETUP_PWD -AsPlainText -Force; ";
        s += &format!("$u = Get-LocalUser -Name {n} -ErrorAction SilentlyContinue; \
                       if ($u) {{ Set-LocalUser -Name {n} -Password $p }} else {{ $u = New-LocalUser -Name {n} -Password $p -PasswordNeverExpires }}; ");
    } else {
        s += &format!("$u = Get-LocalUser -Name {n} -ErrorAction SilentlyContinue; \
                       if (-not $u) {{ $u = New-LocalUser -Name {n} -NoPassword }}; ");
    }
    for g in &u.groups {
        s += &format!(
            "$g = {}; if (-not (Get-LocalGroupMember -Group $g | Where-Object {{ $_.SID -eq $u.SID }})) {{ Add-LocalGroupMember -Group $g -Member $u }}; ",
            group_expr(g)
        );
    }
    s
}

impl UserStep<'_> {
    fn env(&self) -> Vec<(&str, &str)> {
        self.0.password.as_deref().map(|p| vec![("WINSETUP_PWD", p)]).unwrap_or_default()
    }
}

impl Step for UserStep<'_> {
    fn describe(&self) -> String {
        if self.0.groups.is_empty() { t!("Пользователь {name}", name = self.0.name) }
        else { t!("Пользователь {name} (группы: {groups})", name = self.0.name, groups = self.0.groups.join(", ")) }
    }

    fn check(&self) -> Result<bool, String> {
        Ok(ps_env(&check_script(self.0), &self.env())? == "ok")
    }

    fn apply(&self) -> Result<(), String> {
        ps_env(&apply_script(self.0), &self.env()).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> User {
        User { name: "operator".into(), password: Some("S3cret'pwd".into()), groups: vec!["Remote Desktop Users".into(), "Мои".into()] }
    }

    #[test]
    fn sids() {
        assert_eq!(group_sid("remote desktop users"), Some("S-1-5-32-555"));
        assert_eq!(group_sid("Мои"), None);
    }

    #[test]
    fn password_never_in_script() {
        let u = user();
        for s in [check_script(&u), apply_script(&u)] {
            assert!(!s.contains("S3cret"), "{s}");
            assert!(s.contains("$env:WINSETUP_PWD"));
            assert!(s.contains("-SID 'S-1-5-32-555'") && s.contains("-Name 'Мои'"));
        }
    }

    #[test]
    fn no_password_creates_without_one() {
        let u = User { name: "kiosk".into(), password: None, groups: vec![] };
        assert!(apply_script(&u).contains("-NoPassword"));
        assert!(!check_script(&u).contains("ValidateCredentials"));
    }
}
