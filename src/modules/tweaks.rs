//! Твики и Windows Update — это наборы записей реестра, каждый набор — один шаг.

use super::registry::{dword, RegistryStep};
use crate::config::{Tweaks, UpdateMode, WindowsUpdate};

const ADV: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const CDM: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";
const WU: &str = r"HKLM\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate";
const AU: &str = r"HKLM\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU";

pub fn tweak_steps(t: &Tweaks) -> Vec<RegistryStep> {
    let mut s = Vec::new();
    if let Some(on) = t.telemetry {
        // 0 = Security (минимум, реально соблюдается только в Enterprise/Education; в Pro/Home = 1), 3 = Full
        s.push(RegistryStep::labeled(
            if on { t!("Твик: телеметрия включена") } else { t!("Твик: телеметрия выключена") },
            vec![dword(r"HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection", "AllowTelemetry", if on { 3 } else { 0 })],
        ));
    }
    if let Some(on) = t.ads {
        let v = on as u32;
        s.push(RegistryStep::labeled(
            if on { t!("Твик: реклама включена") } else { t!("Твик: реклама выключена") },
            vec![
                dword(r"HKCU\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo", "Enabled", v),
                dword(CDM, "SilentInstalledAppsEnabled", v),
                dword(CDM, "SystemPaneSuggestionsEnabled", v),
                dword(CDM, "SubscribedContent-338388Enabled", v), // советы в «Пуске»
            ],
        ));
    }
    if let Some(on) = t.show_file_extensions {
        s.push(RegistryStep::labeled(
            if on { t!("Твик: показывать расширения файлов") } else { t!("Твик: скрывать расширения файлов") },
            vec![dword(ADV, "HideFileExt", if on { 0 } else { 1 })],
        ));
    }
    if let Some(on) = t.show_hidden_files {
        s.push(RegistryStep::labeled(
            if on { t!("Твик: показывать скрытые файлы") } else { t!("Твик: не показывать скрытые файлы") },
            vec![dword(ADV, "Hidden", if on { 1 } else { 2 })],
        ));
    }
    s
}

pub fn update_steps(w: &WindowsUpdate) -> Vec<RegistryStep> {
    let mut s = Vec::new();
    match &w.mode {
        Some(UpdateMode::Disabled) => s.push(RegistryStep::labeled(t!("Windows Update: выключено"), vec![dword(AU, "NoAutoUpdate", 1)])),
        Some(m) => {
            // AUOptions: 2 = только уведомлять, 4 = скачивать и ставить автоматически
            let (label, opt) = if *m == UpdateMode::Notify { (t!("Windows Update: только уведомлять"), 2) } else { (t!("Windows Update: автоматически"), 4) };
            s.push(RegistryStep::labeled(label, vec![dword(AU, "NoAutoUpdate", 0), dword(AU, "AUOptions", opt)]));
        }
        None => {}
    }
    if let Some(days) = w.defer_feature_days {
        s.push(RegistryStep::labeled(
            &t!("Windows Update: отложить обновления компонентов на {days} дн.", days = days),
            vec![dword(WU, "DeferFeatureUpdates", 1), dword(WU, "DeferFeatureUpdatesPeriodInDays", days as u32)],
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vals(s: &RegistryStep) -> Vec<(&str, i64)> {
        s.regs.iter().map(|r| (r.name.as_str(), r.value.as_integer().unwrap())).collect()
    }

    #[test]
    fn tweaks_map() {
        let t = Tweaks { telemetry: None, ads: Some(false), show_file_extensions: Some(true), show_hidden_files: Some(false) };
        let s = tweak_steps(&t);
        assert_eq!(s.len(), 3); // один шаг на твик
        assert_eq!(s[0].regs.len(), 4);
        assert_eq!(vals(&s[1]), [("HideFileExt", 0)]);
        assert_eq!(vals(&s[2]), [("Hidden", 2)]);
    }

    #[test]
    fn update_map() {
        let s = update_steps(&WindowsUpdate { mode: Some(UpdateMode::Notify), defer_feature_days: Some(30) });
        assert_eq!(vals(&s[0]), [("NoAutoUpdate", 0), ("AUOptions", 2)]);
        assert_eq!(vals(&s[1]), [("DeferFeatureUpdates", 1), ("DeferFeatureUpdatesPeriodInDays", 30)]);
    }
}
