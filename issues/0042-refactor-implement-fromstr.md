# StreamType/StreamMode が FromStr trait を実装せず clippy を抑制している

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-implement-fromstr

## 目的

`src/stream_id.rs:50,84` の `StreamType::from_str` と `StreamMode::from_str` が、標準の `FromStr` trait ではなく独自メソッドで実装され、`#[allow(clippy::should_implement_trait)]` で抑制されている。

## 現状

```rust
#[allow(clippy::should_implement_trait)]
pub fn from_str(s: &str) -> Option<Self> {
```

## 設計方針

`impl std::str::FromStr for StreamType` および `impl std::str::FromStr for StreamMode` を実装する。既存の `from_str` メソッドは削除し、呼び出し元を `s.parse()` に変更する。

## 完了条件

- `FromStr` trait が実装されていること
- `#[allow(clippy::should_implement_trait)]` が削除されていること
- `cargo test` で全テストが通過すること
