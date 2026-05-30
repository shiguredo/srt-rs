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

## 解決方法

調査の結果、本 issue は偽陽性であり、現在の実装 (`src/srt_connection.rs:847` の `self.initial_seq = hs.initial_packet_seq;`、closed #0001 で追加) が正しいと判定した。コード変更は行わない (close)。

### 判定根拠

libsrt (Haivision 公式実装) のソースで実挙動を確認した:

- `CUDT::acceptAndRespond` (`srtcore/core.cpp`): Listener は `m_iISN = w_hs.m_iISN;` で Caller の ISN を自身の送信 ISN として採用する。該当行のコメントは "use peer's ISN and send it back for security check"。
- Caller 側の security check (`srtcore/core.cpp`): `(!m_config.bRendezvous) && (m_ConnRes.m_iISN != m_iISN)` が真なら `MN_SECURITY` で接続を拒否する。すなわち Listener は CONCLUSION レスポンスで Caller の ISN をそのまま返さなければならない。

したがって本 issue が提案する L847 の削除 (Listener が自身の独立 ISN を返す) を適用すると、libsrt Caller との接続が security check で拒否され、相互運用性が壊れる。現状の実装が libsrt と一致しており正しい。

仕様 (`refs/srt/draft-sharabayko-srt.md`) 1659 行の "TODO: Incorrect?" は、1660-1662 の precedence に関する文章記述に対する仕様著者自身の疑念であり、libsrt の実挙動は曖昧さなく現状実装を支持する。本 issue が依拠した「ISN は precedence の例外に含まれない」という根拠は、この未確定な文章記述に基づくものだった。

### closed #0001 との関係

本 issue は closed #0001 と正反対の主張だが、上記 libsrt 検証により #0001 が正しいと確定した。#0001 の reopen は不要である。
