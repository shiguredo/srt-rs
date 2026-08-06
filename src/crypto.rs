//! SRT 暗号化モジュール
//!
//! SRT は AES-CTR で暗号化を行う。
//! - KEK (Key Encrypting Key): パスフレーズから PBKDF2 で導出
//! - SEK (Stream Encrypting Key): ランダム生成、KEK で AES Key Wrap
//! - AES-CTR でデータ暗号化

use std::num::NonZeroU32;

use aws_lc_rs::cipher::{AES_128, AES_256, DecryptingKey, DecryptionContext, UnboundCipherKey};
use aws_lc_rs::iv::{FixedLength, IV_LEN_128_BIT};
use aws_lc_rs::key_wrap::{AES_128 as KW_AES_128, AES_256 as KW_AES_256, AesKek, KeyWrap};
use aws_lc_rs::pbkdf2::{self, PBKDF2_HMAC_SHA1};

use crate::error::Error;

/// PBKDF2 のイテレーション回数 (SRT 仕様)
const PBKDF2_ITERATIONS: u32 = 2048;

/// 鍵長
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyLength {
    /// AES-128 (16 bytes)
    #[default]
    Aes128 = 16,
    /// AES-256 (32 bytes)
    Aes256 = 32,
}

impl KeyLength {
    /// バイト長を取得
    #[expect(clippy::len_without_is_empty)]
    pub fn len(self) -> usize {
        self as usize
    }

    /// バイト長から KeyLength を取得
    pub fn from_len(len: usize) -> Option<Self> {
        match len {
            16 => Some(Self::Aes128),
            32 => Some(Self::Aes256),
            _ => None,
        }
    }

    /// ハンドシェイクの Encryption Field 値から取得
    pub fn from_encryption_field(value: u16) -> Option<Self> {
        match value {
            2 => Some(Self::Aes128),
            4 => Some(Self::Aes256),
            _ => None,
        }
    }

    /// ハンドシェイクの Encryption Field 値へ変換
    pub fn to_encryption_field(self) -> u16 {
        match self {
            Self::Aes128 => 2,
            Self::Aes256 => 4,
        }
    }
}

/// 鍵フラグ (奇数/偶数)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyFlag {
    /// 偶数鍵
    #[default]
    Even = 0b01,
    /// 奇数鍵
    Odd = 0b10,
}

impl KeyFlag {
    /// KK フィールド値から取得
    pub fn from_kk_field(value: u8) -> Option<Self> {
        match value & 0b11 {
            0b01 => Some(Self::Even),
            0b10 => Some(Self::Odd),
            _ => None,
        }
    }

    /// KK フィールド値へ変換
    pub fn to_kk_field(self) -> u8 {
        self as u8
    }

    /// 反対の鍵フラグを取得
    pub fn other(self) -> Self {
        match self {
            Self::Even => Self::Odd,
            Self::Odd => Self::Even,
        }
    }
}

/// KM Refresh 状態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KmRefreshState {
    /// アイドル状態 (リフレッシュ不要)
    #[default]
    Idle,
    /// 事前通知中 (新キーを生成・送信済み、切り替え待ち)
    PreAnnounce,
    /// 鍵切り替え完了、古いキーの廃棄待ち
    PostAnnounce,
}

/// 暗号化コンテキスト
#[derive(Debug)]
pub struct CryptoContext {
    /// Key Encrypting Key (PBKDF2 で導出)
    kek: Vec<u8>,
    /// Stream Encrypting Key (偶数)
    sek_even: Vec<u8>,
    /// Stream Encrypting Key (奇数)
    sek_odd: Vec<u8>,
    /// Salt (16 bytes)
    salt: [u8; 16],
    /// 現在使用中の鍵
    current_key: KeyFlag,
    /// 鍵長
    key_length: KeyLength,
    /// 暗号化したパケット数
    encrypted_packet_count: u64,
    /// KM Refresh 状態
    km_refresh_state: KmRefreshState,
    /// 次のキー (事前通知中に生成)
    next_key: Option<KeyFlag>,
}

impl CryptoContext {
    /// KM リフレッシュ期間 (2^25 パケット)
    pub const KM_REFRESH_PERIOD: u64 = 1 << 25;

    /// KM 事前通知期間 (4000 パケット)
    pub const KM_PRE_ANNOUNCE_PERIOD: u64 = 4000;

    /// パスフレーズから暗号化コンテキストを生成 (送信側)
    ///
    /// salt と sek は外部から乱数で生成して渡す。
    pub fn new_sender(
        passphrase: &str,
        key_length: KeyLength,
        salt: [u8; 16],
        sek: &[u8],
    ) -> Result<Self, Error> {
        if sek.len() != key_length.len() {
            return Err(Error::crypto_error("invalid SEK length"));
        }

        // KEK を PBKDF2 で導出
        let kek = derive_kek(passphrase, &salt, key_length);

        // SEK (初期は偶数キーのみ)
        let sek_even = sek.to_vec();
        let sek_odd = vec![0u8; key_length.len()];

        Ok(Self {
            kek,
            sek_even,
            sek_odd,
            salt,
            current_key: KeyFlag::Even,
            key_length,
            encrypted_packet_count: 0,
            km_refresh_state: KmRefreshState::Idle,
            next_key: None,
        })
    }

    /// パスフレーズとキーマテリアルから暗号化コンテキストを生成 (受信側)
    ///
    /// KEK で SEK をアンラップする。
    pub fn new_receiver(
        passphrase: &str,
        salt: [u8; 16],
        wrapped_sek: &[u8],
        key_flag: KeyFlag,
        key_length: KeyLength,
    ) -> Result<Self, Error> {
        // KEK を PBKDF2 で導出
        let kek = derive_kek(passphrase, &salt, key_length);

        // SEK をアンラップ
        let sek = unwrap_sek(&kek, wrapped_sek, key_length)?;

        let (sek_even, sek_odd) = match key_flag {
            KeyFlag::Even => (sek.clone(), vec![0u8; key_length.len()]),
            KeyFlag::Odd => (vec![0u8; key_length.len()], sek.clone()),
        };

        Ok(Self {
            kek,
            sek_even,
            sek_odd,
            salt,
            current_key: key_flag,
            key_length,
            encrypted_packet_count: 0,
            km_refresh_state: KmRefreshState::Idle,
            next_key: None,
        })
    }

    /// Salt を取得
    pub fn salt(&self) -> &[u8; 16] {
        &self.salt
    }

    /// 現在の鍵フラグを取得
    pub fn current_key(&self) -> KeyFlag {
        self.current_key
    }

    /// 鍵長を取得
    pub fn key_length(&self) -> KeyLength {
        self.key_length
    }

    /// SEK をラップして取得 (KM メッセージ用)
    pub fn wrap_sek(&self, key_flag: KeyFlag) -> Result<Vec<u8>, Error> {
        let sek = match key_flag {
            KeyFlag::Even => &self.sek_even,
            KeyFlag::Odd => &self.sek_odd,
        };
        wrap_sek(&self.kek, sek, self.key_length)
    }

    /// データを暗号化する
    pub fn encrypt(&mut self, packet_index: u32, payload: &mut [u8]) -> Result<KeyFlag, Error> {
        let sek = match self.current_key {
            KeyFlag::Even => &self.sek_even,
            KeyFlag::Odd => &self.sek_odd,
        };

        encrypt_payload(sek, &self.salt, packet_index, payload, self.key_length)?;
        self.encrypted_packet_count += 1;

        Ok(self.current_key)
    }

    /// データを復号化する
    pub fn decrypt(
        &self,
        packet_index: u32,
        key_flag: KeyFlag,
        payload: &mut [u8],
    ) -> Result<(), Error> {
        let sek = match key_flag {
            KeyFlag::Even => &self.sek_even,
            KeyFlag::Odd => &self.sek_odd,
        };

        // AES-CTR は暗号化と復号化が同じ操作
        encrypt_payload(sek, &self.salt, packet_index, payload, self.key_length)
    }

    /// KM Refresh 状態を取得
    pub fn km_refresh_state(&self) -> KmRefreshState {
        self.km_refresh_state
    }

    /// 事前通知が必要かどうか (2^25 - 4000 パケット)
    pub fn should_pre_announce(&self) -> bool {
        self.km_refresh_state == KmRefreshState::Idle
            && self.encrypted_packet_count >= Self::KM_REFRESH_PERIOD - Self::KM_PRE_ANNOUNCE_PERIOD
    }

    /// 鍵切り替えが必要かどうか (2^25 パケット)
    pub fn should_switch_key(&self) -> bool {
        self.km_refresh_state == KmRefreshState::PreAnnounce
            && self.encrypted_packet_count >= Self::KM_REFRESH_PERIOD
    }

    /// 古い鍵の廃棄が必要かどうか (2^25 + 4000 パケット)
    pub fn should_decommission_old_key(&self) -> bool {
        self.km_refresh_state == KmRefreshState::PostAnnounce
            && self.encrypted_packet_count >= Self::KM_PRE_ANNOUNCE_PERIOD
    }

    /// 新しい SEK で事前通知を開始
    ///
    /// new_sek は外部から乱数で生成して渡す。
    /// ラップされた SEK を返す。呼び出し側は KMREQ を送信する必要がある。
    pub fn start_pre_announce(&mut self, new_sek: &[u8]) -> Result<(KeyFlag, Vec<u8>), Error> {
        if new_sek.len() != self.key_length.len() {
            return Err(Error::crypto_error("invalid SEK length"));
        }

        let new_key_flag = self.current_key.other();

        match new_key_flag {
            KeyFlag::Even => self.sek_even = new_sek.to_vec(),
            KeyFlag::Odd => self.sek_odd = new_sek.to_vec(),
        }

        self.next_key = Some(new_key_flag);
        self.km_refresh_state = KmRefreshState::PreAnnounce;

        let wrapped_sek = self.wrap_sek(new_key_flag)?;
        Ok((new_key_flag, wrapped_sek))
    }

    /// 鍵を切り替える (2^25 パケット到達時)
    pub fn switch_key(&mut self) {
        if let Some(next_key) = self.next_key.take() {
            self.current_key = next_key;
            self.encrypted_packet_count = 0;
            self.km_refresh_state = KmRefreshState::PostAnnounce;
        }
    }

    /// 古い鍵を廃棄 (2^25 + 4000 パケット到達時)
    pub fn decommission_old_key(&mut self) {
        // 古い鍵をゼロクリア
        let old_key = self.current_key.other();
        match old_key {
            KeyFlag::Even => self.sek_even.fill(0),
            KeyFlag::Odd => self.sek_odd.fill(0),
        }
        self.km_refresh_state = KmRefreshState::Idle;
    }

    /// 受信した KM メッセージから SEK を更新
    pub fn update_sek(&mut self, wrapped_sek: &[u8], key_flag: KeyFlag) -> Result<(), Error> {
        let sek = unwrap_sek(&self.kek, wrapped_sek, self.key_length)?;

        match key_flag {
            KeyFlag::Even => self.sek_even = sek,
            KeyFlag::Odd => self.sek_odd = sek,
        }

        self.current_key = key_flag;
        Ok(())
    }
}

/// PBKDF2 で KEK を導出
fn derive_kek(passphrase: &str, salt: &[u8; 16], key_length: KeyLength) -> Vec<u8> {
    let mut kek = vec![0u8; key_length.len()];
    // Salt の下位 64 bits (8 bytes) を使用
    let salt_lsb = &salt[8..16];
    let iterations = NonZeroU32::new(PBKDF2_ITERATIONS).expect("iterations should be non-zero");
    pbkdf2::derive(
        PBKDF2_HMAC_SHA1,
        iterations,
        salt_lsb,
        passphrase.as_bytes(),
        &mut kek,
    );
    kek
}

/// SEK を AES Key Wrap でラップ
fn wrap_sek(kek: &[u8], sek: &[u8], key_length: KeyLength) -> Result<Vec<u8>, Error> {
    let algorithm = match key_length {
        KeyLength::Aes128 => &KW_AES_128,
        KeyLength::Aes256 => &KW_AES_256,
    };
    let aes_kek = AesKek::new(algorithm, kek)
        .map_err(|e| Error::crypto_error(format!("invalid KEK: {e}")))?;
    let mut wrapped = vec![0u8; sek.len() + 8];
    aes_kek
        .wrap(sek, &mut wrapped)
        .map_err(|e| Error::crypto_error(format!("AES key wrap failed: {e}")))?;
    Ok(wrapped)
}

/// SEK を AES Key Wrap でアンラップ
fn unwrap_sek(kek: &[u8], wrapped: &[u8], key_length: KeyLength) -> Result<Vec<u8>, Error> {
    if wrapped.len() < 8 {
        return Err(Error::crypto_error("wrapped key too short"));
    }

    let algorithm = match key_length {
        KeyLength::Aes128 => &KW_AES_128,
        KeyLength::Aes256 => &KW_AES_256,
    };
    let aes_kek = AesKek::new(algorithm, kek)
        .map_err(|e| Error::crypto_error(format!("invalid KEK: {e}")))?;
    let mut unwrapped = vec![0u8; wrapped.len() - 8];
    aes_kek
        .unwrap(wrapped, &mut unwrapped)
        .map_err(|e| Error::crypto_error(format!("AES key unwrap failed: {e}")))?;
    Ok(unwrapped)
}

/// AES-CTR でペイロードを暗号化/復号化
fn encrypt_payload(
    sek: &[u8],
    salt: &[u8; 16],
    packet_index: u32,
    payload: &mut [u8],
    key_length: KeyLength,
) -> Result<(), Error> {
    // カウンタブロック (AES-CTR の初期 IV) を構築する。
    // 根拠資料: draft-sharabayko-srt.md「Encryption」セクション内「AES Counter」サブセクション。
    // 128-bit のカウンタブロックをビッグエンディアンの 16 バイト配列とみなすと:
    //   - bits 0-15 (bytes 14-15): block counter。各パケットの先頭ブロックでは 0。Salt とは XOR しない
    //   - bits 16-47 (bytes 10-13): packet index
    //   - bits 48-127 (bytes 0-9): ゼロ
    //   - 上位 112 bits (bytes 0-13) を IV = MSB(112, Salt) (= salt[0..14]) と XOR する
    // この構造は libsrt の haicrypt 実装のカウンタブロックと一致する。
    // 仕様の節構成・行番号・式表現は将来変更される可能性がある。
    let mut iv = [0u8; 16];
    // 上位 112 bits (bytes 0-13) に IV = MSB(112, Salt) を置く。bytes 14-15 は 0 のまま。
    iv[..14].copy_from_slice(&salt[..14]);

    // packet index を bytes 10-13 に XOR する (to_be_bytes は [MSB, .., LSB])。
    let pi_bytes = packet_index.to_be_bytes();
    iv[10] ^= pi_bytes[0];
    iv[11] ^= pi_bytes[1];
    iv[12] ^= pi_bytes[2];
    iv[13] ^= pi_bytes[3];

    let algorithm = match key_length {
        KeyLength::Aes128 => &AES_128,
        KeyLength::Aes256 => &AES_256,
    };

    let unbound_key = UnboundCipherKey::new(algorithm, sek)
        .map_err(|e| Error::crypto_error(format!("invalid SEK: {e}")))?;

    // CTR モードでは暗号化と復号化は同じ操作
    // DecryptionContext を使ってカスタム IV を指定
    let key = DecryptingKey::ctr(unbound_key)
        .map_err(|e| Error::crypto_error(format!("failed to create CTR key: {e}")))?;

    let context = DecryptionContext::Iv128(FixedLength::<IV_LEN_128_BIT>::from(&iv));
    key.decrypt(payload, context)
        .map_err(|e| Error::crypto_error(format!("encryption failed: {e}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_length() {
        assert_eq!(KeyLength::Aes128.len(), 16);
        assert_eq!(KeyLength::Aes256.len(), 32);
    }

    #[test]
    fn test_derive_kek() {
        let passphrase = "test_passphrase";
        let salt = [0u8; 16];
        let kek = derive_kek(passphrase, &salt, KeyLength::Aes128);
        assert_eq!(kek.len(), 16);
    }

    #[test]
    fn test_wrap_unwrap_sek() {
        let kek = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let sek = vec![
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ];

        let wrapped = wrap_sek(&kek, &sek, KeyLength::Aes128)
            .expect("SEK のラップは有効な入力では成功する想定");
        let unwrapped = unwrap_sek(&kek, &wrapped, KeyLength::Aes128)
            .expect("ラップ済み SEK のアンラップは成功する想定");

        assert_eq!(sek, unwrapped);
    }

    #[test]
    fn test_encrypt_decrypt() {
        let sek = vec![
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let salt = [0u8; 16];
        let packet_index = 12345u32;
        let original = b"Hello, SRT!".to_vec();

        let mut encrypted = original.clone();
        encrypt_payload(&sek, &salt, packet_index, &mut encrypted, KeyLength::Aes128)
            .expect("暗号化は有効な入力では成功する想定");

        // 暗号化されていることを確認
        assert_ne!(original, encrypted);

        // 復号化
        encrypt_payload(&sek, &salt, packet_index, &mut encrypted, KeyLength::Aes128)
            .expect("復号化は有効な入力では成功する想定");

        // 元に戻っていることを確認
        assert_eq!(original, encrypted);
    }

    #[test]
    fn test_km_refresh_state_transitions() {
        let salt = [0u8; 16];
        let sek = vec![0x42u8; 16];
        let mut crypto = CryptoContext::new_sender("passphrase", KeyLength::Aes128, salt, &sek)
            .expect("Sender コンテキストの生成は成功する想定");

        // 初期状態
        assert_eq!(crypto.km_refresh_state(), KmRefreshState::Idle);
        assert_eq!(crypto.current_key(), KeyFlag::Even);

        // 事前通知開始
        let new_sek = vec![0x43u8; 16];
        let (new_key, wrapped) = crypto
            .start_pre_announce(&new_sek)
            .expect("事前通知は正常な鍵で成功する想定");
        assert_eq!(new_key, KeyFlag::Odd);
        assert!(!wrapped.is_empty());
        assert_eq!(crypto.km_refresh_state(), KmRefreshState::PreAnnounce);

        // 鍵切り替え
        crypto.switch_key();
        assert_eq!(crypto.current_key(), KeyFlag::Odd);
        assert_eq!(crypto.km_refresh_state(), KmRefreshState::PostAnnounce);

        // 古い鍵の廃棄
        crypto.decommission_old_key();
        assert_eq!(crypto.km_refresh_state(), KmRefreshState::Idle);
    }

    #[test]
    fn test_km_refresh_should_pre_announce() {
        let salt = [0u8; 16];
        let sek = vec![0x42u8; 16];
        let mut crypto = CryptoContext::new_sender("passphrase", KeyLength::Aes128, salt, &sek)
            .expect("Sender コンテキストの生成は成功する想定");

        // 初期状態では事前通知不要
        assert!(!crypto.should_pre_announce());

        // 2^25 - 4000 パケット未満では事前通知不要
        crypto.encrypted_packet_count =
            CryptoContext::KM_REFRESH_PERIOD - CryptoContext::KM_PRE_ANNOUNCE_PERIOD - 1;
        assert!(!crypto.should_pre_announce());

        // 2^25 - 4000 パケット以上で事前通知が必要
        crypto.encrypted_packet_count =
            CryptoContext::KM_REFRESH_PERIOD - CryptoContext::KM_PRE_ANNOUNCE_PERIOD;
        assert!(crypto.should_pre_announce());
    }

    #[test]
    fn test_key_flag_kk_field_mapping() {
        assert_eq!(KeyFlag::Even.to_kk_field(), 0b01);
        assert_eq!(KeyFlag::Odd.to_kk_field(), 0b10);
        assert_eq!(KeyFlag::from_kk_field(0b01), Some(KeyFlag::Even));
        assert_eq!(KeyFlag::from_kk_field(0b10), Some(KeyFlag::Odd));
        assert_eq!(KeyFlag::from_kk_field(0b00), None);
        assert_eq!(KeyFlag::from_kk_field(0b11), None);
    }

    #[test]
    fn test_km_refresh_encrypt_with_key_switch() {
        let salt = [0u8; 16];
        let sek = vec![0x42u8; 16];
        let mut crypto = CryptoContext::new_sender("passphrase", KeyLength::Aes128, salt, &sek)
            .expect("Sender コンテキストの生成は成功する想定");
        let mut payload1 = b"test data 1".to_vec();
        let mut payload2 = b"test data 2".to_vec();

        // 偶数キーで暗号化
        let key1 = crypto
            .encrypt(1, &mut payload1)
            .expect("暗号化は成功する想定");
        assert_eq!(key1, KeyFlag::Even);

        // 事前通知開始
        let new_sek = vec![0x43u8; 16];
        let _ = crypto
            .start_pre_announce(&new_sek)
            .expect("事前通知は正常な鍵で成功する想定");

        // まだ偶数キーで暗号化
        let key2 = crypto
            .encrypt(2, &mut payload2)
            .expect("暗号化は成功する想定");
        assert_eq!(key2, KeyFlag::Even);

        // 鍵切り替え
        crypto.switch_key();

        // 奇数キーで暗号化
        let mut payload3 = b"test data 3".to_vec();
        let key3 = crypto
            .encrypt(3, &mut payload3)
            .expect("暗号化は成功する想定");
        assert_eq!(key3, KeyFlag::Odd);
    }

    #[test]
    fn test_unwrap_sek_short_input() {
        let kek = vec![0x42u8; 16];

        assert!(unwrap_sek(&kek, &[], KeyLength::Aes128).is_err());
        assert!(unwrap_sek(&kek, &[0; 7], KeyLength::Aes128).is_err());
    }
}
