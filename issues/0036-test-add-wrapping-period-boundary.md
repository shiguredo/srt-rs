# TSBPD wrapping period 境界値テストが不足している

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-add-wrapping-period-boundary
- Polished: 2026-08-01

## 目的

`WRAPPING_PERIOD_START`（MAX_TIMESTAMP - 30_000_000 µs = 30 秒前）、`WRAPPING_PERIOD_END_MIN`（30_000_000 µs = 30 秒）、`WRAPPING_PERIOD_END_MAX`（60_000_000 µs = 60 秒）の境界値近傍での TSBPD ラップアラウンド挙動をテストする単体テストが存在しない。既存の「ラップ」テスト (`test_pop_ready_blocks_on_loss_across_wrap_boundary` 等) はシーケンス番号 (31-bit) のラップであり、タイムスタンプ (32-bit) のラップとは無関係である。

## 優先度根拠

32-bit タイムスタンプのラップアラウンドは SRT の重要な仕様であり、境界値テストは必須。#0021（終了判定の `pop_ready()` 移動）と #0044（終了範囲の開区間化）が境界挙動を変更するため、実装後の回帰を防ぐテストが必要。

## 設計方針

#0021 → #0044 → 本 issue の順で実装し、#0021 / #0044 実装後のコードをテスト対象とする。終了境界 (60 秒) の期待値は #0044 の開区間仕様 (`ts < WRAPPING_PERIOD_END_MAX`) に従う。

境界値テストのケース (直前 = 境界値 - 1 µs、ちょうど = 境界値そのもの):

- `WRAPPING_PERIOD_START` 直前: 開始しない / ちょうど: 開始する (現行コードの仕様 (inclusive) をロックする。参照実装 (libsrt) は strictly greater で 1 µs 排他的だが、本 issue は現行コードの挙動をテストする)
- `WRAPPING_PERIOD_END_MIN` 直前: 終了しない / ちょうど: 終了する (#0044 は上限のみ開区間化し、下限は変更しない。下限 inclusive は現行コードと参照実装 (libsrt) に整合する)
- `WRAPPING_PERIOD_END_MAX` 直前: 終了する / ちょうど: 終了しない (#0044 実装後)

検証方法: テストは `src/srt_receiver.rs` の `#[cfg(test)]` モジュール内に置くため、`wrapping_period_active` / `tsbpd_time_base` に直接アクセスして検証できる (既存の `test_drop_too_late_uses_tsbpd_time_base` が `buf.loss_list` に直接アクセスするのと同じパターン)。公開 API 経由の間接観測は不要であり、配信タイミングの観測では終了判定の発火有無を判別できない (ラップ後パケットの補正が相殺するため)。END 系テストは `wrapping_period_active` をフィールド直接設定で有効化してから終了境界のパケットを送る (START 境界のパケット経由で有効化すると、そのパケットがバッファに残り `pop_ready` の対象に混ざるため)。#0021 の検証テスト (ラップ後配信時刻の整合・`drop_too_late()`) とはスコープを分け、本 issue は境界の開始・終了判定に絞る。

なお、終了判定の発火後に遅延到着したラップ前パケットで開始判定が再発火し、`tsbpd_time_base` が二重に加算される問題 (#0021 の設計方針で既知問題と明記) は本 issue のテスト対象にしない。

## 相互作用

- #0021 (終了判定の `pop_ready()` 移動、Polished 済み) と #0044 (終了範囲の開区間化) が同じ wrapping period 実装を変更するため、本 issue は #0021 → #0044 の後に実装する
- #0028 (`receive()` の責務分割) は `receive()` を変更するため、本 issue は #0028 の後に実装する
- #0031 (インラインテストの `tests/` への移動) は open のままである。本 issue の検証方法は src/ 内の `#[cfg(test)]` モジュールを前提とするため、#0031 が先に実装された場合は private フィールドにアクセスできなくなる。本 issue を先に実装するか、#0031 の存続判断を確定させてから実装する

## テスト

テストは `src/srt_receiver.rs` の `#[cfg(test)]` モジュールに追加する (既存の TSBPD 系テストと同じ場所)。境界値は `super::WRAPPING_PERIOD_*` 定数を参照する。

## CHANGES.md

機能に直接影響しない変更 (後方互換を壊さない) のため、shiguredo-changelog 規約に従い `### misc` サブセクションに `[UPDATE]` エントリ (例: `[UPDATE] TSBPD wrapping period の境界値テストを追加する`。担当者行を付けて追加すること) を追加する。

## 完了条件

- `WRAPPING_PERIOD_START` 直前/ちょうど、`WRAPPING_PERIOD_END_MIN` 直前/ちょうど、`WRAPPING_PERIOD_END_MAX` 直前/ちょうどの境界値テストが追加され、それぞれの期待値 (開始/終了の発火・非発火) が検証されていること
- `cargo test` で全テストが通過すること
- CHANGES.md の `### misc` セクションに `[UPDATE]` エントリが追加されていること
