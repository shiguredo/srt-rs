# encode_le_words が UTF-8 バイト境界を無視して切り詰める

- Created: 2026-08-16
- Branch: feature/fix-encode-le-words-utf8-boundary
- Polished: 2026-08-16

## 目的

`src/srt_handshake.rs` の `encode_le_words` 関数内で、`bytes[..len]` によりバイト単位の切り詰めが行われる。`max_len` (512 バイト) の境界がマルチバイト UTF-8 文字の途中に当たると、不完全な UTF-8 シーケンスが生成され、`decode_le_words` で `String::from_utf8` が失敗して `None` を返す。

## 現状

```rust
let len = bytes.len().min(max_len);
let truncated = &bytes[..len];
```

## 設計方針

`floor_char_boundary` を使用して、UTF-8 文字の境界で安全に切り下げる。`s.as_bytes()` のスライスに対して `floor_char_boundary` を呼び出し、マルチバイト文字の途中で切れないようにする。

## 完了条件

- 512 バイト境界がマルチバイト文字の途中に当たる場合でも、不完全な UTF-8 シーケンスが生成されないこと
- マルチバイト文字の境界テストが追加されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/srt_handshake.rs` の `encode_le_words` 関数内で、`bytes[..len]` の代わりに `bytes[..bytes.floor_char_boundary(len)]` を使用して UTF-8 文字境界で切り詰める。テストは `tests/test_srt_handshake.rs` に追加し、マルチバイト文字が境界にかかるケースを検証する。
