# process_retransmit で暗号化失敗時にエラーハンドリングが不十分

- Priority: High
- Created: 2026-08-16
- Branch: feature/fix-process-retransmit-encrypt-error-handling
- Polished: 2026-08-29

## 目的

`src/srt_connection.rs` の `process_retransmit` メソッド内で、`crypto.encrypt` の失敗が `if let Ok(...)` で握り潰されている。暗号化に失敗した場合、`encryption_flag` が 0 のまま平文ペイロードの再送パケットが送信される。受信側の `handle_data_packet` は `encryption_flag == 0` のパケットを復号せずエラーも返さず受け入れるため、対向でエラーが発生するわけではなく、暗号化が有効な接続で平文 (未暗号化) のペイロードがそのままアプリケーションに配信される。一次資料 (refs/srt/draft-sharabayko-srt.md) の Data Packets セクションにある KK フィールド定義でも 00b は「data is not encrypted」を示す正当な値であり、受信側が KK=0 をエラー扱いする規定はない。

なお、現状のコードパスでは `crypto.encrypt` は失敗しない (SEK は `CryptoContext::new_sender` と `start_pre_announce` で長さ検証済み、`new_receiver` と `update_sek` は AES-KW の整合性検証を通った unwrap 結果しか受け入れないため。`encrypt` の失敗は `encrypt_payload` 内の aws-lc-rs 呼び出しに由来し、ライブラリ内部の失敗条件は残る)。そのため本 issue は現に発生するバグの修正ではなく、**防御的エラー処理の一貫性**の確保である。

## 現状

```rust
if let Some(ref mut crypto) = self.crypto
    && let Ok(key_flag) =
        crypto.encrypt(packet.sequence_number, &mut packet.payload)
{
    packet.encryption_flag = key_flag.to_kk_field();
}
```

失敗分岐にログ出力もエラー伝播もない。`SenderBuffer::push` が `encryption_flag: 0` の平文パケットをバッファに保存し、`pop_retransmit` がそれを clone して返すため、encrypt 失敗時の再送パケットは `encryption_flag: 0` かつ平文ペイロードのまま送信される。

## 設計方針

`process_retransmit` の戻り値を `Result<(), Error>` に変更し、`crypto.encrypt` のエラーを `?` 演算子で呼び出し元に伝播させる。`tracing::error!` によるログ出力と該当パケットの再送スキップのみの対応は採用しない (スキップされたパケットは対向の NAK で loss_list に再追加されるまで再送されず、TLPKTDROP による期限切れ削除まで再送失敗が続く可能性があり、アプリケーションはエラーを検知できないため)。

根拠:

- 初回送信パス (`send`) と受信パス (`handle_data_packet` の decrypt) はどちらも暗号化エラーを `?` で伝播しており、再送パスだけが握り潰している
- 呼び出し元は `handle_timer` (`TimerId::Retransmit` 分岐) と `handle_nak` の 2 箇所で、いずれも `Result<(), Error>` を返すため `?` で伝播可能
- `process_retransmit` は while ループで複数パケットを処理するが、伝播時は最初の失敗でループを抜ける。未処理のパケットは loss_list に残り、次回の再送機会 (対向 NAK または Retransmit タイマー) で処理される。失敗したパケットは loss_list から取り出し済みのため、対向の NAK で loss_list に再追加された後に再送される
- `SenderBuffer` 側の実装変更は不要 (`pop_retransmit` は変更しない)

## テスト

`encrypt` の失敗パスは公開 API から発生させられない (モック・スタブ禁止のため) ため、失敗パス自体のテストは作成しない。検証はコードレビュー (`if let Ok` パターンの除去) と既存テストの通過による。

戻り値の型変更に伴い、`process_retransmit` を直接呼び出す既存テストを修正する:

- `tests/test_srt_connection.rs` の `test_process_retransmit`
- `pbt/tests/prop_connection.rs` の `prop_process_retransmit_noop_when_disconnected`

## 完了条件

- `process_retransmit` の戻り値が `Result<(), Error>` に変更され、`crypto.encrypt` の失敗が `?` 演算子で呼び出し元に伝播されること
- `if let Ok(...)` による握り潰しパターンが除去されていること
- `tests/test_srt_connection.rs` と `pbt/tests/prop_connection.rs` の直接呼び出し箇所が新しい戻り値型に追従していること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること (握り潰しは潜在バグであり、バグ修正として `[FIX]` に分類する)
- `cargo test` で全テストが通過すること
