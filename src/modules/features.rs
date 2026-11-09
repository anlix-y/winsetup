use super::{ps, q};
use crate::step::Step;
use std::cell::Cell;

pub struct FeatureStep {
    pub name: String,
    pub enable: bool,
    reboot: Cell<bool>,
}

impl FeatureStep {
    pub fn new(name: &str, enable: bool) -> Self { Self { name: name.into(), enable, reboot: Cell::new(false) } }
}

/// Состояние из Get-WindowsOptionalFeature → уже как нужно?
fn is_done(enable: bool, state: &str) -> bool {
    if enable {
        matches!(state, "Enabled" | "EnablePending")
    } else {
        state.starts_with("Disabled") || state == "DisablePending" // Disabled, DisabledWithPayloadRemoved
    }
}

impl Step for FeatureStep {
    fn describe(&self) -> String {
        if self.enable { t!("Компонент Windows {name}: включить", name = self.name) } else { t!("Компонент Windows {name}: отключить", name = self.name) }
    }

    fn check(&self) -> Result<bool, String> {
        let state = ps(&format!("(Get-WindowsOptionalFeature -Online -FeatureName {}).State", q(&self.name)))?;
        if state.is_empty() { return Err(t!("компонент {name} не найден в этой системе", name = self.name)); }
        Ok(is_done(self.enable, &state))
    }

    fn apply(&self) -> Result<(), String> {
        let cmd = if self.enable { "Enable-WindowsOptionalFeature -All" } else { "Disable-WindowsOptionalFeature" };
        let out = ps(&format!("({cmd} -Online -FeatureName {} -NoRestart).RestartNeeded", q(&self.name)))?;
        self.reboot.set(out.eq_ignore_ascii_case("true"));
        Ok(())
    }

    fn needs_reboot(&self) -> bool { self.reboot.get() }
}

#[cfg(test)]
mod tests {
    use super::is_done;

    #[test]
    fn states() {
        assert!(is_done(true, "Enabled"));
        assert!(is_done(true, "EnablePending"));
        assert!(!is_done(true, "Disabled"));
        assert!(is_done(false, "DisabledWithPayloadRemoved"));
        assert!(!is_done(false, "Enabled"));
    }
}
