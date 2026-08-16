# crypto_sek 未指定時のデフォルト値が全ゼロ

- Created: 2026-08-16
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/fix-crypto-sek-default-all-zero
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

`src/srt_connection.rs` の `handle_handshake_caller` メソッド内で、`self.options.crypto_sek.as_deref().unwrap_or(&default_sek)` により、暗号化有効 (`passphrase` が `Some`) かつ `crypto_sek` が `None` の場合に全ゼロ SEK が使われる。SEK (Stream Encrypting Key) は AES-CTR でペイロードを暗号化する鍵そのものであり、全ゼロだと暗号化が実質無効化される。同一パスフレーズの複数セッション間で鍵ストリームが同一になり、暗号文から平文を復元できる可能性がある。

SRT 仕様では SEK は PRNG で生成すべきと規定されている (`refs/srt/draft-sharabayko-srt.md` の「SEK = PRNG(KLen)」)。また、`examples/srt_caller` では `getrandom` で SEK を乱数生成しており、呼び出し側が生成して渡す前提が example と一致している。

## 現状

```rust
let default_sek = vec![0u8; key_length.len()];
let sek = self.options.crypto_sek.as_deref().unwrap_or(&default_sek);
```

`ConnectionOptions::crypto_sek` は `Option<Vec<u8>>` であり、デフォルトは `None`。`crypto_sek` は `passphrase` が設定されている場合のみ使用される (`handle_handshake_caller` 内の `if let Some(ref passphrase) = self.options.passphrase` 分岐内)。`key_length` はピア提示値 (`hs.key_length().unwrap_or(self.options.key_length)`) に依存するため、`default_sek` の長さもピア提示値に依存する。Listener 側は KMREQ の wrapped_key をアンラップして SEK を取得する (`src/srt_connection.rs` の `handle_handshake_listener` → `CryptoContext::update_sek`) ため、本修正の対象は Caller 側のみ。

## 設計方針

`crypto_sek` を `Option` から必須フィールドに変更するのではなく、`None` の場合にエラーを返す設計にする。SEK は呼び出し側が乱数で生成して渡すべき値であり、暗黙のデフォルト値を設定すべきではない。

ただし、エラーチェックは **暗号化が有効な場合 (`passphrase` が `Some`) に限る**。暗号化が無効 (`passphrase` が `None`) の場合は `crypto_sek` は使用されないため、`None` でもエラーにしない。エラーは `Error::handshake_rejected` を使用し、reason 文字列は「encryption enabled but no crypto_sek」のような平文で明記する。

なお、C API (`crates/c-api` の `SrtConnectionOptions`) は `passphrase` フィールドを持つが `crypto_sek` フィールドを持たず、`crypto_sek` は `None` のまま `SrtConnection` へ渡される。本修正の適用後、C API 経由で passphrase を設定した Caller はハンドシェイクが必ず `Error::handshake_rejected` になるため、C API に SEK を渡す手段の追加が必要である (C API の `crypto_salt` への渡す手段の追加は issue 0071 で対応予定)。

## 完了条件

- 暗号化が有効 (`passphrase` が `Some`) かつ `crypto_sek` が `None` の場合にハンドシェイクが `Error::handshake_rejected` を返すこと
- 暗号化が無効 (`passphrase` が `None`) の場合は `crypto_sek` が `None` でもエラーにならないこと
- 暗号化が有効な場合に SEK 未設定を検出するテストが `tests/test_srt_connection.rs` に追加されていること
- 既存の暗号化 e2e テスト 4 件 (`test_handshake_with_encryption`、`test_handshake_with_aes256`、`test_data_transfer_with_encryption`、`test_receive_encrypted_data_with_aes256`) に `crypto_sek` が明示設定されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/srt_connection.rs` の `handle_handshake_caller` 内で、`crypto_sek` の `unwrap_or(&default_sek)` を削除し、`Option` から明示的に取り出す。暗号化有効 (`passphrase` が `Some`) かつ `crypto_sek` が `None` の場合は `Error::handshake_rejected` を返す。

`tests/test_srt_connection.rs` の既存暗号化テスト 4 件に `crypto_sek` の明示設定を追加し、SEK 未設定時にエラーを返すテストを新規追加する。
