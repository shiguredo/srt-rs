# process_retransmit が encrypt 失敗時に平文のままパケットを送信する

- Priority: Low
- Created: 2026-08-30
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/fix-retransmit-plaintext-send
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

暗号化有効な接続で、再送パケットの暗号化に失敗した場合に `encryption_flag` が 0 (非暗号化) のままパケットが送出される経路をなくす。暗号化を有効にした接続で平文のメディアがネットワークに流出するのは機密性の観点で許容できない。

## 現状

- `src/srt_connection.rs` の `process_retransmit` は再送パケットの暗号化を `if let Some(ref mut crypto) = self.crypto && let Ok(key_flag) = crypto.encrypt(...)` で行っており、`encrypt` が `Err` を返した場合は暗号化をスキップして `encryption_flag` が 0 のまま `output_queue` へ積む
- 同ファイルの `send()` は `encrypt` の `Err` を `?` で呼び出し元へ伝播しており、送信経路ごとに失敗時の扱いが不整合
- `encrypt` が `Err` を返すのは `encrypt_payload` の失敗時であり、SEK 長は `CryptoContext::new_sender` / `new_receiver` で検証済みのため通常運用では到達しない。ただし到達した場合の挙動が未定義のままになっている

## 設計方針

`process_retransmit` でも `send()` と同じく `?` で呼び出し元へエラーを伝播する。`process_retransmit` の戻り値は `()` なので `Result<(), Error>` への変更が必要で、呼び出し元の `handle_timer` (`TimerId::Retransmit` 経路) は既に `Result` を返すため伝播は素直である。`///` ドキュメントには `Err` になる条件を記載する。到達不能である現状でも失敗時に平文を送らないことを優先し、テストは `#[cfg(test)]` モジュールで構築可能な範囲で検証する。

## 完了条件

- `process_retransmit` で `encrypt` が失敗した場合に平文のパケットが送出されないこと
- 失敗時に呼び出し元へエラーが伝播すること
- 失敗時の挙動を検証するテストが追加されていること (検証可能な範囲で構成してよい)
- `cargo test` で全テストが通過すること
