# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

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
  - `src/main.rs`: Thin binary entry point and process exit status
  - `src/lib.rs`: Composition root and terminal event loop; the crate's only public API is `run()`
  - `src/domain/task.rs`: Immutable task model, lifecycle transitions, sorting, and DONE week ranges
  - `src/application/app.rs`: Application state, input commands/effects, selection, and task workflow policy
  - `src/application.rs`: Narrow `TaskRepository` port owned by the application layer
  - `src/infrastructure/`: Configuration loading and the filesystem/YAML repository adapter
  - `src/presentation/`: Read-only Ratatui rendering and terminal/neovim lifecycle management

## Data Storage

Tasks are stored as markdown files under `~/.rem-cli/tasks/` with directory-based status management:

```
~/.rem-cli/tasks/
  parking/<uuid>.md
  todo/<uuid>.md
  doing/<uuid>.md
  done/<uuid>.md
```

- Status is determined by which directory the file resides in (not by frontmatter)
- Frontmatter contains: `id`, `name`, `created_at`, `updated_at`, optional `completed_at`, and `deadline` in `yyyy/MM/dd` format (no `status` field)
- Task timestamps use local `NaiveDateTime` values without timezone information
- Status changes move the file between directories via `fs::rename`

## Key Patterns

- Terminal enters raw mode and alternate screen on startup
- Event polling with 100ms timeout
- Key events are handled only on `KeyEventKind::Press`
- Clean terminal restoration on exit (disable raw mode, leave alternate screen)
- Two input modes: `Normal` (navigation/actions) and `Editing` (text input for new tasks)
- PARKING tasks are loaded after the first frame is rendered
- DONE tasks are lazy-loaded on demand (`d` key toggles) and filtered by completion week
- DONE review weeks run from Monday through Sunday; `[` / `]` navigate weeks
- Status columns are displayed horizontally as PARKING / TODO / DOING / DONE
- DONE is hidden by default and toggled with the `d` key
- `j` / `k` move within a status column, while `h` / `l` move between non-empty columns
- `n` / `N` move the selected task forward or backward through the status lifecycle
- Neovim integration: Enter key temporarily exits TUI, opens task file in nvim, then restores TUI
- `AppEffect` passes quit and open-task requests from application policy to the runtime boundary
- After returning from neovim, `App::after_edit()` reloads task metadata through `TaskRepository`
- Application and rendering code receive explicit wall-clock values; filesystem and terminal side effects remain in adapters
- Task selection is identity-based and can explicitly represent an empty selected status column
- Application state is private and rendering consumes an immutable `AppView`
- Task names are wrapped to fit each status column (`wrap_task_name`)
- Editing mode supports cursor movement, insertion, deletion, and horizontal scrolling
- `--version` / `-V` flag prints version and exits without entering TUI

## CI/CD

- GitHub Actions workflow (`.github/workflows/release.yml`) builds release binaries on tag push (`v*`)
- GitHub Actions workflow (`.github/workflows/test.yml`) runs `cargo test` on push to main and PRs targeting main
- Targets: macOS (aarch64, x86_64), Linux (x86_64, aarch64)
- Release artifacts are uploaded to GitHub Releases via `softprops/action-gh-release`
- Distributed via Homebrew tap (`tttol/tap`)

## Code Reviews

- AI code review results are stored in `docs/reviews/` as markdown files
- Naming convention: `ai-review-result_issue-<N>_<YYYYMMDD>.md`
