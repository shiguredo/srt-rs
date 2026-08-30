# pbt の expect メッセージを日本語に統一する

- Priority: Low
- Created: 2026-08-30
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/refactor-japanese-expect-messages-in-pbt
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

PBT のテストコード内の `expect` メッセージに英語が含まれており、shiguredo-rust の規約 (テストコードの expect メッセージは日本語) に反しているため、`src/`・`tests/` と同じ日本語に統一する。

## 現状

- `pbt/tests/prop_crypto.rs` に英語メッセージが最も多く、例えば `"sender creation should succeed"` / `"encrypt should succeed"` / `"wrap should succeed"` / `"receiver creation should succeed"` / `"decrypt should succeed"` / `"creation should succeed"` がある。同じファイル内に `"salt は 16 バイトに変換できる想定"` のような日本語メッセージも存在し、言語が混在している
- `pbt/tests/prop_handshake.rs` / `prop_connection.rs` / `prop_packet.rs` / `prop_stream_id.rs` にも英語メッセージの `expect` がある
- `pbt/tests/prop_buf.rs` / `prop_receiver.rs` / `prop_sender.rs` は日本語のみで、逸脱していない
- `src/` 配下の `#[cfg(test)]` モジュールと `tests/` 配下は日本語で統一されている

## 設計方針

`pbt/tests/` 配下の `expect` メッセージを、既存の日本語メッセージと同じ「〜する想定」形式に統一する。テスト関数名は英語のまま維持する (規約どおり)。`expect` の意味が変わらない範囲で文言を整え、動作は変更しない。

## 完了条件

- `pbt/tests/` 配下の `expect` メッセージがすべて日本語になること
- `cargo test -p pbt` で全テストが通過すること
