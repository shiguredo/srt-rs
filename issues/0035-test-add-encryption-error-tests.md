# 暗号化ハンドシェイクのエラーパステストが不足している

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/test-add-encryption-error-tests

## 目的

以下の暗号化ハンドシェイクのエラーパスがテストされていない:

- `provide_new_sek` の全パス（エラーパス含む）
- KMRSP 不在エラー (`src/srt_connection.rs:774-778`)
- KMREQ 不在エラー (`src/srt_connection.rs:867-871`)
- 誤ったパスフレーズでのハンドシェイク拒否（`BAD_SECRET` 等）
- SYN Cookie 不一致 (`src/srt_connection.rs:841-843`)

## 優先度根拠

セキュリティ関連の重要なエラーパスが未検証。AGENTS.md のカバレッジ指針に従い、単体テストでカバーすべき。

## 完了条件

- 上記エラーパスがテストされていること
- `cargo test` で全テストが通過すること
