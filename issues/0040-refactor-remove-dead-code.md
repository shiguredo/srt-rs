# コードベース内のデッドコードを削除する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-remove-dead-code

## 目的

レビューで特定された以下のデッドコードを削除する:

1. `src/srt_sender.rs:53-54` — `max_buffer_size` (読み取られない)
2. `src/srt_receiver.rs:243-244` — `ReceivedPacket::recv_time` (読み取られない)
3. `src/srt_connection.rs:38` — `ConnectionState::Conclusion` (代入パスなし)
4. `src/srt_handshake.rs:633` — `HandshakeState::Failed` (読み取りなし)
5. `src/srt_handshake.rs:678-679` — `cipher_type::AES_GCM` (未使用)
6. `src/srt_connection.rs:983-1042` — `SRT_CMD_KMREQ`/`KMRSP` 定数重複
7. `src/srt_connection.rs:623` — `handle_data_packet` 内の不要な `clone()`
8. `src/srt_packet.rs:269` — `ControlPacket::new()` (未使用)
9. `src/srt_handshake.rs:503-521` — `add_congestion_extension`/`get_congestion_extension` (接続から未使用)
10. `src/srt_connection.rs:1013` — `check_km_refresh` の未使用パラメータ `_now`

## 完了条件

- 上記デッドコードが削除されていること
- `cargo test` で全テストが通過すること
