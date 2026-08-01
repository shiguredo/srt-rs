# srt_connection.rs が 1509 行で過大 — モジュール分割が必要

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-split-srt-connection
- Polished: 2026-08-01

## 目的

`src/srt_connection.rs` が 1509 行あり、`SrtConnection` の状態遷移・送受信処理・ハンドシェイク処理・NAK エンコーディング・単体テストが 1 ファイルに詰め込まれている。可読性・保守性の低下が著しく、新規機能追加時の変更影響範囲が広くなる。

## 優先度根拠

機能には影響しないが、可読性・保守性の低下が著しい。新規機能追加時の変更影響範囲が広くなる。

## 現状

以下の責務が 1 ファイルに混在している:

- `parse_loss_list` / `encode_loss_list` — NAK の損失リストとワイヤ形式の変換。使用箇所は `handle_nak`（受信 NAK の損失リスト化）と `send_nak`（損失リストの送信）。損失リスト変換の単体テストは同ファイル内に 4 件
- `send_*` メソッド群 — 12 個の送信メソッド (`send_km_request` / `send_km_response` / `send_ack` / `send_nak` / `send_periodic_nak` / `send_ackack` / `send_induction_request` / `send_induction_response` / `send_conclusion_request` / `send_conclusion_response` / `send_keepalive` / `send_shutdown`)
- ハンドシェイク処理 — `handle_handshake`（ディスパッチ）と `handle_handshake_caller` / `handle_handshake_listener`。`send_induction_request` / `send_induction_response` / `send_conclusion_request` / `send_conclusion_response` はハンドシェイク中に呼ばれる送信メソッド

## 設計方針

3 分割をすべて実施する:

1. `parse_loss_list` / `encode_loss_list` を `srt_receiver.rs` に移動する。損失リストは NAK の概念であり、`srt_receiver.rs` は「受信パケットの並べ替えと ACK/NAK 生成を管理する」モジュールとして NAK ペイロードの変換を責務に含む。移動後は `handle_nak`（`mod.rs` に残る）と `send_nak`（`send.rs` に移る）の両方から呼ばれるため、可視性は `pub(crate)` にする。単体テスト 4 件も追随して移動する（shiguredo-rust 規約: 単体テストは対応モジュール内に置く）
2. 送信メソッド 12 個すべてを `src/srt_connection/send.rs` に分割する。`src/srt_connection.rs` を `src/srt_connection/mod.rs` にリネームし、`mod.rs` に `mod send;` / `mod handshake;` を宣言したうえで、`send.rs` に `impl SrtConnection` ブロックを置く。`SrtConnection` のフィールドは `mod.rs` で private のままでよい（Rust のプライバシー規則により子モジュールからアクセス可能。`lib.rs` の `mod srt_connection;` と re-export は無変更）
3. ハンドシェイク処理を `src/srt_connection/handshake.rs` に分割する。移動対象は `handle_handshake` / `handle_handshake_caller` / `handle_handshake_listener` のみとし、ハンドシェイク用の送信メソッド（`send_induction_request` / `send_induction_response` / `send_conclusion_request` / `send_conclusion_response`）は 2 の `send.rs` に含める（送信メソッドの帰属を `send.rs` に一本化し、分割境界の重複を避ける）

分割後も `mod.rs` に約 1000 行が残る見込みであり、本 issue は分割の第一歩として位置づける（残る状態遷移・受信処理のさらなる分割は必要に応じて別 issue で行う）。

## 相互作用

- #0025（SYN cookie のデフォルト値修正）が `handle_handshake_listener` を変更するため、本 issue は #0025 の後に実装する
- #0028（`srt_receiver.rs` の `receive()` 分割）も `srt_receiver.rs` を変更対象とするため、並行実装時は同一ファイルのコンフリクトを避けるために直列に実装する（先後は問わない）
- #0031（インラインテストの `tests/` への移動）は本 issue が `srt_receiver.rs` へ移動する `test_loss_list_encode_decode_*` を含むため、本 issue を先に実装する（#0031 の移動対象は移動後に残るインラインテストになり、本 issue で移動したテスト 4 件は `srt_receiver.rs` のテストモジュールの一部として #0031 の対象に含まれる）。#0032（`tests/sansio_test.rs` のリネーム）も本 issue のテストセクションが言及するファイルを対象とするため、本 issue の後に実装する
- #0037（fuzz ターゲット追加）は `parse_loss_list` / `encode_loss_list` を対象に含むが、`fuzz/` は独立クレート（`Cargo.toml` の `exclude`）のため `pub(crate)` 関数にはアクセスできない。本 issue で `pub(crate)` 化する前に #0037 を実装するか、#0037 実装時に公開パスを確保するか、どちらかの順序判断が必要である
- #0040（`SRT_CMD_KMREQ` / `SRT_CMD_KMRSP` の定数重複除去）と #0047（`LIBSRT_COMPAT_PADDING` のクローン除去）は本 issue が移動する送信メソッド群と同じ箇所を変更対象とするが、いずれの順序でも実質競合しない

## テスト

機能不変のリファクタリングのため、新規テストは追加しない。挙動不変は既存の `tests/sansio_test.rs` と `pbt/` のテストで担保する。損失リスト変換の単体テスト 4 件（`test_loss_list_encode_decode_single` / `test_loss_list_encode_decode_range` / `test_loss_list_encode_decode_mixed` / `test_loss_list_encode_empty`）は移動先モジュール（`srt_receiver.rs`）へ追随する。

## CHANGES.md

機能に直接影響しない内部構造の変更のため、shiguredo-changelog 規約に従い `### misc` サブセクションに `[CHANGE]` エントリ（例: `[CHANGE] srt_connection.rs を srt_connection/ モジュールに分割する`）を追加する。

## 完了条件

- `parse_loss_list` / `encode_loss_list` と損失リスト変換の単体テスト 4 件が `srt_receiver.rs` に移動されていること
- 送信メソッド 12 個が `src/srt_connection/send.rs` に、ハンドシェイク処理（`handle_handshake` / `handle_handshake_caller` / `handle_handshake_listener`）が `src/srt_connection/handshake.rs` に移動されていること
- `src/srt_connection.rs` が `src/srt_connection/mod.rs` にリネームされ、`mod.rs` に `mod send;` / `mod handshake;` が宣言され、`SrtConnection` の公開 API（pub メソッド）が不変であること
- 分割後のモジュール間に循環依存がないこと（コードレビューで確認する）
- `cargo test` で全テストが通過すること
- `cargo build --workspace` と `cargo clippy --workspace --all-targets -- -D warnings` が通過すること
- CHANGES.md の `### misc` セクションに `[CHANGE]` エントリが追加されていること
