# KM メッセージを保持する公開型の Debug が鍵材料を出力する

- Priority: Medium
- Created: 2026-08-29
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/fix-km-material-debug-leaks
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

デバッグ出力経由で鍵材料が漏れる経路を塞ぐ。`CryptoContext` (平文の SEK / KEK を保持) の `Debug` は手動実装に変えてマスク済みだが、KM メッセージそのものを保持する公開型は `#[derive(Debug)]` のままで、ラップ済み SEK とそれを導出するための材料を `{:?}` に出力する。

`KmMessage` は `salt` と `wrapped_key` を同時に持つ。`HandshakeExtension` は `ExtensionType::KmReq` / `KmRsp` のとき `data` に KM メッセージのエンコード済みバイト列を、`ControlPacket` は `ControlType::UserDefined` の subtype 3 / 4 のとき `control_info` に同じバイト列を持つ。`SrtPacket::Control` も `Debug` を derive しているため、KMREQ / KMRSP を受信した `SrtPacket` 1 個を `{:?}` で出力するだけで鍵材料がそろう。

ラップ済み SEK 単体は KEK を知らなければ復号できないが、salt が揃うと `KEK = PBKDF2(passphrase, LSB(64, Salt), 2048, KLen)` によるパスフレーズのオフライン辞書検証が可能になる。仕様上も KM のラップには既知の平文テンプレート (padding) があるため、当たり判定が成立する (`refs/srt/draft-sharabayko-srt.md`「Encryption」セクション内「Key Material Exchange」冒頭の "there is a padding, which is a known template, so the responder knows from the KM that it has the right KEK")。節構成・表現は将来変更される可能性がある。

現時点で本体・examples・テストがこれらの `Debug` から鍵材料をログへ出す経路は存在しない (`src/srt_connection.rs` の受信ログは `type={:?}` と `info_len` のみで、バイト列は出さない)。`src/lib.rs` から re-export された公開 API である以上、利用者の `dbg!()` や将来のログ追加で漏れるリスクを構造的に防ぐ。

## 現状

- `src/srt_handshake.rs` の `KmMessage` は `#[derive(Debug, Clone, PartialEq, Eq)]` で、`salt: [u8; 16]` と `wrapped_key: Vec<u8>` をフィールドに持つ
- `src/srt_handshake.rs` の `HandshakeExtension` は `#[derive(Debug, Clone, PartialEq, Eq)]` で、`ext_type: ExtensionType` と `data: Vec<u8>` を持つ
- `src/srt_packet.rs` の `ControlPacket` は `#[derive(Debug, Clone, PartialEq, Eq)]` で、`control_type: ControlType`、`subtype: u16`、`control_info: Vec<u8>` を持つ
- `src/srt_packet.rs` の `SrtPacket` (`Debug` を derive) は `Control(ControlPacket)` を介して同上のバイト列を出力しうる
- `CryptoContext` の手動 `Debug` 実装 (`src/crypto.rs`) が前例として存在する

## 設計方針

`CryptoContext` と同じ方針で、バイト列のうち鍵材料を含むものだけを条件付きでマスクする。

- `KmMessage`: `wrapped_key` を `[REDACTED]` にする手動 `Debug` を実装する。`salt` は KM メッセージの平文フィールドでありワイヤ上に出る値なのでマスクしない (`CryptoContext` で salt をマスクしないのと同じ判断)。他のフィールド (`version`、`packet_type`、`key_flag`、`keki`、`cipher`、`auth`、`stream_encapsulation`、`key_length`) はメタデータなのでそのまま出力する
- `HandshakeExtension`: `ext_type` が `ExtensionType::KmReq` / `KmRsp` のときだけ `data` を `[REDACTED]` にし、それ以外の拡張 (`HsReq` / `HsRsp` / `SID`) は従来どおりバイト列を出力する手動 `Debug` を実装する
- `ControlPacket`: `control_type == ControlType::UserDefined` かつ `subtype` が KMREQ (3) / KMRSP (4) のときだけ `control_info` を `[REDACTED]` にする手動 `Debug` を実装する。ACK / NAK / Keepalive などの CIF は非秘密なので従来どおり出力し、デバッグ可能性を保つ
- `SrtPacket` は `derive(Debug)` のまま委譲に任せる (上記の手動実装が呼ばれるため追加実装は不要)
- KM の subtype 値は `src/srt_handshake.rs` の `ExtensionType::KmReq` / `KmRsp` を `as u16` した値で判定する (同じ値空間であることは「Key Material」節の UserDefined subtype 定義と、`src/srt_connection.rs` の `handle_user_defined` が既に KMREQ = 3 / KMRSP = 4 で処理していることから裏付けられる)
- 各 `impl Debug` は `CryptoContext` と同様に網羅的分割束縛 (`let Self { ... } = self;`) を置き、フィールド追加時にマスク要否を強制的に判断させる
- マスクしない判断の根拠 (salt および平文 CIF) は `//` で関数内に書き、仕様由来の箇所は節名と「将来変更される可能性がある」旨を添える

## 完了条件

- `KmMessage` の `Debug` が `wrapped_key` を `[REDACTED]` として出力し、`salt` と鍵長はそのまま出力すること
- `HandshakeExtension` の `Debug` が `ext_type` が `KmReq` / `KmRsp` のときのみ `data` を `[REDACTED]` とし、`HsReq` 等のときはバイト列を出力すること
- `ControlPacket` の `Debug` が `ControlType::UserDefined` かつ subtype 3 / 4 のときのみ `control_info` を `[REDACTED]` とし、ACK などではバイト列を出力すること
- 上記を `tests/test_srt_handshake.rs` (新規) と `tests/test_srt_packet.rs` (新規) に公開 API 経由の単体テストとして追加すること。いずれも「マスク対象のバイト列が出力に含まれないこと」と「非マスクケースでバイト列が含まれること」の両方向を検証する
- 各 `impl Debug` に網羅的分割束縛が入っていること (新しいフィールドが黙って出力へ混入しない仕組み)
- `CHANGES.md` の `## develop` に `[FIX]` エントリが追加されていること
- `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` が通過すること

## テスト

`CryptoContext` の場合と同様、マスク出力は固定リテラルであり任意入力の生成に意味がないため、PBT でなく単体テストで検証する。テストは公開 API (`KmMessage::encode` / `decode`、`HandshakeExtension` の pub フィールド、`ControlPacket::new` と `decode`) だけを使って構築する。

## CHANGES.md

- [FIX] KM メッセージを保持する公開型の Debug 出力からラップ済み SEK をマスクする
  - @voluntas

## 相互作用

- issue 0049 (`CryptoContext` の Debug マスク) と同じ方針・同じ `[REDACTED]` マーカーを使う。3 箇所目になるため、マーカー語を共有定数にするかは実装時に判断する (`CryptoContext` と `ConnectionOptions` の 2 箇所段階では定数化しない方針だった)
- issue 0070 (`ConnectionOptions` の Debug が passphrase と SEK を漏らす) と目的が重なるが、対象型・編集ファイルが異なるため別 issue のまま進める
- issue 0029 (srt_handshake の公開 API 縮小) は可視性の変更を対象とし、本 issue は `Debug` の中身を対象とする。両者とも `src/srt_handshake.rs` を触るため実装は直列にする
- issue 0060 (KMREQ / KMRSP の定数重複解消) が `src/srt_connection.rs` のリテラルを `ExtensionType` 経由に置き換えるため、本 issue の subtype 判定と同じ値を使う。先後は問わないが重複定義を増やさないこと
- issue 0076 (C API 削除) がマージされた場合、`crates/c-api` 側の追従は不要になる
