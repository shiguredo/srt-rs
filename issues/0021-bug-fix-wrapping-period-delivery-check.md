# TSBPD wrapping period の終了判定がパケット配信時ではなく受信時に行われている

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-wrapping-period-delivery-check
- Polished: 2026-07-31

## 目的

`src/srt_receiver.rs` の `receive()` が wrapping period の終了判定をパケット受信時に行っているが、SRT 仕様 (draft-sharabayko-srt.md の `#tsbpd-time-base` 節) では「パケットが配信された時点 (delivered, read from the buffer)」で wrapping period を終了すると定義されており、仕様と乖離している。

## 優先度根拠

受信時判定では、ラップ後のパケットの配信時刻が正しく計算されない問題を起こす。ラップ後 ts が 30 秒未満のパケットは終了判定が発火する前に旧 `tsbpd_time_base` で配信時刻が計算され、実時刻より約 71.6 分 (MAX_TIMESTAMP + 1) 過去になるため TSBPD 遅延なしで即配信される。ラップ境界は 01:11:35 時間ごとにしか訪れず、即時の破綻には至らないため Medium。なお、ラップ後のパケットより遅れて到着したラップ前パケットの配信時刻が約 71.6 分未来になる問題は、ラップ後パケットと原理的に区別できないため本 issue のスコープ外とする。

## 現状

`receive()` 内の TSBPD ラップアラウンド期間チェックで、開始判定と終了判定の両方を行っている:

```rust
if self.tsbpd_enabled {
    let ts = packet.timestamp as u64;
    if ts >= WRAPPING_PERIOD_START && !self.wrapping_period_active {
        self.wrapping_period_active = true;
    }
    if self.wrapping_period_active
        && (WRAPPING_PERIOD_END_MIN..=WRAPPING_PERIOD_END_MAX).contains(&ts)
    {
        self.tsbpd_time_base += MAX_TIMESTAMP + 1;
        self.wrapping_period_active = false;
    }
}
```

終了判定で `tsbpd_time_base` を更新した後に配信時刻 (`delivery_time`) を計算するため、ラップ後 ts が 30 〜 60 秒のパケットの配信時刻は正しく計算されるが、上記の「優先度根拠」の問題を残す。

## 根拠

draft-sharabayko-srt.md の `#tsbpd-time-base` 節 (「TSBPD Time Base Calculation」):

> The TSBPD wrapping period starts 30 seconds before reaching the maximum timestamp value of a packet and ends once the packet with timestamp within (30, 60) seconds interval is delivered (read from the buffer). The updated value of TsbpdTimeBase will be recalculated as follows:
>
> ~~~
> TsbpdTimeBase = TsbpdTimeBase + MAX_TIMESTAMP + 1
> ~~~

## 設計方針

- wrapping period の終了判定と `tsbpd_time_base` の更新を `pop_ready()` に移動する。`receive()` には開始判定のみ残す
- `delivery_time` は `receive()` 内で固定計算されるため、終了判定の移動だけではラップ後のパケットの配信時刻が旧 `tsbpd_time_base` で計算されたままになる。ラップ後パケットには配信時刻に MAX_TIMESTAMP + 1 を加算する補正を導入し、ラップ境界で配信時刻の計算を整合させること。ラップ後パケットの判定条件は「`wrapping_period_active` が有効な間に受信した、ts が WRAPPING_PERIOD_START 未満のパケット」を基準に設計すること (ラップ前パケットの ts は WRAPPING_PERIOD_START 以上であるため衝突しない。ts の上限 (WRAPPING_PERIOD_END_MAX) で判定すると TSBPD 遅延が 30 秒を超える構成でラップ後パケットを取りこぼす)
- `drop_too_late()` の未受信パケットの推定配信時刻 (srt_receiver.rs の `drop_too_late` 内のフォールバック値 `tsbpd_time_base + tsbpd_delay_us`) は `tsbpd_time_base` を直接参照するため、終了判定の移動で更新が遅れるとラップ後の損失パケットを約 71.6 分過去と推定して早期にドロップする。ラップ後パケットのフォールバック推定にも MAX_TIMESTAMP + 1 を加算する等、ラップ境界で `drop_too_late()` の挙動も整合させること。このフォールバック式は 0024 の修正対象と同一であり、本 issue を先に実装してから 0024 で推定方法を修正する
- 終了条件の境界値 (60 秒上限の開区間化) の見直しは 0044 のスコープであり、本 issue では判定式を変更せず移動と配信時刻の整合のみを行う。実装順は本 issue → 0044
- 終了判定の発火後に遅延到着したラップ前パケットで開始判定が再発火し、`tsbpd_time_base` が二重に加算される問題は既存の問題であり、本 issue では扱わない
- 境界値テストの追加は 0036 のスコープであり、本 issue はラップ後パケットの配信時刻の整合と `drop_too_late()` の挙動の検証テストに絞る。また、0028 (`receive()` の責務分割) は本 issue の後に実装する (本 issue が wrapping 管理と配信時刻計算の実装を変更するため)

## 完了条件

- wrapping period の終了判定と `tsbpd_time_base` の更新が `pop_ready()` で行われ、`receive()` に終了判定が残っていないこと (開始判定のみ残る)
- ラップ後パケットの配信時刻が正しく計算され、配信タイミングと `drop_too_late()` のドロップ判定がラップ境界で崩れないことを検証するテストが追加されていること
- `cargo test` で全テストが通過すること
