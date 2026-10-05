# Pi integration — development and release

The package exposes only `extensions/todo.js`. It registers `/todo` and
`Ctrl+Alt+T`; startup never downloads or runs the app. Pi keeps ownership
of the terminal for confirmation/download, then suspends its TUI while
the native dashboard runs. Print/RPC/JSON modes and busy agents cannot launch it.

## Installation contract

- The extension and app share the version in `package.json` and `Cargo.toml`.
- Only approved first use fetches the fixed GitHub release's native executable
  and its `.sha256`. The checksum must name the exact platform/version asset.
- Downloads are bounded, cancellable and verified before execution. A version
  probe uses a temporary HOME/data directory; incompatible binaries are rejected.
- The complete binary/receipt directory is committed by rename. Later launches
  verify its SHA-256 and executable permission, without network access.
- Cache: `<PI_CODING_AGENT_DIR or ~/.pi/agent>/tools/terminal-todos/<version>/<target>`.
  Corrupt caches fail closed; the error identifies only the cache to remove.
- Arbitrary PATH `todo` executables are never probed or used: the old Swift CLI
  can interpret flags as new tasks. Homebrew, PATH and real tasks are not changed
  by installation. The native app uses its existing Rust data/import behavior.
- macOS arm64/x64 builds target macOS 11+. Linux arm64/x64 builds use static musl.
  Other platforms are rejected without installing anything.

## Project attribution and data compatibility (v1.4.0, unreleased)

`runDashboard` already passes `ctx.cwd` as the native process's working directory.
Rust captures the Git top-level folder name, falling back to the current folder
when Git is unavailable or it is not a repository. Store only the name on new
tasks, never the absolute path. Project colors are derived from that name in
Rust and shared between TUI and CLI, not saved in the database. Existing tasks
and Swift imports remain unassigned; edits/status changes keep the saved origin.

The app accepts schema 1 and 2, writing schema 2. Reading an existing schema-1
file does not migrate it. Before the first successful mutation, an exact
`todos.v1.backup.json` is atomically created beside the data file under the same
lock. An existing different backup blocks migration rather than being replaced.
Schema-2 files are rejected by v1.3.0 and older without overwriting them. Upgrade
all Pi/CLI readers before sharing migrated data; preserve the current schema-2
file before restoring a backup because the backup excludes post-upgrade changes.

v1.4.0 is prepared locally, not published. Released v1.2.0/v1.3.0 artifacts must
remain immutable. The next approved release must include matching native assets;
Homebrew needs its separately approved source archive/tap update for schema 2.

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
project capture from Pi's nested Git working directory, original-label preservation,
shortcut/editor-draft preservation, offline cache hits, Ctrl+C/SIGINT/SIGTERM, child
failure, cancelled downloads, checksum failure and missing release assets.
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
Only push a release tag after release approval. The existing `v1.2.0` source
archive, checksum and Homebrew formula remain untouched.

Local native packaging (no publish):

```bash
node scripts/package_binary.mjs aarch64-apple-darwin target/release/todo
```

The first `/todo` install requires the matching release assets to be published;
missing assets produce a clear error, not an unapproved Cargo/Rust installation.
Pi-package git checkouts carry the extension; source/Homebrew release packaging
and tap updates remain a separate, explicitly approved operation.
