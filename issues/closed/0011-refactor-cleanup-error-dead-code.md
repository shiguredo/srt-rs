# error.rs の未使用コンストラクタと不要な allow 属性を整理する

- Priority: Low
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-cleanup-error-dead-code

## 目的

`src/error.rs` に以下の死にコードが存在する:

1. 未使用のコンストラクタ 3 件とそれに対応する `ErrorKind` バリアント
2. 未使用の `srt_rejection_code` フィールド
3. 使用中にも関わらず `#[allow(dead_code)]` が付与されているコンストラクタ 2 件

## 優先度根拠

死にコードの除去による保守性改善。機能への影響はない。

## 現状

- `ErrorKind::InvalidInput`、`ErrorKind::Unsupported`、`ErrorKind::ProtocolViolation` — いずれもコンストラクタが未使用
- `srt_rejection_code` フィールド — 常に `None`
- `crypto_error`、`handshake_rejected` — 使用中だが `#[allow(dead_code)]` 属性が残っている

## 設計方針

未使用のコンストラクタ、`ErrorKind` バリアント、`srt_rejection_code` フィールドを削除する。使用中の `crypto_error` と `handshake_rejected` から `#[allow(dead_code)]` を削除する。

### 修正対象

1. `invalid_input`、`unsupported`、`protocol_violation`、`with_srt_rejection_code` の 4 関数を削除する
2. `ErrorKind::InvalidInput`、`ErrorKind::Unsupported`、`ErrorKind::ProtocolViolation` バリアントを削除する
3. `srt_rejection_code` フィールドと `Display` 実装内の関連分岐を削除する
4. `crypto_error` と `handshake_rejected` の `#[allow(dead_code)]` 属性を削除する

### テスト戦略

`tests/test_error.rs`（#0010 で追加予定）で `ErrorKind` バリアント削除の影響を確認する。

## 完了条件

- 未使用のコードが削除されていること
- 不要な `#[allow(dead_code)]` 属性が削除されていること
- `cargo test` で全テストが通過すること

## 解決方法

1. `ErrorKind::InvalidInput`、`ErrorKind::Unsupported`、`ErrorKind::ProtocolViolation` の未使用バリアントを削除
2. `invalid_input`、`unsupported`、`protocol_violation` の未使用コンストラクタを削除
3. `srt_rejection_code` フィールド、`with_srt_rejection_code` メソッド、Display 実装内の関連分岐を削除
4. 使用中の `crypto_error` と `handshake_rejected` から `#[allow(dead_code)]` 属性を削除
