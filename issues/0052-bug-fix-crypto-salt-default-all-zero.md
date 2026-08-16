# crypto_salt 未指定時のデフォルト値が全ゼロ

- Created: 2026-08-16
- Branch: feature/fix-crypto-salt-default-all-zero
- Polished: 2026-08-16

## 目的

`src/srt_connection.rs` の `handle_handshake_caller` メソッド内で、`self.options.crypto_salt.unwrap_or([0u8; 16])` により salt 未設定時に全ゼロが使われる。PBKDF2 の salt が全ゼロになると、同一パスフレーズから同一の KEK が導出され、レインボーテーブル攻撃への耐性が失われる。

SRT 仕様では salt は乱数で生成すべきと規定されている (`refs/srt/draft-sharabayko-srt.md` の「Salt = PRNG(128)」)。また、`examples/srt_caller` では `getrandom` で salt を乱数生成しており、呼び出し側が生成して渡す前提が example と一致している。

## 現状

```rust
let salt = self.options.crypto_salt.unwrap_or([0u8; 16]);
```

`ConnectionOptions::crypto_salt` は `Option<[u8; 16]>` であり、デフォルトは `None`。`crypto_salt` は `passphrase` が設定されている場合のみ使用される (`handle_handshake_caller` 内の `if let Some(ref passphrase) = self.options.passphrase` 分岐内)。Listener 側は `handle_handshake_listener` で KMREQ の salt を使用するため、本修正の対象は Caller 側のみ。

## 設計方針

`crypto_salt` を `Option` から必須フィールドに変更するのではなく、`None` の場合にエラーを返す設計にする。salt は呼び出し側が乱数で生成して渡すべき値であり、暗黙のデフォルト値を設定すべきではない。

ただし、エラーチェックは **暗号化が有効な場合（`passphrase` が `Some`）に限る**。暗号化が無効（`passphrase` が `None`）の場合は `crypto_salt` は使用されないため、`None` でもエラーにしない。エラーは `Error::handshake_rejected` を使用し、reason 文字列は「encryption enabled but no crypto_salt」のような平文で明記する。

なお、`crypto_sek` にも同様の全ゼロデフォルト問題があるが、本 issue は `crypto_salt` に限定し、`crypto_sek` は別 issue で対応する。

また、C API (`crates/c-api` の `SrtConnectionOptions`) は `passphrase` フィールドを持つが `crypto_salt` フィールドを持たず、`crypto_salt` は `None` のまま `SrtConnection` へ渡される。本修正の適用後、C API 経由で passphrase を設定した Caller はハンドシェイクが必ず `Error::handshake_rejected` になる。C API に salt を渡す手段の追加は本 issue の対象外であり、別 issue で対応する。

## 完了条件

- 暗号化が有効（`passphrase` が `Some`）かつ `crypto_salt` が `None` の場合にハンドシェイクが `Error::handshake_rejected` を返すこと
- 暗号化が無効（`passphrase` が `None`）の場合は `crypto_salt` が `None` でもエラーにならないこと
- 暗号化が有効な場合に salt 未設定を検出するテストが `tests/test_srt_connection.rs` に追加されていること
- 既存の暗号化 e2e テスト 4 件 (`test_handshake_with_encryption`、`test_handshake_with_aes256`、`test_data_transfer_with_encryption`、`test_receive_encrypted_data_with_aes256`) に `crypto_salt` が明示設定されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/srt_connection.rs` の `handle_handshake_caller` 内で、`crypto_salt` の `unwrap_or([0u8; 16])` を削除し、`Option` から明示的に取り出す。暗号化有効 (`passphrase` が `Some`) かつ `crypto_salt` が `None` の場合は `Error::handshake_rejected` を返す。

`tests/test_srt_connection.rs` の既存暗号化テスト 4 件に `crypto_salt` の明示設定を追加し、salt 未設定時にエラーを返すテストを新規追加する。
