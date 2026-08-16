# loss_list の Vec を HashSet に変更して受信パスの線形探索を解消する

- Created: 2026-08-16
- Branch: feature/refactor-loss-list-hashset
- Polished: 2026-08-16

## 目的

`src/srt_receiver.rs` の `ReceiverBuffer` 構造体で `loss_list` が `Vec<u32>` として実装されている。`receive` メソッドはパケット受信ごとに呼ばれるホットパスであり、次の 2 箇所で O(n) の線形探索が発生する。

- 損失検出時の `loss_list.contains(&s)` (`src/srt_receiver.rs` の `receive` メソッド内)
- 受信パケットの損失解除時の `loss_list.retain(|&s| s != seq)` (同メソッド内。毎パケット O(loss_list_size))

損失リストが大きい場合、各パケット受信ごとにこれらの線形探索が発生し、性能低下の原因となる。なお、`find_deliverable_seq` メソッド内の `loss_list.iter().any(...)` による O(packets × loss_list) の全走査は本 issue のスコープ外とする。

## 現状

```rust
loss_list: Vec<u32>,
```

```rust
if !self.packets.contains_key(&s) && !self.loss_list.contains(&s) {
```

```rust
self.loss_list.retain(|&s| s != seq);
```

`loss_list` の操作は `push`、`retain`、`contains`、`iter`、`is_empty`、`clone`、`len`。

## 設計方針

`loss_list` を `Vec<u32>` から `HashSet<u32>` に変更する。これにより `contains` と単一要素の削除が平均 O(1) になる。

- `push(s)` は `insert(s)` に置き換える
- 毎パケット実行される `retain(|&s| s != seq)` は `remove(&seq)` に置き換える (HashSet の `retain` は O(n) のままであり、目的を達成できない)。`drop_too_late` 内の `retain` も同様に `remove` に置き換える
- `iter` を必要とする箇所は次の 3 つ
  - `drop_too_late` (`src/srt_receiver.rs` の `drop_too_late`): `iter()` をそのまま使用可能で、Vec への変換は不要
  - `generate_periodic_nak` (`src/srt_receiver.rs` の `generate_periodic_nak`): `NakPacket.loss_list` が `Vec<u32>` のため変換を伴う。**数値昇順にソートしてから Vec に変換する**。`src/srt_connection.rs` の `encode_loss_list` は連続するシーケンス番号を範囲としてエンコードして圧縮する実装であり、ソートされていれば範囲圧縮の効率が最大化される。順序不定のまま渡すと NAK パケットが肥大化する。なお、数値昇順ソートではラップ境界 (0x7FFF_FFFF → 0) をまたぐ連続範囲の圧縮が分割されるが、稀なケースで NAK が 1 範囲分肥大化するだけであり許容する (循環順ソートは `sequence_less_than` が全順序でないため安全に実装できない)
  - `find_deliverable_seq` (`src/srt_receiver.rs` の `find_deliverable_seq`): `iter().any(...)` をそのまま使用可能。ただし本 issue では走査自体は最適化しない (スコープ外)

## 完了条件

- `loss_list` が `HashSet<u32>` に変更されていること
- `receive` メソッド内の `retain(|&s| s != seq)` が `remove(&seq)` に置き換えられていること
- `drop_too_late` メソッド内の `retain(|&s| s != seq)` が `remove(&seq)` に置き換えられていること
- `generate_periodic_nak` が loss_list を数値昇順ソートしてから `Vec<u32>` に変換していること
- 複数の損失要素 (例: 1000, 1001, 1003) で `generate_periodic_nak` の出力が昇順になっていることを検証するテストが追加されていること
- `src/srt_receiver.rs` 内の `#[cfg(test)]` モジュールのテスト 2 箇所 (`assert_eq!(buf.loss_list, vec![1000])` 相当と `assert_eq!(buf.loss_list, Vec::<u32>::new())` 相当) が HashSet 対応に書き換えられていること
- `CHANGES.md` の `## develop` セクションの `misc` に `[UPDATE]` エントリが追加されていること
- `cargo test --workspace` で全テストが通過すること

## 解決方法

`src/srt_receiver.rs` の `ReceiverBuffer` 構造体の `loss_list` フィールドの型を `Vec<u32>` から `HashSet<u32>` に変更し、使用箇所を次のように更新する。

- `receive`: `push(s)` → `insert(s)`、`retain(|&s| s != seq)` → `remove(&seq)`
- `drop_too_late`: `retain` → `remove`
- `generate_periodic_nak`: `iter().copied().collect::<Vec<_>>()` で Vec に変換してから `sort()` で数値昇順に整列
- `find_deliverable_seq`: `iter()` をそのまま使用

`src/srt_receiver.rs` 内の `#[cfg(test)]` モジュールのテストを HashSet 対応に書き換える。
