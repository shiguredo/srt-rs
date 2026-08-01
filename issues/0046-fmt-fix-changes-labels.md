# CHANGES.md の [CHANGE] ラベルと misc セクションの整合性を修正する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-changes-labels
- Polished: 2026-08-01

## 目的

`CHANGES.md` の `### misc` セクションに、ラベルと配置が shiguredo-changelog 規約に従っていないエントリが 2 件ある。これを修正する。

1. `- [CHANGE] sequence_less_than / sequence_greater_than を srt_packet.rs に集約する` — この変更は後方互換を壊さないため `[UPDATE]` が適切
2. `- [CHANGE] 未使用の srt_congestion モジュール (AckInfo, BandwidthMode, CongestionControl, LiveCc) を削除する` — 削除された 4 型は `src/lib.rs` の `pub use` で公開 API だったため `[CHANGE]` は正しい。ただし misc ではなく本体セクション（`## develop` 直下）に置くべき

## 現状

- `CHANGES.md` 冒頭の定義: `[CHANGE]` は後方互換のない変更、`[UPDATE]` は後方互換がある変更
- shiguredo-changelog 規約: 機能に直接影響しない変更（ドキュメント追加、リファクタリング等）は `### misc` サブセクションに記載する。エントリは種別の順番（CHANGE → ADD → UPDATE → FIX）で記載する
- エントリ 1 の実態: 集約前は `src/srt_sender.rs` と `src/srt_receiver.rs` の private 関数、集約後も `src/srt_packet.rs` の `pub(crate)` 関数であり、公開 API には含まれない。後方互換を壊さない
- エントリ 2 の実態: 削除前は `src/lib.rs` で `pub use srt_congestion::{AckInfo, BandwidthMode, CongestionControl, LiveCc};` されており、公開 API だった。公開 API の削除は利用者に影響するため「機能に直接影響しない変更」には該当せず、misc ではなく本体セクションに置くべき

## 設計方針

- エントリ 1 のラベルを `[CHANGE]` から `[UPDATE]` に変更し、misc 内の種別順（CHANGE → ADD → UPDATE → FIX）に従って `[UPDATE]` エントリの列へ移動する
- エントリ 2 は `[CHANGE]` のまま `### misc` から `## develop` 直下の `[CHANGE]` エントリの列へ移動する（先頭の `[CHANGE]` エントリの直下）

### スコープ外

- `- [CHANGE] MSRV (rust-version) を 1.88 から 1.93 に上げる` は対象外（ビルド要件の変更であり misc 配置の妥当性は別途判断する）
- 未実装の #0027・#0028・#0030 は misc に `[CHANGE]` エントリを追加する予定だが、これらは後方互換を壊さない変更のため、実装時は本 issue の原則に従って `[UPDATE]` を付けること（本 issue を先に実装するのが望ましい。本 issue より先に実装する場合は `[UPDATE]` を付けること）
- #0040 の `HandshakeState::Failed` 削除は公開 API の削除であり、本 issue の原則（現状のエントリ 2 の実態を参照）に従って `[CHANGE]` のまま本体セクション（`## develop` 直下の `[CHANGE]` エントリの列、エントリ 2 の直下）に置くこと。#0040 に記載されている misc への追加は本判断により置き換えられるべきである

## 完了条件

- エントリ 1 のラベルが `[UPDATE]` に変更され、misc 内の `[UPDATE]` エントリの列に配置されていること
- エントリ 2 が `[CHANGE]` のまま `### misc` から `## develop` 直下の `[CHANGE]` エントリの列に移動されていること
- `## develop` 直下と misc 内のエントリが種別順（CHANGE → ADD → UPDATE → FIX）に並んでいること

## 解決方法

1. `CHANGES.md` の `### misc` セクションのエントリ 1 を `[UPDATE]` に変更し、担当者行（`- @voluntas`）を含むブロック単位で misc 内の `[UPDATE]` エントリの列（`CI / Release ワークフローの Slack 通知を shiguredo/github-actions の slack-notify に移行する` の直下）へ移動する
2. `CHANGES.md` の `### misc` セクションからエントリ 2 を担当者行を含むブロック単位で削除し、`## develop` 直下の `[CHANGE]` エントリの列（`デバッグ出力を eprintln! から tracing に移行し ConnectionOptions::debug を削除する` の直下）へ移動する
3. エントリの種別順（CHANGE → ADD → UPDATE → FIX）が崩れていないことを確認する
