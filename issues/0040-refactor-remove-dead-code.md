# コードベース内のデッドコードを削除する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/change-remove-dead-code
- Polished: 2026-08-01

## 目的

レビューで特定された以下のデッドコードを削除する:

1. `src/srt_sender.rs` の `SenderBuffer.max_buffer_size` フィールド (読み取られない。`#[allow(dead_code)]` で抑制されている)
2. `src/srt_receiver.rs` の `ReceivedPacket.recv_time` フィールド (読み取られない。`#[allow(dead_code)]` で抑制されている)
3. `src/srt_handshake.rs` の `HandshakeState::Failed` variant (読み取られない。`srt_connection.rs` のハンドシェイクタイムアウト時の代入 (`self.handshake_state = HandshakeState::Failed`) も同時に削除する。公開 API の削除のため CHANGES.md に `[CHANGE]` エントリを追加する)
4. `src/srt_handshake.rs` の `cipher_type::AES_GCM` 定数 (使用されない。`cipher_type` モジュールの `#[allow(dead_code)]` は `AES_CTR` が使用中のため除去できる。`KmMessage::cipher` の doc コメントの「AES-GCM = 4」はワイヤ仕様の記述のため残す)
5. `src/srt_connection.rs` の `check_km_refresh` の未使用パラメータ `_now` (パラメータと呼び出し側の引数を削除)

以下の項目は本 issue の対象から外す:

- `ConnectionState::Conclusion` — crates/c-api の `SrtConnectionState` (C ABI、値 2) と `From` 実装が使用しており、削除は C ABI の変更を伴うため別途判断が必要
- `ControlPacket::new` — pbt/tests/prop_packet.rs のテストが使用中であり「未使用」ではない
- `add_congestion_extension` / `get_congestion_extension` — pbt の roundtrip 検証と単体テストが使用中。「接続から未使用」という根拠はテストでの使用を無視している
- `SRT_CMD_KMREQ` / `SRT_CMD_KMRSP` 定数重複と `handle_data_packet` の不要な `clone()` — デッドコードではなく重複解消・パフォーマンス改善のため、本 issue の趣旨と異なる

## 優先度根拠

機能には影響しないが、`#[allow(dead_code)]` による抑制が 3 箇所にあり、放置された「壊れた窓」状態にある。write-only の `HandshakeState::Failed` は読み取られない状態遷移であり、将来の実装者が誤導される危険がある。

## 相互作用

- #0027 (srt_connection.rs の分割) は send_km_request / send_km_response を変更するが、本 issue の対象 (check_km_refresh の `_now`) とは競合しない。なお、#0027 の相互作用セクションは本 issue が `SRT_CMD_KMREQ` / `SRT_CMD_KMRSP` 定数重複を除去するという旧前提で書かれている (本 issue では対象外) ため、実装時に #0027 側の記述を更新すること
- #0028 (srt_receiver.rs の receive() 分割) は本 issue の対象 (recv_time フィールドの削除) と同じ `receive()` 内のコードを変更するため、並行実装時は直列に実装する (先後は問わない)
- #0029 (公開 API 縮小) は `HandshakeState` と `cipher_type` をスコープ外と明記しているが、本 issue はその両方 (`HandshakeState::Failed` variant・`AES_GCM` 定数) を削除する。直接競合しないが、両方が CHANGES.md の `[CHANGE]` エントリを追加するため、実装時はエントリの重複に注意すること
- #0034 (PBT と重複する単体テストの削除) は本 issue の対象外の関数 (add/get_congestion_extension のテスト) を扱うが、本 issue は対象外のため競合しない

## テスト

新規テストは追加しない。ただし、`HandshakeState::Failed` の代入削除後は、タイムアウト (Disconnected) 後に遅延到着したハンドシェイクパケットが処理され、再接続が成立し得る挙動になる (従来は `Failed` 代入がこのケースを無視していた)。この挙動変化は既存のタイムアウトテストの範囲外のため、許容する。`cargo test --workspace` (pbt を含む) で全テストが通過すること。

## CHANGES.md

`HandshakeState::Failed` の削除は公開 API の後方互換のない変更のため、shiguredo-changelog 規約に従い `### misc` サブセクションに `[CHANGE]` エントリを追加する (例: `[CHANGE] 未使用の HandshakeState::Failed を削除する`。担当者行を付けて追加すること)。#0046 (CHANGES.md のラベル整形) の指針に従い、公開 API の削除が misc に適切かは #0046 の判断に委ねる。

## 完了条件

- 目的セクションで列挙した 5 項目のデッドコードが削除されていること
- `cargo test --workspace` で全テスト (pbt を含む) が通過すること
- `cargo build --workspace` と `cargo clippy --workspace --all-targets -- -D warnings` が通過すること
- CHANGES.md に `[CHANGE]` エントリが追加されていること
