# StreamType/StreamMode が FromStr trait を実装せず clippy を抑制している

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-implement-fromstr
- Polished: 2026-08-01

## 目的

`src/stream_id.rs` の `StreamType::from_str` と `StreamMode::from_str` が、標準の `FromStr` trait ではなく独自メソッドで実装され、`#[allow(clippy::should_implement_trait)]` で抑制されている。shiguredo-rust スキルは lint 抑制に `#[expect]` を使うことを定めており、`#[allow]` での抑制は規約に反する。標準 `FromStr` を実装して抑制自体をなくす。

## 現状

```rust
#[allow(clippy::should_implement_trait)]
pub fn from_str(s: &str) -> Option<Self> {
```

呼び出し元は 6 箇所:

- `src/stream_id.rs` の `AccessControl::parse` 内 2 箇所 (`if let Some(t) = StreamType::from_str(value)` 形式。不正値は黙殺してデフォルト値を維持する)
- `pbt/tests/prop_stream_id.rs` の 4 箇所 (`prop_assert_eq!(parsed, Some(stream_type))` / `prop_assert!(parsed.is_none())` 形式)

## 設計方針

- `impl std::str::FromStr for StreamType` および `impl std::str::FromStr for StreamMode` を実装する。`type Err = ()` とし、不正入力は `Err(())` を返す (`Infallible` は値が構築不能のため不正入力を表現できない)
- 既存の `from_str` メソッドは削除し、呼び出し元を `value.parse()` に変更する。`AccessControl::parse` 内は `value.parse().ok()` で Option に変換し、既存の黙殺挙動 (不正値はデフォルト値を維持) を保つ
- `pbt/tests/prop_stream_id.rs` の 4 箇所は `s.parse().ok()` に変更し、`prop_assert_eq!(parsed, Some(...))` と `prop_assert!(parsed.is_none())` の断言を維持する。`str::parse` は inherent メソッドのため `use std::str::FromStr;` の追加は不要 (追加すると unused import で clippy が失敗する)。`is_none()` を断言する 2 箇所は型が推論されないため、`s.parse::<StreamType>().ok()` のような turbofish で型を明示する (roundtrip の 2 箇所は `prop_assert_eq!` の型制約で推論されるため turbofish 不要)
- `#[allow(clippy::should_implement_trait)]` を削除する

## テスト

`AccessControl::parse` 経由の既存テストと pbt で挙動を担保する。また、`src/stream_id.rs` の `#[cfg(test)]` モジュールに、不正な `t=` / `m=` 値 (`#!::t=invalid,m=invalid`) でデフォルト値 (`StreamType::Stream` / `StreamMode::Request`) が維持される黙殺挙動を固定する単体テストを追加する (成功・失敗の両方の断言を含め、`FromStr` 実装の `Err(())` を直接検証する。既存テストと pbt は `AccessControl::parse` 経由の黙殺をカバーしていない)。変更後も `cargo test --workspace` (pbt を含む) で全テストが通過すること。

## CHANGES.md

`from_str` メソッドの削除は公開 API の後方互換のない変更のため、shiguredo-changelog 規約に従い `[CHANGE]` エントリ (例: `[CHANGE] StreamType / StreamMode の from_str メソッドを FromStr trait 実装に置き換える`。担当者行を付けて追加すること) を追加する。

## 完了条件

- `FromStr` trait (`type Err = ()`) が実装され、既存の `from_str` メソッドが削除されていること
- `#[allow(clippy::should_implement_trait)]` が削除されていること
- 呼び出し元 (src/stream_id.rs の 2 箇所と pbt/tests/prop_stream_id.rs の 4 箇所) が修正されていること
- 黙殺挙動を固定する単体テストが追加されていること
- `cargo test --workspace` で全テスト (pbt を含む) が通過すること
- `cargo clippy --workspace --all-targets -- -D warnings` と `cargo fmt --all -- --check` が通過すること
- CHANGES.md に `[CHANGE]` エントリが追加されていること
