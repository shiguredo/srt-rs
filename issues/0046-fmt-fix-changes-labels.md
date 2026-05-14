# CHANGES.md の [CHANGE] ラベルと misc セクションの整合性を修正する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fmt-fix-changes-labels

## 目的

`CHANGES.md` の 2 つの misc エントリについて:

1. `- [CHANGE] sequence_less_than / sequence_greater_than を srt_packet.rs に集約する` — この変更は後方互換を壊さないため `[UPDATE]` が適切
2. `- [CHANGE] 未使用の srt_congestion モジュール (...) を削除する` — 削除された型が公開 API だったかどうかで `[CHANGE]` か `[UPDATE]` かを判断。公開 API だった場合は `[CHANGE]` で正しいが misc から本体セクションに移動すべき

## 完了条件

- 変更種別ラベルが適切に修正されていること
- エントリの配置がルールに従っていること
