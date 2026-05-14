# TSBPD wrapping period 境界値テストが不足している

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/test-add-wrapping-period-boundary

## 目的

`WRAPPING_PERIOD_START`（MAX_TIMESTAMP - 30秒）、`WRAPPING_PERIOD_END_MIN`（30秒）、`WRAPPING_PERIOD_END_MAX`（60秒）の境界値近傍での TSBPD ラップアラウンド挙動をテストする単体テストが存在しない。

## 優先度根拠

32-bit タイムスタンプのラップアラウンドは SRT の重要な仕様であり、境界値テストは必須。

## 完了条件

- wrapping period 開始・終了の境界値テストが追加されていること
- `cargo test` で全テストが通過すること
