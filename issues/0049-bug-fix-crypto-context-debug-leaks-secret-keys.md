# CryptoContext の Debug が機密鍵を漏洩する

- Created: 2026-08-16
- Branch: feature/fix-crypto-context-debug-leaks-secret-keys
- Polished: 2026-08-16

## 目的

`src/crypto.rs` の `CryptoContext` 構造体に `#[derive(Debug)]` が付与されている。`#[derive(Debug)]` は全フィールドをフォーマットするため、`sek_even`、`sek_odd`、`kek` の生バイト列が `{:?}` や `dbg!()` の出力に含まれてしまう。これらの鍵は機密情報であり、ログやデバッグ出力に漏洩してはならない。

具体的な漏洩経路として、以下のようなケースで鍵が露出する:

- `tracing` の span フィールドや event に `CryptoContext` が含まれた場合、トレース出力に鍵バイト列が記録される
- `unwrap()` / `expect()` のパニックメッセージに `Debug` 経由で鍵バイト列が含まれる
- テストやデバッグ中の `dbg!()` や `println!("{:?}", ctx)` で鍵が標準出力に表示される

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

## 設計方針

`#[derive(Debug)]` を外し、`Debug` を手動実装する。`kek`、`sek_even`、`sek_odd` の各フィールドを `[REDACTED]` でマスクし、他のフィールド (`salt`、`current_key`、`key_length`、`encrypted_packet_count`、`km_refresh_state`、`next_key`) はそのまま出力する。

`salt` は SRT プロトコル上 KM メッセージで平文送信される非機密情報であり、`current_key`、`key_length`、`km_refresh_state`、`next_key` はメタデータ列挙型、`encrypted_packet_count` はカウンタ値であるため、出力しても問題ない。

`src/error.rs` の `Error` 型に手動 `Debug` 実装の前例があるため、参考にできる。

## 完了条件

- `CryptoContext` の `Debug` 実装が、`kek`、`sek_even`、`sek_odd` をマスクした出力になっていること
- `format!("{:?}", ctx)` の結果に `kek`、`sek_even`、`sek_odd` のバイト列が一切含まれず、`[REDACTED]` が含まれていることを検証するテストが追加されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/crypto.rs` の `CryptoContext` から `#[derive(Debug)]` の `Debug` を除去し、`impl std::fmt::Debug for CryptoContext` を手動実装する。マスク対象の 3 フィールド (`kek`、`sek_even`、`sek_odd`) は `[REDACTED]` と表示し、残りのフィールドは通常通り出力する。

テストは `tests/test_crypto.rs` に追加し、`CryptoContext` を構築して `Debug` 出力を検証する。
