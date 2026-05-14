# 英語コメント / 英語テストメッセージを日本語に修正する

- Priority: Low
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-english-comments-and-test-messages

## 目的

AGENTS.md に「コメントは全て日本語にすること」「テストメッセージは全て日本語にすること」と規定されているが、以下の違反が存在する。

## 優先度根拠

コードの統一的な日本語化による保守性改善。機能への影響はない。

## 現状

### 英語コメント（3 箇所）

1. `src/srt_connection.rs:594` — `// Private methods`
2. `src/srt_handshake.rs:278` — `// Peer IP (128 bits = 16 bytes)`
3. `src/srt_handshake.rs:747` — `// S = 0, V = 1, PT = 2`

### 英語テストメッセージ（5 箇所）

4. `src/srt_receiver.rs:869` — `"packet_rate should be > 0"`
5. `src/srt_receiver.rs:872` — `"byte_rate should be > 0"`
6. `src/srt_receiver.rs:903` — `"capacity should be > 0"`
7. `src/srt_congestion.rs:549-551` — `"estimated_input_bandwidth: {}"`
8. `src/srt_congestion.rs:586-588` — `"period_before: {}, period_after: {}"`

## 設計方針

各箇所を日本語に修正する。

### 修正対象

1. `// プライベートメソッド`
2. `// ピア IP（128 ビット = 16 バイト）`
3. 当該行を削除（上位行の日本語コメントに統合）
4. `"パケットレートが 0 より大きいこと"`
5. `"バイトレートが 0 より大きいこと"`
6. `"容量が 0 より大きいこと"`
7. `"推定入力帯域幅: {}"`
8. `"変更前の周期: {}, 変更後の周期: {}"`

### 他 issue との依存関係

- #0003: `srt_congestion.rs` を削除する場合、修正対象 7, 8 は不要になる。#0003 を先に対応すること

### テスト戦略

コメントとテストメッセージの修正のみであり、機能には影響しない。

## 完了条件

- 上記 8 箇所が日本語に修正されていること
- 英語のコメント/テストメッセージが残っていないこと（全ソースを grep で確認）
- `cargo test` で全テストが通過すること

## 解決方法

1. `src/srt_connection.rs:594` の `// Private methods` を `// プライベートメソッド` に修正
2. `src/srt_handshake.rs:278` の `// Peer IP (128 bits = 16 bytes)` を `// ピア IP (128 ビット = 16 バイト)` に修正
3. `src/srt_handshake.rs:747` の `// S = 0, V = 1, PT = 2` を削除
4. `src/srt_receiver.rs` の英語テストメッセージ 4 件を日本語に修正
5. `src/srt_congestion.rs` の修正対象 7, 8 は #0003 のファイル削除により不要化
