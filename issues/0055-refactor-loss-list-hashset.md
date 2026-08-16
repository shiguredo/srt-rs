# loss_list の Vec を HashSet に変更して O(n) 線形探索を解消する

- Priority: High
- Created: 2026-08-16
- Branch: feature/refactor-loss-list-hashset

## 目的

`src/srt_receiver.rs` の `ReceiverBuffer` 構造体で `loss_list` が `Vec<u32>` として実装されている。`receive` メソッド内の損失検出時に `loss_list.contains(&s)` が呼ばれ、O(n) の線形探索が発生する。損失リストが大きい場合、各パケット受信ごとに O(gap_size × loss_list_size) の計算量が発生し、性能低下の原因となる。

## 現状

```rust
loss_list: Vec<u32>,
```

```rust
if !self.packets.contains_key(&s) && !self.loss_list.contains(&s) {
```

`loss_list` の操作は `push`、`retain`、`contains`、`iter` のみ。

## 設計方針

`loss_list` を `Vec<u32>` から `HashSet<u32>` に変更する。`contains` が O(1) になる。`iter` を必要とする箇所 (`drop_too_late`、`generate_periodic_nak`) は `HashSet` から `Vec` への変換を伴うが、呼び出し頻度は低いため問題ない。`retain` は `HashSet` でもサポートされている。

## 完了条件

- `loss_list` が `HashSet<u32>` に変更されていること
- `cargo test` で全テストが通過すること
