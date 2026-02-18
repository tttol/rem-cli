# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

rem-cli is a TUI (Terminal User Interface) TODO management tool written in Rust. The core concept is that all TODO data is stored locally on the filesystem, ensuring data privacy for sensitive information. The binary name is `rem` (invoked via `rem` command).

## Build Commands

```bash
cargo build          # Build the project
cargo run            # Run the application (binary: rem)
cargo test           # Run all tests
cargo test <name>    # Run a specific test
cargo clippy         # Run linter
cargo fmt            # Format code
```

## Architecture

- **TUI Framework**: ratatui (v0.30.0) with crossterm (v0.29.0) backend
- **Source Files**:
  - `src/main.rs`: Terminal setup/cleanup, event loop, neovim integration
  - `src/app.rs`: Application state (`App` struct), input handling, mode management
  - `src/render.rs`: UI rendering logic (layout, task lists, preview panel)
  - `src/task.rs`: Task data model, filesystem I/O, status management

## Data Storage

Tasks are stored as markdown files under `~/.rem-cli/tasks/` with directory-based status management:

```
~/.rem-cli/tasks/
  todo/<uuid>.md
  doing/<uuid>.md
  done/<uuid>.md
```

- Status is determined by which directory the file resides in (not by frontmatter)
- Frontmatter contains: `id`, `name`, `created_at`, `updated_at` (no `status` field)
- Status changes move the file between directories via `fs::rename`

## Key Patterns

- Terminal enters raw mode and alternate screen on startup
- Event polling with 100ms timeout
- Key events are handled only on `KeyEventKind::Press`
- Clean terminal restoration on exit (disable raw mode, leave alternate screen)
- Two input modes: `Normal` (navigation/actions) and `Editing` (text input for new tasks)
- Done tasks are lazy-loaded on demand (`d` key toggles) to keep startup fast
- Preview panel (right 70%) shows the selected task's markdown content, updated on cursor movement
- Neovim integration: Enter key temporarily exits TUI, opens task file in nvim, then restores TUI
- `open_file: Option<PathBuf>` is used as a message-passing mechanism between `App` (state) and `main` (terminal control)
- `--version` / `-V` flag prints version and exits without entering TUI

## CI/CD

- GitHub Actions workflow (`.github/workflows/release.yml`) builds release binaries on tag push (`v*`)
- Targets: macOS (aarch64, x86_64), Linux (x86_64, aarch64)
- Release artifacts are uploaded to GitHub Releases via `softprops/action-gh-release`
- Distributed via Homebrew tap (`tttol/tap`)
