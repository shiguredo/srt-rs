# ACKACK 送信条件が仕様より緩い

- Created: 2026-08-16
- Branch: feature/fix-ackack-condition-too-loose
- Polished: 2026-08-16

## 目的

`src/srt_connection.rs` の `handle_ack` メソッド内で、`pkt.control_info.len() >= 16` で ACKACK を送信している。SRT 仕様では「The sender only acknowledges the receipt of Full ACK packets」と規定されており (`refs/srt/draft-sharabayko-srt.md` の `#ctrl-pkt-ack` 節)、ACKACK は Full ACK の受信に対してのみ送信すべきである。

Full ACK の CIF は 28 bytes (ack_seq 4 + RTT 4 + RTTVar 4 + Buffer 4 + Packet Rate 4 + Link Capacity 4 + Recv Rate 4 = 28 bytes)、Small ACK の CIF は 16 bytes (Available Buffer Size フィールドまで = 4 フィールド × 4 bytes) である。

現在の自実装は ACK を 4 bytes (Light ACK) か 28 bytes (Full ACK) しか生成しないため、自実装同士の通信では現時点で実害がない。しかし、`>= 16` の条件は将来自実装に Small ACK (16 bytes) が追加された場合、および相手 (libsrt 等) が Small ACK を送信してきた場合の両方で、仕様違反の ACKACK を送信してしまう。

## 現状

```rust
if pkt.control_info.len() >= 16 {
    self.send_ackack(pkt.type_specific_info, now);
}
```

`src/srt_connection.rs` の `handle_ack` メソッド内の該当箇所。

## 設計方針

`pkt.control_info.len() >= 16` を `pkt.control_info.len() >= 28` に変更する。28 は Full ACK の CIF サイズ (7 フィールド × 4 bytes) に一致する。

該当箇所のコメント「Full ACK (RTT, RTTVar, Buffer Size, Rate を含む)」も、`>= 28` が Full ACK の CIF サイズに対応すること、および根拠資料名 (`refs/srt/draft-sharabayko-srt.md`) と節 (`#ctrl-pkt-ack`) を明記する内容に更新する。

## 完了条件

- ACKACK 送信条件が `>= 28`（Full ACK の CIF サイズ）に変更されていること
- 16 bytes の CIF (Small ACK 相当) では ACKACK が送信されず、28 bytes の CIF (Full ACK 相当) では送信されることを検証するテストが `src/srt_connection.rs` 内の `#[cfg(test)]` モジュールに追加されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/srt_connection.rs` の `handle_ack` メソッド内のしきい値を `16` から `28` に変更し、該当コメントを更新する。テストは `src/srt_connection.rs` 内の `#[cfg(test)]` モジュールに追加し、16 bytes / 28 bytes の境界で ACKACK 送信の有無を検証する。
