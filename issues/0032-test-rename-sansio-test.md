# tests/sansio_test.rs の命名規則違反を修正する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-rename-sansio-test
- Polished: 2026-08-01

## 目的

`tests/sansio_test.rs` は `src/srt_connection.rs` に対応するテスト (公開 API のみを使用する SRT 接続の e2e テスト) であるが、shiguredo-rust スキルの命名規則「単体テストのファイル名は `tests/test_<module>.rs` とし、`src/<module>.rs` に対応させること」に違反している。現行名 `sansio_test.rs` は「sansio 技法」を名前にしたもので、どのモジュールにも対応しない命名である。

## 現状

ファイル名: `tests/sansio_test.rs` (806 行、公開 API のみを使用する SRT 接続の e2e テスト)

## 設計方針

`tests/test_srt_connection.rs` に `git mv` でリネームする (履歴保持のため。内容は変更しない)。

## 相互作用

- #0027 (srt_connection.rs の分割) と #0028 (srt_receiver.rs の receive() 分割) のテストセクションが `tests/sansio_test.rs` を言及している (文書上の整合のため。両 issue は本ファイルを変更しない) ため、本 issue は #0027 / #0028 の後に実装する
- #0031 (インラインテストの `tests/` への移動) は open のままである。本 issue のリネーム先 (`tests/test_srt_connection.rs`) と #0031 の移動先 (srt_connection.rs のインラインテスト) が同一ファイル名のため、#0031 が先に実装された場合は `git mv` が既存ファイルと衝突する。本 issue を先に実装するか、#0031 の存続判断 (shiguredo-rust 規約: private を対象とするテストは `src/<module>.rs` 内に置く、と矛盾する) を #0031 側で確定させてから実装する
- PBT (pbt/tests/prop_connection.rs) と重複するテストは現に存在する (例: `test_send_before_connected` と `prop_disconnected_send_always_fails`) が、整理は本 issue のスコープ外とする (#0034 の対象は src/ 内のインラインテストのみで tests/ を含まないため、重複整理は別 issue で対応する)

## テスト

リネームのみで内容は不変のため、新規テストは追加しない。既存の `cargo test` で担保する。

## CHANGES.md

機能に直接影響しない変更 (後方互換を壊さない) のため、shiguredo-changelog 規約に従い `### misc` サブセクションに `[UPDATE]` エントリ (例: `[UPDATE] tests/sansio_test.rs を tests/test_srt_connection.rs にリネームする`。担当者行 (`- @ユーザー名`) を付けて追加すること) を追加する。

## 完了条件

- `tests/sansio_test.rs` が `tests/test_srt_connection.rs` にリネームされ、`tests/sansio_test.rs` が存在しないこと
- `cargo test` で全テストが通過すること
- CHANGES.md の `### misc` セクションに `[UPDATE]` エントリが追加されていること
