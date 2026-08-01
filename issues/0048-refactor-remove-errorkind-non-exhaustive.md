# ErrorKind の #[non_exhaustive] が shiguredo-rust 規約に違反している

- Priority: Low
- Created: 2026-08-01
- Branch: feature/refactor-remove-errorkind-non-exhaustive

## 目的

`src/error.rs` の `ErrorKind` に付与された `#[non_exhaustive]` を除去する。shiguredo-rust 規約は「`#[non_exhaustive]` を使わないこと。どうしても必要な場合は許可を得ること」と定めており、本実装は規約に違反している。

## 現状

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    ...
}
```

## 設計方針

`#[non_exhaustive]` を除去する。将来 variant を追加するときは、規約どおり素直に破壊的変更として扱う。

## 完了条件

- `ErrorKind` から `#[non_exhaustive]` が除去されていること
- `cargo test` で全テストが通過すること
