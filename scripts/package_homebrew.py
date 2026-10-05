#!/usr/bin/env python3
"""Build a deterministic source release and a formula with its actual SHA-256.

Local preparation only: this script does not publish, push, or modify a tap.
"""
import gzip
import hashlib
from pathlib import Path
import re
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent
FORMULA = '''class Todo < Formula
  desc "Keyboard-first terminal todo dashboard written in Rust"
  homepage "https://github.com/TemelGunaydin/terminal-todos"
  url "https://github.com/TemelGunaydin/terminal-todos/releases/download/v@VERSION@/terminal-todos-@VERSION@.tar.gz"
  version "@VERSION@"
  sha256 "@SHA256@"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    with_env("HOME" => testpath.to_s, "XDG_DATA_HOME" => (testpath/"data").to_s, "NO_COLOR" => "1") do
      legacy = testpath/".swift_todos.json"
      original = "[\\"Legacy task\\"]\\n"
      legacy.write original

      assert_match "todo @VERSION@", shell_output("#{bin}/todo --version")
      assert_match "dashboard", shell_output("#{bin}/todo --help")
      assert_match "[ ] #1 Legacy task", shell_output("#{bin}/todo list --all")
      system bin/"todo", "add", "Homebrew task"
      assert_match "[ ] #2 Homebrew task", shell_output("#{bin}/todo list")
      data = testpath/"data/terminal-todos/todos.json"
      saved = JSON.parse(data.read)
      assert_equal 2, saved.fetch("version")
      assert_nil saved.fetch("tasks").first["project"]
      assert_equal testpath.basename.to_s, saved.fetch("tasks")[1].fetch("project")
      system bin/"todo", "done", "2"
      assert_match "[x] #2 Homebrew task", shell_output("#{bin}/todo list --done")
      system bin/"todo", "update", "2", "Updated task"
      system bin/"todo", "undo", "2"
      system bin/"todo", "delete", "1"
      assert_match "[ ] #2 Updated task", shell_output("#{bin}/todo list --all")
      system bin/"todo", "add", "Next task"
      assert_match "[ ] #3 Next task", shell_output("#{bin}/todo list")
      assert_equal original, legacy.read
      assert_match "was not found", shell_output("#{bin}/todo done 99 2>&1", 1)

      data.write "broken json"
      assert_match "Invalid task data", shell_output("#{bin}/todo add New 2>&1", 1)
      assert_equal "broken json", data.read
    end
  end
end
'''


def package():
    metadata = tomllib.loads((ROOT / "Cargo.toml").read_text())
    version = metadata["package"]["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("A stable numeric release version is required")
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    entry = next(p for p in lock["package"] if p["name"] == "terminal-todos")
    if entry["version"] != version:
        raise ValueError("Cargo.lock version is stale; run cargo check first")

    # Explicit inclusion avoids packaging .git, .pi, build output or user data.
    paths = [ROOT / name for name in [
        "Cargo.toml", "Cargo.lock", "README.md", "assets/terminal-todos-icon.png",
        "assets/dashboard.png", "docs/pi.md",
        "scripts/install.sh", "scripts/test_tui.py", "scripts/package_homebrew.py",
        "scripts/test_homebrew.rb", "homebrew/README.md", "legacy/swift/README.md",
        "legacy/swift/Package.swift",
    ]]
    for directory, pattern in [("src", "*.rs"), ("tests", "*.rs"), ("legacy/swift/Sources", "*.swift")]:
        paths.extend((ROOT / directory).rglob(pattern))
    if any(not path.is_file() or path.is_symlink() for path in paths):
        raise ValueError("A required source file is missing or is a symlink")

    dist = ROOT / "dist"
    dist.mkdir(exist_ok=True)
    archive = dist / f"terminal-todos-{version}.tar.gz"
    temporary = archive.with_suffix(".tmp")
    try:
        with temporary.open("wb") as output:
            with gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as tar:
                    for path in sorted(set(paths)):
                        relative = path.relative_to(ROOT)
                        info = tar.gettarinfo(str(path), arcname=f"terminal-todos-{version}/{relative}")
                        info.uid = info.gid = info.mtime = 0
                        info.uname = info.gname = ""
                        info.mode = 0o755 if relative.parts[0] == "scripts" else 0o644
                        with path.open("rb") as source:
                            tar.addfile(info, source)
        temporary.replace(archive)
    finally:
        temporary.unlink(missing_ok=True)
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    formula = ROOT / "homebrew/Formula/todo.rb"
    formula.parent.mkdir(parents=True, exist_ok=True)
    formula.write_text(FORMULA.replace("@VERSION@", version).replace("@SHA256@", checksum))
    (dist / f"{archive.name}.sha256").write_text(f"{checksum}  {archive.name}\n")
    print(f"Archive: {archive}\nSHA-256: {checksum}\nFormula: {formula}")
    print("Not published. Upload this exact archive before updating the live tap.")


if __name__ == "__main__":
    package()
