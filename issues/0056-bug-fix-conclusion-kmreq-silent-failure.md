# send_conclusion_request の KMREQ 追加失敗が握り潰される

- Created: 2026-08-16
- Branch: feature/fix-conclusion-kmreq-silent-failure
- Polished: 2026-08-16

## 目的

`src/srt_connection.rs` の `send_conclusion_request` メソッド内で、`wrap_sek` の失敗が `if let Ok(...)` で握り潰されている。暗号化が有効なのに KMREQ 拡張が追加されないと、Listener 側で `"encryption required but no KMREQ"` エラーになるが、`wrap_sek` 失敗の根本原因はログに残らず、デバッグが困難になる。

## 現状

```rust
if let Some(ref crypto) = self.crypto
    && let Ok(wrapped_key) = crypto.wrap_sek(crypto.current_key())
{
    let km_message = KmMessage::new(...);
    hs.add_km_request(&km_message);
}
```

## 設計方針

`wrap_sek` の失敗を `Result` で伝播させるか、少なくとも `tracing::error!` でログに出力する。`send_conclusion_request` の戻り値を `Result<(), Error>` に変更し、`wrap_sek` のエラーを呼び出し元に伝播させる。

## 完了条件

- `wrap_sek` 失敗時にエラーが伝播されるか、またはログに出力されること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/srt_connection.rs` の `send_conclusion_request` メソッドの戻り値を `Result<(), Error>` に変更し、`wrap_sek` の失敗を `?` 演算子で呼び出し元に伝播させる。`send_conclusion_request` の呼び出し元 (`handle_handshake_caller` の INDUCTION 分岐、および `handle_handshake_listener` の CONCLUSION 分岐) でエラーをハンドリングする。
