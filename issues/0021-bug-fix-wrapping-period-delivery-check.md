# TSBPD wrapping period の終了判定がパケット配信時ではなく受信時に行われている

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-wrapping-period-delivery-check

## 目的

`src/srt_receiver.rs` の wrapping period 終了判定がパケット受信時 (`receive()` 内) に行われているが、SRT 仕様 (draft-sharabayko-srt.md 2159-2160 行) では「パケットが配信された時点」で wrapping period を終了すると定義されている。

## 優先度根拠

受信から配信までの間にタイムラグがある場合、wrapping period の終了タイミングが仕様より早まり、TsbpdTimeBase の再計算が前倒しされる可能性がある。長時間ストリーミングでの累積誤差になるが、即時的な破綻には至らないため Medium。

## 現状

```rust
// src/srt_receiver.rs:435-441 (receive() 内)
if self.wrapping_period_active
    && ts >= WRAPPING_PERIOD_END_MIN
    && ts <= WRAPPING_PERIOD_END_MAX
{
    self.tsbpd_time_base += MAX_TIMESTAMP + 1;
    self.wrapping_period_active = false;
}
```

## 根拠

draft-sharabayko-srt.md 2158-2164 行:

> The TSBPD wrapping period starts 30 seconds before reaching the maximum timestamp value of a packet and ends once the packet with timestamp within (30, 60) seconds interval is delivered (read from the buffer).

## 設計方針

wrapping period 終了判定を `pop_ready()` に移動する。受信時には開始判定のみを行い、終了は配信時に行う。

## 完了条件

- wrapping period 終了判定が `pop_ready()` で行われていること
- `cargo test` で全テストが通過すること
