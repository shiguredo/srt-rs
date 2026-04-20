# Listener が Caller の Initial Packet Sequence Number を採用していない

Created: 2026-04-20
Completed: 2026-04-20
Model: Opus 4.7

## 概要

`SrtConnection` の Listener 役割における CONCLUSION 受信処理 (`src/srt_connection.rs:819-868` の `handle_handshake_listener` 内 `HandshakeType::Conclusion` 分岐) で、Caller から受信した Initial Packet Sequence Number (ISN) を Listener 自身の送信 ISN として採用していない。CONCLUSION レスポンスでは Listener が独自に保持している `self.initial_seq` を返している。

## 根拠

SRT 仕様 (`refs/srt/draft-sharabayko-srt.md`) の Conclusion Response セクション (1660-1662 行目) に以下の記述がある:

> The only case when the Listener can have precedence over the Caller is the advertised Cipher Family and Block Size (see {{handshake-encr-fld}}) in the Encryption Field of the Handshake.

CONCLUSION フェーズにおいて Listener が Caller に優先権 (precedence) を持つフィールドは Cipher Family と Block Size のみ。Initial Packet Sequence Number はこの例外に含まれていないため、Caller が CONCLUSION リクエストで送ってきた値を Listener はそのまま採用すべきである。

## 修正内容

`src/srt_connection.rs` の `handle_handshake_listener` の `HandshakeType::Conclusion` 分岐で、CONCLUSION レスポンス送信前に Caller の ISN を採用する。

```rust
// SRT 仕様 (Conclusion Response): Listener が Caller に優先権を持つのは
// Cipher Family と Block Size のみ。ISN は Caller の値を採用する。
self.initial_seq = hs.initial_packet_seq;
```

## 関連箇所

- `src/srt_connection.rs:819-868` - `handle_handshake_listener` の CONCLUSION 分岐
- `src/srt_connection.rs:1268-1306` - `send_conclusion_response` (`self.initial_seq` を参照する)
- `refs/srt/draft-sharabayko-srt.md:1651-1669` - Conclusion Response 仕様

## 解決方法

`src/srt_connection.rs` の `handle_handshake_listener` の `HandshakeType::Conclusion` 分岐において、Cookie 検証直後に `self.initial_seq = hs.initial_packet_seq;` を追加した。これにより `send_conclusion_response` で生成される CONCLUSION レスポンスの Initial Packet Sequence Number が Caller のものと一致するようになる。

CHANGES.md に `[FIX]` エントリを追加した。
