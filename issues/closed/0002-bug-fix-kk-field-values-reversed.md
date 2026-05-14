# KeyFlag の KK フィールド値が SRT 仕様と逆転している

- Priority: High
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-kk-field-values-reversed

## 目的

`src/crypto.rs` の `KeyFlag` 列挙型の判別子値、`from_kk_field`、`to_kk_field` が SRT 仕様と逆転している。仕様準拠に修正し、libsrt など他実装との暗号化通信を可能にする。

## 優先度根拠

暗号化有効時、受信側が KK フィールド値から偶数/奇数 SEK を特定して復号する。値が逆転しているため、仕様準拠の実装との通信では常に誤った SEK で復号を試み、全パケットの復号が失敗する。暗号化は SRT の主要機能であり、相互運用性を完全に阻害するバグとして最優先で修正する。

`from_kk_field` と `to_kk_field` が互いに逆関数として整合しているため、srt-rs 同士の通信や既存の PBT ラウンドトリップテストではこのバグを検出できない。

## 現状

`KeyFlag` の判別子値と `from_kk_field` / `to_kk_field` の 3 箇所が揃って逆転している:

```rust
// src/crypto.rs
pub enum KeyFlag {
    #[default]
    Even = 0b10,  // 仕様では 0b01
    Odd = 0b01,   // 仕様では 0b10
}

pub fn from_kk_field(value: u8) -> Option<Self> {
    match value & 0b11 {
        0b01 => Some(Self::Odd),   // 仕様では Even
        0b10 => Some(Self::Even),  // 仕様では Odd
        _ => None,
    }
}

pub fn to_kk_field(&self) -> u8 {
    *self as u8  // Even → 0b10 (仕様では 0b01), Odd → 0b01 (仕様では 0b10)
}
```

また `srt_packet.rs` のコメントも逆転している:

```rust
// src/srt_packet.rs:109-111
/// 暗号化キーフラグ (KK, 2 bits)
/// 00b: 暗号化なし, 01b: 奇数キー, 10b: 偶数キー  // 仕様と逆
pub encryption_flag: u8,
```

## 根拠

draft-sharabayko-srt.md の 2 箇所で KK フィールドの値が定義されている。

### データパケットの KK フィールド (§3.1, 391-394 行目)

> KK: 2 bits.
> : Key-based Encryption Flag. The flag bits indicate whether or not data is encrypted.
>   The value "00b" (binary) means data is not encrypted. "01b" indicates that data is
>   encrypted with an even key, and "10b" is used for odd key encryption. Refer to {{encryption}}.

### Key Material の KK フィールド (§3.2.2, 873-879 行目)

> Key-based Encryption (KK): 2 bits.
> : This is a fixed-width field that indicates which SEKs (odd and/or even) are provided in the extension:
>
> - 00b: No SEK is provided (invalid extension format);
> - 01b: Even key is provided;
> - 10b: Odd key is provided;
> - 11b: Both even and odd keys are provided.

### KM Refresh における KK フィールドの運用 (§6.1.6, 3111-3112 行目)

> The receiver knows which SEK (odd or even) was used to encrypt the packet by means of the KK field of the SRT Data Packet ({{data-pkt}}).

## 設計方針

`KeyFlag` の判別子値を仕様通りに修正し、`from_kk_field` のマッチアームを入れ替える。`to_kk_field` は `*self as u8` のため、判別子値の修正で自動的に正しくなる。

バリアント名 (`Even` / `Odd`) は変更しない。意味的に正しい名前であり、公開 API のシンボル名を変更する理由がない。`#[default]` は `Even` のままとする。SRT 仕様で初期鍵は偶数キーであり、`CryptoContext::new_sender` も `current_key: KeyFlag::Even` と明示指定しているため問題ない。

### 修正対象

1. `src/crypto.rs` の `KeyFlag` 判別子値を `Even = 0b01`、`Odd = 0b10` に修正する
2. `src/crypto.rs` の `from_kk_field` のマッチアームを入れ替える:
   - 修正前: `0b01 => Odd`, `0b10 => Even`
   - 修正後: `0b01 => Even`, `0b10 => Odd`
3. `src/srt_packet.rs` のコメントを仕様通りに修正する:
   - 修正前: `01b: 奇数キー, 10b: 偶数キー`
   - 修正後: `01b: 偶数キー, 10b: 奇数キー`
4. `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加する:
   - `[FIX] KeyFlag の KK フィールド判別子値を SRT 仕様に準拠するよう修正する`

### 修正不要の確認項目

以下は `from_kk_field` / `to_kk_field` を呼び出しているだけのため、上記修正で自動的に正しくなる。コード変更は不要だが、動作確認は必要:

- `src/srt_handshake.rs` の `KmMessage::encode` (`key_flag.to_kk_field()`)
- `src/srt_handshake.rs` の `KmMessage::decode` (`KeyFlag::from_kk_field(kk_byte)`)
- `src/srt_connection.rs` の送信パケットの暗号化フラグ設定 (`key_flag.to_kk_field()`)
- `src/srt_connection.rs` の受信パケット復号時のキーフラグ解釈 (`KeyFlag::from_kk_field(pkt.encryption_flag)`)

### スコープ外

`from_kk_field` は `0b11` に対して `None` を返す。仕様 §3.2.2 では `11b: Both even and odd keys are provided` と定義されているが、`KeyFlag` 型に `Both` バリアントがないため現状では表現できない。`0b11` 対応は本 issue のスコープ外とし、必要であれば別 issue で対応する。

### テスト戦略

既存の PBT (`pbt/tests/prop_crypto.rs`) のうち、`test_key_flag_from_kk_field_valid` はラウンドトリップのみを検証しているため修正後も通過する。このテストがバグを検出できなかった根因は、仕様の具体値マッピング (`0b01 = Even`) をハードアサートしていないこと。

修正後、`src/crypto.rs` の `#[cfg(test)] mod tests` に仕様値を直接検証する単体テストを追加する:

- `KeyFlag::Even.to_kk_field() == 0b01`
- `KeyFlag::Odd.to_kk_field() == 0b10`
- `KeyFlag::from_kk_field(0b01) == Some(KeyFlag::Even)`
- `KeyFlag::from_kk_field(0b10) == Some(KeyFlag::Odd)`

これは仕様の具体値に対するハードアサーションであり、PBT のラウンドトリップとは役割が異なるため単体テストで記述する。

## 完了条件

- `KeyFlag::Even` の判別子値が `0b01`、`KeyFlag::Odd` の判別子値が `0b10` になっていること
- `from_kk_field` / `to_kk_field` が仕様通りのマッピングを行うこと
- `srt_packet.rs` のコメントが仕様と一致していること
- 仕様値を直接検証する単体テストが追加されていること
- 既存の全テスト (`cargo test`) が通過すること
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること

## 解決方法

1. `src/crypto.rs` の `KeyFlag` 判別子値を仕様通りに修正:
   - `Even = 0b01`、`Odd = 0b10` に変更
2. `src/crypto.rs` の `from_kk_field` のマッチアームを入れ替え:
   - `0b01 => Some(KeyFlag::Even)`、`0b10 => Some(KeyFlag::Odd)` に修正
3. `src/srt_packet.rs` のコメントを修正:
   - `01b: 偶数キー, 10b: 奇数キー` に変更
4. `src/crypto.rs` のテストに `test_key_flag_kk_field_mapping` を追加し、仕様値をハードアサート
5. `CHANGES.md` に `[FIX]` エントリを追加
