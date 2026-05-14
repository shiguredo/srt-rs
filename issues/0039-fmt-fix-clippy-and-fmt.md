# cargo fmt と cargo clippy の違反を修正する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-clippy-and-fmt-violations

## 目的

以下のビルドチェック違反を修正する:

**cargo fmt 違反 (5 箇所):**
- `src/srt_connection.rs:1128` — `if` 式の改行
- `src/srt_receiver.rs:13` — import 順序
- `src/srt_receiver.rs:349` — 関数シグネチャの改行
- `src/srt_receiver.rs:604` — `let` 行の連結
- `src/srt_sender.rs:11` — import 順序

**cargo clippy 違反 (2 件):**
- `src/srt_receiver.rs:436-437` — `manual_range_contains`: `ts >= MIN && ts <= MAX` → `(MIN..=MAX).contains(&ts)`
- `src/srt_packet.rs:390-396` — `items_after_test_module`: `sequence_less_than` 等が `mod tests` より後ろにある

## 完了条件

- `cargo fmt --all -- --check` が通過すること
- `cargo clippy --all-targets --all-features -- -D warnings` が通過すること
