# ConnectionState::Conclusion が write-only で使われていない

- Priority: High
- Created: 2026-08-16
- Branch: feature/refactor-remove-connection-state-conclusion

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
- `cargo test` で全テストが通過すること
