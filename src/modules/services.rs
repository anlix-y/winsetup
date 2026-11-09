use super::{ps, q};
use crate::config::{Service, ServiceState, Startup};
use crate::step::Step;

pub struct ServiceStep<'a>(pub &'a Service);

fn startup_name(s: &Startup) -> &'static str {
    match s { Startup::Auto => "Automatic", Startup::Manual => "Manual", Startup::Disabled => "Disabled" }
}

fn state_name(s: &ServiceState) -> &'static str {
    match s { ServiceState::Running => "Running", ServiceState::Stopped => "Stopped" }
}

/// Ответ PowerShell `StartType|Status` → совпадает ли с желаемым.
fn matches(svc: &Service, current: &str) -> bool {
    let (start, status) = current.split_once('|').unwrap_or((current, ""));
    svc.startup.as_ref().is_none_or(|s| start == startup_name(s))
        && svc.state.as_ref().is_none_or(|s| status == state_name(s))
}

impl Step for ServiceStep<'_> {
    fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(s) = &self.0.startup { parts.push(t!("запуск {v}", v = startup_name(s))); }
        if let Some(s) = &self.0.state { parts.push(t!("состояние {v}", v = state_name(s))); }
        t!("Служба {name}: {parts}", name = self.0.name, parts = parts.join(", "))
    }

    fn check(&self) -> Result<bool, String> {
        let out = ps(&format!("$s = Get-Service -Name {}; \"$($s.StartType)|$($s.Status)\"", q(&self.0.name)))?;
        Ok(matches(self.0, &out))
    }

    fn apply(&self) -> Result<(), String> {
        let name = q(&self.0.name);
        let mut script = String::new();
        if let Some(s) = &self.0.startup {
            script += &format!("Set-Service -Name {name} -StartupType {}; ", startup_name(s));
        }
        match &self.0.state {
            Some(ServiceState::Running) => script += &format!("Start-Service -Name {name}; "),
            Some(ServiceState::Stopped) => script += &format!("Stop-Service -Name {name} -Force; "),
            None => {}
        }
        ps(&script).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_state() {
        let svc = Service { name: "Spooler".into(), startup: Some(Startup::Disabled), state: Some(ServiceState::Stopped) };
        assert!(matches(&svc, "Disabled|Stopped"));
        assert!(!matches(&svc, "Automatic|Stopped"));
        assert!(!matches(&svc, "Disabled|Running"));
        let only_state = Service { name: "x".into(), startup: None, state: Some(ServiceState::Running) };
        assert!(matches(&only_state, "Manual|Running"));
    }
}
