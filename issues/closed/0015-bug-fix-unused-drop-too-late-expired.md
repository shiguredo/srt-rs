# drop_too_late と drop_expired が未使用である

- Priority: Low
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-unused-drop-too-late-expired

## 目的

`ReceiverBuffer::drop_too_late` と `SenderBuffer::drop_expired` の 2 つのメソッドが定義されているが、`src/srt_connection.rs` のどのパスからも呼び出されていない。TLPKTDROP（Too-Late Packet Drop）機能は SRT ハンドシェイクの `srt_flags::TLPKTDROP` フラグで有効化されるが、接続コード側でこれらのメソッドを呼び出す実装が存在しない。

## 優先度根拠

TLPKTDROP 機能が実際には動作しておらず、機能が未完成である。ただし TLPKTDROP はオプショナル機能であり、必須ではない。

## 現状

- `ReceiverBuffer::drop_too_late` — `srt_receiver.rs:583-608`、呼び出し 0 件
- `SenderBuffer::drop_expired` — `srt_sender.rs:346-371`、呼び出し 0 件
- `drop_too_late` には #0008 で指摘されている実装上の問題もある

## 設計方針

TLPKTDROP 機能を接続コードに統合する方針を取る。#0008 で `drop_too_late` の実装を修正した後に、ACK タイマー処理でこれらのメソッドを呼び出すようにする。

単純削除は TLPKTDROP 機能を放棄することになるため採用しない。

### 修正対象

1. `src/srt_connection.rs` の定期的なタイマー処理（ACK 送信タイミング等）で `drop_too_late` と `drop_expired` を呼び出す
2. TLPKTDROP フラグが有効な場合のみ実行する

### テスト戦略

`pbt/tests/prop_receiver.rs` および `pbt/tests/prop_sender.rs` にドロップ動作の検証を追加する。

### 依存関係

- #0008: `drop_too_late` の内部実装修正に依存する
- #0004: `TsbpdTimeBase` の修正に依存する

## 完了条件

- `drop_too_late` と `drop_expired` が接続コードから呼び出されていること
- TLPKTDROP フラグ有効時に正しくパケットがドロップされること
- `cargo test` で全テストが通過すること

## 解決方法

1. `src/srt_connection.rs` の ACK タイマー処理（`TimerId::Ack`）で `drop_too_late` と `drop_expired` を呼び出すよう修正
2. TLPKTDROP フラグ設定済みのため、常時実行される
