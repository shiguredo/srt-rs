# drop_too_late の未受信パケット推定配送時刻がタイムスタンプを含まない

- Priority: Medium
- Created: 2026-05-14
- Completed: 2026-08-15
- Model: DeepSeek V4 Pro
- Branch: feature/fix-drop-too-late-timestamp-estimation
- Polished: 2026-08-15

## 目的

`src/srt_receiver.rs` の `drop_too_late()` で、パケットが一度も受信されていない場合のフォールバック式が `tsbpd_time_base + tsbpd_delay_us` で、パケットタイムスタンプを含んでいない。受信済みパケットの配信時刻は仕様の PktTsbpdTime 式 (`#packet-delivery-time` 節) に従い `tsbpd_time_base + タイムスタンプ + tsbpd_delay_us` で計算されるため、フォールバックはタイムスタンプ 0 相当の過小評価になる。

## 優先度根拠

配送時刻を過小評価し、本来削除すべきでないパケットを早期削除する可能性がある。現行のフォールバックはタイムスタンプ 0 相当の推定であるため、実際のタイムスタンプが大きいほど過小評価の幅が広がり、損失パケットは再送の到着より先にドロップされやすい (NAK は損失検出時に即時送信されるのに対し、`drop_too_late()` は ACK タイマー (10ms 周期) で呼ばれる)。ドロップの有害な結果 (再送機会の喪失による欠落) が顕在化するのは高損失時であるため Medium。

## 現状

`drop_too_late()` 内の損失リストに対するフィルタで、フォールバック式がパケットタイムスタンプを考慮していない:

```rust
.filter(|&seq| {
    let estimated_delivery = self.packets
        .get(&seq)
        .map(|p| p.delivery_time.as_micros())
        .unwrap_or_else(|| self.tsbpd_time_base + self.tsbpd_delay_us);
    now.as_micros() > estimated_delivery + tlpktdrop_threshold
})
```

なお、`receive()` は受信済みパケットを `loss_list` から除去するため、`.map(|p| p.delivery_time.as_micros())` 分岐は実質的に使われず、フォールバックが常に選択される。

## 根拠

draft-sharabayko-srt.md の `#packet-delivery-time` 節 (「Packet Delivery Time」):

> The calculation of packet delivery time (PktTsbpdTime) is performed upon receiving a data packet according to the following formula:
>
> ~~~
> PktTsbpdTime = TsbpdTimeBase + PKT_TIMESTAMP + TsbpdDelay + Drift
> ~~~

仕様の式は受信済みパケットに対する定義であり、未受信パケットの推定配信時刻の計算方式は仕様に定義されていない。本 issue はこの式を未受信パケットへ拡張適用する設計判断である。

## 設計方針

- 欠損パケットの推定配信時刻は、循環順で欠損 seq より大きい最小の受信済み seq (次側) の `delivery_time` をそのまま使用する。探索は `self.packets` (BTreeMap) から数値順で seq より大きい最初の要素を取得し、なければ最小の要素を取る 2 段階で行う。`delivery_time` は受信時に固定計算され 0021 のラップ補正が反映されるため、`tsbpd_time_base` からの再計算は行わない (再計算すると 0021 実装後の `tsbpd_time_base` 更新タイミング (配信時) と食い違い、ラップ境界で誤差が生じる)。次側の `delivery_time` は欠損パケットの真の配信時刻以上であるため、この推定は削除が遅れる側 (過大評価側) になる。タイムスタンプは実用上単調増加する (SRT ライブストリーミングでは送信元のオリジン時刻) ため、次側のタイムスタンプ >= 欠損パケットのタイムスタンプが前提として成り立つ
- 次側の受信パケットは、欠損検出のトリガーとなったパケットが受信済みであること、および配信が HoL ブロッキング (srt_receiver.rs の `find_deliverable_seq()` の `has_gap` 判定) でブロックされることにより、欠損が `loss_list` にある限りバッファに残る。直前側のパケットは配信済みで存在しないことが多いため、次側を基準とする。`loss_list` に複数の欠損がある場合、各欠損 seq に対して個別に次側を探索する (複数の連続欠損が同じ次側を共有する場合も、各欠損の推定は独立に行う)
- なお、次側の受信パケットが存在しない場合は、0021 実装後のフォールバック値 (`tsbpd_time_base + tsbpd_delay_us` + wrapping_period_active の場合は `MAX_TIMESTAMP + 1` の加算) を防御的に維持する (0021 のフォールバック拡張が本 issue の修正で上書きされないよう、0021 の修正を継承する式とする)
- ラップ境界の扱いは 0021 (wrapping period の配信時終了判定とラップ補正) と整合させる。実装順は 0021 → 本 issue とし、0021 のラップ補正が入った後の `delivery_time` と整合する形で推定する (0021 の設計方針にも同旨の記述がある)

## 完了条件

- 欠損パケットの推定配信時刻が次側の受信パケットの `delivery_time` を使用していること
- 次側の選択がシーケンス番号の循環順で行われることを含め、新方式の挙動を検証するテストが追加されていること (ラップ境界での次側選択のケースも含む)
- 既存の `test_drop_too_late_uses_tsbpd_time_base` と `test_drop_too_late_individual_delivery` (src/srt_receiver.rs の `#[cfg(test)]` モジュール) が新方式に合わせて更新されていること。`test_drop_too_late_uses_tsbpd_time_base` では、損失 seq 1000 の推定配信時刻が次側 seq 1001 の `delivery_time` (= `tsbpd_time_base + 200_000 + tsbpd_delay_us`) に変更される。`test_drop_too_late_individual_delivery` では、損失 seq 1001 の推定配信時刻が次側 seq 1002 の `delivery_time` に変更されるため、`now` の値とアサーションの見直しが必要になる
- `cargo test` で全テストが通過すること
- CHANGES.md の `## develop` セクションに `[FIX]` エントリが追加されていること

## 解決方法

- `drop_too_late()` のフォールバック式を、循環順で次側の受信パケットの `delivery_time` を推定値として使用する方式に変更した
- 次側の探索は BTreeMap の `range` で seq より大きい最初の要素を取得し、なければ最小の要素を取る 2 段階で行う
- 次側パケットが存在しない場合の防御的フォールバックは維持し、0021 のラップ補正を継承する
- 既存テストの期待値を新方式に合わせて更新した
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加した
