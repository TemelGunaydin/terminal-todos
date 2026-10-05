<p align="center">
  <img src="assets/terminal-todos-icon.png" width="144" alt="Terminal Todos">
</p>

<h1 align="center">Terminal Todos</h1>

<p align="center">A simple, keyboard-first todo app for your terminal. Built with Rust.</p>

![Terminal Todos dashboard with sample tasks](assets/dashboard.png)

## Use inside Pi

Open the dashboard without leaving [Pi](https://pi.dev/). Supports macOS 11+ and Linux on **arm64 / x64**; no Rust or Homebrew installation needed.

### 1. Install the Pi package

Run this in your **terminal**, not as a Pi slash command:

```bash
pi install git:github.com/TemelGunaydin/terminal-todos@v1.3.0
```

This installs the extension for your user. Add `--local` to install it only for the current project; project extensions require Pi's project-trust approval.

### 2. Reload and open

If Pi is already running, enter these commands **inside Pi**, one at a time:

```text
/reload
/todo
```

Otherwise, start `pi` and run `/todo`. You can also press **Ctrl+Alt+T**.

### 3. Approve the first download

If the managed app is missing, Pi asks **“Install Terminal Todos?”**. Approve to download the binary for your OS/CPU from the matching GitHub release. Its SHA-256 and version are checked before installation completes.

- Nothing is downloaded just by starting Pi.
- **Esc / Ctrl+C** cancels the download.
- Subsequent launches use the verified local copy without downloading again.
- Pi uses its own executable; it does **not** install or upgrade your Homebrew `todo`, or change your PATH.

### Pi shortcuts

| Command / key | Action |
| --- | --- |
| `/todo` | Open the dashboard |
| `Ctrl+Alt+T` | Open it without typing a command |
| `q` / `Ctrl+C` in the dashboard | Close Todo and return to Pi |
| `/reload` in Pi | Reload installed extensions |

Your unsent Pi editor text is preserved. Open Todo while Pi is **idle**; print, JSON and RPC modes cannot launch the interactive dashboard.

## Use as a standalone app

### Homebrew

```bash
brew tap TemelGunaydin/tap
brew install todo
```

Already installed? Run `brew update` followed by `brew upgrade todo` to get the version available in the tap.

Open the dashboard with either command:

```bash
todo
todo tui
```

The standalone Rust app and Pi dashboard share tasks when run with the same HOME / XDG_DATA_HOME.

<details>
<summary>Build from source (Rust 1.89+ and Cargo required)</summary>

```bash
git clone https://github.com/TemelGunaydin/terminal-todos.git
cd terminal-todos
./scripts/install.sh
export PATH="$HOME/.local/bin:$PATH"
todo
```

The installer uses `~/.local/bin` by default. Check `command -v todo` if another installation takes priority.

</details>

## Dashboard keys

Changes save automatically. Use a terminal of at least **60 columns × 20 rows**.

| Key | Action |
| --- | --- |
| `↑` / `↓` or `k` / `j` | Move through tasks or scroll the focused details panel |
| `PageUp` / `PageDown` | Move by a page |
| `Home` / `End` | Jump to the start / end of the focused panel |
| `Tab` / `Shift+Tab` | Switch between tasks and details |
| `a` | Add a task |
| `e` | Edit the selected task |
| `Space` | Complete or reopen the selected task |
| `d` | Delete the selected task, with confirmation |
| `/` | Search task titles as you type |
| `1` / `2` / `3` | Show open / completed / all tasks |
| `r` | Refresh task data |
| `Enter` | Save text, apply a search or confirm deletion |
| `Esc` | Cancel a dialog/search edit, or clear an applied search |
| `y` / `n` in the delete dialog | Confirm / cancel deletion |
| `q` / `Ctrl+C` | Quit; return to Pi when launched from Pi |

While adding/editing/searching, letters are text, not dashboard shortcuts. Unicode and paste are supported. Use `←` / `→`, `Home` / `End`, `Backspace` / `Delete` to edit; **Ctrl+A / Ctrl+E** move to the start / end and **Ctrl+U** clears text before the cursor.

## CLI commands

With the standalone app installed, use these in your terminal:

```bash
todo add "Read a book"
todo list                    # Open tasks
todo list --all              # Every task
todo list --done             # Completed tasks
todo done 1                  # Complete task #1
todo undo 1                  # Reopen task #1
todo update 1 "Read two chapters"
todo delete 1                # Permanently delete task #1
todo --help
```

Task numbers are **stable IDs**, not positions in the current list. `edit` is an alias for `update`; `del` is an alias for `delete`. CLI deletion is immediate; only the dashboard asks for confirmation.

Colors are automatic. Use `todo --color never`, `todo --color always list`, or set `NO_COLOR` to disable automatic colors.

## Data and privacy

Your tasks stay on your device. The Pi launcher does not send task contents to the model; its first installation downloads the executable, not your tasks.

- Default task file: `~/.local/share/terminal-todos/todos.json`.
- With an **absolute** `XDG_DATA_HOME`: `$XDG_DATA_HOME/terminal-todos/todos.json`.
- Existing `~/.swift_todos.json` tasks are imported once when Rust data does not exist. The original Swift file is left unchanged; the old Swift app is **not** live-synchronized with Rust.
- Pi's executable cache is separate: `~/.pi/agent/tools/terminal-todos/<version>/<target>/`. A custom `PI_CODING_AGENT_DIR` moves that cache, not the task file.

## Manage the Pi package

Check installed packages with `pi list`. The command above pins **v1.3.0**: `pi update --extensions` will not move a pinned tag to a newer release. To upgrade later, run `pi install` with the new published tag, then `/reload` in Pi.

To remove the extension:

```bash
pi remove git:github.com/TemelGunaydin/terminal-todos@v1.3.0
```

Then `/reload` or restart Pi. Add `--local` if you installed it for the project. Removing the extension does not delete your tasks.

## Troubleshooting

- **`/todo` is not recognized:** check `pi list`, then `/reload` or restart Pi. For a project-local install, approve project trust.
- **Shortcut does nothing:** use `/todo`; your terminal or another binding may intercept Ctrl+Alt+T.
- **Pi is busy:** wait until streaming and queued work finish, then try again.
- **Dashboard requires an interactive terminal:** use a real terminal, or `todo list` for piped output. Resize if the dashboard asks for more space.
- **Download fails:** check your connection and retry `/todo`. If a release asset is not yet available, wait for its publication; there is no automatic Rust/Cargo fallback.
- **Cached install fails validation:** remove only the executable-cache directory named in the error and retry `/todo`. Do not remove the task-data directory.

[Release binaries](https://github.com/TemelGunaydin/terminal-todos/releases/tag/v1.3.0) · [Development and release notes](docs/pi.md)
