# srt_handshake の内部実装詳細が過剰に公開 API として露出している

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/change-reduce-public-api
- Polished: 2026-08-01

## 目的

`src/lib.rs` の `pub use` で以下の内部実装詳細が公開 API として露出しており、将来の内部リファクタリングが破壊的変更になる:

- `extension_flags` (定数モジュール)
- `srt_flags` (定数モジュール)
- `HsExtensionData` (内部構造体)
- `DEFAULT_FLOW_WINDOW`, `DEFAULT_MTU`, `HS_VERSION_4`, `HS_VERSION_5` (定数)

## 優先度根拠

公開 API の縮小は後方互換のない変更であるが、内部実装詳細の露出を放置すると将来のリファクタリングが全て破壊的変更になる。早期に対処する。

## 現状

```rust
pub use srt_handshake::{
    DEFAULT_FLOW_WINDOW, DEFAULT_MTU, ExtensionType, HS_VERSION_4, HS_VERSION_5,
    HandshakeExtension, HandshakePacket, HandshakeState, HandshakeType, HsExtensionData, KmError,
    KmMessage, extension_flags, srt_flags,
};
```

`srt_flags` は srt_handshake.rs 内 (インラインテスト) と srt_connection.rs 内 (送信メソッド) で、`extension_flags` は srt_handshake.rs 内でのみ使用され、外部クレート (pbt / crates/c-api / examples / fuzz) からの使用はない。`DEFAULT_FLOW_WINDOW` は srt_handshake.rs 内と srt_connection.rs の `init_buffers` で、`DEFAULT_MTU` / `HS_VERSION_4` / `HS_VERSION_5` は srt_handshake.rs 内でのみ使用される (いずれも外部使用なし)。

## 設計方針

1. `extension_flags` / `srt_flags` を `pub(crate)` に絞る (外部使用なし)
2. `DEFAULT_FLOW_WINDOW` / `DEFAULT_MTU` / `HS_VERSION_4` / `HS_VERSION_5` を `pub(crate)` に絞る (外部使用なし)。`HS_VERSION_4` / `HS_VERSION_5` はハンドシェイクのプロトコル固定値 (INDUCTION リクエストは 4、INDUCTION レスポンスと CONCLUSION は 5) で、同一接続内で使い分けられるため `ConnectionOptions` のデフォルト値には内包できない。`ConnectionOptions` に mtu / flow_window フィールドも存在しないため、`pub(crate)` 化で露出のみを止める
3. `HsExtensionData` のフィールド (4 つ) を非公開にし、アクセサ 4 つ (`srt_version()` / `srt_flags()` / `recv_tsbpd_delay()` / `send_tsbpd_delay()`) を公開する。型自体は公開メソッド `get_hs_extension()` の戻り値型であるため公開 API に残す
4. `KmError` は除去対象から外す。公開メソッド `get_km_response()` / `add_km_error()` のシグネチャに現れるエラー型であり、pbt が variant と `from_u32` を直接使用しているため、公開 API として必要
5. 公開 API に残るシンボル (`KmMessage` / `ExtensionType` / `HandshakeExtension` / `HandshakePacket` / `HandshakeState` / `HandshakeType`) は pbt / fuzz が使用しているため変更しない。`SRT_MAGIC_CODE` / `cipher_type` / `stream_encapsulation` (srt_handshake.rs 内の pub だが re-export されていない項目) は本 issue のスコープ外とする

## 相互作用

- pbt (workspace メンバー) の `pbt/tests/prop_handshake.rs` は `HsExtensionData` のフィールドに直接アクセスしている (8 箇所) ため、アクセサ化に合わせてアクセサ経由に書き換える
- #0031 (インラインテストの `tests/` への移動) は srt_handshake.rs のインラインテストを移動対象とし、そのテストは `srt_flags` と `HsExtensionData` のフィールドに直接アクセスしている。どちらの順序で実装してもアクセス経路の調整が必要になる（本 issue を先に実装すると #0031 の移動先 (tests/ の外部テスト) から `pub(crate)` アイテムにアクセスできず、#0031 を先に実装すると本 issue の非公開化で移動済みテストがコンパイルエラーになる）ため、実装時に調整方針を決めてから着手する
- #0033 (PBT の任意入力パニック耐性テストの fuzz 移行) も pbt を変更対象とするため、並行実装時は直列に実装する (先後は問わない)

## テスト

機能不変の変更 (可視性の縮小とフィールドのカプセル化) のため、新規テストは追加しない。挙動不変は既存の `cargo test` (pbt を含む) で担保する。pbt のフィールドアクセスはアクセサ経由に書き換える。

## CHANGES.md

公開 API の後方互換のない変更であるため、`[CHANGE]` エントリ (例: `[CHANGE] srt_handshake の内部実装詳細 (extension_flags / srt_flags / DEFAULT_FLOW_WINDOW / DEFAULT_MTU / HS_VERSION_4 / HS_VERSION_5) を公開 API から除去し、HsExtensionData のフィールドを非公開化する`) を追加する。

## 完了条件

- lib.rs の `pub use srt_handshake::` から `extension_flags` / `srt_flags` / `DEFAULT_FLOW_WINDOW` / `DEFAULT_MTU` / `HS_VERSION_4` / `HS_VERSION_5` が除去されていること
- srt_handshake.rs 内の該当アイテム（`extension_flags` / `srt_flags` / `DEFAULT_FLOW_WINDOW` / `DEFAULT_MTU` / `HS_VERSION_4` / `HS_VERSION_5`）が `pub(crate)` に変更されていること
- `HsExtensionData` のフィールドが非公開で、アクセサ 4 つ (`srt_version()` / `srt_flags()` / `recv_tsbpd_delay()` / `send_tsbpd_delay()`) が公開されていること
- pbt のフィールドアクセスがアクセサ経由に書き換えられていること
- `cargo test` で全テスト (pbt を含む) が通過すること
- `cargo build --workspace` と `cargo clippy --workspace --all-targets -- -D warnings` が通過すること
- CHANGES.md の `## develop` セクションに `[CHANGE]` エントリが追加されていること
