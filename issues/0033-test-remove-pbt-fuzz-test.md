# PBT の任意入力パニック耐性テストを fuzz ターゲットに移す

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-remove-pbt-fuzz-test
- Polished: 2026-08-01

## 目的

`pbt/tests/prop_connection.rs` の `prop_arbitrary_input_no_panic` テストは、任意バイト列を `feed_recv_buf` に渡してパニックしないことを検証している。shiguredo-rust スキルは「PBT に「任意入力でパニックしないことだけを検証するテスト」を書かないこと（fuzzing の役割）」と規定しており、本テストは違反である。任意入力のクラッシュ耐性検証は fuzz ターゲットに移す。

## 優先度根拠

機能には影響しないが、規約違反のテストが残る「壊れた窓」状態を解消する。同カテゴリの 0034 と同じく Low とする。

## 現状

```rust
fn prop_arbitrary_input_no_panic(  // 原文には doc コメント、#[test] 属性、「// パニックしないことを確認」コメントが付く
    data in prop::collection::vec(any::<u8>(), 16..1500),
) {
    let mut conn = SrtConnection::new_caller(make_opts(1));
    let now = Timestamp::from_micros(0);
    let _ = conn.feed_recv_buf(&data, now);
}
```

`prop_arbitrary_input_no_panic` は「不正入力のプロパティテスト（ファジング）」セクション (proptest! ブロック) 内の唯一のテストである。テスト直後には「// ファジングテストは fuzz/ でカバーされるため削除」というコメントが既にあり、削除は予告されているが未実施である。

## 設計方針

- `prop_arbitrary_input_no_panic` を含む proptest! ブロック (セクションコメントと予告コメントを含む) を丸ごと削除する
- 代わりに `fuzz/fuzz_targets/fuzz_connection_feed.rs` を追加する:
  - 既存ターゲット (fuzz_handshake_decode.rs 等) と同じ構成 (`#![no_main]` + `fuzz_target!` マクロ)
  - `ConnectionOptions::default()` で `SrtConnection::new_caller` を生成し、任意バイト列を `feed_recv_buf` に渡す (pbt テスト内の `make_opts` は pbt 専用ヘルパーのため使えない。`now` 引数は PBT 版と同等に `Timestamp::from_micros(0)` の固定値を使う)
  - 未接続状態のまま feed する (PBT 版と同等。既存の decode 系 fuzz ターゲットとは異なる SrtConnection 経路の検証になる)
- `fuzz/Cargo.toml` に `fuzz_connection_feed` の `[[bin]]` エントリを追加する (既存 2 ターゲットと同じ形式)
- 削除と追加は同一変更 (同一 PR) 内で行う (fuzz 未追加の間、パニック耐性の検証が空疎になるのを防ぐ)

## 相互作用

- #0037 (fuzz ターゲット追加) は `ReceiverBuffer::receive` / `KmMessage::decode` / `AccessControl::parse` / `parse_loss_list` / `encode_loss_list` を対象とし、本 issue の `feed_recv_buf` は含まれない。両 issue は同じ `fuzz/fuzz_targets/` と `fuzz/Cargo.toml` を変更するため、並行実装時は直列に実装する (先後は問わない)
- #0029 (公開 API 縮小) も pbt を変更対象とするため、並行実装時は直列に実装する (先後は問わない)

## テスト

PBT テストの削除と fuzz ターゲットの追加のため、新規の単体テストは追加しない。削除後の `cargo test` (pbt を含む) で全テストが通過すること。

## CHANGES.md

機能に直接影響しない変更 (後方互換を壊さない) のため、shiguredo-changelog 規約に従い `### misc` サブセクションに `[UPDATE]` エントリ (例: `[UPDATE] PBT の任意入力パニック耐性テストを fuzz ターゲットに移す`。担当者行を付けて追加すること) を追加する。

## 完了条件

- `prop_arbitrary_input_no_panic` を含むセクション (セクションコメントと予告コメントを含む) が pbt/tests/prop_connection.rs から削除されていること
- `fuzz/fuzz_targets/fuzz_connection_feed.rs` が追加され、`fuzz/Cargo.toml` に `[[bin]]` エントリが追加されていること
- `cargo +nightly fuzz build` でビルドできること (rust-toolchain.toml が stable 固定のため、`cargo fuzz build` は nightly 指定なしでは実行不能。`-Zsanitizer` が stable で拒否される)
- `cargo +nightly fuzz run fuzz_connection_feed -- -runs=1` で短時間実行できること (ターゲットが即時パニックしないことの確認)
- `cargo test` で全テストが通過すること
- CHANGES.md の `### misc` セクションに `[UPDATE]` エントリが追加されていること
