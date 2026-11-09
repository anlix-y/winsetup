//! Пресеты внутри разделов: кусок конфига, который можно добавить или убрать галочкой.

use crate::config::Config;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    /// firewall | service | features | registry
    pub section: String,
    pub name: String,
    /// Строка в GUI: «RDP», «WSL»… — варианты одной группы взаимоисключающие.
    pub group: String,
    /// Описание при наведении (достаточно у одного пресета группы).
    #[serde(default)]
    pub about: Option<String>,
    pub config: Config,
}

#[derive(Deserialize)]
struct File {
    preset: Vec<Preset>,
}

pub fn builtin() -> Vec<Preset> {
    toml::from_str::<File>(include_str!("presets.toml")).expect(t!("встроенные пресеты корректны (проверено тестом)")).preset
}

fn has_all<T: PartialEq>(cfg: &[T], p: &[T]) -> bool { p.iter().all(|x| cfg.contains(x)) }

fn add_all<T: PartialEq + Clone>(cfg: &mut Vec<T>, p: &[T]) {
    for x in p { if !cfg.contains(x) { cfg.push(x.clone()); } }
}

fn remove_all<T: PartialEq>(cfg: &mut Vec<T>, p: &[T]) { cfg.retain(|x| !p.contains(x)); }

/// Опция пресета совпадает с конфигом (None в пресете — «не касается»).
fn opt_has<T: PartialEq>(cfg: &Option<T>, p: &Option<T>) -> bool { p.is_none() || cfg == p }

/// Все записи пресета уже есть в конфиге — галочка стоит.
pub fn is_on(cfg: &Config, p: &Config) -> bool {
    let fw_ok = match (&cfg.firewall, &p.firewall) {
        (_, None) => true,
        (None, Some(_)) => false,
        (Some(c), Some(f)) => opt_has(&c.domain, &f.domain) && opt_has(&c.private, &f.private)
            && opt_has(&c.public, &f.public) && has_all(&c.rule, &f.rule),
    };
    let feat_ok = match (&cfg.features, &p.features) {
        (_, None) => true,
        (None, Some(_)) => false,
        (Some(c), Some(f)) => has_all(&c.enable, &f.enable) && has_all(&c.disable, &f.disable),
    };
    fw_ok && feat_ok && has_all(&cfg.service, &p.service) && has_all(&cfg.registry, &p.registry)
}

pub fn add(cfg: &mut Config, p: &Config) {
    if let Some(f) = &p.firewall {
        let c = cfg.firewall.get_or_insert_with(Default::default);
        if f.domain.is_some() { c.domain = f.domain; }
        if f.private.is_some() { c.private = f.private; }
        if f.public.is_some() { c.public = f.public; }
        // правило с тем же именем заменяется
        c.rule.retain(|x| !f.rule.iter().any(|y| y.name == x.name));
        add_all(&mut c.rule, &f.rule);
    }
    if let Some(f) = &p.features {
        let c = cfg.features.get_or_insert_with(Default::default);
        add_all(&mut c.enable, &f.enable);
        add_all(&mut c.disable, &f.disable);
        // компонент не может быть одновременно включён и выключен — последнее решение побеждает
        c.disable.retain(|x| !f.enable.contains(x));
        c.enable.retain(|x| !f.disable.contains(x));
    }
    // та же служба / то же значение реестра с другими параметрами — заменяем (выбор «включить» снимает «выключить»)
    cfg.service.retain(|x| !p.service.iter().any(|y| y.name.eq_ignore_ascii_case(&x.name)));
    add_all(&mut cfg.service, &p.service);
    cfg.registry.retain(|x| !p.registry.iter().any(|y| same_reg(x, y)));
    add_all(&mut cfg.registry, &p.registry);
}

fn same_reg(a: &crate::config::Registry, b: &crate::config::Registry) -> bool {
    a.path.eq_ignore_ascii_case(&b.path) && a.name.eq_ignore_ascii_case(&b.name)
}

pub fn remove(cfg: &mut Config, p: &Config) {
    if let (Some(c), Some(f)) = (&mut cfg.firewall, &p.firewall) {
        if f.domain.is_some() && c.domain == f.domain { c.domain = None; }
        if f.private.is_some() && c.private == f.private { c.private = None; }
        if f.public.is_some() && c.public == f.public { c.public = None; }
        remove_all(&mut c.rule, &f.rule);
    }
    if let (Some(c), Some(f)) = (&mut cfg.features, &p.features) {
        remove_all(&mut c.enable, &f.enable);
        remove_all(&mut c.disable, &f.disable);
    }
    remove_all(&mut cfg.service, &p.service);
    remove_all(&mut cfg.registry, &p.registry);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(all: &'a [Preset], name: &str) -> &'a Config {
        &all.iter().find(|p| p.name == name).unwrap().config
    }

    #[test]
    fn builtin_valid() {
        let all = builtin();
        assert_eq!(all.len(), 28);
        for p in &all {
            assert!(crate::config::validate(&p.config).is_empty(), "{}", p.name);
            assert!(["firewall", "service", "features", "registry"].contains(&p.section.as_str()));
            assert_eq!(all.iter().filter(|x| x.group == p.group).count(), 2, "группа {} — пара", p.group);
            assert!(all.iter().any(|x| x.group == p.group && x.about.is_some()), "группа {} без описания", p.group);
        }
    }

    #[test]
    fn combine_without_duplicates() {
        let all = builtin();
        let mut cfg = Config::default();
        add(&mut cfg, find(&all, "Отключить печать"));
        add(&mut cfg, find(&all, "Отключить печать")); // повторно — без дубля
        add(&mut cfg, find(&all, "Отключить Xbox-службы"));
        assert_eq!(cfg.service.len(), 5);
        assert!(is_on(&cfg, find(&all, "Отключить печать")));
        remove(&mut cfg, find(&all, "Отключить печать"));
        assert_eq!(cfg.service.len(), 4);
        assert!(!is_on(&cfg, find(&all, "Отключить печать")));
        assert!(is_on(&cfg, find(&all, "Отключить Xbox-службы")));
    }

    #[test]
    fn opposite_unchecks() {
        let all = builtin();
        let mut cfg = Config::default();
        for (on, off) in [("Включить WSL", "Отключить WSL"), ("Открыть RDP", "Закрыть RDP"),
                          ("Включить печать", "Отключить печать"), ("Включить Cortana", "Отключить Cortana"),
                          ("Включить все профили", "Выключить все профили")] {
            add(&mut cfg, find(&all, on));
            add(&mut cfg, find(&all, off));
            assert!(is_on(&cfg, find(&all, off)), "{off}");
            assert!(!is_on(&cfg, find(&all, on)), "{on}");
        }
        assert!(validate_ok(&cfg));
        assert_eq!(cfg.firewall.as_ref().unwrap().rule.len(), 1);
    }

    fn validate_ok(cfg: &Config) -> bool { crate::config::validate(cfg).is_empty() }

    #[test]
    fn profiles_preset() {
        let all = builtin();
        let mut cfg = Config::default();
        let p = find(&all, "Включить все профили");
        assert!(!is_on(&cfg, p));
        add(&mut cfg, p);
        assert!(is_on(&cfg, p));
        remove(&mut cfg, p);
        assert_eq!(cfg.firewall.as_ref().unwrap().public, None);
    }
}
