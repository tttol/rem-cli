# Contributing to rem-cli

Thank you for your interest in contributing to rem-cli!

## Getting Started

1. Fork the repository
2. Clone your fork
   ```bash
   git clone https://github.com/<your-username>/rem-cli.git
   cd rem-cli
   ```
3. Create a branch
   ```bash
   git checkout -b feature/your-feature-name
   ```

## Development

### Prerequisites

- Rust (stable, latest version recommended)

### Build & Run

```bash
cargo build          # Build the project
cargo run            # Run the application
cargo test           # Run all tests
cargo clippy         # Run linter
cargo fmt            # Format code
```

### Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and resolve all warnings
- Keep changes minimal and focused

## Pull Requests

1. Ensure your code builds without warnings (`cargo build`, `cargo clippy`)
2. Format your code (`cargo fmt`)
3. Write a clear PR title and description
4. Keep PRs small and focused on a single change

## Reporting Issues

- Use [GitHub Issues](https://github.com/tttol/rem-cli/issues)
- Include steps to reproduce the problem
- Include your OS and Rust version
