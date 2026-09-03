# 受信バッファのパケット蓄積に上限チェックがない

- Created: 2026-08-16
- Completed: 2026-09-03
- Branch: feature/fix-receiver-buffer-no-limit
- Polished: 2026-08-16

## 目的

`src/srt_receiver.rs` の `ReceiverBuffer` 構造体で `packets: BTreeMap<u32, ReceivedPacket>` の蓄積に上限チェックがない。`receive` メソッドは `expected_seq` 未満の seq を破棄するだけで、`expected_seq` 以上のパケットは無制限に `packets.insert` する。フローウィンドウを無視する悪意あるピアやバグにより大量のパケットが送りつけられた場合、BTreeMap が無制限に肥大化し、メモリ枯渇を引き起こす可能性がある。

さらに、`packets.len()` が `max_buffer_size` (8192) を超えると、`generate_ack` 内の `self.max_buffer_size - self.packets.len() as u32` が u32 アンダーフローを起こし、debug ビルドで panic する。本修正はこの実バグも同時に解消する。

なお、`expected_seq` は受信時点で進み配信 (`pop_ready`) を待たないため、受信側の蓄積は「配信待ち + 未 ACK」になる。TSBPD 配信待ちが 8192 を超える高レート × 長レイテンシ構成では、誠実なピア同士でも `packets.len()` が 8192 を超えうる。また、受信 ACK の `available_buffer` が送信側のフローウィンドウに反映される仕組みは未実装であり、この前提を補強する仕組みも存在しない (送信側のフローウィンドウは in-flight 数のみを制限する)。

## 現状

```rust
packets: BTreeMap<u32, ReceivedPacket>,
```

`ReceiverBuffer` には `max_buffer_size: u32` フィールドが既に存在する (`src/srt_receiver.rs` の `ReceiverBuffer` 構造体、初期値 8192)。ただしこれは ACK の `available_buffer` 計算 (同ファイルの `generate_ack`) に使用されるだけで、`packets` の肥大化を防ぐ制御には使われていない。一方、`SenderBuffer` 側の `max_buffer_size` は `#[expect(dead_code)]` 付きの未使用フィールドである。

## 設計方針

`receive` メソッド内で、`packets.len() >= max_buffer_size` の場合に新しいパケットを受け入れない (破棄する)。「バッファ内の最も古いパケットをドロップする」方式は採用しない。`receive` に入るパケットは既に `expected_seq` 未満がフィルタ済みであり、受信済みの最古パケットをドロップすると `find_deliverable_seq` のギャップ判定と矛盾するためである。

既存の `max_buffer_size` フィールドをそのまま上限値として使用する (新規フィールドの追加は不要)。上限値のパラメータ化 (`ConnectionOptions` からの設定) は本 issue のスコープ外とする。

なお、破棄されたパケットの seq は `packets` に不在のまま残る。バッファが上限未満に戻って後続パケットが受け入れられた時点で、損失検出ループが欠損として `loss_list` に登録し、NAK 経由で再送される。誠実なピアの高レート × 長レイテンシ構成で上限超過が継続する場合、再送が再破棄される (再送ループ) か、TSBPD 期限切れでデータ欠落が生じる。受信バッファのフロー制御 (ACK の `available_buffer` を送信側へ反映して送信を止める仕組み) は未実装であり、別 issue で対応する。本 issue はメモリ枯渇と panic を防ぐセーフティネットとしての上限チェックを導入する。

## 完了条件

- `receive` メソッドで `packets.len() >= max_buffer_size` の場合に新しいパケットが破棄されること
- 破棄されたパケットが `packets` に挿入されず、`loss_list` にも追加されないこと
- `packets.len()` が `max_buffer_size` を超えないこと (u32 アンダーフローの panic が発生しないこと)
- 上限超過を検証するテストが `src/srt_receiver.rs` 内の `#[cfg(test)]` モジュールに追加されていること (`max_buffer_size` は `ReceiverBuffer::new` のパラメータではないため、同一モジュールのテストからフィールドに小さい値を直接設定して超過分が破棄されることを検証する)
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test --workspace` で全テストが通過すること

## 解決方法

`src/srt_receiver.rs` の `receive` メソッド内のパケット挿入箇所で、`self.packets.len() >= self.max_buffer_size` の場合に `return` する (パケットを破棄する)。破棄時は `loss_list` への追加も行わない。

テストは `src/srt_receiver.rs` 内の `#[cfg(test)]` モジュールに追加し、`max_buffer_size` フィールドに小さい値を直接設定して、超過分のパケットが破棄されることを検証する。

## 解決方法

`src/srt_receiver.rs` の `receive` メソッド内のパケット挿入直前に、`packets.len() >= max_buffer_size` の場合に `None` を返して破棄する上限チェックを追加した。破棄時は `loss_list` にも登録せず、`expected_seq` は進めない。`packets.len()` が `max_buffer_size` を超えなくなったため、`generate_ack` 内の `available_buffer` 計算の u32 アンダーフローも発生しない。`receive` の `///` に満杯時の破棄契約を追記した。

`src/srt_receiver.rs` 内の `#[cfg(test)]` モジュールに単体テスト 2 件を追加した。上限到達時の破棄・非挿入・非登録とドレイン後の受け入れ回復、満杯時の `available_buffer == 0` (アンダーフローなし) を検証する。

`CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加した。

なお、`src/srt_sender.rs` の `max_buffer_size` は 0040 と 0067 (デッドコード削除) で削除が提案されており、本 issue では触れない。また、0028 (receive の分割)、0055 (loss_list の HashSet 化)、0073 (find_deliverable_seq の最適化)、0074 (損失検出ループの反復回数上限) は同じ `ReceiverBuffer` の `receive` メソッドを変更するため、並行実装時は直列に実装する (先後は問わない)。
