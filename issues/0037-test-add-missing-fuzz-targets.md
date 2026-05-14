# Fuzzing ターゲットが不足している

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/test-add-missing-fuzz-targets

## 目的

現在の fuzz ターゲットは `fuzz_handshake_decode.rs` と `fuzz_packet_decode.rs` の 2 つのみ。以下のターゲットが不足:

- `ReceiverBuffer::receive`（任意の DataPacket でパニックしないこと）
- `KmMessage::decode`（任意バイト列でパニックしないこと）
- `AccessControl::parse`（任意文字列でパニックしないこと）
- `parse_loss_list` / `encode_loss_list`（任意バイト列/損失リストでパニックしないこと）

## 優先度根拠

AGENTS.md が Fuzzing の役割を「任意入力に対するクラッシュ耐性」と定義しているが、カバレッジが不十分。

## 完了条件

- 上記の fuzz ターゲットが追加されていること
- 各ターゲットが `cargo fuzz` で実行可能であること
