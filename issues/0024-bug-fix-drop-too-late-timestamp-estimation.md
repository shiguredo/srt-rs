# drop_too_late の未受信パケット推定配送時刻がタイムスタンプを含まない

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-drop-too-late-timestamp-estimation

## 目的

`src/srt_receiver.rs:618-621` の `drop_too_late` で、パケットが一度も受信されていない場合のフォールバック式が `tsbpd_time_base + tsbpd_delay_us` で、パケットタイムスタンプを含んでいない。正しい配送時刻の推定には `tsbpd_time_base + <推定タイムスタンプ> + tsbpd_delay_us` が必要。

## 優先度根拠

配送時刻を過小評価し、本来削除すべきでないパケットを早期削除する可能性がある。ただし `loss_list` のエントリは通常短期間で解決されるか、実際に損失したパケットであることが多く、早期削除の影響は限定的。

## 現状

```rust
// src/srt_receiver.rs:618-621
.filter(|&seq| {
    let estimated_delivery = self.packets
        .get(&seq)
        .map(|p| p.delivery_time.as_micros())
        .unwrap_or_else(|| self.tsbpd_time_base + self.tsbpd_delay_us);
    now.as_micros() > estimated_delivery + tlpktdrop_threshold
})
```

フォールバック式がパケットタイムスタンプを考慮していない。

## 設計方針

推定タイムスタンプを近傍の受信パケットから推定するか、最小の `tsbpd_time_base` をフォールバックとしつつも、より安全側（削除が遅れる側）の推定を行う。

## 完了条件

- 未受信パケットの推定配送時刻がタイムスタンプを考慮していること
- `cargo test` で全テストが通過すること
