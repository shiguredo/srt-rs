# 公開メソッドに /// ドキュメントコメントが不足している

- Priority: High
- Created: 2026-08-16
- Branch: feature/fmt-add-missing-doc-comments

## 目的

shiguredo-rust 規約「公開 API には必ず `///` を書くこと」に違反する公開メソッドが多数存在する。以下のメソッドに `///` ドキュメントコメントが不足している。

## 現状

`///` が不足している `pub fn`:

- `src/srt_receiver.rs`: `ReceiverBuffer::new`、`stats`、`ack_number`
- `src/srt_sender.rs`: `SenderBuffer::push`、`push_message`、`pop_retransmit`
- `src/srt_packet.rs`: `ControlPacket::new`
- `src/srt_connection.rs`: `has_retransmit`、`process_retransmit`、`can_send`、`can_send_with_pacing`、`time_until_send`、`set_packet_send_period`、`sender_stats`、`receiver_stats`

## 設計方針

上記の全公開メソッドに `///` ドキュメントコメントを追加する。利用者視点で「それが何であるか・何をするか」を書く。

## 完了条件

- 上記の全公開メソッドに `///` が追加されていること
- `cargo doc` で警告が発生しないこと
