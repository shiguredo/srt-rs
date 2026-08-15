//! SRT Connection (sansio パターン)
//!
//! SRT 接続を管理する状態機械。
//! I/O は外部で行い、この構造体はバッファ駆動型で動作する。

use std::collections::VecDeque;

use crate::buf::write_u32;
use crate::crypto::{CryptoContext, KeyFlag, KeyLength};
use crate::error::Error;
use crate::srt_handshake::{
    DEFAULT_FLOW_WINDOW, HandshakePacket, HandshakeState, HandshakeType, KmError, KmMessage,
    srt_flags,
};
use crate::srt_packet::{ControlPacket, ControlType, DataPacket, SRT_HEADER_SIZE, SrtPacket};
use crate::srt_receiver::ReceiverBuffer;
use crate::srt_sender::SenderBuffer;
use crate::time::Timestamp;

/// 接続の役割
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionRole {
    /// Caller (接続を開始する側)
    Caller,
    /// Listener (接続を待ち受ける側)
    Listener,
}

/// 接続状態
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionState {
    /// 切断状態
    #[default]
    Disconnected,
    /// INDUCTION フェーズ (Caller)
    Induction,
    /// CONCLUSION フェーズ
    Conclusion,
    /// 待ち受け中 (Listener)
    Listening,
    /// 接続確立
    Connected,
    /// 切断中
    Closing,
}

/// タイマー ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimerId {
    /// ACK 送信タイマー (10ms)
    Ack,
    /// NAK 送信タイマー
    Nak,
    /// キープアライブタイマー
    Keepalive,
    /// 再送タイムアウト
    Retransmit,
    /// ハンドシェイクタイムアウト
    Handshake,
    /// 非活性タイムアウト (Keep-alive 未受信検出)
    Inactivity,
}

/// 非活性タイムアウト時間 (マイクロ秒)
/// SRT 仕様では通常 5 秒
const INACTIVITY_TIMEOUT_MICROS: u64 = 5_000_000;

/// libsrt 互換ゼロパディング (4 バイト)
///
/// # 背景
///
/// SRT 仕様 (draft-sharabayko-srt) では、Keepalive、ACKACK、Shutdown などの制御パケットは
/// データ部を持たない (0 バイト) と定義されている。
///
/// # libsrt の実装上の問題
///
/// libsrt は全パケットを「ヘッダ部 + データ部」の 2 つの iovec で writev 送信する設計だが、
/// データ部が 0 バイトの場合 writev が正しく動作しない環境がある。
/// そのため、データ部が 0 バイトのパケットに 4 バイトのゼロパディングを追加している。
///
/// ```c
/// // libsrt/srtcore/packet.cpp より
/// case UMSG_KEEPALIVE:
///     // control info field should be none
///     // but "writev" does not allow this
///     m_PacketVector[PV_DATA].set((void*)&m_extra_pad, 4);
///     break;
/// ```
///
/// # Wireshark との互換性
///
/// Wireshark の SRT dissector も libsrt に合わせて実装されているため、
/// 仕様通りの 16 バイトパケットを送ると "Malformed Packet" と表示される。
///
/// # このライブラリでの対応
///
/// libsrt および Wireshark との相互運用性のため、同様の 4 バイトゼロパディングを追加する。
///
/// 対象パケット:
/// - Keepalive (0x0001)
/// - ACKACK (0x0006)
/// - Shutdown (0x0005)
const LIBSRT_COMPAT_PADDING: [u8; 4] = [0, 0, 0, 0];

/// 接続イベント
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionEvent {
    /// 接続完了
    Connected,
    /// データ受信
    DataReceived {
        payload: Vec<u8>,
        message_number: u32,
        timestamp: u32,
    },
    /// 状態変化
    StateChanged(ConnectionState),
    /// エラー発生
    Error(String),
    /// 切断
    Disconnected { reason: String },
    /// キーリフレッシュが必要
    KeyRefreshNeeded { key_length: usize },
}

/// 接続出力アクション
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionOutput {
    /// パケット送信
    SendPacket(Vec<u8>),
    /// タイマー設定
    SetTimer { id: TimerId, duration_micros: u64 },
    /// タイマークリア
    ClearTimer { id: TimerId },
}

/// 接続オプション
#[derive(Debug, Clone)]
pub struct ConnectionOptions {
    /// ローカルソケット ID
    pub socket_id: u32,
    /// 初期シーケンス番号
    pub initial_seq: Option<u32>,
    /// SYN Cookie (Listener 用)
    pub syn_cookie: Option<u32>,
    /// パスフレーズ (暗号化する場合)
    pub passphrase: Option<String>,
    /// 暗号化用の Salt
    pub crypto_salt: Option<[u8; 16]>,
    /// 暗号化用の SEK
    pub crypto_sek: Option<Vec<u8>>,
    /// 鍵長
    pub key_length: KeyLength,
    /// TSBPD 遅延 (ms)
    pub tsbpd_delay: u16,
    /// SRT バージョン
    pub srt_version: u32,
    /// Stream ID (Caller が Listener に送信する識別子、最大 512 バイト)
    pub stream_id: Option<String>,
}

impl Default for ConnectionOptions {
    fn default() -> Self {
        Self {
            socket_id: 0,
            initial_seq: None,
            syn_cookie: None,
            passphrase: None,
            crypto_salt: None,
            crypto_sek: None,
            key_length: KeyLength::Aes128,
            tsbpd_delay: 120,
            srt_version: 0x010500, // 1.5.0
            stream_id: None,
        }
    }
}

/// SRT 接続
pub struct SrtConnection {
    /// 役割
    role: ConnectionRole,
    /// 状態
    state: ConnectionState,
    /// ハンドシェイク状態
    handshake_state: HandshakeState,
    /// オプション
    options: ConnectionOptions,

    /// ピアソケット ID
    peer_socket_id: u32,
    /// SYN Cookie
    syn_cookie: u32,

    /// 初期シーケンス番号
    initial_seq: u32,

    /// 暗号化コンテキスト
    crypto: Option<CryptoContext>,

    /// 送信バッファ
    sender: Option<SenderBuffer>,
    /// 受信バッファ
    receiver: Option<ReceiverBuffer>,

    /// イベントキュー
    event_queue: VecDeque<ConnectionEvent>,
    /// 出力キュー
    output_queue: VecDeque<ConnectionOutput>,

    /// 接続開始時刻
    start_time: Option<Timestamp>,

    /// 最後の ACK 送信時刻
    last_ack_time: Option<Timestamp>,
    /// 最後の NAK 送信時刻
    last_nak_time: Option<Timestamp>,
    /// 最後のパケット受信時刻 (非活性タイムアウト検出用)
    last_recv_time: Option<Timestamp>,
    /// 受信した KM メッセージ (Listener 用)
    received_km: Option<KmMessage>,
    /// ピアから受信した Stream ID (Listener 用)
    peer_stream_id: Option<String>,
}

impl SrtConnection {
    /// Caller として新しい接続を作成
    pub fn new_caller(options: ConnectionOptions) -> Self {
        let initial_seq = options.initial_seq.unwrap_or(0);
        Self {
            role: ConnectionRole::Caller,
            state: ConnectionState::Disconnected,
            handshake_state: HandshakeState::Initial,
            options,
            peer_socket_id: 0,
            syn_cookie: 0,
            initial_seq,
            crypto: None,
            sender: None,
            receiver: None,
            event_queue: VecDeque::new(),
            output_queue: VecDeque::new(),
            start_time: None,
            last_ack_time: None,
            last_nak_time: None,
            last_recv_time: None,
            received_km: None,
            peer_stream_id: None,
        }
    }

    /// Listener として新しい接続を作成
    pub fn new_listener(options: ConnectionOptions) -> Self {
        let initial_seq = options.initial_seq.unwrap_or(0);
        Self {
            role: ConnectionRole::Listener,
            state: ConnectionState::Listening,
            handshake_state: HandshakeState::Initial,
            options,
            peer_socket_id: 0,
            syn_cookie: 0,
            initial_seq,
            crypto: None,
            sender: None,
            receiver: None,
            event_queue: VecDeque::new(),
            output_queue: VecDeque::new(),
            start_time: None,
            last_ack_time: None,
            last_nak_time: None,
            last_recv_time: None,
            received_km: None,
            peer_stream_id: None,
        }
    }

    /// 現在の状態を取得
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// ピアから受信した Stream ID を取得 (Listener 用)
    pub fn peer_stream_id(&self) -> Option<&str> {
        self.peer_stream_id.as_deref()
    }

    /// 接続を開始 (Caller のみ)
    pub fn connect(&mut self, now: Timestamp) -> Result<(), Error> {
        if self.role != ConnectionRole::Caller {
            return Err(Error::invalid_state("only caller can initiate connection"));
        }

        self.start_time = Some(now);
        self.send_induction_request(now);
        self.set_state(ConnectionState::Induction);
        self.handshake_state = HandshakeState::InductionSent;

        // ハンドシェイクタイムアウト設定
        self.output_queue.push_back(ConnectionOutput::SetTimer {
            id: TimerId::Handshake,
            duration_micros: 1_000_000, // 1秒
        });

        Ok(())
    }

    /// 送信/受信バッファを初期化
    fn init_buffers(&mut self, now: Timestamp, peer_initial_seq: u32, tsbpd_time_base: u64) {
        self.sender = Some(SenderBuffer::new(
            self.initial_seq,
            DEFAULT_FLOW_WINDOW,
            self.options.tsbpd_delay,
        ));
        self.receiver = Some(ReceiverBuffer::new(
            peer_initial_seq,
            self.options.tsbpd_delay,
            now,
            tsbpd_time_base,
        ));
        self.last_ack_time = Some(now);
        self.last_nak_time = Some(now);
    }

    /// 受信データを処理
    pub fn feed_recv_buf(&mut self, buf: &[u8], now: Timestamp) -> Result<(), Error> {
        if buf.len() < SRT_HEADER_SIZE {
            return Err(Error::insufficient_buffer());
        }

        // パケット受信時刻を更新 (非活性タイムアウト検出用)
        if self.state == ConnectionState::Connected {
            self.last_recv_time = Some(now);
            // 非活性タイマーをリセット
            self.output_queue.push_back(ConnectionOutput::SetTimer {
                id: TimerId::Inactivity,
                duration_micros: INACTIVITY_TIMEOUT_MICROS,
            });
        }

        let packet = SrtPacket::decode(buf)?;

        match packet {
            SrtPacket::Data(data_pkt) => {
                tracing::debug!("received DATA packet, seq={}", data_pkt.sequence_number);
                self.handle_data_packet(data_pkt, now)
            }
            SrtPacket::Control(ctrl_pkt) => self.handle_control_packet(ctrl_pkt, now),
        }
    }

    /// 再送が必要なパケットがあるか
    pub fn has_retransmit(&self) -> bool {
        self.sender.as_ref().is_some_and(|s| s.has_retransmit())
    }

    /// 再送パケットを取得して送信キューに追加
    pub fn process_retransmit(&mut self, now: Timestamp) {
        if let Some(ref mut sender) = self.sender {
            while let Some(mut packet) = sender.pop_retransmit(now) {
                // 暗号化
                if let Some(ref mut crypto) = self.crypto
                    && let Ok(key_flag) =
                        crypto.encrypt(packet.sequence_number, &mut packet.payload)
                {
                    packet.encryption_flag = key_flag.to_kk_field();
                }

                let mut buf = Vec::new();
                packet.encode(&mut buf);
                self.output_queue
                    .push_back(ConnectionOutput::SendPacket(buf));
            }
        }
    }

    /// タイマーイベントを処理
    pub fn handle_timer(&mut self, timer_id: TimerId, now: Timestamp) -> Result<(), Error> {
        match timer_id {
            TimerId::Handshake => {
                if self.state != ConnectionState::Connected {
                    self.event_queue
                        .push_back(ConnectionEvent::Error("handshake timeout".to_string()));
                    self.set_state(ConnectionState::Disconnected);
                    self.handshake_state = HandshakeState::Failed;
                }
            }
            TimerId::Keepalive => {
                if self.state == ConnectionState::Connected {
                    self.send_keepalive(now);
                    // 次のキープアライブタイマー設定
                    self.output_queue.push_back(ConnectionOutput::SetTimer {
                        id: TimerId::Keepalive,
                        duration_micros: 1_000_000, // 1秒
                    });
                }
            }
            TimerId::Ack => {
                // 定期 ACK 送信
                if self.state == ConnectionState::Connected {
                    self.send_ack(now);

                    // TLPKTDROP: 期限切れパケットを削除
                    if let Some(receiver) = self.receiver.as_mut() {
                        for seq in receiver.drop_too_late(now) {
                            if let Some(sender) = self.sender.as_mut() {
                                sender.handle_ack(seq);
                            }
                        }
                        while let Some(ready_pkt) = receiver.pop_ready(now) {
                            self.event_queue.push_back(ConnectionEvent::DataReceived {
                                payload: ready_pkt.payload,
                                message_number: ready_pkt.message_number,
                                timestamp: ready_pkt.timestamp,
                            });
                        }
                    }

                    if let Some(sender) = self.sender.as_mut() {
                        let _ = sender.drop_expired(now);
                    }

                    // 次の ACK タイマー設定 (10ms)
                    self.output_queue.push_back(ConnectionOutput::SetTimer {
                        id: TimerId::Ack,
                        duration_micros: 10_000,
                    });
                }
            }
            TimerId::Nak => {
                // Periodic NAK 送信
                if self.state == ConnectionState::Connected {
                    self.send_periodic_nak(now);
                    // 次の NAK タイマー設定
                    let interval = self
                        .receiver
                        .as_ref()
                        .map(|r| r.nak_interval())
                        .unwrap_or(20_000);
                    self.output_queue.push_back(ConnectionOutput::SetTimer {
                        id: TimerId::Nak,
                        duration_micros: interval,
                    });
                }
            }
            TimerId::Retransmit => {
                // 再送処理
                if self.state == ConnectionState::Connected {
                    self.process_retransmit(now);
                }
            }
            TimerId::Inactivity => {
                // 非活性タイムアウト: ピアからのパケット受信がない場合に切断
                if self.state == ConnectionState::Connected {
                    self.event_queue.push_back(ConnectionEvent::Disconnected {
                        reason: "inactivity timeout".to_string(),
                    });
                    self.set_state(ConnectionState::Disconnected);
                }
            }
        }
        Ok(())
    }

    /// データを送信
    pub fn send(&mut self, payload: &[u8], now: Timestamp) -> Result<(), Error> {
        if self.state != ConnectionState::Connected {
            return Err(Error::invalid_state("not connected"));
        }

        if !self.can_send() {
            return Err(Error::invalid_state("send buffer full"));
        }

        let timestamp = self.relative_timestamp(now);
        let peer_socket_id = self.peer_socket_id;

        let packet = {
            let sender = self
                .sender
                .as_mut()
                .ok_or_else(|| Error::invalid_state("sender buffer not initialized"))?;
            sender.push(payload.to_vec(), timestamp, peer_socket_id, now)
        };

        if let Some(mut packet) = packet {
            tracing::debug!(
                "sending DATA packet, seq={}, msg={}, ts={}, dest_socket_id={:#x}, payload_len={}",
                packet.sequence_number,
                packet.message_number,
                packet.timestamp,
                packet.dest_socket_id,
                packet.payload.len()
            );

            // 暗号化
            if let Some(ref mut crypto) = self.crypto {
                let key_flag = crypto.encrypt(packet.sequence_number, &mut packet.payload)?;
                packet.encryption_flag = key_flag.to_kk_field();
            }

            let mut buf = Vec::new();
            packet.encode(&mut buf);
            self.output_queue
                .push_back(ConnectionOutput::SendPacket(buf));

            // 送信時刻を記録 (パケットペーシング用)
            if let Some(ref mut sender) = self.sender {
                sender.record_send_time(now);
            }
        }

        // KM Refresh チェック
        self.check_km_refresh(now);

        Ok(())
    }

    /// 送信可能かどうか (ウィンドウサイズのみ)
    pub fn can_send(&self) -> bool {
        self.sender.as_ref().is_some_and(|s| s.can_send())
    }

    /// 送信可能かどうか (パケットペーシングを含む)
    pub fn can_send_with_pacing(&self, now: Timestamp) -> bool {
        self.sender
            .as_ref()
            .is_some_and(|s| s.can_send_with_pacing(now))
    }

    /// 次の送信可能時刻までの待機時間 (マイクロ秒)
    pub fn time_until_send(&self, now: Timestamp) -> u64 {
        self.sender
            .as_ref()
            .map(|s| s.time_until_send(now))
            .unwrap_or(100_000)
    }

    /// パケット送信間隔を設定 (マイクロ秒)
    pub fn set_packet_send_period(&mut self, period: u64) {
        if let Some(ref mut sender) = self.sender {
            sender.set_packet_send_period(period);
        }
    }

    /// イベントを取得
    pub fn poll_event(&mut self) -> Option<ConnectionEvent> {
        self.event_queue.pop_front()
    }

    /// 出力を取得
    pub fn poll_output(&mut self) -> Option<ConnectionOutput> {
        self.output_queue.pop_front()
    }

    /// 切断
    pub fn disconnect(&mut self, now: Timestamp) {
        if self.state == ConnectionState::Connected {
            self.send_shutdown(now);
            self.set_state(ConnectionState::Closing);
        }
    }

    /// 送信側の統計情報を取得
    pub fn sender_stats(&self) -> Option<crate::srt_sender::SenderStats> {
        self.sender.as_ref().map(|s| s.stats())
    }

    /// 受信側の統計情報を取得
    pub fn receiver_stats(&self) -> Option<crate::srt_receiver::ReceiverStats> {
        self.receiver.as_ref().map(|r| r.stats())
    }

    /// 新しい SEK を提供してキーリフレッシュを開始
    ///
    /// `KeyRefreshNeeded` イベントを受信した後に呼び出す。
    pub fn provide_new_sek(&mut self, new_sek: &[u8], now: Timestamp) -> Result<(), Error> {
        let Some(ref mut crypto) = self.crypto else {
            return Err(Error::with_reason(
                crate::error::ErrorKind::CryptoError,
                "encryption not enabled",
            ));
        };

        let (key_flag, wrapped_key) = crypto.start_pre_announce(new_sek)?;
        let km_message = KmMessage::new(key_flag, crypto.key_length(), *crypto.salt(), wrapped_key);
        self.send_km_request(&km_message, now);

        Ok(())
    }

    // ========================================================================
    // プライベートメソッド
    // ========================================================================

    fn set_state(&mut self, new_state: ConnectionState) {
        if self.state != new_state {
            self.state = new_state;
            self.event_queue
                .push_back(ConnectionEvent::StateChanged(new_state));
        }
    }

    fn relative_timestamp(&self, now: Timestamp) -> u32 {
        // start_time が未設定の場合は明示的に 0 を返す。
        // ハンドシェイク中の Listener 側では start_time が未設定のまま呼ばれる。
        // ハンドシェイクパケットのタイムスタンプは INDUCTION リクエストの
        // hsreq_timestamp から TSBPD 時刻基準を計算するため、レスポンス側の
        // タイムスタンプが 0 でも TSBPD の動作に影響しない。
        self.start_time
            .map_or(0, |s| now.as_micros().saturating_sub(s.as_micros())) as u32
    }

    fn handle_data_packet(&mut self, pkt: DataPacket, now: Timestamp) -> Result<(), Error> {
        if self.state != ConnectionState::Connected {
            return Ok(()); // 接続前のデータは無視
        }

        let mut payload = pkt.payload.clone();

        // 復号化
        if pkt.encryption_flag != 0 {
            if let Some(ref crypto) = self.crypto {
                let key_flag = KeyFlag::from_kk_field(pkt.encryption_flag)
                    .ok_or_else(|| Error::crypto_error("invalid KK flag"))?;
                crypto.decrypt(pkt.sequence_number, key_flag, &mut payload)?;
            } else {
                return Err(Error::crypto_error(
                    "encrypted packet but no crypto context",
                ));
            }
        }

        // 受信バッファに追加し、損失と配信可能パケットを取得
        let (losses, should_ack, ready_packets) = {
            let receiver = match self.receiver.as_mut() {
                Some(r) => r,
                None => return Ok(()),
            };

            let mut decrypted_pkt = pkt;
            decrypted_pkt.payload = payload;

            let losses = receiver.receive(decrypted_pkt, now);
            let should_ack = receiver.should_send_ack(now);

            // 配信可能なパケットを収集
            let mut ready_packets = Vec::new();
            while let Some(ready_pkt) = receiver.pop_ready(now) {
                ready_packets.push(ready_pkt);
            }

            (losses, should_ack, ready_packets)
        };

        // 損失が検出された場合、NAK を送信
        if let Some(loss_list) = losses
            && !loss_list.is_empty()
        {
            self.send_nak(&loss_list, now);
        }

        // Light ACK チェック
        if should_ack {
            self.send_ack(now);
        }

        // 配信可能なパケットをイベントキューに追加
        for ready_pkt in ready_packets {
            self.event_queue.push_back(ConnectionEvent::DataReceived {
                payload: ready_pkt.payload,
                message_number: ready_pkt.message_number,
                timestamp: ready_pkt.timestamp,
            });
        }

        Ok(())
    }

    fn handle_control_packet(&mut self, pkt: ControlPacket, now: Timestamp) -> Result<(), Error> {
        tracing::debug!(
            "received control packet, type={:?}, info_len={}",
            pkt.control_type,
            pkt.control_info.len()
        );
        match pkt.control_type {
            ControlType::Handshake => self.handle_handshake(pkt, now),
            ControlType::Keepalive => Ok(()), // キープアライブは特に処理不要
            ControlType::Ack => self.handle_ack(pkt, now),
            ControlType::Nak => self.handle_nak(pkt, now),
            ControlType::Shutdown => self.handle_shutdown(now),
            ControlType::AckAck => self.handle_ackack(pkt, now),
            ControlType::UserDefined => self.handle_user_defined(pkt, now),
            _ => Ok(()), // その他は無視
        }
    }

    fn handle_handshake(&mut self, pkt: ControlPacket, now: Timestamp) -> Result<(), Error> {
        let hs = HandshakePacket::decode(&pkt)?;

        match self.role {
            ConnectionRole::Caller => self.handle_handshake_caller(hs, pkt.timestamp, now),
            ConnectionRole::Listener => self.handle_handshake_listener(hs, pkt.timestamp, now),
        }
    }

    fn handle_handshake_caller(
        &mut self,
        hs: HandshakePacket,
        hsreq_timestamp: u32,
        now: Timestamp,
    ) -> Result<(), Error> {
        match hs.handshake_type {
            HandshakeType::Induction => {
                // INDUCTION レスポンス受信
                if self.handshake_state != HandshakeState::InductionSent {
                    return Ok(());
                }

                self.syn_cookie = hs.syn_cookie;
                self.peer_socket_id = hs.socket_id;
                tracing::debug!(
                    "received INDUCTION response, peer_socket_id={:#x}, syn_cookie={:#x}",
                    self.peer_socket_id,
                    self.syn_cookie
                );

                // 暗号化コンテキスト生成
                if let Some(ref passphrase) = self.options.passphrase {
                    let key_length = hs.key_length().unwrap_or(self.options.key_length);
                    let salt = self.options.crypto_salt.unwrap_or([0u8; 16]);
                    let default_sek = vec![0u8; key_length.len()];
                    let sek = self.options.crypto_sek.as_deref().unwrap_or(&default_sek);
                    self.crypto = Some(CryptoContext::new_sender(
                        passphrase, key_length, salt, sek,
                    )?);
                }

                // CONCLUSION を送信
                self.send_conclusion_request(now);
                self.handshake_state = HandshakeState::ConclusionSent;
            }
            HandshakeType::Conclusion => {
                // CONCLUSION レスポンス受信 → 接続完了
                if self.handshake_state != HandshakeState::ConclusionSent {
                    return Ok(());
                }

                // CONCLUSION レスポンスの socket_id で更新
                // (INDUCTION とは異なる値の場合がある)
                self.peer_socket_id = hs.socket_id;

                tracing::debug!(
                    "received CONCLUSION response, peer_initial_seq={}, peer_socket_id={:#x}",
                    hs.initial_packet_seq,
                    hs.socket_id
                );

                // KMRSP を検証
                if self.crypto.is_some() {
                    match hs.get_km_response() {
                        Ok(Some(_km)) => {
                            // KM レスポンス受信成功 (ピアが鍵を受け入れた)
                        }
                        Ok(None) => {
                            // 暗号化が有効だが KMRSP がない
                            return Err(Error::handshake_rejected(
                                "encryption enabled but no KMRSP",
                            ));
                        }
                        Err(km_error) => {
                            // KM エラー
                            let reason = match km_error {
                                KmError::Unsecured => "peer is unsecured",
                                KmError::NoSecret => "peer has no secret",
                                KmError::BadSecret => "peer has wrong secret",
                                KmError::BadCryptoMode => "incompatible crypto mode",
                            };
                            return Err(Error::handshake_rejected(reason));
                        }
                    }
                }

                self.handshake_state = HandshakeState::Completed;
                self.set_state(ConnectionState::Connected);
                self.start_time = Some(now);

                // TSBPD 時刻基準を計算
                let tsbpd_time_base = now.as_micros().saturating_sub(hsreq_timestamp as u64);

                // バッファ初期化
                self.init_buffers(now, hs.initial_packet_seq, tsbpd_time_base);

                // ハンドシェイクタイマークリア
                self.output_queue.push_back(ConnectionOutput::ClearTimer {
                    id: TimerId::Handshake,
                });

                // タイマー設定
                self.setup_connection_timers();

                self.event_queue.push_back(ConnectionEvent::Connected);
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_handshake_listener(
        &mut self,
        hs: HandshakePacket,
        hsreq_timestamp: u32,
        now: Timestamp,
    ) -> Result<(), Error> {
        match hs.handshake_type {
            HandshakeType::Induction => {
                // INDUCTION リクエスト受信
                self.peer_socket_id = hs.socket_id;
                self.syn_cookie = self.options.syn_cookie.unwrap_or(0);

                // INDUCTION レスポンス送信
                self.send_induction_response(now);
                self.handshake_state = HandshakeState::InductionReceived;
            }
            HandshakeType::Conclusion => {
                // CONCLUSION リクエスト受信
                if self.handshake_state != HandshakeState::InductionReceived {
                    return Ok(());
                }

                // Cookie 検証
                if hs.syn_cookie != self.syn_cookie {
                    return Err(Error::handshake_rejected("invalid SYN cookie"));
                }

                // SRT 仕様 (Conclusion Response): Listener が Caller に優先権を持つのは
                // Cipher Family と Block Size のみ。ISN は Caller の値を採用する。
                self.initial_seq = hs.initial_packet_seq;

                // Stream ID を取得・保存
                if let Some(stream_id) = hs.get_sid_extension() {
                    self.peer_stream_id = Some(stream_id);
                }

                // KMREQ を処理して CryptoContext を作成
                if let Some(ref passphrase) = self.options.passphrase {
                    if let Some(km_result) = hs.get_km_request() {
                        let km = km_result?;
                        self.crypto = Some(CryptoContext::new_receiver(
                            passphrase,
                            km.salt,
                            &km.wrapped_key,
                            km.key_flag,
                            km.key_length,
                        )?);
                        self.received_km = Some(km);
                    } else {
                        // 暗号化が要求されているが KMREQ がない
                        return Err(Error::handshake_rejected(
                            "encryption required but no KMREQ",
                        ));
                    }
                }

                // CONCLUSION レスポンス送信
                self.send_conclusion_response(now);
                self.handshake_state = HandshakeState::Completed;
                self.set_state(ConnectionState::Connected);
                self.start_time = Some(now);

                // TSBPD 時刻基準を計算
                let tsbpd_time_base = now.as_micros().saturating_sub(hsreq_timestamp as u64);

                // バッファ初期化
                self.init_buffers(now, hs.initial_packet_seq, tsbpd_time_base);

                // タイマー設定
                self.setup_connection_timers();

                self.event_queue.push_back(ConnectionEvent::Connected);
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_ack(&mut self, pkt: ControlPacket, now: Timestamp) -> Result<(), Error> {
        if pkt.control_info.len() < 4 {
            return Ok(()); // 不正な ACK
        }

        let mut buf = pkt.control_info.as_slice();
        let ack_seq = crate::buf::read_u32(&mut buf)?;

        tracing::debug!(
            "received ACK, ack_seq={}, type_specific_info={}, control_info_len={}",
            ack_seq,
            pkt.type_specific_info,
            pkt.control_info.len()
        );

        // 送信バッファから ACK されたパケットを削除
        if let Some(ref mut sender) = self.sender {
            let before = sender.packets_in_buffer();
            sender.handle_ack(ack_seq);
            let after = sender.packets_in_buffer();
            tracing::debug!("sender buffer: {} -> {} packets", before, after);
        }

        // Full ACK の場合、ACKACK を送信
        if pkt.control_info.len() >= 16 {
            // Full ACK (RTT, RTTVar, Buffer Size, Rate を含む)
            self.send_ackack(pkt.type_specific_info, now);
        }

        Ok(())
    }

    fn handle_nak(&mut self, pkt: ControlPacket, now: Timestamp) -> Result<(), Error> {
        // NAK パケットから損失リストをパース
        let loss_list = parse_loss_list(&pkt.control_info);

        // 送信バッファに損失を通知
        if let Some(ref mut sender) = self.sender {
            sender.handle_nak(&loss_list);
        }

        // 即座に再送処理
        self.process_retransmit(now);

        Ok(())
    }

    fn handle_ackack(&mut self, pkt: ControlPacket, now: Timestamp) -> Result<(), Error> {
        let ack_number = pkt.type_specific_info;

        // RTT を更新 (ACK 送信時刻はReceiverBuffer内で管理)
        if let Some(ref mut receiver) = self.receiver {
            receiver.handle_ackack(ack_number, now);
        }

        Ok(())
    }

    fn handle_shutdown(&mut self, now: Timestamp) -> Result<(), Error> {
        // 切断前に受信バッファをフラッシュ (TSBPD を無視して即時配信)
        if let Some(receiver) = self.receiver.as_mut() {
            receiver.set_tsbpd_enabled(false);
            while let Some(ready_pkt) = receiver.pop_ready(now) {
                self.event_queue.push_back(ConnectionEvent::DataReceived {
                    payload: ready_pkt.payload,
                    message_number: ready_pkt.message_number,
                    timestamp: ready_pkt.timestamp,
                });
            }
        }

        self.set_state(ConnectionState::Disconnected);
        self.event_queue.push_back(ConnectionEvent::Disconnected {
            reason: "peer shutdown".to_string(),
        });
        Ok(())
    }

    /// UserDefined パケットを処理 (KM Refresh)
    fn handle_user_defined(&mut self, pkt: ControlPacket, now: Timestamp) -> Result<(), Error> {
        // Subtype で KMREQ/KMRSP を判別
        // SRT_CMD_KMREQ = 3, SRT_CMD_KMRSP = 4
        const SRT_CMD_KMREQ: u16 = 3;
        const SRT_CMD_KMRSP: u16 = 4;

        match pkt.subtype {
            SRT_CMD_KMREQ => {
                // KM Refresh リクエストを受信 (受信側)
                let km = KmMessage::decode(&pkt.control_info)?;

                if let Some(ref mut crypto) = self.crypto {
                    // 新しい SEK を更新
                    crypto.update_sek(&km.wrapped_key, km.key_flag)?;

                    // KMRSP を送信
                    self.send_km_response(&km, now);
                }
            }
            SRT_CMD_KMRSP => {
                // KM Refresh レスポンスを受信 (送信側)
                // 正常に受信できれば、相手が新しい鍵を受け入れた
                // 特に処理は不要 (鍵切り替えは送信側のタイミングで行う)
            }
            _ => {
                // 未知の UserDefined パケットは無視
            }
        }

        Ok(())
    }

    /// KM Refresh をチェックして必要な処理を行う
    fn check_km_refresh(&mut self, _now: Timestamp) {
        // 事前通知が必要かチェック
        let Some(ref crypto) = self.crypto else {
            return;
        };

        if crypto.should_pre_announce() {
            // 外部に新しい SEK が必要なことを通知
            self.event_queue
                .push_back(ConnectionEvent::KeyRefreshNeeded {
                    key_length: crypto.key_length().len(),
                });
        }

        // 鍵切り替えが必要かチェック
        if let Some(ref mut crypto) = self.crypto {
            if crypto.should_switch_key() {
                crypto.switch_key();
            }

            // 古い鍵の廃棄が必要かチェック
            if crypto.should_decommission_old_key() {
                crypto.decommission_old_key();
            }
        }
    }

    /// KMREQ パケットを送信 (KM Refresh)
    fn send_km_request(&mut self, km_message: &KmMessage, now: Timestamp) {
        const SRT_CMD_KMREQ: u16 = 3;

        let pkt = ControlPacket {
            control_type: ControlType::UserDefined,
            subtype: SRT_CMD_KMREQ,
            type_specific_info: 0,
            timestamp: self.relative_timestamp(now),
            dest_socket_id: self.peer_socket_id,
            control_info: km_message.encode(),
        };

        let mut buf = Vec::new();
        pkt.encode(&mut buf);
        self.output_queue
            .push_back(ConnectionOutput::SendPacket(buf));
    }

    /// KMRSP パケットを送信 (KM Refresh)
    fn send_km_response(&mut self, km_message: &KmMessage, now: Timestamp) {
        const SRT_CMD_KMRSP: u16 = 4;

        let pkt = ControlPacket {
            control_type: ControlType::UserDefined,
            subtype: SRT_CMD_KMRSP,
            type_specific_info: 0,
            timestamp: self.relative_timestamp(now),
            dest_socket_id: self.peer_socket_id,
            control_info: km_message.encode(),
        };

        let mut buf = Vec::new();
        pkt.encode(&mut buf);
        self.output_queue
            .push_back(ConnectionOutput::SendPacket(buf));
    }

    /// 接続確立後のタイマーを設定
    fn setup_connection_timers(&mut self) {
        // キープアライブタイマー (1秒)
        self.output_queue.push_back(ConnectionOutput::SetTimer {
            id: TimerId::Keepalive,
            duration_micros: 1_000_000,
        });

        // ACK タイマー (10ms)
        self.output_queue.push_back(ConnectionOutput::SetTimer {
            id: TimerId::Ack,
            duration_micros: 10_000,
        });

        // NAK タイマー (初期値 20ms)
        self.output_queue.push_back(ConnectionOutput::SetTimer {
            id: TimerId::Nak,
            duration_micros: 20_000,
        });

        // 非活性タイマー (5秒)
        self.output_queue.push_back(ConnectionOutput::SetTimer {
            id: TimerId::Inactivity,
            duration_micros: INACTIVITY_TIMEOUT_MICROS,
        });
    }

    /// ACK パケットを送信
    fn send_ack(&mut self, now: Timestamp) {
        let receiver = match self.receiver.as_mut() {
            Some(r) => r,
            None => return,
        };

        let ack_info = receiver.generate_ack(now);
        self.last_ack_time = Some(now);

        let mut control_info = Vec::new();
        write_u32(&mut control_info, ack_info.ack_seq);

        if !ack_info.is_light {
            // Full ACK (SRT 仕様に準拠)
            write_u32(&mut control_info, ack_info.rtt);
            write_u32(&mut control_info, ack_info.rtt_var);
            write_u32(&mut control_info, ack_info.available_buffer);
            write_u32(&mut control_info, ack_info.receiving_rate); // packets/sec
            write_u32(&mut control_info, ack_info.link_capacity); // packets/sec
            write_u32(&mut control_info, ack_info.recv_rate); // bytes/sec
        }

        let pkt = ControlPacket {
            control_type: ControlType::Ack,
            subtype: 0,
            type_specific_info: if ack_info.is_light {
                0
            } else {
                receiver.ack_number()
            },
            timestamp: self.relative_timestamp(now),
            dest_socket_id: self.peer_socket_id,
            control_info,
        };

        let mut buf = Vec::new();
        pkt.encode(&mut buf);
        self.output_queue
            .push_back(ConnectionOutput::SendPacket(buf));
    }

    /// NAK パケットを送信
    fn send_nak(&mut self, loss_list: &[u32], now: Timestamp) {
        if loss_list.is_empty() {
            return;
        }

        let control_info = encode_loss_list(loss_list);

        let pkt = ControlPacket {
            control_type: ControlType::Nak,
            subtype: 0,
            type_specific_info: 0,
            timestamp: self.relative_timestamp(now),
            dest_socket_id: self.peer_socket_id,
            control_info,
        };

        let mut buf = Vec::new();
        pkt.encode(&mut buf);
        self.output_queue
            .push_back(ConnectionOutput::SendPacket(buf));
    }

    /// Periodic NAK を送信
    fn send_periodic_nak(&mut self, now: Timestamp) {
        let receiver = match self.receiver.as_mut() {
            Some(r) => r,
            None => return,
        };

        if let Some(nak) = receiver.generate_periodic_nak() {
            self.send_nak(&nak.loss_list, now);
        }
        self.last_nak_time = Some(now);
    }

    /// ACKACK パケットを送信
    ///
    /// ACKACK は ACK に対する確認応答で、RTT 計算に使用される。
    /// SRT 仕様ではデータ部 0 バイトの 16 バイトパケットだが、
    /// libsrt 互換のため 4 バイトゼロパディングを追加して 20 バイトで送信する。
    /// 詳細は [`LIBSRT_COMPAT_PADDING`] を参照。
    fn send_ackack(&mut self, ack_number: u32, now: Timestamp) {
        let pkt = ControlPacket {
            control_type: ControlType::AckAck,
            subtype: 0,
            type_specific_info: ack_number,
            timestamp: self.relative_timestamp(now),
            dest_socket_id: self.peer_socket_id,
            // libsrt 互換: データ部 0 バイト → 4 バイトゼロパディング
            control_info: LIBSRT_COMPAT_PADDING.to_vec(),
        };

        let mut buf = Vec::new();
        pkt.encode(&mut buf);
        self.output_queue
            .push_back(ConnectionOutput::SendPacket(buf));
    }

    fn send_induction_request(&mut self, now: Timestamp) {
        let hs = HandshakePacket::new_induction_request(self.options.socket_id);
        let pkt = hs.encode(self.relative_timestamp(now), 0);
        let mut buf = Vec::new();
        pkt.encode(&mut buf);
        self.output_queue
            .push_back(ConnectionOutput::SendPacket(buf));
    }

    fn send_induction_response(&mut self, now: Timestamp) {
        let encryption_field = if self.options.passphrase.is_some() {
            self.options.key_length.to_encryption_field()
        } else {
            0
        };

        let hs = HandshakePacket::new_induction_response(
            self.options.socket_id,
            self.syn_cookie,
            encryption_field,
        );
        let pkt = hs.encode(self.relative_timestamp(now), self.peer_socket_id);
        let mut buf = Vec::new();
        pkt.encode(&mut buf);
        self.output_queue
            .push_back(ConnectionOutput::SendPacket(buf));
    }

    fn send_conclusion_request(&mut self, now: Timestamp) {
        let encryption_field = if self.options.passphrase.is_some() {
            self.options.key_length.to_encryption_field()
        } else {
            0
        };

        let has_encryption = self.options.passphrase.is_some();
        tracing::debug!(
            "sending CONCLUSION request, our_initial_seq={}, socket_id={:#x}, syn_cookie={:#x}",
            self.initial_seq,
            self.options.socket_id,
            self.syn_cookie
        );
        let mut hs = HandshakePacket::new_conclusion_request(
            self.options.socket_id,
            self.syn_cookie,
            self.initial_seq,
            encryption_field,
            has_encryption,
        );

        // SRT フラグ
        // CRYPT と REXMITFLG は常に設定 (レガシー互換性フラグ)
        // STREAM フラグは設定しない (Message モードを使用)
        let flags = srt_flags::TSBPDSND
            | srt_flags::TSBPDRCV
            | srt_flags::CRYPT
            | srt_flags::TLPKTDROP
            | srt_flags::PERIODICNAK
            | srt_flags::REXMITFLG;

        hs.add_hs_extension(self.options.srt_version, flags, self.options.tsbpd_delay);

        // 暗号化が有効な場合、KMREQ を追加
        if let Some(ref crypto) = self.crypto
            && let Ok(wrapped_key) = crypto.wrap_sek(crypto.current_key())
        {
            let km_message = KmMessage::new(
                crypto.current_key(),
                crypto.key_length(),
                *crypto.salt(),
                wrapped_key,
            );
            hs.add_km_request(&km_message);
        }

        // Stream ID が設定されている場合、SID 拡張を追加
        if let Some(ref stream_id) = self.options.stream_id {
            hs.add_sid_extension(stream_id);
        }

        // CONCLUSION リクエストは dest_socket_id = 0 で送信 (libsrt 互換)
        let pkt = hs.encode(self.relative_timestamp(now), 0);
        let mut buf = Vec::new();
        pkt.encode(&mut buf);
        self.output_queue
            .push_back(ConnectionOutput::SendPacket(buf));
    }

    fn send_conclusion_response(&mut self, now: Timestamp) {
        let encryption_field = if self.options.passphrase.is_some() {
            self.options.key_length.to_encryption_field()
        } else {
            0
        };

        let has_encryption = self.options.passphrase.is_some();
        let mut hs = HandshakePacket::new_conclusion_response(
            self.options.socket_id,
            self.syn_cookie,
            self.initial_seq,
            encryption_field,
            has_encryption,
        );

        // SRT フラグ
        // CRYPT と REXMITFLG は常に設定 (レガシー互換性フラグ)
        // STREAM フラグは設定しない (Message モードを使用)
        let flags = srt_flags::TSBPDSND
            | srt_flags::TSBPDRCV
            | srt_flags::CRYPT
            | srt_flags::TLPKTDROP
            | srt_flags::PERIODICNAK
            | srt_flags::REXMITFLG;

        hs.add_hs_response(self.options.srt_version, flags, self.options.tsbpd_delay);

        // 受信した KMREQ をそのまま KMRSP として返す
        if let Some(ref km) = self.received_km {
            hs.add_km_response(km);
        }

        let pkt = hs.encode(self.relative_timestamp(now), self.peer_socket_id);
        let mut buf = Vec::new();
        pkt.encode(&mut buf);
        self.output_queue
            .push_back(ConnectionOutput::SendPacket(buf));
    }

    /// Keepalive パケットを送信
    ///
    /// Keepalive は接続の生存確認に使用される。一定時間データの送受信がない場合に送信され、
    /// 相手側は Keepalive を受信することで接続がまだ有効であることを確認できる。
    /// SRT 仕様ではデータ部 0 バイトの 16 バイトパケットだが、
    /// libsrt 互換のため 4 バイトゼロパディングを追加して 20 バイトで送信する。
    /// 詳細は [`LIBSRT_COMPAT_PADDING`] を参照。
    fn send_keepalive(&mut self, now: Timestamp) {
        let pkt = ControlPacket {
            control_type: ControlType::Keepalive,
            subtype: 0,
            type_specific_info: 0,
            timestamp: self.relative_timestamp(now),
            dest_socket_id: self.peer_socket_id,
            // libsrt 互換: データ部 0 バイト → 4 バイトゼロパディング
            control_info: LIBSRT_COMPAT_PADDING.to_vec(),
        };
        let mut buf = Vec::new();
        pkt.encode(&mut buf);
        self.output_queue
            .push_back(ConnectionOutput::SendPacket(buf));
    }

    /// Shutdown パケットを送信
    ///
    /// Shutdown は接続の正常終了を通知する。このパケットを送信後、接続は切断状態に遷移する。
    /// SRT 仕様ではデータ部 0 バイトの 16 バイトパケットだが、
    /// libsrt 互換のため 4 バイトゼロパディングを追加して 20 バイトで送信する。
    /// 詳細は [`LIBSRT_COMPAT_PADDING`] を参照。
    fn send_shutdown(&mut self, now: Timestamp) {
        let pkt = ControlPacket {
            control_type: ControlType::Shutdown,
            subtype: 0,
            type_specific_info: 0,
            timestamp: self.relative_timestamp(now),
            dest_socket_id: self.peer_socket_id,
            // libsrt 互換: データ部 0 バイト → 4 バイトゼロパディング
            control_info: LIBSRT_COMPAT_PADDING.to_vec(),
        };
        let mut buf = Vec::new();
        pkt.encode(&mut buf);
        self.output_queue
            .push_back(ConnectionOutput::SendPacket(buf));
    }
}

/// 損失リストをパース (NAK パケットの control_info から)
fn parse_loss_list(data: &[u8]) -> Vec<u32> {
    let mut result = Vec::new();
    let mut slice = data;

    while slice.len() >= 4 {
        let word = match crate::buf::read_u32(&mut slice) {
            Ok(w) => w,
            Err(_) => break,
        };

        if word & 0x8000_0000 != 0 {
            // Range: [word & 0x7FFF_FFFF, next_word]
            if slice.len() >= 4 {
                let start = word & 0x7FFF_FFFF;
                let end = match crate::buf::read_u32(&mut slice) {
                    Ok(w) => w & 0x7FFF_FFFF,
                    Err(_) => break,
                };
                let mut seq = start;
                while seq != end.wrapping_add(1) & 0x7FFF_FFFF {
                    result.push(seq);
                    seq = seq.wrapping_add(1) & 0x7FFF_FFFF;
                    // 安全のため上限を設ける
                    if result.len() > 1000 {
                        break;
                    }
                }
            }
        } else {
            // Single sequence number
            result.push(word);
        }
    }

    result
}

/// 損失リストをエンコード (NAK パケットの control_info 用)
/// 連続するシーケンス番号は範囲としてエンコードして圧縮する
fn encode_loss_list(loss_list: &[u32]) -> Vec<u8> {
    let mut result = Vec::new();

    if loss_list.is_empty() {
        return result;
    }

    // 連続するシーケンス番号を範囲として検出
    let mut i = 0;
    while i < loss_list.len() {
        let start = loss_list[i];
        let mut end = start;

        // 連続するシーケンス番号を探す
        while i + 1 < loss_list.len() {
            let next = loss_list[i + 1];
            // シーケンス番号のラップアラウンドを考慮した連続判定
            let expected_next = end.wrapping_add(1) & 0x7FFF_FFFF;
            if next == expected_next {
                end = next;
                i += 1;
            } else {
                break;
            }
        }

        if start == end {
            // 単一のシーケンス番号
            write_u32(&mut result, start & 0x7FFF_FFFF);
        } else {
            // 範囲: 2つ以上連続する場合は範囲エンコード
            // 最初の word は MSB を 1 に設定
            write_u32(&mut result, (start & 0x7FFF_FFFF) | 0x8000_0000);
            write_u32(&mut result, end & 0x7FFF_FFFF);
        }

        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_options_default() {
        let opts = ConnectionOptions::default();
        // Sans I/O 化により socket_id は外部から設定する必要がある
        assert_eq!(opts.socket_id, 0);
        assert!(opts.passphrase.is_none());
        assert!(opts.crypto_salt.is_none());
        assert!(opts.crypto_sek.is_none());
        assert_eq!(opts.key_length, KeyLength::Aes128);
    }

    #[test]
    fn test_caller_initial_state() {
        let conn = SrtConnection::new_caller(ConnectionOptions::default());
        assert_eq!(conn.state(), ConnectionState::Disconnected);
        assert_eq!(conn.role, ConnectionRole::Caller);
    }

    #[test]
    fn test_listener_initial_state() {
        let conn = SrtConnection::new_listener(ConnectionOptions::default());
        assert_eq!(conn.state(), ConnectionState::Listening);
        assert_eq!(conn.role, ConnectionRole::Listener);
    }

    #[test]
    fn test_loss_list_encode_decode_single() {
        // 単一のシーケンス番号
        let loss_list = vec![100, 200, 300];
        let encoded = encode_loss_list(&loss_list);
        let decoded = parse_loss_list(&encoded);
        assert_eq!(decoded, loss_list);
    }

    #[test]
    fn test_loss_list_encode_decode_range() {
        // 連続するシーケンス番号は範囲としてエンコードされる
        let loss_list = vec![100, 101, 102, 103, 200, 201];
        let encoded = encode_loss_list(&loss_list);
        let decoded = parse_loss_list(&encoded);
        assert_eq!(decoded, loss_list);
        // 範囲エンコードにより元の 6*4=24 バイトが 3*4=12 バイトに圧縮
        // (100-103 が 8 バイト、200-201 が 8 バイト = 16 バイト)
        assert_eq!(encoded.len(), 16);
    }

    #[test]
    fn test_loss_list_encode_decode_mixed() {
        // 単一と連続の混合
        let loss_list = vec![50, 100, 101, 102, 200];
        let encoded = encode_loss_list(&loss_list);
        let decoded = parse_loss_list(&encoded);
        assert_eq!(decoded, loss_list);
    }

    #[test]
    fn test_loss_list_encode_empty() {
        let loss_list: Vec<u32> = vec![];
        let encoded = encode_loss_list(&loss_list);
        assert!(encoded.is_empty());
    }
}
