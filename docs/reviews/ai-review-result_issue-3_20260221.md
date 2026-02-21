# The result of review
Reload TUI window after editing a task in neovim (PR #5 / Issue #3)

## ❌CRITICAL
なし

## 🔴HIGH
なし

## 🟡MEDIUM

### Comment1: コメントの言語がCLAUDE.mdに違反している
`src/app.rs` に追加された `reload_selected_task` のdocコメントが日本語になっている。

```rust
// 現在（違反）
/// nvimで編集したタスクをファイルから再読み込みし、メモリ上の情報を最新化する。
pub fn reload_selected_task(&mut self) {
```

CLAUDE.md には「Output source-code comment in English. However, for repositories under ~/Git/, please output comments in Japanese」と定義されている。本リポジトリのパスは `~/gitworktree/rem-cli/` であり `~/Git/` 配下ではないため、コメントは英語で記述する必要がある。

```rust
// 修正案
/// Reloads the selected task from its markdown file to reflect the latest metadata in memory.
pub fn reload_selected_task(&mut self) {
```

## 🔵LOW

### Comment1: Task::load を public にすることによる API 表面積の拡大
`Task::load` は汎用的な内部ロジックであり、`path` と `status` を別々に渡す設計は呼び出し側の責務が大きい。`App` から呼び出すためだけに public にするのであれば、`Task` 自身に `reload` メソッドを持たせてカプセル化を維持する方が望ましい。

```rust
// 修正案: Task に reload メソッドを追加し、load は private のまま維持する
impl Task {
    pub fn reload(&self) -> io::Result<Self> {
        Self::load(&self.file_path(), self.status.clone())
    }
}

// App 側の呼び出し
pub fn reload_selected_task(&mut self) {
    if let Some(index) = self.selected_index {
        if let Ok(reloaded) = self.tasks[index].reload() {
            self.tasks[index] = reloaded;
        }
    }
}
```

### Comment2: reload_selected_task のエラーが無音で無視される
`Task::load` が失敗した場合（ファイル削除・権限エラー等）、現在は `if let Ok` で黙殺されるため、ユーザーには何も通知されない。コードベース全体で同様のパターン（`toggle_done` など）が使われており一貫性はあるが、将来的にはエラーをステータスバーや通知として表示する仕組みを検討する余地がある。

### Comment3: main.rs の呼び出し順が分かりにくい
`reload_selected_task()` と `update_preview()` を main 側で続けて呼び出す構造は、呼び出し側がリロードの必要性を知っている必要があり、encapsulation が弱い。nvim 編集後の後処理をまとめた `after_edit()` のような1メソッドに集約すると、将来の変更点が1か所に限定されて保守性が上がる。

```rust
// 修正案（app.rs）
pub fn after_edit(&mut self) {
    self.reload_selected_task();
    self.update_preview();
}

// main.rs 側
app.after_edit();
```
