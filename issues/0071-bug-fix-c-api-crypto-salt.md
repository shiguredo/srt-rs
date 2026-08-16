# C API の SrtConnectionOptions に crypto_salt を渡す手段がない

- Created: 2026-08-16
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-c-api-crypto-salt
- Polished: {YYYY-MM-DD}

## 目的

`crates/c-api/src/lib.rs` の `SrtConnectionOptions` は `passphrase` フィールドを持つが `crypto_salt` 相当のフィールドを持たないため、`srt_connection_new_caller` / `srt_connection_new_listener` は `crypto_salt` が `None` のまま `SrtConnection` へ渡す。

`src/srt_connection.rs` の `ConnectionOptions::crypto_salt` は `Option<[u8; 16]>` であり、暗号化有効時に未指定をエラーにする方針が確定している。この方針の適用後、C API 経由で passphrase を設定した Caller は salt を渡す手段がないためハンドシェイクが必ず失敗する。C API に salt を渡す手段を追加する必要がある。

## 現状

```rust
#[repr(C)]
pub struct SrtConnectionOptions {
    pub socket_id: u32,
    pub passphrase: *const c_char,
    pub key_length: u8,
    pub tsbpd_delay: u16,
}
```

`srt_connection_new_caller` は `ConnectionOptions` を `..ConnectionOptions::default()` で構築するため、`crypto_salt` は常に `None` になる。

Listener 側はハンドシェイクで KMREQ の salt を使用するため、本修正の影響は Caller 側のみ。

## 設計方針

`SrtConnectionOptions` に `crypto_salt: *const u8` フィールドを追加する。既存の `passphrase: *const c_char`（NULL で未設定）と同じ流儀で、**NULL ポインタを未設定**とする。salt は SRT 仕様で固定 16 バイトのため、長さフィールドは追加せず「非 NULL なら先頭 16 バイトを salt として読み取る」仕様とする。

`srt_connection_new_caller` で非 NULL の場合に 16 バイトをコピーして `ConnectionOptions::crypto_salt` へ反映する。Listener は KMREQ の salt を使用するため反映しない。

ヘッダ `crates/c-api/include/srt.h` は `crates/c-api/build.rs` の cbindgen で自動生成されるため、構造体の変更に伴い再生成される。

## 完了条件

- `SrtConnectionOptions` に salt 用フィールドが追加されていること
- `srt_connection_new_caller` が salt 用フィールドの 16 バイトを `ConnectionOptions::crypto_salt` へ反映すること
- C API 経由で passphrase と crypto_salt を設定した Caller と、passphrase を設定した Listener のハンドシェイクが完了し、両方が Connected になる e2e テストが追加されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test --workspace` で全テストが通過すること

## 解決方法

`crates/c-api/src/lib.rs` の `SrtConnectionOptions` に `crypto_salt: *const u8` フィールド（NULL で未設定、16 バイト固定）を追加する。

`srt_connection_new_caller` で salt 用フィールドが非 NULL の場合、ポインタ先から 16 バイトをコピーして `ConnectionOptions::crypto_salt` へ反映する。NULL の場合は `None` のままとする。

```rust
let crypto_salt = if opts.crypto_salt.is_null() {
    None
} else {
    Some(*opts.crypto_salt.cast::<[u8; 16]>())
};
```

`srt_connection_new_listener` は変更しない（Listener は KMREQ の salt を使用するため）。

`crates/c-api/build.rs` の cbindgen により再生成された `crates/c-api/include/srt.h` もコミットに含める。

テストは `crates/c-api/tests/` に新規テストファイルを追加し、C API の関数（`srt_connection_new_caller` / `srt_connection_new_listener` / `srt_connection_connect` / `srt_connection_feed_recv_buf` / `srt_connection_poll_output` / `srt_connection_state` 等）のみでハンドシェイクを駆動し、Caller に passphrase と crypto_salt を、Listener に passphrase を設定して両方が Connected になることを検証する。テストで使う passphrase と salt は `SrtConnectionOptions` を指すポインタが有効な間はスコープ内に保持し、`srt_connection_free` と `srt_output_data_free` で確実に解放する。
