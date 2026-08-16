# 受信 ACK の available_buffer が送信側のフローウィンドウに反映されない

- Created: 2026-08-16
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-available-buffer-flow-control
- Polished: {YYYY-MM-DD}

## 目的

受信側は ACK の `available_buffer` フィールド (受信バッファの空き容量) を計算してワイヤに書いているが (`src/srt_receiver.rs` の `generate_ack`、`src/srt_connection.rs` の `send_ack`)、送信側の `handle_ack` はこのフィールドを読み取らず、`src/srt_sender.rs` の `set_flow_window` は定義のみで呼び出し元が存在しない。このため受信ウィンドウ制御が機能しておらず、受信側バッファの空き状況が送信側に伝わらない。受信バッファのフロー制御を実現するための前提となる機能を実装する。

## 現状

- `src/srt_connection.rs` の `handle_ack` は ACK の `control_info` から `ack_seq` (先頭 4 バイト) のみを読み取り、`available_buffer` を読み取らない。Full ACK の判定は `control_info.len() >= 16` で行い、その場合は ACKACK の送信のみを行う
- `src/srt_sender.rs` の `set_flow_window` は `self.flow_window` を更新する実装を持つが、呼び出し元が存在しない。`flow_window` は `init_buffers` で `DEFAULT_FLOW_WINDOW` (8192) に固定され、接続中に変化しない
- `src/srt_sender.rs` の `can_send` は `in_flight < self.flow_window && in_flight < self.congestion_window` で送信可否を判定するため、`flow_window` を更新すれば送信が抑制される
- `src/srt_receiver.rs` の `generate_ack` は `available_buffer: self.max_buffer_size - self.packets.len() as u32` を計算し、`src/srt_connection.rs` の `send_ack` が Full ACK に書き込む

仕様上、ACK の CIF には Last Acknowledged Packet Sequence Number, RTT, RTT Variance, Available Buffer Size, Packets Receiving Rate, Estimated Link Capacity, Receiving Rate が並び、Available Buffer Size は「受信バッファの空き容量 (パケット数)」である (`refs/srt/draft-sharabayko-srt.md` の ACK control packet の定義)。本実装の Full ACK は全フィールドを含むため、`available_buffer` は `control_info` のオフセット 12 バイト (ack_seq 4 バイト + RTT 4 バイト + RTT Variance 4 バイトの後) に存在する。Light ACK は ack_seq のみを含むため、`available_buffer` を含まない。

## 設計方針

受信バッファの上限チェックを導入する issue の設計方針で、受信バッファのフロー制御 (ACK の `available_buffer` を送信側へ反映して送信を止める仕組み) は未実装であり別 issue で対応すると明記されている。本 issue はその別 issue にあたり、受信側のバッファ空き容量を送信側のフローウィンドウに反映する経路を実装する。

`handle_ack` で Full ACK の `available_buffer` を読み取り、`SenderBuffer::set_flow_window` で `flow_window` を更新する。`can_send` が `flow_window` で送信可否を判定するため、受信側のバッファが満杯に近づくと `available_buffer` が小さくなり、送信側の in-flight 数が制限されて送信が止まる。

なお、受信側のバッファ上限チェック (受信側で `packets.len() >= max_buffer_size` のパケットを破棄する) と組み合わせることで、受信バッファの溢れを防ぐフロー制御が機能する。本 issue は送信側の反映経路のみを実装し、上限チェック側の変更は行わない (上限チェック側は別 issue で対応済み)。

## 完了条件

- `src/srt_connection.rs` の `handle_ack` が Full ACK の `available_buffer` を読み取り、`src/srt_sender.rs` の `set_flow_window` を介して `flow_window` が更新されること
- `flow_window` が更新されると `can_send` の送信可否判定に反映されること (単体テストで検証)
- Light ACK では `available_buffer` を読み取らないこと
- テストが `src/srt_sender.rs` と `src/srt_connection.rs` の `#[cfg(test)]` モジュールに追加されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test --workspace` で全テストが通過すること

## 解決方法

`src/srt_connection.rs` の `handle_ack` で、Full ACK (`control_info.len() >= 16`) の場合に `control_info` のオフセット 12 から 4 バイトを読み取り、`sender.set_flow_window(...)` を呼ぶ。`src/srt_sender.rs` の `set_flow_window` は既存実装をそのまま利用する (新規実装不要)。

テストは以下の 2 つを追加する:

- `src/srt_sender.rs`: `set_flow_window` で `flow_window` を小さくした後、`can_send` が false になること
- `src/srt_connection.rs`: `handle_ack` に `available_buffer` を含む Full ACK を渡した後、`SenderBuffer` の `flow_window` が反映されること (in-flight 数が `available_buffer` を超えると送信が抑制されること)
