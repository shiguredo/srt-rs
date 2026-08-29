# Listener が接続確立後も INDUCTION を受理して送受信バッファを初期化する

- Priority: Medium
- Created: 2026-08-30
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/fix-listener-induction-state-gate
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

`handle_handshake_listener` の INDUCTION 受信分岐にはハンドシェイク状態の gate がない。接続確立済み (`ConnectionState::Connected`) の Listener が別のピアから INDUCTION を受けると、そのまま INDUCTION を返し続けてしまう。その後その Cookie を載せた CONCLUSION が届くと受理され、

- `initial_seq` が後発ピアの初期シーケンス番号で上書きされる
- `init_buffers` が呼ばれ、送受信バッファが再生成される (未配送データ・損失リスト・統計が失われる)
- `peer_socket_id` が後発ピアのソケット ID に書き換わる (既存ピア宛のパケット送信先が変わる)

という、既存接続の破壊が起こる。Listener 側の Cookie は接続ごとに生成されピアのアドレスには紐付かないため (`SrtConnection::new_listener` の doc に明記済み)、INDUCTION を送信できるピアなら正当な手順で Cookie を取得できてしまう。

sans-io な設計では受信パケットの仕分け (どの `SrtConnection` に流すか) はアプリ側の責務だが、ライブラリ側が状態外の受理条件で接続を壊さないことが前提になる。

## 現状

- `src/srt_connection.rs` の `handle_handshake_listener` の `HandshakeType::Induction` 分岐は `self.peer_socket_id` の更新、`send_induction_response`、`self.handshake_state = HandshakeState::InductionReceived` を無条件に行う。`self.state` (`ConnectionState`) も既存の `handshake_state` も見ていない
- 対照的に `HandshakeType::Conclusion` 分岐は `handshake_state != HandshakeState::InductionReceived` のときに `Ok(())` を返して無視する gate を持つ
- CONCLUSION 受理時は `initial_seq = hs.initial_packet_seq` と `init_buffers` を実行し、`set_state(ConnectionState::Connected)` で再接続状態に入る
- `examples/srt_listener` は受信パケットを `recv_from` で受け取るが、記録した `peer_addr` で仕分けせず単一の `SrtConnection` へ `feed_recv_buf` している。したがって 1 接続 = 1 `SrtConnection` の運用で到達しうる
- Listener 側にハンドシェイクタイムアウトはない (`TimerId::Handshake` は `connect()`、つまり Caller 側のみで設定される)

## 設計方針

- `handle_handshake_listener` の INDUCTION 受理条件を、接続がまだ確立していない状態に限定する。判定には `self.state` を使い、`ConnectionState::Connected` の間は INDUCTION を無視 (`Ok(())`) する。`Closing` 等の扱いをどうするかは実装時に `ConnectionState` の全 variant を並べて決め、コメントに理由を書く
- `ConnectionState::Listening` への復帰経路 (切断処理) との整合を確認する。再接続を許すかどうかは本 issue の方針では変更しない (既存の `Disconnected` / `Closing` からの復帰経路を壊さない)
- ピアアドレスとの紐付けは行わない。`feed_recv_buf` は送信元アドレスを受け取らず、sans-io 設計では仕分けはアプリ側の責務であるため、ライブラリ側でできるのは状態に基づく受理制限まで。この限界は `SrtConnection::new_listener` の doc に既に明記済みなので重複して書かない
- CONCLUSION 側の gate (INDUCTION 未受信なら無視) は維持する。INDUCTION を止めれば旧ピアの Cookie を知らないピアは CONCLUSION まで進めないが、gate 自体が壊れているわけではない

## 完了条件

- `src/srt_connection.rs` の `handle_handshake_listener` の INDUCTION 分岐に状態 gate が入っていること
- `ConnectionState::Connected` の Listener に別 socket_id の INDUCTION を `feed_recv_buf` しても、応答パケットが出力されず、`peer_socket_id`・`initial_seq`・送受信バッファ (統計または滞留パケット数で観察する) が変わらないことを検証するテストが追加されていること
- 同条件でその Cookie を載せた CONCLUSION を送っても再接続が成立しない (状態が `Connected` のままか、既存ピアとの接続が維持される) ことを検証するテストが追加されていること
- 既存のハンドシェイク e2e テスト (初回 INDUCTION → CONCLUSION) および `pbt/tests/prop_connection.rs` の相互接続プロパティが通過すること
- `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` が通過すること
- `CHANGES.md` の `## develop` に `[FIX]` エントリが追加されていること

## テスト

意図的なエラーパス (状態が不正なときの無視) であり、PBT で表現できる性質 (任意の入力で接続状態が不変であること) とも取れるが、対象は具体的な状態遷移の 1 経路なので単体テスト側で書く。`tests/test_srt_connection.rs` の「エラーケーステスト」または「ハンドシェイクテスト」章に置き、既存のヘルパー (`establish_connection`、`exchange_induction_and_take_cookie`、`feed_conclusion_with_cookie`) を再利用する。

## CHANGES.md

- [FIX] Listener が接続確立後に受信した INDUCTION を無視するようにする
  - @voluntas

## 相互作用

- issue 0025 (SYN Cookie の生成) で Cookie がピア追従型になった結果、本 issue の缺口が意味を持つようになった (修正前は Cookie が 0 で固定であり、INDUCTION を経由しないピアでも確立処理へ進めた)。issue 0025 の doc に「1 つの `SrtConnection` を複数ピアで共有しないこと」という利用側制約を書いたが、本 issue でライブラリ側を閉じる
- issue 0083 (拒否時の rejection reason 応答) と同じ分岐を触る。0083 が INDUCTION を無視したときに何も返さない方針なら両者は独立だが、拒否応答を返す設計なら直列で決める
- issue 0027 (`handle_handshake_*` の分割) は同じ関数を移動するため直列にする
- Listener 側にハンドシェイクタイムアウトが無い件は本 issue では扱わない
