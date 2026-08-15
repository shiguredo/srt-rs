# wrapping period 終了範囲の上限が inclusive

- Priority: Low
- Created: 2026-05-14
- Completed: 2026-08-15
- Model: DeepSeek V4 Pro
- Branch: feature/fix-wrapping-period-end-exclusive
- Polished: 2026-08-15

## 目的

`src/srt_receiver.rs` の wrapping period 終了条件 `(WRAPPING_PERIOD_END_MIN..=WRAPPING_PERIOD_END_MAX).contains(&ts)` が 60 秒を含む閉区間となっている。SRT 仕様 (draft-sharabayko-srt.md の `#tsbpd-time-base` 節) では「within (30, 60) seconds interval」と括弧表記（開区間）で示されているため、上限は `ts < WRAPPING_PERIOD_END_MAX` が正しい。

なお、下限 (`WRAPPING_PERIOD_END_MIN`) は変更しない。仕様の開区間 (30, 60) を文字通り読むと下限も含まれないが、下限 inclusive は現行コードと参照実装 (libsrt) に整合する。参照実装 libsrt は `(usPktTimestamp >= TSBPD_WRAP_PERIOD) && (usPktTimestamp <= (TSBPD_WRAP_PERIOD * 2))` (TSBPD_WRAP_PERIOD = 30 秒) で両端 inclusive [30 秒, 60 秒] に実装しており、本 issue は上限のみ仕様文言準拠として開区間化する (下限を開区間化しない理由はこの参照実装との整合にある)。

## 優先度根拠

実害は 60 秒ちょうどのタイムスタンプのパケットにのみ影響し、終了判定の発火が遅れる。ラップ後 ts が [30 秒, 60 秒) の範囲のパケットを 1 つも受信しない場合 (30 秒以上の受信ギャップ・低レート伝送) は `wrapping_period_active` が残留し、次のラップ周期以降の全パケットに誤った補正が適用され続ける可能性がある。ただしラップ周期 01:11:35 ごとの稀な事象であり、通常運用では発生頻度は極めて低い。

## 設計方針

- 終了条件を `(WRAPPING_PERIOD_END_MIN..WRAPPING_PERIOD_END_MAX).contains(&ts)` (上限のみ開区間) に変更する。下限は変更しない
- #0021 (終了判定の `pop_ready()` 移動、Polished 済み) の後に実装する。#0021 実装後は終了判定が `pop_ready()` に移動するため、変更対象は #0021 実装後の `pop_ready()` 内の終了判定である
- 定数コメント「タイムスタンプがこの値を超えたら終了」が実際の終了条件 (下限以上・上限以下) と不整合のため更新する
- 開始条件 (`ts >= WRAPPING_PERIOD_START`) の参照実装 (libsrt は strictly greater) との差異は本 issue のスコープ外とする

## 相互作用

- #0021 (Polished 済み) は終了判定を `pop_ready()` に移動するため、本 issue は #0021 の後に実装する (#0021 の設計方針で実装順は #0021 → #0044)
- #0036 (Polished 済み) は #0021 → #0044 → #0036 の順で実装し、`WRAPPING_PERIOD_END_MAX` ちょうど「終了しない」を #0044 実装後の期待値としている。境界値テストは #0036 が担当する

## テスト

境界値テスト (60 秒ちょうどで終了しないこと) は #0036 のスコープであり、本 issue では新規テストを追加しない。既存の `cargo test` で全テストが通過すること。

## CHANGES.md

バグ修正のため、`[FIX]` エントリ (例: `[FIX] wrapping period 終了範囲の上限を開区間 (60 秒を含まない) に修正する`。担当者行を付けて追加すること) を追加する。

## 完了条件

- #0021 実装後の `pop_ready()` 内の終了条件が `(WRAPPING_PERIOD_END_MIN..WRAPPING_PERIOD_END_MAX).contains(&ts)` (上限のみ開区間化) になっていること
- 下限 (`WRAPPING_PERIOD_END_MIN`) が変更されていないこと
- 定数コメントが更新されていること
- `cargo test` で全テストが通過すること
- CHANGES.md に `[FIX]` エントリが追加されていること

## 解決方法

- `pop_ready()` 内の終了条件を `(WRAPPING_PERIOD_END_MIN..WRAPPING_PERIOD_END_MAX)` (上限のみ開区間) に変更した
- 下限 (`WRAPPING_PERIOD_END_MIN`) は変更していない
- 定数コメントを実際の終了条件に合わせて更新した
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加した
