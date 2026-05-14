# unwrap_sek が wrapped.len() < 8 の入力でパニックする

- Priority: High
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-unwrap-sek-short-input-panic

## 目的

`src/crypto.rs:379` の `unwrap_sek` 関数で `wrapped.len() - 8` を計算してアンラップ先バッファサイズを決定している。ピアから 8 バイト未満のラップ済み鍵データが送信された場合、debug ビルドで整数アンダーフローパニックが発生する。攻撃者からの不正データで容易にクラッシュさせられる。

## 優先度根拠

ネットワークから受信したデータでパニックが発生するため、DoS 脆弱性に相当する。ピアからの不正な KM メッセージで容易にトリガー可能。

## 現状

```rust
// src/crypto.rs:372-384
fn unwrap_sek(kek: &[u8], wrapped: &[u8], key_length: KeyLength) -> Result<Vec<u8>, Error> {
    // ...
    let mut unwrapped = vec![0u8; wrapped.len() - 8]; // wrapped.len() < 8 でパニック
    aes_kek
        .unwrap(wrapped, &mut unwrapped)
        // ...
}
```

呼び出し元:
1. `CryptoContext::new_receiver` (line 187) - ハンドシェイク時の SEK アンラップ
2. `CryptoContext::update_sek` (line 328) - KM Refresh 時の SEK 更新

いずれもピアからの受信データを直接 `wrapped` として渡す。

## 設計方針

`wrapped.len() < 8` の場合は `Error::crypto_error` を返す。AES Key Wrap のラップ済みデータは最低でも 8 バイトの integrity check vector を含むため、8 バイト未満のデータは仕様上も不正である。

### 修正対象

1. `src/crypto.rs` の `unwrap_sek` の先頭にサイズチェックを追加する:

```rust
if wrapped.len() < 8 {
    return Err(Error::crypto_error("wrapped key too short"));
}
```

2. `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加する:
   - `[FIX] unwrap_sek で 8 バイト未満の入力によるパニックを修正する`

### テスト戦略

`src/crypto.rs` の `#[cfg(test)] mod tests` に以下の単体テストを追加する:

- `wrapped.len() == 0` の入力で `Err` が返ること
- `wrapped.len() == 7` の入力で `Err` が返ること
- `wrapped.len() == 8` の入力で関数が進行すること（ラップフォーマット不正により後段の `unwrap` でエラーになることは問題ない）

## 完了条件

- `wrapped.len() < 8` の入力で `Error` が返ること
- debug ビルドでパニックしないこと
- 上記のテストが追加されていること
- `cargo test` で全テストが通過すること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること

## 解決方法

1. `src/crypto.rs` の `unwrap_sek` 関数の先頭に `wrapped.len() < 8` のサイズチェックを追加
2. `src/crypto.rs` に `test_unwrap_sek_short_input` テストを追加
3. `CHANGES.md` に `[FIX]` エントリを追加
