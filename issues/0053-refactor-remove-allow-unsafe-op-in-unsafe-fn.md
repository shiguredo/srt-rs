# crates/c-api の #![allow(unsafe_op_in_unsafe_fn)] が shiguredo-rust 規約に違反する

- Created: 2026-08-16
- Branch: feature/refactor-remove-allow-unsafe-op-in-unsafe-fn
- Polished: 2026-08-16

## 目的

`crates/c-api/src/lib.rs` のモジュールレベルで `#![allow(unsafe_op_in_unsafe_fn)]` が使用されている。shiguredo-rust 規約は「`#[allow(...)]` を使わないこと（例外なし）。必ず `#[expect(...)]` を使うこと」と定めている。`#[allow(...)]` は lint 項目がなくなったりコードの修正で不要になったときに気づけないため、`#[expect(...)]` に置き換える必要がある。

CHANGES.md には既に `[UPDATE] #[allow] を #[expect] に置き換え、不要になった抑制を削除する` が記録されており、本 issue はその取りこぼし対応である。

なお、当該 crate は `edition = "2024"` であり、`unsafe_op_in_unsafe_fn` は edition 2024 では既に `deny` by default となっている。`#![allow(unsafe_op_in_unsafe_fn)]` がこれを抑制している状態であり、`#[expect]` に置き換えることで、将来的に lint 条件が変更された場合に未充足の期待として検出できる。

## 現状

```rust
#![allow(unsafe_op_in_unsafe_fn)]
```

## 設計方針

`#![allow(unsafe_op_in_unsafe_fn)]` を `#![expect(unsafe_op_in_unsafe_fn)]` に変更する。`expect` にすることで、lint が発火しなくなった場合（例: すべての unsafe 関数内で unsafe ブロックが明示され、lint の抑制自体が不要になった場合）に未充足の警告が出力されるようになる。

## 完了条件

- `#![allow(unsafe_op_in_unsafe_fn)]` が `#![expect(unsafe_op_in_unsafe_fn)]` に置き換えられていること
- `#![expect(unsafe_op_in_unsafe_fn)]` が fulfilled（充足）状態であり、未充足の警告が出力されないこと
- `cargo clippy --workspace --all-targets -- -D warnings` が通過すること
- `cargo test --workspace` が通過すること

## 解決方法

`crates/c-api/src/lib.rs` の `#![allow(unsafe_op_in_unsafe_fn)]` を `#![expect(unsafe_op_in_unsafe_fn)]` に 1 行置換する。CHANGES.md への新規エントリ追加は不要（既存の `[UPDATE] #[allow] を #[expect] に置き換え` が本修正を含む）。
