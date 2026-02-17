![Static Badge](https://img.shields.io/badge/Rust-gray?logo=rust)

# rem-cli - Remember everything locally

A TUI (Terminal User Interface) TODO management tool. `rem` stands for "remember". All data is stored locally on the filesystem, keeping your sensitive information private.

## ✨ Highlights

- **Written in Rust** - Fast, safe, and reliable
- **Local-first** - All TODO data stored on your filesystem, no cloud sync
- **TUI** - Interactive terminal interface powered by ratatui
- **Vim-like keybindings** - Navigate with j/k
- **Preview panel** - View task details without leaving the TUI
- **Neovim integration** - Edit task files directly in neovim

## 🤔 Why rem-cli?

**Before:** Scattering TODOs across multiple apps, cloud services, and sticky notes...

**After:** One terminal command, all your tasks in one place, fully offline.

- No account signup or cloud dependency
- Sensitive tasks stay on your machine
- Markdown files you can version control or back up however you like

## 🚀 Features

- **Three-column status management** - TODO / DOING / DONE
- **Keyboard-driven workflow** - Add, navigate, and update tasks without touching the mouse
- **Live preview** - Right panel (70%) shows the selected task's markdown content
- **Lazy loading** - DONE tasks are loaded on demand to keep startup fast
- **Neovim integration** - Press Enter to open and edit a task file in neovim

## ⌨️ Keybindings

| Key | Action |
|-----|--------|
| `a` | Add a new task |
| `j` / `k` | Navigate down / up |
| `n` | Move task to next status (TODO -> DOING -> DONE) |
| `d` | Toggle DONE tasks visibility |
| `Enter` | Open task file in neovim |
| `q` / `Esc` | Quit |

## 📦 Installation

### macOS
```bash
brew tap tttol/tap
brew install rem-cli
```

### Linux
```bash
# For x86_64 (Intel/AMD)
curl -LO https://github.com/tttol/rem-cli/releases/latest/download/rem-cli-x86_64-unknown-linux-gnu.tar.gz
tar xzf rem-cli-x86_64-unknown-linux-gnu.tar.gz
sudo mv rem-cli /usr/local/bin/
```

```bash
# For aarch64 (ARM64)
curl -LO https://github.com/tttol/rem-cli/releases/latest/download/rem-cli-aarch64-unknown-linux-gnu.tar.gz
tar xzf rem-cli-aarch64-unknown-linux-gnu.tar.gz
sudo mv rem-cli /usr/local/bin/
```

### Build from source
```bash
git clone https://github.com/tttol/rem-cli.git
cd rem-cli
cargo install --path .
```

## 💾 Data Storage

Tasks are stored as markdown files under `~/.rem-cli/tasks/` with directories representing status:

```
~/.rem-cli/tasks/
  todo/
    <uuid>.md
  doing/
    <uuid>.md
  done/
    <uuid>.md
```

Each file contains YAML frontmatter with task metadata. You can freely edit, back up, or version control these files.

## 🌟 Community

- **[Report issues](https://github.com/tttol/rem-cli/issues)** - Found a bug? Let us know
- **Contributing** - PRs are welcome!
