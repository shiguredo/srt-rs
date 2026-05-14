# LIBSRT_COMPAT_PADDING が毎回 to_vec() でクローンされている

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-avoid-padding-clone

## 目的

`src/srt_connection.rs:1192`（ackack）、`src/srt_connection.rs:1344`（keepalive）、`src/srt_connection.rs:1366`（shutdown）の 3 箇所で、同一の 4 バイト配列 `LIBSRT_COMPAT_PADDING` を毎回 `to_vec()` でクローンしている。

## 設計方針

`&[u8]` のまま `control_info` に渡せるよう、`ControlPacket::control_info` を `Cow<[u8]>` にするか、定数を `&[u8]` で保持する。

## 完了条件

- 不要な allocation が削減されていること
- `cargo test` で全テストが通過すること
