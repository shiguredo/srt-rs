# process_retransmit で暗号化失敗時にエラーハンドリングが不十分

- Priority: High
- Created: 2026-08-16
- Branch: feature/bug-fix-process-retransmit-encrypt-error-handling

## 目的

`src/srt_connection.rs` の `process_retransmit` メソッド内で、`crypto.encrypt` の失敗が `if let Ok(...)` で握り潰されている。暗号化に失敗した場合、`encryption_flag` が 0 のまま再送パケットが送信される。暗号化が有効な接続で非暗号化パケットを送信すると、対向で復号化エラーが発生する。

## 現状

```rust
if let Some(ref mut crypto) = self.crypto
    && let Ok(key_flag) =
        crypto.encrypt(packet.sequence_number, &mut packet.payload)
{
    packet.encryption_flag = key_flag.to_kk_field();
}
```

## 設計方針

暗号化失敗時は `tracing::error!` でログを出力し、該当パケットの再送をスキップする。または `process_retransmit` の戻り値を `Result` に変更してエラーを伝播させる。

## 完了条件

- 暗号化失敗時にログ出力またはエラー伝播が行われること
- `cargo test` で全テストが通過すること
