# time.rs の公開 API のテストが不足している

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-add-missing-time-tests
- Polished: 2026-08-01

## 目的

`src/time.rs` の公開 API のうち、`as_millis` と `add_millis` を参照するテストが存在しない。`saturating_sub` は pbt/tests/prop_time.rs が 0..1_000_000 の範囲で検証済みだが、境界値 (u64::MAX 近傍) をカバーしていない。

## 現状

- `pbt/tests/prop_time.rs` は `from_micros` の roundtrip / `add_micros` / `saturating_sub` の 3 件のみ (saturating_sub は 0..1_000_000 の範囲)
- `as_millis`（`self.0 / 1000` の切り捨て）と `add_millis`（`add_micros(millis * 1000)`）は未テスト

## 設計方針

shiguredo-rust スキルの「unittest は pbt で実現できないものだけを書くこと」「PBT でカバーできるものを単体テストで書かないこと」に従い、pbt/tests/prop_time.rs に以下の PBT を追加する (tests/test_time.rs の新設はしない):

- `as_millis`: 任意の micros に対して `as_millis() == micros / 1000`。strategy に `u64::MAX` を `prop_oneof!` で明示的に混ぜる (レンジ戦略だけでは確率的にしか出現しないため)
- `saturating_sub`: strategy を拡張し、`a < b` の飽和 (0 へのクランプ) と `u64::MAX` 近傍の境界を検証 (`u64::MAX` は `prop_oneof!` で明示的に混ぜる)
- `add_millis`: `micros + millis * 1000 <= u64::MAX` の範囲 (`micros <= u64::MAX - millis * 1000`) で `add_millis() == micros + millis * 1000`。範囲外では `u64::MAX` にクランプされることを検証する (実装は `saturating_add` のため。既存の `test_add_micros` が `checked_add` 分岐で回避しているのと同じ考慮が必要)。`millis` は `0..=u64::MAX / 1000` に制限し (検証式内の `millis * 1000` のオーバーフロー防止)、クランプ分岐に確実に到達するため `micros` に大きな値 (`u64::MAX` 近傍) を `prop_oneof!` で明示的に混ぜる

なお、`impl std::ops::Add<u64>` / `impl std::ops::Sub` (time.rs の演算子実装) はメソッド (`add_micros` / `saturating_sub`) と同一セマンティクスの別経路のため、本 issue では対象外とする。

なお、`pbt/tests/prop_error.rs` は現状存在せず、本 issue では新設しない。`ErrorKind` はフィールドなし enum でシリアライズ・デコード経路が存在せず「ラウンドトリップ」が定義不能であり、有限 5 バリアントの検証は PBT の役割ではない。`ErrorKind` の検証は既存の tests/test_error.rs が #0026 (Error のカプセル化) 実装後の `kind()` アクセサ経由で継続される。

## 相互作用

- #0026 (Error のカプセル化) は error.rs を変更するが、本 issue は error.rs のテストを対象外としたため競合しない
- #0034 (PBT と重複する単体テストの削除) の判定基準に従い、本 issue は単体テストを新設しない (PBT での検証に統一)

## テスト

追加した PBT で担保する。

## CHANGES.md

機能に直接影響しない変更 (後方互換がある追加) のため、shiguredo-changelog 規約に従い `### misc` サブセクションに `[ADD]` エントリ (例: `[ADD] time.rs の公開 API の PBT を追加する`。担当者行を付けて追加すること) を追加する。

## 完了条件

- pbt/tests/prop_time.rs に `as_millis` / `saturating_sub` (境界値含む) / `add_millis` の PBT が追加されていること
- `cargo test --workspace` で全テストが通過すること (pbt はワークスペースメンバーであり、ルートの `cargo test` では実行されないため)
- CHANGES.md の `### misc` セクションに `[ADD]` エントリが追加されていること
