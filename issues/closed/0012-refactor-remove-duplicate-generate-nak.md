# generate_nak と generate_periodic_nak が重複している

- Priority: Low
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-remove-duplicate-generate-nak

## 目的

`src/srt_receiver.rs` の `generate_nak` と `generate_periodic_nak` が同一の実装である。また `generate_nak` はどの呼び出し元からも使用されていない。

## 優先度根拠

重複除去による保守性改善。機能には影響しない。

## 現状

両メソッドは `loss_list` を `NakPacket` にラップするだけの同一実装。`generate_nak` は `&mut self`、`generate_periodic_nak` は `&self` である点のみが異なる。

`srt_connection.rs` では `generate_periodic_nak` のみが `send_periodic_nak` から呼び出されており、`generate_nak` に呼び出し元はない。

## 設計方針

未使用の `generate_nak` を削除する。`generate_periodic_nak` は `&self` のみで十分であり、現状の呼び出しに合致する。

### 修正対象

1. `src/srt_receiver.rs` の `generate_nak` メソッドを削除する

### テスト戦略

`generate_periodic_nak` の振る舞いは変更されないため、既存テストでカバレッジが維持される。

## 完了条件

- `generate_nak` が削除されていること
- `cargo test` で全テストが通過すること

## 解決方法

1. `src/srt_receiver.rs` の `generate_nak` メソッドを削除（`generate_periodic_nak` と重複）
2. `src/srt_receiver.rs` と `pbt/tests/prop_receiver.rs` のテストで `generate_nak()` 呼び出しを `generate_periodic_nak()` に置き換え
