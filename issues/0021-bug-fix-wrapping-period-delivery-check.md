# TSBPD wrapping period の終了判定がパケット配信時ではなく受信時に行われている

- Priority: Medium
- Created: 2026-05-14
- Completed: 2026-08-15
- Model: DeepSeek V4 Pro
- Branch: feature/fix-wrapping-period-delivery-check
- Polished: 2026-08-15

## 解決方法

- `receive()` から wrapping period 終了判定を削除し、開始判定のみを残した
- `pop_ready()` に終了判定を移動し、配信パケットのタイムスタンプが `(WRAPPING_PERIOD_END_MIN..=WRAPPING_PERIOD_END_MAX)` の範囲内かつ `wrapping_period_active` が有効な場合に `tsbpd_time_base += MAX_TIMESTAMP + 1` を実行する実装を追加した
- `receive()` 内でラップ後パケット (`wrapping_period_active` が有効かつ `ts < WRAPPING_PERIOD_START`) の配信時刻に `MAX_TIMESTAMP + 1` を加算する補正を導入した
- `drop_too_late()` のフォールバック式に `wrapping_period_active` が有効中の `MAX_TIMESTAMP + 1` 加算を追加した
- ラップ後パケットの配信時刻補正、`drop_too_late()` のフォールバック補正、`pop_ready()` 内の終了判定、TSBPD 無効時の終了判定非発火を検証するテストを追加した
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加した

## 目的

`src/srt_receiver.rs` の `receive()` が wrapping period の終了判定をパケット受信時に行っているが、SRT 仕様 (draft-sharabayko-srt.md の `#tsbpd-time-base` 節) では「パケットが配信された時点 (delivered, read from the buffer)」で wrapping period を終了すると定義されており、仕様と乖離している。受信時終了判定は 0014 (TSBPD wrapping period の追加) が導入したものであり、本 issue はその設計を仕様文言に基づいて修正する。

## 優先度根拠

受信時判定では、ラップ後のパケットの配信時刻が正しく計算されない問題を起こす。ラップ後 `ts` が 30 秒未満のパケットは終了判定が発火する前に旧 `tsbpd_time_base` で配信時刻が計算され、実時刻より約 71.6 分 (`MAX_TIMESTAMP` + 1 = 4,294,967,296 μs) 過去にずれるため遅延が実質的に無効化され即時配信される。ラップ境界は 1 時間 11 分 35 秒 (71.58 分) ごとのタイムスタンプのラップアラウンド時にしか訪れず、即時の破綻には至らないため Medium。なお、ラップ後のパケットより遅れて到着したラップ前パケットの配信時刻が約 71.6 分未来になる問題は、終了判定の発火後に到着したラップ前パケットが次周期の通常のラップ前パケット (同じく `ts` >= `WRAPPING_PERIOD_START` を持ち、開始判定を同じ状態で再発火させるパケット) と原理的に区別できないため本 issue のスコープ外とする。

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

終了判定で `tsbpd_time_base` を更新した後に配信時刻 (`delivery_time`) を計算するため、ラップ後 `ts` が 30 〜 60 秒のパケットの配信時刻は正しく計算されるが、上記の「優先度根拠」の問題を残す。

## 根拠

draft-sharabayko-srt.md の `#tsbpd-time-base` 節 (「TSBPD Time Base Calculation」):

> The TSBPD wrapping period starts 30 seconds before reaching the maximum timestamp value of a packet and ends once the packet with timestamp within (30, 60) seconds interval is delivered (read from the buffer). The updated value of TsbpdTimeBase will be recalculated as follows:
>
> ~~~
> TsbpdTimeBase = TsbpdTimeBase + MAX_TIMESTAMP + 1
> ~~~

また、同仕様の `#packet-delivery-time` 節では配信時刻 (PktTsbpdTime) は「データパケットの受信時に計算される (performed upon receiving a data packet)」と定義されており、現行実装も `receive()` 内で `delivery_time` を受信時に固定計算する。このため終了判定を配信時に移動すると、ラップ後パケットの配信時刻補正が別途必要になる (設計方針参照)。

## 設計方針

- wrapping period の終了判定と `tsbpd_time_base` の更新を `pop_ready()` に移動する。`receive()` には開始判定のみ残す。`pop_ready()` 内では `self.packets.remove(&delivery_seq)` で取得した `ReceivedPacket` から `packet.timestamp` を参照し、`ts` が `(WRAPPING_PERIOD_END_MIN..=WRAPPING_PERIOD_END_MAX)` の範囲内かつ `wrapping_period_active` が有効な場合に `tsbpd_time_base += MAX_TIMESTAMP + 1` を実行して `wrapping_period_active` を false にする。`pop_ready()` 内の終了判定は現行の開始判定と同様に TSBPD 有効を条件に実行すること (`handle_shutdown()` が TSBPD 無効化後に `pop_ready()` でバッファをフラッシュするパスでは終了判定を発火させない。いずれでも実害はないが挙動を確定させる)。なお、現行の受信時終了判定と異なり、終了窗口パケット (ts が 30〜60 秒) が受信済みでも `find_deliverable_seq()` の HoL ブロッキング (損失リストによる gap 判定) により配信が遅れると、その分だけ終了判定も遅れる。これは仕様の「is delivered (read from the buffer)」に従うための動作変更であり、受信時終了判定よりも安全側に倒れる
- `delivery_time` は `receive()` 内で固定計算されるため、終了判定の移動だけではラップ後のパケットの配信時刻が旧 `tsbpd_time_base` で計算されたままになる。ラップ後パケットには配信時刻に `MAX_TIMESTAMP` + 1 を加算する補正を導入し、ラップ境界で配信時刻の計算を整合させること。ラップ後パケットの判定条件は「`wrapping_period_active` が有効な間に受信した、`ts` が `WRAPPING_PERIOD_START` 未満のパケット」を基準に設計すること (ラップ前パケットの `ts` は `WRAPPING_PERIOD_START` 以上であるため衝突しない。`ts` の上限 (`WRAPPING_PERIOD_END_MAX`) で判定すると TSBPD 遅延が 30 秒を超える構成でラップ後パケットを取りこぼす)。なお、`receive()` 内のコード順序では開始判定 (`ts >= WRAPPING_PERIOD_START` のチェック) が配信時刻計算 (`delivery_time` の計算) より前に実行されるため、開始判定をトリガーしたパケット自身 (`ts >= WRAPPING_PERIOD_START`) も `wrapping_period_active = true` の状態で配信時刻計算を通過するが、`ts < WRAPPING_PERIOD_START` の条件により補正は誤適用されない。この順序依存関係は実装時に注意すること
- `drop_too_late()` の未受信パケットの推定配信時刻 (srt_receiver.rs の `drop_too_late` 内のフォールバック値 `tsbpd_time_base + tsbpd_delay_us`) は `tsbpd_time_base` を直接参照するため、終了判定の移動で更新が遅れるとラップ後の損失パケットを約 71.6 分過去と推定して早期にドロップする。損失パケットはタイムスタンプを持たないため、加算の判定基準は `wrapping_period_active` が有効中かどうかとし、有効中はフォールバック式を `tsbpd_time_base + tsbpd_delay_us + MAX_TIMESTAMP + 1` に拡張する (有効中はラップ前損失パケットは最大約 30 秒の過大推定 (ドロップが遅れる安全側) になり、ラップ後損失パケットは base 更新後と同じ推定になり約 71.6 分の過小推定を避ける)。このフォールバック式は 0024 の修正対象と同一であり、本 issue を先に実装してから 0024 で推定方法を修正する
- 終了条件の境界値 (60 秒上限の開区間化) の見直しは 0044 のスコープであり、本 issue では判定式を変更せず移動と配信時刻の整合のみを行う。実装順は本 issue → 0044
- 終了判定の発火後に遅延到着したラップ前パケットで開始判定が再発火し、`tsbpd_time_base` が二重に加算される問題は既存の問題であり、本 issue では扱わない
- ラップ前窗口 (`ts` >= `WRAPPING_PERIOD_START`) のパケットが全て損失して開始判定が一度も発火しない場合、`wrapping_period_active` が有効化されず配信時刻補正と `tsbpd_time_base` の加算の両方が効かないが、これは現行実装と同じ挙動であり本 issue のスコープ外とする
- 境界値テストの追加は 0036 のスコープであり、本 issue はラップ後パケットの配信時刻の整合と `drop_too_late()` の挙動の検証テストに絞る。また、0028 (`receive()` の責務分割) は本 issue の後に実装する (本 issue が wrapping 管理と配信時刻計算の実装を変更するため)

## CHANGES.md

バグ修正のため、`## develop` セクションに `[FIX]` エントリ (例: `[FIX] TSBPD wrapping period の終了判定を受信時ではなくパケット配信時に行うよう修正し、ラップ境界の配信時刻計算を整合させる`。担当者行を付けて追加すること) を追加する。

## 完了条件

- wrapping period の終了判定と `tsbpd_time_base` の更新が `pop_ready()` で行われ、`receive()` に終了判定が残っていないこと (開始判定のみ残る)。`pop_ready()` 内の終了判定は TSBPD 有効時のみ実行されること
- `receive()` 内でラップ後パケット (`wrapping_period_active` が有効かつ `ts` < `WRAPPING_PERIOD_START`) の配信時刻に `MAX_TIMESTAMP + 1` が加算されること
- `drop_too_late()` のフォールバック推定式が `wrapping_period_active` の有効中に `MAX_TIMESTAMP + 1` を加算するよう修正されていること
- ラップ後パケットの配信時刻が正しく計算され、配信タイミングと `drop_too_late()` のドロップ判定がラップ境界で崩れないことを検証するテストが追加されていること
- `cargo test` で全テストが通過すること
- CHANGES.md の `## develop` セクションに `[FIX]` エントリが追加されていること
