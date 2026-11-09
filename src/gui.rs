//! Графический интерфейс (egui). Запускается, когда exe открыт без аргументов.

use crate::catalog::{self, Catalog, Item};
use crate::config::{self, Config};
use crate::engine::{Choice, Outcome};
use crate::runner::{self, Msg, Run};
use eframe::egui::{self, Color32, RichText};
use std::path::PathBuf;

const GREEN: Color32 = Color32::from_rgb(110, 200, 120);
const RED: Color32 = Color32::from_rgb(235, 100, 100);
const YELLOW: Color32 = Color32::from_rgb(230, 190, 90);
const ACCENT: Color32 = Color32::from_rgb(60, 110, 200);

#[derive(Clone, Copy, PartialEq)]
enum Section { General, Software, Network, Firewall, Registry, Services, Users, Features, Power, Updates, Tweaks }

/// Функция, а не константа: подписи переводятся на лету.
fn sections() -> [(Section, &'static str); 11] { [
    (Section::General, t!("Общие")),
    (Section::Software, t!("Софт")),
    (Section::Network, t!("Сеть")),
    (Section::Firewall, t!("Брандмауэр")),
    (Section::Registry, t!("Реестр")),
    (Section::Services, t!("Службы")),
    (Section::Users, t!("Пользователи")),
    (Section::Features, t!("Компоненты Windows")),
    (Section::Power, t!("Электропитание")),
    (Section::Updates, "Windows Update"),
    (Section::Tweaks, t!("Твики")),
] }

/// Форма «своя программа».
#[derive(Default)]
struct CustomForm {
    category: String,
    name: String,
    winget: String,
    url: String,
    args: String,
    // SHA256 отключён: sha256: String,
    check_path: String,
    extract_to: String,
}

impl CustomForm {
    fn opt(s: &str) -> Option<String> { Some(s.trim().to_string()).filter(|s| !s.is_empty()) }

    fn to_item(&self) -> Item {
        Item {
            category: Self::opt(&self.category).unwrap_or_else(|| t!("Мои программы").into()),
            name: self.name.trim().into(),
            winget: Self::opt(&self.winget),
            url: Self::opt(&self.url),
            args: Self::opt(&self.args),
            // SHA256 отключён: sha256: Self::opt(&self.sha256),
            check_path: Self::opt(&self.check_path),
            extract_to: Self::opt(&self.extract_to),
        }
    }

    /// Проверка формата — те же правила, что и для конфига.
    fn errors(&self) -> Vec<String> {
        let mut e = Vec::new();
        if self.name.trim().is_empty() { e.push(t!("укажите название").into()); }
        if self.winget.contains(char::is_whitespace) { e.push(t!("winget-ID без пробелов").into()); }
        let url = self.url.trim();
        if !url.is_empty() && !crate::net::is_url(url) { e.push(t!("ссылка должна начинаться с http:// или https://").into()); }
        let cfg = Config { software: vec![self.to_item().to_software()], ..Default::default() };
        e.extend(config::validate(&cfg).into_iter().map(|s| s.replace("software #1: ", "")));
        e
    }
}

/// Строка журнала в нижней части окна.
struct Line { text: String, color: Color32 }

pub struct App {
    cfg: Config,
    path: Option<PathBuf>,
    section: Section,
    builtin: Catalog,
    presets: Vec<crate::presets::Preset>,
    user: Catalog,
    search: String,
    custom: CustomForm,
    show_custom: bool,
    custom_app: String,
    run: Option<Run>,
    /// Ждём ответа: (описание ошибки, можно ли «всё равно установить»).
    asking: Option<bool>,
    reboot_offer: bool,
    last_log: String,
    log: Vec<Line>,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        load_system_font(&cc.egui_ctx);
        let mut log = Vec::new();
        let user = catalog::load_user().unwrap_or_else(|e| {
            log.push(Line { text: t!("Свой каталог не прочитан: {e}", e = e), color: RED });
            Catalog::default()
        });
        Self {
            cfg: Config::default(), path: None, section: Section::Software,
            builtin: catalog::builtin(), presets: crate::presets::builtin(), user, search: String::new(),
            custom: CustomForm::default(), show_custom: false, custom_app: String::new(),
            run: None, asking: None, reboot_offer: false, last_log: String::new(), log,
        }
    }

    fn say(&mut self, text: impl Into<String>, color: Color32) { self.log.push(Line { text: text.into(), color }); }

    fn busy(&self) -> bool { self.run.is_some() }

    // ---------- файлы ----------

    fn open(&mut self) {
        let Some(p) = rfd::FileDialog::new().add_filter(t!("Конфиг TOML"), &["toml"]).pick_file() else { return };
        match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| config::parse(&t)) {
            Ok(cfg) => {
                self.cfg = cfg;
                self.say(t!("Открыт {path}", path = p.display()), GREEN);
                self.path = Some(p);
            }
            Err(e) => self.say(t!("Не удалось открыть {path}: {e}", path = p.display(), e = e), RED),
        }
    }

    fn save(&mut self) {
        let errs = config::validate(&self.cfg);
        if !errs.is_empty() {
            self.say(t!("Конфиг не сохранён, ошибки:\n{errors}", errors = errs.join("\n")), RED);
            return;
        }
        let mut dlg = rfd::FileDialog::new().add_filter(t!("Конфиг TOML"), &["toml"]).set_file_name("config.toml");
        if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) { dlg = dlg.set_directory(dir); }
        let Some(p) = dlg.save_file() else { return };
        match config::to_toml(&self.cfg).and_then(|t| std::fs::write(&p, t).map_err(|e| e.to_string())) {
            Ok(()) => { self.say(t!("Сохранён {path}", path = p.display()), GREEN); self.path = Some(p); }
            Err(e) => self.say(t!("Не удалось сохранить: {e}", e = e), RED),
        }
    }

    // ---------- запуск ----------

    fn start(&mut self, ctx: &egui::Context, apply: bool) {
        let errs = config::validate(&self.cfg);
        if !errs.is_empty() {
            self.say(t!("Ошибки в конфиге:\n{errors}", errors = errs.join("\n")), RED);
            return;
        }
        for w in config::warnings(&self.cfg) { self.say(format!("⚠ {w}"), YELLOW); }
        self.say(if apply { t!("— Применение —") } else { t!("— План (проверка текущего состояния) —") }, ACCENT);
        let ctx = ctx.clone();
        self.reboot_offer = false;
        self.run = Some(runner::start(self.cfg.clone(), apply, move || ctx.request_repaint()));
    }

    fn poll(&mut self) {
        let Some(run) = &self.run else { return };
        let msgs: Vec<Msg> = run.rx.try_iter().collect();
        for m in msgs {
            match m {
                Msg::Planned(d, Ok(true)) => self.say(t!("✓ уже в порядке   {d}", d = d), GREEN),
                Msg::Planned(d, Ok(false)) => self.say(t!("→ будет изменено  {d}", d = d), YELLOW),
                Msg::Planned(d, Err(e)) => self.say(t!("✗ ошибка проверки {d}: {e}", d = d, e = e), RED),
                Msg::Note(m) => self.say(format!("      {m}"), Color32::GRAY),
                Msg::Started(i, n, d) => self.say(format!("[{i}/{n}] {d}"), Color32::LIGHT_GRAY),
                Msg::Done(_, Outcome::AlreadyOk) => self.say(t!("      ✓ уже в порядке"), GREEN),
                Msg::Done(_, Outcome::Applied) => self.say(t!("      ✓ готово"), GREEN),
                Msg::Done(_, Outcome::Failed(e)) => self.say(format!("      ✗ {e}"), RED),
                Msg::Done(d, Outcome::NotRun) => self.say(t!("      — не выполнено: {d}", d = d), Color32::GRAY),
                Msg::Ask { desc, err, forceable } => {
                    self.say(t!("Ошибка: {desc}: {err}\nВыберите действие кнопкой внизу.", desc = desc, err = err), YELLOW);
                    self.asking = Some(forceable);
                }
                Msg::Finished { log, has_errors, reboot_needed } => {
                    self.run = None;
                    if !log.is_empty() {
                        self.last_log = log;
                        if has_errors {
                            // при ошибке лог сохраняется всегда
                            match crate::save_log(&self.last_log) {
                                Ok(p) => self.say(t!("Были ошибки. Лог сохранён: {p}", p = p), YELLOW),
                                Err(e) => self.say(t!("Не удалось сохранить лог: {e}", e = e), RED),
                            }
                        } else {
                            self.say(t!("Готово, ошибок нет."), GREEN);
                        }
                        self.reboot_offer = reboot_needed;
                        if reboot_needed { self.say(t!("Нужна перезагрузка."), YELLOW); }
                    }
                    break;
                }
            }
        }
    }

    fn answer(&mut self, c: Choice) {
        if let Some(run) = &self.run { run.answer.send(c).ok(); }
        self.asking = None;
    }

    // ---------- разделы ----------

    fn software_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading(t!("Софт"));
        ui.horizontal(|ui| {
            ui.label(t!("Поиск:"));
            ui.add(egui::TextEdit::singleline(&mut self.search).desired_width(240.0));
            if ui.button(t!("+ своя программа")).clicked() { self.show_custom = !self.show_custom; }
        });
        if self.show_custom { self.custom_ui(ui); }
        ui.separator();

        let q = self.search.to_lowercase();
        let items: Vec<(Item, bool)> = self.builtin.software.iter().map(|i| (i.clone(), false))
            .chain(self.user.software.iter().map(|i| (i.clone(), true)))
            .filter(|(i, _)| q.is_empty() || i.name.to_lowercase().contains(&q) || i.category.to_lowercase().contains(&q))
            .collect();
        let mut cats: Vec<&str> = items.iter().map(|(i, _)| i.category.as_str()).collect();
        cats.dedup(); // каталог отсортирован по категориям
        let mut remove_user: Option<Item> = None;

        egui::ScrollArea::vertical().id_salt("catalog").show(ui, |ui| {
            let mut seen = Vec::new();
            for cat in cats {
                if seen.contains(&cat) { continue; }
                seen.push(cat);
                ui.label(RichText::new(crate::i18n::tr(cat)).strong().color(ACCENT));
                for (item, is_user) in items.iter().filter(|(i, _)| i.category == cat) {
                    let pos = self.cfg.software.iter().position(|s| item.matches(s));
                    let mut on = pos.is_some();
                    ui.horizontal(|ui| {
                        let id = item.winget.as_deref().or(item.url.as_deref()).unwrap_or("");
                        if ui.checkbox(&mut on, crate::i18n::tr(&item.name)).on_hover_text(id).changed() {
                            match pos {
                                Some(p) => { self.cfg.software.remove(p); }
                                None => self.cfg.software.push(item.to_software()),
                            }
                        }
                        ui.label(RichText::new(id).small().color(Color32::GRAY));
                        if *is_user && ui.small_button(t!("удалить из каталога")).clicked() { remove_user = Some(item.clone()); }
                    });
                }
            }
            // программы из открытого конфига, которых нет в каталогах
            let extra: Vec<usize> = (0..self.cfg.software.len())
                .filter(|&k| !self.builtin.software.iter().chain(&self.user.software).any(|i| i.matches(&self.cfg.software[k])))
                .collect();
            ui.add_space(8.0);
            self.remove_apps_ui(ui);
            if !extra.is_empty() {
                ui.label(RichText::new(t!("Из конфига (нет в каталоге)")).strong().color(ACCENT));
                let mut drop = None;
                for k in extra {
                    let s = &self.cfg.software[k];
                    let name = s.name.clone().or(s.winget.clone()).or(s.url.clone()).unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label(&name);
                        if ui.small_button(t!("убрать")).clicked() { drop = Some(k); }
                    });
                }
                if let Some(k) = drop { self.cfg.software.remove(k); }
            }
        });

        if let Some(item) = remove_user {
            self.user.software.retain(|i| *i != item);
            self.save_user_catalog();
        }
    }

    fn remove_apps_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new(t!("Удалить встроенные приложения Windows")).strong().color(ACCENT));
        let apps: Vec<_> = self.builtin.remove.iter().chain(&self.user.remove).cloned().collect();
        ui.horizontal_wrapped(|ui| {
            for a in &apps {
                let pos = self.cfg.remove_apps.iter().position(|x| x.eq_ignore_ascii_case(&a.package));
                let mut on = pos.is_some();
                if ui.checkbox(&mut on, crate::i18n::tr(&a.name)).on_hover_text(&a.package).changed() {
                    match pos { Some(p) => { self.cfg.remove_apps.remove(p); } None => self.cfg.remove_apps.push(a.package.clone()) }
                }
            }
        });
        // свои пакеты: Get-AppxPackage | Select Name
        let extra: Vec<String> = self.cfg.remove_apps.iter()
            .filter(|x| !apps.iter().any(|a| a.package.eq_ignore_ascii_case(x))).cloned().collect();
        for x in extra {
            ui.horizontal(|ui| {
                ui.label(&x);
                if ui.small_button("✕").clicked() { self.cfg.remove_apps.retain(|y| *y != x); }
            });
        }
        ui.horizontal(|ui| {
            ui.label(t!("Своё (имя пакета):"));
            ui.add(egui::TextEdit::singleline(&mut self.custom_app).hint_text("Microsoft.WindowsCamera").desired_width(220.0));
            let name = self.custom_app.trim().to_string();
            let ok = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c));
            if !name.is_empty() && !ok { ui.colored_label(RED, t!("латиница, цифры, точки")); }
            if ui.add_enabled(ok, egui::Button::new(t!("Добавить"))).clicked() {
                if !self.cfg.remove_apps.iter().any(|x| x.eq_ignore_ascii_case(&name)) { self.cfg.remove_apps.push(name); }
                self.custom_app.clear();
            }
        });
        ui.label(RichText::new(t!("Список своих пакетов: Get-AppxPackage | Select Name")).small().color(Color32::GRAY));
    }

    fn custom_ui(&mut self, ui: &mut egui::Ui) {
        let errs = self.custom.errors();
        egui::Frame::group(ui.style()).show(ui, |ui| {
            egui::Grid::new("custom").num_columns(2).show(ui, |ui| {
                let f = &mut self.custom;
                for (label, val, hint) in [
                    (t!("Название"), &mut f.name, t!("Моя программа")),
                    (t!("Категория"), &mut f.category, t!("Мои программы")),
                    ("winget-ID", &mut f.winget, t!("Vendor.App — или ссылка ниже")),
                    (t!("Ссылка на .exe"), &mut f.url, "https://…/setup.exe"),
                    (t!("Ключи тихой установки"), &mut f.args, "/S"),
                    // SHA256 отключён: ("SHA256", &mut f.sha256, "64 hex-символа"),
                    (t!("Файл после установки"), &mut f.check_path, r"C:\Program Files\App\app.exe"),
                    (t!("Распаковать .zip в"), &mut f.extract_to, r"C:\App — для архивов вместо установщика"),
                ] {
                    ui.label(label);
                    ui.add(egui::TextEdit::singleline(val).hint_text(hint).desired_width(380.0));
                    ui.end_row();
                }
            });
            for e in &errs { ui.colored_label(RED, e); }
            if ui.add_enabled(errs.is_empty(), egui::Button::new(t!("Добавить в каталог и в конфиг"))).clicked() {
                let item = self.custom.to_item();
                self.cfg.software.push(item.to_software());
                self.user.software.push(item);
                self.save_user_catalog();
                self.custom = CustomForm::default();
                self.show_custom = false;
            }
        });
    }

    fn save_user_catalog(&mut self) {
        if let Err(e) = catalog::save_user(&self.user) { self.say(t!("Свой каталог не сохранён: {e}", e = e), RED); }
    }


    // ---------- нижняя панель ----------

    fn bottom_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            let idle = !self.busy();
            if ui.add_enabled(idle, egui::Button::new(t!("Открыть…"))).clicked() { self.open(); }
            if ui.add_enabled(idle, egui::Button::new(t!("Сохранить…"))).clicked() { self.save(); }
            ui.separator();
            if ui.add_enabled(idle, egui::Button::new(t!("Проверить"))).clicked() { self.start(ctx, false); }
            if ui.add_enabled(idle, egui::Button::new(RichText::new(t!("Применить")).strong()).fill(ACCENT)).clicked() {
                self.start(ctx, true);
            }
            if let Some(p) = &self.path { ui.label(RichText::new(p.display().to_string()).small().color(Color32::GRAY)); }
        });

        // кнопки ответа: всегда видны; неактивны, пока ответ не нужен
        ui.horizontal(|ui| {
            let ask = self.asking.is_some();
            // SHA256 отключён: let force = self.asking == Some(true);
            let btn = |text: &str, on: bool| {
                let b = egui::Button::new(text);
                if on { b.fill(Color32::from_rgb(150, 100, 30)) } else { b }
            };
            let mut choice = None;
            if ui.add_enabled(ask, btn(t!("Остановить"), ask)).clicked() { choice = Some(Choice::Stop); }
            if ui.add_enabled(ask, btn(t!("Пропустить"), ask)).clicked() { choice = Some(Choice::Skip); }
            if ui.add_enabled(ask, btn(t!("Всегда пропускать"), ask)).clicked() { choice = Some(Choice::SkipAll); }
            // SHA256 отключён: if ui.add_enabled(force, btn("Всё равно установить (небезопасно)", force)).clicked() { choice = Some(Choice::Force); }
            // SHA256 отключён: if ui.add_enabled(force, btn("Всё равно устанавливать всё", force)).clicked() { choice = Some(Choice::ForceAll); }
            if let Some(c) = choice { self.answer(c); }
            ui.separator();
            let idle_log = !self.busy() && !self.last_log.is_empty();
            if ui.add_enabled(idle_log, egui::Button::new(t!("Сохранить лог"))).clicked() {
                match crate::save_log(&self.last_log) {
                    Ok(p) => self.say(t!("Лог: {p}", p = p), GREEN),
                    Err(e) => self.say(t!("Не удалось сохранить лог: {e}", e = e), RED),
                }
            }
            let reboot = self.reboot_offer && !self.busy();
            if ui.add_enabled(reboot, btn(t!("Перезагрузить"), reboot)).clicked() {
                std::process::Command::new("shutdown").args(["/r", "/t", "0"]).status().ok();
            }
        });
    }

    fn log_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(t!("Журнал"));
            if ui.add_enabled(!self.busy(), egui::Button::new(t!("Очистить"))).clicked() { self.log.clear(); }
        });
        ui.separator();
        egui::ScrollArea::vertical().id_salt("log").stick_to_bottom(true).auto_shrink(false).show(ui, |ui| {
            for l in &self.log { ui.colored_label(l.color, &l.text); }
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        let ctx = ui.ctx().clone();

        egui::Panel::left("sections").resizable(false).exact_size(190.0).show(ui, |ui| {
            ui.add_space(6.0);
            for (s, name) in sections() {
                if ui.selectable_label(self.section == s, name).clicked() { self.section = s; }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let en = crate::i18n::is_english();
                    if ui.selectable_label(!en, "RU").clicked() { crate::i18n::set_english(false); crate::i18n::save(false); }
                    if ui.selectable_label(en, "EN").clicked() { crate::i18n::set_english(true); crate::i18n::save(true); }
                });
            });
        });
        egui::Panel::bottom("actions").show(ui, |ui| {
            ui.add_space(4.0);
            self.bottom_ui(ui, &ctx);
            ui.add_space(4.0);
        });
        // журнал справа на всю высоту, ширину можно менять мышкой
        egui::Panel::right("log").resizable(true).default_size(420.0).show(ui, |ui| self.log_ui(ui));
        egui::CentralPanel::default_margins().show(ui, |ui| {
            ui.add_enabled_ui(!self.busy(), |ui| {
                use crate::forms;
                if self.section == Section::Software { return self.software_ui(ui); }
                egui::ScrollArea::vertical().id_salt("form").auto_shrink(false).show(ui, |ui| {
                    let (cfg, p) = (&mut self.cfg, &self.presets);
                    match self.section {
                        Section::General => forms::general(ui, cfg),
                        Section::Network => forms::network(ui, cfg),
                        Section::Firewall => forms::firewall(ui, cfg, p),
                        Section::Registry => forms::registry(ui, cfg, p),
                        Section::Services => forms::services(ui, cfg, p),
                        Section::Users => forms::users(ui, cfg),
                        Section::Features => forms::features(ui, cfg, p),
                        Section::Power => forms::power(ui, cfg),
                        Section::Updates => forms::updates(ui, cfg),
                        Section::Tweaks => forms::tweaks(ui, cfg),
                        Section::Software => {}
                    }
                });
            });
        });
    }
}

/// Segoe UI из системы (кириллица) + Segoe UI Symbol (✓ ✗ → ⚠). Нет файлов — встроенный шрифт.
fn load_system_font(ctx: &egui::Context) {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let dir = PathBuf::from(windir).join("Fonts");
    let mut fonts = egui::FontDefinitions::default();
    // (файл, вставить первым?) — основной шрифт первым, символьный — запасным в конец
    for (name, file, first) in [("segoe", "segoeui.ttf", true), ("segoe-symbol", "seguisym.ttf", false)] {
        let Ok(bytes) = std::fs::read(dir.join(file)) else { continue };
        fonts.font_data.insert(name.into(), egui::FontData::from_owned(bytes).into());
        for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let list = fonts.families.entry(fam).or_default();
            if first { list.insert(0, name.into()) } else { list.push(name.into()) }
        }
    }
    ctx.set_fonts(fonts);
}

pub fn run() -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 760.0]).with_title("winsetup"),
        // wgpu (DirectX 12): работает и без видеодрайвера через программный WARP — в ВМ нет OpenGL 2.0
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native("winsetup", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
        .map_err(|e| e.to_string())
}
