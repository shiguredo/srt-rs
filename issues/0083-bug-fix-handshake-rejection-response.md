# ハンドシェイク拒否時に rejection reason を載せた応答を返さない

- Priority: Medium
- Created: 2026-08-30
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/fix-handshake-rejection-response
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

SRT 仕様は、ハンドシェイクの値を受理できない側が拒否理由を載せた応答を返すことを MUST としている。本ライブラリは拒否を `Err` で呼び出し側に返すだけで、ワイヤ上には何も出さない。結果としてピア側は拒否されたことを知らず、再送またはタイムアウトまで失敗理由を判定できない。

特に Cookie 検証の拒否は、SYN Cookie を接続ごとに生成するようになってから通常運用で到達しうる経路になった (正当なピアが前回セッションの Cookie を保持したまま再接続する、UDP の順序入れ替わりで古い CONCLUSION が届く等)。

## 現状

- 根拠資料: draft-sharabayko-srt.md「Handshake Overview」の "When a connection process has failed before either party can send the CONCLUSION handshake, the Handshake Type field will contain the appropriate error value for the rejected connection"、および「The Conclusion Response」の "the connection MUST be rejected by sending a conclusion response with the Handshake Type field carrying the rejection reason"。「Handshake Rejection Reason codes」に 1000 (SRT_REJ_UNKNOWN) から 1017 (SRT_REJ_CRYPTO) までのコード表がある。節構成・表現は将来変更される可能性がある
- `src/srt_connection.rs` の拒否経路はいずれも `Error::handshake_rejected(...)` を返すだけで `output_queue` にパケットを積まない
  - `handle_handshake_caller`: KMRSP 検証、ピアの KM エラー受け取り
  - `handle_handshake_listener`: KMREQ 検証、Cookie 検証 (`"invalid SYN cookie"`)
- Cookie 検証箇所には仕様乖離を自己申告するコメントが入っている (`src/srt_connection.rs` の `handle_handshake_listener` 内)
- `src/srt_handshake.rs` に拒否理由を表す型・定数は存在しない。`HandshakeType` は `Induction` / `Conclusion` / `Wavehand` / `Agreement` / `Peer` / `Unknown(u32)` のいずれかで、`decode` されなかった値は `Unknown` 経由で `handle_handshake` の `_ => {}` により黙って無視される
- `examples/srt_listener` は `feed_recv_buf` が `Err` を返すと `error!` を出すのみ。`examples/srt_caller` も同様で、拒否理由を人間が読める形では取得できるがピアには伝わらない

## 設計方針

- 拒否理由コードを `src/srt_handshake.rs` に追加する。仕様は Handshake Type フィールドにエラー値 (1000 台のコード) を載せるとしているため、`HandshakeType` の variant または併用のいずれかで表現する。各拒否経路をコード表へどう対応付けるかは、意味が近いコードを 1 つ選択してコメントに根拠を書く (例: Cookie 不一致はハンドシェイクデータの不正として扱う)。选型に迷うコードは `SRT_REJ_UNKNOWN` に寄せる
- 拒否応答は `handle_handshake_listener` / `handle_handshake_caller` の `Err` 返却と同時に `output_queue` へ積む。`Err` を返さなくなるという意味ではなく、呼び出し側への通知 (既存の `Result` と `ConnectionEvent`) はそのまま維持する
- 受信側でも拒否応答を処理できるようにする。`HandshakeType` の `decode` で拒否コードを認識し、`handle_handshake` の `_ => {}` で握り潰さない (`ConnectionEvent` か `Err` でピアの拒否を呼び出し側に伝える)
- 相互運用の検証として、libsrt が返す拒否応答のバイト列をテストで扱う場合は、その出典を実際に確認できる範囲に限定する (推測でバイト列を作らない)

## 完了条件

- 拒否理由コードが `src/srt_handshake.rs` に定義され、`src/srt_connection.rs` の 4 か所の拒否経路それぞれが対応するコードを載せた応答パケットを `poll_output` で出力すること
- 拒否応答のエンコード / デコードがラウンドトリップすること (テスト)
- ピアから拒否応答を受信した場合に、ハンドシェイクが失敗したことが呼び出し側へ伝わることを検証するテストが `tests/test_srt_connection.rs` に追加されていること (無視されないこと)
- Cookie 検証箇所の「本実装はエラーを返すだけで応答パケットを出さない」というコメントが削除され、実装と一致していること
- `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` が通過すること
- `CHANGES.md` の `## develop` に `[ADD]` または `[FIX]` のエントリが追加されていること

## テスト

- 拒否応答のエンコード / デコードのラウンドトリップは `pbt/` 側で扱う (`HandshakePacket` の PBT が既に存在する)
- 拒否経路 4 か所のそれぞれについて、条件を満たす 2 接続を構成して応答パケットが出ることを検証する単体テストは、`src/srt_connection.rs` の `#[cfg(test)]` または `tests/test_srt_connection.rs` の既存の章立てに合わせる
- 意図的なエラーパス (どのコードが載るか) は PBT では表現しづらいため単体テスト側で固定する

## CHANGES.md

- [FIX] ハンドシェイク拒否時に rejection reason を載せた応答を返すようにする
  - @voluntas

## 相互作用

- issue 0025 (SYN Cookie の生成) で Cookie 不一致拒否が通常運用経路になった。issue 0025 の実装で残した自己申告コメントが本 issue の起点
- issue 0068 の改善提案に「未知のハンドシェイクタイプを `_ => {}` で無視する」項目があり、本 issue の受信側対応と重複する。どちらで対応するかを決めてから実装すること
- issue 0069 のテスト不足領域にハンドシェイク異常系の項目が残っている。本 issue でカバーする範囲を先に決める
- issue 0027 (`handle_handshake_*` の分割) は同じ関数群を移動するため、実装は直列にする
