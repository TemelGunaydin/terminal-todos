use crate::model::{DATA_VERSION, Database, Task, now};
use anyhow::{Context, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct Store {
    file: PathBuf,
    legacy: Option<PathBuf>,
}

impl Store {
    pub fn new(file: PathBuf, legacy: Option<PathBuf>) -> Self {
        Self { file, legacy }
    }

    pub fn from_environment() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute());
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute());
        let base = match base {
            Some(base) => base,
            None => home
                .as_ref()
                .context("Set HOME or an absolute XDG_DATA_HOME")?
                .join(".local/share"),
        };
        Ok(Self::new(
            base.join("terminal-todos/todos.json"),
            home.map(|p| p.join(".swift_todos.json")),
        ))
    }

    pub fn path(&self) -> &Path {
        &self.file
    }

    pub fn load(&self) -> Result<Database> {
        let _lock = self.lock()?;
        self.load_locked()
    }

    /// Every mutation reads the latest data under one exclusive lock. A TUI
    /// snapshot can therefore never overwrite a newer CLI edit.
    pub fn transact<T>(
        &self,
        change: impl FnOnce(&mut Database) -> Result<T>,
    ) -> Result<(Database, T)> {
        let _lock = self.lock()?;
        let mut db = self.load_locked()?;
        let old_version = db.version;
        let result = change(&mut db)?;
        db.version = DATA_VERSION;
        db.validate()?;
        if old_version == 1 {
            self.backup_v1()?;
        }
        self.save_locked(&db)?;
        Ok((db, result))
    }

    fn lock(&self) -> Result<File> {
        let parent = self.file.parent().context("Data file has no parent")?;
        fs::create_dir_all(parent)
            .with_context(|| format!("Could not create {}", parent.display()))?;
        let lock_path = self.file.with_extension("lock");
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options
            .open(&lock_path)
            .with_context(|| format!("Could not open {}", lock_path.display()))?;
        lock.lock().context("Could not lock task data")?;
        Ok(lock)
    }

    fn load_locked(&self) -> Result<Database> {
        match fs::read(&self.file) {
            Ok(bytes) => {
                let db: Database = serde_json::from_slice(&bytes).with_context(|| {
                    format!(
                        "Invalid task data at {}. The file was not changed.",
                        self.file.display()
                    )
                })?;
                db.validate()?;
                Ok(db)
            }
            Err(error) if error.kind() == ErrorKind::NotFound => self.import_legacy(),
            Err(error) => {
                Err(error).with_context(|| format!("Could not read {}", self.file.display()))
            }
        }
    }

    fn import_legacy(&self) -> Result<Database> {
        let mut db = Database::default();
        let Some(path) = &self.legacy else {
            return Ok(db);
        };
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(db),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Could not read legacy tasks at {}", path.display()));
            }
        };
        let titles: Vec<String> = serde_json::from_slice(&bytes).with_context(|| {
            format!(
                "Invalid legacy task data at {}. No tasks were imported or overwritten.",
                path.display()
            )
        })?;
        let imported_at = now()?;
        for title in titles {
            // Preserve every legacy string exactly, including whitespace.
            let id = db.next_id;
            db.next_id = id
                .checked_add(1)
                .context("Task IDs exhausted during import")?;
            db.tasks.push(Task {
                id,
                title,
                completed: false,
                created_at: imported_at,
                project: None,
            });
        }
        db.validate()?;
        self.save_locked(&db)?;
        // The Swift file remains byte-for-byte intact for rollback.
        Ok(db)
    }

    fn backup_v1(&self) -> Result<()> {
        let backup = self.file.with_extension("v1.backup.json");
        let bytes = fs::read(&self.file).context("Could not read version 1 data for backup")?;
        match fs::symlink_metadata(&backup) {
            Ok(metadata) => {
                if !metadata.is_file() || metadata.is_symlink() || fs::read(&backup)? != bytes {
                    anyhow::bail!(
                        "Existing migration backup {} differs from current data. Keep it and move it aside before retrying. The task file was not changed.",
                        backup.display()
                    );
                }
                return Ok(());
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("Could not inspect migration backup"),
        }
        let parent = self.file.parent().context("Data file has no parent")?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        temp.persist_noclobber(&backup).with_context(|| {
            format!("Could not preserve version 1 data at {}", backup.display())
        })?;
        Ok(())
    }

    fn save_locked(&self, db: &Database) -> Result<()> {
        let parent = self.file.parent().context("Data file has no parent")?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)
            .context("Could not create a temporary task file")?;
        serde_json::to_writer_pretty(&mut temp, db)?;
        writeln!(temp)?;
        temp.as_file()
            .sync_all()
            .context("Could not flush task data")?;
        temp.persist(&self.file)
            .with_context(|| format!("Could not save {}", self.file.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_preserves_original_and_runs_only_once() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let legacy = dir.path().join(".swift_todos.json");
        let original = "[\"  Kitap oku  \",\"Türkçe 🦀\",\"\"]";
        fs::write(&legacy, original)?;
        let store = Store::new(dir.path().join("new/todos.json"), Some(legacy.clone()));
        let db = store.load()?;
        assert_eq!(
            db.tasks
                .iter()
                .map(|task| task.title.as_str())
                .collect::<Vec<_>>(),
            ["  Kitap oku  ", "Türkçe 🦀", ""]
        );
        assert_eq!(fs::read_to_string(&legacy)?, original);
        store.transact(|db| db.delete(1))?;
        fs::write(&legacy, "[\"Changed by Swift\"]")?;
        let db = store.load()?;
        assert_eq!(db.tasks.len(), 2);
        assert_eq!(db.tasks[0].id, 2);
        assert_eq!(db.next_id, 4);
        Ok(())
    }

    #[test]
    fn invalid_legacy_is_never_overwritten_or_imported() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let legacy = dir.path().join("old.json");
        fs::write(&legacy, "broken json")?;
        let store = Store::new(dir.path().join("todos.json"), Some(legacy.clone()));
        assert!(store.transact(|db| db.add("New task")).is_err());
        assert_eq!(fs::read_to_string(&legacy)?, "broken json");
        assert!(!store.path().exists());
        Ok(())
    }

    #[test]
    fn invalid_current_data_and_failed_changes_are_preserved() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let store = Store::new(dir.path().join("todos.json"), None);
        fs::write(store.path(), "broken json")?;
        assert!(store.transact(|db| db.add("Task")).is_err());
        assert_eq!(fs::read_to_string(store.path())?, "broken json");
        fs::remove_file(store.path())?;
        store.transact(|db| db.add("Keep me"))?;
        let before = fs::read(store.path())?;
        assert!(store.transact(|db| db.delete(99)).is_err());
        assert_eq!(fs::read(store.path())?, before);
        Ok(())
    }

    #[test]
    fn parallel_updates_and_stale_snapshots_do_not_lose_tasks() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let store = Store::new(dir.path().join("todos.json"), None);
        let stale = store.load()?;
        let workers: Vec<_> = (0..12)
            .map(|i| {
                let store = store.clone();
                std::thread::spawn(move || store.transact(|db| db.add(&format!("Task {i}"))))
            })
            .collect();
        for worker in workers {
            worker.join().unwrap()?;
        }
        assert!(stale.tasks.is_empty());
        store.transact(|db| db.set_completed(1, true))?;
        let db = store.load()?;
        assert_eq!(db.tasks.len(), 12);
        assert_eq!(db.next_id, 13);
        assert!(db.tasks.iter().find(|task| task.id == 1).unwrap().completed);
        Ok(())
    }

    #[test]
    fn reading_v1_is_nondestructive_and_first_write_keeps_an_exact_backup() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("todos.json");
        let original = "{\"version\":1,\"next_id\":2,\"tasks\":[{\"id\":1,\"title\":\"Old task\",\"completed\":true,\"created_at\":123}]}\n";
        fs::write(&file, original)?;
        let store = Store::new(file.clone(), None);
        assert_eq!(store.load()?.tasks[0].project, None);
        assert_eq!(fs::read_to_string(&file)?, original);
        let backup = dir.path().join("todos.v1.backup.json");
        assert!(!backup.exists());
        assert!(store.transact(|db| db.add("  ")).is_err());
        assert!(!backup.exists());
        store.transact(|db| db.add_for_project("New task", Some("Bookfun")))?;
        assert_eq!(fs::read_to_string(&backup)?, original);
        let db = store.load()?;
        assert_eq!(db.version, DATA_VERSION);
        assert_eq!(db.tasks[0].id, 1);
        assert_eq!(db.tasks[0].created_at, 123);
        assert!(db.tasks[0].completed);
        assert_eq!(db.tasks[0].project, None);
        assert_eq!(db.tasks[1].project.as_deref(), Some("Bookfun"));
        store.transact(|db| db.update(1, "Edited"))?;
        assert_eq!(fs::read_to_string(&backup)?, original);
        Ok(())
    }

    #[test]
    fn a_conflicting_backup_never_overwrites_either_file() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("todos.json");
        let original = "{\"version\":1,\"next_id\":1,\"tasks\":[]}";
        fs::write(&file, original)?;
        let backup = dir.path().join("todos.v1.backup.json");
        fs::write(&backup, "earlier snapshot")?;
        let store = Store::new(file.clone(), None);
        assert!(store.transact(|db| db.add("New task")).is_err());
        assert_eq!(fs::read_to_string(&file)?, original);
        assert_eq!(fs::read_to_string(&backup)?, "earlier snapshot");
        fs::write(&backup, original)?;
        store.transact(|db| db.add("Retry"))?;
        assert_eq!(store.load()?.version, DATA_VERSION);
        assert_eq!(fs::read_to_string(&backup)?, original);
        Ok(())
    }

    #[test]
    fn custom_data_filenames_have_independent_backups() -> Result<()> {
        let dir = tempfile::tempdir()?;
        for filename in ["first.json", "second.json", "todos.v1.backup.json"] {
            let file = dir.path().join(filename);
            let original = "{\"version\":1,\"next_id\":1,\"tasks\":[]}";
            fs::write(&file, original)?;
            let store = Store::new(file.clone(), None);
            store.transact(|db| db.add_for_project("New task", Some("Bookfun")))?;
            assert_eq!(
                fs::read_to_string(file.with_extension("v1.backup.json"))?,
                original
            );
            assert_eq!(store.load()?.tasks[0].project.as_deref(), Some("Bookfun"));
        }
        Ok(())
    }

    #[test]
    fn failed_atomic_replace_keeps_destination_intact() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("todos.json");
        fs::create_dir(&file)?;
        fs::write(file.join("keep"), "intact")?;
        let store = Store::new(file.clone(), None);
        assert!(store.save_locked(&Database::default()).is_err());
        assert_eq!(fs::read_to_string(file.join("keep"))?, "intact");
        Ok(())
    }
}
