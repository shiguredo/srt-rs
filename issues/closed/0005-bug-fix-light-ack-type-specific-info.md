# Light ACK の type_specific_info が 0 であるべき仕様に違反している

- Priority: Medium
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-light-ack-type-specific-info

## 目的

Light ACK 送信時、`type_specific_info`（ACK Acknowledgement Number）に Full ACK 用の `ack_number` が常時設定されている。SRT 仕様では Light ACK および Small ACK の `type_specific_info` は 0 でなければならない。仕様違反を修正する。

## 優先度根拠

仕様違反ではあるが、ACKACK は Light ACK に対して返されないため `ack_number` 値が実際に参照されることはない。機能上の破綻は発生せず、修正の優先度は Medium とする。

## 現状

### `generate_ack`（ `src/srt_receiver.rs:499-528` ）

`is_light` フラグに関わらず `ack_number` を毎回インクリメントし、`ack_timestamps` に記録している:

```rust
self.ack_number = self.ack_number.wrapping_add(1);
self.ack_timestamps.record(self.ack_number, now);
```

### `send_ack`（ `src/srt_connection.rs:1087-1123` ）

Light ACK かどうかに関わらず常に `type_specific_info: receiver.ack_number()` を設定している:

```rust
let pkt = ControlPacket {
    control_type: ControlType::Ack,
    subtype: 0,
    type_specific_info: receiver.ack_number(),
    // ...
};
```

## 根拠

### ACK Acknowledgement Number（ACK, 1065-1066 行目）

> Acknowledgement Number: 32 bits.
> : This field contains the sequential number of the full acknowledgment packet starting from 1, except in the case of Light ACKs and Small ACKs, where this value is 0 (see below).

### Light ACK / Small ACK type_specific_info（ACK, 1101-1104 行目）

> A Light ACK control packet includes only the Last Acknowledged Packet Sequence Number field. The Type-specific Information field should be set to 0.
>
> - A Small ACK includes the fields up to and including the Available Buffer Size field.
>   The Type-specific Information field should be set to 0.

## 設計方針

`generate_ack` で Light ACK 時は `ack_number` のインクリメントと `ack_timestamps.record` の呼び出しをスキップする。`send_ack` で Light ACK 時は `type_specific_info: 0` を設定する。

`ack_number` と `ack_timestamps.record` は対で制御する必要がある。片方だけスキップすると ACKACK 受信時の時刻検索（ `srt_receiver.rs:561` ）が破綻する。

### 修正対象

1. `src/srt_receiver.rs` の `generate_ack` で `is_light` が `true` の場合は以下をスキップする:
   - `self.ack_number = self.ack_number.wrapping_add(1);`
   - `self.ack_timestamps.record(self.ack_number, now);`

2. `src/srt_connection.rs` の `send_ack` で Light ACK 時の `type_specific_info` を `0` に設定する:
   - 修正前: `type_specific_info: receiver.ack_number(),`
   - 修正後: `type_specific_info: if ack_info.is_light { 0 } else { receiver.ack_number() },`

3. `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加する:
   - `[FIX] Light ACK の type_specific_info を SRT 仕様に従い 0 に設定するよう修正する`

### テスト戦略

`src/srt_receiver.rs` の `#[cfg(test)] mod tests` に以下の単体テストを追加する:

- Light ACK 生成時に `ack_number` がインクリメントされないことの検証
- Light ACK 生成時に `type_specific_info` が 0 であることの検証
- Full ACK 生成時には引き続き `ack_number` がインクリメントされることの検証

### スコープ外

- Small ACK は現状の実装に存在しない。Small ACK の実装時に同様の対応が必要だが、本 issue の対象外とする

## 完了条件

- `generate_ack` で Light ACK 時に `ack_number` がインクリメントされないこと
- `send_ack` で Light ACK 時に `type_specific_info` が 0 であること
- Full ACK 時の既存の動作が変更されていないこと
- `cargo test` で全テストが通過すること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること

## 解決方法

1. `src/srt_receiver.rs` の `generate_ack` で `is_light` 時は `ack_number` のインクリメントと `ack_timestamps.record` をスキップするよう修正
2. `src/srt_connection.rs` の `send_ack` で Light ACK 時の `type_specific_info` を `0` に設定するよう修正
3. `src/srt_receiver.rs` に Light ACK / Full ACK の `ack_number` 挙動を検証する単体テストを 2 件追加
4. `CHANGES.md` に `[FIX]` エントリを追加
