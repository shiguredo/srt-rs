# TSBPD 配信時刻の合成が非飽和で、u64 近傍の時刻入力だと panic または過去時刻になる

- Priority: Medium
- Created: 2026-08-29
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/fix-receiver-tsbpd-saturating-add
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

`Timestamp` は公開 API であり、`from_micros` / `add_micros` / `add_millis` (いずれも `u64::MAX` で飽和する) 経由で `u64::MAX` 近傍の値を生成できる。`SrtConnection` のハンドシェイク完了時 (`handle_handshake_listener` と `handle_handshake_caller`) に

```rust
let tsbpd_time_base = now.as_micros().saturating_sub(hsreq_timestamp as u64);
```

で計算される TSBPD 時刻基準は、`now` が `u64::MAX` 近傍なら `tsbpd_time_base` も `u64::MAX` 近傍になる。

一方 `ReceiverBuffer` 側の配信時刻合成は非飽和の `+` を使っているため、その基準値を渡すと

- debug ビルドは overflow で panic する
- release ビルドは wrap して配信時刻が現在より過去になり、TSBPD による即配信や `drop_too_late` / `drop_expired` の誤判定が音もなく発生する

`Timestamp` の加算系がすべて飽和で統一されている (`add_micros`、`add_millis`、`impl Add<u64>`) のに対し、`ReceiverBuffer` だけ方針が異なる。shiguredo-rust の「性能より堅牢性を優先すること」に照らして飽和へ統一し、異常値に対する静かなデータ破棄を panic よりも安全な側へ寄せる。

## 現状

非飽和の `+` を使う箇所 (`src/srt_receiver.rs`)。

- `ReceiverBuffer::receive` 内の配信時刻計算: `let pkt_time = self.tsbpd_time_base + packet.timestamp as u64 + if ... { MAX_TIMESTAMP + 1 } else { 0 };`
- 同上の直後: `Timestamp::from_micros(pkt_time + self.tsbpd_delay_us)`
- `ReceiverBuffer::pop_ready` 内の wrapping period 終了処理: `self.tsbpd_time_base += MAX_TIMESTAMP + 1;`
- `ReceiverBuffer::drop_expired` 内の推定配送時刻のフォールバック: `let base = self.tsbpd_time_base + self.tsbpd_delay_us;` および `base + MAX_TIMESTAMP + 1`

`tsbpd_time_base` は `ReceiverBuffer::new` の引数 (`pub`) として直接も渡せるため、`SrtConnection` を経由せずに到達することもできる。

overflow が成立する具体値は次のとおり。

- `tsbpd_time_base = u64::MAX - 1_000_000`、`packet.timestamp = 5_000_000` とすると `tsbpd_time_base + packet.timestamp as u64` は `u64::MAX + 4_000_000` で overflow
- release では `4_000_000` に wrap し、`tsbpd_delay_us` を足した配信時刻は現在時刻より確実に過去になる

## 設計方針

- 上記 4 箇所の加算を飽和演算 (`saturating_add` など) に統一する。`MAX_TIMESTAMP + 1` によるラップ補正の仕様上の意味 (TSBPD wrapping period。`src/srt_receiver.rs` のコメントが引いている draft-sharabayko-srt.md の `#tsbpd-time-base` 節) は変えない。節構成・表現は将来変更される可能性がある
- 飽和した配信時刻 (`u64::MAX` マイクロ秒) は `pop_ready` / `drop_too_late` / `drop_expired` の `now` との比較で「期限未到来」として扱われる。過去への wrap で即配信・即ドロップするより安全側である。TSBPD Time Base が接続開始からの相対マイクロ秒である以上、`u64` 相当 (約 5.8 億年) を超える基準値は現実には発生しないため、飽和は異常入力に対するフォールバックとして置く
- 対象は `src/srt_receiver.rs` の 4 箇所のみ。`SrtConnection::relative_timestamp` の `u64` → `u32` 切り捨て (極端に大きい `now` でワイヤタイムスタンプが `0xFFFF_FFFF` になり受信側の wrapping period 判定を誤起動させる) は別の問題として本 issue では扱わない

## 完了条件

- `src/srt_receiver.rs` の上記 4 箇所の加算が飽和演算になっていること
- `tsbpd_time_base` を `u64::MAX - 1_000_000` として `ReceiverBuffer::new` し、`packet.timestamp` が 5_000_000 のデータパケットを `receive` しても panic せず、配信時刻が `u64::MAX` に飽和することを検証する単体テストが `src/srt_receiver.rs` の `#[cfg(test)]` に追加されていること
- `pop_ready` の `tsbpd_time_base += MAX_TIMESTAMP + 1` が飽和し、その後の配信時刻計算でも panic しないことを検証するテストが追加されていること
- `cargo test --workspace` と `cargo test --release --workspace` の両方が通過すること (debug のみ・release のみで挙動が分岐しないこと)
- `CHANGES.md` の `## develop` に `[FIX]` エントリが追加されていること

## テスト

境界値・極値の検証であり、`pbt/` 側で扱う性質 (任意入力の等価性) とは役割が異なる。`ReceiverBuffer::new` は公開 API なので `tests/test_srt_receiver.rs` に書くこともできるが、既存の `ReceiverBuffer` テストが `src/srt_receiver.rs` の `#[cfg(test)]` にある慣習に揃える。実装時に 0031 (インラインテストの移設) の方針と整合させること。

## CHANGES.md

- [FIX] TSBPD 配信時刻の計算が境界値を超える時刻入力でオーバーフローする問題を修正する
  - @voluntas

## 相互作用

- issue 0058 (`add_millis` のオーバーフロー修正) で `add_millis` 自体は panic しなくなったが、返された `u64::MAX` を `now` として受け取る本 issue の経路が残る。本 issue が対応してから初めて「`add_millis` 経由で壊れない」と言い切れる
- issue 0021 / 0044 で確立した TSBPD wrapping period の開始・終了判定 (`src/srt_receiver.rs` の `wrapping_period_active` と `pop_ready` の終了判定) と同じ箇所を触る。判定の仕様上の意味を変えないこと
- issue 0059 / 0074 / 0055 / 0073 / 0028 は `src/srt_receiver.rs` の `receive` 周辺を触るため、実装は直列にする
