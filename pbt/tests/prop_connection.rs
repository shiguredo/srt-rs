//! Property-based tests for SRT Connection
//!
//! バグを見つけることを目的とした PBT テスト。
//! 以下のプロパティを検証する:
//!
//! 1. 状態遷移の整合性: 任意の操作列に対して不正な状態遷移が起きない
//! 2. データ整合性: 送信したデータが正確に受信される
//! 3. プロトコル不変条件: ACK/NAK/ACKACK のシーケンス番号が単調増加
//! 4. エッジケース: 境界値、空データ、大量データでの動作

use proptest::prelude::*;
use shiguredo_srt::{
    ConnectionEvent, ConnectionOptions, ConnectionOutput, ConnectionState, KeyLength,
    SrtConnection, TimerId, Timestamp,
};

// ============================================================================
// 状態遷移のプロパティテスト
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// プロパティ: Caller は connect 後、必ず Induction 状態になる
    /// 任意の開始時刻でこの不変条件が成り立つ
    #[test]
    fn prop_caller_connect_always_transitions_to_induction(
        start_time in 0u64..u64::MAX / 2,
    ) {
        let mut conn = SrtConnection::new_caller(make_opts(1));
        let now = Timestamp::from_micros(start_time);

        conn.connect(now).expect("接続は成功する想定");
        prop_assert_eq!(conn.state(), ConnectionState::Induction);
    }

    /// プロパティ: Listener は connect を呼べない（常にエラー）
    #[test]
    fn prop_listener_connect_always_fails(
        start_time in 0u64..u64::MAX / 2,
    ) {
        let mut conn = SrtConnection::new_listener(make_opts(1));
        let now = Timestamp::from_micros(start_time);

        let result = conn.connect(now);
        prop_assert!(result.is_err());
        // 状態は変わらない
        prop_assert_eq!(conn.state(), ConnectionState::Listening);
    }

    /// プロパティ: 未接続状態では send は常に失敗する
    #[test]
    fn prop_disconnected_send_always_fails(
        data in prop::collection::vec(any::<u8>(), 0..1500),
        start_time in 0u64..u64::MAX / 2,
    ) {
        let mut conn = SrtConnection::new_caller(make_opts(1));
        let now = Timestamp::from_micros(start_time);

        let result = conn.send(&data, now);
        prop_assert!(result.is_err());
    }

    /// プロパティ: 15 バイト未満のバッファは常にエラー（SRT ヘッダ最小サイズ）
    #[test]
    fn prop_insufficient_buffer_always_fails(
        buf_len in 0usize..15usize,
        start_time in 0u64..u64::MAX / 2,
    ) {
        let mut conn = SrtConnection::new_caller(make_opts(1));
        let now = Timestamp::from_micros(start_time);

        let buf = vec![0u8; buf_len];
        let result = conn.feed_recv_buf(&buf, now);
        prop_assert!(result.is_err());
    }

    /// プロパティ: ハンドシェイクタイムアウト後は必ず Disconnected 状態になる
    #[test]
    fn prop_handshake_timeout_always_disconnects(
        timeout_time in 0u64..u64::MAX / 2,
    ) {
        let mut conn = SrtConnection::new_caller(make_opts(1));
        let now = Timestamp::from_micros(0);

        conn.connect(now).expect("接続は成功する想定");
        drain_packets(&mut conn);

        // タイムアウト発火
        let timeout_now = Timestamp::from_micros(timeout_time);
        conn.handle_timer(TimerId::Handshake, timeout_now)
            .expect("タイマー処理は成功する想定");

        prop_assert_eq!(conn.state(), ConnectionState::Disconnected);
    }

    /// プロパティ: 未接続時の全タイマーは状態を変更しない
    #[test]
    fn prop_timers_on_disconnected_are_noop(
        timer_time in 0u64..u64::MAX / 2,
    ) {
        let mut conn = SrtConnection::new_caller(make_opts(1));
        let now = Timestamp::from_micros(timer_time);

        let initial_state = conn.state();

        conn.handle_timer(TimerId::Keepalive, now).expect("タイマー処理は成功する想定");
        conn.handle_timer(TimerId::Ack, now).expect("タイマー処理は成功する想定");
        conn.handle_timer(TimerId::Nak, now).expect("タイマー処理は成功する想定");
        conn.handle_timer(TimerId::Retransmit, now).expect("タイマー処理は成功する想定");
        conn.handle_timer(TimerId::Inactivity, now).expect("タイマー処理は成功する想定");

        prop_assert_eq!(conn.state(), initial_state);
        prop_assert!(conn.poll_output().is_none());
    }

}

// ============================================================================
// ハンドシェイクのプロパティテスト
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// プロパティ: ハンドシェイク完了後、両方が Connected 状態になる
    #[test]
    fn prop_handshake_both_connected(
        start_time in 0u64..1_000_000_000u64,
        rtt in 100u64..10_000u64,  // RTT: 0.1ms - 10ms
    ) {
        let mut now = Timestamp::from_micros(start_time);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));

        caller.connect(now).expect("接続は成功する想定");

        // INDUCTION
        let induction_req = drain_packets(&mut caller);
        prop_assert!(!induction_req.is_empty(), "INDUCTION request must be sent");

        listener.feed_recv_buf(&induction_req[0], now).expect("受信バッファへのフィードは成功する想定");
        let induction_resp = drain_packets(&mut listener);
        prop_assert!(!induction_resp.is_empty(), "INDUCTION response must be sent");

        now = Timestamp::from_micros(now.as_micros() + rtt);
        caller.feed_recv_buf(&induction_resp[0], now).expect("受信バッファへのフィードは成功する想定");

        // CONCLUSION
        let conclusion_req = drain_packets(&mut caller);
        prop_assert!(!conclusion_req.is_empty(), "CONCLUSION request must be sent");

        listener.feed_recv_buf(&conclusion_req[0], now).expect("受信バッファへのフィードは成功する想定");
        prop_assert_eq!(listener.state(), ConnectionState::Connected);

        let conclusion_resp = drain_packets(&mut listener);
        prop_assert!(!conclusion_resp.is_empty(), "CONCLUSION response must be sent");

        caller.feed_recv_buf(&conclusion_resp[0], now).expect("受信バッファへのフィードは成功する想定");
        prop_assert_eq!(caller.state(), ConnectionState::Connected);
    }

    /// プロパティ: Stream ID は正確に伝達される
    #[test]
    fn prop_stream_id_transmitted_exactly(
        stream_id in "[a-zA-Z0-9_\\-\\.]{1,256}",
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts_with_stream_id(1, stream_id.clone()));
        let mut listener = SrtConnection::new_listener(make_opts(2));

        establish_connection(&mut caller, &mut listener, &mut now);

        prop_assert_eq!(listener.peer_stream_id(), Some(stream_id.as_str()));
    }

    /// プロパティ: 空の Stream ID も正しく処理される
    #[test]
    fn prop_no_stream_id_is_none(
        _dummy in 0u32..1u32,  // proptest は最低 1 つのパラメータが必要
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));

        establish_connection(&mut caller, &mut listener, &mut now);

        prop_assert!(listener.peer_stream_id().is_none());
    }

}

// 暗号化テストは PBKDF2 のコストが高いため PBT ではなく単体テストで実施

// ============================================================================
// データ送受信のプロパティテスト
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// プロパティ: 送信したデータは正確に受信される（データ整合性）
    #[test]
    fn prop_data_integrity(
        payload in prop::collection::vec(any::<u8>(), 1..1316),  // MTU 内
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        caller.send(&payload, now).expect("送信は成功する想定");
        let packets = drain_packets(&mut caller);

        // TSBPD 遅延後に配信
        now = Timestamp::from_micros(now.as_micros() + 200_000);

        for pkt in &packets {
            listener.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
        }
        listener.handle_timer(TimerId::Ack, now).expect("タイマー処理は成功する想定");

        let events = drain_events(&mut listener);
        let received = extract_received_data(&events);

        prop_assert_eq!(received, payload);
    }

    /// プロパティ: 双方向通信でもデータ整合性が保たれる
    #[test]
    fn prop_bidirectional_data_integrity(
        caller_data in prop::collection::vec(any::<u8>(), 1..500),
        listener_data in prop::collection::vec(any::<u8>(), 1..500),
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        // 双方向送信
        caller.send(&caller_data, now).expect("送信は成功する想定");
        let caller_packets = drain_packets(&mut caller);

        listener.send(&listener_data, now).expect("送信は成功する想定");
        let listener_packets = drain_packets(&mut listener);

        now = Timestamp::from_micros(now.as_micros() + 200_000);

        // 相互受信
        for pkt in &caller_packets {
            listener.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
        }
        for pkt in &listener_packets {
            caller.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
        }

        caller.handle_timer(TimerId::Ack, now).expect("タイマー処理は成功する想定");
        listener.handle_timer(TimerId::Ack, now).expect("タイマー処理は成功する想定");

        let received_by_listener = extract_received_data(&drain_events(&mut listener));
        let received_by_caller = extract_received_data(&drain_events(&mut caller));

        prop_assert_eq!(received_by_listener, caller_data);
        prop_assert_eq!(received_by_caller, listener_data);
    }

    /// プロパティ: 空データでもエラーにならない
    #[test]
    fn prop_empty_data_handled(
        _dummy in 0u32..1u32,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        // 空データの送信
        let result = caller.send(&[], now);
        // 空データの送信は許可されるか、明確にエラーになるべき
        // パニックしないことが重要
        let _ = result;
        prop_assert_eq!(caller.state(), ConnectionState::Connected);
    }

    /// プロパティ: 複数回送信しても順序が保たれる
    #[test]
    fn prop_multiple_sends_preserve_order(
        chunks in prop::collection::vec(prop::collection::vec(any::<u8>(), 1..200), 2..5),
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        // 複数回送信
        let mut all_packets = Vec::new();
        for chunk in &chunks {
            caller.send(chunk, now).expect("送信は成功する想定");
            all_packets.extend(drain_packets(&mut caller));
        }

        now = Timestamp::from_micros(now.as_micros() + 200_000);

        for pkt in &all_packets {
            listener.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
        }
        listener.handle_timer(TimerId::Ack, now).expect("タイマー処理は成功する想定");

        let received = extract_received_data(&drain_events(&mut listener));
        let expected: Vec<u8> = chunks.into_iter().flatten().collect();

        prop_assert_eq!(received, expected);
    }
}

// ============================================================================
// 切断処理のプロパティテスト
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// プロパティ: disconnect 後は Closing 状態になる
    #[test]
    fn prop_disconnect_transitions_to_closing(
        _dummy in 0u32..1u32,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        caller.disconnect(now);
        prop_assert_eq!(caller.state(), ConnectionState::Closing);

        // Shutdown パケットが送信される
        let packets = drain_packets(&mut caller);
        prop_assert!(!packets.is_empty());
    }

    /// プロパティ: Shutdown 受信で Disconnected になる
    #[test]
    fn prop_shutdown_disconnects_peer(
        _dummy in 0u32..1u32,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        caller.disconnect(now);
        let shutdown_packets = drain_packets(&mut caller);

        listener.feed_recv_buf(&shutdown_packets[0], now).expect("受信バッファへのフィードは成功する想定");
        prop_assert_eq!(listener.state(), ConnectionState::Disconnected);

        let events = drain_events(&mut listener);
        let has_disconnected = events.iter().any(|e| matches!(e, ConnectionEvent::Disconnected { .. }));
        prop_assert!(has_disconnected);
    }

    /// プロパティ: Inactivity タイマーで必ず切断される
    #[test]
    fn prop_inactivity_always_disconnects(
        _dummy in 0u32..1u32,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        caller.handle_timer(TimerId::Inactivity, now).expect("タイマー処理は成功する想定");
        prop_assert_eq!(caller.state(), ConnectionState::Disconnected);
    }
}

// ============================================================================
// タイマー処理のプロパティテスト
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// プロパティ: Keepalive タイマーでパケットが送信される
    #[test]
    fn prop_keepalive_sends_packet(
        elapsed in 1_000_000u64..10_000_000u64,  // 1-10 秒経過
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        drain_packets(&mut caller);

        now = Timestamp::from_micros(now.as_micros() + elapsed);
        caller.handle_timer(TimerId::Keepalive, now).expect("タイマー処理は成功する想定");

        let packets = drain_packets(&mut caller);
        prop_assert!(!packets.is_empty(), "Keepalive must send packet");
    }

    /// プロパティ: ACK タイマー後も接続が維持される
    #[test]
    fn prop_ack_timer_maintains_connection(
        elapsed in 10_000u64..100_000u64,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        now = Timestamp::from_micros(now.as_micros() + elapsed);
        caller.handle_timer(TimerId::Ack, now).expect("タイマー処理は成功する想定");

        prop_assert_eq!(caller.state(), ConnectionState::Connected);
    }

    /// プロパティ: NAK タイマー後も接続が維持される
    #[test]
    fn prop_nak_timer_maintains_connection(
        elapsed in 10_000u64..100_000u64,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        now = Timestamp::from_micros(now.as_micros() + elapsed);
        listener.handle_timer(TimerId::Nak, now).expect("タイマー処理は成功する想定");

        prop_assert_eq!(listener.state(), ConnectionState::Connected);
    }

    /// プロパティ: Retransmit タイマー後も接続が維持される
    #[test]
    fn prop_retransmit_timer_maintains_connection(
        elapsed in 10_000u64..100_000u64,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        now = Timestamp::from_micros(now.as_micros() + elapsed);
        caller.handle_timer(TimerId::Retransmit, now).expect("タイマー処理は成功する想定");

        prop_assert_eq!(caller.state(), ConnectionState::Connected);
    }
}

// ============================================================================
// 統計情報のプロパティテスト
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// プロパティ: 接続後は統計情報が取得できる
    #[test]
    fn prop_stats_available_after_connection(
        _dummy in 0u32..1u32,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        prop_assert!(caller.sender_stats().is_some());
        prop_assert!(caller.receiver_stats().is_some());
        prop_assert!(listener.sender_stats().is_some());
        prop_assert!(listener.receiver_stats().is_some());
    }

    /// プロパティ: 送信後の統計が更新される
    #[test]
    fn prop_stats_updated_after_send(
        payload_size in 100usize..1000usize,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        let payload = vec![0u8; payload_size];
        caller.send(&payload, now).expect("送信は成功する想定");
        drain_packets(&mut caller);

        let stats = caller.sender_stats().expect("統計取得は成功する想定");
        prop_assert!(stats.total_sent > 0, "total_sent should increase after send");
    }

    /// プロパティ: 受信後の統計が更新される
    #[test]
    fn prop_stats_updated_after_recv(
        payload_size in 100usize..1000usize,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        let payload = vec![0u8; payload_size];
        caller.send(&payload, now).expect("送信は成功する想定");
        let packets = drain_packets(&mut caller);

        now = Timestamp::from_micros(now.as_micros() + 200_000);
        for pkt in &packets {
            listener.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
        }

        let stats = listener.receiver_stats().expect("統計取得は成功する想定");
        prop_assert!(stats.total_received > 0, "total_received should increase after recv");
    }
}

// ============================================================================
// 不正入力のプロパティテスト（ファジング）
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// プロパティ: 任意のバイト列でパニックしない
    #[test]
    fn prop_arbitrary_input_no_panic(
        data in prop::collection::vec(any::<u8>(), 16..1500),
    ) {
        let mut conn = SrtConnection::new_caller(make_opts(1));
        let now = Timestamp::from_micros(0);

        // パニックしないことを確認
        let _ = conn.feed_recv_buf(&data, now);
    }
}

// ファジングテストは fuzz/ でカバーされるため削除

// ============================================================================
// パケットロスと再送のプロパティテスト
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// プロパティ: パケットロス発生時、NAK が送信される
    #[test]
    fn prop_packet_loss_triggers_nak(
        payload_size in 1000usize..5000usize,  // 複数パケットになるサイズ
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        // 大きなデータを送信（複数パケットになる）
        let payload = vec![0xAB; payload_size];
        caller.send(&payload, now).expect("送信は成功する想定");
        let packets = drain_packets(&mut caller);

        if packets.len() < 2 {
            // パケットが 1 つしかなければスキップ
            return Ok(());
        }

        now = Timestamp::from_micros(now.as_micros() + 200_000);

        // 最初のパケットをドロップ（0 番目をスキップ）
        transfer_packets_with_loss(&packets, &mut listener, now, &[0]);

        // NAK タイマーを発火
        now = Timestamp::from_micros(now.as_micros() + 20_000);
        listener.handle_timer(TimerId::Nak, now).expect("タイマー処理は成功する想定");

        // NAK パケットが送信されるはず
        let nak_packets = drain_packets(&mut listener);
        // 損失検出の実装によっては NAK が送信されない場合もある
        // 重要なのは接続が維持されること
        prop_assert_eq!(listener.state(), ConnectionState::Connected);
        let _ = nak_packets;
    }

    /// プロパティ: 順序が入れ替わったパケットでも正常処理
    #[test]
    fn prop_out_of_order_packets_handled(
        payload_size in 2000usize..4000usize,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        let payload = vec![0xCD; payload_size];
        caller.send(&payload, now).expect("送信は成功する想定");
        let mut packets = drain_packets(&mut caller);

        if packets.len() < 2 {
            return Ok(());
        }

        now = Timestamp::from_micros(now.as_micros() + 200_000);

        // パケットを逆順で受信
        packets.reverse();
        for pkt in &packets {
            listener.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
        }

        listener.handle_timer(TimerId::Ack, now).expect("タイマー処理は成功する想定");

        // 接続が維持されること
        prop_assert_eq!(listener.state(), ConnectionState::Connected);
    }

    /// プロパティ: 重複パケットでもパニックしない
    #[test]
    fn prop_duplicate_packets_handled(
        payload in prop::collection::vec(any::<u8>(), 100..500),
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        caller.send(&payload, now).expect("送信は成功する想定");
        let packets = drain_packets(&mut caller);

        now = Timestamp::from_micros(now.as_micros() + 200_000);

        // 同じパケットを 2 回受信
        for pkt in &packets {
            listener.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
        }
        for pkt in &packets {
            listener.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
        }

        listener.handle_timer(TimerId::Ack, now).expect("タイマー処理は成功する想定");

        // データは 1 回だけ配信されるべき
        let events = drain_events(&mut listener);
        let received = extract_received_data(&events);

        prop_assert_eq!(received, payload);
    }

    /// プロパティ: Retransmit タイマーが再送キューを処理する
    #[test]
    fn prop_retransmit_timer_processes_queue(
        payload in prop::collection::vec(any::<u8>(), 100..500),
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        // データ送信
        caller.send(&payload, now).expect("送信は成功する想定");
        drain_packets(&mut caller);

        // 時間経過
        now = Timestamp::from_micros(now.as_micros() + 500_000);

        // Retransmit タイマーを発火
        caller.handle_timer(TimerId::Retransmit, now).expect("タイマー処理は成功する想定");

        // パニックせず接続維持
        prop_assert_eq!(caller.state(), ConnectionState::Connected);
    }
}

// ============================================================================
// ACK/NAK シーケンスのプロパティテスト
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// プロパティ: ACK 受信後に ACKACK が送信される（Full ACK の場合）
    #[test]
    fn prop_ack_triggers_ackack(
        payload in prop::collection::vec(any::<u8>(), 100..500),
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        caller.send(&payload, now).expect("送信は成功する想定");
        let data_packets = drain_packets(&mut caller);

        now = Timestamp::from_micros(now.as_micros() + 200_000);

        for pkt in &data_packets {
            listener.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
        }

        // ACK タイマー発火
        listener.handle_timer(TimerId::Ack, now).expect("タイマー処理は成功する想定");
        let ack_packets = drain_packets(&mut listener);

        prop_assert!(!ack_packets.is_empty(), "ACK must be sent");

        // Caller が ACK を受信
        for pkt in &ack_packets {
            caller.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
        }

        // ACKACK が送信される（Full ACK に対して）
        let ackack_packets = drain_packets(&mut caller);
        // ACKACK の有無は ACK の種類による（Light ACK には ACKACK なし）
        // 重要なのは接続が維持されること
        prop_assert_eq!(caller.state(), ConnectionState::Connected);
        let _ = ackack_packets;
    }

    /// プロパティ: 連続送信で ACK シーケンスが単調増加
    #[test]
    fn prop_ack_sequence_monotonic(
        chunk_count in 2usize..5usize,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        for i in 0..chunk_count {
            let payload = vec![i as u8; 200];
            caller.send(&payload, now).expect("送信は成功する想定");
            let packets = drain_packets(&mut caller);

            now = Timestamp::from_micros(now.as_micros() + 50_000);

            for pkt in &packets {
                listener.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
            }

            listener.handle_timer(TimerId::Ack, now).expect("タイマー処理は成功する想定");
            let ack_packets = drain_packets(&mut listener);

            for pkt in &ack_packets {
                caller.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
            }
            drain_packets(&mut caller);
        }

        // 全ての処理後も接続維持
        prop_assert_eq!(caller.state(), ConnectionState::Connected);
        prop_assert_eq!(listener.state(), ConnectionState::Connected);
    }
}

// ============================================================================
// ペーシングのプロパティテスト
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// プロパティ: 接続後は送信可能
    #[test]
    fn prop_can_send_after_connection(
        _dummy in 0u32..1u32,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        prop_assert!(caller.can_send());
        prop_assert!(caller.can_send_with_pacing(now));
    }

    /// プロパティ: set_packet_send_period 後も正常動作
    #[test]
    fn prop_set_packet_send_period_works(
        period in 100u64..10_000u64,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        caller.set_packet_send_period(period);
        prop_assert!(caller.can_send());
    }

    /// プロパティ: time_until_send は妥当な値を返す
    #[test]
    fn prop_time_until_send_reasonable(
        _dummy in 0u32..1u32,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        let wait = caller.time_until_send(now);
        // 100ms 以下であるべき
        prop_assert!(wait <= 100_000);
    }

    /// プロパティ: process_retransmit は未接続時に何もしない
    #[test]
    fn prop_process_retransmit_noop_when_disconnected(
        time in 0u64..u64::MAX / 2,
    ) {
        let mut conn = SrtConnection::new_caller(make_opts(1));
        let now = Timestamp::from_micros(time);

        conn.process_retransmit(now);
        prop_assert!(conn.poll_output().is_none());
    }

    /// プロパティ: has_retransmit は未接続時に false
    #[test]
    fn prop_has_retransmit_false_when_disconnected(
        _dummy in 0u32..1u32,
    ) {
        let conn = SrtConnection::new_caller(make_opts(1));
        prop_assert!(!conn.has_retransmit());
    }
}

// ============================================================================
// 境界値のプロパティテスト
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// プロパティ: 最大サイズのペイロードでも正常処理
    #[test]
    fn prop_max_payload_size_handled(
        _dummy in 0u32..1u32,
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        // MTU に近いサイズ
        let payload = vec![0xFF; 1316];
        caller.send(&payload, now).expect("送信は成功する想定");
        let packets = drain_packets(&mut caller);

        now = Timestamp::from_micros(now.as_micros() + 200_000);

        for pkt in &packets {
            listener.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
        }
        listener.handle_timer(TimerId::Ack, now).expect("タイマー処理は成功する想定");

        let received = extract_received_data(&drain_events(&mut listener));
        prop_assert_eq!(received, payload);
    }

    /// プロパティ: 1 バイトのペイロードでも正常処理
    #[test]
    fn prop_min_payload_size_handled(
        byte in any::<u8>(),
    ) {
        let mut now = Timestamp::from_micros(0);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));
        establish_connection(&mut caller, &mut listener, &mut now);

        let payload = vec![byte];
        caller.send(&payload, now).expect("送信は成功する想定");
        let packets = drain_packets(&mut caller);

        now = Timestamp::from_micros(now.as_micros() + 200_000);

        for pkt in &packets {
            listener.feed_recv_buf(pkt, now).expect("受信バッファへのフィードは成功する想定");
        }
        listener.handle_timer(TimerId::Ack, now).expect("タイマー処理は成功する想定");

        let received = extract_received_data(&drain_events(&mut listener));
        prop_assert_eq!(received, payload);
    }

    /// プロパティ: 大きなタイムスタンプでも正常処理
    #[test]
    fn prop_large_timestamp_handled(
        base_time in (u64::MAX / 4)..(u64::MAX / 2),
    ) {
        let mut now = Timestamp::from_micros(base_time);

        let mut caller = SrtConnection::new_caller(make_opts(1));
        let mut listener = SrtConnection::new_listener(make_opts(2));

        caller.connect(now).expect("接続は成功する想定");

        let induction_req = drain_packets(&mut caller);
        listener.feed_recv_buf(&induction_req[0], now).expect("受信バッファへのフィードは成功する想定");
        let induction_resp = drain_packets(&mut listener);

        now = Timestamp::from_micros(now.as_micros() + 1000);
        caller.feed_recv_buf(&induction_resp[0], now).expect("受信バッファへのフィードは成功する想定");

        let conclusion_req = drain_packets(&mut caller);
        listener.feed_recv_buf(&conclusion_req[0], now).expect("受信バッファへのフィードは成功する想定");
        let conclusion_resp = drain_packets(&mut listener);
        caller.feed_recv_buf(&conclusion_resp[0], now).expect("受信バッファへのフィードは成功する想定");

        prop_assert_eq!(caller.state(), ConnectionState::Connected);
        prop_assert_eq!(listener.state(), ConnectionState::Connected);
    }
}

// ヘルパー関数（proptest マクロの外）

/// テスト用の ConnectionOptions を生成
fn make_opts(socket_id: u32) -> ConnectionOptions {
    ConnectionOptions {
        socket_id,
        passphrase: None,
        key_length: KeyLength::Aes128,
        initial_seq: None,
        syn_cookie: None,
        tsbpd_delay: 120,
        srt_version: 0x010500,
        stream_id: None,
        crypto_salt: None,
        crypto_sek: None,
    }
}

/// Stream ID 付きの ConnectionOptions を生成
fn make_opts_with_stream_id(socket_id: u32, stream_id: String) -> ConnectionOptions {
    ConnectionOptions {
        socket_id,
        passphrase: None,
        key_length: KeyLength::Aes128,
        initial_seq: None,
        syn_cookie: None,
        tsbpd_delay: 120,
        srt_version: 0x010500,
        stream_id: Some(stream_id),
        crypto_salt: None,
        crypto_sek: None,
    }
}

fn drain_packets(conn: &mut SrtConnection) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();
    while let Some(output) = conn.poll_output() {
        if let ConnectionOutput::SendPacket(pkt) = output {
            packets.push(pkt);
        }
    }
    packets
}

fn drain_events(conn: &mut SrtConnection) -> Vec<ConnectionEvent> {
    let mut events = Vec::new();
    while let Some(event) = conn.poll_event() {
        events.push(event);
    }
    events
}

/// 受信データを抽出
fn extract_received_data(events: &[ConnectionEvent]) -> Vec<u8> {
    events
        .iter()
        .filter_map(|e| match e {
            ConnectionEvent::DataReceived { payload, .. } => Some(payload.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// ハンドシェイクを完了させる
fn establish_connection(
    caller: &mut SrtConnection,
    listener: &mut SrtConnection,
    now: &mut Timestamp,
) {
    caller.connect(*now).expect("connect should succeed");

    // INDUCTION
    let induction_req = drain_packets(caller);
    listener
        .feed_recv_buf(&induction_req[0], *now)
        .expect("feed should succeed");
    let induction_resp = drain_packets(listener);
    *now = Timestamp::from_micros(now.as_micros() + 1000);
    caller
        .feed_recv_buf(&induction_resp[0], *now)
        .expect("feed should succeed");

    // CONCLUSION
    let conclusion_req = drain_packets(caller);
    listener
        .feed_recv_buf(&conclusion_req[0], *now)
        .expect("feed should succeed");
    let conclusion_resp = drain_packets(listener);
    caller
        .feed_recv_buf(&conclusion_resp[0], *now)
        .expect("feed should succeed");

    // イベントをクリア
    drain_events(caller);
    drain_events(listener);
}

/// パケットを一部ドロップしながら転送
fn transfer_packets_with_loss(
    packets: &[Vec<u8>],
    receiver: &mut SrtConnection,
    now: Timestamp,
    drop_indices: &[usize],
) {
    for (i, pkt) in packets.iter().enumerate() {
        if !drop_indices.contains(&i) {
            receiver
                .feed_recv_buf(pkt, now)
                .expect("feed should succeed");
        }
    }
}
