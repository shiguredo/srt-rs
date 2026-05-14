# drop_too_late がパケット個別のタイムスタンプを無視している

- Priority: Medium
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-drop-too-late-individual-timestamps

## 目的

`src/srt_receiver.rs:591-600` の `drop_too_late` メソッドで、filter クロージャがシーケンス番号 `_seq` を無視し、全 `loss_list` エントリに対して同一の `estimated_delivery = start_time + tsbpd_delay` で判定している。このため全エントリが同時にドロップ対象になるか全くならないかの二択となり、パケット単位の正しいドロップ判定が行われていない。

## 優先度根拠

TSBPD が有効な場合、ドロップすべきでないパケットがドロップされる可能性がある。ただし `loss_list` は通常少数のエントリしか持たず、`start_time + tsbpd_delay` による近似で大幅に誤ることは稀である。優先度は Medium とする。

## 現状

```rust
// src/srt_receiver.rs:591-600
let expired: Vec<u32> = self
    .loss_list
    .iter()
    .copied()
    .filter(|&_seq| {
        // このシーケンス番号の配信予定時刻を推定
        let estimated_delivery = self.start_time.as_micros() + self.tsbpd_delay_us;
        now.as_micros() > estimated_delivery + self.tsbpd_delay_us
    })
    .collect();
```

- `_seq` が使用されておらず、クロージャ内の評価が全要素で同一
- `start_time` の代わりに `TsbpdTimeBase` を使用する必要がある（#0004 で修正予定）
- SRT 仕様（TLPTDR OP, 2169-2227 行目）では各パケットの `PktTsbpdTime` に基づいて個別に判定すべき

## 設計方針

各 `loss_list` エントリについて、そのシーケンス番号から `ReceivedPacket` を引き、個別の `delivery_time` に基づいてドロップ判定を行う。`TsbpdTimeBase` への依存は #0004 に従う。

TLPKTDROP_THRESHOLD は仕様（TLPKTDROP_THRESHOLD, 2179-2185 行目）に従い `1.25 * TsbpdDelay` とし、最低 1 秒を保証する。

### 修正対象

1. `drop_too_late` の filter クロージャで、シーケンス番号から `ReceivedPacket` を検索し、その `delivery_time` を現在時刻と比較するように修正する
2. 比較に `TLPKTDROP_THRESHOLD`（`1.25 * tsbpd_delay_us`、最低 1,000,000 μs）を使用する
3. `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加する:
   - `[FIX] drop_too_late のドロップ判定をパケット個別の配信時刻に基づいて行うよう修正する`

### テスト戦略

`src/srt_receiver.rs` の `#[cfg(test)] mod tests` に以下の単体テストを追加する:

- 2 つの損失パケットのうち、配信時刻を過ぎた 1 つだけがドロップされることの検証

### 他 issue との依存関係

`TsbpdTimeBase` への依存は #0004 で対応する。#0004 の修正後、`start_time` の代わりに `tsbpd_time_base` を使用する形に修正する。

## 完了条件

- `drop_too_late` が各パケットの `delivery_time` を個別に評価していること
- `TLPKTDROP_THRESHOLD` が `1.25 * tsbpd_delay_us` かつ最低 1 秒であること
- `cargo test` で全テストが通過すること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること

## 解決方法

1. `drop_too_late` の filter クロージャで、シーケンス番号から `ReceivedPacket` を検索し、存在すればその `delivery_time` を、なければ `tsbpd_time_base + tsbpd_delay_us` から推定するよう修正
2. `TLPKTDROP_THRESHOLD = max(1.25 * tsbpd_delay_us, 1_000_000)` を導入
3. `src/srt_receiver.rs` に `test_drop_too_late_individual_delivery` テストを追加
4. `CHANGES.md` に `[FIX]` エントリを追加
