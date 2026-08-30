# should_switch_key と should_decommission_old_key の等号境界を検証するテストを追加する

- Priority: Medium
- Created: 2026-08-30
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/refactor-add-km-refresh-boundary-tests
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

KM リフレッシュの発火判定 (`should_switch_key` / `should_decommission_old_key`) は、比較演算子を `>=` から `>` に誤って変更しても既存テストでは一切検出できない。発火タイミングは 1 サイクルあたり 2^25 パケットの暗号運用の要であり、閾値ちょうどで発火することを固定値テストで担保する。

## 現状

- `src/crypto.rs` の `should_switch_key` は `KmRefreshState::PreAnnounce` かつ `encrypted_packet_count >= KM_REFRESH_PERIOD` を返す。`should_decommission_old_key` は `KmRefreshState::PostAnnounce` かつ `encrypted_packet_count >= KM_PRE_ANNOUNCE_PERIOD` を返す
- `src/crypto.rs` の `test_km_refresh_should_pre_announce` は `should_pre_announce` の等号境界 (閾値 - 1 で `false`、閾値で `true`) を検証しているが、`should_switch_key` と `should_decommission_old_key` には同形式のテストが存在しない
- `pbt/tests/prop_crypto.rs` の `test_km_refresh_full_lifecycle` はカウントを閾値をまたぐ値に設定しての遷移検証であり、閾値ちょうどの境界は検証しない
- `src/srt_connection.rs` 内の `#[cfg(test)]` モジュールの KM リフレッシュ系テストは `CryptoContext::encrypt()` を経由する構成のため、判定時のカウントは必ず閾値 + 1 となり、等号境界を検出できない

## 設計方針

`src/crypto.rs` 内の `#[cfg(test)]` モジュールに `test_km_refresh_should_pre_announce` と同形式のテストを追加する。`start_pre_announce` / `switch_key` で状態を遷移させたうえで、private フィールド `encrypted_packet_count` に閾値 - 1 と閾値を直接代入し、`false` / `true` を検証する。private フィールドへの直接代入は同モジュール内のテストであれば正当であり、既存テストと同じ作法である。

## 完了条件

- `should_switch_key` と `should_decommission_old_key` の等号境界 (閾値 - 1 で `false`、閾値で `true`) を検証するテストが `src/crypto.rs` 内の `#[cfg(test)]` モジュールに追加されていること
- `cargo test` で全テストが通過すること
