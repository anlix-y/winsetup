//! Запуск проверки/применения в фоновом потоке для GUI. Общение — через каналы.

use crate::config::Config;
use crate::engine::{self, Choice, Outcome};
use std::sync::mpsc::{channel, Receiver, Sender};

pub enum Msg {
    /// Результат проверки одного шага (план).
    Planned(String, Result<bool, String>),
    Started(usize, usize, String),
    /// Промежуточное сообщение шага («скачиваю …»).
    Note(String),
    Done(String, Outcome),
    /// Нужен ответ пользователя; `forceable` — можно ли «всё равно установить».
    Ask { desc: String, err: String, forceable: bool },
    Finished { log: String, has_errors: bool, reboot_needed: bool },
}

pub struct Run {
    pub rx: Receiver<Msg>,
    pub answer: Sender<Choice>,
}

/// `apply = false` — только план. `wake` вызывается после каждого сообщения (перерисовка окна).
pub fn start(cfg: Config, apply: bool, wake: impl Fn() + Send + Clone + 'static) -> Run {
    let (tx, rx) = channel();
    let (answer, answers) = channel::<Choice>();
    std::thread::spawn(move || {
        let send = |m: Msg| { tx.send(m).ok(); wake(); };
        let note_tx = tx.clone();
        let note_wake = wake.clone();
        crate::step::set_note_sink(Some(Box::new(move |m| { note_tx.send(Msg::Note(m.into())).ok(); note_wake(); })));
        let steps = engine::build_steps(&cfg);
        if !apply {
            for s in &steps { send(Msg::Planned(s.describe(), s.check())); }
            crate::step::set_note_sink(None);
            send(Msg::Finished { log: String::new(), has_errors: false, reboot_needed: false });
            return;
        }
        let report = engine::apply(
            &steps,
            &cfg.general.on_error,
            &mut |desc, err, forceable| {
                send(Msg::Ask { desc: desc.into(), err: err.into(), forceable });
                answers.recv().unwrap_or(Choice::Stop) // окно закрыли — останавливаемся
            },
            &mut |i, n, desc, out| match out {
                None => send(Msg::Started(i, n, desc.into())),
                Some(o) => send(Msg::Done(desc.into(), o.clone())),
            },
        );
        crate::step::set_note_sink(None);
        send(Msg::Finished { log: report.log_text(), has_errors: report.has_errors(), reboot_needed: report.reboot_needed });
    });
    Run { rx, answer }
}

