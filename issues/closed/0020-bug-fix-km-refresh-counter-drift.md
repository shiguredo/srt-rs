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

## 解決方法

調査の結果、本 issue は偽陽性であり、現在の実装が正しいと判定した。コード変更は行わない (close)。

### 判定根拠

KM Refresh の状態機械は `encrypted_packet_count` を基準に以下の順で遷移する (`src/crypto.rs`):

- `should_pre_announce` (Idle かつ count >= 2^25 - 4000) → PreAnnounce へ
- `should_switch` (PreAnnounce かつ count >= 2^25) → `switch_key` で count を 0 にリセットし PostAnnounce へ
- `should_decommission` (PostAnnounce かつ count >= 4000) → `decommission_old_key` で旧鍵を破棄し Idle へ

カウンタのリセットは 1 サイクルにつき 1 回、`switch_key` (count=2^25 の時点) でのみ行われる。これは「新しい現用鍵で暗号化したパケット数」を 0 から数え直すための正しいリセットである。

本 issue が提案する `decommission_old_key` での追加リセットを入れると、`switch_key` (count を 0 にリセット) から `decommission` (count=4000) までの 4000 パケット分が二重にリセットされ、次サイクル以降の pre-announce / switch が毎回 4000 パケットずつ後ろへずれる。すなわち提案修正こそがドリフトを生む。現状の「リセットは switch_key のみ」は各鍵にちょうど 2^25 パケットの寿命を与えており、ドリフトは発生しない。

したがって `decommission_old_key` が count をリセットしないのは意図的かつ正しい。本 issue の前提 (「累積し続ける」「pre-announce が早期発火する」) は誤りである。

(検証箇所: `src/crypto.rs` の `switch_key` (307-311 行)、`decommission_old_key` (316-323 行)、`should_pre_announce` / `should_switch` / `should_decommission` (267-280 行))
