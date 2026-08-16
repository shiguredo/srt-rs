# receive の損失検出ループで loss_list が大量に肥大化する

- Created: 2026-08-16
- Completed: {YYYY-MM-DD}
- Branch: feature/fix-loss-list-blowup
- Polished: {YYYY-MM-DD}

## 目的

`src/srt_receiver.rs` の `receive` メソッド内の損失検出ループは、`expected_seq` から半区間内 (差分が 2^30 未満) に離れた seq を持つ 1 パケットを受信すると、`expected_seq` から seq までの全欠損分を `loss_list` に登録する。差分は最大 2^30 - 1 まで取り得るため、1 パケットの受信で約 10 億エントリ (u32 で約 4 GiB) の `loss_list` が生成され、メモリ枯渇を引き起こす可能性がある。

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

`receive` 冒頭の古すぎるパケットの破棄 (`sequence_less_than(seq, self.expected_seq)`) は `expected_seq` より半区間以上古いパケットのみを対象とするため、この攻撃を防げない。

肥大化した `loss_list` と `new_losses` は次の箇所にも影響する。

- `new_losses` は呼び出し元 (`src/srt_connection.rs`) の `receive` 呼び出しで `send_nak` に渡され、NAK 送信処理も巻き込んで肥大化する
- `generate_periodic_nak` (`src/srt_receiver.rs`) は `loss_list` 全体を clone して NAK に載せるため、`loss_list` の肥大化は NAK パケットの肥大化に直結する
- `find_deliverable_seq` のギャップ判定 (issue 0073 の対象) は `loss_list` を走査するため、肥大化した `loss_list` は配信判定の性能も劣化させる

## 設計方針

損失検出ループの反復回数に上限を設けて、`loss_list` と `new_losses` の肥大化を防ぐ。

- ループの反復回数上限を定数で定義し、上限に達したらループを打ち切る。上限値の候補は既存の `max_buffer_size` (受信バッファ上限。0059 で使用) を流用するか、専用の定数を新設するかを実装時に判断する
- `loss_list` への push 時にもエントリ数上限チェックを行い、上限に達したら追加しない (二重の防御)
- 上限を超えるギャップは実質的に「損失として追跡できない」ため、その場合の扱い (該当パケットの破棄や `expected_seq` の進め方) は `find_deliverable_seq` のギャップ判定 (0073 参照) との整合性を保つ範囲で実装時に判断する
- 誠実なピアの高レート × 長レイテンシ構成でも `expected_seq` と seq の差分が上限 (8192 相当) を超えることは想定されず、上限超過は攻撃または異常状態の検出として扱う

## 完了条件

- `expected_seq` から 2^30 - 1 離れた seq を持つ 1 パケットを受信しても、`loss_list` と `new_losses` のエントリ数が上限を超えないこと
- 上限超過時も panic せず、`expected_seq` の更新・配信処理 (`pop_ready`) が継続すること
- 上限を小さく設定して、遠方の seq のパケット受信で `loss_list` が肥大化しないことを検証するテストが `src/srt_receiver.rs` 内の `#[cfg(test)]` モジュールに追加されていること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test --workspace` で全テストが通過すること

## 解決方法

- `src/srt_receiver.rs` の `receive` メソッドの損失検出ループに反復回数上限を導入する
- `loss_list` への push 時にエントリ数上限チェックを導入する
- テストは 0059 と同様に、テスト対象フィールド (上限値) に小さい値を直接設定して、ループが打ち切られ `loss_list` が肥大化しないことを検証する

なお、0055 (`loss_list` の `HashSet` 化)、0073 (`find_deliverable_seq` の最適化)、0059 (受信バッファ上限) は同じ `receive` メソッドまたは `loss_list` を変更するため、並行実装時は直列に実装する (先後は問わない)。
