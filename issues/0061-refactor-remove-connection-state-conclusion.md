# ConnectionState::Conclusion が write-only で使われていない

- Created: 2026-08-16
- Branch: feature/refactor-remove-connection-state-conclusion
- Polished: 2026-08-16

## 目的

`src/srt_connection.rs` の `ConnectionState::Conclusion` バリアントが定義されているが、`set_state(ConnectionState::Conclusion)` が呼ばれる箇所が存在しない。ハンドシェイクの状態遷移は `handshake_state` で管理されており、`ConnectionState::Conclusion` は C API の `SrtConnectionState` とのマッピングのためだけに存在している。

## 現状

```rust
pub enum ConnectionState {
    Disconnected,
    Induction,
    Conclusion,  // 一度も set_state されない
    Listening,
    Connected,
    Closing,
}
```

`SrtConnectionState` (C API) 側で `Conclusion` とのマッピングが定義されているが、実際に `ConnectionState::Conclusion` が使われることはない。

## 設計方針

`ConnectionState::Conclusion` を削除し、C API 側の `SrtConnectionState` も `Conclusion` を削除する。もし外部互換性のために C API 側で `Conclusion` を残す必要がある場合は、`ConnectionState` に依存せずに C API 側だけでマッピングする。

## 完了条件

- `ConnectionState::Conclusion` が削除されていること
- C API 側の `SrtConnectionState::Conclusion` も削除または適切に処理されていること
- `CHANGES.md` の `## develop` セクションの `misc` に `[UPDATE]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/srt_connection.rs` の `ConnectionState` 列挙型から `Conclusion` バリアントを削除する。`crates/c-api/src/lib.rs` の `SrtConnectionState` 列挙型からも `Conclusion` を削除し、関連するマッピングコードを除去する。
