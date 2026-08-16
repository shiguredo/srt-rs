# CryptoContext の Drop で SEK/KEK がゼロクリアされない

- Priority: Critical
- Created: 2026-08-16
- Branch: feature/bug-fix-crypto-context-drop-not-zeroize-secret-keys

## 目的

`src/crypto.rs` の `CryptoContext` 構造体は `Vec<u8>` で `kek`、`sek_even`、`sek_odd` を保持しているが、`Vec<u8>` のデフォルト `Drop` はメモリ解放のみでゼロクリアを行わない。`decommission_old_key` メソッドでは明示的に `fill(0)` しているが、`CryptoContext` 全体が Drop される際（異常終了時など）に鍵がメモリ上に残留し、再利用されたメモリ領域から鍵が読み取られるリスクがある。

## 現状

```rust
pub struct CryptoContext {
    kek: Vec<u8>,
    sek_even: Vec<u8>,
    sek_odd: Vec<u8>,
    ...
}
```

`CryptoContext` に `Drop` 実装はなく、`Vec<u8>` のデフォルト `Drop` に依存している。

## 設計方針

`CryptoContext` に `Drop` を実装し、`kek`、`sek_even`、`sek_odd` の各フィールドに `fill(0)` を適用する。

## 完了条件

- `CryptoContext` に `Drop` 実装が追加され、`kek`、`sek_even`、`sek_odd` がゼロクリアされること
- `cargo test` で全テストが通過すること
