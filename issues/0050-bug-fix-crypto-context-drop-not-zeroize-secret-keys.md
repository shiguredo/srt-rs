# CryptoContext の Drop で SEK/KEK がゼロクリアされない

- Created: 2026-08-16
- Branch: feature/fix-crypto-context-drop-not-zeroize-secret-keys
- Polished: 2026-08-16

## 目的

`src/crypto.rs` の `CryptoContext` 構造体は `Vec<u8>` で `kek`、`sek_even`、`sek_odd` を保持しているが、`Vec<u8>` のデフォルト `Drop` はメモリ解放のみでゼロクリアを行わない。このため、`CryptoContext` が Drop される際に鍵がヒープメモリ上に残留し、解放後に再利用されたメモリ領域から鍵が読み取られるリスクがある。

なお、`fill(0)` によるゼロクリアは、コンパイラ最適化 (dead store elimination) により解放直前のストアが除去される可能性があり、ゼロクリアを保証できない。既存の `decommission_old_key` メソッドもこの問題を抱えている。

SRT 仕様 (`refs/srt/draft-sharabayko-srt.md`) には鍵のメモリ消去に関する要件はなく、本 issue は仕様準拠ではなくセキュリティベストプラクティスに基づく対応である。

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

`CryptoContext` に `Drop` 実装はなく、`Vec<u8>` のデフォルト `Drop` に依存している。`decommission_old_key` メソッド (`src/crypto.rs` の `decommission_old_key`) は `sek_even.fill(0)` / `sek_odd.fill(0)` でゼロクリアしているが、コンパイラ最適化の影響を受ける。

## 設計方針

`zeroize` クレートを `Cargo.toml` の直接依存に追加し、`CryptoContext` に `Drop` を実装する。`Drop` 内で `kek`、`sek_even`、`sek_odd` の各フィールドに `Zeroize::zeroize()` を適用する。`zeroize` は volatile ストアとコンパイラフェンスを用いて最適化による除去を防止する。`zeroize` は `Cargo.lock` に `aws-lc-rs` の推移的依存として既に存在する (`Cargo.lock` の `zeroize`) ため、直接依存への追加コストは小さい。

`salt` は SRT 仕様上 KM メッセージで平文送信される公開情報であり、ゼロクリア対象外とする (issue 0049 の設計方針と同様)。`current_key`、`key_length`、`km_refresh_state`、`next_key`、`encrypted_packet_count` は鍵素材を含まないため対象外とする。

`decommission_old_key` 内の `fill(0)` も同時に `Zeroize::zeroize()` に置き換える。

なお、`start_pre_announce` と `update_sek` は新しい鍵でフィールドを上書きする際に古い鍵をゼロクリアせず解放する残留リスクがあるが、Drop 実装では防げない問題であり、本 issue のスコープ外とする。

## 完了条件

- `CryptoContext` に `Drop` 実装が追加され、`kek`、`sek_even`、`sek_odd` の 3 フィールドすべてに `Zeroize::zeroize()` が呼ばれていること
- `decommission_old_key` 内の `fill(0)` が `Zeroize::zeroize()` に置き換えられていること
- `decommission_old_key()` 呼び出し後に `sek_even` / `sek_odd` が全バイト 0 になっていることを検証する単体テストが `src/crypto.rs` 内の `#[cfg(test)]` モジュールに追加されていること
- `Cargo.toml` に `zeroize` が用途コメント付きで直接依存として追加されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`Cargo.toml` に `zeroize` を直接依存として追加する。`src/crypto.rs` の `CryptoContext` に `impl Drop for CryptoContext` を実装し、`kek`、`sek_even`、`sek_odd` の各フィールドを `Zeroize::zeroize()` でクリアする。`decommission_old_key` 内の `self.sek_even.fill(0)` / `self.sek_odd.fill(0)` も同様に `Zeroize::zeroize()` に置き換える。

Drop 後のゼロクリアをテストで直接検証することは、Drop 実行後にヒープメモリが解放されるため不可能である。そのため、`Drop` 実装が 3 フィールドすべてに `Zeroize::zeroize()` を呼んでいることをコードレビューで確認する。一方、`decommission_old_key` のゼロクリアは呼び出し後にフィールドが残存するため、`#[cfg(test)]` モジュールの単体テストで検証できる。
