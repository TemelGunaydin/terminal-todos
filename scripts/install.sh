#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
prefix="${PREFIX:-$HOME/.local}"

cargo install --path "$root" --root "$prefix" --force --locked
printf '\nInstalled: %s/bin/todo\n' "$prefix"
printf 'Open the dashboard: %s/bin/todo\n' "$prefix"
printf 'Make sure %s/bin is before other todo installations in your PATH.\n' "$prefix"
printf 'Check the selected executable with: command -v todo\n'
