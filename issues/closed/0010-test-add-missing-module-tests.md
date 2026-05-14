# buf.rs / error.rs / time.rs に対応するテストが存在しない

- Priority: Medium
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/add-missing-module-tests

## 目的

以下の 3 つのソースモジュールに対応するテストファイルが存在しない:

1. `src/buf.rs` — `ByteSliceExt` trait と `VecExt` trait
2. `src/error.rs` — `ErrorKind` 列挙、`Error` 型、`Display` / `Debug` 実装
3. `src/time.rs` — `Timestamp` 型の全メソッド

AGENTS.md は「各 `src/<module>.rs` に対応する PBT または単体テストを置く」と規定している。

## 優先度根拠

テストがないことで変更時の退行確認ができない。ただしこれらのモジュールは他モジュールのテストを通じて間接的に検証されており、緊急性は Medium。

## 設計方針

PBT と単体テストを AGENTS.md の役割分担に従って作成する。

### テスト対象

1. `pbt/tests/prop_buf.rs` — write/read のラウンドトリップ（u16/u32/u64/u128 → write → read → 一致）
2. `tests/test_buf.rs` — PBT で実現できないエラーパス（`read_utf8` の invalid UTF-8 入力）
3. `tests/test_error.rs` — `check_buffer_size` の境界値、`Error::new` の振る舞い
4. `pbt/tests/prop_time.rs` — `from_micros(x).as_micros() == x` のラウンドトリップ

## 完了条件

- 上記 4 つのテストファイルが作成されていること
- 全テストが `cargo test` で通過すること

## 解決方法

1. `pbt/tests/prop_buf.rs` を作成し、u16/u32/u64/bytes の write/read ラウンドトリップ PBT を追加
2. `tests/test_buf.rs` を作成し、無効な UTF-8 入力のエラーパステストを追加
3. `tests/test_error.rs` を作成し、バッファサイズ境界値テストを追加
4. `pbt/tests/prop_time.rs` を作成し、Timestamp の from_micros/as_micros ラウンドトリップ PBT を追加
