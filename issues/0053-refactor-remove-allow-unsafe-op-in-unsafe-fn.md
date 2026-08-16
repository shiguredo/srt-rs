# crates/c-api の #![allow(unsafe_op_in_unsafe_fn)] が shiguredo-rust 規約に違反する

- Priority: High
- Created: 2026-08-16
- Branch: feature/refactor-remove-allow-unsafe-op-in-unsafe-fn

## 目的

`crates/c-api/src/lib.rs` のモジュールレベルで `#![allow(unsafe_op_in_unsafe_fn)]` が使用されている。shiguredo-rust 規約は「`#[allow(...)]` を使わないこと（例外なし）。必ず `#[expect(...)]` を使うこと」と定めている。

## 現状

```rust
#![allow(unsafe_op_in_unsafe_fn)]
```

## 設計方針

`#![allow(unsafe_op_in_unsafe_fn)]` を `#![expect(unsafe_op_in_unsafe_fn)]` に変更する。`expect` にすることで、すべての unsafe 関数内で unsafe 操作が近い将来必須化された際に、明示的に `unsafe` ブロックを追加する必要がある箇所が一目で分かるようになる。

## 完了条件

- `#![allow(unsafe_op_in_unsafe_fn)]` が `#![expect(unsafe_op_in_unsafe_fn)]` に置き換えられていること
- `cargo clippy --workspace --all-targets -- -D warnings` が通過すること
