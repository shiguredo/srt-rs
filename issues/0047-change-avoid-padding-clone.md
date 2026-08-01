# ControlPacket::control_info を Cow<[u8]> に変更し LIBSRT_COMPAT_PADDING のクローンを排除する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/change-avoid-padding-clone
- Polished: 2026-08-01

## 目的

`src/srt_connection.rs` の `send_ackack` / `send_keepalive` / `send_shutdown` の 3 箇所で、同一の 4 バイト配列 `LIBSRT_COMPAT_PADDING` を毎回 `to_vec()` でクローンしている。ヒープ確保の発生しない形に変更する。

## 現状

- `LIBSRT_COMPAT_PADDING` は `src/srt_connection.rs` に `const LIBSRT_COMPAT_PADDING: [u8; 4] = [0, 0, 0, 0];` として定義されている
- `ControlPacket::control_info` は `pub control_info: Vec<u8>`（`src/srt_packet.rs`）で、`ControlPacket` は `src/lib.rs` の `pub use` で公開 API に含まれる
- 3 箇所とも `control_info: LIBSRT_COMPAT_PADDING.to_vec()` で毎回ヒープ確保している
- パディング自体は SRT 仕様（Keep-alive / Shutdown / ACKACK は CIF を含まない）ではなく、libsrt との相互運用性のために送る 4 バイトゼロパディングであり、維持する必要がある

## 設計方針

`ControlPacket::control_info` の型を `Vec<u8>` から `Cow<'static, [u8]>` に変更し、3 箇所を `Cow::Borrowed(&LIBSRT_COMPAT_PADDING)` にする。これによりヒープ確保が発生しなくなる。`'static` にする理由は、`ControlPacket` にライフタイムパラメータを追加すると `SrtPacket` enum や各ハンドラのシグネチャに波及するためである。`LIBSRT_COMPAT_PADDING` は定数であり `'static` で借用でき、decode パスは `slice.to_vec().into()` で所有値になるため、構造体にライフタイムパラメータを追加する必要はない。

これは公開 API の破壊的変更であり、後方互換のない変更（change カテゴリ）として扱う。

### 修正対象

1. `ControlPacket::control_info` の型を `Cow<'static, [u8]>` に変更する（`src/srt_packet.rs`）
2. `ControlPacket::new()` の `control_info: Vec::new()` を `Cow::default()` にする
3. `decode_with_first_word` の `slice.to_vec()` を `slice.to_vec().into()` にする
4. 3 箇所（`send_ackack` / `send_keepalive` / `send_shutdown`）の `control_info: LIBSRT_COMPAT_PADDING.to_vec()` を `control_info: Cow::Borrowed(&LIBSRT_COMPAT_PADDING)` にする
5. 直接構築箇所の型変換: `src/srt_handshake.rs`（encode の `control_info`）、`src/srt_connection.rs`（ACK / NAK / KMREQ / KMRSP の構築箇所）は `Vec<u8>` のまま `.into()` で `Cow` に変換する
6. `control_info.as_slice()` を使っている 2 箇所（`src/srt_handshake.rs` の decode、`src/srt_connection.rs` の handle_ack）は、`Cow<[u8]>` には inherent の `as_slice()` がないため、`as_ref()` か `&pkt.control_info[..]` に書き換える
7. テストの直接構築箇所（`src/srt_packet.rs` の単体テスト `vec![1, 2, 3, 4]`、`pbt/tests/prop_packet.rs`、`pbt/tests/prop_handshake.rs`）の `control_info: Vec::new()` 等を `Cow` に合わせて修正する

### 変更不要の確認項目

- `encode` の `buf.write_bytes(&self.control_info)` と `encoded_size` の `self.control_info.len()` は `Deref` によりそのまま動く
- `parse_loss_list(&pkt.control_info)` と `KmMessage::decode(&pkt.control_info)` は `&Cow<[u8]>` から `&[u8]` への deref coercion でそのまま動く
- `#[derive(Clone)]` による `Cow<[u8]>` のクローンは成立する

### テスト戦略

既存テスト（単体・pbt・fuzz）で `ControlPacket` の encode / decode が引き続き正しく動作することを確認する。新規テストの追加は不要。

### 他 issue との相互作用

- #0027（`srt_connection.rs` のモジュール分割）は本 issue が変更する送信メソッド群と同じ箇所を変更対象とするが、いずれの順序で実装しても実質競合しない
- #0031（インラインテストの移動）と #0034（PBT と重複する単体テストの削除）は、修正対象 7 が変更する `src/srt_packet.rs` の単体テスト（`test_control_packet_encode_decode`）と同じテストを対象とする。先に #0031 が実装された場合は移動先（`tests/test_srt_packet.rs`）で型を合わせ、先に #0034 が実装された場合は該当テストが削除済みのため参照を削除する
- #0046（CHANGES.md のラベル整形）の原則に従い、公開 API の破壊的変更のため本体セクション（`## develop` 直下の `[CHANGE]` エントリの列）にエントリを追加する

## 完了条件

- `ControlPacket::control_info` が `Cow<'static, [u8]>` になっていること
- 3 箇所の `LIBSRT_COMPAT_PADDING.to_vec()` が消滅し、`Cow::Borrowed(&LIBSRT_COMPAT_PADDING)` になっていること
- 上記 3 箇所でヒープ確保が発生しないこと
- `cargo test` で全テストが通過すること
- `cargo fmt --all --check` と `cargo clippy --workspace --all-targets -- -D warnings` が通過すること

## 解決方法

1. `ControlPacket::control_info` の型を `Cow<'static, [u8]>` に変更し、`new()` と `decode_with_first_word` を修正する（`src/srt_packet.rs`）
2. 3 箇所（`send_ackack` / `send_keepalive` / `send_shutdown`）を `Cow::Borrowed(&LIBSRT_COMPAT_PADDING)` にする（`src/srt_connection.rs`）
3. 直接構築箇所（`src/srt_handshake.rs`、`src/srt_connection.rs` の ACK / NAK / KMREQ / KMRSP、`src/srt_packet.rs` の単体テスト、pbt の構築箇所）の型を合わせる
4. `as_slice()` 2 箇所を `as_ref()` か `&pkt.control_info[..]` に書き換える
5. `CHANGES.md` の `## develop` 直下に `[CHANGE]` エントリ（例: `[CHANGE] ControlPacket::control_info を Cow<[u8]> に変更する`。担当者行を付けて追加すること）を追加する
6. `cargo test` で全テストが通過することを確認する
