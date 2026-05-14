# tests/sansio_test.rs の命名規則違反を修正する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/test-rename-sansio-test

## 目的

`tests/sansio_test.rs` は `src/srt_connection.rs` に対応する単体テストであるが、AGENTS.md の命名規則「`tests/test_<module>.rs`」に違反している。

## 現状

ファイル名: `tests/sansio_test.rs` （828 行）

## 設計方針

`tests/test_srt_connection.rs` にリネームする。

## 完了条件

- ファイル名が `tests/test_srt_connection.rs` になっていること
- `cargo test` で全テストが通過すること
