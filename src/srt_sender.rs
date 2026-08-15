//! SRT 送信バッファ
//!
//! 送信パケットの保持と再送を管理する。
//!
//! ## 機能
//!
//! - 送信パケットのバッファリング (ACK 受信まで保持)
//! - NAK による再送キュー管理
//! - ACK によるバッファ解放
//! - 送信ウィンドウ管理

use std::collections::{BTreeMap, VecDeque};

use crate::srt_packet::{DataPacket, PacketPosition, sequence_less_than};
use crate::time::Timestamp;

/// 送信パケットエントリ
#[derive(Debug, Clone)]
struct SentPacket {
    /// パケットデータ
    packet: DataPacket,
    /// 送信時刻
    sent_time: Timestamp,
    /// 再送回数
    retransmit_count: u32,
}

/// 送信バッファ
#[derive(Debug)]
pub struct SenderBuffer {
    /// 送信済みパケット (sequence_number -> SentPacket)
    packets: BTreeMap<u32, SentPacket>,

    /// 損失リスト (NAK で報告されたパケット)
    loss_list: VecDeque<u32>,

    /// 最古の未 ACK シーケンス番号
    oldest_unacked: u32,

    /// 次の送信シーケンス番号
    next_seq: u32,

    /// 次のメッセージ番号
    next_msg: u32,

    /// フローウィンドウサイズ
    flow_window: u32,

    /// 輻輳ウィンドウサイズ
    congestion_window: u32,

    /// バッファ最大サイズ (パケット数)
    #[expect(dead_code)]
    max_buffer_size: u32,

    /// レイテンシ (マイクロ秒)
    latency_us: u64,
    /// パケット送信間隔 (マイクロ秒)
    packet_send_period: u64,
    /// 最後のパケット送信時刻
    last_send_time: Option<Timestamp>,
    /// 送信パケット総数
    total_sent: u64,
    /// 送信バイト総数
    total_bytes_sent: u64,
}

impl SenderBuffer {
    /// 新しい送信バッファを作成
    pub fn new(initial_seq: u32, flow_window: u32, latency_ms: u16) -> Self {
        Self {
            packets: BTreeMap::new(),
            loss_list: VecDeque::new(),
            oldest_unacked: initial_seq,
            next_seq: initial_seq,
            next_msg: 1,
            flow_window,
            congestion_window: 16, // 初期値
            max_buffer_size: 8192,
            latency_us: latency_ms as u64 * 1000,
            packet_send_period: 0, // 初期値は制限なし
            last_send_time: None,
            total_sent: 0,
            total_bytes_sent: 0,
        }
    }

    /// 次のシーケンス番号を取得
    pub fn next_sequence_number(&self) -> u32 {
        self.next_seq
    }

    /// 次のメッセージ番号を取得
    pub fn next_message_number(&self) -> u32 {
        self.next_msg
    }

    /// 送信可能かどうか (ウィンドウサイズのみチェック)
    pub fn can_send(&self) -> bool {
        let in_flight = self.packets_in_flight();
        in_flight < self.flow_window && in_flight < self.congestion_window
    }

    /// 送信可能かどうか (パケットペーシングを含む)
    pub fn can_send_with_pacing(&self, now: Timestamp) -> bool {
        if !self.can_send() {
            return false;
        }

        // パケットペーシングチェック
        if self.packet_send_period > 0
            && let Some(last_time) = self.last_send_time
        {
            let elapsed = now.as_micros().saturating_sub(last_time.as_micros());
            if elapsed < self.packet_send_period {
                return false;
            }
        }

        true
    }

    /// 次の送信可能時刻までの待機時間 (マイクロ秒)
    ///
    /// 即座に送信可能な場合は 0 を返す
    pub fn time_until_send(&self, now: Timestamp) -> u64 {
        if !self.can_send() {
            // バッファが満杯の場合は長めの待機時間を返す
            return 100_000; // 100ms
        }

        if self.packet_send_period == 0 {
            return 0;
        }

        if let Some(last_time) = self.last_send_time {
            let elapsed = now.as_micros().saturating_sub(last_time.as_micros());
            if elapsed < self.packet_send_period {
                return self.packet_send_period - elapsed;
            }
        }

        0
    }

    /// パケット送信間隔を設定 (マイクロ秒)
    pub fn set_packet_send_period(&mut self, period: u64) {
        self.packet_send_period = period;
    }

    /// 送信時刻を記録
    pub fn record_send_time(&mut self, now: Timestamp) {
        self.last_send_time = Some(now);
    }

    /// 送信中のパケット数
    pub fn packets_in_flight(&self) -> u32 {
        self.packets.len() as u32
    }

    /// バッファ内のパケット数
    pub fn packets_in_buffer(&self) -> usize {
        self.packets.len()
    }

    /// バッファが空かどうか
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    /// 再送が必要なパケットがあるか
    pub fn has_retransmit(&self) -> bool {
        !self.loss_list.is_empty()
    }

    /// 輻輳ウィンドウを設定
    pub fn set_congestion_window(&mut self, cwnd: u32) {
        self.congestion_window = cwnd;
    }

    /// フローウィンドウを設定
    pub fn set_flow_window(&mut self, flow_window: u32) {
        self.flow_window = flow_window;
    }

    /// ペイロードをバッファに追加して送信パケットを生成
    pub fn push(
        &mut self,
        payload: Vec<u8>,
        timestamp: u32,
        dest_socket_id: u32,
        now: Timestamp,
    ) -> Option<DataPacket> {
        if !self.can_send() {
            return None;
        }

        let packet = DataPacket {
            sequence_number: self.next_seq,
            position: PacketPosition::Single,
            order_flag: false,
            encryption_flag: 0,
            retransmitted: false,
            message_number: self.next_msg,
            timestamp,
            dest_socket_id,
            payload,
        };

        // バッファに保存
        self.packets.insert(
            self.next_seq,
            SentPacket {
                packet: packet.clone(),
                sent_time: now,
                retransmit_count: 0,
            },
        );

        // 統計を更新
        self.total_sent += 1;
        self.total_bytes_sent += packet.payload.len() as u64;

        // シーケンス番号とメッセージ番号を進める
        self.next_seq = self.next_seq.wrapping_add(1) & 0x7FFF_FFFF;
        self.next_msg = self.next_msg.wrapping_add(1) & 0x03FF_FFFF;

        Some(packet)
    }

    /// 大きなメッセージを分割して送信
    pub fn push_message(
        &mut self,
        payload: &[u8],
        max_payload_size: usize,
        timestamp: u32,
        dest_socket_id: u32,
        now: Timestamp,
    ) -> Vec<DataPacket> {
        let mut packets = Vec::new();
        let chunks: Vec<&[u8]> = payload.chunks(max_payload_size).collect();
        let total_chunks = chunks.len();

        for (i, chunk) in chunks.into_iter().enumerate() {
            if !self.can_send() {
                break;
            }

            let position = match (i, total_chunks) {
                (0, 1) => PacketPosition::Single,
                (0, _) => PacketPosition::First,
                (n, total) if n == total - 1 => PacketPosition::Last,
                _ => PacketPosition::Middle,
            };

            let packet = DataPacket {
                sequence_number: self.next_seq,
                position,
                order_flag: true, // 順序付きメッセージ
                encryption_flag: 0,
                retransmitted: false,
                message_number: self.next_msg,
                timestamp,
                dest_socket_id,
                payload: chunk.to_vec(),
            };

            self.packets.insert(
                self.next_seq,
                SentPacket {
                    packet: packet.clone(),
                    sent_time: now,
                    retransmit_count: 0,
                },
            );

            // 統計を更新
            self.total_sent += 1;
            self.total_bytes_sent += packet.payload.len() as u64;

            self.next_seq = self.next_seq.wrapping_add(1) & 0x7FFF_FFFF;
            packets.push(packet);
        }

        // メッセージ番号は次のメッセージで進める
        if !packets.is_empty() {
            self.next_msg = self.next_msg.wrapping_add(1) & 0x03FF_FFFF;
        }

        packets
    }

    /// 再送パケットを取得
    pub fn pop_retransmit(&mut self, now: Timestamp) -> Option<DataPacket> {
        while let Some(seq) = self.loss_list.pop_front() {
            if let Some(entry) = self.packets.get_mut(&seq) {
                entry.retransmit_count += 1;
                entry.sent_time = now;

                let mut packet = entry.packet.clone();
                packet.retransmitted = true;
                return Some(packet);
            }
            // パケットが既に ACK されている場合はスキップ
        }
        None
    }

    /// ACK を処理してバッファを解放
    ///
    /// `ack_seq` は次に期待するシーケンス番号 (この番号未満は全て ACK)
    pub fn handle_ack(&mut self, ack_seq: u32) {
        // ack_seq より小さいシーケンス番号のパケットを全て削除
        let to_remove: Vec<u32> = self
            .packets
            .keys()
            .copied()
            .filter(|&seq| sequence_less_than(seq, ack_seq))
            .collect();

        for seq in to_remove {
            self.packets.remove(&seq);
        }

        // 損失リストからも削除
        self.loss_list
            .retain(|&seq| !sequence_less_than(seq, ack_seq));

        // oldest_unacked を更新
        if sequence_less_than(self.oldest_unacked, ack_seq) {
            self.oldest_unacked = ack_seq;
        }
    }

    /// NAK を処理して損失リストに追加
    pub fn handle_nak(&mut self, lost_sequences: &[u32]) {
        for &seq in lost_sequences {
            // バッファに存在するパケットのみ追加
            if self.packets.contains_key(&seq) && !self.loss_list.contains(&seq) {
                self.loss_list.push_back(seq);
            }
        }
    }

    /// 期限切れパケットを削除 (TLPKTDROP)
    pub fn drop_expired(&mut self, now: Timestamp) -> Vec<u32> {
        let mut dropped = Vec::new();

        // TLPKTDROP 閾値: SRT latency の 1.25 倍、最低 1 秒
        // 仕様 (draft-sharabayko-srt.md の #too-late-packet-drop 節) の推奨値に従う。
        let threshold = (self.latency_us * 125 / 100).max(1_000_000);

        let expired: Vec<u32> = self
            .packets
            .iter()
            .filter_map(|(&seq, entry)| {
                let elapsed = now.as_micros().saturating_sub(entry.sent_time.as_micros());
                if elapsed > threshold { Some(seq) } else { None }
            })
            .collect();

        for seq in expired {
            self.packets.remove(&seq);
            dropped.push(seq);
        }

        // 損失リストからも削除
        self.loss_list.retain(|seq| !dropped.contains(seq));

        dropped
    }

    /// バッファ内の最古のパケット送信時刻を取得
    pub fn oldest_packet_time(&self) -> Option<Timestamp> {
        self.packets.values().next().map(|e| e.sent_time)
    }

    /// 統計情報を取得
    pub fn stats(&self) -> SenderStats {
        let total_retransmits: u32 = self.packets.values().map(|e| e.retransmit_count).sum();

        // 再送回数別カウント
        let mut retransmits_once = 0u32;
        let mut retransmits_twice = 0u32;
        let mut retransmits_many = 0u32;
        for entry in self.packets.values() {
            match entry.retransmit_count {
                1 => retransmits_once += 1,
                2 => retransmits_twice += 1,
                n if n >= 3 => retransmits_many += 1,
                _ => {}
            }
        }

        SenderStats {
            packets_in_buffer: self.packets.len() as u32,
            packets_in_loss_list: self.loss_list.len() as u32,
            total_retransmits,
            total_sent: self.total_sent,
            total_bytes_sent: self.total_bytes_sent,
            retransmits_once,
            retransmits_twice,
            retransmits_many,
        }
    }
}

/// 送信統計
#[derive(Debug, Clone, Copy, Default)]
pub struct SenderStats {
    /// バッファ内のパケット数
    pub packets_in_buffer: u32,
    /// 損失リストのパケット数
    pub packets_in_loss_list: u32,
    /// 再送回数の合計
    pub total_retransmits: u32,
    /// 送信パケット総数
    pub total_sent: u64,
    /// 送信バイト総数
    pub total_bytes_sent: u64,
    /// 1 回再送されたパケット数
    pub retransmits_once: u32,
    /// 2 回再送されたパケット数
    pub retransmits_twice: u32,
    /// 3 回以上再送されたパケット数
    pub retransmits_many: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sender_buffer_new() {
        let buf = SenderBuffer::new(1000, 8192, 120);
        assert_eq!(buf.next_sequence_number(), 1000);
        assert!(buf.can_send());
        assert!(buf.is_empty());
    }

    #[test]
    fn test_sender_buffer_push() {
        let mut buf = SenderBuffer::new(1000, 8192, 120);
        let now = Timestamp::from_micros(0);

        let packet = buf.push(vec![1, 2, 3], 100, 12345, now);
        assert!(packet.is_some());
        let pkt = packet.expect("送信パケットは Some になる想定");
        assert_eq!(pkt.sequence_number, 1000);
        assert_eq!(buf.next_sequence_number(), 1001);
        assert_eq!(buf.packets_in_flight(), 1);
    }

    #[test]
    fn test_sender_buffer_ack() {
        let mut buf = SenderBuffer::new(1000, 8192, 120);
        let now = Timestamp::from_micros(0);

        // 3 パケット送信
        buf.push(vec![1], 100, 1, now);
        buf.push(vec![2], 100, 1, now);
        buf.push(vec![3], 100, 1, now);

        assert_eq!(buf.packets_in_flight(), 3);

        // ACK 1002 = パケット 1000, 1001 を ACK
        buf.handle_ack(1002);
        assert_eq!(buf.packets_in_flight(), 1);
    }

    #[test]
    fn test_sender_buffer_nak() {
        let mut buf = SenderBuffer::new(1000, 8192, 120);
        let now = Timestamp::from_micros(0);

        buf.push(vec![1], 100, 1, now);
        buf.push(vec![2], 100, 1, now);
        buf.push(vec![3], 100, 1, now);

        // パケット 1001 を損失報告
        buf.handle_nak(&[1001]);
        assert!(buf.has_retransmit());

        // 再送パケットを取得
        let retransmit = buf.pop_retransmit(now);
        assert!(retransmit.is_some());
        let pkt = retransmit.expect("再送パケットは Some になる想定");
        assert_eq!(pkt.sequence_number, 1001);
        assert!(pkt.retransmitted);
    }

    #[test]
    fn test_sequence_less_than() {
        assert!(sequence_less_than(100, 200));
        assert!(!sequence_less_than(200, 100));
        assert!(!sequence_less_than(100, 100));

        // ラップアラウンド
        assert!(sequence_less_than(0x7FFF_FFFE, 1));
    }

    #[test]
    fn test_packet_pacing() {
        let mut buf = SenderBuffer::new(1000, 8192, 120);

        // 初期状態: パケットペーシングなし
        assert!(buf.can_send());
        assert!(buf.can_send_with_pacing(Timestamp::from_micros(0)));
        assert_eq!(buf.time_until_send(Timestamp::from_micros(0)), 0);

        // パケット送信間隔を設定 (1000 マイクロ秒 = 1ms)
        buf.set_packet_send_period(1000);

        // 送信時刻を記録
        buf.record_send_time(Timestamp::from_micros(0));

        // 直後は送信不可
        assert!(buf.can_send()); // ウィンドウのみのチェックは可
        assert!(!buf.can_send_with_pacing(Timestamp::from_micros(500))); // ペーシングで不可
        assert_eq!(buf.time_until_send(Timestamp::from_micros(500)), 500);

        // 1000μs 後は送信可能
        assert!(buf.can_send_with_pacing(Timestamp::from_micros(1000)));
        assert_eq!(buf.time_until_send(Timestamp::from_micros(1000)), 0);
    }

    #[test]
    fn test_handle_ack_wrap_around() {
        // BTreeMap の自然順とシーケンス番号順が一致しないラップアラウンド境界のテスト
        // take_while では途中で停止しラップ前のパケットが取りこぼされるが、
        // filter であれば全要素が巡回され正しく削除される
        let mut buf = SenderBuffer::new(0x7FFF_FFFD, 8192, 120);
        let now = Timestamp::from_micros(0);

        // 0x7FFF_FFFD, 0x7FFF_FFFE, 0x7FFF_FFFF (ラップ前)
        buf.push(vec![1], 100, 1, now);
        buf.push(vec![2], 100, 1, now);
        buf.push(vec![3], 100, 1, now);
        // 0, 1, 3 (ラップ後, 3 は ACK されずに残る)
        buf.push(vec![4], 100, 1, now);
        buf.push(vec![5], 100, 1, now);
        buf.push(vec![6], 100, 1, now);

        assert_eq!(buf.packets_in_flight(), 6);

        // ACK 2: 0, 1, 0x7FFF_FFFD, 0x7FFF_FFFE, 0x7FFF_FFFF が削除対象
        // BTreeMap 順: [0, 1, 3, 0x7FFF_FFFD, 0x7FFF_FFFE, 0x7FFF_FFFF]
        // take_while の場合: 0, 1 まで処理し 3 で停止 → ラップ前が残る
        // filter の場合: 全巡回 → ラップ前も削除される
        buf.handle_ack(2);
        assert_eq!(buf.packets_in_flight(), 1);
    }

    #[test]
    fn test_drop_expired_threshold_1s_floor() {
        // latency_ms = 10 (10ms) の場合、1.25 * 10_000 = 12_500 < 1_000_000 なので
        // 閾値は 1_000_000 (1 秒) になる。
        let mut buf = SenderBuffer::new(0, 8192, 10);
        let send_time = Timestamp::from_micros(0);
        buf.push(vec![1], 100, 1, send_time);

        // elapsed = 1_000_000 は閾値と等しいので drop されない (> 判定)
        let now = Timestamp::from_micros(1_000_000);
        let dropped = buf.drop_expired(now);
        assert!(dropped.is_empty(), "等号では drop されないはず");

        // elapsed = 1_000_001 は閾値を超えるので drop される
        let now = Timestamp::from_micros(1_000_001);
        let dropped = buf.drop_expired(now);
        assert_eq!(dropped, vec![0], "閾値超過で drop されるはず");
    }

    #[test]
    fn test_drop_expired_threshold_125pct() {
        // latency_ms = 1000 (1000ms) の場合、1.25 * 1_000_000 = 1_250_000 > 1_000_000 なので
        // 閾値は 1_250_000 になる。
        let mut buf = SenderBuffer::new(0, 8192, 1000);
        let send_time = Timestamp::from_micros(0);
        buf.push(vec![1], 100, 1, send_time);

        // elapsed = 1_250_000 は閾値と等しいので drop されない (> 判定)
        let now = Timestamp::from_micros(1_250_000);
        let dropped = buf.drop_expired(now);
        assert!(dropped.is_empty(), "等号では drop されないはず");

        // elapsed = 1_250_001 は閾値を超えるので drop される
        let now = Timestamp::from_micros(1_250_001);
        let dropped = buf.drop_expired(now);
        assert_eq!(dropped, vec![0], "閾値超過で drop されるはず");
    }

    #[test]
    fn test_drop_expired_threshold_boundary() {
        // latency_ms = 800 の場合、1.25 * 800_000 = 1_000_000 = max(1_000_000, 1_000_000) = 1_000_000
        // 閾値はちょうど 1_000_000 になる (1 秒下限と 1.25 倍側の境界)。
        let mut buf = SenderBuffer::new(0, 8192, 800);
        let send_time = Timestamp::from_micros(0);
        buf.push(vec![1], 100, 1, send_time);

        // elapsed = 1_000_000 は閾値と等しいので drop されない
        let now = Timestamp::from_micros(1_000_000);
        let dropped = buf.drop_expired(now);
        assert!(dropped.is_empty(), "境界値の等号では drop されないはず");

        // elapsed = 1_000_001 は閾値を超えるので drop される
        let now = Timestamp::from_micros(1_000_001);
        let dropped = buf.drop_expired(now);
        assert_eq!(dropped, vec![0], "境界値の超過で drop されるはず");
    }
}
