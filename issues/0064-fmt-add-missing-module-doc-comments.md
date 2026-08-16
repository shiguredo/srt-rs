# lib.rs / error.rs / buf.rs / time.rs に //! モジュールドキュメントが不足している

- Priority: High
- Created: 2026-08-16
- Branch: feature/fmt-add-missing-module-doc-comments

## 目的

shiguredo-rust 規約「`src/<module>.rs` の先頭にそのモジュールの責務を 1〜2 行で書くこと」に違反するモジュールが存在する。以下のモジュールに `//!` モジュールドキュメントが不足している。

## 現状

`//!` が不足しているモジュール:

- `src/lib.rs`: クレートレベルのドキュメント (docs.rs のトップページに表示される)
- `src/error.rs`: エラー型モジュール
- `src/buf.rs`: バイト列読み書きユーティリティモジュール
- `src/time.rs`: タイムスタンプ型モジュール

他のモジュール (`crypto.rs`、`srt_handshake.rs`、`srt_connection.rs`、`srt_receiver.rs`、`srt_sender.rs`、`srt_packet.rs`、`stream_id.rs`) は `//!` が記述されている。

## 設計方針

各モジュールの先頭に `//!` でモジュールの責務を 1〜2 行で書く。

## 完了条件

- 上記の全モジュールに `//!` が追加されていること
- `cargo doc` で警告が発生しないこと
