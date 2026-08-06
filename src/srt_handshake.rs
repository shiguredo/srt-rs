//! SRT ハンドシェイク
//!
//! Caller-Listener モードのハンドシェイクを実装する。
//!
//! ## フロー
//!
//! ```text
//! Caller                              Listener
//!   |                                    |
//!   |------ INDUCTION (version=4) ------>|
//!   |<----- INDUCTION (cookie) ----------|
//!   |                                    |
//!   |------ CONCLUSION (HS ext) -------->|
//!   |<----- CONCLUSION (HS ext) ---------|
//!   |                                    |
//! ```

use std::net::IpAddr;

use crate::buf::{
    read_bytes, read_u8, read_u16, read_u32, write_bytes, write_u8, write_u16, write_u32,
};
use crate::crypto::{KeyFlag, KeyLength};
use crate::error::Error;
use crate::srt_packet::{ControlPacket, ControlType};

/// ハンドシェイクバージョン
pub const HS_VERSION_4: u32 = 4;
pub const HS_VERSION_5: u32 = 5;

/// デフォルト MTU サイズ
pub const DEFAULT_MTU: u32 = 1500;

/// デフォルトフローウィンドウサイズ
pub const DEFAULT_FLOW_WINDOW: u32 = 8192;

/// ハンドシェイクタイプ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum HandshakeType {
    /// DONE (0xFFFFFFFD)
    Done = 0xFFFFFFFD,
    /// AGREEMENT (0xFFFFFFFE)
    Agreement = 0xFFFFFFFE,
    /// CONCLUSION (0xFFFFFFFF)
    Conclusion = 0xFFFFFFFF,
    /// WAVEAHAND (0x00000000)
    Waveahand = 0x00000000,
    /// INDUCTION (0x00000001)
    Induction = 0x00000001,
}

impl HandshakeType {
    /// u32 から変換
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0xFFFFFFFD => Some(Self::Done),
            0xFFFFFFFE => Some(Self::Agreement),
            0xFFFFFFFF => Some(Self::Conclusion),
            0x00000000 => Some(Self::Waveahand),
            0x00000001 => Some(Self::Induction),
            _ => None,
        }
    }
}

/// SRT Magic Code (HSv5 確認用)
pub const SRT_MAGIC_CODE: u16 = 0x4A17;

/// ハンドシェイク拡張フラグ
pub mod extension_flags {
    /// HSREQ 拡張
    pub const HSREQ: u16 = 0x0001;
    /// KMREQ 拡張
    pub const KMREQ: u16 = 0x0002;
    /// CONFIG 拡張
    pub const CONFIG: u16 = 0x0004;
}

/// ハンドシェイク拡張タイプ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ExtensionType {
    /// ハンドシェイク拡張リクエスト
    HsReq = 1,
    /// ハンドシェイク拡張レスポンス
    HsRsp = 2,
    /// キーマテリアルリクエスト
    KmReq = 3,
    /// キーマテリアルレスポンス
    KmRsp = 4,
    /// ストリーム ID
    Sid = 5,
    /// 輻輳制御
    Congestion = 6,
    /// パケットフィルタ
    Filter = 7,
    /// グループ
    Group = 8,
}

impl ExtensionType {
    /// u16 から変換
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::HsReq),
            2 => Some(Self::HsRsp),
            3 => Some(Self::KmReq),
            4 => Some(Self::KmRsp),
            5 => Some(Self::Sid),
            6 => Some(Self::Congestion),
            7 => Some(Self::Filter),
            8 => Some(Self::Group),
            _ => None,
        }
    }
}

/// SRT フラグ
pub mod srt_flags {
    /// TSBPD 送信有効
    pub const TSBPDSND: u32 = 0x00000001;
    /// TSBPD 受信有効
    pub const TSBPDRCV: u32 = 0x00000002;
    /// 暗号化対応
    pub const CRYPT: u32 = 0x00000004;
    /// Too-late packet drop 有効
    pub const TLPKTDROP: u32 = 0x00000008;
    /// 定期 NAK 有効
    pub const PERIODICNAK: u32 = 0x00000010;
    /// 再送フラグ対応
    pub const REXMITFLG: u32 = 0x00000020;
    /// ストリームモード
    pub const STREAM: u32 = 0x00000040;
    /// パケットフィルタ対応
    pub const PACKET_FILTER: u32 = 0x00000080;
}

/// ハンドシェイクパケット
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakePacket {
    /// ハンドシェイクバージョン
    pub version: u32,
    /// 暗号化フィールド
    pub encryption_field: u16,
    /// 拡張フィールド
    pub extension_field: u16,
    /// 初期パケットシーケンス番号
    pub initial_packet_seq: u32,
    /// MTU サイズ
    pub mtu: u32,
    /// フローウィンドウサイズ
    pub flow_window: u32,
    /// ハンドシェイクタイプ
    pub handshake_type: HandshakeType,
    /// SRT ソケット ID
    pub socket_id: u32,
    /// SYN Cookie
    pub syn_cookie: u32,
    /// ピア IP アドレス
    pub peer_ip: IpAddr,
    /// 拡張
    pub extensions: Vec<HandshakeExtension>,
}

impl HandshakePacket {
    /// 新しい INDUCTION リクエストを作成 (Caller)
    pub fn new_induction_request(socket_id: u32) -> Self {
        Self {
            version: HS_VERSION_4,
            encryption_field: 0,
            extension_field: 2, // Magic value for HS v5
            initial_packet_seq: 0,
            mtu: DEFAULT_MTU,
            flow_window: DEFAULT_FLOW_WINDOW,
            handshake_type: HandshakeType::Induction,
            socket_id,
            syn_cookie: 0,
            peer_ip: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            extensions: Vec::new(),
        }
    }

    /// 新しい INDUCTION レスポンスを作成 (Listener)
    pub fn new_induction_response(socket_id: u32, syn_cookie: u32, encryption_field: u16) -> Self {
        Self {
            version: HS_VERSION_5,
            encryption_field,
            extension_field: SRT_MAGIC_CODE, // HSv5 確認
            initial_packet_seq: 0,
            mtu: DEFAULT_MTU,
            flow_window: DEFAULT_FLOW_WINDOW,
            handshake_type: HandshakeType::Induction,
            socket_id,
            syn_cookie,
            peer_ip: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            extensions: Vec::new(),
        }
    }

    /// 新しい CONCLUSION リクエストを作成 (Caller)
    pub fn new_conclusion_request(
        socket_id: u32,
        syn_cookie: u32,
        initial_packet_seq: u32,
        encryption_field: u16,
        has_encryption: bool,
    ) -> Self {
        let extension_field = if has_encryption {
            extension_flags::HSREQ | extension_flags::KMREQ
        } else {
            extension_flags::HSREQ
        };
        Self {
            version: HS_VERSION_5,
            encryption_field,
            extension_field,
            initial_packet_seq,
            mtu: DEFAULT_MTU,
            flow_window: DEFAULT_FLOW_WINDOW,
            handshake_type: HandshakeType::Conclusion,
            socket_id,
            syn_cookie,
            peer_ip: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            extensions: Vec::new(),
        }
    }

    /// 新しい CONCLUSION レスポンスを作成 (Listener)
    pub fn new_conclusion_response(
        socket_id: u32,
        syn_cookie: u32,
        initial_packet_seq: u32,
        encryption_field: u16,
        has_encryption: bool,
    ) -> Self {
        let extension_field = if has_encryption {
            extension_flags::HSREQ | extension_flags::KMREQ
        } else {
            extension_flags::HSREQ
        };
        Self {
            version: HS_VERSION_5,
            encryption_field,
            extension_field,
            initial_packet_seq,
            mtu: DEFAULT_MTU,
            flow_window: DEFAULT_FLOW_WINDOW,
            handshake_type: HandshakeType::Conclusion,
            socket_id,
            syn_cookie,
            peer_ip: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            extensions: Vec::new(),
        }
    }

    /// 制御パケットからデコード
    #[track_caller]
    pub fn decode(packet: &ControlPacket) -> Result<Self, Error> {
        if packet.control_type != ControlType::Handshake {
            return Err(Error::invalid_data("not a handshake packet"));
        }

        let mut buf = packet.control_info.as_slice();
        Error::check_buffer_size(48, buf)?; // 最小サイズ

        let version = read_u32(&mut buf)?;
        let encryption_field = read_u16(&mut buf)?;
        let extension_field = read_u16(&mut buf)?;
        let initial_packet_seq = read_u32(&mut buf)? & 0x7FFF_FFFF;
        let mtu = read_u32(&mut buf)?;
        let flow_window = read_u32(&mut buf)?;
        let handshake_type_raw = read_u32(&mut buf)?;
        let handshake_type = HandshakeType::from_u32(handshake_type_raw).ok_or_else(|| {
            Error::invalid_data(format!("unknown handshake type: {handshake_type_raw:#x}"))
        })?;
        let socket_id = read_u32(&mut buf)?;
        let syn_cookie = read_u32(&mut buf)?;

        // ピア IP (128 ビット = 16 バイト)
        let ip_bytes = read_bytes(&mut buf, 16)?;
        let peer_ip = parse_peer_ip(&ip_bytes);

        // 拡張のパース
        let mut extensions = Vec::new();
        while buf.len() >= 4 {
            let ext_type_raw = read_u16(&mut buf)?;
            let ext_len = read_u16(&mut buf)? as usize * 4; // 4バイト単位

            if buf.len() < ext_len {
                break;
            }

            let ext_data = read_bytes(&mut buf, ext_len)?;

            if let Some(ext_type) = ExtensionType::from_u16(ext_type_raw) {
                extensions.push(HandshakeExtension {
                    ext_type,
                    data: ext_data,
                });
            }
        }

        Ok(Self {
            version,
            encryption_field,
            extension_field,
            initial_packet_seq,
            mtu,
            flow_window,
            handshake_type,
            socket_id,
            syn_cookie,
            peer_ip,
            extensions,
        })
    }

    /// 制御パケットにエンコード
    pub fn encode(&self, timestamp: u32, dest_socket_id: u32) -> ControlPacket {
        let mut control_info = Vec::new();

        write_u32(&mut control_info, self.version);
        write_u16(&mut control_info, self.encryption_field);
        write_u16(&mut control_info, self.extension_field);
        write_u32(&mut control_info, self.initial_packet_seq & 0x7FFF_FFFF);
        write_u32(&mut control_info, self.mtu);
        write_u32(&mut control_info, self.flow_window);
        write_u32(&mut control_info, self.handshake_type as u32);
        write_u32(&mut control_info, self.socket_id);
        write_u32(&mut control_info, self.syn_cookie);

        // Peer IP
        encode_peer_ip(&self.peer_ip, &mut control_info);

        // 拡張
        for ext in &self.extensions {
            write_u16(&mut control_info, ext.ext_type as u16);
            let len_in_words = ext.data.len().div_ceil(4);
            write_u16(&mut control_info, len_in_words as u16);
            write_bytes(&mut control_info, &ext.data);
            // パディング
            let padding = len_in_words * 4 - ext.data.len();
            for _ in 0..padding {
                write_u8(&mut control_info, 0);
            }
        }

        ControlPacket {
            control_type: ControlType::Handshake,
            subtype: 0,
            type_specific_info: 0,
            timestamp,
            dest_socket_id,
            control_info,
        }
    }

    /// HSREQ 拡張を追加
    pub fn add_hs_extension(&mut self, srt_version: u32, srt_flags: u32, tsbpd_delay: u16) {
        let mut data = Vec::new();
        write_u32(&mut data, srt_version);
        write_u32(&mut data, srt_flags);
        write_u16(&mut data, tsbpd_delay); // Receiver TSBPD delay
        write_u16(&mut data, tsbpd_delay); // Sender TSBPD delay

        self.extensions.push(HandshakeExtension {
            ext_type: ExtensionType::HsReq,
            data,
        });
    }

    /// HSRSP 拡張を追加
    pub fn add_hs_response(&mut self, srt_version: u32, srt_flags: u32, tsbpd_delay: u16) {
        let mut data = Vec::new();
        write_u32(&mut data, srt_version);
        write_u32(&mut data, srt_flags);
        write_u16(&mut data, tsbpd_delay);
        write_u16(&mut data, tsbpd_delay);

        self.extensions.push(HandshakeExtension {
            ext_type: ExtensionType::HsRsp,
            data,
        });
    }

    /// HSREQ/HSRSP 拡張を取得
    pub fn get_hs_extension(&self) -> Option<HsExtensionData> {
        for ext in &self.extensions {
            if (ext.ext_type == ExtensionType::HsReq || ext.ext_type == ExtensionType::HsRsp)
                && ext.data.len() >= 12
            {
                let mut buf = ext.data.as_slice();
                let srt_version = read_u32(&mut buf).ok()?;
                let srt_flags = read_u32(&mut buf).ok()?;
                let recv_tsbpd_delay = read_u16(&mut buf).ok()?;
                let send_tsbpd_delay = read_u16(&mut buf).ok()?;
                return Some(HsExtensionData {
                    srt_version,
                    srt_flags,
                    recv_tsbpd_delay,
                    send_tsbpd_delay,
                });
            }
        }
        None
    }

    /// 鍵長を取得
    pub fn key_length(&self) -> Option<KeyLength> {
        KeyLength::from_encryption_field(self.encryption_field)
    }

    /// KMREQ 拡張を追加
    pub fn add_km_request(&mut self, km_message: &KmMessage) {
        self.extensions.push(HandshakeExtension {
            ext_type: ExtensionType::KmReq,
            data: km_message.encode(),
        });
    }

    /// KMRSP 拡張を追加 (成功時: 同じ KM メッセージを返す)
    pub fn add_km_response(&mut self, km_message: &KmMessage) {
        self.extensions.push(HandshakeExtension {
            ext_type: ExtensionType::KmRsp,
            data: km_message.encode(),
        });
    }

    /// KMRSP エラー拡張を追加 (失敗時)
    pub fn add_km_error(&mut self, error: KmError) {
        let mut data = Vec::new();
        write_u32(&mut data, error as u32);
        self.extensions.push(HandshakeExtension {
            ext_type: ExtensionType::KmRsp,
            data,
        });
    }

    /// KMREQ 拡張を取得
    pub fn get_km_request(&self) -> Option<Result<KmMessage, Error>> {
        for ext in &self.extensions {
            if ext.ext_type == ExtensionType::KmReq {
                return Some(KmMessage::decode(&ext.data));
            }
        }
        None
    }

    /// KMRSP 拡張を取得
    ///
    /// 成功時は Ok(Some(KmMessage))、エラー時は Err(KmError) を返す。
    /// KMRSP 拡張がない場合は Ok(None) を返す。
    pub fn get_km_response(&self) -> Result<Option<KmMessage>, KmError> {
        for ext in &self.extensions {
            if ext.ext_type == ExtensionType::KmRsp {
                // エラーレスポンスの場合は 4 バイト
                if ext.data.len() == 4 {
                    let error_code =
                        u32::from_be_bytes([ext.data[0], ext.data[1], ext.data[2], ext.data[3]]);
                    if let Some(km_error) = KmError::from_u32(error_code) {
                        return Err(km_error);
                    }
                }
                // 正常な KM メッセージ
                match KmMessage::decode(&ext.data) {
                    Ok(km) => return Ok(Some(km)),
                    Err(_) => continue,
                }
            }
        }
        Ok(None)
    }

    /// Stream ID 拡張を追加
    ///
    /// Stream ID は UTF-8 文字列で、最大 512 バイト。
    /// 32-bit little endian words として格納される。
    pub fn add_sid_extension(&mut self, stream_id: &str) {
        self.extensions.push(HandshakeExtension {
            ext_type: ExtensionType::Sid,
            data: encode_le_words(stream_id, 512),
        });
    }

    /// Stream ID 拡張を取得
    ///
    /// Stream ID は 32-bit little endian words として格納されているため、
    /// デコード時にバイト順を復元する。
    pub fn get_sid_extension(&self) -> Option<String> {
        for ext in &self.extensions {
            if ext.ext_type == ExtensionType::Sid {
                return decode_le_words(&ext.data);
            }
        }
        None
    }

    /// Congestion 拡張を追加
    ///
    /// 輻輳制御アルゴリズムを指定する。
    /// ライブストリーミングでは "live" を使用する。
    ///
    /// Stream ID と同様に 32-bit little endian words として格納される。
    pub fn add_congestion_extension(&mut self, congestion_control: &str) {
        self.extensions.push(HandshakeExtension {
            ext_type: ExtensionType::Congestion,
            data: encode_le_words(congestion_control, 512),
        });
    }

    /// Congestion 拡張を取得
    ///
    /// 輻輳制御アルゴリズム名を取得する。
    /// 例: "live", "file"
    pub fn get_congestion_extension(&self) -> Option<String> {
        for ext in &self.extensions {
            if ext.ext_type == ExtensionType::Congestion {
                return decode_le_words(&ext.data);
            }
        }
        None
    }
}

/// 文字列を 32-bit little endian words 形式にエンコードする
fn encode_le_words(s: &str, max_len: usize) -> Vec<u8> {
    let bytes = s.as_bytes();
    let len = bytes.len().min(max_len);
    let truncated = &bytes[..len];

    let padded_len = (len + 3) & !3;
    let mut data = vec![0u8; padded_len];

    for (i, chunk) in truncated.chunks(4).enumerate() {
        let offset = i * 4;
        for (j, &byte) in chunk.iter().enumerate() {
            data[offset + (3 - j)] = byte;
        }
    }

    data
}

/// 32-bit little endian words 形式から文字列をデコードする
fn decode_le_words(data: &[u8]) -> Option<String> {
    let mut bytes = Vec::new();

    for chunk in data.chunks(4) {
        for i in (0..chunk.len()).rev() {
            bytes.push(chunk[i]);
        }
    }

    while bytes.last() == Some(&0) {
        bytes.pop();
    }

    String::from_utf8(bytes).ok()
}

/// ハンドシェイク拡張
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandshakeExtension {
    /// 拡張タイプ
    pub ext_type: ExtensionType,
    /// 拡張データ
    pub data: Vec<u8>,
}

/// HS 拡張データ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HsExtensionData {
    /// SRT バージョン
    pub srt_version: u32,
    /// SRT フラグ
    pub srt_flags: u32,
    /// 受信側 TSBPD 遅延 (ms)
    pub recv_tsbpd_delay: u16,
    /// 送信側 TSBPD 遅延 (ms)
    pub send_tsbpd_delay: u16,
}

/// Peer IP をパース
fn parse_peer_ip(bytes: &[u8]) -> IpAddr {
    if bytes.len() < 16 {
        return IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED);
    }

    // IPv4 の場合: 最初の 4 バイトが IP、残りは 0
    let is_ipv4 = bytes[4..16].iter().all(|&b| b == 0);

    if is_ipv4 {
        IpAddr::V4(std::net::Ipv4Addr::new(
            bytes[0], bytes[1], bytes[2], bytes[3],
        ))
    } else {
        let mut octets = [0u8; 16];
        octets.copy_from_slice(bytes);
        IpAddr::V6(std::net::Ipv6Addr::from(octets))
    }
}

/// Peer IP をエンコード
fn encode_peer_ip(ip: &IpAddr, buf: &mut Vec<u8>) {
    match ip {
        IpAddr::V4(ipv4) => {
            write_bytes(buf, &ipv4.octets());
            // 残り 12 バイトは 0
            for _ in 0..12 {
                write_u8(buf, 0);
            }
        }
        IpAddr::V6(ipv6) => {
            write_bytes(buf, &ipv6.octets());
        }
    }
}

/// ハンドシェイク状態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HandshakeState {
    /// 初期状態
    #[default]
    Initial,
    /// INDUCTION 送信済み (Caller)
    InductionSent,
    /// INDUCTION 受信済み (Listener)
    InductionReceived,
    /// CONCLUSION 送信済み
    ConclusionSent,
    /// 完了
    Completed,
    /// 失敗
    Failed,
}

/// Key Material メッセージ
///
/// SRT 仕様 §3.2.1 に基づく Key Material 構造体。
/// ハンドシェイク拡張 (KMREQ/KMRSP) で使用される。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KmMessage {
    /// KM バージョン (V): 3 bits, 現在は 1
    pub version: u8,
    /// パケットタイプ (PT): 4 bits, KMmsg = 2
    pub packet_type: u8,
    /// 鍵フラグ (KK): 2 bits
    pub key_flag: KeyFlag,
    /// KEK インデックス: 通常 0 (デフォルトストリームキー)
    pub keki: u32,
    /// 暗号化方式 (Cipher): AES-CTR = 2, AES-GCM = 4
    pub cipher: u8,
    /// 認証方式 (Auth): None = 0, AES-GCM = 1
    pub auth: u8,
    /// ストリームカプセル化 (SE): MPEG-TS/SRT = 2
    pub stream_encapsulation: u8,
    /// 鍵長
    pub key_length: KeyLength,
    /// Salt (16 bytes)
    pub salt: [u8; 16],
    /// ラップされた SEK
    pub wrapped_key: Vec<u8>,
}

/// KM メッセージ署名 ('HAI' = Haivision)
const KM_SIGNATURE: u16 = 0x2029;

/// KM バージョン
const KM_VERSION: u8 = 1;

/// パケットタイプ: Key Material Message
const KM_PACKET_TYPE: u8 = 2;

/// 暗号化方式
#[expect(dead_code)]
pub mod cipher_type {
    /// AES-CTR
    pub const AES_CTR: u8 = 2;
    /// AES-GCM (v1.6.0 以降)
    pub const AES_GCM: u8 = 4;
}

/// ストリームカプセル化
pub mod stream_encapsulation {
    /// MPEG-TS/SRT
    pub const MPEG_TS_SRT: u8 = 2;
}

impl KmMessage {
    /// 新しい KM メッセージを作成
    pub fn new(
        key_flag: KeyFlag,
        key_length: KeyLength,
        salt: [u8; 16],
        wrapped_key: Vec<u8>,
    ) -> Self {
        Self {
            version: KM_VERSION,
            packet_type: KM_PACKET_TYPE,
            key_flag,
            keki: 0,
            cipher: cipher_type::AES_CTR,
            auth: 0,
            stream_encapsulation: stream_encapsulation::MPEG_TS_SRT,
            key_length,
            salt,
            wrapped_key,
        }
    }

    /// バイト列にエンコード
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // 最初の 4 バイト: S(1) | V(3) | PT(4) | Sign(16) | Resv1(6) | KK(2)
        let first_byte = (self.version << 4) | self.packet_type;
        write_u8(&mut buf, first_byte);
        write_u16(&mut buf, KM_SIGNATURE);
        // Resv1 (6 bits) | KK (2 bits)
        write_u8(&mut buf, self.key_flag.to_kk_field());

        // KEKI (32 bits)
        write_u32(&mut buf, self.keki);

        // Cipher (8) | Auth (8) | SE (8) | Resv2 (8)
        write_u8(&mut buf, self.cipher);
        write_u8(&mut buf, self.auth);
        write_u8(&mut buf, self.stream_encapsulation);
        write_u8(&mut buf, 0); // Resv2

        // Resv3 (16) | SLen/4 (8) | KLen/4 (8)
        write_u16(&mut buf, 0); // Resv3
        write_u8(&mut buf, 4); // SLen/4 = 16/4 = 4
        write_u8(&mut buf, (self.key_length.len() / 4) as u8); // KLen/4

        // Salt (16 bytes)
        write_bytes(&mut buf, &self.salt);

        // Wrapped Key
        write_bytes(&mut buf, &self.wrapped_key);

        buf
    }

    /// バイト列からデコード
    pub fn decode(data: &[u8]) -> Result<Self, Error> {
        if data.len() < 16 {
            return Err(Error::invalid_data("KM message too short"));
        }

        let mut buf = data;

        // 最初の 4 バイト
        let first_byte = read_u8(&mut buf)?;
        let version = (first_byte >> 4) & 0x07;
        let packet_type = first_byte & 0x0F;

        let signature = read_u16(&mut buf)?;
        if signature != KM_SIGNATURE {
            return Err(Error::invalid_data(format!(
                "invalid KM signature: {signature:#06x}, expected {KM_SIGNATURE:#06x}"
            )));
        }

        let kk_byte = read_u8(&mut buf)?;
        let key_flag = KeyFlag::from_kk_field(kk_byte)
            .ok_or_else(|| Error::invalid_data("invalid KK field"))?;

        // KEKI
        let keki = read_u32(&mut buf)?;

        // Cipher, Auth, SE, Resv2
        let cipher = read_u8(&mut buf)?;
        let auth = read_u8(&mut buf)?;
        let stream_encapsulation = read_u8(&mut buf)?;
        let _resv2 = read_u8(&mut buf)?;

        // Resv3, SLen/4, KLen/4
        let _resv3 = read_u16(&mut buf)?;
        let slen_div4 = read_u8(&mut buf)? as usize;
        let klen_div4 = read_u8(&mut buf)? as usize;

        let slen = slen_div4 * 4;
        let klen = klen_div4 * 4;

        if slen != 16 {
            return Err(Error::invalid_data(format!(
                "unsupported salt length: {slen}"
            )));
        }

        let key_length = KeyLength::from_len(klen)
            .ok_or_else(|| Error::invalid_data(format!("invalid key length: {klen}")))?;

        // Salt
        if buf.len() < slen {
            return Err(Error::invalid_data("KM message too short for salt"));
        }
        let salt_bytes = read_bytes(&mut buf, slen)?;
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&salt_bytes);

        // Wrapped Key (残り全て)
        let wrapped_key = buf.to_vec();

        Ok(Self {
            version,
            packet_type,
            key_flag,
            keki,
            cipher,
            auth,
            stream_encapsulation,
            key_length,
            salt,
            wrapped_key,
        })
    }
}

/// KM レスポンスエラー
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum KmError {
    /// 暗号化されていない (ピアは暗号化するが、エージェントは暗号化を宣言していない)
    Unsecured = 0,
    /// シークレットがない (ピアは復号化する鍵を持っていない)
    NoSecret = 3,
    /// シークレットが間違っている (ピアは間違った鍵を持っている)
    BadSecret = 4,
    /// 暗号化モードが違う (ピアは異なる暗号化モードを期待している)
    BadCryptoMode = 5,
}

impl KmError {
    /// u32 から変換
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Unsecured),
            3 => Some(Self::NoSecret),
            4 => Some(Self::BadSecret),
            5 => Some(Self::BadCryptoMode),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_encode_decode() {
        let original = HandshakePacket {
            version: HS_VERSION_5,
            encryption_field: 2,
            extension_field: extension_flags::HSREQ,
            initial_packet_seq: 12345,
            mtu: 1500,
            flow_window: 8192,
            handshake_type: HandshakeType::Conclusion,
            socket_id: 0x12345678,
            syn_cookie: 0xABCDEF01,
            peer_ip: IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 1)),
            extensions: Vec::new(),
        };

        let packet = original.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        assert_eq!(original.version, decoded.version);
        assert_eq!(original.encryption_field, decoded.encryption_field);
        assert_eq!(original.extension_field, decoded.extension_field);
        assert_eq!(original.initial_packet_seq, decoded.initial_packet_seq);
        assert_eq!(original.mtu, decoded.mtu);
        assert_eq!(original.flow_window, decoded.flow_window);
        assert_eq!(original.handshake_type, decoded.handshake_type);
        assert_eq!(original.socket_id, decoded.socket_id);
        assert_eq!(original.syn_cookie, decoded.syn_cookie);
    }

    #[test]
    fn test_hs_extension() {
        let mut hs = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        hs.add_hs_extension(0x010500, srt_flags::TSBPDSND | srt_flags::TSBPDRCV, 120);

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        let ext = decoded
            .get_hs_extension()
            .expect("HS 拡張は Some になる想定");
        assert_eq!(ext.srt_version, 0x010500);
        assert_eq!(ext.srt_flags, srt_flags::TSBPDSND | srt_flags::TSBPDRCV);
        assert_eq!(ext.recv_tsbpd_delay, 120);
    }

    #[test]
    fn test_km_message_encode_decode() {
        let salt = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ];
        let wrapped_key = vec![
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
            0x99, 0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22,
        ];

        let original = KmMessage::new(KeyFlag::Even, KeyLength::Aes128, salt, wrapped_key.clone());

        let encoded = original.encode();
        let decoded = KmMessage::decode(&encoded)
            .expect("エンコード済み KM メッセージのデコードは成功する想定");

        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.packet_type, 2);
        assert_eq!(decoded.key_flag, KeyFlag::Even);
        assert_eq!(decoded.cipher, cipher_type::AES_CTR);
        assert_eq!(decoded.key_length, KeyLength::Aes128);
        assert_eq!(decoded.salt, salt);
        assert_eq!(decoded.wrapped_key, wrapped_key);
    }

    #[test]
    fn test_km_extension_in_handshake() {
        let salt = [0u8; 16];
        let wrapped_key = vec![0u8; 24]; // AES-128 wrapped = 16 + 8

        let km_message = KmMessage::new(KeyFlag::Even, KeyLength::Aes128, salt, wrapped_key);

        let mut hs = HandshakePacket::new_conclusion_request(1, 2, 3, 2, true);
        hs.add_hs_extension(0x010500, srt_flags::TSBPDSND | srt_flags::CRYPT, 120);
        hs.add_km_request(&km_message);

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        // KM リクエストを取得
        let km_result = decoded.get_km_request();
        assert!(km_result.is_some());
        let km = km_result
            .expect("KM リクエストは Some になる想定")
            .expect("KM メッセージのデコードは成功する想定");
        assert_eq!(km.key_flag, KeyFlag::Even);
        assert_eq!(km.key_length, KeyLength::Aes128);
    }

    #[test]
    fn test_km_error_response() {
        let mut hs = HandshakePacket::new_conclusion_response(1, 2, 3, 0, true);
        hs.add_km_error(KmError::BadSecret);

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        let result = decoded.get_km_response();
        assert!(matches!(result, Err(KmError::BadSecret)));
    }

    #[test]
    fn test_sid_extension_basic() {
        let mut hs = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        hs.add_sid_extension("test_stream");

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        let sid = decoded.get_sid_extension();
        assert_eq!(sid, Some("test_stream".to_string()));
    }

    #[test]
    fn test_sid_extension_access_control() {
        let mut hs = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        hs.add_sid_extension("#!::u=admin,r=live/stream1");

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        let sid = decoded.get_sid_extension();
        assert_eq!(sid, Some("#!::u=admin,r=live/stream1".to_string()));
    }

    #[test]
    fn test_sid_extension_with_padding() {
        // 5 文字 → 8 バイトにパディング
        let mut hs = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        hs.add_sid_extension("hello");

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        let sid = decoded.get_sid_extension();
        assert_eq!(sid, Some("hello".to_string()));
    }

    #[test]
    fn test_sid_extension_exact_4_bytes() {
        // 4 文字 → パディング不要
        let mut hs = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        hs.add_sid_extension("test");

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        let sid = decoded.get_sid_extension();
        assert_eq!(sid, Some("test".to_string()));
    }

    #[test]
    fn test_sid_extension_long_string() {
        // 長い文字列
        let long_sid = "a".repeat(100);
        let mut hs = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        hs.add_sid_extension(&long_sid);

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        let sid = decoded.get_sid_extension();
        assert_eq!(sid, Some(long_sid));
    }

    #[test]
    fn test_sid_extension_empty() {
        let mut hs = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        hs.add_sid_extension("");

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        // 空文字列の場合は None になる (すべてゼロパディング)
        let sid = decoded.get_sid_extension();
        assert_eq!(sid, Some("".to_string()));
    }

    #[test]
    fn test_no_sid_extension() {
        let hs = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        let sid = decoded.get_sid_extension();
        assert!(sid.is_none());
    }

    #[test]
    fn test_congestion_extension_live() {
        let mut hs = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        hs.add_congestion_extension("live");

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        let cc = decoded.get_congestion_extension();
        assert_eq!(cc, Some("live".to_string()));
    }

    #[test]
    fn test_congestion_extension_file() {
        // FileCC はサポートしないが、デコードはできる
        let mut hs = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        hs.add_congestion_extension("file");

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        let cc = decoded.get_congestion_extension();
        assert_eq!(cc, Some("file".to_string()));
    }

    #[test]
    fn test_no_congestion_extension() {
        let hs = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        let cc = decoded.get_congestion_extension();
        assert!(cc.is_none());
    }

    #[test]
    fn test_congestion_extension_with_sid() {
        // Congestion 拡張と SID 拡張を同時に使用
        let mut hs = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        hs.add_congestion_extension("live");
        hs.add_sid_extension("test_stream");

        let packet = hs.encode(1000, 0);
        let decoded = HandshakePacket::decode(&packet)
            .expect("エンコード済みハンドシェイクパケットのデコードは成功する想定");

        let cc = decoded.get_congestion_extension();
        assert_eq!(cc, Some("live".to_string()));

        let sid = decoded.get_sid_extension();
        assert_eq!(sid, Some("test_stream".to_string()));
    }
}
