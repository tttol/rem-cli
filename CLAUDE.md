# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

rem-cli is a TUI (Terminal User Interface) TODO management tool written in Rust. The core concept is that all TODO data is stored locally on the filesystem, ensuring data privacy for sensitive information.

## Build Commands

```bash
cargo build          # Build the project
cargo run            # Run the application
cargo test           # Run all tests
cargo test <name>    # Run a specific test
cargo clippy         # Run linter
cargo fmt            # Format code
```

## Architecture

- **TUI Framework**: ratatui (v0.30.0) with crossterm (v0.29.0) backend
- **Application Pattern**: Event loop with state management in `App` struct
  - `main()`: Terminal setup, event loop, cleanup
  - `App`: Application state and input handling
  - `render()`: UI rendering logic

## Key Patterns

- Terminal enters raw mode and alternate screen on startup
- Event polling with 100ms timeout
- Key events are handled only on `KeyEventKind::Press`
- Clean terminal restoration on exit (disable raw mode, leave alternate screen)
