//! Электропитание через встроенный powercfg.

use crate::config::{Power, PowerPlan};
use crate::step::Step;
use std::process::Command;

fn powercfg(args: &[&str]) -> Result<String, String> {
    let out = Command::new("powercfg").args(args).output().map_err(|e| t!("не удалось запустить powercfg: {e}", e = e))?;
    // вывод в OEM-кодировке; разбираем только GUID и hex-числа, поэтому lossy достаточно
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() { Ok(text) } else { Err(format!("powercfg: {}", text.trim())) }
}

pub struct PlanStep(PowerPlan);

impl PlanStep {
    fn guid(&self) -> &'static str {
        match self.0 {
            PowerPlan::Balanced => "381b4222-f694-41f0-9685-ff5bb260df2e",
            PowerPlan::HighPerformance => "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c",
            PowerPlan::PowerSaver => "a1841308-3541-4fab-bc81-f71556f20b4a",
        }
    }
}

impl Step for PlanStep {
    fn describe(&self) -> String {
        let name = match self.0 {
            PowerPlan::Balanced => t!("сбалансированная"),
            PowerPlan::HighPerformance => t!("высокая производительность"),
            PowerPlan::PowerSaver => t!("экономия энергии"),
        };
        t!("Электропитание: схема «{name}»", name = name)
    }
    fn check(&self) -> Result<bool, String> {
        Ok(powercfg(&["/getactivescheme"])?.to_lowercase().contains(self.guid()))
    }
    fn apply(&self) -> Result<(), String> {
        // shortcut: на ноутбуках с Modern Standby схемы кроме «сбалансированной» скрыты — будет ошибка; при необходимости дублировать схему через /duplicatescheme
        powercfg(&["/setactive", self.guid()]).map(|_| ())
    }
}

/// Таймаут от сети (AC) в минутах; 0 = никогда.
pub struct TimeoutStep {
    label: &'static str,
    sub: &'static str,      // подгруппа powercfg /query
    setting: &'static str,  // параметр powercfg /query
    change: &'static str,   // имя для powercfg /change
    minutes: u32,
}

/// В выводе `powercfg /query` последние два hex-значения — текущие AC и DC (в секундах).
/// Подписи локализованы, поэтому ищем числа, а не текст.
fn parse_ac_seconds(out: &str) -> Option<u32> {
    let vals: Vec<u32> = out.lines()
        .filter_map(|l| l.rsplit_once(": 0x").and_then(|(_, h)| u32::from_str_radix(h.trim(), 16).ok()))
        .collect();
    vals.len().checked_sub(2).map(|i| vals[i])
}

impl Step for TimeoutStep {
    fn describe(&self) -> String {
        if self.minutes == 0 { t!("Электропитание: {what} — никогда", what = crate::i18n::tr(self.label)) }
        else { t!("Электропитание: {what} — {min} мин.", what = crate::i18n::tr(self.label), min = self.minutes) }
    }
    fn check(&self) -> Result<bool, String> {
        let out = powercfg(&["/query", "SCHEME_CURRENT", self.sub, self.setting])?;
        let secs = parse_ac_seconds(&out).ok_or(t!("не удалось разобрать вывод powercfg"))?;
        Ok(secs == self.minutes * 60)
    }
    fn apply(&self) -> Result<(), String> {
        powercfg(&["/change", self.change, &self.minutes.to_string()]).map(|_| ())
    }
}

pub fn steps(p: &Power) -> Vec<Box<dyn Step>> {
    let mut s: Vec<Box<dyn Step>> = Vec::new();
    if let Some(plan) = p.plan { s.push(Box::new(PlanStep(plan))); }
    // таймауты меняются в активной схеме — поэтому идут после выбора схемы
    if let Some(m) = p.sleep_ac_minutes {
        s.push(Box::new(TimeoutStep { label: t!("сон от сети"), sub: "SUB_SLEEP", setting: "STANDBYIDLE", change: "standby-timeout-ac", minutes: m }));
    }
    if let Some(m) = p.monitor_off_ac_minutes {
        s.push(Box::new(TimeoutStep { label: t!("отключение экрана от сети"), sub: "SUB_VIDEO", setting: "VIDEOIDLE", change: "monitor-timeout-ac", minutes: m }));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::parse_ac_seconds;

    #[test]
    fn parse_query_ru() {
        let out = "  Идентификатор параметра питания: 29f6c1db-86da-48c5-9fdb-f2b67b1f44da  (Переход в спящий режим через)
      Минимальное возможное значение: 0x00000000
      Максимальное возможное значение: 0xffffffff
      Приращение возможного значения: 0x00000001
      Единицы возможного значения: Секунды
    Текущий индекс параметра питания от сети переменного тока: 0x00000708
    Текущий индекс параметра питания от сети постоянного тока: 0x00000384";
        assert_eq!(parse_ac_seconds(out), Some(1800));
        assert_eq!(parse_ac_seconds("ничего"), None);
    }
}
