# handle_ack の take_while がシーケンス番号ラップアラウンド後にバッファリークする

- Priority: Medium
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-handle-ack-wrap-around-buffer-leak

## 目的

`src/srt_sender.rs:314-318` の `handle_ack` メソッドで、ACK されたパケットをバッファから削除する際に `take_while` を使用している。シーケンス番号は 31 ビット循環空間であり、ラップアラウンド後に論理的に古いパケットが `BTreeMap` の末尾に位置し、`take_while` の対象外となって解放漏れが発生する。

## 優先度根拠

解放漏れにより `SenderBuffer` が無制限に肥大化し、長時間のストリーミングでメモリ枯渇のリスクがある。ラップアラウンドは約 58 時間（31-bit 空間 / packet rate）で発生するため、長時間セッションで顕在化する。

## 現状

```rust
// src/srt_sender.rs:314-318
let to_remove: Vec<u32> = self
    .packets
    .keys()
    .copied()
    .take_while(|&seq| sequence_less_than(seq, ack_seq))
    .collect();
```

`BTreeMap` は `u32` の自然順でキーを巡回する。例えばバッファが `[3, 4, 5, 6, ..., 0x7FFFFFF3, 0x7FFFFFF4, 0, 1, 2]` の状態で ACK seq=4 が到着した場合、`take_while` は `0` から始まる自然順で進み、`0 < 4` → `1 < 4` → `2 < 4` → `3 < 4` まで true、`4 < 4` で停止する。`0x7FFFFFF3` や `0x7FFFFFF4` は BTreeMap 末尾に位置するため評価されず削除漏れとなる。

## 設計方針

`take_while` を `filter` に置き換える。`filter` は全要素を巡回するため、ラップ前のパケットも正しく削除対象に含まれる。

### 修正対象

1. `src/srt_sender.rs:318` を以下のように修正する:
   - 修正前: `.take_while(|&seq| sequence_less_than(seq, ack_seq))`
   - 修正後: `.filter(|&seq| sequence_less_than(seq, ack_seq))`

2. `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加する:
   - `[FIX] handle_ack の take_while を filter に置き換え、シーケンス番号ラップアラウンド後のバッファリークを修正する`

### テスト戦略

`src/srt_sender.rs` の `#[cfg(test)] mod tests` に以下の単体テストを追加する:

- ラップアラウンド境界を含むテストデータで `handle_ack` を実行し、ラップ前のパケットが正しく削除されることを検証する

## 完了条件

- `take_while` が `filter` に置き換えられていること
- ラップアラウンド境界のテストが追加されていること
- `cargo test` で全テストが通過すること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること

## 解決方法

1. `src/srt_sender.rs` の `handle_ack` で `take_while` を `filter` に置き換え、全要素が巡回されるよう修正
2. `src/srt_sender.rs` にラップアラウンド境界の単体テストを追加
3. `CHANGES.md` に `[FIX]` エントリを追加
