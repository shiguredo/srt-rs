# add_hs_extension と add_hs_response の重複を解消する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-deduplicate-hs-extension

## 目的

`src/srt_handshake.rs:358-383` の `add_hs_extension` と `add_hs_response` が同一実装であり、唯一の差異は `ExtensionType::HsReq` / `HsRsp` の値のみ。

## 設計方針

内部メソッドに統合し、`ext_type` を引数で受け取るようにする。

## 完了条件

- 重複が解消されていること
- `cargo test` で全テストが通過すること
