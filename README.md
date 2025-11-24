# 📝 Terminal Todos

**Terminal Todos** is a lightweight command-line (CLI) todo manager written in Swift.  
You can add, list, update, and delete todos directly from the terminal.

---

## 🚀 Features

- ✅ Add todos quickly from the terminal  
- 📋 List all todos at any time  
- ✏️ Update a todo’s text  
- 🗑 Delete a todo  
- 💾 Data is stored in `~/.swift_todos.json`  
- 🍺 Installable via Homebrew  
- ⚡ Built in Swift, lightweight and fast  

---

## 📦 Installation

### With Homebrew (Recommended)

```bash
brew tap TemelGunaydin/tap
brew install todo
```

---

## 🛠 Usage

Add a new todo:

```bash
todo add "Kitap oku"
```

List all todos:

```bash
todo list
```

Update a todo:

```bash
todo update 1 "Yeni metin"
```

Delete a todo:

```bash
todo delete 1
```

---

## 📁 Data File

The application automatically stores todos here:

```text
~/.swift_todos.json
```
