use std::{
    fs,
    process::{Command, Output},
};
use tempfile::TempDir;

struct Sandbox {
    home: TempDir,
}
impl Sandbox {
    fn new() -> Self {
        Self {
            home: tempfile::tempdir().unwrap(),
        }
    }
    fn run(&self, args: &[&str]) -> Output {
        self.run_from(&std::env::current_dir().unwrap(), args)
    }
    fn run_from(&self, cwd: &std::path::Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_todo"))
            .current_dir(cwd)
            .args(args)
            .env("HOME", self.home.path())
            .env("XDG_DATA_HOME", self.home.path().join("data"))
            .env("TERM", "xterm-256color")
            .env_remove("NO_COLOR")
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
    fn file(&self) -> std::path::PathBuf {
        self.home.path().join("data/terminal-todos/todos.json")
    }
}

#[test]
fn task_lifecycle_uses_stable_ids_and_aliases() {
    let sandbox = Sandbox::new();
    assert!(
        sandbox
            .ok(&["add", "Read", "a", "book"])
            .contains("Added #1: Read a book")
    );
    sandbox.ok(&["add", "Second task"]);
    sandbox.ok(&["done", "1"]);
    let open = sandbox.ok(&["list"]);
    assert!(!open.contains("#1"));
    assert!(open.contains("[ ] #2"));
    assert!(sandbox.ok(&["list", "--done"]).contains("[x] #1"));
    sandbox.ok(&["undo", "1"]);
    sandbox.ok(&["edit", "1", "Türkçe görev 🦀"]);
    sandbox.ok(&["del", "1"]);
    assert!(sandbox.ok(&["add", "Third task"]).contains("Added #3"));
    let all = sandbox.ok(&["list", "--all"]);
    assert!(all.contains("#2") && all.contains("#3"));
    assert!(
        !all.contains("\x1b"),
        "Piped output should not contain ANSI colors"
    );
}

#[test]
fn invalid_commands_and_text_do_not_change_data() {
    let sandbox = Sandbox::new();
    sandbox.ok(&["add", "Keep me"]);
    let before = fs::read(sandbox.file()).unwrap();
    for args in [
        vec!["add", "   "],
        vec!["update", "1", ""],
        vec!["delete", "0"],
        vec!["done", "99"],
        vec!["delete", "bad"],
        vec!["udpate", "1", "Typo"],
        vec!["list", "--all", "--done"],
        vec!["add", "bad\u{1b}[2J"],
    ] {
        assert!(!sandbox.run(&args).status.success(), "{args:?} should fail");
        assert_eq!(
            fs::read(sandbox.file()).unwrap(),
            before,
            "{args:?} changed data"
        );
    }
}

#[test]
fn imports_swift_tasks_once_and_keeps_rollback_file() {
    let sandbox = Sandbox::new();
    let legacy = sandbox.home.path().join(".swift_todos.json");
    let original = "[\"Book\",\"Türkçe 🦀\"]\n";
    fs::write(&legacy, original).unwrap();
    let list = sandbox.ok(&["list", "--all"]);
    assert!(list.contains("#1 Book") && list.contains("#2 Türkçe 🦀"));
    sandbox.ok(&["done", "1"]);
    sandbox.ok(&["add", "New task"]);
    assert_eq!(fs::read_to_string(legacy).unwrap(), original);
    let list = sandbox.ok(&["list", "--all"]);
    assert_eq!(list.matches("Book").count(), 1);
    assert!(list.contains("[x] #1") && list.contains("#3 New task"));
}

#[test]
fn malformed_legacy_and_current_files_are_never_overwritten() {
    let sandbox = Sandbox::new();
    let legacy = sandbox.home.path().join(".swift_todos.json");
    fs::write(&legacy, "broken legacy").unwrap();
    assert!(!sandbox.run(&["add", "Task"]).status.success());
    assert!(!sandbox.file().exists());
    assert_eq!(fs::read_to_string(&legacy).unwrap(), "broken legacy");
    fs::remove_file(legacy).unwrap();
    sandbox.ok(&["add", "Keep me"]);
    fs::write(sandbox.file(), "broken current").unwrap();
    assert!(!sandbox.run(&["add", "Task"]).status.success());
    assert_eq!(
        fs::read_to_string(sandbox.file()).unwrap(),
        "broken current"
    );
}

#[test]
fn dashboard_rejects_pipes_before_creating_task_data() {
    let sandbox = Sandbox::new();
    let output = sandbox.run(&[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("interactive terminal"));
    assert!(!sandbox.file().exists());
    assert!(sandbox.ok(&["--help"]).contains("dashboard"));
}

#[test]
fn explicit_color_is_supported_without_changing_plain_default() {
    let sandbox = Sandbox::new();
    sandbox.ok(&["add", "Task"]);
    assert!(sandbox.ok(&["--color", "always", "list"]).contains("\x1b["));
    assert!(!sandbox.ok(&["list", "--color", "never"]).contains("\x1b["));
}

#[test]
fn project_origin_is_saved_once_and_shown_in_plain_and_colored_output() {
    let sandbox = Sandbox::new();
    let alpha = sandbox.home.path().join("Alpha project 🦀");
    let beta = sandbox.home.path().join("Beta");
    fs::create_dir(&alpha).unwrap();
    fs::create_dir(&beta).unwrap();
    assert!(
        sandbox
            .run_from(&alpha, &["add", "Alpha task"])
            .status
            .success()
    );
    assert!(
        sandbox
            .run_from(&beta, &["add", "Beta task"])
            .status
            .success()
    );
    assert!(
        sandbox
            .run_from(&beta, &["update", "1", "Edited from Beta"])
            .status
            .success()
    );
    assert!(sandbox.run_from(&beta, &["done", "1"]).status.success());
    assert!(sandbox.run_from(&alpha, &["undo", "1"]).status.success());
    let data: serde_json::Value =
        serde_json::from_slice(&fs::read(sandbox.file()).unwrap()).unwrap();
    assert_eq!(data["version"], 2);
    assert_eq!(data["tasks"][0]["project"], "Alpha project 🦀");
    assert_eq!(data["tasks"][1]["project"], "Beta");
    let plain = sandbox.ok(&["list", "--all"]);
    assert!(plain.contains("[Alpha project 🦀]") && plain.contains("[Beta]"));
    assert!(!plain.contains('\u{1b}'));
    let (r, g, b) = terminal_todos::project::rgb("Alpha project 🦀");
    let colored = sandbox.ok(&["--color", "always", "list"]);
    assert!(colored.contains(&format!("\u{1b}[1;38;2;{r};{g};{b}m[Alpha project 🦀]")));
}

#[test]
fn nested_repository_add_uses_the_root_name() {
    let sandbox = Sandbox::new();
    let root = sandbox.home.path().join("SampleRepo");
    fs::create_dir_all(root.join("src/nested")).unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        sandbox
            .run_from(&root.join("src/nested"), &["add", "Nested note"])
            .status
            .success()
    );
    let data: serde_json::Value =
        serde_json::from_slice(&fs::read(sandbox.file()).unwrap()).unwrap();
    assert_eq!(data["tasks"][0]["project"], "SampleRepo");
    assert!(sandbox.ok(&["list"]).contains("[SampleRepo]"));
}

#[test]
fn existing_v1_tasks_are_not_backfilled_and_migration_is_backed_up() {
    let sandbox = Sandbox::new();
    fs::create_dir_all(sandbox.file().parent().unwrap()).unwrap();
    let original = "{\"version\":1,\"next_id\":2,\"tasks\":[{\"id\":1,\"title\":\"Existing note\",\"completed\":true,\"created_at\":123}]}\n";
    fs::write(sandbox.file(), original).unwrap();
    assert!(sandbox.ok(&["list", "--all"]).contains("Existing note"));
    assert_eq!(fs::read_to_string(sandbox.file()).unwrap(), original);
    let project = sandbox.home.path().join("NewProject");
    fs::create_dir(&project).unwrap();
    assert!(
        sandbox
            .run_from(&project, &["add", "New note"])
            .status
            .success()
    );
    let data: serde_json::Value =
        serde_json::from_slice(&fs::read(sandbox.file()).unwrap()).unwrap();
    assert_eq!(data["tasks"][0]["project"], serde_json::Value::Null);
    assert_eq!(data["tasks"][0]["created_at"], 123);
    assert_eq!(data["tasks"][0]["completed"], true);
    assert_eq!(data["tasks"][1]["project"], "NewProject");
    assert_eq!(
        fs::read_to_string(sandbox.file().with_file_name("todos.v1.backup.json")).unwrap(),
        original
    );
}

#[test]
fn concurrent_processes_do_not_lose_or_duplicate_tasks() {
    let sandbox = Sandbox::new();
    let processes: Vec<_> = (0..8)
        .map(|i| {
            Command::new(env!("CARGO_BIN_EXE_todo"))
                .args(["add", &format!("Task {i}")])
                .env("HOME", sandbox.home.path())
                .env("XDG_DATA_HOME", sandbox.home.path().join("data"))
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for process in processes {
        assert!(process.wait_with_output().unwrap().status.success());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(sandbox.file()).unwrap()).unwrap();
    assert_eq!(value["tasks"].as_array().unwrap().len(), 8);
    assert_eq!(value["next_id"], 9);
}
