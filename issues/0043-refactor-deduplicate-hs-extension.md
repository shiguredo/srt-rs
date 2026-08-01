# add_hs_extension と add_hs_response の重複を解消する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-deduplicate-hs-extension
- Polished: 2026-08-01

## 目的

`src/srt_handshake.rs` の `add_hs_extension` と `add_hs_response` が同一実装であり、機能的差異は `ExtensionType::HsReq` / `HsRsp` の値のみ (コメントと doc コメントに差異があるが、機能は同一)。

## 設計方針

- 内部メソッド (例: `fn add_hs_extension_internal(&mut self, ext_type: ExtensionType, srt_version: u32, srt_flags: u32, tsbpd_delay: u16)`) を private で追加し、実装本体はこの内部メソッドに 1 つだけ置く。doc コメントは「HSREQ/HSRSP 拡張を追加」のように両対応の文言にする
- 既存の公開メソッド `add_hs_extension` と `add_hs_response` は、内部メソッドへの委譲 (1 行) として残す。両メソッドは公開 API であり、pbt (独立クレート) が `use shiguredo_srt::` 経由で使用しているため削除しない (後方互換の維持)
- 呼び出し元は変更不要 (公開メソッドのシグネチャが不変のため)
- `add_hs_extension` にのみ存在する `// Receiver TSBPD delay` / `// Sender TSBPD delay` のコメントを統合メソッドに引き継ぐ (引き継ぐ際に日本語化する。このコメントは #0041 の英語コメント日本語化の対象でもある)

## 相互作用

- #0027 (srt_connection.rs の分割) は呼び出し元 (`send_conclusion_request` / `send_conclusion_response`) を send.rs に移動するが、公開メソッドのシグネチャが不変のため競合しない
- #0040 (デッドコード削除) と #0041 (英語コメント修正) は同じ srt_handshake.rs を変更するが、本 issue の対象 (add_hs_extension / add_hs_response) とは競合しない。ただし、#0041 は本 issue が引き継ぐ `// Receiver TSBPD delay` / `// Sender TSBPD delay` コメントの日本語化も対象とするため、#0041 を先に実装した場合は日本語化済みのコメントを引き継ぐ。#0041 側の相互作用にも本 issue との関係を追記すること
- #0031 (インラインテストの `tests/` への移動) は open のままである。本 issue のテスト追加場所 (src/srt_handshake.rs の `#[cfg(test)]`) に影響しうるため、並行実装時は直列に実装する

## テスト

- 既存のテスト (srt_connection.rs 経由の呼び出しと pbt の roundtrip) で挙動を担保する
- 統合時に `HsReq` / `HsRsp` の取り違えを検出するため、`src/srt_handshake.rs` の `#[cfg(test)]` モジュールに、encode 後の拡張の `ext_type` が期待どおりであることを検証するテストを追加する。`add_hs_extension` で `HsReq`、`add_hs_response` で `HsRsp` になることの両方向を検証する (既存の roundtrip テストは `get_hs_extension()` が両方の ext_type を受け入れるため、取り違えを検出できない。委譲後の取り違えは両委譲行どちらの typo でも起きるため、片方だけの検証では不十分)。#0031 が先に実装された場合は、`tests/test_srt_handshake.rs` に置く (公開 API のみで書ける)

## CHANGES.md

機能に直接影響しない変更 (後方互換を壊さない) のため、shiguredo-changelog 規約に従い `### misc` サブセクションに `[UPDATE]` エントリ (例: `[UPDATE] add_hs_extension / add_hs_response の重複実装を統合する`。担当者行を付けて追加すること) を追加する。

## 完了条件

- `add_hs_extension` と `add_hs_response` の実装本体が 1 つの内部メソッドに統合され、公開メソッド 2 つが委譲になっていること
- 公開メソッドのシグネチャが不変であること (後方互換の維持)
- 統合メソッドに `// Receiver TSBPD delay` / `// Sender TSBPD delay` のコメントが日本語化された形で引き継がれていること
- `ext_type` の取り違えを検出するテスト (両方向) が追加されていること
- `cargo test --workspace` で全テスト (pbt を含む) が通過すること
- `cargo clippy --workspace --all-targets -- -D warnings` が通過すること
- CHANGES.md の `### misc` セクションに `[UPDATE]` エントリが追加されていること
