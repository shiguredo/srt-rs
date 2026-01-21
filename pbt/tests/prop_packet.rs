//! Property-based tests for SRT packets

use proptest::prelude::*;
use shiguredo_srt::{
    ControlPacket, ControlType, DataPacket, PacketPosition, SRT_HEADER_SIZE, SrtPacket,
};

/// DataPacket のラウンドトリップテスト
fn arb_data_packet() -> impl Strategy<Value = DataPacket> {
    (
        any::<u32>().prop_map(|n| n & 0x7FFF_FFFF), // sequence_number (31 bits)
        prop::sample::select(vec![
            PacketPosition::Single,
            PacketPosition::First,
            PacketPosition::Middle,
            PacketPosition::Last,
        ]),
        any::<bool>(),                               // order_flag
        any::<u8>().prop_map(|n| n & 0b11),          // encryption_flag (0-3)
        any::<bool>(),                               // retransmitted
        any::<u32>().prop_map(|n| n & 0x03FF_FFFF),  // message_number (26 bits)
        any::<u32>(),                                // timestamp
        any::<u32>(),                                // dest_socket_id
        prop::collection::vec(any::<u8>(), 0..1400), // payload
    )
        .prop_map(
            |(
                sequence_number,
                position,
                order_flag,
                encryption_flag,
                retransmitted,
                message_number,
                timestamp,
                dest_socket_id,
                payload,
            )| {
                DataPacket {
                    sequence_number,
                    position,
                    order_flag,
                    encryption_flag,
                    retransmitted,
                    message_number,
                    timestamp,
                    dest_socket_id,
                    payload,
                }
            },
        )
}

/// ControlPacket のラウンドトリップテスト
fn arb_control_packet() -> impl Strategy<Value = ControlPacket> {
    (
        prop::sample::select(vec![
            ControlType::Keepalive,
            ControlType::AckAck,
            ControlType::Shutdown,
            ControlType::DropReq,
            ControlType::PeerError,
        ]),
        any::<u16>(),                                // subtype
        any::<u32>(),                                // type_specific_info
        any::<u32>(),                                // timestamp
        any::<u32>(),                                // dest_socket_id
        prop::collection::vec(any::<u8>(), 0..1000), // control_info
    )
        .prop_map(
            |(
                control_type,
                subtype,
                type_specific_info,
                timestamp,
                dest_socket_id,
                control_info,
            )| {
                ControlPacket {
                    control_type,
                    subtype,
                    type_specific_info,
                    timestamp,
                    dest_socket_id,
                    control_info,
                }
            },
        )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn test_data_packet_roundtrip(packet in arb_data_packet()) {
        let mut buf = Vec::new();
        packet.encode(&mut buf);

        let decoded = SrtPacket::decode(&buf).expect("decode should succeed");

        match decoded {
            SrtPacket::Data(decoded_packet) => {
                prop_assert_eq!(packet.sequence_number, decoded_packet.sequence_number);
                prop_assert_eq!(packet.position, decoded_packet.position);
                prop_assert_eq!(packet.order_flag, decoded_packet.order_flag);
                prop_assert_eq!(packet.encryption_flag, decoded_packet.encryption_flag);
                prop_assert_eq!(packet.retransmitted, decoded_packet.retransmitted);
                prop_assert_eq!(packet.message_number, decoded_packet.message_number);
                prop_assert_eq!(packet.timestamp, decoded_packet.timestamp);
                prop_assert_eq!(packet.dest_socket_id, decoded_packet.dest_socket_id);
                prop_assert_eq!(packet.payload, decoded_packet.payload);
            }
            _ => prop_assert!(false, "expected DataPacket"),
        }
    }

    #[test]
    fn test_control_packet_roundtrip(packet in arb_control_packet()) {
        let mut buf = Vec::new();
        packet.encode(&mut buf);

        let decoded = SrtPacket::decode(&buf).expect("decode should succeed");

        match decoded {
            SrtPacket::Control(decoded_packet) => {
                prop_assert_eq!(packet.control_type, decoded_packet.control_type);
                prop_assert_eq!(packet.subtype, decoded_packet.subtype);
                prop_assert_eq!(packet.type_specific_info, decoded_packet.type_specific_info);
                prop_assert_eq!(packet.timestamp, decoded_packet.timestamp);
                prop_assert_eq!(packet.dest_socket_id, decoded_packet.dest_socket_id);
                prop_assert_eq!(packet.control_info, decoded_packet.control_info);
            }
            _ => prop_assert!(false, "expected ControlPacket"),
        }
    }

    #[test]
    fn test_packet_decode_invalid_data(data in prop::collection::vec(any::<u8>(), 0..16)) {
        // 16 バイト未満のデータはヘッダ不足でエラーになるはず
        let result = SrtPacket::decode(&data);
        if data.len() < 16 {
            prop_assert!(result.is_err());
        }
    }

    #[test]
    fn test_data_packet_new(
        seq in any::<u32>(),
        msg in any::<u32>(),
        ts in any::<u32>(),
        sock_id in any::<u32>(),
        payload_len in 0usize..1000usize,
    ) {
        let payload = vec![0u8; payload_len];
        let packet = DataPacket::new(seq, msg, ts, sock_id, payload.clone());

        // sequence_number は 31 ビットにマスクされる
        prop_assert_eq!(packet.sequence_number, seq & 0x7FFF_FFFF);
        // message_number は 26 ビットにマスクされる
        prop_assert_eq!(packet.message_number, msg & 0x03FF_FFFF);
        prop_assert_eq!(packet.timestamp, ts);
        prop_assert_eq!(packet.dest_socket_id, sock_id);
        prop_assert_eq!(packet.payload, payload);
        prop_assert_eq!(packet.position, PacketPosition::Single);
        prop_assert!(!packet.order_flag);
        prop_assert_eq!(packet.encryption_flag, 0);
        prop_assert!(!packet.retransmitted);
    }

    #[test]
    fn test_data_packet_encoded_size(packet in arb_data_packet()) {
        let size = packet.encoded_size();
        prop_assert_eq!(size, SRT_HEADER_SIZE + packet.payload.len());

        // 実際にエンコードして確認
        let mut buf = Vec::new();
        packet.encode(&mut buf);
        prop_assert_eq!(buf.len(), size);
    }

    #[test]
    fn test_control_packet_new(
        ts in any::<u32>(),
        sock_id in any::<u32>(),
    ) {
        let packet = ControlPacket::new(ControlType::Keepalive, ts, sock_id);

        prop_assert_eq!(packet.control_type, ControlType::Keepalive);
        prop_assert_eq!(packet.subtype, 0);
        prop_assert_eq!(packet.type_specific_info, 0);
        prop_assert_eq!(packet.timestamp, ts);
        prop_assert_eq!(packet.dest_socket_id, sock_id);
        prop_assert!(packet.control_info.is_empty());
    }

    #[test]
    fn test_control_packet_encoded_size(packet in arb_control_packet()) {
        let size = packet.encoded_size();
        prop_assert_eq!(size, SRT_HEADER_SIZE + packet.control_info.len());

        // 実際にエンコードして確認
        let mut buf = Vec::new();
        packet.encode(&mut buf);
        prop_assert_eq!(buf.len(), size);
    }

    #[test]
    fn test_srt_packet_encode_data(packet in arb_data_packet()) {
        let srt_packet = SrtPacket::Data(packet.clone());
        let mut buf = Vec::new();
        srt_packet.encode(&mut buf);

        // 直接エンコードと同じ結果
        let mut direct_buf = Vec::new();
        packet.encode(&mut direct_buf);
        prop_assert_eq!(buf, direct_buf);
    }

    #[test]
    fn test_srt_packet_encode_control(packet in arb_control_packet()) {
        let srt_packet = SrtPacket::Control(packet.clone());
        let mut buf = Vec::new();
        srt_packet.encode(&mut buf);

        // 直接エンコードと同じ結果
        let mut direct_buf = Vec::new();
        packet.encode(&mut direct_buf);
        prop_assert_eq!(buf, direct_buf);
    }

    #[test]
    fn test_packet_position_roundtrip(
        bits in 0u8..4u8,
    ) {
        let position = PacketPosition::from_bits(bits);
        let encoded = position.to_bits();
        let decoded = PacketPosition::from_bits(encoded);
        prop_assert_eq!(position, decoded);
    }
}
