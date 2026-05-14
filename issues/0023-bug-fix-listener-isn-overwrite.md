# Listener が自身の ISN を Caller の ISN で上書きしている

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-listener-isn-overwrite

## 目的

`src/srt_connection.rs:847` の `handle_handshake_listener` で `self.initial_seq = hs.initial_packet_seq` とし、CONCLUSION リクエストから受け取った Caller の ISN で Listener 自身の ISN を上書きしている。SRT 仕様では各ピアが独立して自身の ISN を宣言する。

## 優先度根拠

Listener の送信シーケンス番号が Caller のものに置き換わるため、ISN の衝突が発生しうる。両者が同じ ISN から開始した場合、ACK が正しく機能しない。ただし ISN は通常ランダム生成されること、Listener が先にデータ送信することは稀なことから Medium。

## 現状

```rust
// src/srt_connection.rs:847
self.initial_seq = hs.initial_packet_seq;
```

## 根拠

draft-sharabayko-srt.md 564-565 行:

> Initial Packet Sequence Number: 31 bits. The sequence number of the very first data packet to be sent.

また 1659-1662 行に「The only case when the Listener can have precedence over the Caller is the advertised Cipher Family and Block Size」とあり、ISN は交渉の対象ではない。

## 設計方針

`self.initial_seq`（Listener 自身の送信用 ISN）は Caller に上書きせず、`ConnectionOptions::initial_seq` から設定された値を保持する。代わりに `init_buffers` へ渡す `peer_initial_seq` として Caller の ISN を使用する。

## 完了条件

- `self.initial_seq` が CONCLUSION 受信時に上書きされないこと
- `init_buffers` に正しく Caller の ISN が渡されていること
- `cargo test` で全テストが通過すること
