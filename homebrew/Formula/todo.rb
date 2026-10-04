class Todo < Formula
  desc "Keyboard-first terminal todo dashboard written in Rust"
  homepage "https://github.com/TemelGunaydin/terminal-todos"
  url "https://github.com/TemelGunaydin/terminal-todos/releases/download/v1.2.0/terminal-todos-1.2.0.tar.gz"
  version "1.2.0"
  sha256 "3f9db352961b6481123fa00598fbf49ccfb2ecf85ba7ba562c3a89099e762c3f"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    with_env("HOME" => testpath.to_s, "XDG_DATA_HOME" => (testpath/"data").to_s, "NO_COLOR" => "1") do
      legacy = testpath/".swift_todos.json"
      original = "[\"Legacy task\"]\n"
      legacy.write original

      assert_match "todo 1.2.0", shell_output("#{bin}/todo --version")
      assert_match "dashboard", shell_output("#{bin}/todo --help")
      assert_match "[ ] #1 Legacy task", shell_output("#{bin}/todo list --all")
      system bin/"todo", "add", "Homebrew task"
      assert_match "[ ] #2 Homebrew task", shell_output("#{bin}/todo list")
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

      data = testpath/"data/terminal-todos/todos.json"
      data.write "broken json"
      assert_match "Invalid task data", shell_output("#{bin}/todo add New 2>&1", 1)
      assert_equal "broken json", data.read
    end
  end
end
