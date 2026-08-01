# SYN Cookie のフォールバック値が 0 で Cookie 検証が実質無効

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-syn-cookie-default
- Polished: 2026-07-31

## 目的

`src/srt_connection.rs` の `handle_handshake_listener()` で `self.syn_cookie = self.options.syn_cookie.unwrap_or(0)` としており、`syn_cookie` 未設定時はフォールバック値の 0 が Cookie として使われる。CONCLUSION リクエストの検証 `hs.syn_cookie != self.syn_cookie` は、正当なフローでは `0 != 0` で常に通過し、ハンドシェイク Cookie の DoS 防御機能が実質無効になっている。

## 優先度根拠

Cookie の値 (0) は攻撃者に既知であり、INDUCTION レスポンスを受信しなくても CONCLUSION リクエストを偽装でき、検証を通過して接続確立処理 (バッファ初期化・タイマー設定等) に到達するため、サービス拒否攻撃のリスクがある。修正後も乱数 Cookie は IP にバインドされないため、ソース IP を偽装する攻撃者は実 IP での INDUCTION 交換で取得した Cookie を偽装 CONCLUSION に流用できる。Cookie は SrtConnection (Listener) の生成ごとに新しくなるが、プロセス生存中に SrtConnection を使い回す利用では固定のままである。リスクの緩和度は利用方法に依存するが、サービス拒否攻撃の主目的であるリソース消費は接続確立ごとに限られるため Medium。

## 現状

`handle_handshake_listener()` 内で、INDUCTION 受信時に Cookie を設定している:

```rust
self.syn_cookie = self.options.syn_cookie.unwrap_or(0);
```

CONCLUSION 受信時の Cookie 検証:

```rust
if hs.syn_cookie != self.syn_cookie {
    return Err(Error::handshake_rejected("invalid SYN cookie"));
}
```

`ConnectionOptions` の Default は `syn_cookie: None` のため、未設定時は常に Cookie=0 で送受信され、検証を通過する。Caller 側は INDUCTION レスポンスの Cookie をそのまま CONCLUSION リクエストに使う (srt_connection.rs の `handle_handshake_caller` 内の `self.syn_cookie = hs.syn_cookie`)。

## 根拠

draft-sharabayko-srt.md の `#caller-listener-handshake` 節 (「The Induction Phase」):

> The INDUCTION phase serves only to set a cookie on the Listener so that it doesn't allocate resources, thus mitigating a potential DoS attack that might be perpetrated by flooding the Listener with handshake commands.

同節 (「The Induction Response」):

> SYN Cookie: a cookie that is crafted based on host, port and current time with 1 minute accuracy to avoid SYN flooding attack {{RFC4987}}.

仕様は Cookie を host / port / 現在時刻に基づいて生成するとしているが、詳細なアルゴリズムは規定していない。本 issue は暗号学的乱数による生成で代替する設計判断である。

## 設計方針

- `syn_cookie` 未設定時は暗号学的乱数 (aws_lc_rs::rand::fill) で生成する。aws-lc-rs は既存の依存であり、新規依存は不要。生成失敗時は panic させる (examples/srt_listener の `rand_u32` と同様に `.expect()` を使用する)
- Cookie は `SrtConnection` (Listener) の生成時に 1 回生成する。`SrtConnection` は接続ごとに生成される API 設計 (srt_connection.rs の `new_listener`) であり、生成ごとに新しい Cookie になる。ただし例のようにプロセス起動時に 1 回しか生成しない利用では、Cookie はプロセス生存中固定になる (アプリの利用方法に依存する)。INDUCTION 受信ごとに再生成すると、同一接続の INDUCTION 再送 (UDP の重複配信等) の後に前の Cookie で CONCLUSION を送った場合に検証が失敗するため。現在の実装は INDUCTION 受信のたびに設定している
- `ConnectionOptions` へのテストモード追加は行わない。テストで固定値を使いたい場合は既存の `syn_cookie: Some(固定値)` を設定する。Caller は INDUCTION レスポンスの Cookie をそのまま CONCLUSION に使うため、既存テストは乱数化後も通過する
- Cookie が一致しない CONCLUSION の拒否テストは本 issue で実施する。0035 に含まれる「SYN Cookie 不一致」のテスト項目は本 issue に移管する (0035 の該当項目は削除)。実装順は本 issue → 0035
- 0027 (`handle_handshake_listener` を含む `handle_handshake_*` の分割リファクタ) は本 issue の後に実装する
- examples/srt_listener は既に `syn_cookie: Some(rand_u32())` で乱数 Cookie を設定しているが、本 issue の修正後もそのまま動作する (変更不要)

## 完了条件

- `syn_cookie` 未設定時に 0 以外の値が Cookie として使われること (INDUCTION レスポンスをデコードして `syn_cookie` フィールドを確認する等。乱数のため、複数接続で異なる値になることも確認する)
- Cookie が一致しない CONCLUSION リクエストが拒否されることを検証するテストが追加されていること
- 0035 の「SYN Cookie 不一致」のテスト項目が削除されていること
- `cargo test` で全テストが通過すること
- CHANGES.md の `## develop` セクションに `[FIX]` エントリが追加されていること
