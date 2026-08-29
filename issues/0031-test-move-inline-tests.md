# src/ 内の #[cfg(test)] mod tests を tests/test_<module>.rs に移動する

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/test-move-inline-tests

## 目的

以下の 8 ファイルの `#[cfg(test)] mod tests` を、AGENTS.md の命名規則に従い `tests/test_<module>.rs` に移動する:

- `src/crypto.rs:432-586` → `tests/test_crypto.rs`
- `src/srt_receiver.rs:698-1215` → `tests/test_srt_receiver.rs`
- `src/srt_sender.rs:429-553` → `tests/test_srt_sender.rs`
- `src/srt_handshake.rs:847-1094` → `tests/test_srt_handshake.rs`
- `src/srt_packet.rs:329-387` → `tests/test_srt_packet.rs`
- `src/stream_id.rs:295-390` → `tests/test_stream_id.rs`
- `src/srt_connection.rs:1458-1523` → `tests/test_srt_connection.rs`
- `src/time.rs` (テストモジュール内は `OVERFLOW_BOUNDARY_MILLIS` と `add_millis` の境界値テスト) → `tests/test_time.rs`

なお上記の行番号は作成時点のもので、現在はずれいている。行番号ではなくシンボル名で特定する規約に合わせる更新は、この issue の着手時に現状と突き合わせて行うこと。

## 優先度根拠

AGENTS.md の規定違反であり、コードベースの一貫性を損なう。機能には影響しない。

## 設計方針

各 `#[cfg(test)]` ブロックの内容を対応する `tests/test_<module>.rs` に移動する。非公開 API へのアクセスが必要なテストは `pub(crate)` 化またはテストユーティリティの公開で対応する。

## 完了条件

- 8 ファイルの `#[cfg(test)] mod tests` が削除されていること
- 対応する `tests/test_<module>.rs` が作成されていること
- `src/time.rs` と issue 0038 (PBT で `add_millis` を検証する) の役割分担を決めていること。0038 が `millis` を飽和しない範囲に制限する限り、境界値・極値の検証は PBT では代替できない
- `cargo test` で全テストが通過すること
