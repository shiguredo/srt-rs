# 再送パスで KM リフレッシュのチェックが行われない

- Priority: Medium
- Created: 2026-08-30
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/fix-check-km-refresh-on-retransmit
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

新規送信が止まり再送だけが続く期間にも、KM リフレッシュの判定と鍵の切り替えが進行するようにする。再送でも `encrypted_packet_count` は増えるが、チェックは `send()` 経由でしか行われないため、閾値を超えてもイベント発行・鍵切り替えが遅延する。

## 現状

- `src/srt_connection.rs` の `process_retransmit` は `crypto.encrypt` を呼んで `encrypted_packet_count` を増やすが、`check_km_refresh` を呼ばない
- `check_km_refresh` を呼んでいるのは `send()` のみである
- そのため、ロス率高い環境でアプリが新規 `send()` を止めて再送だけが走る間に暗号化カウントが閾値を超えても、`KeyRefreshNeeded` イベントの発行も `switch_key` も次に `send()` が再開するまで行われない

## 設計方針

`process_retransmit` の末尾 (または `handle_timer` の `TimerId::Retransmit` 経路) でも `check_km_refresh` を呼ぶ。`check_km_refresh` はイベント発行済みフラグと `KmRefreshState` の遷移により冪等であり、`send()` との重複呼び出しで重複発行や二重遷移が起きないことをテストで担保する。

## 完了条件

- 再送のみが続く期間でも `KeyRefreshNeeded` イベントが発行され、鍵切り替えが進行すること
- `send()` と再送の両経路から呼んでもイベントが重複発行されないこと
- 上記を検証するテストが `src/srt_connection.rs` 内の `#[cfg(test)]` モジュールに追加されていること
- `cargo test` で全テストが通過すること
