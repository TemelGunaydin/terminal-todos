# Original Swift release

The original `Package.swift` and `Sources/` are retained unchanged for reference and rollback.

```bash
cd legacy/swift
swift run todo_terminal list
```

This version reads/writes `~/.swift_todos.json`. Rust imports that file once into its own store without modifying it. The two versions are not live-synchronized; returning to Swift returns to the pre-migration snapshot, not to changes made in Rust. Back up the Rust data before rollback.

Do not run this legacy executable during Rust development tests against your real HOME. Use an isolated account/directory if exercising legacy data.
