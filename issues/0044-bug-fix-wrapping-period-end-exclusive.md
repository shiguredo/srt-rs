# wrapping period 終了範囲の上限が inclusive

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-wrapping-period-end-exclusive

## 目的

`src/srt_receiver.rs:437` の wrapping period 終了条件が `ts <= WRAPPING_PERIOD_END_MAX`（60 秒を含む閉区間）となっている。SRT 仕様 (draft-sharabayko-srt.md 2159-2160 行) では「within (30, 60) seconds interval」と括弧表記（開区間）で示されているため、`ts < WRAPPING_PERIOD_END_MAX` が正しい。

## 完了条件

- 終了条件が `ts < WRAPPING_PERIOD_END_MAX` になっていること
- `cargo test` で全テストが通過すること
