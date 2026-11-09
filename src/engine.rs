//! Ядро: конфиг → шаги → dry-run → применение → отчёт.

use crate::config::{Config, OnError};
use crate::modules::registry::RegistryStep;
use crate::step::Step;

/// Ответ пользователя при ошибке шага (on_error = "ask").
#[derive(Debug, PartialEq, Clone, Copy)]
#[allow(dead_code)] // SHA256 отключён: Force/ForceAll сейчас нигде не выбираются
pub enum Choice { Stop, Skip, SkipAll, Force, ForceAll }

#[derive(Debug, PartialEq, Clone)]
pub enum Outcome {
    AlreadyOk,
    Applied,
    Failed(String),
    NotRun,
}

pub struct Report {
    pub results: Vec<(String, Outcome)>,
    pub reboot_needed: bool,
}

impl Report {
    pub fn has_errors(&self) -> bool {
        self.results.iter().any(|(_, o)| matches!(o, Outcome::Failed(_)))
    }

    pub fn log_text(&self) -> String {
        self.results.iter().map(|(d, o)| match o {
            Outcome::AlreadyOk => t!("[уже в порядке] {d}\n", d = d),
            Outcome::Applied => t!("[применено]     {d}\n", d = d),
            Outcome::Failed(e) => t!("[ОШИБКА]       {d}: {e}\n", d = d, e = e),
            Outcome::NotRun => t!("[не выполнено]  {d}\n", d = d),
        }).collect()
    }
}

/// Фиксированный порядок модулей: имя ПК → сеть → компоненты → службы → реестр → твики →
/// питание → Windows Update → брандмауэр → пользователи → удаление встроенных приложений → софт. Внутри модуля — как в конфиге.
pub fn build_steps(cfg: &Config) -> Vec<Box<dyn Step + '_>> {
    let mut steps: Vec<Box<dyn Step + '_>> = Vec::new();
    use crate::modules::{computer::ComputerNameStep, features::FeatureStep, services::ServiceStep};
    steps.extend(cfg.computer.iter().map(|c| Box::new(ComputerNameStep(c)) as Box<dyn Step>));
    steps.extend(cfg.network.iter().map(|n| Box::new(crate::modules::network::NetworkStep(n)) as Box<dyn Step>));
    if let Some(f) = &cfg.features {
        steps.extend(f.enable.iter().map(|n| Box::new(FeatureStep::new(n, true)) as Box<dyn Step>));
        steps.extend(f.disable.iter().map(|n| Box::new(FeatureStep::new(n, false)) as Box<dyn Step>));
    }
    steps.extend(cfg.service.iter().map(|s| Box::new(ServiceStep(s)) as Box<dyn Step>));
    steps.extend(cfg.registry.iter().map(|r| Box::new(RegistryStep::new(r.clone())) as Box<dyn Step>));
    use crate::modules::{power, tweaks};
    let boxed = |v: Vec<RegistryStep>| v.into_iter().map(|r| Box::new(r) as Box<dyn Step>);
    if let Some(t) = &cfg.tweaks { steps.extend(boxed(tweaks::tweak_steps(t))); }
    if let Some(p) = &cfg.power { steps.extend(power::steps(p)); }
    if let Some(w) = &cfg.windows_update { steps.extend(boxed(tweaks::update_steps(w))); }
    if let Some(fw) = &cfg.firewall { steps.extend(crate::modules::firewall::steps(fw)); }
    steps.extend(cfg.user.iter().map(|u| Box::new(crate::modules::users::UserStep(u)) as Box<dyn Step>));
    steps.extend(cfg.remove_apps.iter().map(|a| Box::new(crate::modules::apps::RemoveAppStep(a.clone())) as Box<dyn Step>));
    steps.extend(cfg.software.iter().map(crate::modules::software::step));
    steps
}

/// `progress(i, total, описание, None)` — шаг начат; `Some(&итог)` — шаг завершён.
pub fn apply(
    steps: &[Box<dyn Step + '_>],
    on_error: &OnError,
    ask: &mut dyn FnMut(&str, &str, bool) -> Choice,
    progress: &mut dyn FnMut(usize, usize, &str, Option<&Outcome>),
) -> Report {
    let mut results = Vec::new();
    let mut reboot_needed = false;
    let mut skip_all = *on_error == OnError::Continue;
    let mut force_all = false;
    let mut stopped = false;

    let mut run_apply = |step: &Box<dyn Step + '_>| match step.apply() {
        Ok(()) => {
            reboot_needed |= step.needs_reboot();
            Outcome::Applied
        }
        Err(e) => Outcome::Failed(e),
    };

    let total = steps.len();
    for (i, step) in steps.iter().enumerate() {
        let desc = step.describe();
        if stopped {
            progress(i + 1, total, &desc, Some(&Outcome::NotRun));
            results.push((desc, Outcome::NotRun));
            continue;
        }
        progress(i + 1, total, &desc, None);
        let mut outcome = match step.check() {
            Ok(true) => Outcome::AlreadyOk,
            Ok(false) => run_apply(step),
            Err(e) => Outcome::Failed(e),
        };
        progress(i + 1, total, &desc, Some(&outcome));
        // ошибку, которую можно обойти (неверный SHA256), после «всегда» обходим без вопроса
        if force_all && matches!(outcome, Outcome::Failed(_)) && step.can_force() {
            step.set_force();
            outcome = run_apply(step);
            progress(i + 1, total, &desc, Some(&outcome));
        }
        while let Outcome::Failed(err) = &outcome {
            if *on_error == OnError::Stop {
                stopped = true;
            } else if !skip_all {
                let forceable = step.can_force();
                match ask(&desc, err, forceable) {
                    Choice::Stop => stopped = true,
                    Choice::Skip => {}
                    Choice::SkipAll => skip_all = true,
                    c @ (Choice::Force | Choice::ForceAll) if forceable => {
                        force_all |= c == Choice::ForceAll;
                        step.set_force();
                        outcome = run_apply(step);
                        progress(i + 1, total, &desc, Some(&outcome));
                        continue; // если снова ошибка — спросить ещё раз
                    }
                    Choice::Force | Choice::ForceAll => continue, // неприменимо — переспросить
                }
            }
            break;
        }
        results.push((desc, outcome));
    }
    Report { results, reboot_needed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Fake { done: bool, fail: bool, reboot: bool, applied: Cell<bool> }

    fn fake(done: bool, fail: bool) -> Box<dyn Step> {
        Box::new(Fake { done, fail, reboot: false, applied: Cell::new(false) })
    }

    impl Step for Fake {
        fn describe(&self) -> String { format!("fake done={} fail={}", self.done, self.fail) }
        fn check(&self) -> Result<bool, String> { Ok(self.done || self.applied.get()) }
        fn apply(&self) -> Result<(), String> {
            if self.fail { return Err("сбой".into()) }
            self.applied.set(true);
            Ok(())
        }
        fn needs_reboot(&self) -> bool { self.reboot }
    }

    fn outcomes(r: &Report) -> Vec<&Outcome> { r.results.iter().map(|(_, o)| o).collect() }

    #[test]
    fn dry_run_does_not_apply() {
        let steps = vec![fake(true, false), fake(false, false)];
        // проверка (check) не меняет состояние
        assert_eq!(steps[0].check(), Ok(true));
        assert_eq!(steps[1].check(), Ok(false));
        assert_eq!(steps[1].check(), Ok(false));
    }

    #[test]
    fn idempotent_second_run() {
        let steps = vec![fake(false, false)];
        let first = apply(&steps, &OnError::Ask, &mut |_, _, _| panic!(), &mut |_, _, _, _| {});
        assert_eq!(outcomes(&first), [&Outcome::Applied]);
        let second = apply(&steps, &OnError::Ask, &mut |_, _, _| panic!(), &mut |_, _, _, _| {});
        assert_eq!(outcomes(&second), [&Outcome::AlreadyOk]);
    }

    #[test]
    fn on_error_stop() {
        let steps = vec![fake(false, true), fake(false, false)];
        let r = apply(&steps, &OnError::Stop, &mut |_, _, _| panic!("не должен спрашивать"), &mut |_, _, _, _| {});
        assert!(matches!(r.results[0].1, Outcome::Failed(_)));
        assert_eq!(r.results[1].1, Outcome::NotRun);
        assert!(r.has_errors());
    }

    #[test]
    fn on_error_continue_never_asks() {
        let steps = vec![fake(false, true), fake(false, true), fake(false, false)];
        let r = apply(&steps, &OnError::Continue, &mut |_, _, _| panic!("не должен спрашивать"), &mut |_, _, _, _| {});
        assert_eq!(r.results[2].1, Outcome::Applied);
    }

    #[test]
    fn ask_skip_all_asks_once() {
        let steps = vec![fake(false, true), fake(false, true), fake(false, false)];
        let mut asked = 0;
        let r = apply(&steps, &OnError::Ask, &mut |_, _, _| { asked += 1; Choice::SkipAll }, &mut |_, _, _, _| {});
        assert_eq!(asked, 1);
        assert_eq!(r.results[2].1, Outcome::Applied);
    }

    #[test]
    fn ask_stop() {
        let steps = vec![fake(false, true), fake(false, false)];
        let r = apply(&steps, &OnError::Ask, &mut |_, _, _| Choice::Stop, &mut |_, _, _, _| {});
        assert_eq!(r.results[1].1, Outcome::NotRun);
    }

    #[test]
    fn reboot_flag() {
        let steps: Vec<Box<dyn Step>> = vec![Box::new(Fake { done: false, fail: false, reboot: true, applied: Cell::new(false) })];
        assert!(apply(&steps, &OnError::Ask, &mut |_, _, _| Choice::Stop, &mut |_, _, _, _| {}).reboot_needed);
    }

    /// Падает «по хэшу», пока не вызван set_force.
    struct Hashy { forced: Cell<bool>, applied: Cell<bool> }
    fn hashy() -> Box<dyn Step> { Box::new(Hashy { forced: Cell::new(false), applied: Cell::new(false) }) }
    impl Step for Hashy {
        fn describe(&self) -> String { "hashy".into() }
        fn check(&self) -> Result<bool, String> { Ok(self.applied.get()) }
        fn apply(&self) -> Result<(), String> {
            if !self.forced.get() { return Err("SHA256 не совпадает".into()) }
            self.applied.set(true);
            Ok(())
        }
        fn can_force(&self) -> bool { !self.forced.get() }
        fn set_force(&self) { self.forced.set(true) }
    }

    #[test]
    fn force_installs_anyway() {
        let steps = vec![hashy()];
        let r = apply(&steps, &OnError::Ask, &mut |_, _, f| { assert!(f); Choice::Force }, &mut |_, _, _, _| {});
        assert_eq!(r.results[0].1, Outcome::Applied);
    }

    #[test]
    fn force_all_asks_once() {
        let steps = vec![hashy(), hashy()];
        let mut asked = 0;
        let r = apply(&steps, &OnError::Ask, &mut |_, _, _| { asked += 1; Choice::ForceAll }, &mut |_, _, _, _| {});
        assert_eq!(asked, 1);
        assert!(!r.has_errors());
    }

    #[test]
    fn force_ignored_for_ordinary_errors() {
        // обычная ошибка: Force неприменим → переспрос, затем Skip
        let steps = vec![fake(false, true)];
        let mut answers = vec![Choice::Skip, Choice::Force];
        let r = apply(&steps, &OnError::Ask, &mut |_, _, f| { assert!(!f); answers.pop().unwrap() }, &mut |_, _, _, _| {});
        assert!(answers.is_empty());
        assert!(r.has_errors());
    }

    #[test]
    fn continue_policy_never_forces() {
        let steps = vec![hashy()];
        let r = apply(&steps, &OnError::Continue, &mut |_, _, _| panic!(), &mut |_, _, _, _| {});
        assert!(r.has_errors());
    }
}
