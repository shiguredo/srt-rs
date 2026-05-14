# TSBPD 配信時刻計算が SRT 仕様と一致しない

- Priority: High
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-tsbpd-delivery-time-calculation

## 目的

`src/srt_receiver.rs` の TSBPD 配信時刻計算が `start_time`（接続完了時刻）をベースにしているが、SRT 仕様では `TsbpdTimeBase = T_NOW - HSREQ_TIMESTAMP` （ハンドシェイク受信時の時刻とハンドシェイクタイムスタンプの差）をベースにすると定義されている。

`start_time` は受信側ローカルクロックの絶対時刻、`packet.timestamp` は送信側クロックからの相対時刻であり、異なるクロックドメインの値を直接加算している。`TsbpdTimeBase` を正しく計算することで、送信側タイムスタンプを受信側クロックに変換し、正確な配信時刻を得る。

## 優先度根拠

TSBPD は SRT のパケット配信の基盤であり、配信タイミングの誤差はライブストリーミングの品質に直結する。`TsbpdTimeBase` は仕様上 `RTT_0/2`（初期片道遅延）に近似する値であり、RTT が大きい環境（衛星回線等）では数百ミリ秒の誤差が発生する。

## 現状

### 配信時刻計算 (`src/srt_receiver.rs:416-419`)

```rust
let delivery_time = if self.tsbpd_enabled {
    let pkt_time = self.start_time.as_micros() + packet.timestamp as u64;
    Timestamp::from_micros(pkt_time + self.tsbpd_delay_us)
} else {
    now
};
```

`start_time`（`srt_connection.rs:863` で `now` として設定）は接続完了時刻であり、仕様の `TsbpdTimeBase = T_NOW - HSREQ_TIMESTAMP` とは異なる。

### 損失パケットのドロップ判定 (`src/srt_receiver.rs:597`)

```rust
let estimated_delivery = self.start_time.as_micros() + self.tsbpd_delay_us;
```

この箇所も同様に `start_time` を使用しており、修正が必要。

### HSREQ_TIMESTAMP の取得経路の問題

`handle_handshake` (`srt_connection.rs:694`) は `ControlPacket` を受け取り `pkt.timestamp` が利用可能だが、`handle_handshake_listener` (`srt_connection.rs:804`) には `HandshakePacket` のみが渡されており、`pkt.timestamp` （HSREQ_TIMESTAMP） が伝搬されていない。

## 根拠

### Packet Delivery Time （Packet Delivery Time, 2110-2116 行目）

> Packet delivery time is the moment, estimated by the receiver, when a packet should be delivered
> to the upstream application. The calculation of packet delivery time (PktTsbpdTime) is performed
> upon receiving a data packet according to the following formula:
>
> PktTsbpdTime = TsbpdTimeBase + PKT_TIMESTAMP + TsbpdDelay + Drift

### TSBPD Time Base Calculation （TSBPD Time Base Calculation, 2136-2146 行目）

> The initial value of TSBPD time base (TsbpdTimeBase) is calculated at the moment of
> the second handshake request is received as follows:
>
> TsbpdTimeBase = T_NOW - HSREQ_TIMESTAMP
>
> where T_NOW is the current time according to the receiver clock;
> HSREQ_TIMESTAMP is the handshake packet timestamp, in microseconds.

> The value of TsbpdTimeBase is approximately equal to the initial one-way delay of the link RTT_0/2

## 設計方針

CONCLUSION 受信時に `TsbpdTimeBase = T_NOW - HSREQ_TIMESTAMP` を計算し、`ReceiverBuffer` に渡す。`start_time` フィールドは `last_ack_time` 初期値や `ReceivingRateEstimator` で引き続き使用するため残す。TSBPD 計算でのみ `tsbpd_time_base` を使用するよう変更する。

### 修正対象

1. `src/srt_connection.rs` の `handle_handshake` で `pkt.timestamp` を `handle_handshake_listener` / `handle_handshake_caller` に渡す
2. `src/srt_connection.rs` の CONCLUSION 受信処理で `tsbpd_time_base = now.as_micros() - hsreq_timestamp as u64` を計算し保持する
3. `src/srt_connection.rs` の `init_buffers` に `tsbpd_time_base: u64` 引数を追加する （`TsbpdTimeBase` は時刻差であり絶対時刻ではないため `Timestamp` ではなく `u64` とする）
4. `src/srt_receiver.rs` の `ReceiverBuffer` に `tsbpd_time_base: u64` フィールドを追加し、`new` のシグネチャを変更する
5. `src/srt_receiver.rs:418` の配信時刻計算を `self.tsbpd_time_base + packet.timestamp as u64 + self.tsbpd_delay_us` に修正する
6. `src/srt_receiver.rs:597` の損失パケットドロップ判定を `self.tsbpd_time_base + self.tsbpd_delay_us` に修正する
7. `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加する:
   - `[FIX] TSBPD 配信時刻計算を SRT 仕様の TsbpdTimeBase に準拠するよう修正する`

### テスト戦略

`ReceiverBuffer::new` のシグネチャが変更されるため、以下のテストが影響を受ける:

- `src/srt_receiver.rs` の `#[cfg(test)] mod tests`
- `pbt/tests/prop_receiver.rs`

修正後、`src/srt_receiver.rs` の `#[cfg(test)] mod tests` に以下の単体テストを追加する:

- `TsbpdTimeBase` が正しく計算されていることの検証（`start_time` との差分が `HSREQ_TIMESTAMP` に相当すること）
- 配信時刻が `tsbpd_time_base + packet.timestamp + tsbpd_delay` で計算されることの検証

### スコープ外

- **Drift 補正**: 仕様の配信時刻計算式には `+ Drift` が含まれるが、Drift Management （仕様 Drift Management, 2233 行目以降）は独立した機構であり、本 issue では対応しない。必要であれば別 issue で対応する
- **TSBPD Wrapping Period**: 32-bit タイムスタンプのラップアラウンド対応は issue #0014 で対応する。#0014 は本 issue の修正後に `tsbpd_time_base` フィールドを利用する形で実装する

## 完了条件

- CONCLUSION 受信時に `TsbpdTimeBase = T_NOW - HSREQ_TIMESTAMP` が計算されていること
- `ReceiverBuffer` の TSBPD 配信時刻計算が `tsbpd_time_base` ベースになっていること
- `drop_too_late` のドロップ判定も `tsbpd_time_base` ベースに修正されていること
- `start_time` が TSBPD 以外の用途（`last_ack_time` 初期値、`ReceivingRateEstimator`）で引き続き使用されていること
- 既存の全テスト (`cargo test`) が通過すること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること

## 解決方法

1. `src/srt_receiver.rs` の `ReceiverBuffer` に `tsbpd_time_base: u64` フィールドを追加し、`new()` のシグネチャに引数を追加
2. `src/srt_receiver.rs` の配信時刻計算（`receive`）を `self.start_time.as_micros() + packet.timestamp` から `self.tsbpd_time_base + packet.timestamp` に修正
3. `src/srt_receiver.rs` の `drop_too_late` のドロップ判定を `self.start_time.as_micros() + self.tsbpd_delay_us` から `self.tsbpd_time_base + self.tsbpd_delay_us` に修正
4. `src/srt_receiver.rs` の `start_time` フィールドを削除（TSBPD 計算から参照がなくなったため未使用になった）
5. `src/srt_connection.rs` の `handle_handshake` で `pkt.timestamp` を `handle_handshake_caller` / `handle_handshake_listener` に伝搬
6. CONCLUSION 受信時に `TsbpdTimeBase = T_NOW - HSREQ_TIMESTAMP` を計算し、`init_buffers` 経由で `ReceiverBuffer` に渡す
7. `src/srt_receiver.rs` に TSBPD の仕様値を直接検証する単体テストを 3 件追加
8. `CHANGES.md` に `[FIX]` エントリを追加
