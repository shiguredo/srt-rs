# start_pre_announce の状態遷移を wrap_sek 成功後にまとめる

- Priority: Low
- Created: 2026-08-30
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/refactor-transactional-start-pre-announce
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

`start_pre_announce` が `wrap_sek` 失敗時に、SEK の上書きや KM リフレッシュ状態の遷移を巻き戻さずに `Err` を返すため、失敗後に内部状態が不整合になる余地をなくす。

## 現状

- `src/crypto.rs` の `start_pre_announce` は、`new_sek` の長さ検証の後、`current_key.other()` 側の SEK を `new_sek` で上書きし、`next_key` を設定し、`km_refresh_state` を `PreAnnounce` に変更してから、最後に `wrap_sek` を呼んでいる
- `wrap_sek` が `Err` を返すと、上記の状態変更が確定したまま `Err` が返る。その後 `send()` が走ると `should_switch_key` が成立し、ピアへ新しい鍵を通知しないまま `current_key` が切り替わる
- 現在の不変条件 (`derive_kek` が `key_length.len()` バイトの KEK を返す、`new_sek` 長は冒頭で検証済み) の下では `wrap_sek` は実質到達しない。ただし到達不能であることが、コード上の離れた箇所の不変条件に依存している

## 設計方針

`wrap_sek` を先に実行して成功した後でのみ、SEK の上書き・`next_key` の設定・`km_refresh_state` の遷移を行う。外部から観測可能な成功時の挙動 (戻り値・ラップ済み SEK) は変わらない。失敗時に内部状態が一切変わらないことをテストで担保する。

## 完了条件

- `wrap_sek` 失敗時に SEK / `next_key` / `km_refresh_state` / `current_key` が一切変わらないこと
- 既存の KM リフレッシュ系テスト (`test_km_refresh_state_transitions` 等) が通ること
- `cargo test` で全テストが通過すること
