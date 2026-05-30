# find_deliverable_seq がシーケンス番号ラップアラウンド境界で配送順序を誤る

- Priority: High
- Created: 2026-05-14
- Completed: 2026-05-30
- Polished: 2026-05-30
- Model: DeepSeek V4 Pro
- Branch: feature/fix-deliverable-seq-wrap-around

## 目的

`src/srt_receiver.rs` の `find_deliverable_seq` / `pop_ready` が配信ポインタを持たず、`packets` (`BTreeMap<u32, ReceivedPacket>`) の u32 数値順イテレーションに依存している。この数値順は 31-bit シーケンス番号の循環順とラップアラウンド境界 (0x7FFF_FFFF → 0) で食い違うため、境界をまたぐパケットが揃ったときに誤った順序で上位アプリへ配送される。TSBPD のシーケンス順配信保証が破れる。

## 優先度根拠

ラップ境界をまたぐ連続パケットが配送されるとき、シーケンス順が逆転し上位アプリに誤った順序でデータが渡る (サイレントなデータ破損)。31-bit シーケンス番号空間 (2^31 パケット) の一巡はパケットレート依存で、例えば 1,000 packets/s で約 25 日、100,000 packets/s で約 6 時間。長時間接続では必ず一度は境界を踏むため High とする。

注: 本 issue の旧版は「約 71.6 分ごとに発生する」としていたが、これは 32-bit タイムスタンプのラップ周期 (TSBPD wrapping period、closed #0014 で対応済み) であり、31-bit シーケンス番号のラップとは無関係なため削除した。

## 現状

```rust
// src/srt_receiver.rs:498-515
pub fn pop_ready(&mut self, now: Timestamp) -> Option<DataPacket> {
    let delivery_seq = self.find_deliverable_seq(now)?;
    self.packets.remove(&delivery_seq).map(|e| e.packet)
}

fn find_deliverable_seq(&self, now: Timestamp) -> Option<u32> {
    for (&seq, entry) in &self.packets {  // BTreeMap の u32 数値順
        let time_ok = !self.tsbpd_enabled || entry.delivery_time <= now;
        let has_gap = self.loss_list.iter().any(|&s| sequence_less_than(s, seq));
        if time_ok && !has_gap {
            return Some(seq);
        }
    }
    None
}
```

この設計は配信ポインタを持たず、毎回 `packets` を BTreeMap の u32 数値順に走査して「先行する損失 (`loss_list`) が無い最小数値 seq」を配信する。`loss_list` による HoL ブロッキング自体は機能しているが、配信候補の選択が u32 数値順に依存しているため、31-bit シーケンス番号の循環順とラップ境界で食い違って破綻する。

### 再現条件

ラップ境界をまたぐ連続パケットが欠損なく全て揃っている場合 (loss_list が空) に発生する。

例: `expected_seq = 0x7FFF_FFFE` の状態で `0x7FFF_FFFE`、`0x7FFF_FFFF`、`0`、`1` を受信する (順不同でよい)。

- `packets` の BTreeMap 数値順は `[0, 1, 0x7FFF_FFFE, 0x7FFF_FFFF]`
- 欠損がないため `loss_list` は空で `has_gap` は常に false
- `find_deliverable_seq` は最小数値の `seq = 0` を最初に返す
- 結果、配送順は `0, 1, 0x7FFF_FFFE, 0x7FFF_FFFF` となり、正しい循環順 `0x7FFF_FFFE, 0x7FFF_FFFF, 0, 1` と一致しない

旧版が挙げていた例 `[0x7FFF_FFFE, 0, 1]` (中間の `0x7FFF_FFFF` が欠損) では、`receive()` が中間の `0x7FFF_FFFF` を `loss_list` に登録するため `has_gap` が `0` と `1` を正しくブロックし、バグは顕在化しない。バグは欠損ゼロのラップ連続区間でのみ起きるため、再現条件を上記に修正した。

なお、目的で挙げた「先行する未配送パケットがバッファ内に存在するケースを `has_gap` が検出できない」点も、配信候補を u32 数値順で選ぶという同じ根本原因によるものであり、下記の循環順最小選択により同時に解消される。

## 根拠

draft-sharabayko-srt.md より引用する (行番号は将来変わりうる)。

シーケンス番号の定義 (「Data packet structure」セクション、377-378 行付近):

> Packet Sequence Number: 31 bits. The sequential number of the data packet. Range [0; 2^31 - 1].

TSBPD によるシーケンス順配信 (「Live Streaming」セクション、3246-3253 行):

> As TSBPD is enabled, the receiver will still deliver packets in order, but based on the timestamps. In the case of a packet arriving too late and skipped by the TLPKTDROP mechanism, the order of delivery is still maintained except for potential sequence discontinuity.

実装側の `src/srt_packet.rs` の `sequence_less_than` は 31-bit 循環比較 (`diff = b.wrapping_sub(a) & 0x7FFF_FFFF; diff > 0 && diff < 0x4000_0000`) を実装済みで、これを配信候補の順序判定に用いる。

## 設計方針

現状の配信セマンティクス (先行する損失が `loss_list` に残る間は配信を止め、損失が再送到着または `drop_too_late` によるドロップで `loss_list` から消えたら配信を再開する) を保持したまま、BTreeMap の数値順への依存だけを除く。`find_deliverable_seq` が配信候補の中から「循環順で最小」の seq を選ぶように修正する。

```rust
fn find_deliverable_seq(&self, now: Timestamp) -> Option<u32> {
    let mut best: Option<u32> = None;
    for (&seq, entry) in &self.packets {
        let time_ok = !self.tsbpd_enabled || entry.delivery_time <= now;
        let has_gap = self.loss_list.iter().any(|&s| sequence_less_than(s, seq));
        if time_ok && !has_gap {
            // 循環順で最小の seq を配信候補とする (BTreeMap の数値順に依存しない)
            best = match best {
                Some(b) if sequence_less_than(b, seq) => Some(b),
                _ => Some(seq),
            };
        }
    }
    best
}
```

これにより:

- ラップ境界をまたぐ連続パケット `[0x7FFF_FFFE, 0x7FFF_FFFF, 0, 1]` でも、循環順で最小の `0x7FFF_FFFE` が最初に選ばれ、正しい順序で配送される
- 目的で挙げた「先行する未配送パケットがバッファ内に存在するケースを `has_gap` が検出できない」点も、循環順最小選択により先行パケットが先に配送されるため解消される
- `loss_list` による HoL ブロッキングと、`drop_too_late` が穴を `loss_list` から消した後に後続を配信する「穴スキップ」という現状セマンティクスは保持される

### 配信ポインタ (delivery_head) 方式を採らない理由

配信ポインタ `delivery_head` を導入し `pop_ready` をポインタ駆動にする案も考えられるが、採らない。この方式は `delivery_head` が損失パケットを指したときの前進 (穴スキップ) を別途実装する必要があり、それは TLPKTDROP (時刻ベースのドロップ) のセマンティクスそのもので #0024 / closed #0015 の範疇になる。本 issue から穴スキップを切り離すと、`drop_too_late` が穴を `loss_list` から落とした後に配信ポインタが前進できず、欠落区間以降のパケットが恒久的に取り残される機能回帰が生じる。本 issue はラップ境界の順序誤りに限定するため、現状セマンティクスをそのまま保持する循環順最小選択を採る。

### 前提

`sequence_less_than` による「循環順最小」は、バッファ内の全パケットが循環空間の半窓 (0x4000_0000) 内に収まっていることを前提とする。受信バッファのパケットは `expected_seq` 近傍のフロー制御ウィンドウ (通常数千〜数万パケットで半窓よりはるかに小さい) 内にあり、加えて TSBPD 有効時は半窓近く離れたパケットの配信時刻が遥か未来で `time_ok` にならず、TSBPD 無効時は穴が HoL ブロックを続けるため、実用上この前提は満たされる。

ただし `receive()` は受信シーケンスの上限方向の admission control を持たない (重複と「古すぎる」のチェックのみ)。理論上はバッファ内 spread が半窓を超える構成が作れるが、上記の理由で実用上は発生しない。防御的な上限受理制限 (半窓超えの seq を拒否する) を入れるかは本 issue のスコープ外とし、必要なら別 issue で扱う。

closed #0006 は送信側 `handle_ack` で同型のバグ (BTreeMap 数値順と 31-bit 循環順の不一致) を循環順での評価で修正済みで、本 issue は受信側の姉妹バグである。

## スコープ

本 issue は「受信済みパケットがラップ境界で誤順序配送される」修正に限定する。損失パケットのドロップ後の穴スキップは現状どおり `loss_list` 経由で機能し続ける (設計方針参照)。`drop_too_late` の推定配信時刻の正確化は別 issue #0024、TLPKTDROP の接続コードへの統合は closed #0015 の範疇であり、本 issue では触らない。

## 相互作用 (確認済み)

- `receive()` / `expected_seq` 更新 / ACK 生成 / `loss_list` / NAK / `drop_too_late`: いずれも変更しない。本修正は `find_deliverable_seq` の配信候補選択ロジックのみを変える
- `pop_ready` の呼び出し元 (`src/srt_connection.rs` の 3 箇所): シグネチャ不変、無変更で済む
- 同じ `srt_receiver.rs` を触る #0021 / #0044 (wrapping period 終了判定を `pop_ready` へ移動) とは `pop_ready` / `find_deliverable_seq` 周辺で競合しうる。番号順 (#0018 → #0021) で対応すること
- 公開 API は不変で、内部の配送順序の正常化のみ。後方互換は保たれ、変更種別は `[FIX]`

## テスト戦略

ラップ境界をまたぐ配送順序と、ドロップ後の穴スキップが維持されることを検証する。既存の配送系 PBT (`pbt/tests/prop_receiver.rs`) は `initial_seq` を `0..0x7FFF_FF00` に限定しラップ近傍を除外しているため、この回帰を検出できない。

- PBT (`pbt/tests/prop_receiver.rs`): `initial_seq` をラップ境界近傍に固定し、境界をまたぐ連続パケットを順不同 (proptest の `prop_shuffle` 等) で `receive` させ、`pop_ready` の取り出し列が循環順 (`initial_seq` から `wrapping_add` した列) と一致することを検証する。tsbpd 有効・無効の両方を含める
- 単体テスト (`src/srt_receiver.rs` の `#[cfg(test)] mod tests`): 上記の再現条件 (`0x7FFF_FFFE, 0x7FFF_FFFF, 0, 1` を順不同受信 → 循環順で配送) を固定値で検証する。ラップ境界は意図的な境界値ケースであり、PBT より単体テスト向き

## 修正対象

1. `src/srt_receiver.rs` の `find_deliverable_seq` を、配信候補から循環順最小の seq を選ぶロジックに修正する
2. `pbt/tests/prop_receiver.rs` にラップ境界をまたぐ配送順序の PBT を追加する
3. `src/srt_receiver.rs` のインラインテストに再現条件の単体テストを追加する
4. `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加する
   - 例: `[FIX] 受信バッファがシーケンス番号ラップアラウンド境界でパケットを誤った順序で配送する問題を修正する`

## 完了条件

- `find_deliverable_seq` が配信候補から循環順最小の seq を選ぶように修正されていること
- ラップ境界をまたぐ連続パケットが循環順で配送されること (tsbpd 有効・無効の両方)
- `loss_list` に損失が残る間はそれより後ろのパケットを配信しない既存挙動が維持されていること
- `drop_too_late` が穴を落とした後に後続パケットが配信される既存挙動が維持されていること
- `CHANGES.md` に `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/srt_receiver.rs` の `find_deliverable_seq` を、配信候補 (時刻 OK かつ `has_gap` でない seq) の中から `sequence_less_than` による循環順で最小の seq を選ぶよう修正した。BTreeMap の u32 数値順への依存を除き、ラップアラウンド境界をまたぐ連続パケットを循環順で配送する。`loss_list` による HoL ブロッキングと `drop_too_late` の穴スキップのセマンティクスは変更していない。コメントに一次資料 (Live Streaming セクションの順序配信要件) と、早期 return せず全候補を走査する理由を明記した。

### テスト

完了条件の各項目を、PBT と単体テストで役割分担して検証する。

- PBT (`pbt/tests/prop_receiver.rs` の `test_pop_ready_wrap_around_delivery_order`): `wrap_around_run` Strategy で必ずラップ境界をまたぐ連続列を生成し、`prop_shuffle` で順不同に受信させ、`pop_ready` の取り出し列が循環順と一致することを検証する。tsbpd 有効・無効の両方を含む。既存の配送系 PBT は `initial_seq` をラップ近傍から除外しておりこの回帰を検出できないため、本 PBT を追加した。
- 単体テスト (`src/srt_receiver.rs` の `#[cfg(test)] mod tests`): PBT で組みにくい損失ありの境界ケースを置く。`test_pop_ready_blocks_on_loss_across_wrap_boundary` で損失残存時の HoL ブロッキング維持を、`test_pop_ready_skips_hole_after_drop_across_wrap_boundary` で `drop_too_late` による穴除去後の循環順配送を検証する。

`CHANGES.md` の `## develop` に `[FIX]` エントリを追加した。
