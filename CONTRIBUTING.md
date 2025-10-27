# Codex Notes
- Run `cargo fmt` and `cargo check` before sending patches.
- Build in release mode with `cargo build --release`.
- After any change that touches the CLI, reinstall the binary locally with  
  `cargo install --path . --root ~/.local --force` so `~/.local/bin/xrandr-utils` stays in sync.
