# sans-io パターン違反: debug オプションが直接 eprintln! を出力している

- Priority: High
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-sansio-debug-eprintln

## 目的

`srt_connection.rs` の 7 箇所で `self.options.debug` が true の場合に `eprintln!` で直接 stderr に書き込んでいる。sans-io パターンは全出力を `output_queue` 経由で外部に委ねるべきであり、これは根本的契約違反である。

## 優先度根拠

sans-io パターンの核心的契約違反。埋め込み環境や WASM など stderr が存在しない環境での動作を不可能にし、テスト容易性・移植性を損なう。

## 現状

以下の 7 箇所で `eprintln!` が直接呼ばれている:

- `src/srt_connection.rs:349` — DATA パケット受信ログ
- `src/srt_connection.rs:494` — send メソッドログ
- `src/srt_connection.rs:686` — 制御パケット受信ログ
- `src/srt_connection.rs:729` — INDUCTION レスポンスログ
- `src/srt_connection.rs:762` — CONCLUSION レスポンスログ
- `src/srt_connection.rs:905` — ACK 受信ログ
- `src/srt_connection.rs:1238` — パケット送信ログ

## 設計方針

デバッグ情報を `ConnectionEvent` の新しいバリアント（例: `ConnectionEvent::Debug(String)`）としてイベントキューに流す。呼び出し側が出力先を決定できるようにする。

## 完了条件

- `eprintln!` が削除されていること
- デバッグ情報が `ConnectionEvent` 経由で外部に渡されていること
- `cargo test` で全テストが通過すること
