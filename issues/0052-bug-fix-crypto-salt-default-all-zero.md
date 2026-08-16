# crypto_salt 未指定時のデフォルト値が全ゼロ

- Created: 2026-08-16
- Branch: feature/fix-crypto-salt-default-all-zero
- Polished: 2026-08-16

## 目的

`src/srt_connection.rs` の `handle_handshake_caller` メソッド内で、`self.options.crypto_salt.unwrap_or([0u8; 16])` により salt 未設定時に全ゼロが使われる。SRT 仕様では salt は PRNG で生成すべきと規定されており（`refs/srt/draft-sharabayko-srt.md` の `Salt = PRNG(128)`）、PBKDF2 の salt が全ゼロになると同一パスフレーズから同一の KEK が導出され、レインボーテーブル攻撃への耐性が失われる。

なお、`src/crypto.rs` の `CryptoContext::new_sender` の doc comment でも「salt と sek は外部から乱数で生成して渡す」と明記されており、暗黙のデフォルト値は設計意図に反する。

## 現状

`handle_handshake_caller` メソッド内:

```rust
if let Some(ref passphrase) = self.options.passphrase {
    let salt = self.options.crypto_salt.unwrap_or([0u8; 16]);
    let default_sek = vec![0u8; key_length.len()];
    let sek = self.options.crypto_sek.as_deref().unwrap_or(&default_sek);
    self.crypto = Some(CryptoContext::new_sender(passphrase, key_length, salt, sek)?);
}
```

`crypto_salt` は `passphrase` が設定されている場合のみ使用される。`ConnectionOptions::crypto_salt` は `Option<[u8; 16]>` であり、デフォルトは `None`。

## 設計方針

`crypto_salt` を `Option` から必須フィールドに変更するのではなく、`None` の場合にエラーを返す設計にする。salt は呼び出し側が乱数で生成して渡すべき値であり、暗黙のデフォルト値を設定すべきではない。暗号化が有効（`passphrase` が `Some`）かつ `crypto_salt` が `None` の場合、`handle_handshake_caller` は `Error::handshake_rejected` を返す。

## 完了条件

- 暗号化が有効（`passphrase` が `Some`）かつ `crypto_salt` が `None` の場合にハンドシェイクがエラーを返すこと
- 暗号化が無効（`passphrase` が `None`）の場合は `crypto_salt` が `None` でもエラーにならないこと
- salt 未設定を検出するテストが追加されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/srt_connection.rs` の `handle_handshake_caller` 内で、`crypto_salt` の `unwrap_or([0u8; 16])` を削除し、`Option` から明示的に取り出す。`None` の場合は `Error::handshake_rejected("crypto_salt is required when encryption is enabled")` を返す。

テストは `tests/test_srt_connection.rs` に追加し、salt 未設定時にエラーが返ることを検証する。

なお、`crypto_sek` も同様に `None` 時に全ゼロがデフォルト値として使われる問題があるが、本 issue では `crypto_salt` に限定し、`crypto_sek` の全ゼロ問題は別 issue で対応する。
