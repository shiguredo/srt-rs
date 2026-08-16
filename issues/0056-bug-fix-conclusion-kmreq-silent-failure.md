# send_conclusion_request の KMREQ 追加失敗が握り潰される

- Created: 2026-08-16
- Branch: feature/fix-conclusion-kmreq-silent-failure
- Polished: 2026-08-16

## 目的

`src/srt_connection.rs` の `send_conclusion_request` メソッド内で、`wrap_sek` の失敗が `if let Ok(...)` で握り潰されている。暗号化が有効なのに KMREQ 拡張が追加されないと、Listener 側で `"encryption required but no KMREQ"` エラーになるが、`wrap_sek` 失敗の根本原因はログにもエラー伝播にも残らない。

なお、現状のコードパスでは `wrap_sek` は失敗しない (KEK は `derive_kek` が常に `key_length.len()` バイトで生成し、SEK は `CryptoContext::new_sender` が長さを検証済みのため)。そのため本 issue は現に発生するバグの修正ではなく、**防御的エラー処理の一貫性**の確保である。既存の KM refresh 経路 (`provide_new_sek` → `start_pre_announce`) では wrap 失敗が `?` で伝播済みであり、ハンドシェイク経路だけが例外的に握り潰している状態を解消する。

## 現状

```rust
if let Some(ref crypto) = self.crypto
    && let Ok(wrapped_key) = crypto.wrap_sek(crypto.current_key())
{
    let km_message = KmMessage::new(
        crypto.current_key(),
        crypto.key_length(),
        *crypto.salt(),
        wrapped_key,
    );
    hs.add_km_request(&km_message);
}
```

失敗分岐にログ出力もエラー伝播もない。

## 設計方針

`send_conclusion_request` の戻り値を `Result<(), Error>` に変更し、`wrap_sek` のエラーを `?` 演算子で呼び出し元に伝播させる。`tracing::error!` によるログ出力のみの対応は採用しない (エラーを呼び出し元で検知できる必要があるため)。

呼び出し元は `handle_handshake_caller` の INDUCTION 分岐の 1 箇所のみであり、`Result<(), Error>` を返すメソッド内のため `?` で伝播可能。

## 完了条件

- `send_conclusion_request` の戻り値が `Result<(), Error>` に変更され、`wrap_sek` の失敗が `?` 演算子で呼び出し元 (`handle_handshake_caller`) に伝播されること
- `if let Ok(...)` による握り潰しパターンが除去されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること (握り潰しは潜在バグであり、バグ修正として `[FIX]` に分類する)
- `cargo test` で全テストが通過すること

## 解決方法

`src/srt_connection.rs` の `send_conclusion_request` の戻り値を `Result<(), Error>` に変更し、`if let Ok(wrapped_key)` を `?` 演算子に置き換えてエラーを伝播させる。呼び出し元 (`handle_handshake_caller` の INDUCTION 分岐) に `?` を追加する。

`wrap_sek` 失敗パスは公開 API から発生させられない (モック・スタブ禁止のため) ため、失敗パス自体のテストは作成しない。検証はコードレビュー (`if let Ok` パターンの除去) と既存テストの通過による。
