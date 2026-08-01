# PBT と重複する単体テストを削除する

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-remove-pbt-unit-duplication
- Polished: 2026-08-01

## 目的

shiguredo-rust スキルは「PBT でカバーできるものを単体テストで書かないこと」「unittest は pbt で実現できないものだけを書くこと」と規定している。これに違反し、固定値の単体テストが PBT と重複して多数存在する。

## 優先度根拠

機能には影響しないが、規約違反のテストが残る「壊れた窓」状態を解消する。同カテゴリの 0032 / 0033 と同じく Low とする。

## 現状

以下のモジュールの `#[cfg(test)]` に PBT (pbt/tests/prop_<module>.rs) と同一プロパティを検証する単体テストが存在する (削除対象の最終確定は設計方針の判定基準に従って実装時に PBT と突き合わせる):

- `srt_receiver.rs` (29 テスト中): `test_receiver_buffer_new` / `test_receiver_buffer_receive_in_order` / `test_receiver_buffer_loss_detection` / `test_receiver_buffer_duplicate` / `test_receiver_buffer_pop_ready` / `test_receiver_buffer_ack_generation` / `test_receiver_buffer_nak_generation` / `test_loss_rate_zero` / `test_rtt_calculation_with_ack_timestamps`
- `crypto.rs` (9 テスト中 4 件): `test_key_length` / `test_encrypt_decrypt` / `test_km_refresh_state_transitions` / `test_key_flag_kk_field_mapping`
- `srt_handshake.rs` (16 テスト中 13 件): `test_handshake_encode_decode` / `test_hs_extension` / `test_km_extension_in_handshake` / `test_km_error_response` / `test_sid_extension_basic` / `test_sid_extension_access_control` / `test_sid_extension_long_string` / `test_sid_extension_empty` / `test_no_sid_extension` / `test_congestion_extension_live` / `test_congestion_extension_file` / `test_no_congestion_extension` / `test_congestion_extension_with_sid`
- `srt_sender.rs` (7 テスト中 4 件): `test_sender_buffer_new` / `test_sender_buffer_push` / `test_sender_buffer_ack` / `test_sender_buffer_nak`
- `srt_packet.rs` (3 テスト中): `test_data_packet_encode_decode` / `test_control_packet_encode_decode`
- `stream_id.rs` (10 テスト中): `test_roundtrip` / `test_parse_with_type_and_mode` / `test_parse_with_host` / `test_parse_with_session` / `test_parse_with_custom` / `test_parse_wrong_prefix` / `test_parse_no_prefix`

## 設計方針

削除対象の判定基準: 以下の両方を満たすテストを削除する。

1. 対応する PBT が同一プロパティを検証している (PBT の strategy が単体テストの固定値を包含する)
2. 単体テストが PBT で検証されない追加の断言を含まない

以下のテストは削除対象外として残す:

- private 実装を直接テストするもの (crypto の `derive_kek` / `wrap_sek` / `unwrap_sek` 関連、receiver の `AckTimestampTracker` / `ReceivingRateEstimator` / `LinkCapacityEstimator` 関連)。PBT は公開 API のみを対象とするため代替不能
- 正確値・累積値・初期値を検証するもの (`test_jitter_calculation` の jitter 31/60、`test_total_bytes_received` の累積 19 → 38、`test_jitter_no_packets` の初期値 0、`test_loss_rate_calculation` の正確値 2000、`test_packet_pacing` の `time_until_send` 正確値 500、`test_km_refresh_encrypt_with_key_switch` の PreAnnounce 中のキー選択等。対応する PBT は粗い検証のみ)
- 境界値・ラップ回帰 (`test_km_refresh_should_pre_announce`、`test_handle_ack_wrap_around`、`test_sequence_less_than`、`test_pop_ready_blocks_on_loss_across_wrap_boundary`、TSBPD 系 (`test_tsbpd_*` / `test_drop_too_late_*`)、Light/Full ACK 番号系 (`test_light_ack_*` / `test_full_ack_*`) 等)
- パディング境界・定数チェック (`test_sid_extension_with_padding` / `test_sid_extension_exact_4_bytes`、`test_km_message_encode_decode` の version/packet_type/cipher 断言、`test_packet_position` のビット割当 (`from_bits(0b10) == First`)、stream_id の `test_parse_basic` のデフォルト値断言等)

なお、`test_ack_includes_recv_rate` は実質的な断言 (`!ack.is_light`) が PBT で検証されておらず、同じ断言を残存する `test_full_ack_increments_ack_number` が担保するため、削除対象から外す。

PBT が同一プロパティを検証していない場合は、単体テストを削除する前に PBT を強化してから削除する (例: `test_receiver_buffer_loss_detection` / `test_receiver_buffer_nak_generation` の損失シーケンス番号の正確値 (vec![1001]) は PBT が長さのみ検証するため、削除する場合は PBT へシーケンス番号の断言を追加する)。

## 相互作用

- #0031 (インラインテストの `tests/` への移動) は open のままである。本 issue の削除で対象が減るため、本 issue を先に実装する (または #0031 の存続判断を確定させてから実装する)
- #0028 のテストセクションが本 issue の削除対象テスト (src/srt_receiver.rs の既存単体テスト) を言及しているため、並行実装時は直列に実装する (先後は問わない)
- #0030 (`encrypt_payload` のリネーム) も src/crypto.rs の単体テストを変更対象としうるため、並行実装時は直列に実装する

## テスト

削除後の `cargo test` (pbt を含む) で全テストが通過すること。削除前後のカバレッジ (`cargo llvm-cov`) で、削除したテストが検証していたプロパティが PBT で担保されていることを確認する。

## CHANGES.md

機能に直接影響しない変更 (後方互換を壊さない) のため、shiguredo-changelog 規約に従い `### misc` サブセクションに `[UPDATE]` エントリ (例: `[UPDATE] PBT と重複する単体テストを削除する`。担当者行を付けて追加すること) を追加する。

## 完了条件

- 現状セクションで列挙した削除対象テストが削除されていること
- 設計方針で列挙した残存カテゴリ (private 実装テスト・正確値/累積値/初期値・境界値・ラップ回帰・パディング境界・定数チェック) に該当するテストが残っていること
- PBT を強化した場合はその強化が追加されていること
- `cargo test` で全テストが通過すること
- CHANGES.md の `### misc` セクションに `[UPDATE]` エントリが追加されていること
