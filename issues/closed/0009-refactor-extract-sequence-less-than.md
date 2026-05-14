# sequence_less_than が srt_sender.rs と srt_receiver.rs で重複定義されている

- Priority: Low
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-extract-sequence-less-than

## 目的

`sequence_less_than` および `sequence_greater_than` の同一ロジックが `src/srt_sender.rs:430-434` と `src/srt_receiver.rs:675-683` に重複定義されている。DRY 原則に反し、修正漏れや挙動不整合のリスクがある。

## 優先度根拠

重複除去による保守性改善が目的であり、機能には影響しない。

## 現状

`src/srt_sender.rs` に `sequence_less_than`、`src/srt_receiver.rs` に `sequence_less_than` と `sequence_greater_than` が定義されており、両者は完全に同一の実装。`sequence_greater_than` は `sequence_less_than(b, a)` の薄いラッパー。

## 設計方針

両関数を `src/srt_packet.rs` に移動し `pub(crate)` として公開する。シーケンス番号はパケット層の概念であり、`srt_packet.rs` が適切な配置場所である。

### 修正対象

1. `sequence_less_than` と `sequence_greater_than` を `src/srt_packet.rs` に `pub(crate)` で移動する
2. `src/srt_sender.rs` と `src/srt_receiver.rs` の重複定義を削除し、`crate::srt_packet::sequence_less_than` を使用するよう変更する

### テスト戦略

既存テストで `sequence_less_than` / `sequence_greater_than` が関与する振る舞いが引き続き正しく動作することを確認する。新規テストの追加は不要。

## 完了条件

- `sequence_less_than` / `sequence_greater_than` が `src/srt_packet.rs` に 1 箇所のみ存在すること
- すべての呼び出し元が正しく import されていること
- `cargo test` で全テストが通過すること

## 解決方法

1. `sequence_less_than` と `sequence_greater_than` を `src/srt_packet.rs` に `pub(crate)` で移動
2. `src/srt_sender.rs` と `src/srt_receiver.rs` の重複定義を削除し、`crate::srt_packet` から import するよう修正
