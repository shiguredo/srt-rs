# 再接続時に古いイベントが event_queue に残る

- Priority: Medium
- Created: 2026-08-30
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/change-clear-event-queue-on-disconnect
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

`disconnect()` 後に同一インスタンスで再接続した場合、前の接続で消費されなかったイベントが `event_queue` に残り続ける問題を見直す。残った `KeyRefreshNeeded` に応答すると、新しい接続の暗号化カウンタが閾値に達していない段階で鍵ローテーションが始まり、意図しない挙動になる。

## 現状

- `src/srt_connection.rs` の `disconnect()` は shutdown の送信と状態遷移のみを行い、`event_queue` / `output_queue` をクリアしない
- `connect()` は接続状態をチェックせずに再実行できるため、切断済みインスタンスでの再接続は許容された使用パターンと解釈できる
- 再接続時にはハンドシェイクで `CryptoContext` が再生成され、`key_refresh_event_sent` フラグも初期化される (イベント発行の重複防止フラグとしては新しい接続と同じ起点に揃う) 一方、`event_queue` はクリアされず非対称になっている
- 古い `KeyRefreshNeeded` が残ったまま再接続すると、利用者がそれに応答して `provide_new_sek` を呼んだ時点で、カウンタが 0 に近い新しい接続でも事前通知が始まる

## 設計方針

`connect()` 時 (または `disconnect()` 時) に `event_queue` / `output_queue` をクリアする方式と、イベントに世代を持たせる方式のいずれを採るかを決める。利用者がまだ `poll_event()` していないイベントを握り潰さないか、切断済み接続のイベントに意味が無いことを利用者契約としてどう説明するかを含めて設計を確定する。

## 完了条件

- 再接続後に前接続のイベントが残らないこと (または設計判断とその根拠が明記され、挙動が文書化されていること)
- 上記を検証するテストが追加されていること
- `cargo test` で全テストが通過すること
