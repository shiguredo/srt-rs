# find_deliverable_seq の loss_list 全走査を最適化する

- Created: 2026-08-16
- Completed: {YYYY-MM-DD}
- Branch: feature/refactor-find-deliverable-seq-loss-scan
- Polished: {YYYY-MM-DD}

## 目的

`src/srt_receiver.rs` の `find_deliverable_seq` メソッド内の配信可否判定 `self.loss_list.iter().any(|&s| sequence_less_than(s, seq))` は、`pop_ready` の呼び出しごとに packets の全要素 × loss_list の全要素の O(packets × loss_list) 走査を発生させる高頻度パスである。この判定は「loss_list の循環順での最小値が seq より前か」に等しく、O(1) で判定できる。loss_list の最小値 (循環順) を保持・参照することで全走査を排除する。

なお、本 issue は issue 0055 (`loss_list` の `Vec` から `HashSet` への変更) で「スコープ外」と明記され、ユーザー承認のうえで別 issue として分離されたものである。

## 現状

- `find_deliverable_seq` (`src/srt_receiver.rs`) は配信候補の各パケットに対して `has_gap` 判定を行い、そのたびに `loss_list.iter().any(...)` で loss_list を全走査する
- `has_gap` 判定は「loss_list のいずれかの要素 s が seq より循環順 (`sequence_less_than`。`src/srt_packet.rs` に定義) で前にあるか」であり、これは「loss_list の循環順での最小値が seq より前か」と等価である
- `loss_list` は `ReceiverBuffer` の `Vec<u32>` フィールドであり、issue 0055 の実装後は `HashSet<u32>` になる (順序情報を持たない)

## 設計方針

主案として、loss_list の循環順での最小値を保持する最小値キャッシュを採用する。

- `ReceiverBuffer` に最小値を保持するフィールドを追加する。`find_deliverable_seq` の `has_gap` 判定を `!loss_list.is_empty() && sequence_less_than(loss_min, seq)` の O(1) に変更する
- 最小値は要素の挿入時 (`receive` の損失検出) に更新する。要素の削除時 (`receive` と `drop_too_late` の `remove` 相当) に最小値自身が削除された場合のみ、O(loss_list) の再計算を行う。削除は遅延到着パケットの回復時のみに発生するため頻度が低く、再計算の実効コストは小さい
- issue 0055 で `loss_list` が `HashSet<u32>` になるため、順序情報をフィールドとして別途保持する形になる。issue 0055 の後に実装する前提とし、0055 の実装と同時に本最適化を含めるか別途フィールドを追加するかは実装時に判断する

代替案の検討結果:

- BTreeSet 化による先頭要素参照は不採用。`BTreeSet::first()` は数値順の最小値を返すため、ラップ境界 (0x7FFF_FFFF → 0) をまたぐと循環順の最小値と食い違う。`find_deliverable_seq` の doc コメントが述べる「循環順で最小を選ぶ」ロジック (ラップ境界をまたぐ連続パケットの配送順序の逆転防止) と整合しない
- `sequence_less_than` は半区間定義 (差分が 2^30 未満のときのみ true) のため、loss_list の要素が相互に 2^30 以上離れると循環順最小値の概念が崩れる。ただし loss_list の要素は `expected_seq` 周辺に集中するため実用上問題にならない。この前提の扱いは実装時に判断する

## 完了条件

- `find_deliverable_seq` の `has_gap` 判定が loss_list の全要素走査を行わない (O(1)) こと
- ラップ境界をまたぐケースを含め、`find_deliverable_seq` の配信順序の挙動が変更前と変わらないことを検証するテストが追加されていること
- `CHANGES.md` の `## develop` セクションの `misc` に `[UPDATE]` エントリが追加されていること
- `cargo test --workspace` で全テストが通過すること

## 解決方法

- `find_deliverable_seq` の `has_gap` 判定を loss_list の循環順最小値を参照する O(1) 判定に変更する。最小値キャッシュのフィールド名・挿入時更新・削除時再計算の詳細は実装時に判断する
- テストは、loss_list の最小値が seq より前のケース・後のケース・空のケース・ラップ境界をまたぐケースで `find_deliverable_seq` の結果が変更前と一致することを検証する
