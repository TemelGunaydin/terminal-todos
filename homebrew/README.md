# Rust Homebrew release packaging

[`homebrew/Formula/todo.rb`](https://github.com/TemelGunaydin/terminal-todos/blob/main/homebrew/Formula/todo.rb) builds the standalone Rust app with:

```ruby
depends_on "rust" => :build

system "cargo", "install", *std_cargo_args
```

The Rust version is **1.5.0**, including full-note OSC52 clipboard requests, session-local project selection, automatic project labels and schema-2 data support. The formula retains the tap's MIT license declaration and has no Swift-only macOS restriction. Native application tests/builds pass for macOS and Linux on arm64/x64; the actual Homebrew formula install/test has been exercised locally on macOS.

The formula points to an immutable GitHub release asset. Publishing a formula does not change an existing installation: users choose when to run `brew update` and `brew upgrade temelgunaydin/tap/todo`.

## Prepare the actual release archive and checksum

Requires Python 3.11+:

```bash
cargo check --locked
python3 scripts/package_homebrew.py
```

Run from the repository root. This creates:

- `dist/terminal-todos-1.5.0.tar.gz`
- `dist/terminal-todos-1.5.0.tar.gz.sha256`
- `homebrew/Formula/todo.rb`, with the SHA-256 of that exact archive.

The source archive includes the lockfile, Rust sources/tests, original Swift sources for rollback, icon, real dashboard screenshot, README/developer notes, and installation/test scripts. It excludes `.git`, `.pi`, build output, and user data. Stable file order, normalized metadata, and a fixed gzip timestamp make identical inputs reproduce the same checksum.

**Re-run packaging after any included file changes.** The formula intentionally does not point to GitHub's automatically generated source tarball: its checksum will differ. Upload the exact generated `.tar.gz` as a release asset.

## Validate without changing your Homebrew installation

```bash
ruby -c homebrew/Formula/todo.rb
HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_DEVELOPER=1 brew style homebrew/Formula/todo.rb
HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_DEVELOPER=1 brew ruby scripts/test_homebrew.rb
```

The last command loads Homebrew's real formula class and runs its `install` and `test` methods with temporary prefix, HOME, logs, and build directories. It verifies the archive's actual checksum, builds using `std_cargo_args` (including `--locked`), and exercises migration, add/list/update/done/undo/delete, saved project attribution, schema-2 data, stable IDs, unchanged legacy data, and corruption protection.

It uses the current Rust toolchain/Cargo cache and does **not** run `brew install`, link files into Homebrew's prefix, edit the live tap, or change your tasks. This is a scoped local formula check, not a published-URL fetch or a full bottle build.

The formula test requires Homebrew's `minitest` dependency. In a writable Homebrew checkout, it can be provisioned with `brew install-bundler-gems --groups=formula_test`. For Nix-managed Homebrew, do not change the read-only `/nix/store` or the system installation to run development checks: use a temporary, writable Homebrew checkout instead. The formula passed `brew style` in an isolated Homebrew **7.0.4** checkout, and its install/test methods were exercised with temporary installation and data directories.

## Publishing a release

Publishing or updating the tap is a separate, explicit approval step. After review and approval:

1. Commit the scoped Rust/branding/packaging changes, excluding unrelated `.pi` files.
2. Verify the generated archive matches that source, and run the validation commands.
3. After all four native targets pass CI, publish tag/release **`v1.5.0`** on `TemelGunaydin/terminal-todos`. The tag workflow publishes native binaries/checksums; upload the exact separately generated source archive and checksum. Verify the archive's Rust sources, manifest and lockfile match the released tag.
4. Download the published asset and verify its SHA-256 against the formula.
5. Copy `homebrew/Formula/todo.rb` into **`TemelGunaydin/homebrew-tap`**, replacing `Formula/todo.rb`; review and commit/push that tap change separately.
6. Only then should users run `brew update` and `brew upgrade temelgunaydin/tap/todo`.

A published release archive should be immutable. If its source changes after publication, create a new version rather than overwriting the asset. The previous v1.2.0/v1.3.0/v1.4.0 tags, assets and checksums remain unchanged.

## Rollback

Previous formulas are available in the tap's Git history. Before the first successful write to schema-1 Rust data, v1.4.0 preserves an exact `todos.v1.backup.json` beside the current file. Earlier Rust versions reject schema 2; keep the current file before restoring a schema-1 backup, which excludes post-upgrade changes. All Pi/CLI instances sharing migrated data must use v1.4.0+.

The Rust application leaves `~/.swift_todos.json` intact, and the original Swift sources remain in `legacy/swift/`.

Switching back to Swift restores the old snapshot, **not** tasks edited in Rust after import. Back up `~/.local/share/terminal-todos/todos.json` (or the configured XDG data path) before any rollback.
