//! Формы разделов конфига для GUI.

use crate::config::*;
use crate::presets::{self, Preset};
use eframe::egui::{self, Color32, RichText, Ui};
use std::fmt::Display;
use std::net::Ipv4Addr;
use std::str::FromStr;

const RED: Color32 = Color32::from_rgb(235, 100, 100);
const ACCENT: Color32 = Color32::from_rgb(60, 110, 200);
const W: f32 = 260.0;

// ---------- поля ----------

/// Поле с разбором значения. Неверный текст остаётся в поле (красным), в конфиг попадает только верное.
fn parsed<T>(ui: &mut Ui, salt: &str, value: &mut T, width: f32, show: impl Fn(&T) -> String, parse: impl Fn(&str) -> Option<T>) {
    let id = ui.make_persistent_id(salt);
    let shown = show(value);
    // (текст в поле, значение, из которого он получен)
    let mut st: (String, String) = ui.data_mut(|d| d.get_temp(id)).unwrap_or((shown.clone(), shown.clone()));
    if st.1 != shown { st = (shown.clone(), shown); } // значение изменилось снаружи (открыт файл, пресет)
    let ok = parse(st.0.trim()).is_some();
    let mut te = egui::TextEdit::singleline(&mut st.0).desired_width(width);
    if !ok { te = te.text_color(RED); }
    let resp = ui.add(te);
    if !ok { resp.clone().on_hover_text(t!("неверное значение — не сохранится")); }
    if resp.changed() {
        if let Some(v) = parse(st.0.trim()) { *value = v; st.1 = show(value); }
    }
    ui.data_mut(|d| d.insert_temp(id, st));
}

fn opt_num<T: FromStr + Display + Clone>(ui: &mut Ui, salt: &str, v: &mut Option<T>, width: f32) {
    parsed(ui, salt, v, width,
        |v| v.as_ref().map(|x| x.to_string()).unwrap_or_default(),
        |s| if s.is_empty() { Some(None) } else { s.parse().ok().map(Some) });
}

fn opt_ip(ui: &mut Ui, salt: &str, v: &mut Option<Ipv4Addr>) { opt_num(ui, salt, v, 130.0) }

fn ip_list(ui: &mut Ui, salt: &str, v: &mut Vec<Ipv4Addr>) {
    parsed(ui, salt, v, W,
        |v| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "),
        |s| s.split(',').map(str::trim).filter(|x| !x.is_empty()).map(|x| x.parse().ok()).collect());
}

fn str_list(ui: &mut Ui, salt: &str, v: &mut Vec<String>) {
    parsed(ui, salt, v, W,
        |v| v.join(", "),
        |s| Some(s.split(',').map(str::trim).filter(|x| !x.is_empty()).map(String::from).collect()));
}

fn text(ui: &mut Ui, v: &mut String, hint: &str) {
    ui.add(egui::TextEdit::singleline(v).hint_text(hint).desired_width(W));
}

fn opt_text(ui: &mut Ui, v: &mut Option<String>, hint: &str) {
    let mut s = v.clone().unwrap_or_default();
    if ui.add(egui::TextEdit::singleline(&mut s).hint_text(hint).desired_width(W)).changed() {
        *v = Some(s).filter(|s| !s.is_empty());
    }
}

fn combo<T: PartialEq + Clone>(ui: &mut Ui, salt: &str, v: &mut T, options: &[(T, &str)]) -> egui::Response {
    let current = options.iter().find(|(o, _)| o == v).map(|(_, l)| *l).unwrap_or("?");
    egui::ComboBox::from_id_salt(ui.make_persistent_id(salt)).selected_text(current).show_ui(ui, |ui| {
        for (o, l) in options { ui.selectable_value(v, o.clone(), *l); }
    }).response
}

/// Подпись поля с описанием при наведении.
fn lbl(ui: &mut Ui, text: &str, about: &str) {
    ui.label(text).on_hover_text(about);
}

fn tri(ui: &mut Ui, salt: &str, v: &mut Option<bool>, on: &str, off: &str) {
    combo(ui, salt, v, &[(None, t!("не менять")), (Some(true), on), (Some(false), off)]);
}

/// Список записей: рисует каждую, крестик удаляет, кнопка добавляет.
fn list<T>(ui: &mut Ui, items: &mut Vec<T>, add_label: &str, new: impl Fn() -> T, mut row: impl FnMut(&mut Ui, &mut T)) {
    let mut del = None;
    for (i, item) in items.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| row(ui, item));
                    if ui.small_button("✕").on_hover_text(t!("удалить")).clicked() { del = Some(i); }
                });
            });
        });
    }
    if let Some(i) = del { items.remove(i); }
    if ui.button(add_label).clicked() { items.push(new()); }
}

/// Ошибки конфига, относящиеся к разделу (по префиксу сообщения).
fn errors(ui: &mut Ui, cfg: &Config, prefixes: &[&str]) {
    for e in validate(cfg).iter().filter(|e| prefixes.iter().any(|p| e.starts_with(p))) {
        ui.colored_label(RED, e);
    }
}

fn presets_ui(ui: &mut Ui, cfg: &mut Config, all: &[Preset], section: &str) {
    ui.label(RichText::new(t!("Готовые наборы")).strong().color(ACCENT));
    let list: Vec<&Preset> = all.iter().filter(|p| p.section == section).collect();
    let mut groups: Vec<&str> = Vec::new();
    for p in &list { if !groups.contains(&p.group.as_str()) { groups.push(&p.group); } }
    egui::Grid::new(("presets", section)).num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
        for g in groups {
            let opts: Vec<&Preset> = list.iter().copied().filter(|p| p.group == g).collect();
            let about = opts.iter().find_map(|p| p.about.as_deref()).map(crate::i18n::tr).unwrap_or("");
            let label = ui.label(crate::i18n::tr(g));
            if !about.is_empty() { label.on_hover_text(about); }
            ui.horizontal(|ui| {
                let current = opts.iter().position(|p| presets::is_on(cfg, &p.config));
                if ui.selectable_label(current.is_none(), t!("не менять")).clicked() {
                    for p in &opts { presets::remove(cfg, &p.config); }
                }
                for (i, p) in opts.iter().enumerate() {
                    // «Открыть RDP» → «Открыть»: группа уже написана слева
                    let full = crate::i18n::tr(&p.name);
                    let verb = full.split(' ').next().unwrap_or(full);
                    let hover = if about.is_empty() { full.to_string() } else { format!("{full}\n\n{about}") };
                    if ui.selectable_label(current == Some(i), verb).on_hover_text(hover).clicked() && current != Some(i) {
                        presets::add(cfg, &p.config); // заодно снимает противоположный вариант
                    }
                }
            });
            ui.end_row();
        }
    });
    ui.separator();
}

/// Опциональный раздел: редактируем копию, пустой раздел в конфиг не пишем.
fn optional<T: Default + PartialEq + Clone>(v: &mut Option<T>, f: impl FnOnce(&mut T)) {
    let mut x = v.clone().unwrap_or_default();
    f(&mut x);
    *v = (x != T::default()).then_some(x);
}

// ---------- разделы ----------

pub fn general(ui: &mut Ui, cfg: &mut Config) {
    ui.heading(t!("Общие"));
    egui::Grid::new("general").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        lbl(ui, t!("Название конфига"), t!("Для себя: под каким именем узнать этот конфиг. На настройку не влияет."));
        opt_text(ui, &mut cfg.general.name, t!("Офисный ПК"));
        ui.end_row();
        lbl(ui, t!("При ошибке шага"), t!("Что делать, если шаг не удался: спросить вас, пропустить его и продолжить или остановиться."));
        combo(ui, "on_error", &mut cfg.general.on_error,
            &[(OnError::Ask, t!("спрашивать")), (OnError::Continue, t!("продолжать")), (OnError::Stop, t!("останавливаться"))]);
        ui.end_row();
        lbl(ui, t!("Перезагрузка, если нужна"), t!("Некоторые изменения (имя ПК, компоненты) вступают в силу только после перезагрузки."));
        combo(ui, "reboot", &mut cfg.general.reboot,
            &[(Reboot::Ask, t!("спрашивать")), (Reboot::Always, t!("сразу")), (Reboot::Never, t!("никогда"))]);
        ui.end_row();
        lbl(ui, t!("Имя компьютера"), t!("Сетевое имя ПК: до 15 латинских букв, цифр и дефисов. Нужна перезагрузка."));
        let mut name = cfg.computer.as_ref().map(|c| c.name.clone()).unwrap_or_default();
        if ui.add(egui::TextEdit::singleline(&mut name).hint_text(t!("не менять")).desired_width(W)).changed() {
            cfg.computer = (!name.is_empty()).then_some(Computer { name });
        }
        ui.end_row();
    });
    errors(ui, cfg, &["computer"]);
}

pub fn network(ui: &mut Ui, cfg: &mut Config) {
    ui.heading(t!("Сеть"));
    ui.label(RichText::new(t!("Имя адаптера — из Get-NetAdapter (обычно «Ethernet»). Неверный IP оборвёт связь с ПК.")).small());
    list(ui, &mut cfg.network, t!("+ адаптер"),
        || Network { adapter: "Ethernet".into(), dhcp: true, ip: None, prefix: None, gateway: None, dns: vec![] },
        |ui, n| {
            egui::Grid::new("net").num_columns(2).show(ui, |ui| {
                lbl(ui, t!("Адаптер"), t!("Имя сетевого адаптера, как в «Сетевых подключениях» или Get-NetAdapter.")); text(ui, &mut n.adapter, "Ethernet"); ui.end_row();
                lbl(ui, t!("Адрес"), t!("DHCP — адрес выдаёт роутер автоматически. Без DHCP — статический адрес, заданный ниже.")); ui.checkbox(&mut n.dhcp, t!("получать по DHCP")); ui.end_row();
                if !n.dhcp {
                    lbl(ui, t!("IP / префикс"), t!("Статический адрес и длина маски: 24 = 255.255.255.0."));
                    ui.horizontal(|ui| { opt_ip(ui, "ip", &mut n.ip); ui.label("/"); opt_num(ui, "prefix", &mut n.prefix, 40.0); });
                    ui.end_row();
                    lbl(ui, t!("Шлюз"), t!("Адрес роутера — выход в другие сети и Интернет.")); opt_ip(ui, "gw", &mut n.gateway); ui.end_row();
                }
                lbl(ui, t!("DNS (через запятую)"), t!("Серверы имён, например 1.1.1.1, 8.8.8.8. Пусто — не менять (при DHCP — получать автоматически).")); ip_list(ui, "dns", &mut n.dns); ui.end_row();
            });
        });
    errors(ui, cfg, &["network"]);
}

pub fn firewall(ui: &mut Ui, cfg: &mut Config, all: &[Preset]) {
    ui.heading(t!("Брандмауэр"));
    presets_ui(ui, cfg, all, "firewall");
    optional(&mut cfg.firewall, |fw| {
        egui::Grid::new("profiles").num_columns(2).show(ui, |ui| {
            lbl(ui, t!("Доменный профиль"), t!("Действует, когда ПК в сети домена Active Directory.")); tri(ui, "d", &mut fw.domain, t!("включить"), t!("выключить")); ui.end_row();
            lbl(ui, t!("Частный профиль"), t!("Действует в доверенных сетях: дом, офис.")); tri(ui, "p", &mut fw.private, t!("включить"), t!("выключить")); ui.end_row();
            lbl(ui, t!("Общий профиль"), t!("Действует в публичных сетях: кафе, вокзал. Самый строгий.")); tri(ui, "u", &mut fw.public, t!("включить"), t!("выключить")); ui.end_row();
        });
        ui.label(RichText::new(t!("Правила")).strong());
        list(ui, &mut fw.rule, t!("+ правило"),
            || FirewallRule { name: String::new(), direction: Direction::In, action: Action::Allow, protocol: Protocol::Tcp, port: None },
            |ui, r| {
                ui.horizontal(|ui| { lbl(ui, t!("Имя"), t!("Имя правила в брандмауэре Windows. По нему правило находится при повторном запуске.")); text(ui, &mut r.name, "RDP"); });
                ui.horizontal(|ui| {
                    combo(ui, "dir", &mut r.direction, &[(Direction::In, t!("входящие")), (Direction::Out, t!("исходящие"))]).on_hover_text(t!("Входящие — подключения к этому ПК, исходящие — из него наружу."));
                    combo(ui, "act", &mut r.action, &[(Action::Allow, t!("разрешить")), (Action::Block, t!("блокировать"))]).on_hover_text(t!("Разрешить или заблокировать подходящий трафик."));
                    combo(ui, "proto", &mut r.protocol,
                        &[(Protocol::Tcp, "TCP"), (Protocol::Udp, "UDP"), (Protocol::Icmp, "ICMP (ping)"), (Protocol::Any, t!("любой"))]).on_hover_text(t!("TCP/UDP — с портом; ICMP — ping; «любой» — весь трафик."));
                    lbl(ui, t!("порт"), t!("Номер порта, например 3389 для RDP. Пусто — все порты.")); opt_num(ui, "port", &mut r.port, 60.0);
                });
            });
    });
    errors(ui, cfg, &["firewall"]);
}

pub fn registry(ui: &mut Ui, cfg: &mut Config, all: &[Preset]) {
    ui.heading(t!("Реестр"));
    presets_ui(ui, cfg, all, "registry");
    list(ui, &mut cfg.registry, t!("+ значение"),
        || Registry { path: r"HKLM\SOFTWARE\".into(), name: String::new(), kind: RegType::Dword, value: 0.into() },
        |ui, r| {
            egui::Grid::new("reg").num_columns(2).show(ui, |ui| {
                lbl(ui, t!("Раздел"), t!("Путь к разделу: HKLM\\… — для всего ПК, HKCU\\… — для текущего пользователя.")); text(ui, &mut r.path, r"HKLM\SOFTWARE\…"); ui.end_row();
                lbl(ui, t!("Имя"), t!("Имя параметра внутри раздела.")); text(ui, &mut r.name, "AllowTelemetry"); ui.end_row();
                lbl(ui, t!("Тип"), t!("DWORD — число (флаги 0/1), строка — текст, список строк — через «;»."));
                let before = r.kind.clone();
                combo(ui, "kind", &mut r.kind, &[
                    (RegType::Dword, "DWORD"), (RegType::Qword, "QWORD"), (RegType::String, t!("строка")),
                    (RegType::ExpandString, t!("строка с %переменными%")), (RegType::MultiString, t!("список строк")),
                ]);
                if r.kind != before { r.value = default_value(&r.kind); }
                ui.end_row();
                lbl(ui, t!("Значение"), t!("Что записать. Неверное значение подсвечивается красным."));
                let kind = r.kind.clone();
                parsed(ui, "value", &mut r.value, W, |v| show_value(v), |s| parse_value(&kind, s));
                ui.end_row();
            });
        });
    errors(ui, cfg, &["registry"]);
}

fn default_value(k: &RegType) -> toml::Value {
    match k {
        RegType::Dword | RegType::Qword => 0.into(),
        RegType::String | RegType::ExpandString => "".into(),
        RegType::MultiString => toml::Value::Array(vec![]),
    }
}

fn show_value(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        toml::Value::Array(a) => a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("; "),
        other => other.to_string(),
    }
}

/// Список строк вводится через «;».
fn parse_value(k: &RegType, s: &str) -> Option<toml::Value> {
    match k {
        RegType::Dword => s.parse::<u32>().ok().map(|n| (n as i64).into()),
        RegType::Qword => s.parse::<i64>().ok().filter(|n| *n >= 0).map(Into::into),
        RegType::String | RegType::ExpandString => Some(s.into()),
        RegType::MultiString => Some(toml::Value::Array(
            s.split(';').map(str::trim).filter(|x| !x.is_empty()).map(|x| x.into()).collect(),
        )),
    }
}

pub fn services(ui: &mut Ui, cfg: &mut Config, all: &[Preset]) {
    ui.heading(t!("Службы"));
    presets_ui(ui, cfg, all, "service");
    list(ui, &mut cfg.service, t!("+ служба"),
        || Service { name: String::new(), startup: None, state: None },
        |ui, s| {
            ui.horizontal(|ui| {
                lbl(ui, t!("Имя службы"), t!("Короткое имя службы (не отображаемое): Spooler, wuauserv. Список — Get-Service.")); text(ui, &mut s.name, "Spooler");
                combo(ui, "startup", &mut s.startup, &[
                    (None, t!("запуск: не менять")), (Some(Startup::Auto), t!("автоматически")),
                    (Some(Startup::Manual), t!("вручную")), (Some(Startup::Disabled), t!("отключена")),
                ]).on_hover_text(t!("Тип запуска: автоматически — при старте Windows, вручную — по требованию, отключена — не запускается."));
                combo(ui, "state", &mut s.state, &[
                    (None, t!("состояние: не менять")), (Some(ServiceState::Running), t!("запущена")), (Some(ServiceState::Stopped), t!("остановлена")),
                ]).on_hover_text(t!("Запустить или остановить службу сейчас."));
            });
        });
    errors(ui, cfg, &["service"]);
}

pub fn users(ui: &mut Ui, cfg: &mut Config) {
    ui.heading(t!("Пользователи"));
    ui.label(RichText::new(t!("Группы — английскими именами (Administrators, Users, Remote Desktop Users…), программа найдёт их на любой языковой версии Windows.")).small());
    list(ui, &mut cfg.user, t!("+ пользователь"),
        || User { name: String::new(), password: None, groups: vec![] },
        |ui, u| {
            egui::Grid::new("user").num_columns(2).show(ui, |ui| {
                lbl(ui, t!("Имя"), t!("Имя локальной учётной записи. Нет такой — будет создана.")); text(ui, &mut u.name, "operator"); ui.end_row();
                lbl(ui, t!("Пароль"), t!("Ставится при каждом запуске, если не совпадает. Хранится в конфиге открытым текстом!"));
                let mut p = u.password.clone().unwrap_or_default();
                if ui.add(egui::TextEdit::singleline(&mut p).password(true).hint_text(t!("без пароля")).desired_width(W)).changed() {
                    u.password = Some(p).filter(|p| !p.is_empty());
                }
                ui.end_row();
                lbl(ui, t!("Группы (через запятую)"), t!("Administrators — администраторы, Users — обычные, Remote Desktop Users — вход по RDP.")); str_list(ui, "groups", &mut u.groups); ui.end_row();
            });
        });
}

pub fn features(ui: &mut Ui, cfg: &mut Config, all: &[Preset]) {
    ui.heading(t!("Компоненты Windows"));
    presets_ui(ui, cfg, all, "features");
    ui.label(RichText::new(t!("Имена — из Get-WindowsOptionalFeature -Online.")).small());
    optional(&mut cfg.features, |f| {
        egui::Grid::new("feat").num_columns(2).show(ui, |ui| {
            lbl(ui, t!("Включить"), t!("Имена компонентов через запятую, например NetFx3, TelnetClient. Может потребоваться перезагрузка.")); str_list(ui, "en", &mut f.enable); ui.end_row();
            lbl(ui, t!("Отключить"), t!("Имена компонентов через запятую, например SMB1Protocol.")); str_list(ui, "dis", &mut f.disable); ui.end_row();
        });
    });
}

pub fn power(ui: &mut Ui, cfg: &mut Config) {
    ui.heading(t!("Электропитание"));
    optional(&mut cfg.power, |p| {
        egui::Grid::new("power").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            lbl(ui, t!("Схема"), t!("Сбалансированная — обычный режим, высокая производительность — без экономии, экономия — для ноутбуков."));
            combo(ui, "plan", &mut p.plan, &[
                (None, t!("не менять")), (Some(PowerPlan::Balanced), t!("сбалансированная")),
                (Some(PowerPlan::HighPerformance), t!("высокая производительность")), (Some(PowerPlan::PowerSaver), t!("экономия энергии")),
            ]);
            ui.end_row();
            lbl(ui, t!("Сон от сети, мин (0 — никогда)"), t!("Через сколько минут бездействия ПК уснёт при работе от сети.")); opt_num(ui, "sleep", &mut p.sleep_ac_minutes, 60.0); ui.end_row();
            lbl(ui, t!("Отключение экрана, мин (0 — никогда)"), t!("Через сколько минут бездействия погаснет экран.")); opt_num(ui, "mon", &mut p.monitor_off_ac_minutes, 60.0); ui.end_row();
        });
    });
}

pub fn updates(ui: &mut Ui, cfg: &mut Config) {
    ui.heading("Windows Update");
    optional(&mut cfg.windows_update, |w| {
        egui::Grid::new("wu").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            lbl(ui, t!("Режим"), t!("Автоматически — Windows сама ставит обновления, уведомлять — только сообщает, выключено — не проверяет (небезопасно)."));
            combo(ui, "mode", &mut w.mode, &[
                (None, t!("не менять")), (Some(UpdateMode::Auto), t!("автоматически")),
                (Some(UpdateMode::Notify), t!("только уведомлять")), (Some(UpdateMode::Disabled), t!("выключено")),
            ]);
            ui.end_row();
            lbl(ui, t!("Отложить обновления компонентов, дней"), t!("На сколько дней отложить крупные обновления Windows (до 365). Исправления безопасности приходят как обычно.")); opt_num(ui, "days", &mut w.defer_feature_days, 60.0); ui.end_row();
        });
    });
    errors(ui, cfg, &["windows_update"]);
}

pub fn tweaks(ui: &mut Ui, cfg: &mut Config) {
    ui.heading(t!("Твики"));
    optional(&mut cfg.tweaks, |t| {
        egui::Grid::new("tw").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            lbl(ui, t!("Телеметрия"), t!("Отправка диагностических данных в Microsoft.")); tri(ui, "tel", &mut t.telemetry, t!("включить"), t!("выключить")); ui.end_row();
            lbl(ui, t!("Реклама и советы"), t!("Рекламный ID, предложения приложений и советы в «Пуске».")); tri(ui, "ads", &mut t.ads, t!("включить"), t!("выключить")); ui.end_row();
            lbl(ui, t!("Расширения файлов"), t!("Показывать .exe, .txt и т.п. в Проводнике — помогает заметить вирусы вида «фото.jpg.exe».")); tri(ui, "ext", &mut t.show_file_extensions, t!("показывать"), t!("скрывать")); ui.end_row();
            lbl(ui, t!("Скрытые файлы"), t!("Показывать скрытые файлы и папки в Проводнике.")); tri(ui, "hid", &mut t.show_hidden_files, t!("показывать"), t!("скрывать")); ui.end_row();
        });
    });
    ui.label(RichText::new(t!("Телеметрия полностью выключается только в Enterprise/Education; в Home/Pro — до минимума.")).small());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_values() {
        assert_eq!(parse_value(&RegType::Dword, "42"), Some(42.into()));
        assert_eq!(parse_value(&RegType::Dword, "-1"), None);
        assert_eq!(parse_value(&RegType::Dword, "4294967296"), None);
        let m = parse_value(&RegType::MultiString, "один; два").unwrap();
        assert_eq!(show_value(&m), "один; два");
    }
}
