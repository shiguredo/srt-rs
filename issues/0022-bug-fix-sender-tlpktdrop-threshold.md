# Sender 側 drop_expired の TLPKTDROP 閾値が仕様に準拠していない

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-sender-tlpktdrop-threshold

## 目的

`src/srt_sender.rs:346-371` の `drop_expired()` が、送信側の期限切れ判定に `self.latency_us` をそのまま閾値として使っている。SRT 仕様 (§3.5 TLPKTDROP) では `TLPKTDROP_THRESHOLD = max(1.25 * TsbpdDelay, 1_000_000)` と定義されている。受信側 (`srt_receiver.rs:607-608`) は正しく実装済み。

## 優先度根拠

送信側の方が短い閾値でパケットを破棄してしまい、再送に必要なパケットを早期に捨てる可能性がある。ただし TLPKTDROP はオプショナル機能であるため Medium。

## 現状

```rust
// src/srt_sender.rs:346-371
pub fn drop_expired(&mut self, now: Timestamp) -> Vec<u32> {
    let expired: Vec<u32> = self.packets.iter()
        .filter_map(|(&seq, entry)| {
            let elapsed = now.as_micros().saturating_sub(entry.sent_time.as_micros());
            if elapsed > self.latency_us {  // 誤: latency をそのまま閾値に
                Some(seq)
            } else {
                None
            }
        }).collect();
    // ...
}
```

## 根拠

draft-sharabayko-srt.md 2179-2185 行:

> The recommended TLPKTDROP_THRESHOLD value is 1.25 times the SRT latency value. Note that the SRT sender keeps packets for at least 1 second.

## 設計方針

`self.latency_us` の代わりに `max(1.25 * self.latency_us, 1_000_000)` を使用する。

## 完了条件

- `drop_expired` の閾値が `max(1.25 * latency_us, 1_000_000)` になっていること
- `cargo test` で全テストが通過すること
