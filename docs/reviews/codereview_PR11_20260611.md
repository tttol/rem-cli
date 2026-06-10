# The result of review
Add task deadlines and overdue highlighting

## ❌CRITICAL
None.

## 🔴HIGH
### Comment1: Existing task migration can corrupt the original file
The deadline migration writes directly to the task path with `fs::write`. If the write is interrupted or fails after truncation, the original task and its markdown body can be lost. Write the migrated content to a temporary file and replace the original only after the complete write succeeds.

## 🟡MEDIUM
### Comment1: Legacy deadline values are not normalized
The loader accepts `yyyy-MM-dd`, but it leaves the old value unchanged on disk. This means the repository can indefinitely contain both formats despite `yyyy/MM/dd` being the current frontmatter contract. Normalize legacy values during the existing migration path.

## 🔵LOW
None.
