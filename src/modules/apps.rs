//! Удаление встроенных приложений Windows (Appx): у всех пользователей и из образа для новых.

use super::{ps, q};
use crate::step::Step;

pub struct RemoveAppStep(pub String);

impl Step for RemoveAppStep {
    fn describe(&self) -> String { t!("Удалить встроенное приложение: {name}", name = self.0) }

    fn check(&self) -> Result<bool, String> {
        let n = q(&self.0);
        let out = ps(&format!(
            "$a = Get-AppxPackage -AllUsers -Name {n}; \
             $p = Get-AppxProvisionedPackage -Online | Where-Object DisplayName -eq {n}; \
             if ($a -or $p) {{ 'present' }} else {{ 'absent' }}"
        ))?;
        Ok(out == "absent")
    }

    fn apply(&self) -> Result<(), String> {
        let n = q(&self.0);
        // provisioned — чтобы приложение не появилось у новых пользователей
        ps(&format!(
            "Get-AppxPackage -AllUsers -Name {n} | Remove-AppxPackage -AllUsers; \
             Get-AppxProvisionedPackage -Online | Where-Object DisplayName -eq {n} | Remove-AppxProvisionedPackage -Online -AllUsers | Out-Null"
        )).map(|_| ())
    }
}
