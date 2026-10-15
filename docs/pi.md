# Pi integration — development and release

The package exposes only `extensions/todo.js`. It registers `/todo` and
`Ctrl+Alt+T`; startup never downloads or runs the app. Pi keeps ownership
of the terminal for confirmation/download, then suspends its TUI while
the native dashboard runs. Print/RPC/JSON modes and busy agents cannot launch it.

## Current Pi release: v1.5.1

v1.5.1 moves Pi's executable cache out of the deprecated `tools/` directory and
into `cache/terminal-todos/`, without changing task data or dashboard behavior.
Use `git:github.com/TemelGunaydin/terminal-todos@v1.5.1` for the Pi package.
The matching native binaries/checksums are built for all four targets by the tag
workflow. Existing installations should move `tools/terminal-todos/` into `cache/`
once, only when the destination does not already exist; never move task data.

Homebrew remains at v1.5.0, which has its own source archive/formula. Its release
tag points to `33feaaf`; later publishing documentation does not change it.
Older tags, assets and checksums remain immutable. Homebrew and Pi upgrades are
separate, explicitly approved operations.

- `/todo` from a busy command context offers an approved `waitForIdle()`; it never
  calls abort or clears queued messages. Shortcuts without that API refuse with
  actionable guidance. A queue before opening, or work still pending after waiting
  or installation, blocks terminal handoff. Concurrent invocations remain guarded.
- Native `c` sends the selected note's complete UTF-8 title to OSC52 via Crossterm.
  No external clipboard command, model call, or storage mutation is involved.
  Terminal acceptance cannot be acknowledged, so the UI says “Copy sent”.
- Native `p` lists saved project names plus detected/current names, with a new-name
  input validated by the existing model rule. Selection is session-local and only
  affects additions; existing origins and the schema remain unchanged.

## Installation contract

- The extension and app share the version in `package.json` and `Cargo.toml`.
- Only approved first use fetches the fixed GitHub release's native executable
  and its `.sha256`. The checksum must name the exact platform/version asset.
- Downloads are bounded, cancellable and verified before execution. A version
  probe uses a temporary HOME/data directory; incompatible binaries are rejected.
- The complete binary/receipt directory is committed by rename. Later launches
  verify its SHA-256 and executable permission, without network access.
- Cache: `<PI_CODING_AGENT_DIR or ~/.pi/agent>/cache/terminal-todos/<version>/<target>`.
  Keep executable caches outside Pi's deprecated `tools/` directory to avoid startup warnings.
  Corrupt caches fail closed; the error identifies only the cache to remove.
- Arbitrary PATH `todo` executables are never probed or used: the old Swift CLI
  can interpret flags as new tasks. Homebrew, PATH and real tasks are not changed
  by installation. The native app uses its existing Rust data/import behavior.
- macOS arm64/x64 builds target macOS 11+. Linux arm64/x64 builds use static musl.
  Other platforms are rejected without installing anything.

## Project attribution and data compatibility (v1.4.0)

`runDashboard` already passes `ctx.cwd` as the native process's working directory.
Rust defaults to the Git top-level folder name, falling back to the current folder
when Git is unavailable or it is not a repository. The dashboard's `p` overrides
that creation project for the current session; the CLI still detects its cwd.
Store only the name on new tasks, never the absolute path. Project colors are derived from that name in
Rust and shared between TUI and CLI, not saved in the database. Existing tasks
and Swift imports remain unassigned; edits/status changes keep the saved origin.

The app accepts schema 1 and 2, writing schema 2. Reading an existing schema-1
file does not migrate it. Before the first successful mutation, an exact
`todos.v1.backup.json` is atomically created beside the data file under the same
lock. An existing different backup blocks migration rather than being replaced.
Schema-2 files are rejected by v1.3.0 and older without overwriting them. Upgrade
all Pi/CLI readers before sharing migrated data; preserve the current schema-2
file before restoring a backup because the backup excludes post-upgrade changes.

v1.4.0 has matching native release assets and a separate Homebrew source archive.
The Homebrew formula/tap is updated independently to support schema 2. The older
v1.2.0/v1.3.0 tags, assets and checksums remain immutable.

## Verification

```bash
npm test
cargo test --locked
cargo build --release --locked
uv run --no-project --with pyte scripts/test_tui.py target/release/todo
uv run --no-project --with pyte scripts/test_pi_tui.py /absolute/path/to/pi/dist/bundle/cli.js
```

The PTY suite uses the installed Pi itself, temporary configuration and task
storage, a dummy model without inference, and an intercepted download transport.
It covers both terminal modes, approval/decline, installation, CLI-shared tasks,
project capture/selection from Pi's nested Git working directory, original-label
preservation, full OSC52 note payloads, shortcut/editor-draft preservation, and
busy-turn waiting through a held local fake SSE stream (no inference/network model).
Pi 1.0.4 passes both fullscreen and regular modes. It also checks offline cache hits,
Ctrl+C/SIGINT/SIGTERM, child failure, cancelled downloads, checksum failure and
missing release assets.
It places a trap on PATH to prove the old CLI is never invoked.

Try without registering a global package:

```bash
pi -e .
```

## Release

`.github/workflows/release.yml` tests/builds all four native targets. Main,
PR and manual runs produce artifacts only. An explicitly pushed matching
`v<version>` tag creates a new release with four executables and their checksums.
It refuses mismatched tags and existing releases; it does not replace assets.
Only push a release tag after release approval. Previous releases remain untouched;
a new Homebrew version gets a separately uploaded source archive/checksum and tap update.

Local native packaging (no publish):

```bash
node scripts/package_binary.mjs aarch64-apple-darwin target/release/todo
```

The first `/todo` install requires the matching release assets to be published;
missing assets produce a clear error, not an unapproved Cargo/Rust installation.
Pi-package git checkouts carry the extension; source/Homebrew release packaging
and tap updates remain a separate, explicitly approved operation.
