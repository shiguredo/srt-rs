# SRT_CMD_KMREQ と SRT_CMD_KMRSP の定数が重複定義されている

- Created: 2026-08-16
- Branch: feature/refactor-deduplicate-srt-cmd-km-constants
- Polished: 2026-08-16

## 目的

`src/srt_connection.rs` 内で、`SRT_CMD_KMREQ` (= 3) と `SRT_CMD_KMRSP` (= 4) の定数が `handle_user_defined`、`send_km_request`、`send_km_response` の関数内で重複定義されている (`SRT_CMD_KMREQ` が 2 箇所、`SRT_CMD_KMRSP` が 2 箇所)。同一の値を複数箇所で定義しているため、値の変更が必要な場合に修正漏れが発生するリスクがある。

なお、値 3 と 4 は既に `src/srt_handshake.rs` の `ExtensionType::KmReq` / `ExtensionType::KmRsp` (`#[repr(u16)]` 付き enum) として定義されており、SRT 仕様上も UserDefined 制御パケットの subtype はハンドシェイク拡張タイプと同一の値空間である (`refs/srt/draft-sharabayko-srt.md` の `{{sec-ctrlpkt-km}}` 節)。

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

新規にモジュールレベル定数を作るのではなく、既存の `ExtensionType::KmReq` / `ExtensionType::KmRsp` を `as u16` で流用する。これにより値 3/4 の定義が `ExtensionType` の 1 箇所に集約され、修正漏れリスクが解消される。仕様上も UserDefined subtype は handshake-ext-type の値空間を参照するため、仕様整合的である。

`src/srt_connection.rs` の `use crate::srt_handshake::{...}` に `ExtensionType` を追加し、関数内の `SRT_CMD_KMREQ` / `SRT_CMD_KMRSP` 定義を削除して、使用箇所を `ExtensionType::KmReq as u16` / `ExtensionType::KmRsp as u16` に置き換える。

## 完了条件

- `handle_user_defined`、`send_km_request`、`send_km_response` 内の `SRT_CMD_KMREQ` / `SRT_CMD_KMRSP` の関数内定義がすべて除去されていること
- 使用箇所が `ExtensionType::KmReq as u16` / `ExtensionType::KmRsp as u16` に置き換えられていること
- `SRT_CMD_KMREQ` / `SRT_CMD_KMRSP` の定数定義が `src/srt_connection.rs` からすべて除去され、値の根拠が `src/srt_handshake.rs` の `ExtensionType` の enum 定義に一元化されていること (`src/srt_connection.rs` の `// SRT_CMD_KMREQ = 3, SRT_CMD_KMRSP = 4` コメントも含めて除去・更新されていること)
- `CHANGES.md` の `## develop` セクションの `misc` に `[UPDATE]` エントリが追加されていること (例: `[UPDATE] SRT_CMD_KMREQ / SRT_CMD_KMRSP の定数定義を ExtensionType に一元化する`。担当者行を付けて追加すること)
- `cargo test --workspace` で全テストが通過すること
- `cargo clippy --workspace --all-targets -- -D warnings` が通過すること

## 解決方法

`src/srt_connection.rs` の `use crate::srt_handshake::{...}` に `ExtensionType` を追加する。`handle_user_defined`、`send_km_request`、`send_km_response` 内の `const SRT_CMD_KMREQ: u16 = 3;` / `const SRT_CMD_KMRSP: u16 = 4;` を削除し、使用箇所を `ExtensionType::KmReq as u16` / `ExtensionType::KmRsp as u16` に置き換える。`// SRT_CMD_KMREQ = 3, SRT_CMD_KMRSP = 4` コメントは、除去するか `ExtensionType` への参照コメントに置き換える。

挙動不変のリファクタリングであり、新規テストは不要。UserDefined パス (subtype 3/4) を直接テストする既存テストは存在しないが、定数の差し替えのみで挙動は不変であるため、既存のテスト (`tests/test_srt_connection.rs` の暗号化ハンドシェイクテスト、`src/crypto.rs` の KM Refresh 単体テスト) の通過で担保する。

なお、0027 (srt_connection の分割) は `send_km_request` / `send_km_response` を移動対象に含むため、並行実装時は競合に注意する (先後は問わない)。
