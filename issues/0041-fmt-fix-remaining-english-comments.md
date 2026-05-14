# 英語コメントの残存箇所を日本語に修正する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fmt-fix-remaining-english-comments

## 目的

以下の 3 箇所で英語のドキュメントコメントが残っている:

1. `src/crypto.rs:23` — `/// AES-128 (16 bytes)`
2. `src/crypto.rs:25` — `/// AES-256 (32 bytes)`
3. `src/srt_connection.rs:1` — `//! SRT Connection (sansio パターン)` に英語混在

AGENTS.md の「コメントは全て日本語にすること」に違反している。

## 完了条件

- 上記 3 箇所が日本語に修正されていること
- `cargo test` で全テストが通過すること
