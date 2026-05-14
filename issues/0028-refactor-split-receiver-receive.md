# srt_receiver.rs の receive() メソッドが 99 行で責務過多

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-split-receiver-receive

## 目的

`src/srt_receiver.rs:393-491` の `receive()` メソッドが 99 行あり、1 メソッドに 10 個の責務が詰め込まれている。責務分離の観点で問題がある。

## 優先度根拠

機能には影響しないが、可読性・テスト容易性を著しく損なう。変更時の影響把握が困難。

## 現状

`receive()` メソッドが以下の責務を全て擔っている:
1. 重複チェック
2. 古いパケットの破棄
3. 統計情報更新
4. ジッター計算
5. TSBPD ラップアラウンド管理
6. 配信時刻計算
7. バッファ挿入
8. 損失検出
9. expected_seq 更新
10. 損失リストのクリーンアップ

## 設計方針

各処理をプライベートメソッドに抽出する。例:
- `check_duplicate(seq) -> bool`
- `update_statistics(seq, packet_size, now)`
- `calculate_jitter(timestamp, now)`
- `update_tsbpd_wrapping(timestamp)`
- `calculate_delivery_time(timestamp, now) -> Timestamp`
- `detect_losses(seq) -> Vec<u32>`
- `advance_expected_seq()`

## 完了条件

- `receive()` メソッドが責務ごとに分割されていること
- `cargo test` で全テストが通過すること
