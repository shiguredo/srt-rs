# tests/test_time.rs と pbt/tests/prop_error.rs が存在しない

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/test-add-missing-test-files

## 目的

AGENTS.md の命名規則に従い、以下のテストファイルが必要だが存在しない:

- `tests/test_time.rs` — `as_millis()` の端数処理や `saturating_sub` の境界値
- `pbt/tests/prop_error.rs` — `ErrorKind` の全バリアントのラウンドトリップ

## 完了条件

- `tests/test_time.rs` が作成されていること
- `pbt/tests/prop_error.rs` が作成されていること
- `cargo test` で全テストが通過すること
