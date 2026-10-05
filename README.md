<p align="center">
  <img src="assets/terminal-todos-icon.png" width="144" alt="Terminal Todos">
</p>

<h1 align="center">Terminal Todos</h1>

<p align="center">A simple, keyboard-first todo app for your terminal. Built with Rust.</p>

![Terminal Todos dashboard with sample tasks](assets/dashboard.png)

## Install

```bash
brew tap TemelGunaydin/tap
brew install todo
```

## Pi

```bash
pi install git:github.com/TemelGunaydin/terminal-todos
```

Use `/todo` or **Ctrl+Alt+T**. First use asks to install a verified binary (macOS/Linux, arm64/x64); `q` returns to Pi. No Rust needed.

## Usage

Open the dashboard:

```bash
todo
```

**Keys:** `↑` / `↓` navigate · `a` add · `e` edit · `Space` complete · `d` delete · `/` search · `q` quit

Or use CLI commands:

```bash
todo add "Read a book"
todo list
todo done 1
todo undo 1
todo update 1 "Read two chapters"
todo delete 1
todo --help
```

Your tasks stay on your device. Existing Swift tasks are imported automatically without changing the original file.
