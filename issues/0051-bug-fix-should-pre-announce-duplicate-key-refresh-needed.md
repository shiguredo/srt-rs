# should_pre_announce が KeyRefreshNeeded イベントを重複発行する

- Created: 2026-08-16
- Branch: feature/fix-should-pre-announce-duplicate-key-refresh-needed
- Polished: 2026-08-16

## 目的

`src/srt_connection.rs` の `check_km_refresh` メソッドは `send()` のたびに呼ばれる。`should_pre_announce()` は `km_refresh_state == Idle` かつ `encrypted_packet_count >= threshold` の間 true を返し続ける。`should_pre_announce` は `&self` の参照メソッドで状態を変更しないため、`start_pre_announce` が呼ばれて状態が `PreAnnounce` に遷移するまでは、`send()` のたびに `ConnectionEvent::KeyRefreshNeeded` イベントがキューに積まれる。

なお、`check_km_refresh` 内の `should_switch_key` と `should_decommission_old_key` は、それぞれ `switch_key()` や `decommission_old_key()` が状態を遷移させるため、同様の重複問題は発生しない。`should_pre_announce` だけが `Idle` 状態で状態遷移を伴わないために重複する。

## 現状

`check_km_refresh` メソッド内:

```rust
if crypto.should_pre_announce() {
    self.event_queue.push_back(ConnectionEvent::KeyRefreshNeeded {
        key_length: crypto.key_length().len(),
    });
}
```

`should_pre_announce` は状態を変更しない `&self` の参照メソッドであり、`km_refresh_state` が `Idle` のままである限り、毎回 true を返す。

## 設計方針

`SrtConnection` に `key_refresh_event_sent: bool` フィールドを追加し、`KeyRefreshNeeded` イベントを発行したら true に設定する。`should_pre_announce()` の判定に加えて `!self.key_refresh_event_sent` も条件に含めることで、重複発行を防ぐ。

キュー内の既存イベントを線形探索する方式は、利用者が `poll_event()` でイベントを消費した後、`provide_new_sek()` を呼ぶ前に `send()` が呼ばれると再度発行されてしまうため、`bool` フラグ方式がより確実である。

`key_refresh_event_sent` のリセットは、キーリフレッシュの 1 サイクルが完了し `should_pre_announce()` が次のサイクルで再度 true になる前に、`decommission_old_key()` が `km_refresh_state` を `Idle` に戻すタイミングで行う。

## 完了条件

- `should_pre_announce` が true の間、`send()` を複数回呼んでも `KeyRefreshNeeded` イベントが 1 回だけ発行されること
- `poll_event()` でイベントを消費後、`provide_new_sek()` を呼ぶ前に `send()` が呼ばれても再発行されないこと
- キーリフレッシュの 1 サイクル完了後、次のサイクルで再度 `KeyRefreshNeeded` が発行されること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- 重複発行を防止するテストが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/srt_connection.rs` の `SrtConnection` 構造体に `key_refresh_event_sent: bool` フィールドを追加し、`check_km_refresh` 内の `KeyRefreshNeeded` 発行条件に `!self.key_refresh_event_sent` を追加する。イベント発行時に `self.key_refresh_event_sent = true` を設定する。

`decommission_old_key` の呼び出し後に `self.key_refresh_event_sent = false` を設定し、次のキーリフレッシュサイクルに備える。

テストは `tests/test_srt_connection.rs` に追加し、`send()` を複数回呼んだ場合のイベント重複防止と、イベント消費後再発行の防止を検証する。
