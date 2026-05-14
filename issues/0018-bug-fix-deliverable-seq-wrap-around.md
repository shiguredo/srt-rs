# find_deliverable_seq がシーケンス番号ラップアラウンド境界で配送順序を誤る

- Priority: High
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-deliverable-seq-wrap-around

## 目的

`src/srt_receiver.rs:502-510` の `find_deliverable_seq` メソッドで、`BTreeMap` の数値順イテレーションと 31-bit シーケンス番号順がラップアラウンド境界で一致しない。また `has_gap` 判定も `loss_list` のみを参照し、未配送の先行パケットがバッファ内に存在するケースを検出できない。

## 優先度根拠

TSBPD 有効時にラップアラウンド境界をまたぐパケット受信で順序誤りが発生し、上位アプリケーションに誤った順序でデータが配送される。約 71.6 分ごとに発生する。

## 現状

```rust
fn find_deliverable_seq(&self, now: Timestamp) -> Option<u32> {
    for (&seq, entry) in &self.packets {  // BTreeMap 数値順
        let time_ok = !self.tsbpd_enabled || entry.delivery_time <= now;
        let has_gap = self.loss_list.iter().any(|&s| sequence_less_than(s, seq));
        if time_ok && !has_gap {
            return Some(seq);  // ラップ境界で順序誤り
        }
    }
    None
}
```

例えば `[0x7FFF_FFFE, 0, 1]` がバッファにある場合、BTreeMap は `[0, 1, 0x7FFF_FFFE]` の順でイテレートし、seq=0 を 0x7FFF_FFFE より先に配送してしまう。

## 設計方針

`expected_seq` からの一貫した配送を行う。シーケンス番号順の配送キューを別途管理するか、`find_deliverable_seq` のロジックを修正して `expected_seq` からの連続性を保証する。

## 完了条件

- ラップアラウンド境界を含むパケットが正しいシーケンス番号順で配送されること
- `cargo test` で全テストが通過すること
