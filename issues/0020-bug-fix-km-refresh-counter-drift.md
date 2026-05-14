# encrypted_packet_count が KM Refresh サイクル間で累積し続ける

- Priority: High
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-km-refresh-counter-drift

## 目的

`src/crypto.rs:316` の `decommission_old_key()` が `encrypted_packet_count` をリセットしていない。`switch_key()` で 0 にリセットされた後、約 4000 パケットで decommission されるが、その時点のカウンタ値が次のサイクルに引き継がれる。N サイクル後には N×4000 パケット分のドリフトが発生する。

## 優先度根拠

長時間のストリーミングで KM Refresh のタイミングが徐々にずれ、最終的に pre-announce が仕様の 2^25 パケットより大幅に早期に発火する。鍵のライフサイクル管理が破綻する。

## 現状

```rust
pub fn decommission_old_key(&mut self) {
    let old_key = self.current_key.other();
    match old_key {
        KeyFlag::Even => self.sek_even.fill(0),
        KeyFlag::Odd => self.sek_odd.fill(0),
    }
    self.km_refresh_state = KmRefreshState::Idle;
}
```

`encrypted_packet_count` のリセットが欠落している。

## 設計方針

`decommission_old_key()` 内で `self.encrypted_packet_count = 0;` を追加する。

## 完了条件

- `decommission_old_key()` で `encrypted_packet_count` がリセットされていること
- `cargo test` で全テストが通過すること
