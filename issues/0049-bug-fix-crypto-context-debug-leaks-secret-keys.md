# CryptoContext の Debug が機密鍵を漏洩する

- Created: 2026-08-16
- Branch: feature/fix-crypto-context-debug-leaks-secret-keys
- Polished: 2026-08-16

## 目的

`src/crypto.rs` の `CryptoContext` 構造体に `#[derive(Debug)]` が付与されており、`sek_even`、`sek_odd`、`kek` の生バイト列が `{:?}` や `dbg!()` の出力に含まれてしまう。これらの鍵は機密情報であり、ログやデバッグ出力に漏洩してはならない。

現時点で `CryptoContext` の `Debug` がログやパニックメッセージに直接出力される経路はないが、`CryptoContext` は公開 API (`src/lib.rs` で re-export) であり、利用者が `dbg!()` や `format!("{:?}", ...)` を呼んだ場合や、将来のデバッグ出力追加時に鍵バイト列が漏洩するリスクがある。公開 API としての将来リスクを防ぐために修正する。

## 現状

```rust
#[derive(Debug)]
pub struct CryptoContext {
    kek: Vec<u8>,
    sek_even: Vec<u8>,
    sek_odd: Vec<u8>,
    salt: [u8; 16],
    current_key: KeyFlag,
    key_length: KeyLength,
    encrypted_packet_count: u64,
    km_refresh_state: KmRefreshState,
    next_key: Option<KeyFlag>,
}
```

`#[derive(Debug)]` は全フィールドをフォーマットするため、機密である `kek`、`sek_even`、`sek_odd` のバイト列がそのまま出力される。

## 設計方針

`#[derive(Debug)]` を外し、`Debug` を手動実装する。`kek`、`sek_even`、`sek_odd` の各フィールドを `[REDACTED]` でマスクし、他のフィールド (`salt`、`current_key`、`key_length`、`encrypted_packet_count`、`km_refresh_state`、`next_key`) はそのまま出力する。

`salt` は SRT 仕様上 KM メッセージで平文送信される公開情報である。SRT はデータパケットのペイロードのみを暗号化し、制御パケット (KM メッセージ含む) は暗号化されない (`refs/srt/draft-sharabayko-srt.md` の「SRT encrypts only the payload of SRT data packets」)。`current_key`、`key_length`、`km_refresh_state`、`next_key` は鍵素材を含まないメタデータ列挙型、`encrypted_packet_count` はパケットカウンタであるため、出力しても問題ない。

手動 `Debug` 実装の前例として `src/error.rs` の `impl std::fmt::Debug for Error` が参考になる。

## 完了条件

- `CryptoContext` の `Debug` 実装が、`kek`、`sek_even`、`sek_odd` を `[REDACTED]` でマスクした出力になっていること
- `format!("{:?}", ctx)` の出力に `kek`、`sek_even`、`sek_odd` のバイト列が含まれず、`[REDACTED]` が 3 回含まれることを検証するテストが `tests/test_crypto.rs` に追加されていること
- `format!("{:?}", ctx)` の出力に非マスクフィールド (salt 等) の値が含まれることを検証するテストが追加されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/crypto.rs` の `CryptoContext` から `#[derive(Debug)]` の `Debug` を除去し、`impl std::fmt::Debug for CryptoContext` を手動実装する。マスク対象の 3 フィールド (`kek`、`sek_even`、`sek_odd`) は `[REDACTED]` と表示し、残りのフィールドは通常通り出力する。

テストは `tests/test_crypto.rs` に追加し、`CryptoContext` を構築して `Debug` 出力に鍵バイト列が含まれないことを検証する。
