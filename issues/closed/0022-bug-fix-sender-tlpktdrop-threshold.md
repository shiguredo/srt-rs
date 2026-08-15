# Sender 側 drop_expired の TLPKTDROP 閾値が仕様の推奨値に従っていない

- Priority: Medium
- Created: 2026-05-14
- Completed: 2026-08-15
- Model: DeepSeek V4 Pro
- Branch: feature/fix-sender-tlpktdrop-threshold
- Polished: 2026-08-15

## 解決方法

- `drop_expired()` の閾値を `self.latency_us` から `max(latency_us * 125 / 100, 1_000_000)` に変更した
- PBT テスト (`test_sender_buffer_drop_expired`) の生成範囲を `10u16..1000u16` に拡張し、`before_expire`/`after_expire` の計算を新閾値ベースに更新した
- 1 秒下限・1.25 倍側・境界値 (`latency_ms = 800`) の単体テストを `src/srt_sender.rs` の `#[cfg(test)]` モジュールに追加した
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加した

## 目的

`src/srt_sender.rs` の `drop_expired()` が、送信側の期限切れ判定に `self.latency_us` をそのまま閾値として使っている。draft-sharabayko-srt.md の `#too-late-packet-drop` 節では `TLPKTDROP_THRESHOLD` として SRT latency の 1.25 倍、かつ 1 秒以上を推奨しており、受信側 (`src/srt_receiver.rs` の `drop_too_late()` 内の `tlpktdrop_threshold` 計算) はこの推奨どおりに実装済みである。

## 優先度根拠

送信側の方が短い閾値でパケットを破棄してしまい、再送に必要なパケットを早期に捨てる可能性がある。なお本ライブラリでは TLPKTDROP フラグは常時設定され、`drop_expired()` もフラグ判定なしで常時呼ばれるため、実質常時有効である。ただし実際に drop が発動するのは ACK が長時間届かず再送も進まない高損失時に限られる (再送時には `sent_time` が更新されるため) ことから Medium。

## 現状

`drop_expired()` は送信時刻からの経過時間が `self.latency_us` を超えたパケットを期限切れとして破棄する (`srt_connection.rs` の `handle_timer` 内の ACK タイマーから 10ms 周期で呼び出される):

```rust
.filter_map(|(&seq, entry)| {
    let elapsed = now.as_micros().saturating_sub(entry.sent_time.as_micros());
    if elapsed > self.latency_us {
        Some(seq)
    } else {
        None
    }
})
```

受信側の `drop_too_late()` は `max(1.25 * tsbpd_delay_us, 1_000_000)` を閾値としており、送信側と受信側で閾値が非対称になっている。

## 根拠

draft-sharabayko-srt.md の `#too-late-packet-drop` 節:

> The recommended threshold value is 1.25 times the SRT latency value.
>
> Note that the SRT sender keeps packets for at least 1 second in case the latency is not high enough for a large RTT (that is, if TLPKTDROP_THRESHOLD is less than 1 second).

仕様は「定義」ではなく「推奨」であり、`max(1.25 * latency, 1_000_000)` はこの 2 つの記述から導出される。導出結果は受信側実装 (closed 0008 で導入済み) と一致する。

## 設計方針

- `self.latency_us` の代わりに `max(latency_us * 125 / 100, 1_000_000)` (µs 単位、受信側の `tlpktdrop_threshold` 計算と同じ `* 125 / 100` と `max` の整数演算。`latency_us` は u16 の ms 値に 1000 を掛けた値であり、`latency_us * 125` は最大 `65_535_000 * 125 = 8_191_875_000` で `u64` に収まるため `as u128` キャストは不要。`latency_us` は構築後不変のため、`drop_expired()` 呼び出しのたびに計算するか事前計算するかは実装者の判断に委ねる) を閾値として使用する
- 判定基準時刻は変更しない (現状の `sent_time` 基準のまま。再送時に `sent_time` を更新する既存挙動も維持する)。仕様の「packet timestamp 基準」への変更は本 issue のスコープ外とする。本ライブラリでは再送時に `sent_time` が更新されるため、`sent_time` 基準でも実質的な再送猶予は確保されており、短い閾値による早期破棄の方が問題として大きいため優先度は閾値修正に置く

## 完了条件

- `drop_expired()` の閾値が `max(latency_us * 125 / 100, 1_000_000)` になっていること
- 既存の `test_sender_buffer_drop_expired` (pbt/tests/prop_sender.rs) が新閾値に合わせて更新されていること。`latency_ms` の生成範囲を `10u16..1000u16` に拡張し、`before_expire` と `after_expire` の時刻計算を `max(latency_us * 125 / 100, 1_000_000) - 1000` と `max(latency_us * 125 / 100, 1_000_000) + 1000` に変更すること (テスト側では `latency_us` は private フィールドのため `latency_ms as u64 * 1000` に展開して使用する)
- 閾値の境界 (`latency_ms = 800` で 1 秒下限と 1.25 倍側が切り替わる) を検証するテストが `src/srt_sender.rs` の `#[cfg(test)]` モジュールに追加されていること。`latency_ms = 800` で `elapsed = 1_000_000` のとき drop されない (等号)、`elapsed = 1_000_001` のとき drop されることを確認する
- drop 判定の排他性 (`elapsed > threshold` の不等号) を検証するテストが `src/srt_sender.rs` の `#[cfg(test)]` モジュールに追加されていること。`latency_ms = 1000` で `elapsed = 1_250_000` のとき drop されない (等号)、`elapsed = 1_250_001` のとき drop されることを確認する
- `cargo test` で全テストが通過すること
- CHANGES.md の `## develop` セクションに `[FIX]` エントリ (`[FIX] drop_expired の TLPKTDROP 閾値を仕様の推奨値 (max(1.25 * latency, 1 秒)) に合わせる`。担当者行を付けて追加すること) を追加すること
