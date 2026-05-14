# PBT に任意入力パニック耐性テストが書かれている

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/test-remove-pbt-fuzz-test

## 目的

`pbt/tests/prop_connection.rs:541-551` の `prop_arbitrary_input_no_panic` テストは、任意バイト列を `feed_recv_buf` に渡してパニックしないことを検証している。AGENTS.md は「PBT に「任意入力でパニックしないことだけを検証するテスト」を書かない（fuzzing の役割）」と規定している。

## 現状

```rust
fn prop_arbitrary_input_no_panic(
    data in prop::collection::vec(any::<u8>(), 16..1500),
) {
    let mut conn = SrtConnection::new_caller(make_opts(1));
    let now = Timestamp::from_micros(0);
    let _ = conn.feed_recv_buf(&data, now);
}
```

## 設計方針

このテストを削除し、代わりに `fuzz/fuzz_targets/` に `fuzz_connection_feed.rs` を追加する。

## 完了条件

- `prop_arbitrary_input_no_panic` が削除されていること
- 対応する fuzz ターゲットが追加されていること
- `cargo test` で全テストが通過すること
