# ConnectionOptions の Debug がパスフレーズと SEK を漏洩する

- Created: 2026-08-16
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-connection-options-debug-leaks-secrets
- Polished: {YYYY-MM-DD}

## 目的

`src/srt_connection.rs` の `ConnectionOptions` 構造体に `#[derive(Debug, Clone)]` が付与されており、`passphrase` と `crypto_sek` の実値が `{:?}` や `dbg!()` の出力に含まれてしまう。パスフレーズと SEK は機密情報であり、ログやデバッグ出力に漏洩してはならない。

`ConnectionOptions` は公開 API (`src/lib.rs` で re-export) であり、利用者が `dbg!()` や `format!("{:?}", ...)` を呼んだ場合や、将来のデバッグ出力追加時にパスフレーズや SEK のバイト列が漏洩するリスクがある。issue 0049 (CryptoContext の Debug が機密鍵を漏洩する) と同種の問題であり、同じ方針で修正する。

## 現状

```rust
#[derive(Debug, Clone)]
pub struct ConnectionOptions {
    pub socket_id: u32,
    pub initial_seq: Option<u32>,
    pub syn_cookie: Option<u32>,
    pub passphrase: Option<String>,
    pub crypto_salt: Option<[u8; 16]>,
    pub crypto_sek: Option<Vec<u8>>,
    pub key_length: KeyLength,
    pub tsbpd_delay: u16,
    pub srt_version: u32,
    pub stream_id: Option<String>,
}
```

`#[derive(Debug)]` は全フィールドをフォーマットするため、機密である `passphrase` と `crypto_sek` の実値がそのまま出力される。

## 設計方針

`#[derive(Debug)]` を外し、`Debug` を手動実装する。`passphrase` と `crypto_sek` の各フィールドを `[REDACTED]` でマスクし、他のフィールドはそのまま出力する。

`crypto_salt` は SRT 仕様上 KM メッセージで平文送信される公開情報であり、マスクしない (issue 0049 の設計方針と同様)。残りのフィールド (`socket_id`、`initial_seq`、`syn_cookie`、`key_length`、`tsbpd_delay`、`srt_version`、`stream_id`) も機密ではないためそのまま出力する。

`Clone` は引き続き必要であるため、`#[derive(Clone)]` は維持する。

手動 `Debug` 実装の前例として `src/error.rs` の `impl std::fmt::Debug for Error`、および issue 0049 で実装予定の `CryptoContext` の `Debug` が参考になる。

## 完了条件

- `ConnectionOptions` の `Debug` 実装が、`passphrase` と `crypto_sek` を `[REDACTED]` でマスクした出力になっていること
- `format!("{:?}", opts)` の出力にパスフレーズと SEK の実値が含まれず、`[REDACTED]` が 2 回含まれることを検証するテストが `tests/test_srt_connection.rs` に追加されていること
- `format!("{:?}", opts)` の出力に非マスクフィールド (socket_id 等) の値が含まれることを検証するテストが追加されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/srt_connection.rs` の `ConnectionOptions` から `#[derive(Debug)]` を除去し、`impl std::fmt::Debug for ConnectionOptions` を手動実装する。マスク対象の 2 フィールド (`passphrase`、`crypto_sek`) は `[REDACTED]` と表示し、残りのフィールドは通常通り出力する。`#[derive(Clone)]` は維持する。

テストは `tests/test_srt_connection.rs` に追加し、`ConnectionOptions` を構築して `Debug` 出力にパスフレーズと SEK の実値が含まれないことを検証する。
