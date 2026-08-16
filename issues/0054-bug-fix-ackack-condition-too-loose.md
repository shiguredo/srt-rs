# ACKACK 送信条件が仕様より緩い

- Priority: High
- Created: 2026-08-16
- Branch: feature/bug-fix-ackack-condition-too-loose

## 目的

`src/srt_connection.rs` の `handle_ack` メソッド内で、`pkt.control_info.len() >= 16` で ACKACK を送信している。仕様 (draft-sharabayko-srt.md) では「The sender only acknowledges the receipt of Full ACK packets」と規定されており、Full ACK の CIF は 28 bytes (ack_seq + RTT + RTTVar + Buffer + Packet Rate + Link Capacity + Recv Rate) である。現在の Small ACK は未実装だが、将来実装された場合に Small ACK (16 bytes) でも ACKACK を送信してしまう。

## 現状

```rust
if pkt.control_info.len() >= 16 {
    self.send_ackack(pkt.type_specific_info, now);
}
```

## 設計方針

`pkt.control_info.len() >= 28` に変更する。

## 完了条件

- ACKACK 送信条件が Full ACK の CIF サイズ (28 bytes) に変更されていること
- `cargo test` で全テストが通過すること
