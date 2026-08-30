# check_km_refresh が KeyRefreshNeeded イベントを重複発行する

- Created: 2026-08-16
- Completed: 2026-08-30
- Branch: feature/fix-should-pre-announce-duplicate-key-refresh-needed
- Polished: 2026-08-16

## 目的

`src/srt_connection.rs` の `check_km_refresh` メソッドは `send()` のたびに呼ばれる。`should_pre_announce()` は `km_refresh_state == Idle` かつ `encrypted_packet_count >= threshold` の間 true を返し続け、`should_pre_announce` は状態を変更しない `&self` 参照メソッドのため、`start_pre_announce` が呼ばれて状態が `PreAnnounce` に遷移するまでは `send()` のたびに `ConnectionEvent::KeyRefreshNeeded` イベントがキューに積まれる。

重複イベントを消費したアプリケーションがイベントごとに `provide_new_sek` を呼ぶと、KMREQ が複数回送信されて異なる SEK で鍵が上書きされる可能性がある。

なお、`check_km_refresh` 内の `should_switch_key` と `should_decommission_old_key` は、それぞれ `switch_key()` / `decommission_old_key()` が状態を遷移させるため、同様の重複問題は発生しない。`should_pre_announce` だけが `Idle` 状態で状態遷移を伴わないために重複する。

## 現状

`check_km_refresh` メソッド内:

```rust
if crypto.should_pre_announce() {
    self.event_queue.push_back(ConnectionEvent::KeyRefreshNeeded {
        key_length: crypto.key_length().len(),
    });
}
```

## 設計方針

`SrtConnection` に `key_refresh_event_sent: bool` フィールド（初期値 `false`）を追加し、`KeyRefreshNeeded` イベントを発行したら true に設定する。`check_km_refresh` 内で `crypto.should_pre_announce()` の判定に加えて `!self.key_refresh_event_sent` も条件に含めることで、重複発行を防ぐ。

キュー内の既存イベントを走査して判定する方式は、利用者が `poll_event()` でイベントを消費した後、`provide_new_sek()` を呼ぶ前に `send()` が呼ばれると再発行されてしまい、「1 回だけ発行」という完了条件を満たせない。`bool` フラグ方式はイベント消費の影響を受けず確実である。

`key_refresh_event_sent` のリセットは、キーリフレッシュの 1 サイクルが完了し `decommission_old_key()` が `km_refresh_state` を `Idle` に戻したタイミングで行う。これにより次のサイクルで再度 `KeyRefreshNeeded` を発行できる。

## 完了条件

- `should_pre_announce` が true の間、`send()` を複数回呼んでも `KeyRefreshNeeded` イベントが 1 回だけ発行されること
- `poll_event()` でイベントを消費後、`provide_new_sek()` を呼ぶ前に `send()` が呼ばれても再発行されないこと
- キーリフレッシュの 1 サイクル完了後、次のサイクルで再度 `KeyRefreshNeeded` が発行されること
- 重複発行を防止するテストが `src/srt_connection.rs` 内の `#[cfg(test)]` モジュールに追加されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

- `src/srt_connection.rs` の `SrtConnection` に `key_refresh_event_sent: bool` フィールド (初期値 `false`) を追加し、`check_km_refresh` 内の `KeyRefreshNeeded` 発行条件に `!self.key_refresh_event_sent` を追加した。イベント発行時に `self.key_refresh_event_sent = true` を設定し、`decommission_old_key()` の呼び出し後に `self.key_refresh_event_sent = false` でリセットする
- フラグは `poll_event()` でのイベント消費状況には依存させない。イベント消費後に `provide_new_sek()` を呼ぶ前に `send()` されても再発行されない
- テストは `src/srt_connection.rs` 内の `#[cfg(test)]` モジュールに追加した。`encrypted_packet_count` は `CryptoContext` の private フィールドであり、Rust の privacy 上 `srt_connection.rs` のテストモジュールからは直接設定できないため、`src/crypto.rs` に `#[cfg(test)]` 専用の `set_encrypted_packet_count_for_test()` を追加して閾値付近の状態を作れるようにした (通常のビルドには含まれない)。テストは接続済み状態の `SrtConnection` と `CryptoContext` を実オブジェクトのまま直接構築し、検証シナリオは次の 3 つ
  - `test_key_refresh_needed_emitted_once`: `send()` を複数回呼んでも `KeyRefreshNeeded` が 1 回しか発行されないこと
  - `test_key_refresh_needed_not_reemitted_after_poll`: `poll_event()` でイベントを消費後、`provide_new_sek()` を呼ぶ前に `send()` を呼んでも再発行されないこと
  - `test_key_refresh_needed_reemitted_next_cycle`: 1 サイクル完了 (`decommission_old_key`) 後に再度 `KeyRefreshNeeded` が発行されること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加した
