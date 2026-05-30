# sans-io 違反: debug オプションが直接 eprintln! を出力している

- Priority: Medium
- Created: 2026-05-14
- Completed: 2026-05-30
- Polished: 2026-05-30
- Model: DeepSeek V4 Pro
- Branch: feature/change-sansio-debug-tracing

## 目的

`src/srt_connection.rs` の 8 箇所で `self.options.debug` が true の場合に `eprintln!` で直接 stderr に書き込んでいる。これは 2 つの問題を抱える。

1. sans-io パターン (全出力を外部に委ね、I/O 副作用を持たない状態機械) において、状態機械内から直接 stderr へ書く I/O 副作用である
2. CLAUDE.md「ログは tracing を使うこと」に違反している

これらの `eprintln!` を `tracing::debug!` に置き換え、デバッグ出力の有無と出力先を tracing-subscriber 側で制御できるようにする。あわせて冗長になる `ConnectionOptions::debug` フィールドを削除する。

## 優先度根拠

8 箇所すべて `if self.options.debug` (デフォルト false) でガードされた opt-in のデバッグ出力であり、プロトコル状態・`output_queue`・データ整合性・相互運用性には影響しない。デフォルト動作は壊れていない。問題は「デバッグ出力先を呼び出し側が制御できず stderr 固定である」点 (移植性・テスト容易性・ログ方針違反) で、機能不全ではなく設計純度の問題のため Medium とする。

注: 旧版は High かつ「stderr が存在しない環境での動作を不可能にする」としていたが、デフォルト `debug=false` では `eprintln!` は呼ばれず、問題は `debug=true` 時のデバッグ出力先の制御不能に限られる。誇張を除き、優先度を Medium に修正した。

## 現状

`src/srt_connection.rs` に `eprintln!` が 8 箇所ある。いずれも `if self.options.debug` ブロック内にある (行番号は将来変わりうるため関数名と内容で特定する)。

- `feed_recv_buf` の DATA パケット受信ログ (348 行付近)
- `send` の DATA 送信ログ (seq / msg / ts / dest_socket_id / payload_len、495 行付近)
- `handle_control_packet` の制御パケット種別ログ (686 行付近)
- `handle_handshake_caller` の INDUCTION レスポンスログ (729 行付近)
- `handle_handshake_caller` の CONCLUSION レスポンスログ (761 行付近)
- `handle_ack` の ACK 受信ログ (906 行付近)
- `handle_ack` のバッファ増減ログ (`Buffer: {} -> {} packets`、920 行付近)
- `send_conclusion_request` の CONCLUSION リクエスト送信ログ (1243 行付近)

旧版は 7 箇所としていたが、920 行のバッファログが漏れていたため 8 箇所に修正した。

## 設計方針

tracing を導入し、`eprintln!` を `tracing::debug!` に置き換える。デバッグ出力の有無は tracing-subscriber の level filter (RUST_LOG / EnvFilter 等) で制御し、出力先も subscriber が決める。

- 状態機械から直接 stderr へ書く I/O 副作用がなくなる (subscriber がなければ `tracing::debug!` は no-op)
- CLAUDE.md のログ方針 (tracing 使用、ログメッセージ英語) に合致する。既存メッセージは `[DEBUG]` 付き英語なので、`[DEBUG]` プレフィックスは tracing の level 表示に委ねて削除する

### `options.debug` フィールドの削除

`tracing::debug!` は内部で level チェックを行うため、`if self.options.debug` による gate は不要になる。冗長な `ConnectionOptions::debug` フィールド (161 行、default は 177 行) を削除し、デバッグ出力の制御を tracing-subscriber に一本化する。

- `ConnectionOptions::debug` の削除は公開 API の後方互換のない変更であり、CHANGES.md は `[CHANGE]`、ブランチ prefix は `feature/change-` とする
- `debug` フィールドを設定しているのは `examples/srt_caller` と `examples/srt_listener` の 2 つ (それぞれ `--debug` フラグを noargs で取り `ConnectionOptions` に渡している)。これらを tracing-subscriber の設定 (例: `--debug` フラグで EnvFilter の level を `debug` に上げる) に移行する
- `crates/c-api` は `ConnectionOptions::debug` を設定していない (Default 経由) ため、フィールド削除の影響はない
- ただしテストコードには `debug` を明示設定する箇所があり (後述)、フィールド削除でコンパイルエラーになる。`debug` を参照する全 `*.rs` を grep で洗い出し、examples・c-api・テストを漏れなく対応すること

### ConnectionEvent には流さない

デバッグ用の観測トレースを `ConnectionEvent::Debug` として `event_queue` に流す案も考えられるが、採らない。

- `ConnectionEvent` は `Connected` / `DataReceived` / `StateChanged` / `Error` / `Disconnected` / `KeyRefreshNeeded` というプロトコル意味論を持つイベント型であり、観測トレースを混ぜると型の責務が濁る
- `ConnectionEvent` への新バリアント追加は、消費側の網羅 match (`examples/srt_caller`、`examples/srt_listener`、いずれも catch-all なし) をコンパイルエラーで壊す後方互換破壊になる
- quinn-proto / rustls / str0m などの実用的な sans-io 実装は、プロトコル状態に影響する非決定性 (時刻・乱数・I/O) は注入で排除する一方、観測ロギングは tracing / log でコア内から直接出している。tracing は I/O を強制せず subscriber がなければ no-op であり、sans-io の「プロトコル出力を委ねる」契約と両立する

## 依存追加

`Cargo.toml` に tracing を追加する (用途: 構造化ログ)。tracing-subscriber は examples 側のバイナリでのみ使用し、ライブラリ本体は tracing のファサードのみに依存する。バージョンはマイナーまで指定する (CLAUDE.md のライブラリ方針)。

- ライブラリ本体: `tracing = "0.1"`
- examples: `tracing-subscriber = { version = "0.3", features = ["env-filter"] }` (EnvFilter で level を制御するため `env-filter` feature を有効化する)

## テスト戦略

デバッグ出力は tracing に委ねるため、ライブラリ本体には新規テストを課さない (ログ出力の検証は subscriber 依存であり、CLAUDE.md のモック禁止方針とも整合しない)。既存テスト (`tests/sansio_test.rs`、`pbt/tests/prop_connection.rs`) が引き続き通過すること、および `eprintln!` が 1 箇所も残らないことを確認する。

## 他 issue との関係

- #0027 (srt_connection.rs 分割) は同じファイルを触る。`eprintln!` が分割対象領域に散在するため、本 issue を先に行い出力経路を tracing に統一してから #0027 で分割する方が手戻りが少ない

## 修正対象

1. `Cargo.toml` (ライブラリ本体) に `tracing` 依存を追加する
2. `src/srt_connection.rs` の 8 箇所の `eprintln!` を `tracing::debug!` に置き換える (`[DEBUG]` プレフィックスは削除)
3. 各 `eprintln!` を囲む `if self.options.debug` の gate を削除する
4. `ConnectionOptions::debug` フィールド (161 行) と Default の該当行 (177 行) を削除する
5. `examples/srt_caller`、`examples/srt_listener` を、`--debug` フラグで tracing-subscriber の level を制御するよう移行する (`tracing-subscriber` を `env-filter` feature 付きで examples の依存に追加)
6. `debug` フィールドに依存するテストを修正する (`grep -rn "debug" --include=*.rs` で漏れなく洗い出す)
   - `tests/sansio_test.rs` の `test_debug_mode` を削除する (`debug: true` でも接続できることを確認するテストだが、`debug` フィールド削除後は意味を失う。接続確立自体は他のテストでカバー済み)
   - `pbt/tests/prop_connection.rs` の `make_opts` / `make_opts_with_stream_id` から `debug: false` 行を削除する
7. `CHANGES.md` の `## develop` セクションに `[CHANGE]` エントリを追加する
   - 例: `[CHANGE] デバッグ出力を eprintln! から tracing に移行し ConnectionOptions::debug を削除する`

## 完了条件

- `src/srt_connection.rs` に `eprintln!` が 1 箇所も残っていないこと (8 箇所すべて `tracing::debug!` に置換)
- `ConnectionOptions::debug` フィールドが削除されていること
- `debug` を参照していた全箇所 (examples・テスト) が移行・修正されていること
- `examples/srt_caller`、`examples/srt_listener` が tracing-subscriber でデバッグ出力を制御するよう移行されていること
- `Cargo.toml` に `tracing` 依存が追加されていること
- `CHANGES.md` に `[CHANGE]` エントリが追加されていること
- `cargo test`、`cargo build --workspace` (examples 含む)、`cargo clippy --workspace -- -D warnings` (tracing 導入後の未使用 import 等を含む) が通過すること

## 解決方法

`src/srt_connection.rs` の 8 箇所の `eprintln!`（`if self.options.debug` でガードされた `[DEBUG]` 出力）を `tracing::debug!` に置き換え、`if self.options.debug` ゲートを除去した。あわせて冗長になった `ConnectionOptions::debug` フィールドと Default の該当行を削除し、デバッグ出力の制御を tracing-subscriber に一本化した。状態機械から直接 stderr へ書く I/O 副作用がなくなり、sans-io の「プロトコル出力を委ねる」契約と整合する（subscriber がなければ `tracing::debug!` は no-op）。ログメッセージは `received X` / `sending X` の形に統一し、`[DEBUG]` プレフィックスは tracing の level 表示に委ねて削除した。

- `Cargo.toml` に `tracing = "0.1"` を追加した。
- `examples/srt_caller`、`examples/srt_listener` に `tracing-subscriber`（`env-filter` feature）を追加し、`--debug` フラグで EnvFilter の level を制御するよう移行した。`RUST_LOG` を優先し、`--debug` 指定時のみ `shiguredo_srt=debug` を出す（ライブラリのログは debug レベルのため、既定の info では出ない）。
- `crates/c-api` は `ConnectionOptions::debug` を参照していない（Default 経由）ため影響はない。

### テスト

ログ出力の検証は subscriber 依存でモック禁止方針と整合しないため、新規テストは課さない。`tests/sansio_test.rs` の `test_debug_mode`（`debug: true` でも接続できることを確認するテストだが、フィールド削除後は意味を失う。接続確立は他のハンドシェイクテストでカバー済み）を削除し、`pbt/tests/prop_connection.rs` の `make_opts` / `make_opts_with_stream_id` から `debug: false` 行を削除した。

`CHANGES.md` の `## develop` に `[CHANGE]` エントリを追加した。

注: examples のアプリ自身の出力（統計・接続状況）は I/O 層からの意図的なコンソール出力であり sans-io 違反ではないため、`eprintln!` のまま残す（本 issue はライブラリ状態機械の sans-io 純度を対象とする）。
