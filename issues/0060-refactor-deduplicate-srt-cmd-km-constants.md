# SRT_CMD_KMREQ と SRT_CMD_KMRSP の定数が重複定義されている

- Priority: High
- Created: 2026-08-16
- Branch: feature/refactor-deduplicate-srt-cmd-km-constants

## 目的

`src/srt_connection.rs` 内で、`SRT_CMD_KMREQ` (= 3) と `SRT_CMD_KMRSP` (= 4) の定数が `handle_user_defined`、`send_km_request`、`send_km_response` の 3 箇所で重複定義されている。同一の定数を 3 箇所で定義しているため、値の変更が必要な場合に修正漏れが発生するリスクがある。

## 現状

```rust
// handle_user_defined 内
const SRT_CMD_KMREQ: u16 = 3;
const SRT_CMD_KMRSP: u16 = 4;

// send_km_request 内
const SRT_CMD_KMREQ: u16 = 3;

// send_km_response 内
const SRT_CMD_KMRSP: u16 = 4;
```

## 設計方針

モジュールレベルの定数として 1 箇所にまとめる。`impl SrtConnection` ブロックの外側、`INACTIVITY_TIMEOUT_MICROS` や `LIBSRT_COMPAT_PADDING` と同列に定義する。

## 完了条件

- `SRT_CMD_KMREQ` と `SRT_CMD_KMRSP` がモジュールレベルの定数として 1 箇所に定義されていること
- `cargo test` で全テストが通過すること
