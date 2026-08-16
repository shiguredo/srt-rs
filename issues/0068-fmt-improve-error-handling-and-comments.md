# 改善提案の一括対応 (エラーハンドリング・コメント・命名)

- Priority: Medium
- Created: 2026-08-16
- Branch: feature/fmt-improve-error-handling-and-comments

## 目的

review-code で検出された以下の改善提案を一括で対応する。

## 改善提案一覧

### 1. expect("iterations should be non-zero") が英語 (src/crypto.rs)

`expect` メッセージが英語で書かれている。他の `expect` は日本語で統一されている。

### 2. encode でカスタムキーに = や , が含まれる場合のエスケープ不在 (src/stream_id.rs)

`encode` で生成した文字列を `parse` に戻すと、`=` や `,` を含むカスタムキー/値が正しく復元されない。

### 3. Timestamp の Add/Sub 実装が saturating を使っている (src/time.rs)

Rust の `+` 演算子は通常 wrap または panic だが、この実装は saturating。呼び出し側が `+` を通常の加算として使うと、オーバーフロー時に `u64::MAX` に張り付く予期せぬ挙動になる。

### 4. insufficient_buffer エラーに理由情報が含まれない (src/error.rs)

`check_buffer_size` も `required_size` と実際の `buf.len()` を reason に含めない。

### 5. parse_loss_list の損失数上限 1000 が任意 (src/srt_connection.rs)

上限到達時に残りの損失リストが無視されるが、その事実が呼び出し元に通知されない。

### 6. send_conclusion_request の dest_socket_id = 0 のコメントが不正確 (src/srt_connection.rs)

コメントは「libsrt 互換」としているが、仕様で Conclusion Request の `dest_socket_id = 0` は MUST 規定である。

### 7. send_induction_request の dest_socket_id = 0 にコメントがない (src/srt_connection.rs)

### 8. test_sid_extension_empty のコメントと実装の不一致 (src/srt_handshake.rs)

コメントは「空文字列の場合は None になる」としているが、実際のアサーションは `Some("")`。

### 9. handle_handshake_caller/listener が未知のハンドシェイクタイプを無視する (src/srt_connection.rs)

DONE / AGREEMENT / WAVEAHAND タイプを無視する。Rendezvous モードの対向と誤接続した場合に気づけない。

### 10. handle_shutdown が Closing 状態のチェックをしていない (src/srt_connection.rs)

### 11. derive_kek の PBKDF2 salt が下位 8 バイトのみ使用する理由のコメントが不十分 (src/crypto.rs)

### 12. 空文字列キーがカスタムキーとして登録される (src/stream_id.rs)

### 13. generate_ack の is_light 判定ロジックにコメントがない (src/srt_receiver.rs)

### 14. find_deliverable_seq の O(n×m) 計算量 (src/srt_receiver.rs)

### 15. crates/c-api の srt_connection_new_caller と new_listener でコード重複 (crates/c-api/src/lib.rs)

### 16. crates/c-api の Error と Disconnected のメッセージが捨てられる (crates/c-api/src/lib.rs)

### 17. examples 間で乱数生成方法が不一致 (examples/srt_caller, examples/srt_listener)

srt_caller は getrandom クレート、srt_listener は aws_lc_rs::rand::fill を使用。shiguredo-rust 規約「暗号ライブラリは aws-lc-rs を使うこと」。

### 18. srt_listener の Cargo.toml に不要な aws-lc-rs 依存 (examples/srt_listener/Cargo.toml)

## 設計方針

各項目について、修正するかどうかを判断する。緊急度の低い項目は pending にしてもよい。

## 完了条件

- 上記の改善提案が適切に処理されていること
- `cargo test` で全テストが通過すること
