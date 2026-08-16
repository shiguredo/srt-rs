# should_pre_announce が KeyRefreshNeeded イベントを重複発行する

- Priority: High
- Created: 2026-08-16
- Branch: feature/bug-fix-should-pre-announce-duplicate-key-refresh-needed

## 目的

`src/srt_connection.rs` の `check_km_refresh` メソッドは `send()` のたびに呼ばれる。`should_pre_announce()` は `km_refresh_state == Idle` かつ `encrypted_packet_count >= threshold` の間 true を返し続けるため、`start_pre_announce` が呼ばれて状態が `PreAnnounce` に遷移するまでは `send()` のたびに `ConnectionEvent::KeyRefreshNeeded` イベントがキューに積まれる。

## 現状

`check_km_refresh` メソッド内:

```rust
if crypto.should_pre_announce() {
    self.event_queue.push_back(ConnectionEvent::KeyRefreshNeeded {
        key_length: crypto.key_length().len(),
    });
}
```

`should_pre_announce` は状態を変更しない参照メソッドであり、`km_refresh_state` が `Idle` のままである限り、毎回 true を返す。

## 設計方針

`check_km_refresh` 内で `KeyRefreshNeeded` イベントを発行した後、`CryptoContext` の状態を `PreAnnounce` に進めるか、イベント発行済みフラグを別途管理して重複発行を防ぐ。`start_pre_announce` は外部から SEK を受け取る設計のため、状態遷移は `provide_new_sek` 経由で行う必要がある。そのため、`check_km_refresh` で状態を直接進めるのではなく、`KeyRefreshNeeded` が既にキューにあるかどうかを判定する方式が適切。

## 完了条件

- `should_pre_announce` が true の間、`send()` を複数回呼んでも `KeyRefreshNeeded` イベントが 1 回だけ発行されること
- `cargo test` で全テストが通過すること
- 重複発行を防止するテストが追加されていること
