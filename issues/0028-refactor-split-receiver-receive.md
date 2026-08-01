# srt_receiver.rs の receive() メソッドが責務過多

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-split-receiver-receive
- Polished: 2026-08-01

## 目的

`src/srt_receiver.rs` の `ReceiverBuffer::receive()` メソッドが 1 メソッドに 10 個の責務を詰め込んでいる。責務分離の観点で問題がある。

## 優先度根拠

機能には影響しないが、可読性を著しく損なう。変更時の影響把握が困難。

## 現状

`receive()` メソッドが以下の責務を全て担っている:

1. 重複チェック
2. 古いパケットの破棄
3. 統計情報更新
4. ジッター計算
5. TSBPD ラップアラウンド管理
6. 配信時刻計算
7. バッファ挿入
8. 損失検出
9. expected_seq 更新
10. 損失リストのクリーンアップ

## 設計方針

各処理をプライベートメソッドに抽出する:

- `check_duplicate(seq) -> bool` — 重複チェック（`total_duplicates` のインクリメントと重複判定。早期 return は呼び出し側の `receive()` が `if self.check_duplicate(seq) { return None; }` で行う）
- `update_statistics(packet_size, now)` — 統計情報更新（`total_received` / `packets_since_ack` / `total_bytes_received` の更新と帯域推定の記録。`seq` は使わないため引数に含めない）
- `calculate_jitter(timestamp, now)`
- `update_tsbpd_wrapping(timestamp)` — TSBPD ラップアラウンドの開始判定（終了判定は #0021 で `pop_ready()` に移動されるため、#0021 実装後のコードを前提とする）
- `calculate_delivery_time(timestamp, now) -> Timestamp` — 配信時刻計算（#0021 実装後のラップ後パケットへの MAX_TIMESTAMP + 1 加算補正を含むコードを前提とする）
- `detect_losses(seq) -> Vec<u32>` — 損失検出（`loss_list` への追加と `total_lost` の更新も含む。仕様の `#packet-naks` 節のギャップ検出に対応する）
- `advance_expected_seq()` — expected_seq の更新

抽出メソッドの呼び出し順序はデータ依存に基づく不変条件であり、入れ替えると挙動が変わる（重複チェックと古いパケットの破棄は統計情報更新より先に行い、バッファ挿入は advance_expected_seq の前に行い、損失検出は expected_seq 更新前の値からギャップを走査し、損失リストのクリーンアップは検出後に行う）。残りの責務（古いパケットの破棄、バッファ挿入、損失リストのクリーンアップ）は短いため、メソッド化は任意とする。

## 相互作用

- #0021（wrapping period 終了判定の `pop_ready()` 移動とラップ後配信時刻の補正）が責務 5・6 の実装を変更し、#0044（終了範囲の開区間化）は終了判定式を変更するため、本 issue は #0021 / #0044 の後に実装し、#0021 実装後のコードを分割対象とする（#0021 実装後は終了判定が `pop_ready()` に移動するため、本 issue の分割対象（`receive()` 内の開始判定と配信時刻計算）に #0044 の変更は直接及ばない）
- #0027 も `srt_receiver.rs` を変更対象とするため、並行実装時は同一ファイルのコンフリクトを避けるために直列に実装する（先後は問わない）
- #0031（インラインテストの `tests/` への移動）、#0032（`tests/sansio_test.rs` のリネーム）、#0034（PBT と重複する単体テストの削除）は本 issue のテストセクションが言及するテストを対象とするため、本 issue を先に実装するか、先に実装された場合はテストセクションの言及を更新する

## テスト

機能不変のリファクタリングのため、新規テストは追加しない。挙動不変は既存の `tests/sansio_test.rs` と `pbt/` のテスト、および `src/srt_receiver.rs` の既存単体テストで担保する（TSBPD ラップアラウンドのパスは #0021 の検証テストで、境界値は #0036 でカバーされる）。

## CHANGES.md

機能に直接影響しない内部構造の変更のため、shiguredo-changelog 規約に従い `### misc` サブセクションに `[CHANGE]` エントリ（例: `[CHANGE] srt_receiver.rs の receive() を責務ごとのプライベートメソッドに分割する`）を追加する。

## 完了条件

- `receive()` から設計方針のメソッド群（7 個）が抽出され、`receive()` 本体が早期 return とメソッド呼び出しのフローになっていること（残りの責務のうちメソッド化が任意とされたものはインラインのままでよい）
- `receive()` の公開シグネチャと挙動が不変であること
- `cargo test` で全テストが通過すること
- `cargo build --workspace` と `cargo clippy --workspace --all-targets -- -D warnings` が通過すること
- CHANGES.md の `### misc` セクションに `[CHANGE]` エントリが追加されていること
