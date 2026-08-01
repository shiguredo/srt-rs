# Fuzzing ターゲットが不足している

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-add-missing-fuzz-targets
- Polished: 2026-08-01

## 目的

現在の fuzz ターゲットは `fuzz_handshake_decode.rs` と `fuzz_packet_decode.rs` の 2 つのみ (現時点では。ただし #0033 の `fuzz_connection_feed` が先に実装された場合は 3 つになる)。以下の公開 API に対する fuzz ターゲットが不足している:

- `ReceiverBuffer::receive`（任意の DataPacket でパニックしないこと）
- `KmMessage::decode`（任意バイト列でパニックしないこと。既存の `fuzz_handshake_decode` は `HandshakePacket::decode` のみで拡張を raw で保持するため `KmMessage::decode` に到達しない)
- `AccessControl::parse`（任意文字列でパニックしないこと）

なお、`parse_loss_list` / `encode_loss_list` は private 関数のため直接の fuzz ターゲットにはできない (shiguredo-rust スキル: 「PBT と Fuzzing は private な実装を対象にできない（必ず公開 API を対象とする）」「テストのために private な API を無理矢理公開しないこと」。完全な `pub` 化は #0029 の公開 API 縮小とも矛盾する)。`parse_loss_list` は公開 API 経由 (`feed_recv_buf` → `handle_control_packet` → `handle_nak`) で間接的にカバーされ、#0033 の `fuzz_connection_feed` が到達する。`encode_loss_list` は接続確立状態が必要で fuzz ターゲットとしては不向きなため、本 issue の対象から外す。

## 優先度根拠

shiguredo-rust スキルは Fuzzing の役割を「任意入力に対するクラッシュ耐性（パニック安全性）」と定義しているが、カバレッジが不十分。

## 設計方針

3 つの fuzz ターゲットを追加する (既存ターゲットと同じ構成: `#![no_main]` + `fuzz_target!` マクロ):

- `fuzz/fuzz_targets/fuzz_receiver_receive.rs`: 任意の DataPacket を `ReceiverBuffer::receive` に渡す。`seq` のみ初期値 ± 小さな offset の範囲に制限し、`timestamp` / `payload` は任意値とする (任意の `seq` だと損失検出ループが最大 2^30 回走り、`loss_list` への push でメモリ枯渇する。巨大な seq ギャップの挙動は検証対象外とする。`seq` を過去方向にも取ることで、古いパケットの破棄分岐もカバーする)。`ReceiverBuffer::new` の引数 (`initial_seq` / `tsbpd_delay_ms` / `start_time` / `tsbpd_time_base`) と `now` は固定値にし、毎回新規インスタンスを生成する (`packets` の無制限増加とオーバーフローの防止)
- `fuzz/fuzz_targets/fuzz_km_message_decode.rs`: 任意バイト列を `KmMessage::decode` に渡す
- `fuzz/fuzz_targets/fuzz_stream_id_parse.rs`: 任意バイト列を `String::from_utf8_lossy` で `&str` に変換して `AccessControl::parse` に渡す
- `fuzz/Cargo.toml` に各ターゲットの `[[bin]]` エントリを追加する (既存 2 ターゲットと同じ形式)

## 相互作用

- #0033 (fuzz_connection_feed 追加) は同じ `fuzz/fuzz_targets/` と `fuzz/Cargo.toml` を変更するため、並行実装時は直列に実装する (先後は問わない)。なお、#0033 と #0027 の相互作用セクションは本 issue が `parse_loss_list` / `encode_loss_list` を対象に含むという旧前提で書かれているため、実装時に両側の記述を更新すること
- #0027 (`parse_loss_list` / `encode_loss_list` の移動と `pub(crate)` 化) は本 issue の対象外の関数を変更するため競合しない (本 issue は pub 関数のみを対象とする)
- #0033 の `fuzz_connection_feed` 実装後は、UserDefined 制御パケット (KMREQ) 経由で `KmMessage::decode` に到達しうるため、本 issue の fuzz_km_message_decode との重複を考慮する (直接ターゲットと間接経路の両方を維持する)

## テスト

新規の単体テストは追加しない。各ターゲットのビルドと短時間実行で検証する。

## CHANGES.md

機能に直接影響しない変更 (後方互換がある追加) のため、shiguredo-changelog 規約に従い `### misc` サブセクションに `[ADD]` エントリ (例: `[ADD] fuzz ターゲットを追加する`。担当者行を付けて追加すること) を追加する。

## 完了条件

- 3 つの fuzz ターゲット (`fuzz_receiver_receive` / `fuzz_km_message_decode` / `fuzz_stream_id_parse`) が追加され、`fuzz/Cargo.toml` に `[[bin]]` エントリが追加されていること
- `cargo +nightly fuzz build` でビルドできること (rust-toolchain.toml が stable 固定のため、`cargo fuzz build` は nightly 指定なしでは実行不能。`-Zsanitizer` が stable で拒否される)
- `cargo +nightly fuzz run <ターゲット> -- -runs=1` で短時間実行できること (ターゲットが即時パニックしないことの確認)
- CHANGES.md の `### misc` セクションに `[ADD]` エントリが追加されていること
