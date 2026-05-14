# AES-CTR の IV 構築で XOR 位置が 2 バイトずれている

- Priority: High
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-aes-ctr-iv-construction

## 目的

`src/crypto.rs:401-410` の `encrypt_payload` 関数で、packet_index を IV の bytes[12:15]（最下位 32 ビット）に XOR しているが、SRT 仕様 (§6.1.2, draft-sharabayko-srt.md 3040-3055 行) ではカウンタ構造の bits 16-47 にパケットインデックスを配置し、その上位 112 ビットを Salt と XOR する。つまり bytes[10:13] に XOR すべきであり、**2 バイトずれている**。

## 優先度根拠

このバグにより libsrt を含む SRT 仕様準拠の他実装との暗号化通信が一切不可能になる。暗号化は SRT の主要機能であり、相互運用性を完全に破壊する最優先の致命的バグ。

## 現状

```rust
// src/crypto.rs:401-409
let mut iv = [0u8; 16];
iv.copy_from_slice(salt);

let pi_bytes = packet_index.to_be_bytes();
iv[12] ^= pi_bytes[0];
iv[13] ^= pi_bytes[1];
iv[14] ^= pi_bytes[2];
iv[15] ^= pi_bytes[3];
```

## 根拠

draft-sharabayko-srt.md 3040-3055 行:

> The 128-bit counter block is filled as follows:
> - a block counter in the least significant 16 bits (bits 0-15)
> - a packet index (packet sequence number) in the next 32 bits (bits 16-47)
> - eighty zeroed bits (bits 48-127)
>
> The upper 112 bits of this sequence are XORed with the Salt (MSB(Salt, 112))...

カウンタ構造の bits 16-47（= bytes 10-13）がパケットインデックス位置。Salt の上位 112 bits（bytes 0-13）をカウンタの上位 112 bits と XOR するため、XOR の対象は bytes 10-13 となる。

## 設計方針

`iv[12]^=...` を `iv[10]^=...` に修正する。コメントも正しい内容に更新する。

### 修正対象

1. `src/crypto.rs:406-409` の XOR 位置を `iv[10]^=...` に修正
2. `src/crypto.rs:398-399` のコメントを仕様に準拠した内容に修正

### テスト戦略

既存の PBT（`pbt/tests/prop_crypto.rs`）は自前実装同士のラウンドトリップのみを検証しているため、IV 構築の誤りを検出できない。修正後、libsrt との相互運用テストで検証する必要がある。

## 完了条件

- `packet_index` の XOR 位置が bytes[10:13] になっていること
- コメントが正しい実装内容を反映していること
- `cargo test` で全テストが通過すること
