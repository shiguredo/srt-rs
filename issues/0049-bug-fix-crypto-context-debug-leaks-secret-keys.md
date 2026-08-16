# CryptoContext の Debug が機密鍵を漏洩する

- Priority: Critical
- Created: 2026-08-16
- Branch: feature/bug-fix-crypto-context-debug-leaks-secret-keys

## 目的

`src/crypto.rs` の `CryptoContext` 構造体に `#[derive(Debug)]` が付与されており、`sek_even`、`sek_odd`、`kek` の生バイト列が `{:?}` や `dbg!()` の出力に含まれてしまう。これらの鍵は機密情報であり、ログやデバッグ出力に漏洩してはならない。

## 現状

```rust
#[derive(Debug)]
pub struct CryptoContext {
    kek: Vec<u8>,
    sek_even: Vec<u8>,
    sek_odd: Vec<u8>,
    ...
}
```

`#[derive(Debug)]` は全フィールドをフォーマットするため、鍵のバイト列がそのまま出力される。

## 設計方針

`Debug` を手動実装し、`kek`、`sek_even`、`sek_odd` の各フィールドを `[REDACTED]` でマスクする。他のフィールド (`salt`、`current_key`、`key_length`、`encrypted_packet_count`、`km_refresh_state`、`next_key`) は出力しても問題ない。

## 完了条件

- `CryptoContext` の `Debug` 実装が、`kek`、`sek_even`、`sek_odd` をマスクした出力になっていること
- `cargo test` で全テストが通過すること
