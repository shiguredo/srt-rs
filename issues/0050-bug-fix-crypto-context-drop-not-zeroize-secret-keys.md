# CryptoContext の Drop で SEK/KEK がゼロクリアされない

- Created: 2026-08-16
- Branch: feature/fix-crypto-context-drop-not-zeroize-secret-keys
- Polished: 2026-08-16

## 目的

`src/crypto.rs` の `CryptoContext` 構造体は `Vec<u8>` で `kek`、`sek_even`、`sek_odd` を保持しているが、`Vec<u8>` のデフォルト `Drop` はメモリ解放のみでゼロクリアを行わない。`decommission_old_key` メソッドでも `fill(0)` でゼロクリアしているが、`fill(0)` はコンパイラ最適化（dead store elimination）で除去される可能性がある。このため、`CryptoContext` 全体が Drop される際（異常終了時・通常終了時ともに）に鍵がメモリ上に残留し、再利用されたメモリ領域から鍵が読み取られるリスクがある。

## 現状

```rust
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

`CryptoContext` に `Drop` 実装はなく、`Vec<u8>` のデフォルト `Drop` に依存している。`decommission_old_key` も `fill(0)` を使用しており、コンパイラ最適化の影響を受ける。

## 設計方針

`zeroize` クレート（`Cargo.lock` に推移的依存として既に存在）を `Cargo.toml` の直接依存に追加し、`CryptoContext` に `Drop` を実装する。`Drop` 内で `kek`、`sek_even`、`sek_odd` の各フィールドに `Zeroize::zeroize()` を適用する。`zeroize` は `core::ptr::write_volatile` とコンパイラフェンスを用いて最適化による除去を防止する。

`decommission_old_key` 内の `fill(0)` も同時に `Zeroize::zeroize()` に置き換える。

## 完了条件

- `CryptoContext` に `Drop` 実装が追加され、`kek`、`sek_even`、`sek_odd` が `Zeroize::zeroize()` でゼロクリアされること
- `decommission_old_key` 内の `fill(0)` が `Zeroize::zeroize()` に置き換えられていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`Cargo.toml` に `zeroize` を依存として追加する。`src/crypto.rs` の `CryptoContext` に `impl Drop for CryptoContext` を実装し、`kek`、`sek_even`、`sek_odd` の各フィールドを `Zeroize::zeroize()` でクリアする。`decommission_old_key` 内の `self.sek_even.fill(0)` / `self.sek_odd.fill(0)` も同様に `Zeroize::zeroize()` に置き換える。

Drop 後のゼロクリアを直接検証するテストは実装が困難なため、`zeroize` クレートの `Zeroize` トレイト実装のコンパイル確認と、既存テストの通過をもって検証とする。
