//! Property-based tests for SRT handshake

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use proptest::prelude::*;
use shiguredo_srt::{
    ControlPacket, ControlType, ExtensionType, HandshakeExtension, HandshakePacket, HandshakeType,
    KeyFlag, KeyLength, KmError, KmMessage,
};

/// HandshakePacket の生成 (IPv4)
fn arb_handshake_packet_ipv4() -> impl Strategy<Value = HandshakePacket> {
    (
        any::<u32>(),                               // version
        any::<u16>(),                               // encryption_field
        any::<u16>(),                               // extension_field
        any::<u32>().prop_map(|n| n & 0x7FFF_FFFF), // initial_packet_seq (31 bits)
        any::<u32>().prop_map(|n| n.max(100)),      // mtu (min 100)
        any::<u32>().prop_map(|n| n.max(32)),       // flow_window (min 32)
        prop::sample::select(vec![
            HandshakeType::Done,
            HandshakeType::Induction,
            HandshakeType::Conclusion,
            HandshakeType::Agreement,
            HandshakeType::Waveahand,
        ]),
        any::<u32>(),     // socket_id
        any::<u32>(),     // syn_cookie
        any::<[u8; 4]>(), // IPv4 address bytes
    )
        .prop_map(
            |(
                version,
                encryption_field,
                extension_field,
                initial_packet_seq,
                mtu,
                flow_window,
                handshake_type,
                socket_id,
                syn_cookie,
                ip_bytes,
            )| {
                HandshakePacket {
                    version,
                    encryption_field,
                    extension_field,
                    initial_packet_seq,
                    mtu,
                    flow_window,
                    handshake_type,
                    socket_id,
                    syn_cookie,
                    peer_ip: IpAddr::V4(Ipv4Addr::new(
                        ip_bytes[0],
                        ip_bytes[1],
                        ip_bytes[2],
                        ip_bytes[3],
                    )),
                    extensions: Vec::new(),
                }
            },
        )
}

/// HandshakePacket の生成 (IPv6)
fn arb_handshake_packet_ipv6() -> impl Strategy<Value = HandshakePacket> {
    (
        any::<u32>(),                               // version
        any::<u16>(),                               // encryption_field
        any::<u16>(),                               // extension_field
        any::<u32>().prop_map(|n| n & 0x7FFF_FFFF), // initial_packet_seq (31 bits)
        any::<u32>().prop_map(|n| n.max(100)),      // mtu (min 100)
        any::<u32>().prop_map(|n| n.max(32)),       // flow_window (min 32)
        prop::sample::select(vec![
            HandshakeType::Done,
            HandshakeType::Induction,
            HandshakeType::Conclusion,
            HandshakeType::Agreement,
            HandshakeType::Waveahand,
        ]),
        any::<u32>(),      // socket_id
        any::<u32>(),      // syn_cookie
        any::<[u8; 16]>(), // IPv6 address bytes
    )
        .prop_map(
            |(
                version,
                encryption_field,
                extension_field,
                initial_packet_seq,
                mtu,
                flow_window,
                handshake_type,
                socket_id,
                syn_cookie,
                ip_bytes,
            )| {
                HandshakePacket {
                    version,
                    encryption_field,
                    extension_field,
                    initial_packet_seq,
                    mtu,
                    flow_window,
                    handshake_type,
                    socket_id,
                    syn_cookie,
                    peer_ip: IpAddr::V6(Ipv6Addr::from(ip_bytes)),
                    extensions: Vec::new(),
                }
            },
        )
}

/// KmMessage の生成
fn arb_km_message() -> impl Strategy<Value = KmMessage> {
    (
        prop::sample::select(vec![KeyFlag::Even, KeyFlag::Odd]),
        prop::sample::select(vec![KeyLength::Aes128, KeyLength::Aes256]),
        any::<[u8; 16]>(), // salt
                           // wrapped_key サイズ: key_length + 8 bytes
    )
        .prop_flat_map(|(key_flag, key_length, salt)| {
            let wrapped_key_len = key_length.len() + 8;
            prop::collection::vec(any::<u8>(), wrapped_key_len..=wrapped_key_len).prop_map(
                move |wrapped_key| KmMessage::new(key_flag, key_length, salt, wrapped_key),
            )
        })
}

/// Stream ID 文字列の生成
fn arb_stream_id() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9_#!:=/,.-]{0,100}")
        .expect("ストリーム ID 用の正規表現は有効な想定")
}

/// Congestion control 名の生成
fn arb_congestion_name() -> impl Strategy<Value = String> {
    prop::sample::select(vec!["live".to_string(), "file".to_string()])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // ===== HandshakePacket roundtrip (IPv4) =====
    #[test]
    fn test_handshake_packet_roundtrip_ipv4(packet in arb_handshake_packet_ipv4()) {
        let control_packet = packet.encode(1000, 0);
        let decoded = HandshakePacket::decode(&control_packet).expect("decode should succeed");

        prop_assert_eq!(packet.version, decoded.version);
        prop_assert_eq!(packet.encryption_field, decoded.encryption_field);
        prop_assert_eq!(packet.extension_field, decoded.extension_field);
        prop_assert_eq!(packet.initial_packet_seq, decoded.initial_packet_seq);
        prop_assert_eq!(packet.mtu, decoded.mtu);
        prop_assert_eq!(packet.flow_window, decoded.flow_window);
        prop_assert_eq!(packet.handshake_type, decoded.handshake_type);
        prop_assert_eq!(packet.socket_id, decoded.socket_id);
        prop_assert_eq!(packet.syn_cookie, decoded.syn_cookie);
        prop_assert_eq!(packet.peer_ip, decoded.peer_ip);
    }

    // ===== HandshakePacket roundtrip (IPv6) =====
    #[test]
    fn test_handshake_packet_roundtrip_ipv6(packet in arb_handshake_packet_ipv6()) {
        let control_packet = packet.encode(1000, 0);
        let decoded = HandshakePacket::decode(&control_packet).expect("decode should succeed");

        prop_assert_eq!(packet.version, decoded.version);
        prop_assert_eq!(packet.encryption_field, decoded.encryption_field);
        prop_assert_eq!(packet.extension_field, decoded.extension_field);
        prop_assert_eq!(packet.initial_packet_seq, decoded.initial_packet_seq);
        prop_assert_eq!(packet.mtu, decoded.mtu);
        prop_assert_eq!(packet.flow_window, decoded.flow_window);
        prop_assert_eq!(packet.handshake_type, decoded.handshake_type);
        prop_assert_eq!(packet.socket_id, decoded.socket_id);
        prop_assert_eq!(packet.syn_cookie, decoded.syn_cookie);
        prop_assert_eq!(packet.peer_ip, decoded.peer_ip);
    }

    // ===== Decode non-handshake packet =====
    #[test]
    fn test_handshake_decode_non_handshake_packet(
        subtype in any::<u16>(),
        type_specific_info in any::<u32>(),
        timestamp in any::<u32>(),
        dest_socket_id in any::<u32>(),
    ) {
        let packet = ControlPacket {
            control_type: ControlType::Keepalive,
            subtype,
            type_specific_info,
            timestamp,
            dest_socket_id,
            control_info: Vec::new(),
        };

        let result = HandshakePacket::decode(&packet);
        prop_assert!(result.is_err());
    }

    // ===== HandshakeType::from_u32 =====
    #[test]
    fn test_handshake_type_from_u32_valid(
        hs_type in prop::sample::select(vec![
            (0xFFFFFFFDu32, HandshakeType::Done),
            (0xFFFFFFFE, HandshakeType::Agreement),
            (0xFFFFFFFF, HandshakeType::Conclusion),
            (0x00000000, HandshakeType::Waveahand),
            (0x00000001, HandshakeType::Induction),
        ])
    ) {
        let (value, expected) = hs_type;
        prop_assert_eq!(HandshakeType::from_u32(value), Some(expected));
    }

    #[test]
    fn test_handshake_type_from_u32_invalid(
        value in any::<u32>().prop_filter("not a valid handshake type", |v| {
            !matches!(*v, 0xFFFFFFFD | 0xFFFFFFFE | 0xFFFFFFFF | 0x00000000 | 0x00000001)
        })
    ) {
        prop_assert_eq!(HandshakeType::from_u32(value), None);
    }

    // ===== ExtensionType::from_u16 =====
    #[test]
    fn test_extension_type_from_u16_valid(
        ext_type in prop::sample::select(vec![
            (1u16, ExtensionType::HsReq),
            (2, ExtensionType::HsRsp),
            (3, ExtensionType::KmReq),
            (4, ExtensionType::KmRsp),
            (5, ExtensionType::Sid),
            (6, ExtensionType::Congestion),
            (7, ExtensionType::Filter),
            (8, ExtensionType::Group),
        ])
    ) {
        let (value, expected) = ext_type;
        prop_assert_eq!(ExtensionType::from_u16(value), Some(expected));
    }

    #[test]
    fn test_extension_type_from_u16_invalid(
        value in any::<u16>().prop_filter("not a valid extension type", |v| {
            !matches!(*v, 1..=8)
        })
    ) {
        prop_assert_eq!(ExtensionType::from_u16(value), None);
    }

    // ===== new_induction_request =====
    #[test]
    fn test_new_induction_request(socket_id in any::<u32>()) {
        let packet = HandshakePacket::new_induction_request(socket_id);

        prop_assert_eq!(packet.version, 4);
        prop_assert_eq!(packet.encryption_field, 0);
        prop_assert_eq!(packet.extension_field, 2);
        prop_assert_eq!(packet.initial_packet_seq, 0);
        prop_assert_eq!(packet.handshake_type, HandshakeType::Induction);
        prop_assert_eq!(packet.socket_id, socket_id);
        prop_assert_eq!(packet.syn_cookie, 0);

        // エンコード/デコードのラウンドトリップ
        let ctrl = packet.encode(0, 0);
        let decoded = HandshakePacket::decode(&ctrl).expect("decode should succeed");
        prop_assert_eq!(decoded.socket_id, socket_id);
    }

    // ===== new_induction_response =====
    #[test]
    fn test_new_induction_response(
        socket_id in any::<u32>(),
        syn_cookie in any::<u32>(),
        encryption_field in any::<u16>(),
    ) {
        let packet = HandshakePacket::new_induction_response(socket_id, syn_cookie, encryption_field);

        prop_assert_eq!(packet.version, 5);
        prop_assert_eq!(packet.encryption_field, encryption_field);
        prop_assert_eq!(packet.extension_field, 0x4A17); // SRT_MAGIC_CODE
        prop_assert_eq!(packet.handshake_type, HandshakeType::Induction);
        prop_assert_eq!(packet.socket_id, socket_id);
        prop_assert_eq!(packet.syn_cookie, syn_cookie);

        // エンコード/デコードのラウンドトリップ
        let ctrl = packet.encode(0, 0);
        let decoded = HandshakePacket::decode(&ctrl).expect("decode should succeed");
        prop_assert_eq!(decoded.socket_id, socket_id);
        prop_assert_eq!(decoded.syn_cookie, syn_cookie);
    }

    // ===== new_conclusion_request =====
    #[test]
    fn test_new_conclusion_request(
        socket_id in any::<u32>(),
        syn_cookie in any::<u32>(),
        initial_packet_seq in any::<u32>(),
        encryption_field in any::<u16>(),
        has_encryption in any::<bool>(),
    ) {
        let packet = HandshakePacket::new_conclusion_request(
            socket_id,
            syn_cookie,
            initial_packet_seq,
            encryption_field,
            has_encryption,
        );

        prop_assert_eq!(packet.version, 5);
        prop_assert_eq!(packet.encryption_field, encryption_field);
        prop_assert_eq!(packet.handshake_type, HandshakeType::Conclusion);
        prop_assert_eq!(packet.socket_id, socket_id);
        prop_assert_eq!(packet.syn_cookie, syn_cookie);

        // extension_field のチェック
        if has_encryption {
            prop_assert_eq!(packet.extension_field, 0x0001 | 0x0002); // HSREQ | KMREQ
        } else {
            prop_assert_eq!(packet.extension_field, 0x0001); // HSREQ only
        }

        // ラウンドトリップ
        let ctrl = packet.encode(0, 0);
        let decoded = HandshakePacket::decode(&ctrl).expect("decode should succeed");
        prop_assert_eq!(decoded.socket_id, socket_id);
    }

    // ===== new_conclusion_response =====
    #[test]
    fn test_new_conclusion_response(
        socket_id in any::<u32>(),
        syn_cookie in any::<u32>(),
        initial_packet_seq in any::<u32>(),
        encryption_field in any::<u16>(),
        has_encryption in any::<bool>(),
    ) {
        let packet = HandshakePacket::new_conclusion_response(
            socket_id,
            syn_cookie,
            initial_packet_seq,
            encryption_field,
            has_encryption,
        );

        prop_assert_eq!(packet.version, 5);
        prop_assert_eq!(packet.encryption_field, encryption_field);
        prop_assert_eq!(packet.handshake_type, HandshakeType::Conclusion);
        prop_assert_eq!(packet.socket_id, socket_id);
        prop_assert_eq!(packet.syn_cookie, syn_cookie);

        // extension_field のチェック
        if has_encryption {
            prop_assert_eq!(packet.extension_field, 0x0001 | 0x0002);
        } else {
            prop_assert_eq!(packet.extension_field, 0x0001);
        }
    }

    // ===== HS Extension roundtrip =====
    #[test]
    fn test_hs_extension_roundtrip(
        srt_version in any::<u32>(),
        srt_flags in any::<u32>(),
        tsbpd_delay in any::<u16>(),
    ) {
        let mut packet = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        packet.add_hs_extension(srt_version, srt_flags, tsbpd_delay);

        let ctrl = packet.encode(1000, 0);
        let decoded = HandshakePacket::decode(&ctrl).expect("decode should succeed");

        let ext = decoded.get_hs_extension().expect("should have hs extension");
        prop_assert_eq!(ext.srt_version, srt_version);
        prop_assert_eq!(ext.srt_flags, srt_flags);
        prop_assert_eq!(ext.recv_tsbpd_delay, tsbpd_delay);
        prop_assert_eq!(ext.send_tsbpd_delay, tsbpd_delay);
    }

    // ===== HS Response Extension roundtrip =====
    #[test]
    fn test_hs_response_extension_roundtrip(
        srt_version in any::<u32>(),
        srt_flags in any::<u32>(),
        tsbpd_delay in any::<u16>(),
    ) {
        let mut packet = HandshakePacket::new_conclusion_response(1, 2, 3, 0, false);
        packet.add_hs_response(srt_version, srt_flags, tsbpd_delay);

        let ctrl = packet.encode(1000, 0);
        let decoded = HandshakePacket::decode(&ctrl).expect("decode should succeed");

        let ext = decoded.get_hs_extension().expect("should have hs extension");
        prop_assert_eq!(ext.srt_version, srt_version);
        prop_assert_eq!(ext.srt_flags, srt_flags);
        prop_assert_eq!(ext.recv_tsbpd_delay, tsbpd_delay);
    }

    // ===== KmMessage roundtrip =====
    #[test]
    fn test_km_message_roundtrip(km in arb_km_message()) {
        let encoded = km.encode();
        let decoded = KmMessage::decode(&encoded).expect("decode should succeed");

        prop_assert_eq!(km.key_flag, decoded.key_flag);
        prop_assert_eq!(km.key_length, decoded.key_length);
        prop_assert_eq!(km.salt, decoded.salt);
        prop_assert_eq!(km.wrapped_key, decoded.wrapped_key);
    }

    // ===== KM Request Extension roundtrip =====
    #[test]
    fn test_km_request_extension_roundtrip(km in arb_km_message()) {
        let mut packet = HandshakePacket::new_conclusion_request(1, 2, 3, 2, true);
        packet.add_km_request(&km);

        let ctrl = packet.encode(1000, 0);
        let decoded = HandshakePacket::decode(&ctrl).expect("decode should succeed");

        let km_result = decoded.get_km_request().expect("should have km request");
        let decoded_km = km_result.expect("km decode should succeed");

        prop_assert_eq!(km.key_flag, decoded_km.key_flag);
        prop_assert_eq!(km.key_length, decoded_km.key_length);
        prop_assert_eq!(km.salt, decoded_km.salt);
        prop_assert_eq!(km.wrapped_key, decoded_km.wrapped_key);
    }

    // ===== KM Response Extension roundtrip =====
    #[test]
    fn test_km_response_extension_roundtrip(km in arb_km_message()) {
        let mut packet = HandshakePacket::new_conclusion_response(1, 2, 3, 2, true);
        packet.add_km_response(&km);

        let ctrl = packet.encode(1000, 0);
        let decoded = HandshakePacket::decode(&ctrl).expect("decode should succeed");

        let km_result = decoded.get_km_response().expect("should have km response");
        let decoded_km = km_result.expect("should be Some(KmMessage)");

        prop_assert_eq!(km.key_flag, decoded_km.key_flag);
        prop_assert_eq!(km.key_length, decoded_km.key_length);
        prop_assert_eq!(km.salt, decoded_km.salt);
        prop_assert_eq!(km.wrapped_key, decoded_km.wrapped_key);
    }

    // ===== KM Error Extension =====
    #[test]
    fn test_km_error_extension(
        error in prop::sample::select(vec![
            KmError::Unsecured,
            KmError::NoSecret,
            KmError::BadSecret,
            KmError::BadCryptoMode,
        ])
    ) {
        let mut packet = HandshakePacket::new_conclusion_response(1, 2, 3, 0, true);
        packet.add_km_error(error);

        let ctrl = packet.encode(1000, 0);
        let decoded = HandshakePacket::decode(&ctrl).expect("decode should succeed");

        let result = decoded.get_km_response();
        prop_assert!(matches!(result, Err(e) if e == error));
    }

    // ===== KmError::from_u32 =====
    #[test]
    fn test_km_error_from_u32_valid(
        error_type in prop::sample::select(vec![
            (0u32, KmError::Unsecured),
            (3, KmError::NoSecret),
            (4, KmError::BadSecret),
            (5, KmError::BadCryptoMode),
        ])
    ) {
        let (value, expected) = error_type;
        prop_assert_eq!(KmError::from_u32(value), Some(expected));
    }

    #[test]
    fn test_km_error_from_u32_invalid(
        value in any::<u32>().prop_filter("not a valid KmError", |v| {
            !matches!(*v, 0 | 3 | 4 | 5)
        })
    ) {
        prop_assert_eq!(KmError::from_u32(value), None);
    }

    // ===== SID Extension roundtrip =====
    #[test]
    fn test_sid_extension_roundtrip(stream_id in arb_stream_id()) {
        let mut packet = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        packet.add_sid_extension(&stream_id);

        let ctrl = packet.encode(1000, 0);
        let decoded = HandshakePacket::decode(&ctrl).expect("decode should succeed");

        let decoded_sid = decoded.get_sid_extension().expect("should have sid");
        prop_assert_eq!(stream_id, decoded_sid);
    }

    // ===== Congestion Extension roundtrip =====
    #[test]
    fn test_congestion_extension_roundtrip(cc_name in arb_congestion_name()) {
        let mut packet = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        packet.add_congestion_extension(&cc_name);

        let ctrl = packet.encode(1000, 0);
        let decoded = HandshakePacket::decode(&ctrl).expect("decode should succeed");

        let decoded_cc = decoded.get_congestion_extension().expect("should have congestion");
        prop_assert_eq!(cc_name, decoded_cc);
    }

    // ===== Multiple Extensions =====
    #[test]
    fn test_multiple_extensions(
        srt_version in any::<u32>(),
        srt_flags in any::<u32>(),
        tsbpd_delay in any::<u16>(),
        stream_id in arb_stream_id(),
        cc_name in arb_congestion_name(),
    ) {
        let mut packet = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        packet.add_hs_extension(srt_version, srt_flags, tsbpd_delay);
        packet.add_sid_extension(&stream_id);
        packet.add_congestion_extension(&cc_name);

        let ctrl = packet.encode(1000, 0);
        let decoded = HandshakePacket::decode(&ctrl).expect("decode should succeed");

        // HS Extension
        let hs_ext = decoded.get_hs_extension().expect("should have hs extension");
        prop_assert_eq!(hs_ext.srt_version, srt_version);

        // SID Extension
        let sid = decoded.get_sid_extension().expect("should have sid");
        prop_assert_eq!(sid, stream_id);

        // Congestion Extension
        let cc = decoded.get_congestion_extension().expect("should have congestion");
        prop_assert_eq!(cc, cc_name);
    }

    // ===== key_length() =====
    #[test]
    fn test_key_length_method(
        enc_field in prop::sample::select(vec![
            (2u16, Some(KeyLength::Aes128)),
            (4u16, Some(KeyLength::Aes256)),
            (0u16, None),
            (1u16, None),
            (3u16, None),
            (5u16, None),
        ])
    ) {
        let (encryption_field, expected) = enc_field;
        let packet = HandshakePacket::new_induction_response(1, 2, encryption_field);
        prop_assert_eq!(packet.key_length(), expected);
    }

    // ===== Decode with invalid handshake type =====
    #[test]
    fn test_decode_invalid_handshake_type(
        version in any::<u32>(),
        encryption_field in any::<u16>(),
        extension_field in any::<u16>(),
        initial_packet_seq in any::<u32>(),
        mtu in any::<u32>(),
        flow_window in any::<u32>(),
        invalid_type in any::<u32>().prop_filter("not a valid hs type", |v| {
            !matches!(*v, 0xFFFFFFFD | 0xFFFFFFFE | 0xFFFFFFFF | 0x00000000 | 0x00000001)
        }),
        socket_id in any::<u32>(),
        syn_cookie in any::<u32>(),
    ) {
        // 手動で不正な handshake_type を持つパケットを作成
        let mut control_info = Vec::new();
        control_info.extend_from_slice(&version.to_be_bytes());
        control_info.extend_from_slice(&encryption_field.to_be_bytes());
        control_info.extend_from_slice(&extension_field.to_be_bytes());
        control_info.extend_from_slice(&(initial_packet_seq & 0x7FFF_FFFF).to_be_bytes());
        control_info.extend_from_slice(&mtu.to_be_bytes());
        control_info.extend_from_slice(&flow_window.to_be_bytes());
        control_info.extend_from_slice(&invalid_type.to_be_bytes());
        control_info.extend_from_slice(&socket_id.to_be_bytes());
        control_info.extend_from_slice(&syn_cookie.to_be_bytes());
        // peer_ip (16 bytes)
        control_info.extend_from_slice(&[0u8; 16]);

        let packet = ControlPacket {
            control_type: ControlType::Handshake,
            subtype: 0,
            type_specific_info: 0,
            timestamp: 0,
            dest_socket_id: 0,
            control_info,
        };

        let result = HandshakePacket::decode(&packet);
        prop_assert!(result.is_err());
    }

    // ===== Decode with short buffer =====
    #[test]
    fn test_decode_short_buffer(
        data in prop::collection::vec(any::<u8>(), 0..48),
    ) {
        let packet = ControlPacket {
            control_type: ControlType::Handshake,
            subtype: 0,
            type_specific_info: 0,
            timestamp: 0,
            dest_socket_id: 0,
            control_info: data,
        };

        let result = HandshakePacket::decode(&packet);
        prop_assert!(result.is_err());
    }

    // ===== KmMessage decode with invalid signature =====
    #[test]
    fn test_km_message_decode_invalid_signature(
        version in 0u8..8,
        packet_type in 0u8..16,
        invalid_signature in any::<u16>().prop_filter("not KM signature", |s| *s != 0x2029),
        kk in 1u8..3,
    ) {
        let mut data = vec![
            (version << 4) | packet_type,
            (invalid_signature >> 8) as u8,
            (invalid_signature & 0xFF) as u8,
            kk,
        ];
        // KEKI + cipher + auth + SE + resv2 + resv3 + slen + klen + salt
        data.extend_from_slice(&[0u8; 4]); // KEKI
        data.extend_from_slice(&[2, 0, 2, 0]); // cipher, auth, SE, resv2
        data.extend_from_slice(&[0, 0, 4, 4]); // resv3, slen/4, klen/4
        data.extend_from_slice(&[0u8; 16]); // salt
        data.extend_from_slice(&[0u8; 24]); // wrapped key

        let result = KmMessage::decode(&data);
        prop_assert!(result.is_err());
    }

    // ===== KmMessage decode with invalid salt length =====
    #[test]
    fn test_km_message_decode_invalid_salt_length(
        invalid_slen in 0u8..4, // 0, 1, 2, 3 → 実際のサイズは 0, 4, 8, 12 バイト (16 以外)
    ) {
        let mut data = vec![
            0x12, // version=1, packet_type=2
            0x20, 0x29, // signature
            0x02, // kk=Even
        ];
        data.extend_from_slice(&[0u8; 4]); // KEKI
        data.extend_from_slice(&[2, 0, 2, 0]); // cipher, auth, SE, resv2
        data.extend_from_slice(&[0, 0, invalid_slen, 4]); // resv3, invalid slen/4, klen/4

        let result = KmMessage::decode(&data);
        prop_assert!(result.is_err());
    }

    // ===== KmMessage decode with invalid key length =====
    #[test]
    fn test_km_message_decode_invalid_key_length(
        invalid_klen in any::<u8>().prop_filter("not valid klen/4", |k| {
            !matches!(*k, 4 | 6 | 8) // 16, 24, 32 byte keys
        }),
    ) {
        let mut data = vec![
            0x12, // version=1, packet_type=2
            0x20, 0x29, // signature
            0x02, // kk=Even
        ];
        data.extend_from_slice(&[0u8; 4]); // KEKI
        data.extend_from_slice(&[2, 0, 2, 0]); // cipher, auth, SE, resv2
        data.extend_from_slice(&[0, 0, 4, invalid_klen]); // resv3, slen/4=4, invalid klen/4
        data.extend_from_slice(&[0u8; 16]); // salt
        data.extend_from_slice(&[0u8; 24]); // some wrapped key

        let result = KmMessage::decode(&data);
        prop_assert!(result.is_err());
    }

    // ===== KmMessage decode too short =====
    #[test]
    fn test_km_message_decode_too_short(
        data in prop::collection::vec(any::<u8>(), 0..16),
    ) {
        let result = KmMessage::decode(&data);
        prop_assert!(result.is_err());
    }

    // ===== HandshakeExtension data =====
    #[test]
    fn test_handshake_extension_struct(
        ext_type in prop::sample::select(vec![
            ExtensionType::HsReq,
            ExtensionType::HsRsp,
            ExtensionType::KmReq,
            ExtensionType::KmRsp,
            ExtensionType::Sid,
            ExtensionType::Congestion,
        ]),
        data in prop::collection::vec(any::<u8>(), 0..100),
    ) {
        let ext = HandshakeExtension {
            ext_type,
            data: data.clone(),
        };

        prop_assert_eq!(ext.ext_type, ext_type);
        prop_assert_eq!(ext.data, data);
    }

    // ===== Extension parsing with unknown type =====
    #[test]
    fn test_extension_parsing_unknown_type(
        version in any::<u32>(),
        encryption_field in any::<u16>(),
        extension_field in any::<u16>(),
        socket_id in any::<u32>(),
        unknown_ext_type in any::<u16>().prop_filter("not a valid ext type", |v| {
            !matches!(*v, 1..=8)
        }),
    ) {
        // 有効なハンドシェイクパケットを作成し、未知の拡張タイプを追加
        let mut control_info = Vec::new();
        control_info.extend_from_slice(&version.to_be_bytes());
        control_info.extend_from_slice(&encryption_field.to_be_bytes());
        control_info.extend_from_slice(&extension_field.to_be_bytes());
        control_info.extend_from_slice(&0u32.to_be_bytes()); // initial_packet_seq
        control_info.extend_from_slice(&1500u32.to_be_bytes()); // mtu
        control_info.extend_from_slice(&8192u32.to_be_bytes()); // flow_window
        control_info.extend_from_slice(&0x00000001u32.to_be_bytes()); // handshake_type = Induction
        control_info.extend_from_slice(&socket_id.to_be_bytes());
        control_info.extend_from_slice(&0u32.to_be_bytes()); // syn_cookie
        control_info.extend_from_slice(&[0u8; 16]); // peer_ip

        // 未知の拡張タイプ
        control_info.extend_from_slice(&unknown_ext_type.to_be_bytes());
        control_info.extend_from_slice(&1u16.to_be_bytes()); // length in words = 1
        control_info.extend_from_slice(&[0u8; 4]); // 4 bytes of extension data

        let packet = ControlPacket {
            control_type: ControlType::Handshake,
            subtype: 0,
            type_specific_info: 0,
            timestamp: 0,
            dest_socket_id: 0,
            control_info,
        };

        // デコードは成功するが、未知の拡張は無視される
        let result = HandshakePacket::decode(&packet);
        prop_assert!(result.is_ok());
        let decoded = result.expect("既知の拡張のみを含むパケットのデコードは成功する想定");
        prop_assert!(decoded.extensions.is_empty());
    }

    // ===== KmMessage with invalid KK field =====
    #[test]
    fn test_km_message_decode_invalid_kk_field(
        invalid_kk in any::<u8>().prop_filter("not valid kk", |k| {
            !matches!(*k & 0x03, 0x01 | 0x02)
        }),
    ) {
        let mut data = vec![
            0x12, // version=1, packet_type=2
            0x20, 0x29, // signature
            invalid_kk, // invalid KK field
        ];
        data.extend_from_slice(&[0u8; 4]); // KEKI
        data.extend_from_slice(&[2, 0, 2, 0]); // cipher, auth, SE, resv2
        data.extend_from_slice(&[0, 0, 4, 4]); // resv3, slen/4, klen/4
        data.extend_from_slice(&[0u8; 16]); // salt
        data.extend_from_slice(&[0u8; 24]); // wrapped key

        let result = KmMessage::decode(&data);
        prop_assert!(result.is_err());
    }

    // ===== get_hs_extension with short data =====
    #[test]
    fn test_get_hs_extension_short_data(
        short_len in 0usize..12,
    ) {
        let mut packet = HandshakePacket::new_conclusion_request(1, 2, 3, 0, false);
        packet.extensions.push(HandshakeExtension {
            ext_type: ExtensionType::HsReq,
            data: vec![0u8; short_len],
        });

        // 12 バイト未満のデータでは None を返す
        let result = packet.get_hs_extension();
        prop_assert!(result.is_none());
    }

    // ===== No extensions =====
    #[test]
    fn test_no_extensions(socket_id in any::<u32>()) {
        let packet = HandshakePacket::new_induction_request(socket_id);

        prop_assert!(packet.get_hs_extension().is_none());
        prop_assert!(packet.get_km_request().is_none());
        prop_assert!(packet.get_km_response().is_ok());
        prop_assert!(
            packet
                .get_km_response()
                .expect("KM レスポンスの取得は成功する想定")
                .is_none()
        );
        prop_assert!(packet.get_sid_extension().is_none());
        prop_assert!(packet.get_congestion_extension().is_none());
    }
}
