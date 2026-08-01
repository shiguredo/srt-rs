# 暗号化まわりのエラーパステストが不足している

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-add-encryption-error-tests
- Polished: 2026-08-01

## 目的

以下の暗号化関連のエラーパスがテストされていない:

- `provide_new_sek` の全パス:
  - 暗号化未設定 → `ErrorKind::CryptoError`（"encryption not enabled"）
  - SEK 長不一致 → `ErrorKind::CryptoError`（"invalid SEK length"）
  - 正常系 → KMREQ 送信と `Ok(())`（`poll_output` で取得したパケットを `SrtPacket::decode` し、`ControlPacket.subtype == 3` (SRT_CMD_KMREQ) であることを検証する）
- KMRSP 不在エラー（`handle_handshake_caller` の KMRSP 検証で "encryption enabled but no KMRSP" → `ErrorKind::HandshakeRejected`。caller のみ passphrase を設定した 2 接続で再現する)
- KMREQ 不在エラー（`handle_handshake_listener` の KMREQ 処理で "encryption required but no KMREQ" → `ErrorKind::HandshakeRejected`。listener のみ passphrase を設定した 2 接続で再現する)
- 誤ったパスフレーズでの拒否:
  - Listener 側: caller と listener で異なる passphrase を設定した 2 接続で再現する。`CryptoContext::new_receiver` の SEK アンラップ失敗（"AES key unwrap failed" → `ErrorKind::CryptoError`）。CryptoContext 単体レベルの既存 PBT (`test_wrong_passphrase_fails_unwrap`) とは異なり、ハンドシェイク経路 (KMREQ の salt を使った KEK 導出 → アンラップ失敗 → エラー伝播) を検証する
  - Caller 側: caller は passphrase 設定済みで、ピアが KM エラー付き KMRSP を返した場合の拒否（`KmError::BadSecret` → "peer has wrong secret" → `ErrorKind::HandshakeRejected`。`HandshakePacket::add_km_error(KmError::BadSecret)` で手組みパケットを注入して再現する。注入前に INDUCTION 交換を完了させて caller の `HandshakeState::ConclusionSent` 到達を済ませること。`KmError` の残り 3 種 (Unsecured / NoSecret / BadCryptoMode) は本 issue のスコープ外とする)

## 優先度根拠

セキュリティ関連の重要なエラーパスが未検証。shiguredo-rust スキルのカバレッジ駆動のテスト作成手順（エラーパス未カバー → 単体テストまたは fuzzing で対応）に従う。

## 相互作用

- #0025（SYN cookie のデフォルト値修正）に「SYN Cookie 不一致」のテスト項目が移管されている（「0035 に含まれる『SYN Cookie 不一致』のテスト項目は本 issue に移管する」）。本 issue は #0025 の後に実装し、SYN Cookie テストは #0025 側で実施する
- #0032（tests/sansio_test.rs のリネーム）の後に実装し、テストはリネーム後の `tests/test_srt_connection.rs` に追加する（#0032 の完了条件で `tests/sansio_test.rs` は存在しなくなるため）

## テスト

テストは `tests/sansio_test.rs`（#0032 実装後は `tests/test_srt_connection.rs`）に追加する（既存の暗号化テストと同じパターン）。エラー検証では `let _ = ...` でエラーを握りつぶす既存の転送ヘルパーを使わず、`feed_recv_buf` の戻り値を直接検証すること。reason 文字列の検証は `format!` で詳細が付加される場合があるため、厳密一致ではなく前方一致（`starts_with`）で検証すること。

## CHANGES.md

機能に直接影響しない変更（後方互換を壊さない）のため、shiguredo-changelog 規約に従い `### misc` サブセクションに `[UPDATE]` エントリ（例: `[UPDATE] 暗号化まわりのエラーパステストを追加する`。担当者行を付けて追加すること）を追加する。

## 完了条件

- 目的セクションで列挙したエラーパスと `provide_new_sek` の正常系 (KMREQ 送信) がテストされ、エラー種別（`ErrorKind::CryptoError` / `ErrorKind::HandshakeRejected`）と reason 文字列が検証されていること
- `cargo test` で全テストが通過すること
- CHANGES.md の `### misc` セクションに `[UPDATE]` エントリが追加されていること
