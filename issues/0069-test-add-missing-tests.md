# テスト不足領域の追加

- Priority: Medium
- Created: 2026-08-16
- Branch: feature/test-add-missing-tests

## 目的

review-code で検出された以下のテスト不足領域に対してテストを追加する。

## 不足しているテスト

### 1. handle_timer の Handshake タイマーテスト (tests/test_srt_connection.rs)

`handle_timer` の全 `TimerId` 分岐のうち、`Keepalive`、`Nak`、`Retransmit`、`Inactivity`、`Ack` はテストされているが、`Handshake` タイマー（タイムアウトによる切断）のテストがない。

### 2. Disconnected 状態での send() 呼び出しテスト (tests/test_srt_connection.rs)

`test_send_before_connected` は接続前のテスト。切断後の `Disconnected` 状態での `send()` は別のコードパスだが、テストされていない。

### 3. Closing 状態の遷移テスト (tests/test_srt_connection.rs)

`disconnect()` 後に `Closing` 状態になるが、その状態での `feed_recv_buf` や `handle_timer` の挙動を検証するテストがない。

### 4. ハンドシェイク異常系テスト (tests/test_srt_connection.rs)

不正な SYN cookie、不正な handshake type、不正な拡張を含むハンドシェイクパケットを送信した場合のエラー処理テストが存在しない。

### 5. handle_ack に不正な ACK を渡すテスト (tests/test_srt_connection.rs)

`control_info < 4 bytes` の ACK を処理するテストがない。

### 6. handle_nak に空の NAK を渡すテスト (tests/test_srt_connection.rs)

### 7. process_retransmit のテスト (tests/test_srt_connection.rs)

暗号化あり・なしの両方のケースがテストされていない。

### 8. AES-256 KAT のテストカバレッジ不足 (tests/test_crypto.rs)

`aes128_kat_packet_index_zero` と `aes128_kat_packet_index_near_max` に対応する AES-256 のテストがない。

### 9. tests/test_buf.rs のテストカバレッジ不足

`read_utf8` のみテストされ、`read_u8`、`read_u16`、`read_u64`、`read_bytes` のエラーパス（バッファ不足）がテストされていない。

### 10. tests/test_error.rs のテストカバレッジ不足

`read_u32` のバッファサイズエラーのみテストされ、他の `read_*` 関数のバッファ不足テストがない。

## 設計方針

各項目についてテストを追加する。PBT で実現できるものは PBT で、単体テストでなければならないものは単体テストで書く。

## 完了条件

- 上記の全テストが追加されていること
- `cargo test --workspace` で全テストが通過すること
