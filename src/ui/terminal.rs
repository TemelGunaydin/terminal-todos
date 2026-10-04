use super::{Action, App, draw};
use crate::{model::safe, store::Store};
use anyhow::{Context, Result, bail};
use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyModifiers},
    execute,
};
use std::{
    io::{self, IsTerminal},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

struct TerminalGuard {
    signals: Vec<signal_hook::SigId>,
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableBracketedPaste);
        ratatui::restore();
        for signal in self.signals.drain(..) {
            signal_hook::low_level::unregister(signal);
        }
    }
}

pub fn run(store: Store, color: bool) -> Result<i32> {
    if !io::stdin().is_terminal()
        || !io::stdout().is_terminal()
        || std::env::var("TERM").as_deref() == Ok("dumb")
    {
        bail!(
            "The dashboard requires an interactive terminal. Use `todo list` or `todo list --all` for plain output."
        );
    }
    let db = store.load()?;
    let interrupted = Arc::new(AtomicUsize::new(0));
    // Install restoration before initialization, including partial failures.
    let mut guard = TerminalGuard { signals: vec![] };
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        guard.signals.push(signal_hook::flag::register_usize(
            signal,
            interrupted.clone(),
            signal as usize,
        )?);
    }
    let mut terminal = ratatui::try_init().context("Could not open the terminal dashboard")?;
    execute!(io::stdout(), EnableBracketedPaste)?;
    let mut app = App::new(db, color);
    let mut refresh_at = Instant::now() + Duration::from_secs(1);
    let mut age_at = Instant::now() + Duration::from_secs(60);
    let mut dirty = true;
    loop {
        let signal = interrupted.load(Ordering::Relaxed);
        if signal != 0 {
            return Ok(128 + signal as i32);
        }
        if Instant::now() >= refresh_at {
            match store.load() {
                Ok(db) if db != app.db => {
                    app.replace(db, None);
                    dirty = true;
                }
                Err(error) => {
                    let message = format!("{error:#}");
                    if app.message != message {
                        app.notify(message, true);
                        dirty = true;
                    }
                }
                _ => {}
            }
            refresh_at = Instant::now() + Duration::from_secs(1);
        }
        if Instant::now() >= age_at {
            age_at = Instant::now() + Duration::from_secs(60);
            dirty = true;
        }
        if dirty {
            terminal.draw(|frame| draw(frame, &mut app))?;
            dirty = false;
        }
        if !event::poll(Duration::from_millis(80))? {
            continue;
        }
        let action = match event::read()? {
            Event::Key(key) => {
                let size = terminal.size()?;
                if size.width < 60 || size.height < 20 {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        Action::Quit(130)
                    } else if key.code == KeyCode::Char('q') {
                        Action::Quit(0)
                    } else {
                        Action::None
                    }
                } else {
                    app.key(key)
                }
            }
            Event::Paste(text) => {
                app.paste(&text);
                Action::None
            }
            Event::Resize(_, _) => Action::None,
            _ => continue,
        };
        dirty = true;
        match action {
            Action::Quit(code) => return Ok(code),
            Action::Reload => refresh_at = Instant::now(),
            Action::None => {}
            action => apply(action, &store, &mut app),
        }
    }
}

fn apply(action: Action, store: &Store, app: &mut App) {
    let added = matches!(action, Action::Add(_));
    let (result, verb) = match action {
        Action::Add(text) => (store.transact(|db| db.add(&text)), "Added"),
        Action::Edit(id, text) => (store.transact(|db| db.update(id, &text)), "Updated"),
        Action::Toggle(id) => (store.transact(|db| db.toggle(id)), "Status changed"),
        Action::Delete(id) => (store.transact(|db| db.delete(id)), "Deleted"),
        _ => return,
    };
    match result {
        Ok((db, task)) => app.saved(
            db,
            format!("{verb} #{}: {}", task.id, safe(&task.title)),
            added.then_some(task.id),
        ),
        Err(error) => app.notify(format!("{error:#}"), true),
    }
}
