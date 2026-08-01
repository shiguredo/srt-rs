# Sender 側 drop_expired の TLPKTDROP 閾値が仕様の推奨値に従っていない

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-sender-tlpktdrop-threshold
- Polished: 2026-07-31

## 目的

`src/srt_sender.rs` の `drop_expired()` が、送信側の期限切れ判定に `self.latency_us` をそのまま閾値として使っている。draft-sharabayko-srt.md の `#too-late-packet-drop` 節では `TLPKTDROP_THRESHOLD` として SRT latency の 1.25 倍、かつ 1 秒以上を推奨しており、受信側 (`src/srt_receiver.rs` の `drop_too_late()` 内の `tlpktdrop_threshold` 計算) はこの推奨どおりに実装済みである。

## 優先度根拠

送信側の方が短い閾値でパケットを破棄してしまい、再送に必要なパケットを早期に捨てる可能性がある。なお本ライブラリでは TLPKTDROP フラグは常時設定され、`drop_expired()` もフラグ判定なしで常時呼ばれるため、実質常時有効である。ただし実際に drop が発動するのは ACK が長時間届かず再送も進まない高損失時に限られる (再送時には `sent_time` が更新されるため) ことから Medium。

## 現状

`drop_expired()` は送信時刻からの経過時間が `self.latency_us` を超えたパケットを期限切れとして破棄する:

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

- `self.latency_us` の代わりに `max(latency_us * 125 / 100, 1_000_000)` (µs 単位、受信側の `tlpktdrop_threshold` 計算と同じ `* 125 / 100` と `max` の整数演算) を閾値として使用する。`latency_us` は u16 の ms 値に 1000 を掛けた値であり、この演算でオーバーフローしない
- 判定基準時刻は変更しない (現状の `sent_time` 基準のまま。再送時に `sent_time` を更新する既存挙動も維持する)。仕様の「packet timestamp 基準」への変更は本 issue のスコープ外とする

## 完了条件

- `drop_expired()` の閾値が `max(latency_us * 125 / 100, 1_000_000)` になっていること
- 既存の `test_sender_buffer_drop_expired` (pbt/tests/prop_sender.rs) が新閾値に合わせて更新されていること (現状の生成範囲 `latency_ms in 10u16..100u16` では常に 1 秒下限が支配的になるため、1.25 倍側の境界を検証するには 800ms 超の生成範囲も必要になる)
- 閾値の境界 (1.25 倍側と 1 秒下限の切り替わり) と drop 判定の排他性 (`elapsed > threshold` の不等号) を検証するテストが追加されていること
- `cargo test` で全テストが通過すること
- CHANGES.md の `## develop` セクションに `[FIX]` エントリが追加されていること
