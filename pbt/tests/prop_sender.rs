//! Property-based tests for SRT SenderBuffer

use proptest::prelude::*;
use shiguredo_srt::{SenderBuffer, Timestamp};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn test_sender_buffer_new(
        initial_seq in 0u32..0x7FFF_FFFFu32,
        flow_window in 1u32..10000u32,
        latency_ms in 0u16..1000u16,
    ) {
        let buf = SenderBuffer::new(initial_seq, flow_window, latency_ms);
        prop_assert_eq!(buf.next_sequence_number(), initial_seq);
        prop_assert!(buf.can_send());
        prop_assert!(buf.is_empty());
        prop_assert_eq!(buf.packets_in_flight(), 0);
    }

    #[test]
    fn test_sender_buffer_push_increments_seq(
        initial_seq in 0u32..0x7FFF_FF00u32,
        payload_len in 1usize..1000usize,
    ) {
        let mut buf = SenderBuffer::new(initial_seq, 8192, 120);
        let now = Timestamp::from_micros(0);
        let payload = vec![0u8; payload_len];

        let packet = buf.push(payload.clone(), 100, 12345, now);
        prop_assert!(packet.is_some());
        let pkt = packet.expect("送信パケットは Some になる想定");
        prop_assert_eq!(pkt.sequence_number, initial_seq);
        prop_assert_eq!(pkt.payload.len(), payload_len);
        prop_assert_eq!(buf.next_sequence_number(), (initial_seq + 1) & 0x7FFF_FFFF);
        prop_assert_eq!(buf.packets_in_flight(), 1);
    }

    #[test]
    fn test_sender_buffer_push_multiple(
        initial_seq in 0u32..0x7FFF_FF00u32,
        count in 1usize..16usize, // 初期 congestion_window は 16
    ) {
        let mut buf = SenderBuffer::new(initial_seq, 8192, 120);
        let now = Timestamp::from_micros(0);

        for i in 0..count {
            let packet = buf.push(vec![i as u8], 100, 12345, now);
            prop_assert!(packet.is_some());
        }

        prop_assert_eq!(buf.packets_in_flight(), count as u32);
        prop_assert_eq!(
            buf.next_sequence_number(),
            (initial_seq.wrapping_add(count as u32)) & 0x7FFF_FFFF
        );
    }

    #[test]
    fn test_sender_buffer_ack_clears_packets(
        initial_seq in 0u32..0x7FFF_FF00u32,
        count in 1usize..16usize, // 初期 congestion_window は 16
        ack_count in 0usize..16usize,
    ) {
        let mut buf = SenderBuffer::new(initial_seq, 8192, 120);
        let now = Timestamp::from_micros(0);

        // パケットを送信
        for i in 0..count {
            buf.push(vec![i as u8], 100, 12345, now);
        }

        // ACK を処理
        let ack_seq = initial_seq.wrapping_add(ack_count.min(count) as u32) & 0x7FFF_FFFF;
        buf.handle_ack(ack_seq);

        let expected_remaining = count.saturating_sub(ack_count);
        prop_assert_eq!(buf.packets_in_flight(), expected_remaining as u32);
    }

    #[test]
    fn test_sender_buffer_nak_adds_to_loss_list(
        initial_seq in 0u32..0x7FFF_FF00u32,
    ) {
        let mut buf = SenderBuffer::new(initial_seq, 8192, 120);
        let now = Timestamp::from_micros(0);

        // 3 パケット送信
        buf.push(vec![1], 100, 1, now);
        buf.push(vec![2], 100, 1, now);
        buf.push(vec![3], 100, 1, now);

        prop_assert!(!buf.has_retransmit());

        // 中間のパケットを NAK
        let lost_seq = (initial_seq + 1) & 0x7FFF_FFFF;
        buf.handle_nak(&[lost_seq]);

        prop_assert!(buf.has_retransmit());
    }

    #[test]
    fn test_sender_buffer_retransmit(
        initial_seq in 0u32..0x7FFF_FF00u32,
    ) {
        let mut buf = SenderBuffer::new(initial_seq, 8192, 120);
        let now = Timestamp::from_micros(0);

        // 3 パケット送信
        buf.push(vec![1], 100, 1, now);
        buf.push(vec![2], 100, 1, now);
        buf.push(vec![3], 100, 1, now);

        // 中間のパケットを NAK
        let lost_seq = (initial_seq + 1) & 0x7FFF_FFFF;
        buf.handle_nak(&[lost_seq]);

        // 再送パケットを取得
        let retransmit = buf.pop_retransmit(now);
        prop_assert!(retransmit.is_some());
        let pkt = retransmit.expect("再送パケットは Some になる想定");
        prop_assert_eq!(pkt.sequence_number, lost_seq);
        prop_assert!(pkt.retransmitted);

        // 再送後は loss_list が空
        prop_assert!(!buf.has_retransmit());
    }

    #[test]
    fn test_sender_buffer_flow_window(
        flow_window in 1u32..16u32, // 初期 congestion_window は 16
    ) {
        let mut buf = SenderBuffer::new(0, flow_window, 120);
        buf.set_congestion_window(1000); // フローウィンドウテスト用に増やす
        let now = Timestamp::from_micros(0);

        // フローウィンドウ分のパケットを送信
        for i in 0..flow_window {
            let packet = buf.push(vec![i as u8], 100, 1, now);
            prop_assert!(packet.is_some());
        }

        // これ以上は送信不可
        prop_assert!(!buf.can_send());
        let packet = buf.push(vec![0], 100, 1, now);
        prop_assert!(packet.is_none());
    }

    #[test]
    fn test_sender_buffer_congestion_window(
        cwnd in 1u32..50u32,
    ) {
        let mut buf = SenderBuffer::new(0, 8192, 120);
        buf.set_congestion_window(cwnd);
        let now = Timestamp::from_micros(0);

        // 輻輳ウィンドウ分のパケットを送信
        for i in 0..cwnd {
            let packet = buf.push(vec![i as u8], 100, 1, now);
            prop_assert!(packet.is_some());
        }

        // これ以上は送信不可
        prop_assert!(!buf.can_send());
    }

    #[test]
    fn test_sender_buffer_pacing(
        period in 100u64..10000u64,
    ) {
        let mut buf = SenderBuffer::new(0, 8192, 120);
        buf.set_packet_send_period(period);

        // 送信時刻を記録
        let send_time = Timestamp::from_micros(0);
        buf.record_send_time(send_time);

        // 直後は送信不可
        let half_period = Timestamp::from_micros(period / 2);
        prop_assert!(!buf.can_send_with_pacing(half_period));
        prop_assert!(buf.time_until_send(half_period) > 0);

        // period 経過後は送信可能
        let after_period = Timestamp::from_micros(period);
        prop_assert!(buf.can_send_with_pacing(after_period));
        prop_assert_eq!(buf.time_until_send(after_period), 0);
    }

    #[test]
    fn test_sender_buffer_stats(
        count in 1usize..16usize, // 初期 congestion_window は 16
    ) {
        let mut buf = SenderBuffer::new(0, 8192, 120);
        let now = Timestamp::from_micros(0);

        for i in 0..count {
            buf.push(vec![i as u8; 100], 100, 1, now);
        }

        let stats = buf.stats();
        prop_assert_eq!(stats.packets_in_buffer, count as u32);
        prop_assert_eq!(stats.total_sent, count as u64);
        prop_assert_eq!(stats.total_bytes_sent, (count * 100) as u64);
    }

    #[test]
    fn test_sender_buffer_drop_expired(
        latency_ms in 10u16..1000u16,
    ) {
        let mut buf = SenderBuffer::new(0, 8192, latency_ms);
        let send_time = Timestamp::from_micros(0);

        // パケットを送信
        buf.push(vec![1], 100, 1, send_time);
        buf.push(vec![2], 100, 1, send_time);

        // 新閾値: max(latency_us * 125 / 100, 1_000_000)
        let latency_us = latency_ms as u64 * 1000;
        let threshold = (latency_us * 125 / 100).max(1_000_000);

        // 閾値経過前は drop されない
        let before_expire = Timestamp::from_micros(threshold.saturating_sub(1000));
        let dropped = buf.drop_expired(before_expire);
        prop_assert!(dropped.is_empty());

        // 閾値経過後は drop される
        let after_expire = Timestamp::from_micros(threshold + 1000);
        let dropped = buf.drop_expired(after_expire);
        prop_assert_eq!(dropped.len(), 2);
    }

    #[test]
    fn test_sender_buffer_message_split(
        payload_size in 1usize..1500usize, // congestion_window に収まるように
        max_payload in 100usize..500usize,
    ) {
        let mut buf = SenderBuffer::new(0, 8192, 120);
        buf.set_congestion_window(1000); // 大きなメッセージ用に増やす
        let now = Timestamp::from_micros(0);
        let payload = vec![0u8; payload_size];

        let packets = buf.push_message(&payload, max_payload, 100, 1, now);

        let expected_count = payload_size.div_ceil(max_payload);
        prop_assert_eq!(packets.len(), expected_count);

        // 全ペイロードが含まれているか確認
        let total_bytes: usize = packets.iter().map(|p| p.payload.len()).sum();
        prop_assert_eq!(total_bytes, payload_size);
    }

    #[test]
    fn test_sender_buffer_oldest_packet_time(
        initial_seq in 0u32..0x7FFF_FF00u32,
    ) {
        let mut buf = SenderBuffer::new(initial_seq, 8192, 120);

        // 空の場合は None
        prop_assert!(buf.oldest_packet_time().is_none());

        // パケットを追加
        let t1 = Timestamp::from_micros(1000);
        buf.push(vec![1], 100, 1, t1);

        let t2 = Timestamp::from_micros(2000);
        buf.push(vec![2], 100, 1, t2);

        // 最古のパケット時刻を取得
        let oldest = buf.oldest_packet_time();
        prop_assert!(oldest.is_some());
        prop_assert_eq!(
            oldest.expect("最古パケット時刻は Some になる想定")
                .as_micros(),
            1000
        );
    }

    #[test]
    fn test_sequence_wraparound(
        offset in 0u32..5u32, // 初期 congestion_window は 16
    ) {
        // シーケンス番号のラップアラウンドをテスト
        let initial_seq = 0x7FFF_FFFF - offset;
        let mut buf = SenderBuffer::new(initial_seq, 8192, 120);
        buf.set_congestion_window(100); // ラップアラウンドテスト用に増やす
        let now = Timestamp::from_micros(0);

        // ラップアラウンドを超えてパケットを送信
        for _ in 0..(offset + 10) {
            buf.push(vec![0], 100, 1, now);
        }

        // シーケンス番号が正しくラップアラウンド
        let expected = ((initial_seq as u64 + offset as u64 + 10) & 0x7FFF_FFFF) as u32;
        prop_assert_eq!(buf.next_sequence_number(), expected);
    }
}
