use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};

pub const DATA_VERSION: u32 = 2;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub completed: bool,
    pub created_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Database {
    pub version: u32,
    pub next_id: u64,
    pub tasks: Vec<Task>,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            version: DATA_VERSION,
            next_id: 1,
            tasks: vec![],
        }
    }
}

impl Database {
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.version, 1 | DATA_VERSION) {
            bail!(
                "Unsupported data version {}. The file was not changed.",
                self.version
            );
        }
        let mut ids = BTreeSet::new();
        for task in &self.tasks {
            if task.id == 0 || task.id >= self.next_id || !ids.insert(task.id) {
                bail!("Invalid or duplicate task ID. The file was not changed.");
            }
            if let Some(project) = &task.project {
                if self.version == 1 {
                    bail!(
                        "Project metadata requires data version {DATA_VERSION}. The file was not changed."
                    );
                }
                valid_project(project)?;
            }
        }
        if self.next_id == 0 {
            bail!("Invalid next task ID. The file was not changed.");
        }
        Ok(())
    }

    pub fn add(&mut self, title: &str) -> Result<Task> {
        self.add_for_project(title, None)
    }

    pub fn add_for_project(&mut self, title: &str, project: Option<&str>) -> Result<Task> {
        let title = valid_title(title)?;
        let project = project.map(valid_project).transpose()?;
        let next_id = self.next_id.checked_add(1).context("Task IDs exhausted")?;
        let task = Task {
            id: self.next_id,
            title,
            completed: false,
            created_at: now()?,
            project,
        };
        self.tasks.push(task.clone());
        self.next_id = next_id;
        self.version = DATA_VERSION;
        Ok(task)
    }

    pub fn update(&mut self, id: u64, title: &str) -> Result<Task> {
        let title = valid_title(title)?;
        let task = self.task_mut(id)?;
        task.title = title;
        Ok(task.clone())
    }

    pub fn set_completed(&mut self, id: u64, completed: bool) -> Result<Task> {
        let task = self.task_mut(id)?;
        task.completed = completed;
        Ok(task.clone())
    }

    pub fn toggle(&mut self, id: u64) -> Result<Task> {
        let task = self.task_mut(id)?;
        task.completed = !task.completed;
        Ok(task.clone())
    }

    pub fn delete(&mut self, id: u64) -> Result<Task> {
        let index = self
            .tasks
            .iter()
            .position(|task| task.id == id)
            .with_context(|| {
                format!("Task #{id} was not found. Use `todo list --all` to see IDs.")
            })?;
        Ok(self.tasks.remove(index))
    }

    fn task_mut(&mut self, id: u64) -> Result<&mut Task> {
        self.tasks
            .iter_mut()
            .find(|task| task.id == id)
            .with_context(|| format!("Task #{id} was not found. Use `todo list --all` to see IDs."))
    }
}

pub fn now() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System clock is before 1970")?
        .as_secs())
}

fn valid_title(title: &str) -> Result<String> {
    let title = title.trim();
    if title.is_empty() {
        bail!("Task text cannot be empty.");
    }
    if title.chars().any(char::is_control) {
        bail!("Task text cannot contain control characters or line breaks.");
    }
    Ok(title.to_owned())
}

fn valid_project(project: &str) -> Result<String> {
    if project.trim().is_empty() || project.chars().any(char::is_control) {
        bail!(
            "Project name cannot be empty or contain control characters. The file was not changed."
        );
    }
    Ok(project.to_owned())
}

/// Untrusted task text must never emit terminal escape sequences.
pub fn safe(text: &str) -> String {
    text.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
}

pub fn age(created_at: u64) -> String {
    let elapsed = now().unwrap_or(created_at).saturating_sub(created_at);
    match elapsed {
        0..60 => "just now".into(),
        60..3600 => format!("{}m ago", elapsed / 60),
        3600..86400 => format!("{}h ago", elapsed / 3600),
        _ => format!("{}d ago", elapsed / 86400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletion_never_reuses_or_shifts_ids() -> Result<()> {
        let mut db = Database::default();
        db.add("First")?;
        db.add("Second")?;
        db.delete(1)?;
        assert_eq!(db.add("Third")?.id, 3);
        assert_eq!(db.tasks[0].id, 2);
        db.validate()
    }

    #[test]
    fn validates_titles_without_mutating_on_error() -> Result<()> {
        let mut db = Database::default();
        let task = db.add("  Türkçe görev 🦀  ")?;
        assert_eq!(task.title, "Türkçe görev 🦀");
        assert!(db.add("   ").is_err());
        assert!(db.update(1, "bad\u{1b}[2J").is_err());
        assert_eq!(db.tasks, [task]);
        assert!(db.toggle(99).is_err());
        Ok(())
    }

    #[test]
    fn corrupted_and_future_schemas_are_rejected() -> Result<()> {
        let mut db = Database::default();
        db.add("First")?;
        db.tasks.push(db.tasks[0].clone());
        assert!(db.validate().is_err());
        db.tasks.pop();
        db.version = DATA_VERSION + 1;
        assert!(db.validate().is_err());
        Ok(())
    }

    #[test]
    fn old_tasks_are_unassigned_and_project_origin_survives_mutations() -> Result<()> {
        let mut db: Database = serde_json::from_str(
            r#"{"version":1,"next_id":2,"tasks":[{"id":1,"title":"Old task","completed":false,"created_at":123}]}"#,
        )?;
        db.validate()?;
        assert_eq!(db.tasks[0].project, None);
        let new = db.add_for_project("New task", Some("Bookfun"))?;
        assert_eq!(db.version, DATA_VERSION);
        assert_eq!(new.project.as_deref(), Some("Bookfun"));
        db.update(2, "Edited elsewhere")?;
        db.toggle(2)?;
        assert_eq!(db.tasks[1].project, new.project);
        assert_eq!(db.tasks[0].created_at, 123);
        db.validate()
    }

    #[test]
    fn invalid_project_never_creates_a_task() -> Result<()> {
        let mut db = Database::default();
        for project in ["", "  ", "bad\u{1b}[2J", "bad\nname"] {
            assert!(db.add_for_project("Task", Some(project)).is_err());
            assert!(db.tasks.is_empty());
            assert_eq!(db.next_id, 1);
        }
        db.add_for_project("Task", Some("Good"))?;
        db.tasks[0].project = Some("\u{7}".into());
        assert!(db.validate().is_err());
        Ok(())
    }

    #[test]
    fn untrusted_text_cannot_control_the_terminal() {
        assert_eq!(safe("a\n\u{1b}[2J\t\u{7}b"), "a  [2J  b");
    }
}
