# receive の損失検出ループで loss_list が大量に肥大化する

- Created: 2026-08-16
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-loss-list-blowup
- Polished: 2026-09-03

## 目的

`src/srt_receiver.rs` の `receive` メソッド内の損失検出ループは、`expected_seq` から半区間内 (差分が 2^30 未満) に離れた seq を持つ 1 パケットを受信すると、`expected_seq` から seq までの全欠損分を `loss_list` に登録する。差分は最大 2^30 - 1 まで取り得るため、1 パケットの受信で約 10 億エントリずつが `loss_list` と `new_losses` の両方に push され、ピークで合計約 8 GiB (u32 4 バイト × 約 2^30 × 2 リスト) のメモリ枯渇を引き起こす可能性がある。

issue 0059 の修正 (受信バッファ `packets` の上限チェック) は `packets` への挿入を制限するものであり、攻撃パケット 1 個は上限未満で挿入されてしまうため、損失検出ループによる `loss_list` の肥大化は防げない。本 issue は 0059 とは別ルートのメモリ枯渇を修正する (0059 の polish 中に分離された)。

## 現状

`receive` (`src/srt_receiver.rs`) の損失検出ループ:

```rust
let mut s = self.expected_seq;
while sequence_less_than(s, seq) {
    if !self.packets.contains_key(&s) && !self.loss_list.contains(&s) {
        new_losses.push(s);
        self.loss_list.push(s);
        self.total_lost += 1;
    }
    s = s.wrapping_add(1) & 0x7FFF_FFFF;
}
```

`sequence_less_than` (`src/srt_packet.rs`) は半区間比較 (差分が 0 より大きく 2^30 未満のとき true) のため、攻撃者が `expected_seq` より 2^30 - 1 進んだ位置の seq を持つ 1 パケットを送ると、ループが約 2^30 回実行され、`loss_list` と `new_losses` の両方に約 2^30 エントリが push される。

`receive` 冒頭の古すぎるパケットの破棄 (`sequence_less_than(seq, self.expected_seq)`) は `expected_seq` との差分が (0, 2^30) の範囲にあるパケットのみを対象とする。半区間以上離れた seq は未来側と判定されて破棄されないため、この攻撃を防げない。

肥大化した `loss_list` と `new_losses` は次の箇所にも影響する。

- `new_losses` は呼び出し元 (`src/srt_connection.rs`) の `receive` 呼び出しで `send_nak` に渡され、NAK 送信処理も巻き込んで肥大化する
- `generate_periodic_nak` (`src/srt_receiver.rs`) は `loss_list` 全体を clone して NAK に載せるため、`loss_list` の肥大化は NAK パケットの肥大化に直結する
- `find_deliverable_seq` のギャップ判定 (issue 0073 の対象) は `loss_list` を走査するため、肥大化した `loss_list` は配信判定の性能も劣化させる

## 設計方針

損失検出ループの反復回数に上限を設けて、`loss_list` と `new_losses` の肥大化を防ぐ。

- ループの反復回数上限に既存の `max_buffer_size` (受信バッファ上限。0059 で `packets` の上限として使う方針) を流用し、反復回数が上限に達したらループを打ち切る。`loss_list` の各エントリは将来 `packets` に収容される欠損の追跡であり、`packets` の収容上限を超える追跡は不要なため、専用の定数は新設しない
- `loss_list` への追加時にもエントリ数上限チェックを行い、上限に達したら追加しない (二重の防御)。反復上限と追加上限は同一値 (`max_buffer_size`) とする。0055 の適用後は `push` を `insert` に読み替える。上限により追加しなかった分は `total_lost` に計上しない (追跡した損失のみを計上する)
- 上限を超えるギャップが検出された場合、当該パケットは破棄し (`packets` に残さない)、`loss_list` と `new_losses` のいずれにも追加しない。`expected_seq` は進めない。未登録のまま受け入れると `find_deliverable_seq` のギャップ判定 (0073 参照) が欠損を飛ばした配送と判定しうるため、破棄して欠番のまま残す。後続の誠実なパケット受信時に損失検出ループが欠損を `loss_list` に登録し直し、NAK 経由で回復する
- なお、0059 の `packets.len()` 上限 (TSBPD 滞留による肥大化の防止) とは指標が異なる (本 issue は `expected_seq` と seq の差分による損失登録の肥大化)。上限超過は攻撃または異常状態として扱い、誠実なトラフィックの疎通に影響しないことは完了条件の回復テストで担保する

## 完了条件

- `expected_seq` から 2^30 - 1 離れた seq を持つ 1 パケットを受信しても、`loss_list` と `new_losses` のエントリ数が `max_buffer_size` を超えず、当該パケットが破棄されること
- 上限超過時も panic せず、打ち切り前に到着済みの配信可能パケットが `pop_ready` で取得でき、打ち切り後に `expected_seq` 以降の誠実なパケットを受信すると損失検出と NAK 生成が動作すること
- 上限を小さく設定して、遠方の seq のパケット受信で `loss_list` が肥大化しないことを検証するテストが `src/srt_receiver.rs` 内の `#[cfg(test)]` モジュールに追加されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test --workspace` で全テストが通過すること

## 解決方法

- `src/srt_receiver.rs` の `receive` メソッドの損失検出ループに `max_buffer_size` による反復回数上限を導入する
- `loss_list` への追加時にエントリ数上限チェックを導入する (0055 適用後は `insert` 操作に対する上限チェックに読み替える)
- テストは 0059 と同様に、`max_buffer_size` フィールドに小さい値を直接設定して、ループが打ち切られ `loss_list` が肥大化しないこと、超過パケットが破棄されること、打ち切り後の誠実なパケット受信で損失検出と NAK 生成が動作することを検証する

なお、0028 (`receive` の分割。損失検出ループの抽出を含む)、0055 (`loss_list` の `HashSet` 化)、0073 (`find_deliverable_seq` の最適化)、0059 (受信バッファ上限) は同じ `receive` メソッドまたは `loss_list` を変更するため、並行実装時は直列に実装する (先後は問わない)。
