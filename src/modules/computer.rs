use super::{ps, q};
use crate::config::Computer;
use crate::step::Step;

pub struct ComputerNameStep<'a>(pub &'a Computer);

impl Step for ComputerNameStep<'_> {
    fn describe(&self) -> String { t!("Имя компьютера: {name}", name = self.0.name) }

    fn check(&self) -> Result<bool, String> {
        // имя «после перезагрузки» — чтобы повторный запуск до ребута не переименовывал снова
        let pending = ps("(Get-ItemProperty 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\ComputerName\\ComputerName').ComputerName")?;
        Ok(pending.eq_ignore_ascii_case(&self.0.name))
    }

    fn apply(&self) -> Result<(), String> {
        ps(&format!("Rename-Computer -NewName {} -Force -WarningAction SilentlyContinue", q(&self.0.name))).map(|_| ())
    }

    fn needs_reboot(&self) -> bool { true }
}
