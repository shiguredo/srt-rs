//! SRT 接続の e2e テスト
//!
//! sansio パターンを活用して、実ソケットなしで Caller/Listener の相互接続をテストする。

use shiguredo_srt::{
    ConnectionEvent, ConnectionOptions, ConnectionOutput, ConnectionState, KeyLength,
    SrtConnection, TimerId, Timestamp,
};

/// テスト用のデフォルトオプション (TSBPD 遅延を 0 にして即時配信)
fn test_options() -> ConnectionOptions {
    ConnectionOptions {
        tsbpd_delay: 0,
        ..Default::default()
    }
}

/// テスト用のタイムスタンプを生成
fn ts(micros: u64) -> Timestamp {
    Timestamp::from_micros(micros)
}

/// Caller の出力パケットを Listener に転送
fn transfer_caller_to_listener(
    caller: &mut SrtConnection,
    listener: &mut SrtConnection,
    now: Timestamp,
) {
    while let Some(output) = caller.poll_output() {
        if let ConnectionOutput::SendPacket(data) = output {
            let _ = listener.feed_recv_buf(&data, now);
        }
    }
}

/// Listener の出力パケットを Caller に転送
fn transfer_listener_to_caller(
    listener: &mut SrtConnection,
    caller: &mut SrtConnection,
    now: Timestamp,
) {
    while let Some(output) = listener.poll_output() {
        if let ConnectionOutput::SendPacket(data) = output {
            let _ = caller.feed_recv_buf(&data, now);
        }
    }
}

/// 双方向でパケットを交換 (1ラウンド)
fn exchange_packets(caller: &mut SrtConnection, listener: &mut SrtConnection, now: Timestamp) {
    transfer_caller_to_listener(caller, listener, now);
    transfer_listener_to_caller(listener, caller, now);
}

/// 接続が確立するまでパケットを交換
fn establish_connection(
    caller: &mut SrtConnection,
    listener: &mut SrtConnection,
) -> Result<(), String> {
    let now = ts(0);
    caller.connect(now).map_err(|e| e.to_string())?;

    // 最大 10 ラウンドで接続確立を試みる
    for i in 0..10 {
        let now = ts(i * 10_000);
        exchange_packets(caller, listener, now);

        if caller.state() == ConnectionState::Connected
            && listener.state() == ConnectionState::Connected
        {
            return Ok(());
        }
    }

    Err(format!(
        "connection not established: caller={:?}, listener={:?}",
        caller.state(),
        listener.state()
    ))
}

/// イベントから Connected を探す
fn find_connected_event(conn: &mut SrtConnection) -> bool {
    while let Some(event) = conn.poll_event() {
        if matches!(event, ConnectionEvent::Connected) {
            return true;
        }
    }
    false
}

/// イベントから受信データを収集
fn collect_received_data(conn: &mut SrtConnection) -> Vec<Vec<u8>> {
    let mut data = Vec::new();
    while let Some(event) = conn.poll_event() {
        if let ConnectionEvent::DataReceived { payload, .. } = event {
            data.push(payload);
        }
    }
    data
}

// ============================================================================
// ハンドシェイクテスト
// ============================================================================

#[test]
fn test_handshake_without_encryption() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    assert_eq!(caller.state(), ConnectionState::Connected);
    assert_eq!(listener.state(), ConnectionState::Connected);

    // Connected イベントが発火していることを確認
    assert!(find_connected_event(&mut caller));
    assert!(find_connected_event(&mut listener));
}

#[test]
fn test_handshake_with_encryption() {
    let passphrase = "test-passphrase".to_string();

    let caller_opts = ConnectionOptions {
        passphrase: Some(passphrase.clone()),
        key_length: KeyLength::Aes128,
        tsbpd_delay: 0,
        ..Default::default()
    };
    let listener_opts = ConnectionOptions {
        passphrase: Some(passphrase),
        key_length: KeyLength::Aes128,
        tsbpd_delay: 0,
        ..Default::default()
    };

    let mut caller = SrtConnection::new_caller(caller_opts);
    let mut listener = SrtConnection::new_listener(listener_opts);

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    assert_eq!(caller.state(), ConnectionState::Connected);
    assert_eq!(listener.state(), ConnectionState::Connected);
}

#[test]
fn test_handshake_with_aes256() {
    let passphrase = "test-passphrase-256".to_string();

    let caller_opts = ConnectionOptions {
        passphrase: Some(passphrase.clone()),
        key_length: KeyLength::Aes256,
        tsbpd_delay: 0,
        ..Default::default()
    };
    let listener_opts = ConnectionOptions {
        passphrase: Some(passphrase),
        key_length: KeyLength::Aes256,
        tsbpd_delay: 0,
        ..Default::default()
    };

    let mut caller = SrtConnection::new_caller(caller_opts);
    let mut listener = SrtConnection::new_listener(listener_opts);

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    assert_eq!(caller.state(), ConnectionState::Connected);
    assert_eq!(listener.state(), ConnectionState::Connected);
}

#[test]
fn test_handshake_with_stream_id() {
    let stream_id = "#!::r=live/stream1".to_string();

    let caller_opts = ConnectionOptions {
        stream_id: Some(stream_id.clone()),
        tsbpd_delay: 0,
        ..Default::default()
    };
    let listener_opts = test_options();

    let mut caller = SrtConnection::new_caller(caller_opts);
    let mut listener = SrtConnection::new_listener(listener_opts);

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // Listener 側で Stream ID を受信できていることを確認
    assert_eq!(listener.peer_stream_id(), Some(stream_id.as_str()));
}

// ============================================================================
// データ送受信テスト
// ============================================================================

#[test]
fn test_data_transfer_caller_to_listener() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // イベントをクリア
    while caller.poll_event().is_some() {}
    while listener.poll_event().is_some() {}

    // Caller からデータ送信
    let test_data = b"Hello, SRT!";
    let now = ts(100_000);
    caller.send(test_data, now).expect("send should succeed");

    // パケット転送
    transfer_caller_to_listener(&mut caller, &mut listener, now);

    // ACK タイマー発火をシミュレート (データ配信のため)
    listener
        .handle_timer(TimerId::Ack, now)
        .expect("timer should succeed");

    // Listener 側でデータ受信を確認
    let received = collect_received_data(&mut listener);
    assert_eq!(received.len(), 1);
    assert_eq!(received[0], test_data);
}

#[test]
fn test_data_transfer_with_encryption() {
    let passphrase = "encryption-test".to_string();

    let caller_opts = ConnectionOptions {
        passphrase: Some(passphrase.clone()),
        key_length: KeyLength::Aes128,
        tsbpd_delay: 0,
        ..Default::default()
    };
    let listener_opts = ConnectionOptions {
        passphrase: Some(passphrase),
        key_length: KeyLength::Aes128,
        tsbpd_delay: 0,
        ..Default::default()
    };

    let mut caller = SrtConnection::new_caller(caller_opts);
    let mut listener = SrtConnection::new_listener(listener_opts);

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // イベントをクリア
    while caller.poll_event().is_some() {}
    while listener.poll_event().is_some() {}

    // 暗号化されたデータ送信
    let test_data = b"Encrypted message!";
    let now = ts(100_000);
    caller.send(test_data, now).expect("send should succeed");

    // パケット転送
    transfer_caller_to_listener(&mut caller, &mut listener, now);

    // ACK タイマー発火
    listener
        .handle_timer(TimerId::Ack, now)
        .expect("timer should succeed");

    // 復号化されたデータを確認
    let received = collect_received_data(&mut listener);
    assert_eq!(received.len(), 1);
    assert_eq!(received[0], test_data);
}

#[test]
fn test_multiple_data_packets() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // イベントをクリア
    while caller.poll_event().is_some() {}
    while listener.poll_event().is_some() {}

    // 複数パケット送信
    let packets: Vec<&[u8]> = vec![b"Packet 1", b"Packet 2", b"Packet 3"];

    for (i, data) in packets.iter().enumerate() {
        let now = ts(100_000 + (i as u64) * 1000);
        caller.send(data, now).expect("send should succeed");
        transfer_caller_to_listener(&mut caller, &mut listener, now);
    }

    // ACK タイマー発火
    let now = ts(200_000);
    listener
        .handle_timer(TimerId::Ack, now)
        .expect("timer should succeed");

    // 全パケット受信を確認
    let received = collect_received_data(&mut listener);
    assert_eq!(received.len(), 3);
    assert_eq!(received[0], b"Packet 1");
    assert_eq!(received[1], b"Packet 2");
    assert_eq!(received[2], b"Packet 3");
}

// ============================================================================
// 双方向通信テスト
// ============================================================================

#[test]
fn test_bidirectional_communication() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // イベントをクリア
    while caller.poll_event().is_some() {}
    while listener.poll_event().is_some() {}

    let now = ts(100_000);

    // Caller → Listener
    caller
        .send(b"From Caller", now)
        .expect("send should succeed");
    transfer_caller_to_listener(&mut caller, &mut listener, now);

    // Listener → Caller
    listener
        .send(b"From Listener", now)
        .expect("send should succeed");
    transfer_listener_to_caller(&mut listener, &mut caller, now);

    // ACK タイマー発火
    caller
        .handle_timer(TimerId::Ack, now)
        .expect("timer should succeed");
    listener
        .handle_timer(TimerId::Ack, now)
        .expect("timer should succeed");

    // 受信確認
    let caller_received = collect_received_data(&mut caller);
    let listener_received = collect_received_data(&mut listener);

    assert_eq!(caller_received.len(), 1);
    assert_eq!(caller_received[0], b"From Listener");

    assert_eq!(listener_received.len(), 1);
    assert_eq!(listener_received[0], b"From Caller");
}

// ============================================================================
// 切断テスト
// ============================================================================

#[test]
fn test_disconnect() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // イベントをクリア
    while caller.poll_event().is_some() {}
    while listener.poll_event().is_some() {}

    // Caller から切断
    let now = ts(100_000);
    caller.disconnect(now);

    // Shutdown パケット転送
    transfer_caller_to_listener(&mut caller, &mut listener, now);

    // Listener が Disconnected イベントを受信
    let mut disconnected = false;
    while let Some(event) = listener.poll_event() {
        if matches!(event, ConnectionEvent::Disconnected { .. }) {
            disconnected = true;
        }
    }
    assert!(disconnected, "listener should receive disconnected event");

    assert_eq!(listener.state(), ConnectionState::Disconnected);
}

// ============================================================================
// エラーケーステスト
// ============================================================================

#[test]
fn test_send_before_connected() {
    let mut caller = SrtConnection::new_caller(test_options());

    // 接続前に送信を試みる
    let result = caller.send(b"test", ts(0));
    assert!(result.is_err());
}

#[test]
fn test_caller_connect_twice() {
    let mut caller = SrtConnection::new_caller(test_options());

    caller.connect(ts(0)).expect("first connect should succeed");

    // 2回目の connect はエラーにならない (状態がすでに変わっているため)
    // ただし Caller 以外で connect を呼ぶとエラー
    let mut listener = SrtConnection::new_listener(test_options());
    let result = listener.connect(ts(0));
    assert!(result.is_err());
}

// ============================================================================
// タイマーテスト
// ============================================================================

#[test]
fn test_keepalive_timer() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // Keepalive タイマー発火
    let now = ts(1_000_000);
    caller
        .handle_timer(TimerId::Keepalive, now)
        .expect("timer should succeed");

    // Keepalive パケットが送信される
    let mut has_packet = false;
    while let Some(output) = caller.poll_output() {
        if matches!(output, ConnectionOutput::SendPacket(_)) {
            has_packet = true;
        }
    }
    assert!(has_packet);
}

#[test]
fn test_nak_timer() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // NAK タイマー発火
    let now = ts(1_000_000);
    listener
        .handle_timer(TimerId::Nak, now)
        .expect("timer should succeed");

    // 接続は維持される
    assert_eq!(listener.state(), ConnectionState::Connected);
}

#[test]
fn test_retransmit_timer() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // Retransmit タイマー発火
    let now = ts(1_000_000);
    caller
        .handle_timer(TimerId::Retransmit, now)
        .expect("timer should succeed");

    // 接続は維持される
    assert_eq!(caller.state(), ConnectionState::Connected);
}

#[test]
fn test_inactivity_timeout() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // イベントをクリア
    while caller.poll_event().is_some() {}

    // 非活性タイムアウト発火
    let now = ts(10_000_000);
    caller
        .handle_timer(TimerId::Inactivity, now)
        .expect("timer should succeed");

    // 切断される
    assert_eq!(caller.state(), ConnectionState::Disconnected);

    // Disconnected イベントが発生
    let mut disconnected = false;
    while let Some(event) = caller.poll_event() {
        if matches!(event, ConnectionEvent::Disconnected { .. }) {
            disconnected = true;
        }
    }
    assert!(disconnected);
}

// ============================================================================
// 統計テスト
// ============================================================================

#[test]
fn test_sender_stats() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // 初期状態の統計
    let stats = caller.sender_stats().expect("stats should be available");
    assert_eq!(stats.packets_in_buffer, 0);
    assert_eq!(stats.total_sent, 0);

    // データ送信
    let now = ts(100_000);
    caller.send(b"test data", now).expect("send should succeed");

    // 送信後の統計
    let stats = caller.sender_stats().expect("stats should be available");
    assert_eq!(stats.packets_in_buffer, 1);
    assert_eq!(stats.total_sent, 1);
}

#[test]
fn test_receiver_stats() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // 初期状態の統計
    let stats = listener
        .receiver_stats()
        .expect("stats should be available");
    assert_eq!(stats.total_received, 0);

    // データ受信
    let now = ts(100_000);
    caller.send(b"test data", now).expect("send should succeed");
    transfer_caller_to_listener(&mut caller, &mut listener, now);

    // 受信後の統計
    let stats = listener
        .receiver_stats()
        .expect("stats should be available");
    assert_eq!(stats.total_received, 1);
}

// ============================================================================
// パケットペーシングテスト
// ============================================================================

#[test]
fn test_packet_pacing() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // パケット送信間隔を設定 (1000μs = 1ms)
    caller.set_packet_send_period(1000);

    // 最初の送信
    let now = ts(100_000);
    caller.send(b"first", now).expect("send should succeed");

    // 直後は送信不可
    assert!(!caller.can_send_with_pacing(ts(100_500)));
    assert!(caller.time_until_send(ts(100_500)) > 0);

    // 1ms 後は送信可能
    assert!(caller.can_send_with_pacing(ts(101_000)));
    assert_eq!(caller.time_until_send(ts(101_000)), 0);
}

// ============================================================================
// ACK/NAK/ACKACK テスト
// ============================================================================

#[test]
fn test_ack_ackack_flow() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // データ送信
    let now = ts(100_000);
    caller.send(b"test data", now).expect("send should succeed");
    transfer_caller_to_listener(&mut caller, &mut listener, now);

    // ACK 生成 (Listener)
    let now = ts(110_000);
    listener
        .handle_timer(TimerId::Ack, now)
        .expect("timer should succeed");

    // ACK を Caller に転送
    transfer_listener_to_caller(&mut listener, &mut caller, now);

    // Caller が ACKACK を送信
    // (送信バッファがクリアされる)
    let stats = caller.sender_stats().expect("stats should be available");
    assert_eq!(stats.packets_in_buffer, 0);
}

#[test]
fn test_has_retransmit() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // 初期状態では再送なし
    assert!(!caller.has_retransmit());

    // データ送信
    let now = ts(100_000);
    caller.send(b"test", now).expect("send should succeed");

    // まだ再送リストは空
    assert!(!caller.has_retransmit());
}

#[test]
fn test_process_retransmit() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // データ送信
    let now = ts(100_000);
    caller.send(b"test", now).expect("send should succeed");

    // 再送処理 (NAK がなければ何もしない)
    caller.process_retransmit(now);

    // パケットはまだバッファにある
    let stats = caller.sender_stats().expect("stats should be available");
    assert_eq!(stats.packets_in_buffer, 1);
}

// ============================================================================
// 追加テスト: より多くのコードパスをカバー
// ============================================================================

#[test]
fn test_can_send() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    // 未接続時は送信不可
    assert!(!caller.can_send());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // 接続後は送信可能
    assert!(caller.can_send());
}

#[test]
fn test_large_data_transfer() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // イベントをクリア
    while caller.poll_event().is_some() {}
    while listener.poll_event().is_some() {}

    // 2KB のデータを送信 (複数パケットに分割される可能性)
    let test_data = vec![0xAB; 2000];
    let now = ts(100_000);
    caller.send(&test_data, now).expect("send should succeed");

    // パケット転送
    transfer_caller_to_listener(&mut caller, &mut listener, now);

    // ACK タイマー発火
    listener
        .handle_timer(TimerId::Ack, now)
        .expect("timer should succeed");

    // データ受信を確認
    let received = collect_received_data(&mut listener);
    assert!(!received.is_empty());

    // 受信データの合計が送信データと一致
    let total_received: Vec<u8> = received.into_iter().flatten().collect();
    assert_eq!(total_received, test_data);
}

#[test]
fn test_receive_encrypted_data_with_aes256() {
    let passphrase = "aes256-encrypt-test".to_string();

    let caller_opts = ConnectionOptions {
        passphrase: Some(passphrase.clone()),
        key_length: KeyLength::Aes256,
        tsbpd_delay: 0,
        ..Default::default()
    };
    let listener_opts = ConnectionOptions {
        passphrase: Some(passphrase),
        key_length: KeyLength::Aes256,
        tsbpd_delay: 0,
        ..Default::default()
    };

    let mut caller = SrtConnection::new_caller(caller_opts);
    let mut listener = SrtConnection::new_listener(listener_opts);

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // イベントをクリア
    while caller.poll_event().is_some() {}
    while listener.poll_event().is_some() {}

    // 暗号化されたデータ送信
    let test_data = b"AES-256 encrypted message!";
    let now = ts(100_000);
    caller.send(test_data, now).expect("send should succeed");

    // パケット転送
    transfer_caller_to_listener(&mut caller, &mut listener, now);

    // ACK タイマー発火
    listener
        .handle_timer(TimerId::Ack, now)
        .expect("timer should succeed");

    // 復号化されたデータを確認
    let received = collect_received_data(&mut listener);
    assert_eq!(received.len(), 1);
    assert_eq!(received[0], test_data);
}

#[test]
fn test_listener_receive_shutdown() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // イベントをクリア
    while listener.poll_event().is_some() {}

    // Listener から切断
    let now = ts(100_000);
    listener.disconnect(now);

    // Shutdown パケット転送
    transfer_listener_to_caller(&mut listener, &mut caller, now);

    // Caller が Disconnected イベントを受信
    let mut disconnected = false;
    while let Some(event) = caller.poll_event() {
        if matches!(event, ConnectionEvent::Disconnected { .. }) {
            disconnected = true;
        }
    }
    assert!(disconnected, "caller should receive disconnected event");
}

#[test]
fn test_multiple_sends_before_transfer() {
    let mut caller = SrtConnection::new_caller(test_options());
    let mut listener = SrtConnection::new_listener(test_options());

    establish_connection(&mut caller, &mut listener).expect("connection should be established");

    // イベントをクリア
    while caller.poll_event().is_some() {}
    while listener.poll_event().is_some() {}

    let now = ts(100_000);

    // 複数のデータを続けて送信
    for i in 0..5 {
        let data = format!("Message {}", i);
        caller
            .send(data.as_bytes(), now)
            .expect("send should succeed");
    }

    // まとめてパケット転送
    transfer_caller_to_listener(&mut caller, &mut listener, now);

    // ACK タイマー発火
    listener
        .handle_timer(TimerId::Ack, now)
        .expect("timer should succeed");

    // 全データ受信を確認
    let received = collect_received_data(&mut listener);
    assert_eq!(received.len(), 5);
}
