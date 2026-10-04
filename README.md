<p align="center">
  <img src="assets/terminal-todos-icon.png" width="144" alt="Terminal Todos — violet terminal prompt and checklist icon">
</p>

<h1 align="center">Terminal Todos</h1>

<p align="center">Your tasks. Nothing left behind.</p>

A keyboard-first **Rust + Ratatui** todo dashboard with its own **midnight-indigo and violet** identity. Lavender accents highlight selection and completed tasks, peach marks open tasks, and rose signals errors. Open it with `todo`, or use quick CLI commands without entering the dashboard.

Local files only. No accounts, server, or network access at runtime.

## Dashboard

- Summary cards for open, completed, and total tasks.
- A scrollable task list and a details panel with a clearly highlighted selection.
- Side-by-side panels at 100+ columns; stacked panels in smaller terminals.
- Add/edit dialogs, live title search, open/completed/all filters, and deletion confirmation.
- Unicode-aware text editing and bracketed paste, including Turkish text and emoji.
- Automatic saves and refresh of changes made by another Rust `todo` process.
- Terminal screen, cursor, paste mode, and input settings restored on normal exit, Ctrl+C, SIGINT, or SIGTERM.

The dashboard requires interactive stdin/stdout and at least **60 columns × 20 rows**. **100+ columns × 28+ rows** is more comfortable. A smaller window shows a resize hint; `q` and Ctrl+C still work.

| Key | Action |
| --- | --- |
| `↑` / `↓`, `k` / `j` | Select a task; scroll when Details is focused |
| `Page Up` / `Page Down` | Move a page |
| `Home` / `End` | Jump to the first/last task or top/bottom of details |
| `a` | Add a task |
| `e` | Edit the selected task |
| `Space` | Complete or reopen the selected task |
| `d` | Ask to delete the selected task |
| `1` / `2` / `3` | Show open / completed / all tasks |
| `/` | Search titles; matches are shown as you type |
| `Esc` | Cancel a dialog; clear an applied search |
| `Tab` / `Shift+Tab` | Switch between Tasks and Details |
| `r` | Refresh saved tasks immediately |
| `q` | Quit, outside a dialog |
| `Ctrl+C` | Exit from any screen |

In add/edit/search dialogs, use `Enter` to save/apply, `Esc` to cancel, arrows/Home/End to move the cursor, and `Ctrl+U` to clear text before the cursor. In the delete dialog, `Enter`/`y` confirms and `Esc`/`n` cancels. CLI deletion is immediate.

## Run and install

Requires **Rust 1.89+** and Cargo. Developed and tested on macOS; Linux is also a target. Windows support has not been verified.

Run from this checkout:

```bash
cargo run --locked
cargo run --locked -- add "Read a book"
cargo run --locked -- list --all
```

Install from source without `sudo`:

```bash
./scripts/install.sh
```

This installs `todo` at `~/.local/bin/todo`. A custom prefix is supported:

```bash
PREFIX="$HOME/tools" ./scripts/install.sh
```

If needed, put the install directory **before** other directories in your shell's PATH:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

### Homebrew

Install from the tap:

```bash
brew tap TemelGunaydin/tap
brew install todo
```

Upgrade the previous Swift release to Rust **1.2.0**:

```bash
brew update
brew upgrade temelgunaydin/tap/todo
```

The first data access imports Swift tasks without changing the original file. The formula is in [`homebrew/Formula/todo.rb`](homebrew/Formula/todo.rb); release packaging and verification are documented in [`homebrew/README.md`](homebrew/README.md).

The source installer and Homebrew install into different directories. Check `command -v todo` to confirm which executable your shell is using.

## CLI

```bash
todo                              # Interactive dashboard
todo tui                          # Explicit dashboard command
todo add "Read a book"
todo add Write integration tests  # Multi-word text also works without quotes
todo list                         # Open tasks
todo list --done                   # Completed tasks
todo list --all                    # All tasks
todo update 1 "Read two chapters"  # `edit` is an alias
todo done 1
todo undo 1
todo delete 1                      # `del` is an alias
todo --help
```

**Numbers are stable task IDs, not row positions.** Deleting #1 does not rename #2; IDs are not reused. The dashboard and CLI display the same IDs.

Unlike the old Swift shorthand, bare text is not treated as an add command: use `todo add "Your task"`. Unknown commands fail instead of silently creating tasks.

Plain CLI output is pipe-friendly and automatically omits colors:

```bash
todo list --all > tasks.txt
todo --color never                # Monochrome dashboard
NO_COLOR=1 todo                   # Disable automatic colors
todo --color always list          # Explicitly force CLI colors
```

A dashboard still uses terminal-control sequences in monochrome mode. `TERM=dumb` and redirected stdin/stdout are rejected for the dashboard; use `todo list` instead.

## Data and Swift migration

Rust stores tasks in:

```text
$XDG_DATA_HOME/terminal-todos/todos.json
```

When `XDG_DATA_HOME` is unset or not absolute, it uses:

```text
~/.local/share/terminal-todos/todos.json
```

On the first data access, if the Rust data file does not exist, the application imports the old **`~/.swift_todos.json`** string array:

- Every string and its original ordering are preserved.
- IDs start at 1, and imported tasks are open.
- Creation times reflect the import time; Swift did not record creation dates.
- **The original Swift file is never changed or removed.**
- Once a Rust file exists, imports do not repeat, even if the old Swift file changes.
- Invalid old/new JSON, unsupported schemas, and storage failures are reported instead of overwritten or silently treated as empty data.

Writes use an exclusive file lock, a temporary file in the same directory, and atomic replacement. Each edit reads current data while holding the lock, so a stale dashboard cannot overwrite unrelated CLI changes. If two processes edit the same task, the last successful edit wins.

The Swift and Rust versions are **not live-synchronized**. After migration, use Rust consistently. For rollback, the original source is preserved in [`legacy/swift`](legacy/swift), and the original JSON remains usable by Swift. Changes made in Rust after import will not appear in the old Swift snapshot; back up the Rust file before rolling back.

## Development and verification

```bash
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

The Rust tests cover task IDs, validation, migration/rollback preservation, malformed files, atomic-write failures, concurrent writes, CLI commands/aliases, Unicode editing, keyboard state, and bounded layouts.

Real-terminal acceptance tests use temporary HOME/data directories and do not touch your tasks:

```bash
cargo build --locked
uv run --no-project --with pyte scripts/test_tui.py target/debug/todo
```

They exercise add/edit/complete/reopen/delete, confirmation/cancellation, live search, Unicode paste, concurrent CLI changes, paging, resizing, monochrome mode, and terminal restoration. Optional screen dumps are written to `/tmp/terminal-todos-wide.txt` and `/tmp/terminal-todos-narrow.txt`.

### Exit codes

- `0`: completed successfully, or quit the dashboard with `q`.
- `1`: task, storage, or terminal error.
- `2`: invalid CLI syntax.
- `130`: Ctrl+C/SIGINT.
- `143`: SIGTERM.

Project grouping, priorities, deadlines, and cloud sync are outside this first Rust release.
