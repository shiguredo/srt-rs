# error.rs の全フィールドが pub でカプセル化されていない

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-error-encapsulation

## 目的

`src/error.rs` の `Error` 構造体の全フィールド (`kind`, `reason`, `location`, `backtrace`) が `pub` で露出している。また `Debug` 実装が `Display` と同一で、`unwrap()` 失敗時に長大なバックトレースを含む `Display` 出力が表示される。

## 優先度根拠

内部表現をカプセル化せず公開することで、将来の内部構造変更が破壊的変更になる。`Debug` と `Display` の区別がないことは Rust の慣習に反する。

## 現状

```rust
pub struct Error {
    pub kind: ErrorKind,
    pub reason: String,
    pub location: &'static Location<'static>,
    pub backtrace: Backtrace,
}
```

```rust
impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")  // Display に委譲
    }
}
```

## 設計方針

1. フィールドを非公開にし、アクセサメソッドを提供する
2. `Debug` 実装を構造的表現（フィールド名付き）に変更し、`Display` はユーザ向けの短いメッセージを維持する

## 完了条件

- `Error` のフィールドが非公開になっていること
- 必要なアクセサメソッドが提供されていること
- `Debug` 出力にバックトレースが含まれないこと
- `cargo test` で全テストが通過すること
