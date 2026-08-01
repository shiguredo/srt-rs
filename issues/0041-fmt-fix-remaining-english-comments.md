# 英語コメントの残存箇所を日本語に修正する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-remaining-english-comments
- Polished: 2026-08-01

## 目的

AGENTS.md の「コメントは全て日本語にすること」に違反する英語コメントが残存している。代表例:

1. `src/crypto.rs` の `KeyLength::Aes128` / `KeyLength::Aes256` の doc コメント（`AES-128 (16 bytes)` / `AES-256 (32 bytes)`）
2. `src/srt_connection.rs` のモジュール doc（`SRT Connection (sansio パターン)` に英語混在）
3. 同種の残存コメント: `Salt (16 bytes)`（crypto.rs / srt_handshake.rs）、`// Peer IP`（srt_handshake.rs）、`// Wrapped Key`（srt_handshake.rs）、`// packets/sec` 等（srt_connection.rs）

## 優先度根拠

コードの統一的な日本語化による保守性改善。機能への影響はない。

## 現状

closed/0016（fmt-fix-english-comments-and-test-messages）が 8 箇所を修正済みだが、完了条件「英語のコメント/テストメッセージが残っていないこと（全ソースを grep で確認）」は実質未達のまま close され、上記の英語コメントが残存している。

## 設計方針

対象外とする英語表現（この基準に該当しない完全英語のコメントはすべて日本語化の対象）:

- SRT 仕様の技術用語（AES-CTR、SYN Cookie、TSBPD、KMREQ、ACK 等）
- コード例・ASCII 図・引用コード内の英語
- enum 定数名・変数名・型名そのもの

修正後の日本語表現は、技術用語を括弧書きで補う形にする（例: `AES-128 (16 バイト)`、`ピア IP (128 ビット = 16 バイト)`。AGENTS.md の「全角と半角の間には半角スペースを入れること」に従う）。完全英語のコメントだけでなく、英語混在のコメント（例: `// Salt の下位 64 bits (8 bytes) を使用`）も日本語化の対象とする。テスト内の panic メッセージ等の英語ログメッセージも日本語化する（AGENTS.md の「テストのログメッセージは全て日本語にすること」に従う）。なお、本番コードのログメッセージは AGENTS.md の「ログメッセージは全て英語にすること」に従い日本語化しない。

## 相互作用

- #0027 (srt_connection.rs の分割) は `src/srt_connection.rs` を `src/srt_connection/mod.rs` にリネームするため、本 issue の対象ファイルの所在が変わりうる。並行実装時は直列に実装する（先後は問わない）
- #0032 (tests/sansio_test.rs のリネーム) は本 issue の完了条件が明記するファイル名を変更するため、本 issue は #0032 の後に実装し、リネーム後のファイル名 (`tests/test_srt_connection.rs`) を参照する

## テスト

コメント修正は挙動に影響しないため、新規テストは追加しない。`cargo test` で全テストが通過すること。

## 完了条件

- 対象外基準に該当しない英語コメントが src/・tests/・pbt/ 配下に残っていないこと（全ソースを grep で確認。「日本語文字を含まないコメント行」と「英字と日本語が混在するコメント行」の両方を抽出し、対象外基準を目視で適用して判定する。tests/ と pbt/ のコメント（例: `// Caller → Listener`、`//! Property-based tests for SRT crypto`）も対象に含む）
- テストのログメッセージ（expect / panic メッセージ等）に英語が残っていないこと（src/・tests/・pbt/ 配下を grep で確認。tests/sansio_test.rs の expect メッセージ 58 箇所と tests/test_crypto.rs の約 5 箇所を含む。本番コードの expect / panic メッセージ（例: crypto.rs の `expect("iterations should be non-zero")`）は「ログメッセージ」に含めず日本語化しない）
- `cargo test` で全テストが通過すること
