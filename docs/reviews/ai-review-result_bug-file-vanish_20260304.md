# The result of review
Fix bug that the file content is cleared when updating status (PR #8)

## ❌CRITICAL
Nothing.

## 🔴HIGH
Nothing.

## 🟡MEDIUM
Nothing.

## 🔵LOW

### Comment1: Silent error suppression in `update_status`
`src/task.rs` - `update_status` method

`let _ = self.update_frontmatter_preserving_body();` silently discards the `io::Result`. If writing fails (e.g. disk full), the file will have been moved to the new directory but its frontmatter (including `updated_at`) will remain stale with no indication to the caller. This is consistent with the existing pattern in the same method, but worth noting as a future improvement.

### Comment2: Frontmatter delimiter ambiguity in body extraction
`src/task.rs` - `update_frontmatter_preserving_body` method

The body is extracted by searching for the first occurrence of `"\n---\n"` in the content after the opening delimiter. If a user writes `\n---\n` inside the markdown body (e.g. a YAML code block or a horizontal rule sequence), the extraction will silently truncate the body at that point. This is an inherent limitation of simple string-based frontmatter parsing, and unlikely to matter in practice given how the app is used.

### Comment3: Verbose test setup line
`src/task.rs` - `update_status_moves_file_between_directories` test
`tests/integration_test.rs` - `forward_status_moves_md_file_to_next_directory` test

```rust
// Current: hard to read at a glance
fs::write(task.file_path(), format!("{}{}", fs::read_to_string(task.file_path()).unwrap(), body)).unwrap();

// Clearer:
let existing = fs::read_to_string(task.file_path()).unwrap();
fs::write(task.file_path(), format!("{}{}", existing, body)).unwrap();
```
