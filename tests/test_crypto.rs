//! src/crypto.rs に対応する単体テスト (Known Answer Test)
//!
//! AES-CTR のカウンタブロック構築が SRT 仕様
//! (draft-sharabayko-srt.md の「Encryption」セクション内「AES Counter」サブセクション)
//! に準拠していることを検証する。
//!
//! self-roundtrip (同一の encrypt_payload で暗号化・復号する) ではカウンタブロックが
//! ずれていても送受信で同じ値が使われて成立してしまうため、ラウンドトリップ系の PBT では
//! カウンタブロック構築位置の誤りを検出できない。ここでは仕様の byte 配置に従って
//! カウンタブロックを別経路で構築し、その IV で計算した暗号文と CryptoContext::encrypt の
//! 出力が一致することを検証する。
//!
//! 注: この KAT は encrypt_payload と同じ byte 配置の解釈を用いて期待値を組み立てるため、
//! 今回修正した「XOR 位置のずれ」「bytes 14-15 への Salt 残留」のような配置のずれの回帰を
//! 検出する回帰防止テストである。byte 配置そのものの妥当性 (bytes 10-13 が packet index か等)
//! は libsrt の haicrypt 実装 (hcrypt_SetCtrIV) とのバイト単位の突き合わせで担保しており、
//! 同じ byte 解釈の誤りを spec_counter_block と encrypt_payload が共有する種類の誤りは
//! この KAT では検出できない。
//!
//! このうち「カウンタブロックの bytes 14-15 が暗号文に影響しない」という不変条件
//! (= Salt の下位 2 バイトが IV に残留しないこと) のみは、pbt/tests/prop_crypto.rs の
//! test_salt_low_bytes_do_not_affect_ciphertext が encrypt_payload の内部を参照せず外部観測で
//! 独立に検証する。一方、packet index が bytes 10-13 に置かれること自体の絶対的な正しさは
//! KAT・PBT のいずれでも独立検証されておらず、上記の libsrt 突き合わせに依拠する。

use aws_lc_rs::cipher::{AES_128, AES_256, DecryptingKey, DecryptionContext, UnboundCipherKey};
use aws_lc_rs::iv::{FixedLength, IV_LEN_128_BIT};
use shiguredo_srt::{CryptoContext, KeyLength};

/// SRT 仕様の byte 配置に従ってカウンタブロック (AES-CTR の初期 IV) を構築する。
///
/// 根拠資料: draft-sharabayko-srt.md「AES Counter」サブセクション。
/// 128-bit のカウンタブロックをビッグエンディアンの 16 バイト配列とみなすと:
///   - bytes 0-13: 上位 112 bits。IV = MSB(112, Salt) (= salt[0..14]) を置く。
///     うち bytes 10-13 には packet index を XOR する
///   - bytes 14-15: block counter 領域。先頭ブロックでは 0 (Salt とは XOR しない)
///
/// encrypt_payload とは別経路で同じ byte 配置を構築する (モジュールコメントの注を参照)。
fn spec_counter_block(salt: &[u8; 16], packet_index: u32) -> [u8; 16] {
    let mut ctr = [0u8; 16];
    ctr[..14].copy_from_slice(&salt[..14]);
    let pi = packet_index.to_be_bytes();
    ctr[10] ^= pi[0];
    ctr[11] ^= pi[1];
    ctr[12] ^= pi[2];
    ctr[13] ^= pi[3];
    ctr
}

/// 明示構築したカウンタブロックを IV として AES-CTR を適用する (暗号化・復号は同一操作)。
fn aes_ctr_apply(sek: &[u8], iv: &[u8; 16], data: &mut [u8]) {
    let algorithm = match sek.len() {
        16 => &AES_128,
        32 => &AES_256,
        _ => unreachable!("SEK length must be 16 or 32"),
    };
    let unbound =
        UnboundCipherKey::new(algorithm, sek).expect("UnboundCipherKey should be created");
    let key = DecryptingKey::ctr(unbound).expect("CTR key should be created");
    let context = DecryptionContext::Iv128(FixedLength::<IV_LEN_128_BIT>::from(iv));
    key.decrypt(data, context)
        .expect("CTR apply should succeed");
}

/// 仕様の byte 配置で構築したカウンタブロックで計算した暗号文と
/// CryptoContext::encrypt の出力を照合する。
fn assert_kat(
    key_length: KeyLength,
    salt: [u8; 16],
    sek: &[u8],
    packet_index: u32,
    plaintext: &[u8],
) {
    let mut expected = plaintext.to_vec();
    aes_ctr_apply(sek, &spec_counter_block(&salt, packet_index), &mut expected);

    let mut ctx = CryptoContext::new_sender("test_passphrase", key_length, salt, sek)
        .expect("sender should be created");
    let mut actual = plaintext.to_vec();
    ctx.encrypt(packet_index, &mut actual)
        .expect("encrypt should succeed");

    assert_eq!(
        actual, expected,
        "暗号文が仕様準拠のカウンタブロックで計算した期待値と一致すること"
    );
}

#[test]
fn aes128_kat_packet_index_nonzero() {
    // salt[14], salt[15] (= 0xF1, 0xF2) と packet_index (= 0x0A0B0C0D) がともに 0 でない。
    let salt: [u8; 16] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0xF1,
        0xF2,
    ];
    let sek: [u8; 16] = [
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E,
        0x1F,
    ];
    // 2 ブロック分 (32 バイト) の平文でブロックをまたぐ鍵ストリームも検証する。
    let plaintext: [u8; 32] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
        0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D,
        0x1E, 0x1F,
    ];
    assert_kat(KeyLength::Aes128, salt, &sek, 0x0A0B0C0D, &plaintext);
}

#[test]
fn aes128_kat_packet_index_zero() {
    // packet_index = 0 のとき、salt の下位 2 バイト (bytes 14-15) が IV に残らず 0 になることを
    // packet index の XOR 効果と切り離して検証する。salt[14], salt[15] (= 0xE1, 0xE2) は 0 でない。
    let salt: [u8; 16] = [
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xE1,
        0xE2,
    ];
    let sek: [u8; 16] = [
        0x0F, 0x0E, 0x0D, 0x0C, 0x0B, 0x0A, 0x09, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01,
        0x00,
    ];
    let plaintext: [u8; 17] = [
        0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xA0, 0xB0, 0xC0, 0xD0, 0xE0, 0xF0,
        0x0F, 0xF0,
    ];
    assert_kat(KeyLength::Aes128, salt, &sek, 0, &plaintext);
}

#[test]
fn aes128_kat_packet_index_near_max() {
    // packet_index を u32::MAX 付近にし、ビッグエンディアンのバイト配置 (bytes 10-13) が
    // 正しいことを確認する。平文長 20 バイトでブロック非整列の末尾処理も通す。
    let salt: [u8; 16] = [
        0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE,
        0xAF,
    ];
    let sek: [u8; 16] = [
        0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E,
        0x2F,
    ];
    let plaintext: [u8; 20] = [
        0xFF, 0xFE, 0xFD, 0xFC, 0xFB, 0xFA, 0xF9, 0xF8, 0xF7, 0xF6, 0xF5, 0xF4, 0xF3, 0xF2, 0xF1,
        0xF0, 0xEF, 0xEE, 0xED, 0xEC,
    ];
    assert_kat(KeyLength::Aes128, salt, &sek, 0xFFFF_FFFE, &plaintext);
}

#[test]
fn aes256_kat_packet_index_nonzero() {
    let salt: [u8; 16] = [
        0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE,
        0xBF,
    ];
    let sek: [u8; 32] = [
        0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E,
        0x3F, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D,
        0x4E, 0x4F,
    ];
    let plaintext: [u8; 32] = [
        0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
        0x55, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA,
        0xAA, 0xAA,
    ];
    assert_kat(KeyLength::Aes256, salt, &sek, 0x1234_5678, &plaintext);
}
