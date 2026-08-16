# 受信バッファに明示的な上限がない

- Priority: High
- Created: 2026-08-16
- Branch: feature/bug-fix-receiver-buffer-no-limit

## 目的

`src/srt_receiver.rs` の `ReceiverBuffer` 構造体で `packets: BTreeMap<u32, ReceivedPacket>` にサイズ上限がない。`SenderBuffer` には `max_buffer_size` フィールドがあるが、`ReceiverBuffer` には対応する制限が存在しない。攻撃者やバグにより大量のパケットが送りつけられた場合、BTreeMap が無制限に肥大化し、メモリ枯渇を引き起こす可能性がある。

## 現状

```rust
packets: BTreeMap<u32, ReceivedPacket>,
```

`SenderBuffer` 側には `max_buffer_size: u32` が存在するが、`ReceiverBuffer` 側にはない。

## 設計方針

`ReceiverBuffer::new` に `max_buffer_size` パラメータを追加する、または `receive` メソッド内でバッファサイズをチェックし、上限を超えた場合は古いパケットをドロップする。`SenderBuffer` の `max_buffer_size` のように `u32` で管理する。

## 完了条件

- `ReceiverBuffer` にバッファ上限が設定され、超過時にパケットがドロップされること
- `cargo test` で全テストが通過すること
