# SID/Congestion 拡張のエンコード/デコードが重複している

- Priority: Low
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-extract-le-words-encoding

## 目的

`src/srt_handshake.rs` の `add_sid_extension`、`get_sid_extension`、`add_congestion_extension`、`get_congestion_extension` の 4 メソッドが、32-bit little endian words へのエンコード/デコード処理を重複実装している。DRY 原則に違反しており、共通化により保守性を向上させる。

## 優先度根拠

重複除去による保守性改善。機能には影響しない。

## 現状

4 つのメソッドはいずれも以下の同一ロジックを持つ:
1. 文字列 → 4 バイト境界パディング + LE words 形式への変換（エンコード）
2. LE words → バイト順復元 + ゼロパディング除去 → 文字列への変換（デコード）

`add_sid_extension` と `add_congestion_extension` は extension type のみ異なる同一実装。

## 設計方針

`encode_le_words` と `decode_le_words` の 2 つのユーティリティ関数を抽出し、4 つのメソッドから重複を除去する。

```rust
fn encode_le_words(s: &str, max_len: usize) -> Vec<u8> { ... }
fn decode_le_words(data: &[u8]) -> Option<String> { ... }
```

### 修正対象

1. `encode_le_words` と `decode_le_words` を `src/srt_handshake.rs` 内に `fn` として定義する
2. `add_sid_extension`、`get_sid_extension`、`add_congestion_extension`、`get_congestion_extension` の実装を共通関数を使う形に書き換える

### テスト戦略

既存テストで拡張のエンコード/デコードが引き続き正しく動作することを確認する。

## 完了条件

- 4 メソッドが共通の `encode_le_words` / `decode_le_words` を使用していること
- `cargo test` で全テストが通過すること

## 解決方法

1. `encode_le_words` と `decode_le_words` の共通ユーティリティ関数を `src/srt_handshake.rs` に追加
2. `add_sid_extension`、`get_sid_extension`、`add_congestion_extension`、`get_congestion_extension` の実装を共通関数を使う形に書き換え
