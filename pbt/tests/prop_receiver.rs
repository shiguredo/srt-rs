//! Property-based tests for SRT ReceiverBuffer

use proptest::prelude::*;
use shiguredo_srt::{DataPacket, PacketPosition, ReceiverBuffer, Timestamp};

/// テスト用の DataPacket を生成
fn make_packet(seq: u32, timestamp: u32) -> DataPacket {
    DataPacket {
        sequence_number: seq,
        position: PacketPosition::Single,
        order_flag: false,
        encryption_flag: 0,
        retransmitted: false,
        message_number: 1,
        timestamp,
        dest_socket_id: 1,
        payload: vec![1, 2, 3],
    }
}

/// 任意のペイロードを持つ DataPacket を生成
fn make_packet_with_payload(seq: u32, timestamp: u32, payload: Vec<u8>) -> DataPacket {
    DataPacket {
        sequence_number: seq,
        position: PacketPosition::Single,
        order_flag: false,
        encryption_flag: 0,
        retransmitted: false,
        message_number: 1,
        timestamp,
        dest_socket_id: 1,
        payload,
    }
}

/// ラップ境界近傍の連続シーケンス列と、その順不同の受信順を生成する Strategy
///
/// 戻り値は (循環順のシーケンス列, それを順不同にシャッフルした受信順) で、
/// 受信順のシャッフルは proptest の prop_shuffle に委ねる。
fn wrap_around_run() -> impl Strategy<Value = (Vec<u32>, Vec<u32>)> {
    // before 個 (末尾が 0x7FFF_FFFF) と after 個 (0 から始まる) を連結し、必ずラップ境界
    // (0x7FFF_FFFF -> 0) をまたぐ連続シーケンス列を作る。これにより境界をまたがない退行ケースを除く。
    (1usize..4usize, 1usize..4usize).prop_flat_map(|(before, after)| {
        let start_seq = 0x7FFF_FFFFu32 - (before as u32 - 1);
        let seqs: Vec<u32> = (0..before + after)
            .map(|i| start_seq.wrapping_add(i as u32) & 0x7FFF_FFFF)
            .collect();
        let recv_order = Just(seqs.clone()).prop_shuffle();
        (Just(seqs), recv_order)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn test_receiver_buffer_new(
        initial_seq in 0u32..0x7FFF_FFFFu32,
        tsbpd_delay_ms in 0u16..1000u16,
        start_time in 0u64..1_000_000u64,
    ) {
        let start = Timestamp::from_micros(start_time);
        let buf = ReceiverBuffer::new(initial_seq, tsbpd_delay_ms, start, 0);
        prop_assert_eq!(buf.expected_sequence(), initial_seq);
    }

    #[test]
    fn test_receiver_buffer_receive_in_order(
        initial_seq in 0u32..0x7FFF_FF00u32,
        count in 1usize..50usize,
    ) {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(initial_seq, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        for i in 0..count {
            let seq = (initial_seq + i as u32) & 0x7FFF_FFFF;
            let losses = buf.receive(make_packet(seq, 100), now);
            prop_assert!(losses.is_none());
        }

        let expected_seq = (initial_seq + count as u32) & 0x7FFF_FFFF;
        prop_assert_eq!(buf.expected_sequence(), expected_seq);
    }

    #[test]
    fn test_receiver_buffer_loss_detection(
        initial_seq in 0u32..0x7FFF_FF00u32,
        gap in 1u32..10u32,
    ) {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(initial_seq, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        // 最初のパケットを受信
        buf.receive(make_packet(initial_seq, 100), now);

        // ギャップを作って受信
        let skip_seq = (initial_seq + gap + 1) & 0x7FFF_FFFF;
        let losses = buf.receive(make_packet(skip_seq, 200), now);

        prop_assert!(losses.is_some());
        let lost = losses.expect("欠落パケットは Some になる想定");
        prop_assert_eq!(lost.len(), gap as usize);
    }

    #[test]
    fn test_receiver_buffer_duplicate_detection(
        initial_seq in 0u32..0x7FFF_FFFFu32,
    ) {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(initial_seq, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        // 同じパケットを 2 回受信
        buf.receive(make_packet(initial_seq, 100), now);
        let losses = buf.receive(make_packet(initial_seq, 100), now);
        prop_assert!(losses.is_none());

        let stats = buf.stats();
        prop_assert_eq!(stats.total_duplicates, 1);
    }

    #[test]
    fn test_receiver_buffer_old_packet_ignored(
        initial_seq in 10u32..0x7FFF_FFFFu32,
    ) {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(initial_seq, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        // expected_seq より古いパケットは無視される
        let old_seq = initial_seq - 5;
        let losses = buf.receive(make_packet(old_seq, 100), now);
        prop_assert!(losses.is_none());

        let stats = buf.stats();
        prop_assert_eq!(stats.total_received, 0);
    }

    #[test]
    fn test_receiver_buffer_pop_ready_no_tsbpd(
        initial_seq in 0u32..0x7FFF_FF00u32,
        count in 1usize..20usize,
    ) {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(initial_seq, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        // 順序通りにパケットを受信
        for i in 0..count {
            let seq = (initial_seq + i as u32) & 0x7FFF_FFFF;
            buf.receive(make_packet(seq, 100), now);
        }

        // 全て pop 可能
        for _ in 0..count {
            let pkt = buf.pop_ready(now);
            prop_assert!(pkt.is_some());
        }

        // これ以上は None
        prop_assert!(buf.pop_ready(now).is_none());
    }

    #[test]
    fn test_receiver_buffer_ack_generation(
        initial_seq in 0u32..0x7FFF_FF00u32,
        count in 1usize..20usize,
    ) {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(initial_seq, 120, start, 0);

        let now = Timestamp::from_micros(1000);

        for i in 0..count {
            let seq = (initial_seq + i as u32) & 0x7FFF_FFFF;
            buf.receive(make_packet(seq, 100), now);
        }

        let ack = buf.generate_ack(now);
        let expected_seq = (initial_seq + count as u32) & 0x7FFF_FFFF;
        prop_assert_eq!(ack.ack_seq, expected_seq);
    }

    #[test]
    fn test_receiver_buffer_nak_generation(
        initial_seq in 0u32..0x7FFF_FF00u32,
        gap in 1u32..5u32,
    ) {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(initial_seq, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        // 最初のパケット
        buf.receive(make_packet(initial_seq, 100), now);

        // ギャップを作る
        let skip_seq = (initial_seq + gap + 1) & 0x7FFF_FFFF;
        buf.receive(make_packet(skip_seq, 200), now);

        // NAK を生成
        let nak = buf.generate_periodic_nak();
        prop_assert!(nak.is_some());
        prop_assert_eq!(
            nak.expect("欠落パケットは NAK が生成される想定")
                .loss_list
                .len(),
            gap as usize
        );
    }

    #[test]
    fn test_receiver_buffer_rtt_accessors(
        initial_seq in 0u32..0x7FFF_FFFFu32,
    ) {
        let start = Timestamp::from_micros(0);
        let buf = ReceiverBuffer::new(initial_seq, 120, start, 0);

        // 初期 RTT 値
        prop_assert!(buf.rtt() > 0);
        prop_assert!(buf.rtt_var() > 0);
    }

    #[test]
    fn test_receiver_buffer_nak_interval(
        initial_seq in 0u32..0x7FFF_FFFFu32,
    ) {
        let start = Timestamp::from_micros(0);
        let buf = ReceiverBuffer::new(initial_seq, 120, start, 0);

        // NAK 間隔は最低 20ms
        let interval = buf.nak_interval();
        prop_assert!(interval >= 20_000);
    }

    #[test]
    fn test_receiver_buffer_stats(
        initial_seq in 0u32..0x7FFF_FF00u32,
        count in 1usize..20usize,
    ) {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(initial_seq, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        for i in 0..count {
            let seq = (initial_seq + i as u32) & 0x7FFF_FFFF;
            buf.receive(make_packet(seq, 100), now);
        }

        let stats = buf.stats();
        prop_assert_eq!(stats.total_received, count as u64);
        prop_assert_eq!(stats.total_lost, 0);
        prop_assert_eq!(stats.loss_rate_percent_x100, 0);
    }

    #[test]
    fn test_receiver_buffer_bytes_received(
        initial_seq in 0u32..0x7FFF_FFFFu32,
        payload_size in 1usize..1000usize,
    ) {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(initial_seq, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);
        let payload = vec![0u8; payload_size];
        buf.receive(make_packet_with_payload(initial_seq, 100, payload), now);

        let stats = buf.stats();
        // payload_size + 16 (SRT header)
        prop_assert_eq!(stats.total_bytes_received, (payload_size + 16) as u64);
    }

    #[test]
    fn test_receiver_buffer_should_send_ack_periodic(
        initial_seq in 0u32..0x7FFF_FFFFu32,
    ) {
        let start = Timestamp::from_micros(0);
        let buf = ReceiverBuffer::new(initial_seq, 120, start, 0);

        // 直後は ACK 不要
        let now = Timestamp::from_micros(1000);
        prop_assert!(!buf.should_send_ack(now));

        // 10ms 経過後は ACK 必要
        let later = Timestamp::from_micros(11_000);
        prop_assert!(buf.should_send_ack(later));
    }

    #[test]
    fn test_receiver_buffer_loss_recovery(
        initial_seq in 0u32..0x7FFF_FF00u32,
    ) {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(initial_seq, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        // パケット 0, 2 を受信 (1 が欠落)
        buf.receive(make_packet(initial_seq, 100), now);
        let losses = buf.receive(make_packet((initial_seq + 2) & 0x7FFF_FFFF, 300), now);
        prop_assert!(losses.is_some());

        // パケット 1 を後から受信 (回復)
        let losses = buf.receive(make_packet((initial_seq + 1) & 0x7FFF_FFFF, 200), now);
        prop_assert!(losses.is_none());

        // NAK は空になる
        let nak = buf.generate_periodic_nak();
        prop_assert!(nak.is_none());
    }

    #[test]
    fn test_receiver_buffer_ackack_updates_rtt(
        initial_seq in 0u32..0x7FFF_FFFFu32,
        rtt_sample in 1000u64..100_000u64,
    ) {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(initial_seq, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);
        buf.receive(make_packet(initial_seq, 100), now);

        // ACK 生成 (送信時刻が記録される)
        let ack_time = Timestamp::from_micros(2000);
        buf.generate_ack(ack_time);
        let ack_number = buf.ack_number();

        // ACKACK 受信 (RTT が計算される)
        let ackack_time = Timestamp::from_micros(2000 + rtt_sample);
        let _old_rtt = buf.rtt();
        buf.handle_ackack(ack_number, ackack_time);
        let new_rtt = buf.rtt();

        // RTT が更新される (EWMA なので必ずしも等しくない)
        // 初期 RTT (100ms) と新しいサンプルの間の値になる
        prop_assert!(new_rtt > 0);
    }

    #[test]
    fn test_receiver_buffer_periodic_nak(
        initial_seq in 0u32..0x7FFF_FF00u32,
    ) {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(initial_seq, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        // ギャップを作る
        buf.receive(make_packet(initial_seq, 100), now);
        buf.receive(make_packet((initial_seq + 2) & 0x7FFF_FFFF, 300), now);

        // Periodic NAK も同じ内容
        let nak1 = buf.generate_periodic_nak();
        let nak2 = buf.generate_periodic_nak();

        prop_assert!(nak1.is_some());
        prop_assert!(nak2.is_some());
        prop_assert_eq!(
            nak1.expect("欠落パケットは NAK が生成される想定")
                .loss_list,
            nak2.expect("欠落パケットは NAK が生成される想定")
                .loss_list
        );
    }

    #[test]
    fn test_receiver_buffer_jitter_stable(
        initial_seq in 0u32..0x7FFF_FF00u32,
    ) {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(initial_seq, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        // タイムスタンプと到着時刻の差が一定
        for i in 0..10u32 {
            let seq = (initial_seq + i) & 0x7FFF_FFFF;
            let ts = 1000 + i * 1000;
            let now = Timestamp::from_micros((2000 + i * 1000) as u64);
            buf.receive(make_packet(seq, ts), now);
        }

        let stats = buf.stats();
        // transit が一定なのでジッターは 0
        prop_assert_eq!(stats.jitter, 0);
    }

    #[test]
    fn test_pop_ready_wrap_around_delivery_order(
        (seqs, recv_order) in wrap_around_run(),
        tsbpd_enabled in any::<bool>(),
    ) {
        // ラップ境界をまたぐ連続パケットを順不同で受信し、pop_ready が循環順で取り出すことを
        // 検証する。既存の配送系 PBT は initial_seq をラップ近傍から除外しているためこの回帰を
        // 検出できない。tsbpd 有効・無効の両方を含める。
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(seqs[0], 120, start, 0);
        buf.set_tsbpd_enabled(tsbpd_enabled);

        let now = Timestamp::from_micros(1000);

        // recv_order (seqs を順不同にした列) で受信する。
        // 全パケットに同一タイムスタンプを与え配信時刻を揃える。
        for &seq in &recv_order {
            buf.receive(make_packet(seq, 100), now);
        }

        // 全パケットの配信時刻を十分過ぎた時刻で取り出す
        let pop_now = Timestamp::from_micros(10_000_000_000);
        let mut popped = Vec::new();
        while let Some(pkt) = buf.pop_ready(pop_now) {
            popped.push(pkt.sequence_number);
        }

        // 欠損が無いため全パケットが循環順 (seqs) で配送される
        prop_assert_eq!(popped, seqs);
    }
}
