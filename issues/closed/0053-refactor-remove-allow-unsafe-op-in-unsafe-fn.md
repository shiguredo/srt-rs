# crates/c-api の #![allow(unsafe_op_in_unsafe_fn)] が shiguredo-rust 規約に違反する

- Created: 2026-08-16
- Completed: 2026-08-29
- Branch: feature/refactor-remove-allow-unsafe-op-in-unsafe-fn
- Polished: 2026-08-16

## 目的

`crates/c-api/src/lib.rs` のモジュールレベルで `#![allow(unsafe_op_in_unsafe_fn)]` が使用されている。shiguredo-rust 規約は「`#[allow(...)]` を使わないこと（例外なし）。必ず `#[expect(...)]` を使うこと」と定めている。`#[allow(...)]` では、その lint 項目がなくなったりコードの修正によって不要になったときに気づけないためである。

本属性はリポジトリ内で唯一の残存 `#[allow]` であり、本体 (`src/`) は `CHANGES.md` の `[UPDATE] #[allow] を #[expect] に置き換え、不要になった抑制を削除する` で対応済みの取りこぼしである。

なお、当該 crate は `edition = "2024"` であり、`unsafe_op_in_unsafe_fn` は edition 2024 でデフォルト警告に昇格している。`#![allow(...)]` がこれを抑制している状態である。

## 現状

```rust
#![allow(unsafe_op_in_unsafe_fn)]
```

`crates/c-api/src/lib.rs` には 13 個の unsafe 関数があり、その内部で裸の unsafe 操作が計 87 箇所ある。`#![allow(unsafe_op_in_unsafe_fn)]` が lint 警告を抑制している。

## 設計方針

`#![allow(unsafe_op_in_unsafe_fn)]` を `#![expect(unsafe_op_in_unsafe_fn)]` に変更する。`expect` は lint が発火している間は `allow` と同様に警告を抑止し、lint が発火しなくなったとき（全 unsafe 操作が `unsafe` ブロックで明示された場合など）に未充足の期待 (`unfulfilled_lint_expectations`) として検出する。これにより、抑制自体が不要になったタイミングを `cargo clippy --workspace --all-targets -- -D warnings` で気づけるようになる。

## 完了条件

- `#![allow(unsafe_op_in_unsafe_fn)]` が `#![expect(unsafe_op_in_unsafe_fn)]` に置き換えられていること
- `#![expect(unsafe_op_in_unsafe_fn)]` が充足状態であり、未充足の期待警告が出力されないこと
- `cargo clippy --workspace --all-targets -- -D warnings` が通過すること
- `cargo test --workspace` が通過すること

## 解決方法

- `crates/c-api/src/lib.rs` の `#![allow(unsafe_op_in_unsafe_fn)]` を `#![expect(unsafe_op_in_unsafe_fn)]` に置き換えた。あわせて属性の上に 3 行の `//` コメントを追加し、各操作を個別の unsafe ブロックで囲まない方針 (unsafe fn の境界で `# Safety` 契約を表す) と、crate レベル抑止のため将来追加される unsafe 関数の警告も抑止されるという代価を明記した
- `expect` の充足は実測で確認した。属性を除くと `unsafe_op_in_unsafe_fn` は 87 箇所 (生ポインタ参照 79、`CStr::from_ptr` 2、`slice::from_raw_parts` 3、`Box::from_raw` 3) で発火するため期待は充足されており、逆に全操作を `unsafe` ブロックで囲んだ状態では `unfulfilled_lint_expectations` が発火して `-D warnings` でビルドが落ちることを確認した
- CHANGES.md への新規エントリ追加は行わない。既存の `[UPDATE] #[allow] を #[expect] に置き換え、不要になった抑制を削除する` (未リリースの `## develop` 配下) が本修正を含む
- 本 crate は `crate-type = ["cdylib", "staticlib"]` のみで統合テストを持たないため、検証根拠は `cargo clippy --workspace --all-targets -- -D warnings` と `cargo test --workspace` の通過に置く
