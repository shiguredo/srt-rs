# PBT と重複する単体テストが多数存在する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/test-remove-pbt-unit-duplication

## 目的

AGENTS.md「PBT でカバーできるものを単体テストで書かない」に違反し、以下の重複が存在する:

- `src/srt_receiver.rs` の `#[cfg(test)]`: `test_receiver_buffer_receive_in_order` ほか 10 件以上が PBT と重複
- `src/crypto.rs` の `#[cfg(test)]`: `test_km_refresh_state_transitions` 等が PBT と重複
- `src/srt_handshake.rs` の `#[cfg(test)]`: `test_handshake_encode_decode` 等が PBT と重複
- `src/srt_sender.rs` の `#[cfg(test)]`: `test_sender_buffer_push` 等が PBT と重複

## 設計方針

PBT が strategy ベースで網羅的に検証しているプロパティについては、固定値の単体テストを削除する。エラーパスや境界値のテストのみ単体テストとして残す。

## 完了条件

- PBT と重複する単体テストが削除されていること
- PBT でカバーされないエラーパス・境界値のテストは残っていること
- `cargo test` で全テストが通過すること
