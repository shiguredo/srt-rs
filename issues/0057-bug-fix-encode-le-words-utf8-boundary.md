# encode_le_words が UTF-8 バイト境界を無視して切り詰める

- Created: 2026-08-16
- Branch: feature/fix-encode-le-words-utf8-boundary
- Polished: 2026-08-16

## 目的

`src/srt_handshake.rs` の `encode_le_words` 関数内で、`bytes[..len]` によりバイト単位の切り詰めが行われる。`max_len` (512 バイト) の境界がマルチバイト UTF-8 文字の途中に当たると、不完全な UTF-8 シーケンスが生成され、`decode_le_words` で `String::from_utf8` が失敗して `None` を返す。

この結果、Listener 側で `peer_stream_id` が設定されず、Stream ID 情報が失われる (`src/srt_connection.rs` のハンドシェイク処理で `decode_le_words` の戻り値を使用する箇所)。

なお、512 バイト上限は SRT 仕様に由来する (`refs/srt/draft-sharabayko-srt.md` の「The maximum allowed size of the StreamID extension is 512 bytes」)。

## 現状

```rust
let len = bytes.len().min(max_len);
let truncated = &bytes[..len];
```

## 設計方針

`str::floor_char_boundary` を使用して、UTF-8 文字の境界で安全に切り下げる。`s.floor_char_boundary(len)` を呼び出し、マルチバイト文字の途中で切れないようにする。`floor_char_boundary` は Rust 1.85 で安定化されており、MSRV 1.93 で利用可能である。

## 完了条件

- 512 バイト境界がマルチバイト文字の途中に当たる場合でも、不完全な UTF-8 シーケンスが生成されないこと
- マルチバイト文字の境界テストが追加されていること (例: `"あ"` × 171 = 513 バイトで 512 境界に当たる文字列、512 バイトちょうどの ASCII 文字列)
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/srt_handshake.rs` の `encode_le_words` 関数内で、`bytes[..len]` の代わりに `bytes[..bytes.floor_char_boundary(len)]` を使用して UTF-8 文字境界で切り詰める。

テストは `src/srt_handshake.rs` 内の `#[cfg(test)]` モジュールに追加し、`encode_le_words` (private 関数) を直接呼んで境界ケースを検証する。
