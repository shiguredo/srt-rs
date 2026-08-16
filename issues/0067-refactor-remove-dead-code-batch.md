# 削除候補の一括対応 (デッドコード・未使用・過剰抽象化)

- Priority: Medium
- Created: 2026-08-16
- Branch: feature/refactor-remove-dead-code-batch

## 目的

review-code で検出された以下の削除候補を一括で対応する。

## 削除候補一覧

### 1. cipher_type::AES_GCM が未使用 (src/srt_handshake.rs)

`cipher_type::AES_GCM` 定数 (値 4) がどこからも参照されていない。`#[expect(dead_code)]` がモジュール全体に付与されているが、`AES_CTR` は使用中。`AES_GCM` を削除すれば `#[expect(dead_code)]` も除去できる。

### 2. HandshakeState::Failed が write-only (src/srt_handshake.rs, src/srt_connection.rs)

`Failed` は `handle_timer` のハンドシェイクタイムアウト時に設定されるが、後続の処理で読み取られることはない。`handle_handshake_caller` と `handle_handshake_listener` では `Failed` がチェックされない。

### 3. ReceivedPacket::recv_time フィールドが未使用 (src/srt_receiver.rs)

`#[expect(dead_code)]` で抑制されている。書き込まれているが読み取られていない。

### 4. SenderBuffer::max_buffer_size フィールドが未使用 (src/srt_sender.rs)

`#[expect(dead_code)]` で抑制されている。書き込まれているが読み取られていない。

### 5. AccessControlBuilder がテスト専用 (src/stream_id.rs)

プロダクションコードでは `AccessControl::parse()` と `AccessControl::encode()` が直接使われており、Builder はテストと PBT でのみ使用されている。shiguredo-rust 規約「推測で追加された抽象化（使われていない Builder）」に該当する可能性がある。

### 6. add_km_error がテスト専用 (src/srt_handshake.rs)

`add_km_error` はテストと PBT でのみ使用されている。`get_km_response()` のエラーパスをテストするために必要な API ではあるが、プロダクションコードでは呼ばれない。

### 7. stream_encapsulation モジュールの定数が 1 つだけ (src/srt_handshake.rs)

`MPEG_TS_SRT` 定数 1 つのために `pub mod stream_encapsulation` が作られている。`KmMessage` 内で直接定義すれば十分。

### 8. drop_too_late 内の到達不能コード (src/srt_receiver.rs)

`self.packets.get(&seq)` は `loss_list` 内の seq に対して常に `None` を返す。`loss_list` に追加される条件は `!self.packets.contains_key(&s)` であり、受信済みパケットが `loss_list` に入ることはない。

### 9. check_km_refresh の _now パラメータが未使用 (src/srt_connection.rs)

`_now: Timestamp` として受け取っているが使用していない。

## 設計方針

各項目について、削除するか維持するかを判断する。維持する場合は `#[expect(dead_code)]` の代わりにコメントで理由を説明する。

## 完了条件

- 上記の削除候補が適切に処理されていること
- `cargo test` で全テストが通過すること
- `cargo clippy --workspace --all-targets -- -D warnings` が通過すること
