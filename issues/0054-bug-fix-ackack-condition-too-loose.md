# ACKACK 送信条件が仕様より緩い

- Created: 2026-08-16
- Branch: feature/fix-ackack-condition-too-loose
- Polished: 2026-08-16

## 目的

`src/srt_connection.rs` の `handle_ack` メソッド内で、`pkt.control_info.len() >= 16` で ACKACK を送信している。SRT 仕様では「The sender only acknowledges the receipt of Full ACK packets」と規定されており、Full ACK の CIF (Control Information Field) は 28 bytes である。現在の Small ACK は未実装だが、将来 Small ACK (16 bytes) が実装された場合に、誤って ACKACK を送信してしまう。

## 現状

```rust
if pkt.control_info.len() >= 16 {
    self.send_ackack(pkt.type_specific_info, now);
}
```

## 設計方針

`pkt.control_info.len() >= 16` を `pkt.control_info.len() >= 28` に変更する。

## 完了条件

- ACKACK 送信条件が `>= 28`（Full ACK の CIF サイズ）に変更されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/srt_connection.rs` の `handle_ack` メソッド内のしきい値を `16` から `28` に変更する。
