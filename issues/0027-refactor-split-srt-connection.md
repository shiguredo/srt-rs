# srt_connection.rs が 1523 行で過大 — モジュール分割が必要

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-split-srt-connection

## 目的

`src/srt_connection.rs` が 1523 行あり、AGENTS.md の「テストが長くなるのはモジュール自体が大きすぎるサイン」という指針に該当する。複数の責務が 1 ファイルに詰め込まれている。

## 優先度根拠

機能には影響しないが、可読性・保守性の低下が著しい。新規機能追加時の変更影響範囲が広くなる。

## 現状

以下の責務が 1 ファイルに混在している:

- `parse_loss_list` / `encode_loss_list` (L1377-1456) — NAK エンコーディング、`srt_receiver.rs` に移動可能
- `send_*` メソッド群 (L1105-1373) — 10 個の送信メソッド
- ハンドシェイク処理 (L704-895) — `handle_handshake_caller` (90行) + `handle_handshake_listener` (77行)

## 設計方針

案:
1. NAK エンコード/デコードを `srt_receiver.rs` に移動
2. 送信メソッド群を `src/srt_connection/send.rs` に分割
3. ハンドシェイク状態機械を `src/srt_connection/handshake.rs` に分割

## 完了条件

- `srt_connection.rs` が分割されていること
- モジュール間の依存関係が明確であること
- `cargo test` で全テストが通過すること
