# srt-rs

[![shiguredo_srt](https://img.shields.io/crates/v/shiguredo_srt.svg)](https://crates.io/crates/shiguredo_srt)
[![Documentation](https://docs.rs/shiguredo_srt/badge.svg)](https://docs.rs/shiguredo_srt)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

## About Shiguredo's open source software

We will not respond to PRs or issues that have not been discussed on Discord. Also, Discord is only available in Japanese.

Please read <https://github.com/shiguredo/oss> before use.

## 時雨堂のオープンソースソフトウェアについて

利用前に <https://github.com/shiguredo/oss> をお読みください。

## 概要

Rust で実装された Sans I/O な SRT (Secure Reliable Transport) プロトコルライブラリです。

## 特徴

- Sans I/O
  - <https://sans-io.readthedocs.io/>
- ライブストリーミング専用 (LiveCC)
- AES-128/256 暗号化対応

## 機能

### コア

- 送受信バッファ管理
- パケット順序制御と重複検出
- ACK/NAK/ACKACK 処理
- RTT 計算 (EWMA)
- Flow Window 管理
- TSBPD (Timestamp-Based Packet Delivery)
- TLPKTDROP (Too-Late Packet Drop)

### 暗号化

- AES-CTR 暗号化/復号化
- KEK 導出 (PBKDF2)
- SEK ラップ/アンラップ (RFC 3394)
- KM Refresh (2^25 パケット毎の自動鍵更新)

### 輻輳制御 (LiveCC)

- CWND 動的調整 (BDP ベース)
- 入力レート測定
- MAX_BW 自動調整
- Packet Pacing

### 統計情報

- パケットロス率
- ジッター (RFC 3550)
- 受信/送信バイト数
- 再送パケット詳細

### ハンドシェイク拡張

- Stream ID (Access Control 構文対応)
- Congestion 拡張

## サンプル

### srt-listener

SRT Listener (受信側) の例です。

```bash
cargo run -p srt-listener -- --port 9000
cargo run -p srt-listener -- --port 9000 --passphrase secret
cargo run -p srt-listener -- --host 127.0.0.1 --port 9000
```

Listener は Caller の接続確立（Connected）から切断までに受信したデータを、
切断時にカレントディレクトリへ `srt_<timestamp>.ts` として保存します。

**オプション:**

- `-h, --host <HOST>`: バインドアドレス (デフォルト: 0.0.0.0)
- `-p, --port <PORT>`: リッスンポート
- `--passphrase <PASSPHRASE>`: 暗号化パスフレーズ
- `--mp4`: 切断時に受信した MPEG2-TS を MP4 に変換し、カレントディレクトリへ `srt_<timestamp>.mp4` として保存します。（H.264 + AAC のみ対応。変換に失敗した場合も `srt_<timestamp>.ts` は保存されます）。

### srt-caller

SRT Caller (送信側) の例です。

```bash
cargo run -p srt-caller -- --host 127.0.0.1 --port 9000
cargo run -p srt-caller -- --host 127.0.0.1 --port 9000 --passphrase secret
cat input.ts | cargo run -p srt-caller -- --host 127.0.0.1 --port 9000
```

**オプション:**

- `-h, --host <HOST>`: 接続先アドレス
- `-p, --port <PORT>`: 接続先ポート
- `--passphrase <PASSPHRASE>`: 暗号化パスフレーズ

### Listener と Caller の接続テスト

```bash
# ターミナル 1: Listener 起動
cargo run -p srt-listener -- --port 9000

# ターミナル 2: Caller からデータ送信
echo 'Hello, SRT!' | cargo run -p srt-caller -- --host 127.0.0.1 --port 9000
```

## FFmpeg との連携

### FFmpeg → Listener (このライブラリで受信)

```bash
# Listener 起動
cargo run -p srt-listener -- --port 9000

# FFmpeg から送信
ffmpeg -re -i input.mp4 -c copy -f mpegts "srt://127.0.0.1:9000?mode=caller"
```

### Caller (このライブラリで送信) → FFmpeg

```bash
# FFmpeg で受信
ffmpeg -i "srt://0.0.0.0:9000?mode=listener" -c copy output.ts

# Caller から送信
cat input.ts | cargo run -p srt-caller -- --host 127.0.0.1 --port 9000
```

## OBS との連携

### OBS → Listener (このライブラリで受信)

1. Listener を起動:

   ```bash
   cargo run -p srt-listener -- --port 9000
   ```

2. OBS の設定:
   - 設定 → 配信 → サービス: カスタム
   - サーバー: `srt://127.0.0.1:9000?mode=caller`

## libsrt 互換性

### データ部 0 バイトパケットの 4 バイトゼロパディング

#### 問題の背景

SRT 仕様 (draft-sharabayko-srt) では、Keepalive、ACKACK、Shutdown などの制御パケットはデータ部を持たない (0 バイト) と定義されている。

#### libsrt の実装上の問題

libsrt は全パケットを「ヘッダ部 + データ部」の 2 つの iovec で `writev` 送信する設計だが、データ部が 0 バイトの場合 `writev` が正しく動作しない環境がある。そのため、データ部が 0 バイトのパケットに 4 バイトのゼロパディングを追加している。

```c
// libsrt/srtcore/packet.cpp より
case UMSG_KEEPALIVE:
    // control info field should be none
    // but "writev" does not allow this
    m_PacketVector[PV_DATA].set((void*)&m_extra_pad, 4);
    break;
```

#### Wireshark の問題

Wireshark の SRT dissector も libsrt に合わせて実装されているため、仕様通りの 16 バイトパケットを送ると "Malformed Packet" と表示される。

#### このライブラリでの対応

libsrt および Wireshark との相互運用性のため、同様の 4 バイトゼロパディングを追加する。

対象パケット:

- Keepalive (0x0001)
- ACKACK (0x0006)
- Shutdown (0x0005)

## 対象外

このライブラリはライブストリーミング専用のため、以下の機能は対象外です。

- FileCC (File Transfer Congestion Control)
- Rendezvous ハンドシェイク
- Group Membership 拡張

## 規格書

このライブラリが準拠している仕様です。

- draft-sharabayko-srt - SRT Protocol
  - <https://datatracker.ietf.org/doc/html/draft-sharabayko-srt>

## ライセンス

Apache License 2.0

```text
Copyright 2026-2026, Shiguredo Inc.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```
