# ConnectionState::Conclusion が定義されているだけで使用されない

- Created: 2026-08-16
- Branch: feature/change-remove-connection-state-conclusion
- Polished: 2026-08-16

## 目的

`src/srt_connection.rs` の `ConnectionState::Conclusion` バリアントが定義されているが、`set_state(ConnectionState::Conclusion)` が呼ばれる箇所が存在しない (コードベース全体の grep で使用箇所は定義と C API の `From` 実装のみ)。ハンドシェイクの状態遷移は `handshake_state` で管理されており、`ConnectionState::Conclusion` は C API の `SrtConnectionState` とのマッピングのためだけに存在している。open issue 0040 は「削除は C ABI の変更を伴うため別途判断が必要」として対象外にしており、本 issue がその判断を担う。

## 現状

```rust
pub enum ConnectionState {
    Disconnected,
    Induction,
    Conclusion,
    Listening,
    Connected,
    Closing,
}
```

(実コードは `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]` と `#[default]` 属性を持つ。`Conclusion` は一度も `set_state` されない)

`SrtConnectionState` (C API) 側で `Conclusion = 2` とのマッピングが定義されている (`crates/c-api/src/lib.rs` の `SrtConnectionState` と `From` 実装)。cbindgen 生成の `crates/c-api/include/srt.h` にも `SRT_CONNECTION_STATE_CONCLUSION = 2` が公開されている。

## 設計方針

`ConnectionState::Conclusion` を削除する (公開 Rust API の variant 削除であり、後方互換のない変更)。

C API 側の `SrtConnectionState::Conclusion = 2` は**明示値を維持したまま残す**。`Conclusion` を削除して後続 variant の値を自動採番に任せると `Listening` 等の値がずれ、canary リリース (`2026.1.0-canary.1`) とコミット済みの `srt.h` に対して ABI 破壊になるためである。`From<ConnectionState>` 実装の `ConnectionState::Conclusion => Self::Conclusion` arm のみを削除する。

## 完了条件

- `ConnectionState::Conclusion` が削除されていること
- `crates/c-api/src/lib.rs` の `SrtConnectionState::Conclusion = 2` の明示値が維持されていること (後続 variant の値が変わらないこと)
- `From<ConnectionState>` 実装の `ConnectionState::Conclusion` arm が削除されていること
- `CHANGES.md` の `## develop` セクションの `misc` に `[CHANGE]` エントリが追加されていること (公開 API の variant 削除のため)
- `cargo test --workspace` で全テストが通過すること
- `cargo clippy --workspace --all-targets -- -D warnings` が通過すること

## 解決方法

`src/srt_connection.rs` の `ConnectionState` 列挙型から `Conclusion` バリアントを削除する。`crates/c-api/src/lib.rs` の `SrtConnectionState` は `Conclusion = 2` の明示値を維持したまま残し、`From<ConnectionState>` 実装の該当 arm のみを削除する。`include/srt.h` は cbindgen の生成物 (`crates/c-api/build.rs`) のため、再生成で対応する (手動編集しない)。

なお、0062 (lib.rs の re-export 削除) も `crates/c-api/src/lib.rs` の import を変更するため、並行実装時は競合に注意する (先後は問わない)。
