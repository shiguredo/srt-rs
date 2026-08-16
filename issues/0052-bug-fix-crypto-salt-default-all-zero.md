# crypto_salt 未指定時のデフォルト値が全ゼロ

- Priority: High
- Created: 2026-08-16
- Branch: feature/bug-fix-crypto-salt-default-all-zero

## 目的

`src/srt_connection.rs` の `handle_handshake_caller` メソッド内で、`self.options.crypto_salt.unwrap_or([0u8; 16])` により salt 未設定時に全ゼロが使われる。PBKDF2 の salt が全ゼロになると、同一パスフレーズから同一の KEK が導出され、レインボーテーブル攻撃への耐性が失われる。

## 現状

```rust
let salt = self.options.crypto_salt.unwrap_or([0u8; 16]);
```

`ConnectionOptions::crypto_salt` は `Option<[u8; 16]>` であり、デフォルトは `None`。

## 設計方針

`crypto_salt` を `Option` から必須フィールドに変更するのではなく、`None` の場合にエラーを返す設計にする。salt は呼び出し側が乱数で生成して渡すべき値であり、暗黙のデフォルト値を設定すべきではない。`handle_handshake_caller` 内で `crypto_salt` が `None` の場合は `Error` を返す。

## 完了条件

- `crypto_salt` が `None` の場合にハンドシェイクがエラーを返すこと
- 暗号化が有効な場合に salt 未設定を検出するテストが追加されていること
- `cargo test` で全テストが通過すること
