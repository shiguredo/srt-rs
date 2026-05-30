# AES-CTR のカウンタブロック構築が SRT 仕様に準拠していない

- Priority: High
- Created: 2026-05-14
- Completed: 2026-05-30
- Polished: 2026-05-30
- Model: DeepSeek V4 Pro
- Branch: feature/fix-aes-ctr-iv-construction

## 目的

`src/crypto.rs` の `encrypt_payload` 関数におけるカウンタブロック (AES-CTR の初期 IV) 構築に、SRT 仕様 (draft-sharabayko-srt.md の「Encryption」セクション内「AES Counter」サブセクション) と一致しない 2 つの不具合がある。

1. パケットインデックスの XOR 位置が 2 バイト後方にずれている (現状 bytes 12-15、正しくは bytes 10-13)
2. カウンタブロック下位 16 bit (bytes 14-15、block counter 領域) に Salt の下位 2 バイトが残留している (正しくは 0)

このため packet_index が 0 でないパケットの暗号文が libsrt 等の仕様準拠実装と一致せず、暗号化通信の相互運用が成立しない。

## 優先度根拠

暗号化 (パスフレーズ指定) を有効にした場合、packet_index が 0 でないパケットで libsrt との復号が破綻する。暗号化は SRT の主要機能であり、相互運用性を破壊する致命的バグであるため High とする。

なお srt-rs 同士は送受信ともに同一の誤ったカウンタブロックを構築するため通信できてしまい、このバグは顕在化しない (テストでも検出されない。後述)。

## 現状

```rust
// src/crypto.rs encrypt_payload 内
let mut iv = [0u8; 16];
iv.copy_from_slice(salt); // 全 16 バイトをコピーするため bytes 14-15 に Salt が残る

let pi_bytes = packet_index.to_be_bytes();
iv[12] ^= pi_bytes[0]; // XOR 位置が 2 バイト後方にずれている
iv[13] ^= pi_bytes[1];
iv[14] ^= pi_bytes[2];
iv[15] ^= pi_bytes[3];
```

2 つの不具合:

- パケットインデックスを bytes 12-15 に XOR している (正しくは bytes 10-13)
- `iv.copy_from_slice(salt)` により bytes 14-15 に `salt[14]`、`salt[15]` が残る (正しくは block counter 領域なので 0)

## 根拠

draft-sharabayko-srt.md 「Encryption」セクション内「AES Counter」サブセクション (執筆時点で 3038-3055 行。行番号は将来変わりうる) の逐語引用:

> The counter for AES-CTR is the size of the cipher's block, i.e. 128 bits. It is derived from a 128-bit sequence consisting of
>
> - a block counter in the least significant 16 bits which counts the blocks in a packet;
> - a packet index, based on the packet sequence number in the SRT header, in the next 32 bits;
> - eighty zeroed bits.
>
> The upper 112 bits of this sequence are XORed with an Initialization Vector (IV) to produce a unique counter for each crypto block. The IV is derived from the Salt provided in the Keying Material (...):
>
> IV = MSB(112, Salt): Most significant 112 bits of the salt.

(blockquote 内の `(...)` は一次資料の相互参照リンク `{{sec-ctrlpkt-km}}` を省略した箇所。それ以外は逐語引用。)

以下は上記仕様からの導出 (issue 著者による補足であり、引用ではない)。

128-bit のカウンタブロックをビッグエンディアンの 16 バイト配列とみなすと、byte `i` は bits `[127 - 8i .. 120 - 8i]` を保持する。これに仕様の構造を当てはめると:

- bits 0-15 (bytes 14-15): block counter。各パケットの先頭ブロックでは 0。Salt とは XOR されない
- bits 16-47 (bytes 10-13): packet index
- bits 48-127 (bytes 0-9): ゼロ
- 上位 112 bits (bits 16-127 = bytes 0-13) を `IV = MSB(112, Salt)` (= Salt の上位 14 バイト `salt[0..14]`) と XOR する

したがって正しいカウンタブロックは次のようになる:

- bytes 0-9 = `salt[0..10]`
- bytes 10-13 = `salt[10..14]` XOR packet_index (ビッグエンディアン)
- bytes 14-15 = 0

`packet_index.to_be_bytes()` は `[MSB, .., LSB]` の順なので、MSB バイト `pi_bytes[0]` が bits 40-47 (byte 10) に、LSB バイト `pi_bytes[3]` が bits 16-23 (byte 13) に対応する。

### 仕様内のもう 1 つの記述との食い違い

同じ仕様の「Encrypting the Payload」(3176 行) と「Decrypting the Payload」(3222 行) には、`encrypt_payload` が実装する操作そのものの定義として、上記の構造記述とは別表現の式が書かれている:

> IV = (MSB(112, Salt) << 2) XOR (PktSeqNo)

この式の `<< 2` は draft 上で単位が曖昧で、「AES Counter」サブセクションの構造記述 (packet index = bits 16-47 = bytes 10-13、IV = MSB(112, Salt) を上位 112 bit に配置) と Salt が 0 でない場合に食い違って見える。

本 issue では、libsrt の実装 (カウンタブロックの図: 上位 14 バイトに Salt、bytes 10-13 に packet index を network order で XOR、bytes 14-15 は 0) と一致する「AES Counter」サブセクションの構造記述を正とする。`<< 2` を含む式は draft の既知の表記上の不明瞭さとみなす。ただしこれは設計判断を含むため、最終的な相互運用性は後述の KAT (libsrt との突き合わせ) で確定する。

## 設計方針

`encrypt_payload` のカウンタブロック構築を以下に修正する:

```rust
let mut iv = [0u8; 16];
// IV = MSB(112, Salt): Salt の上位 14 バイトのみを使う。
// 下位 2 バイト (bytes 14-15) は block counter 領域なので 0 のままにする。
iv[..14].copy_from_slice(&salt[..14]);

// packet index をカウンタの bits 16-47 (bytes 10-13) に XOR する。
let pi_bytes = packet_index.to_be_bytes();
iv[10] ^= pi_bytes[0];
iv[11] ^= pi_bytes[1];
iv[12] ^= pi_bytes[2];
iv[13] ^= pi_bytes[3];
```

同箇所のコメントは、資料名・サブセクション名・将来変更の可能性を明記した正確な内容に更新する (CLAUDE.md の「資料由来の機能を実装する場合は、根拠資料名、節番号、将来変更される可能性をコードコメントで明記」に従う)。

`encrypt_payload` は `encrypt()` と `decrypt()` の両方から呼ばれる (AES-CTR は暗号化と復号が同一操作) ため、この修正は送受信の双方に同時に反映される。

## テスト戦略

既存テストはこのバグを原理的に検出できない:

- `pbt/tests/prop_crypto.rs` のラウンドトリップ系 (`test_encrypt_decrypt_roundtrip` など) と、`src/crypto.rs` のインライン単体テスト `test_encrypt_decrypt` は、いずれも同一の `encrypt_payload` で暗号化・復号する self-roundtrip である。AES-CTR の対称性により、カウンタブロックがどうずれていても送受信で同一の値が使われ、ラウンドトリップは常に成立する。よって構築位置の誤りを検出できない。
- `packet_index = 0` のときは `pi_bytes` が全ゼロで、XOR 位置がどこであっても結果が変わらないため、位置の誤りが暗号文に現れない。検証には `packet_index` が 0 でないケースが必須。

回帰防止のため、既知の (salt, sek, packet_index, 平文) に対する期待暗号文を固定値とする KAT (Known Answer Test) を `tests/test_crypto.rs` (新規ファイル。`src/crypto.rs` に対応する命名規則どおり) に追加する。これは PBT では表現できない「アルゴリズムの絶対的正しさ」の検証であり、CLAUDE.md のテスト役割分担 (単体テストは PBT で実現できないものを書く) に合致する。

KAT には最低限以下のケースを含める:

- salt の全 16 バイトが 0 でない (特に `salt[14]` と `salt[15]` が 0 でない) + packet_index が 0 でない値。salt が全ゼロだと bytes 14-15 のゼロ化も後述の公式の食い違いも検出できない (差が出ない) ため、必ず salt の下位 2 バイトを 0 でない値にする
- packet_index = `u32::MAX` 付近 (ビッグエンディアンのバイト配置が正しいか確認する)

テストベクターの入手方法:

- 案 1 (推奨): libsrt の haicrypt / cryspr 実装を用いて、既知の鍵・salt・packet_index・平文から期待暗号文を 1 組生成し、固定値として埋め込む。これは srt-rs とは独立した実装による検証であり、相互運用性を本当に担保できる唯一の手段。なお libsrt のビルド・実行環境はこのリポジトリに含まれないため、ベクター生成時に別途用意する必要がある
- 案 2 (補助): aws-lc-rs の AES (ECB 等) で「仕様どおりに手で組み立てたカウンタブロック」から鍵ストリームを独立に計算し、`encrypt_payload` の出力と照合する。ただし「手で組み立てる」際の bit 解釈は `encrypt_payload` と同じであり、両者が同じ解釈違い (byte 位置のずれ、Salt のシフト有無) を犯せば検出できない。bit 解釈の誤りに対しては独立検証にならない点に注意。回帰防止には使えるが、相互運用性の最終確認は案 1 で行う

注: モック / スタブは使わない。KAT は実際の暗号計算の固定入出力であり、CLAUDE.md のモック禁止方針に反しない。libsrt との実通信を伴う相互運用テストはこのリポジトリのテスト基盤に枠組みがないため、本 issue では KAT による回帰防止を採用する。

### packet_index の導出に関する注意

`encrypt_payload` の `packet_index` 引数には、呼び出し元 (`src/srt_connection.rs`) がデータパケットの Packet Sequence Number をそのまま渡している。仕様の構造記述は「a packet index, based on the packet sequence number」と書いており、packet index と Packet Sequence Number が完全に同一かは仕様上必ずしも明示的でない。本 issue のスコープはカウンタブロック内の byte 配置 (bytes 10-13) と bytes 14-15 のゼロ化に限るが、案 1 で libsrt と暗号文を突き合わせる際は、libsrt がカウンタに用いる packet index の導出が一致していることも前提になる。一致しない場合は byte 配置を直しても暗号文が一致しないため、KAT で不一致が出たらこの導出を切り分けること。

## 後方互換

この修正は暗号化データのワイヤフォーマット (カウンタブロックの構成) を変える。修正前の srt-rs 同士は通信できていたが、修正後は旧 srt-rs と暗号通信できなくなる。一方、修正前は libsrt 等の仕様準拠実装と通信できなかったため、実運用上の互換性低下はない。develop 内の未リリースな仕様準拠修正であり、前例 (closed #0002 KK フィールド値の仕様準拠修正) と同様に CHANGES.md には `[FIX]` として記載する。

## 他 issue との依存関係

- #0030 は同じ `encrypt_payload` を `apply_aes_ctr` へ改名する。両者は同一関数を編集するため、番号順どおり本 issue (#0017) を先に対応し、その後 #0030 で改名する

## 修正対象

1. `src/crypto.rs` の `encrypt_payload` 内カウンタブロック構築を上記「設計方針」のコードに修正する (Salt の上位 14 バイトのみコピーし、packet index を bytes 10-13 に XOR する)
2. 同箇所のコメントを仕様準拠の正確な内容 (資料名・「AES Counter」サブセクション・将来変更の可能性) に更新する
3. `tests/test_crypto.rs` に KAT を追加する
4. `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加する
   - 例: `[FIX] AES-CTR のカウンタブロック構築を SRT 仕様に準拠するよう修正する`

## 完了条件

- packet index の XOR 位置がカウンタブロックの bytes 10-13 になっていること
- カウンタブロックの下位 16 bit (bytes 14-15) が 0 になっていること (Salt 下位 2 バイトが残らないこと)
- コメントが資料名・サブセクション名付きで正確な内容に更新されていること
- `tests/test_crypto.rs` の KAT が追加され、packet_index が 0 でないケースで仕様準拠の期待暗号文と一致すること
- `CHANGES.md` に `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/crypto.rs` の `encrypt_payload` のカウンタブロック構築を SRT 仕様の「AES Counter」サブセクションに準拠するよう修正した。

- `iv.copy_from_slice(salt)` (全 16 バイトコピー) を `iv[..14].copy_from_slice(&salt[..14])` に変更し、カウンタブロックの bytes 14-15 (block counter 領域) を 0 のままにした (Salt 下位 2 バイトの残留を除去)。
- packet index の XOR 位置を bytes 12-15 から bytes 10-13 (bits 16-47) に修正した。
- コメントを根拠資料名 (draft-sharabayko-srt.md)・サブセクション名 (AES Counter)・将来変更の可能性・libsrt haicrypt 実装との一致を明記する内容に更新した。

この構築が libsrt の `hcrypt_SetCtrIV` (haicrypt/hcrypt.h) とバイト単位で一致することをソースで確認した。

### テスト

- `tests/test_crypto.rs` (新規) に KAT を追加した。仕様の byte 配置で構築したカウンタブロックで計算した暗号文と `CryptoContext::encrypt` の出力を照合する。packet_index が 0 でないケース (AES-128/256)、packet_index=0 で bytes 14-15 のゼロ化を切り分けるケース、packet_index が u32::MAX 付近のケースを含む。
- KAT は `encrypt_payload` と同じ byte 解釈を共有する回帰防止テストのため、`pbt/tests/prop_crypto.rs` に非循環の PBT `test_salt_low_bytes_do_not_affect_ciphertext` を追加した。salt の下位 2 バイトだけを変えても暗号文が一致する不変条件を、`encrypt_payload` の内部を参照せず外部観測で検証する。

`CHANGES.md` の `## develop` に `[FIX]` エントリを追加した。
