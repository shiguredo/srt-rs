//! SRT パケット共通構造
//!
//! SRT パケットは UDP ペイロードとして送信される。
//! F ビット (最上位ビット) でデータパケット (0) と制御パケット (1) を区別する。

use crate::buf::{read_u32, write_bytes, write_u32};
use crate::error::Error;

/// SRT パケットの最小ヘッダサイズ (16 bytes)
pub const SRT_HEADER_SIZE: usize = 16;

/// パケットタイプ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    /// データパケット (F=0)
    Data,
    /// 制御パケット (F=1)
    Control,
}

impl PacketType {
    /// 最初の 32 ビットからパケットタイプを判定
    pub fn from_first_word(word: u32) -> Self {
        if word & 0x8000_0000 != 0 {
            PacketType::Control
        } else {
            PacketType::Data
        }
    }
}

/// SRT パケット (データまたは制御)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SrtPacket {
    /// データパケット
    Data(DataPacket),
    /// 制御パケット
    Control(ControlPacket),
}

impl SrtPacket {
    /// バイト列からデコード
    #[track_caller]
    pub fn decode(buf: &[u8]) -> Result<Self, Error> {
        Error::check_buffer_size(SRT_HEADER_SIZE, buf)?;

        let mut slice = buf;
        let first_word = read_u32(&mut slice)?;

        if PacketType::from_first_word(first_word) == PacketType::Data {
            // データパケットとしてデコード
            DataPacket::decode_with_first_word(first_word, buf).map(SrtPacket::Data)
        } else {
            // 制御パケットとしてデコード
            ControlPacket::decode_with_first_word(first_word, buf).map(SrtPacket::Control)
        }
    }

    /// バイト列にエンコード
    pub fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            SrtPacket::Data(pkt) => pkt.encode(buf),
            SrtPacket::Control(pkt) => pkt.encode(buf),
        }
    }
}

/// パケット位置フラグ (PP)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PacketPosition {
    /// メッセージの最初のパケット (10b)
    First = 0b10,
    /// メッセージの中間パケット (00b)
    Middle = 0b00,
    /// メッセージの最後のパケット (01b)
    Last = 0b01,
    /// 単一パケットでメッセージ全体 (11b)
    #[default]
    Single = 0b11,
}

impl PacketPosition {
    /// PP フィールド値から取得
    pub fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0b10 => Self::First,
            0b00 => Self::Middle,
            0b01 => Self::Last,
            0b11 => Self::Single,
            _ => unreachable!(),
        }
    }

    /// PP フィールド値へ変換
    pub fn to_bits(self) -> u8 {
        self as u8
    }
}

/// データパケット
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPacket {
    /// パケットシーケンス番号 (31 bits)
    pub sequence_number: u32,
    /// パケット位置フラグ (PP, 2 bits)
    pub position: PacketPosition,
    /// 順序フラグ (O, 1 bit)
    pub order_flag: bool,
    /// 暗号化キーフラグ (KK, 2 bits)
    /// 00b: 暗号化なし, 01b: 偶数キー, 10b: 奇数キー
    pub encryption_flag: u8,
    /// 再送信フラグ (R, 1 bit)
    pub retransmitted: bool,
    /// メッセージ番号 (26 bits)
    pub message_number: u32,
    /// タイムスタンプ (マイクロ秒)
    pub timestamp: u32,
    /// 宛先ソケット ID
    pub dest_socket_id: u32,
    /// ペイロード
    pub payload: Vec<u8>,
}

impl DataPacket {
    /// 新しいデータパケットを作成
    pub fn new(
        sequence_number: u32,
        message_number: u32,
        timestamp: u32,
        dest_socket_id: u32,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            sequence_number: sequence_number & 0x7FFF_FFFF,
            position: PacketPosition::Single,
            order_flag: false,
            encryption_flag: 0,
            retransmitted: false,
            message_number: message_number & 0x03FF_FFFF,
            timestamp,
            dest_socket_id,
            payload,
        }
    }

    /// バイト列からデコード (最初の 32 ビットは既に読み込み済み)
    #[track_caller]
    fn decode_with_first_word(first_word: u32, buf: &[u8]) -> Result<Self, Error> {
        Error::check_buffer_size(SRT_HEADER_SIZE, buf)?;

        let mut slice = &buf[4..]; // 最初の 4 バイトはスキップ

        let sequence_number = first_word & 0x7FFF_FFFF;

        let second_word = read_u32(&mut slice)?;
        let position = PacketPosition::from_bits(((second_word >> 30) & 0b11) as u8);
        let order_flag = (second_word >> 29) & 1 != 0;
        let encryption_flag = ((second_word >> 27) & 0b11) as u8;
        let retransmitted = (second_word >> 26) & 1 != 0;
        let message_number = second_word & 0x03FF_FFFF;

        let timestamp = read_u32(&mut slice)?;
        let dest_socket_id = read_u32(&mut slice)?;

        let payload = slice.to_vec();

        Ok(Self {
            sequence_number,
            position,
            order_flag,
            encryption_flag,
            retransmitted,
            message_number,
            timestamp,
            dest_socket_id,
            payload,
        })
    }

    /// バイト列にエンコード
    pub fn encode(&self, buf: &mut Vec<u8>) {
        // First word: F=0, sequence_number
        let first_word = self.sequence_number & 0x7FFF_FFFF;
        write_u32(buf, first_word);

        // Second word: PP, O, KK, R, message_number
        let second_word = ((self.position.to_bits() as u32) << 30)
            | ((self.order_flag as u32) << 29)
            | ((self.encryption_flag as u32 & 0b11) << 27)
            | ((self.retransmitted as u32) << 26)
            | (self.message_number & 0x03FF_FFFF);
        write_u32(buf, second_word);

        write_u32(buf, self.timestamp);
        write_u32(buf, self.dest_socket_id);
        write_bytes(buf, &self.payload);
    }

    /// エンコード後のサイズを取得
    pub fn encoded_size(&self) -> usize {
        SRT_HEADER_SIZE + self.payload.len()
    }
}

/// 制御パケットタイプ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ControlType {
    /// ハンドシェイク
    Handshake = 0x0000,
    /// キープアライブ
    Keepalive = 0x0001,
    /// ACK (確認応答)
    Ack = 0x0002,
    /// NAK (損失報告)
    Nak = 0x0003,
    /// 輻輳警告
    CongestionWarning = 0x0004,
    /// シャットダウン
    Shutdown = 0x0005,
    /// ACKACK
    AckAck = 0x0006,
    /// ドロップ要求
    DropReq = 0x0007,
    /// ピアエラー
    PeerError = 0x0008,
    /// ユーザー定義
    UserDefined = 0x7FFF,
}

impl ControlType {
    /// 値から ControlType を取得
    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0000 => Some(Self::Handshake),
            0x0001 => Some(Self::Keepalive),
            0x0002 => Some(Self::Ack),
            0x0003 => Some(Self::Nak),
            0x0004 => Some(Self::CongestionWarning),
            0x0005 => Some(Self::Shutdown),
            0x0006 => Some(Self::AckAck),
            0x0007 => Some(Self::DropReq),
            0x0008 => Some(Self::PeerError),
            0x7FFF => Some(Self::UserDefined),
            _ => None,
        }
    }
}

/// 制御パケット
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlPacket {
    /// 制御タイプ (15 bits)
    pub control_type: ControlType,
    /// サブタイプ (16 bits)
    pub subtype: u16,
    /// タイプ固有情報 (32 bits)
    pub type_specific_info: u32,
    /// タイムスタンプ (マイクロ秒)
    pub timestamp: u32,
    /// 宛先ソケット ID
    pub dest_socket_id: u32,
    /// 制御情報フィールド (CIF)
    pub control_info: Vec<u8>,
}

impl ControlPacket {
    /// 新しい制御パケットを作成
    pub fn new(control_type: ControlType, timestamp: u32, dest_socket_id: u32) -> Self {
        Self {
            control_type,
            subtype: 0,
            type_specific_info: 0,
            timestamp,
            dest_socket_id,
            control_info: Vec::new(),
        }
    }

    /// バイト列からデコード (最初の 32 ビットは既に読み込み済み)
    #[track_caller]
    fn decode_with_first_word(first_word: u32, buf: &[u8]) -> Result<Self, Error> {
        Error::check_buffer_size(SRT_HEADER_SIZE, buf)?;

        let mut slice = &buf[4..]; // 最初の 4 バイトはスキップ

        let control_type_raw = ((first_word >> 16) & 0x7FFF) as u16;
        let control_type = ControlType::from_u16(control_type_raw).ok_or_else(|| {
            Error::invalid_data(format!("unknown control type: {control_type_raw:#x}"))
        })?;
        let subtype = (first_word & 0xFFFF) as u16;

        let type_specific_info = read_u32(&mut slice)?;
        let timestamp = read_u32(&mut slice)?;
        let dest_socket_id = read_u32(&mut slice)?;

        let control_info = slice.to_vec();

        Ok(Self {
            control_type,
            subtype,
            type_specific_info,
            timestamp,
            dest_socket_id,
            control_info,
        })
    }

    /// バイト列にエンコード
    pub fn encode(&self, buf: &mut Vec<u8>) {
        // First word: F=1, control_type, subtype
        let first_word = 0x8000_0000
            | ((self.control_type as u32 & 0x7FFF) << 16)
            | (self.subtype as u32 & 0xFFFF);
        write_u32(buf, first_word);

        write_u32(buf, self.type_specific_info);
        write_u32(buf, self.timestamp);
        write_u32(buf, self.dest_socket_id);
        write_bytes(buf, &self.control_info);
    }

    /// エンコード後のサイズを取得
    pub fn encoded_size(&self) -> usize {
        SRT_HEADER_SIZE + self.control_info.len()
    }
}

/// シーケンス番号の比較 (ラップアラウンド対応, 31-bit)
pub(crate) fn sequence_less_than(a: u32, b: u32) -> bool {
    let diff = b.wrapping_sub(a) & 0x7FFF_FFFF;
    diff > 0 && diff < 0x4000_0000
}

/// シーケンス番号の比較 (ラップアラウンド対応, 31-bit)
pub(crate) fn sequence_greater_than(a: u32, b: u32) -> bool {
    sequence_less_than(b, a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_packet_encode_decode() {
        let original = DataPacket {
            sequence_number: 12345,
            position: PacketPosition::Single,
            order_flag: true,
            encryption_flag: 0b10,
            retransmitted: false,
            message_number: 100,
            timestamp: 1000000,
            dest_socket_id: 0x12345678,
            payload: b"Hello, SRT!".to_vec(),
        };

        let mut buf = Vec::new();
        original.encode(&mut buf);

        let decoded = match SrtPacket::decode(&buf)
            .expect("エンコード済みパケットのデコードは成功する想定")
        {
            SrtPacket::Data(pkt) => pkt,
            _ => panic!("expected data packet"),
        };

        assert_eq!(original, decoded);
    }

    #[test]
    fn test_control_packet_encode_decode() {
        let original = ControlPacket {
            control_type: ControlType::Ack,
            subtype: 0,
            type_specific_info: 42,
            timestamp: 2000000,
            dest_socket_id: 0xABCDEF01,
            control_info: vec![1, 2, 3, 4],
        };

        let mut buf = Vec::new();
        original.encode(&mut buf);

        let decoded = match SrtPacket::decode(&buf)
            .expect("エンコード済みパケットのデコードは成功する想定")
        {
            SrtPacket::Control(pkt) => pkt,
            _ => panic!("expected control packet"),
        };

        assert_eq!(original, decoded);
    }

    #[test]
    fn test_packet_position() {
        assert_eq!(PacketPosition::from_bits(0b10), PacketPosition::First);
        assert_eq!(PacketPosition::from_bits(0b00), PacketPosition::Middle);
        assert_eq!(PacketPosition::from_bits(0b01), PacketPosition::Last);
        assert_eq!(PacketPosition::from_bits(0b11), PacketPosition::Single);
    }
}
