# lib.rs で内部実装詳細が過剰に公開 API として露出している

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-reduce-public-api

## 目的

`src/lib.rs` の `pub use` で以下の内部実装詳細が公開 API として露出しており、将来の内部リファクタリングが破壊的変更になる:

- `extension_flags` (定数群)
- `srt_flags` (定数群)
- `HsExtensionData` (内部構造体)
- `KmError` (内部エラー型)
- `DEFAULT_FLOW_WINDOW`, `DEFAULT_MTU`, `HS_VERSION_4`, `HS_VERSION_5` (定数)

## 現状

```rust
pub use srt_handshake::{
    DEFAULT_FLOW_WINDOW, DEFAULT_MTU, ExtensionType, HS_VERSION_4, HS_VERSION_5,
    HandshakeExtension, HandshakePacket, HandshakeState, HandshakeType, HsExtensionData, KmError,
    KmMessage, extension_flags, srt_flags,
};
```

## 設計方針

- `DEFAULT_*`, `HS_VERSION_*` は `ConnectionOptions` のデフォルト値に内包する
- `extension_flags`, `srt_flags` は `pub(crate)` に絞る
- `HsExtensionData`, `KmError` は必要最小限のアクセサのみ公開する

## 完了条件

- 公開 API から内部実装詳細が除去されていること
- `cargo test` で全テストが通過すること
