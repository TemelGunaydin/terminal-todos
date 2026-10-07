mod clipboard;
mod input;
mod render;
mod terminal;

use crate::model::{Database, Task};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use input::Editor;
use ratatui::widgets::ListState;
pub use render::draw;
use std::collections::BTreeSet;
pub use terminal::run;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Open,
    Done,
    All,
}
impl Filter {
    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Done => "DONE",
            Self::All => "ALL",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tasks,
    Details,
}
#[derive(Clone, Debug)]
pub enum InputKind {
    Add,
    Edit(u64),
    Search,
    Project,
}
#[derive(Clone, Debug)]
pub enum Mode {
    Normal,
    Input {
        kind: InputKind,
        editor: Editor,
        original_query: String,
    },
    ConfirmDelete(Task),
    ProjectPicker {
        projects: Vec<String>,
        selected: usize,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Quit(i32),
    Reload,
    Add(String),
    Edit(u64, String),
    Toggle(u64),
    Delete(u64),
    Copy { id: u64, text: String },
}

pub struct App {
    pub db: Database,
    pub list: ListState,
    pub filter: Filter,
    pub query: String,
    pub focus: Focus,
    pub mode: Mode,
    pub message: String,
    pub error: bool,
    pub color: bool,
    pub project: Option<String>,
    default_project: Option<String>,
    pub page_size: usize,
    pub detail_scroll: u16,
    pub detail_max: u16,
    pub detail_page: u16,
}

impl App {
    pub fn new(db: Database, color: bool, project: Option<String>) -> Self {
        let mut app = Self {
            db,
            list: ListState::default(),
            filter: Filter::Open,
            query: String::new(),
            focus: Focus::Tasks,
            mode: Mode::Normal,
            message: "Ready · changes save automatically".into(),
            error: false,
            color,
            default_project: project.clone(),
            project,
            page_size: 1,
            detail_scroll: 0,
            detail_max: 0,
            detail_page: 1,
        };
        app.select(0);
        app
    }

    fn project_names(&self) -> Vec<String> {
        self.db
            .tasks
            .iter()
            .filter_map(|task| task.project.clone())
            .chain(self.default_project.clone())
            .chain(self.project.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn select_project(&mut self, project: String) {
        self.notify(format!("New tasks use project: {project}"), false);
        self.project = Some(project);
    }

    pub fn visible(&self) -> Vec<&Task> {
        let query = self.query.to_lowercase();
        self.db
            .tasks
            .iter()
            .filter(|task| {
                (match self.filter {
                    Filter::Open => !task.completed,
                    Filter::Done => task.completed,
                    Filter::All => true,
                }) && task.title.to_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn selected(&self) -> Option<&Task> {
        self.list
            .selected()
            .and_then(|i| self.visible().get(i).copied())
    }

    pub fn replace(&mut self, db: Database, preferred_id: Option<u64>) {
        let previous_id = self.selected().map(|task| task.id);
        let id = preferred_id.or(previous_id);
        let old_index = self.list.selected().unwrap_or(0);
        self.db = db;
        self.reselect(id, old_index);
        if self.selected().map(|task| task.id) != previous_id {
            self.detail_scroll = 0;
        }
    }

    fn reselect(&mut self, id: Option<u64>, fallback: usize) {
        let index = id
            .and_then(|id| self.visible().iter().position(|task| task.id == id))
            .unwrap_or(fallback);
        self.select(index);
    }

    fn select(&mut self, index: usize) {
        let length = self.visible().len();
        let selected = if length == 0 {
            None
        } else {
            Some(index.min(length - 1))
        };
        if self.list.selected() != selected {
            self.detail_scroll = 0;
        }
        self.list.select(selected);
    }

    fn set_query(&mut self, query: String) {
        let id = self.selected().map(|task| task.id);
        self.query = query;
        self.detail_scroll = 0;
        self.reselect(id, 0);
    }

    pub fn notify(&mut self, message: String, error: bool) {
        self.message = message;
        self.error = error;
    }

    pub fn saved(&mut self, db: Database, message: String, added_id: Option<u64>) {
        self.mode = Mode::Normal;
        if added_id.is_some() {
            self.filter = Filter::Open;
            self.query.clear();
        }
        self.detail_scroll = 0;
        self.replace(db, added_id);
        self.notify(message, false);
    }

    pub fn paste(&mut self, text: &str) {
        let mut query = None;
        if let Mode::Input { kind, editor, .. } = &mut self.mode {
            editor.insert(text);
            if matches!(kind, InputKind::Search) {
                query = Some(editor.text.clone());
            }
        }
        if let Some(query) = query {
            self.set_query(query);
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Action {
        if key.kind == KeyEventKind::Release {
            return Action::None;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit(130);
        }
        // Holding an action key must not repeatedly toggle or submit a task.
        if key.kind == KeyEventKind::Repeat
            && !matches!(
                key.code,
                KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::PageUp
                    | KeyCode::PageDown
                    | KeyCode::Backspace
                    | KeyCode::Delete
            )
            && (matches!(self.mode, Mode::Normal | Mode::ConfirmDelete(_))
                || matches!(self.mode, Mode::ProjectPicker { .. })
                || key.code == KeyCode::Enter)
        {
            return Action::None;
        }

        let mode = std::mem::replace(&mut self.mode, Mode::Normal);
        match mode {
            Mode::Input {
                kind,
                mut editor,
                original_query,
            } => {
                let action = match key.code {
                    KeyCode::Esc => {
                        if matches!(kind, InputKind::Search) {
                            self.set_query(original_query);
                        }
                        self.notify("Cancelled".into(), false);
                        return Action::None;
                    }
                    KeyCode::Enter => match kind {
                        InputKind::Add => Action::Add(editor.text.clone()),
                        InputKind::Edit(id) => Action::Edit(id, editor.text.clone()),
                        InputKind::Search => {
                            self.notify("Search applied · Esc clears the search".into(), false);
                            return Action::None;
                        }
                        InputKind::Project => {
                            match crate::model::valid_project(editor.text.trim()) {
                                Ok(project) => {
                                    self.select_project(project);
                                    return Action::None;
                                }
                                Err(error) => self.notify(error.to_string(), true),
                            }
                            Action::None
                        }
                    },
                    _ => {
                        editor.key(key);
                        if matches!(kind, InputKind::Search) {
                            self.set_query(editor.text.clone());
                        }
                        Action::None
                    }
                };
                self.mode = Mode::Input {
                    kind,
                    editor,
                    original_query,
                };
                action
            }
            Mode::ConfirmDelete(task) => match key.code {
                KeyCode::Esc | KeyCode::Char('n' | 'N') => {
                    self.notify("Deletion cancelled".into(), false);
                    Action::None
                }
                KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                    let id = task.id;
                    self.mode = Mode::ConfirmDelete(task);
                    Action::Delete(id)
                }
                _ => {
                    self.mode = Mode::ConfirmDelete(task);
                    Action::None
                }
            },
            Mode::ProjectPicker {
                projects,
                mut selected,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.notify("Project selection cancelled".into(), false);
                        return Action::None;
                    }
                    KeyCode::Enter => {
                        if let Some(project) = projects.get(selected) {
                            self.select_project(project.clone());
                        } else {
                            self.notify(
                                "Enter a project name · Enter selects, Esc cancels".into(),
                                false,
                            );
                            self.mode = Mode::Input {
                                kind: InputKind::Project,
                                editor: Editor::new(String::new()),
                                original_query: String::new(),
                            };
                        }
                        return Action::None;
                    }
                    KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => {
                        selected = (selected + 1).min(projects.len())
                    }
                    KeyCode::Home => selected = 0,
                    KeyCode::End => selected = projects.len(),
                    _ => {}
                }
                self.mode = Mode::ProjectPicker { projects, selected };
                Action::None
            }
            Mode::Normal => self.normal_key(key),
        }
    }

    fn normal_key(&mut self, key: KeyEvent) -> Action {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return Action::None;
        }
        match key.code {
            KeyCode::Char('q') => return Action::Quit(0),
            KeyCode::Char('r') => return Action::Reload,
            KeyCode::Char('c') => {
                if let Some(task) = self.selected() {
                    return Action::Copy {
                        id: task.id,
                        text: task.title.clone(),
                    };
                }
            }
            KeyCode::Char('p') => {
                let projects = self.project_names();
                let selected = self
                    .project
                    .as_ref()
                    .and_then(|project| projects.iter().position(|name| name == project))
                    .unwrap_or(0);
                self.notify(
                    "Select the project for new tasks · no existing tasks change".into(),
                    false,
                );
                self.mode = Mode::ProjectPicker { projects, selected };
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = if self.focus == Focus::Tasks {
                    Focus::Details
                } else {
                    Focus::Tasks
                }
            }
            KeyCode::Char('a') => {
                self.notify("Adding a task · Enter saves, Esc cancels".into(), false);
                self.mode = Mode::Input {
                    kind: InputKind::Add,
                    editor: Editor::new(String::new()),
                    original_query: String::new(),
                }
            }
            KeyCode::Char('e') => {
                if let Some(task) = self.selected().cloned() {
                    self.notify(format!("Editing #{}", task.id), false);
                    self.mode = Mode::Input {
                        kind: InputKind::Edit(task.id),
                        editor: Editor::new(crate::model::safe(&task.title)),
                        original_query: String::new(),
                    };
                }
            }
            KeyCode::Char('d') => {
                if let Some(task) = self.selected().cloned() {
                    self.notify("Confirm deletion or press Esc to cancel".into(), false);
                    self.mode = Mode::ConfirmDelete(task);
                }
            }
            KeyCode::Char(' ') => {
                if let Some(task) = self.selected() {
                    return Action::Toggle(task.id);
                }
            }
            KeyCode::Char('/') => {
                self.notify("Searching task titles".into(), false);
                self.mode = Mode::Input {
                    kind: InputKind::Search,
                    editor: Editor::new(self.query.clone()),
                    original_query: self.query.clone(),
                }
            }
            KeyCode::Esc => self.set_query(String::new()),
            KeyCode::Char('1' | '2' | '3') => {
                let id = self.selected().map(|task| task.id);
                self.filter = match key.code {
                    KeyCode::Char('1') => Filter::Open,
                    KeyCode::Char('2') => Filter::Done,
                    _ => Filter::All,
                };
                self.detail_scroll = 0;
                self.reselect(id, 0);
            }
            code if self.focus == Focus::Details => {
                self.detail_scroll = match code {
                    KeyCode::Up | KeyCode::Char('k') => self.detail_scroll.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => self.detail_scroll.saturating_add(1),
                    KeyCode::PageUp => self.detail_scroll.saturating_sub(self.detail_page),
                    KeyCode::PageDown => self.detail_scroll.saturating_add(self.detail_page),
                    KeyCode::Home => 0,
                    KeyCode::End => self.detail_max,
                    _ => self.detail_scroll,
                }
                .min(self.detail_max);
            }
            code => {
                let index = self.list.selected().unwrap_or(0);
                let index = match code {
                    KeyCode::Up | KeyCode::Char('k') => index.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => index.saturating_add(1),
                    KeyCode::PageUp => index.saturating_sub(self.page_size),
                    KeyCode::PageDown => index.saturating_add(self.page_size),
                    KeyCode::Home => 0,
                    KeyCode::End => self.visible().len().saturating_sub(1),
                    _ => index,
                };
                self.select(index);
            }
        }
        Action::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn app() -> Result<App> {
        let mut db = Database::default();
        db.add("First task")?;
        db.add("Türkçe görev")?;
        db.add("Last task")?;
        Ok(App::new(db, true, None))
    }

    #[test]
    fn navigation_clamps_and_selection_survives_external_deletion() -> Result<()> {
        let mut app = app()?;
        app.key(key(KeyCode::Up));
        assert_eq!(app.selected().unwrap().id, 1);
        app.key(key(KeyCode::Down));
        let mut db = app.db.clone();
        db.delete(1)?;
        app.replace(db, None);
        assert_eq!(app.selected().unwrap().id, 2);
        app.key(key(KeyCode::End));
        app.key(key(KeyCode::Down));
        assert_eq!(app.selected().unwrap().id, 3);
        Ok(())
    }

    #[test]
    fn filtering_after_completion_selects_the_next_real_id() -> Result<()> {
        let mut app = app()?;
        assert_eq!(app.key(key(KeyCode::Char(' '))), Action::Toggle(1));
        let mut db = app.db.clone();
        db.toggle(1)?;
        app.saved(db, "Completed".into(), None);
        assert_eq!(app.selected().unwrap().id, 2);
        app.key(key(KeyCode::Char('2')));
        assert_eq!(app.selected().unwrap().id, 1);
        Ok(())
    }

    #[test]
    fn input_does_not_trigger_shortcuts_and_search_escape_restores_filter() -> Result<()> {
        let mut app = app()?;
        app.key(key(KeyCode::Char('a')));
        for ch in "add quit task".chars() {
            assert_eq!(app.key(key(KeyCode::Char(ch))), Action::None);
        }
        assert_eq!(
            app.key(key(KeyCode::Enter)),
            Action::Add("add quit task".into())
        );
        app.key(key(KeyCode::Esc));
        app.key(key(KeyCode::Char('/')));
        app.paste("Türkçe");
        assert_eq!(app.visible().len(), 1);
        app.key(key(KeyCode::Esc));
        assert_eq!(app.visible().len(), 3);
        assert!(app.query.is_empty());
        Ok(())
    }

    #[test]
    fn delete_requires_confirmation_and_empty_navigation_is_safe() {
        let mut app = App::new(Database::default(), false, None);
        for code in [
            KeyCode::Down,
            KeyCode::End,
            KeyCode::Char(' '),
            KeyCode::Char('d'),
        ] {
            assert_eq!(app.key(key(code)), Action::None);
        }
        assert!(app.selected().is_none());
        app.db.add("Keep me").unwrap();
        app.select(0);
        assert_eq!(app.key(key(KeyCode::Char('d'))), Action::None);
        app.key(key(KeyCode::Esc));
        assert_eq!(app.db.tasks.len(), 1);
        app.key(key(KeyCode::Char('d')));
        assert_eq!(app.key(key(KeyCode::Enter)), Action::Delete(1));
    }

    #[test]
    fn external_deletion_resets_details_even_when_list_index_stays_the_same() -> Result<()> {
        let mut app = app()?;
        app.detail_scroll = 8;
        let mut db = app.db.clone();
        db.delete(1)?;
        app.replace(db, None);
        assert_eq!(app.list.selected(), Some(0));
        assert_eq!(app.selected().unwrap().id, 2);
        assert_eq!(app.detail_scroll, 0);
        Ok(())
    }

    #[test]
    fn copy_uses_selected_full_note_and_is_not_an_editor_shortcut() -> Result<()> {
        let mut app = app()?;
        app.key(key(KeyCode::Down));
        let before = app.db.clone();
        assert_eq!(
            app.key(key(KeyCode::Char('c'))),
            Action::Copy {
                id: 2,
                text: "Türkçe görev".into()
            }
        );
        app.key(key(KeyCode::Tab));
        assert_eq!(
            app.key(key(KeyCode::Char('c'))),
            Action::Copy {
                id: 2,
                text: "Türkçe görev".into()
            }
        );
        app.key(key(KeyCode::Char('a')));
        assert_eq!(app.key(key(KeyCode::Char('c'))), Action::None);
        assert_eq!(app.key(key(KeyCode::Enter)), Action::Add("c".into()));
        for shortcut in ['e', '/'] {
            app.key(key(KeyCode::Char(shortcut)));
            assert_eq!(app.key(key(KeyCode::Char('c'))), Action::None);
            assert_eq!(app.key(key(KeyCode::Char('p'))), Action::None);
            let Mode::Input { editor, .. } = &app.mode else {
                panic!("Expected text input")
            };
            assert!(editor.text.ends_with("cp"));
            app.key(key(KeyCode::Enter));
        }
        assert_eq!(app.db, before);
        let mut empty = App::new(Database::default(), false, None);
        assert_eq!(empty.key(key(KeyCode::Char('c'))), Action::None);
        Ok(())
    }

    #[test]
    fn project_picker_is_unique_session_only_and_preserves_existing_origins() -> Result<()> {
        let mut db = Database::default();
        db.add_for_project("One", Some("Alpha"))?;
        db.add_for_project("Two", Some("Alpha"))?;
        db.add_for_project("Three", Some("Beta"))?;
        let before = db.clone();
        let mut app = App::new(db, true, Some("Current".into()));
        app.key(key(KeyCode::Char('p')));
        let Mode::ProjectPicker { projects, selected } = &app.mode else {
            panic!("Expected picker")
        };
        assert_eq!(projects, &["Alpha", "Beta", "Current"]);
        assert_eq!(*selected, 2);
        app.key(key(KeyCode::Up));
        app.key(key(KeyCode::Enter));
        assert_eq!(app.project.as_deref(), Some("Beta"));
        assert_eq!(app.db, before);
        app.key(key(KeyCode::Char('p')));
        app.key(key(KeyCode::Home));
        app.key(key(KeyCode::Esc));
        assert_eq!(app.project.as_deref(), Some("Beta"));
        app.key(key(KeyCode::Char('p')));
        app.key(key(KeyCode::End));
        app.key(key(KeyCode::Enter));
        app.paste("  New project 🦀  ");
        app.key(key(KeyCode::Enter));
        assert_eq!(app.project.as_deref(), Some("New project 🦀"));
        assert_eq!(app.db, before);
        let reopened = App::new(before, true, Some("Current".into()));
        assert_eq!(reopened.project.as_deref(), Some("Current"));
        Ok(())
    }

    #[test]
    fn empty_project_is_rejected_and_project_letters_are_text() {
        let mut app = App::new(Database::default(), false, None);
        app.key(key(KeyCode::Char('p')));
        app.key(key(KeyCode::Enter));
        app.key(key(KeyCode::Enter));
        assert!(app.error);
        assert!(matches!(
            app.mode,
            Mode::Input {
                kind: InputKind::Project,
                ..
            }
        ));
        assert_eq!(app.project, None);
        assert_eq!(app.key(key(KeyCode::Char('p'))), Action::None);
        app.key(key(KeyCode::Char('c')));
        app.key(key(KeyCode::Enter));
        assert_eq!(app.project.as_deref(), Some("pc"));
    }

    #[test]
    fn repeats_cannot_double_toggle() -> Result<()> {
        let mut app = app()?;
        let mut repeat = key(KeyCode::Char(' '));
        repeat.kind = KeyEventKind::Repeat;
        assert_eq!(app.key(repeat), Action::None);
        Ok(())
    }
}
