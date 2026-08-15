//! SRT 受信バッファ
//!
//! 受信パケットの並べ替えと ACK/NAK 生成を管理する。
//!
//! ## 機能
//!
//! - パケット順序制御 (再順序化バッファ)
//! - 重複パケット検出
//! - 損失検出と NAK 生成
//! - ACK 生成 (定期 ACK / Light ACK)
//! - TSBPD (Time-based Packet Delivery)
//! - 受信レート / リンク容量の推定

use std::collections::BTreeMap;

use crate::srt_packet::{DataPacket, sequence_greater_than, sequence_less_than};
use crate::time::Timestamp;

/// Light ACK 送信間隔 (パケット数)
const LIGHT_ACK_INTERVAL: u32 = 64;

/// 定期 ACK 間隔 (マイクロ秒)
const ACK_INTERVAL_US: u64 = 10_000; // 10ms

/// ACK 送信時刻の追跡に保持する最大エントリ数
const MAX_ACK_TIMESTAMPS: usize = 16;

/// Link Capacity 推定に使用するサンプル数
const LINK_CAPACITY_SAMPLES: usize = 16;

/// タイムスタンプの最大値 (32-bit)
const MAX_TIMESTAMP: u64 = 0xFFFF_FFFF;

/// TSBPD ラップアラウンド期間: MAX_TIMESTAMP 到達の 30 秒前から開始
const WRAPPING_PERIOD_START: u64 = MAX_TIMESTAMP - 30_000_000;

/// TSBPD ラップアラウンド期間: タイムスタンプがこの範囲内で終了
const WRAPPING_PERIOD_END_MIN: u64 = 30_000_000;

/// TSBPD ラップアラウンド期間: タイムスタンプがこの値未満で終了する上限 (開区間)
const WRAPPING_PERIOD_END_MAX: u64 = 60_000_000;

/// ACK 送信時刻の追跡 (RTT 計算用)
#[derive(Debug)]
struct AckTimestampTracker {
    /// ACK 番号 -> 送信時刻のマッピング
    timestamps: BTreeMap<u32, Timestamp>,
}

impl AckTimestampTracker {
    fn new() -> Self {
        Self {
            timestamps: BTreeMap::new(),
        }
    }

    /// ACK 送信時刻を記録
    fn record(&mut self, ack_number: u32, send_time: Timestamp) {
        self.timestamps.insert(ack_number, send_time);

        // 古いエントリを削除
        while self.timestamps.len() > MAX_ACK_TIMESTAMPS {
            if let Some(&oldest) = self.timestamps.keys().next() {
                self.timestamps.remove(&oldest);
            }
        }
    }

    /// ACK 送信時刻を取得
    fn get(&self, ack_number: u32) -> Option<Timestamp> {
        self.timestamps.get(&ack_number).copied()
    }
}

/// 受信レート推定器
#[derive(Debug)]
struct ReceivingRateEstimator {
    /// 最後のパケット到着時刻
    last_packet_time: Option<Timestamp>,
    /// 到着間隔の合計 (マイクロ秒)
    interval_sum: u64,
    /// サンプル数
    sample_count: u32,
    /// 受信バイト数 (現在の測定期間)
    bytes_received: u64,
    /// 測定期間開始時刻
    period_start: Timestamp,
    /// 推定 receiving rate (packets/sec)
    estimated_packet_rate: u32,
    /// 推定 receiving rate (bytes/sec)
    estimated_byte_rate: u32,
}

impl ReceivingRateEstimator {
    fn new(start_time: Timestamp) -> Self {
        Self {
            last_packet_time: None,
            interval_sum: 0,
            sample_count: 0,
            bytes_received: 0,
            period_start: start_time,
            estimated_packet_rate: 0,
            estimated_byte_rate: 0,
        }
    }

    /// パケット受信時に呼び出し
    fn on_packet_received(&mut self, now: Timestamp, packet_size: usize) {
        // 到着間隔を計算
        if let Some(last_time) = self.last_packet_time {
            let interval = now.as_micros().saturating_sub(last_time.as_micros());
            // 妥当な間隔のみカウント (1us - 1sec)
            if interval > 0 && interval < 1_000_000 {
                self.interval_sum += interval;
                self.sample_count += 1;
            }
        }
        self.last_packet_time = Some(now);
        self.bytes_received += packet_size as u64;
    }

    /// レートを計算して統計をリセット
    fn calculate_rates(&mut self, now: Timestamp) -> (u32, u32) {
        let elapsed = now
            .as_micros()
            .saturating_sub(self.period_start.as_micros());

        // packets/sec の計算
        let packet_rate = if self.sample_count > 0 && self.interval_sum > 0 {
            let avg_interval = self.interval_sum / self.sample_count as u64;
            1_000_000u64.checked_div(avg_interval).unwrap_or(0) as u32
        } else {
            0
        };

        // bytes/sec の計算
        let byte_rate = (self.bytes_received * 1_000_000)
            .checked_div(elapsed)
            .unwrap_or(0) as u32;

        // EWMA で平滑化 (7/8 * old + 1/8 * new)
        if packet_rate > 0 {
            if self.estimated_packet_rate == 0 {
                self.estimated_packet_rate = packet_rate;
            } else {
                self.estimated_packet_rate =
                    (self.estimated_packet_rate as u64 * 7 / 8 + packet_rate as u64 / 8) as u32;
            }
        }

        if byte_rate > 0 {
            if self.estimated_byte_rate == 0 {
                self.estimated_byte_rate = byte_rate;
            } else {
                self.estimated_byte_rate =
                    (self.estimated_byte_rate as u64 * 7 / 8 + byte_rate as u64 / 8) as u32;
            }
        }

        // 統計をリセット
        self.interval_sum = 0;
        self.sample_count = 0;
        self.bytes_received = 0;
        self.period_start = now;

        (self.estimated_packet_rate, self.estimated_byte_rate)
    }
}

/// Link Capacity 推定器 (Packet Pair Technique)
#[derive(Debug)]
struct LinkCapacityEstimator {
    /// 最後のパケット到着時刻
    last_packet_time: Option<Timestamp>,
    /// Packet Pair 到着間隔のサンプル
    intervals: Vec<u64>,
    /// 推定リンク容量 (packets/sec)
    estimated_capacity: u32,
}

impl LinkCapacityEstimator {
    fn new() -> Self {
        Self {
            last_packet_time: None,
            intervals: Vec::with_capacity(LINK_CAPACITY_SAMPLES),
            estimated_capacity: 0,
        }
    }

    /// パケット受信時に呼び出し
    fn on_packet_received(&mut self, now: Timestamp) {
        if let Some(last_time) = self.last_packet_time {
            let interval = now.as_micros().saturating_sub(last_time.as_micros());

            // 妥当な間隔のみ記録 (1us - 100ms)
            if (1..100_000).contains(&interval) {
                self.intervals.push(interval);
                // 最新 N サンプルを保持
                if self.intervals.len() > LINK_CAPACITY_SAMPLES {
                    self.intervals.remove(0);
                }
            }
        }
        self.last_packet_time = Some(now);
    }

    /// リンク容量を計算
    fn calculate_capacity(&mut self) -> u32 {
        if self.intervals.is_empty() {
            return self.estimated_capacity;
        }

        // 最小間隔を取得 (Packet Pair Technique)
        // 下位 25% の中央値を使用してノイズを軽減
        let mut sorted = self.intervals.clone();
        sorted.sort();

        let quartile_idx = sorted.len() / 4;
        let min_interval = if quartile_idx > 0 {
            sorted[quartile_idx]
        } else {
            sorted[0]
        };

        if let Some(capacity) = 1_000_000u64.checked_div(min_interval).map(|v| v as u32) {
            // EWMA で平滑化
            if self.estimated_capacity == 0 {
                self.estimated_capacity = capacity;
            } else {
                self.estimated_capacity =
                    (self.estimated_capacity as u64 * 7 / 8 + capacity as u64 / 8) as u32;
            }
        }

        self.estimated_capacity
    }
}

/// 受信パケットエントリ
#[derive(Debug, Clone)]
struct ReceivedPacket {
    /// パケットデータ
    packet: DataPacket,
    /// 受信時刻 (統計・ジッター計算用)
    #[expect(dead_code)]
    recv_time: Timestamp,
    /// 配信予定時刻 (TSBPD)
    delivery_time: Timestamp,
}

/// ACK 情報
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckPacket {
    /// ACK シーケンス番号 (次に期待するパケット)
    pub ack_seq: u32,
    /// RTT (マイクロ秒)
    pub rtt: u32,
    /// RTT Variance (マイクロ秒)
    pub rtt_var: u32,
    /// 利用可能バッファサイズ (パケット数)
    pub available_buffer: u32,
    /// 受信レート (パケット/秒)
    pub receiving_rate: u32,
    /// 推定リンク容量 (パケット/秒)
    pub link_capacity: u32,
    /// 受信レート (バイト/秒)
    pub recv_rate: u32,
    /// Light ACK かどうか
    pub is_light: bool,
}

/// NAK 情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NakPacket {
    /// 損失シーケンス番号リスト
    pub loss_list: Vec<u32>,
}

/// 受信バッファ
#[derive(Debug)]
pub struct ReceiverBuffer {
    /// 受信パケット (sequence_number -> ReceivedPacket)
    packets: BTreeMap<u32, ReceivedPacket>,

    /// 次に期待するシーケンス番号
    expected_seq: u32,

    /// 損失リスト (検出した損失パケット)
    loss_list: Vec<u32>,

    /// 最後に ACK 送信した時刻
    last_ack_time: Timestamp,

    /// 最後に ACK 送信したシーケンス番号
    last_ack_seq: u32,

    /// ACK 送信後に受信したパケット数 (Light ACK 用)
    packets_since_ack: u32,

    /// ACK シーケンス番号 (ACK パケット自体の番号)
    ack_number: u32,

    /// TSBPD 遅延 (マイクロ秒)
    tsbpd_delay_us: u64,

    /// TSBPD 有効かどうか
    tsbpd_enabled: bool,

    /// TSBPD 時刻基準 (TsbpdTimeBase = T_NOW - HSREQ_TIMESTAMP, マイクロ秒)
    tsbpd_time_base: u64,

    /// TSBPD ラップアラウンド期間中かどうか
    wrapping_period_active: bool,

    /// RTT (マイクロ秒)
    rtt: u32,

    /// RTT Variance (マイクロ秒)
    rtt_var: u32,

    /// バッファ最大サイズ
    max_buffer_size: u32,

    /// 受信パケット総数 (統計用)
    total_received: u64,

    /// 損失パケット総数 (統計用)
    total_lost: u64,

    /// 重複パケット総数 (統計用)
    total_duplicates: u64,

    /// 受信バイト総数 (統計用)
    total_bytes_received: u64,

    /// ジッター (マイクロ秒) - RFC 3550 方式で計算
    jitter: u32,

    /// 前回のパケット到着間隔 (ジッター計算用)
    last_transit: Option<i64>,

    /// ACK 送信時刻の追跡 (RTT 計算用)
    ack_timestamps: AckTimestampTracker,

    /// 受信レート推定器
    rate_estimator: ReceivingRateEstimator,

    /// リンク容量推定器
    link_capacity_estimator: LinkCapacityEstimator,
}

impl ReceiverBuffer {
    /// 新しい受信バッファを作成
    pub fn new(
        initial_seq: u32,
        tsbpd_delay_ms: u16,
        start_time: Timestamp,
        tsbpd_time_base: u64,
    ) -> Self {
        Self {
            packets: BTreeMap::new(),
            expected_seq: initial_seq,
            loss_list: Vec::new(),
            last_ack_time: start_time,
            last_ack_seq: initial_seq,
            packets_since_ack: 0,
            ack_number: 1,
            tsbpd_delay_us: tsbpd_delay_ms as u64 * 1000,
            tsbpd_enabled: true,
            tsbpd_time_base,
            wrapping_period_active: false,
            rtt: 100_000, // 初期 RTT: 100ms
            rtt_var: 50_000,
            max_buffer_size: 8192,
            total_received: 0,
            total_lost: 0,
            total_duplicates: 0,
            total_bytes_received: 0,
            jitter: 0,
            last_transit: None,
            ack_timestamps: AckTimestampTracker::new(),
            rate_estimator: ReceivingRateEstimator::new(start_time),
            link_capacity_estimator: LinkCapacityEstimator::new(),
        }
    }

    /// TSBPD を有効/無効にする
    pub fn set_tsbpd_enabled(&mut self, enabled: bool) {
        self.tsbpd_enabled = enabled;
    }

    /// 次に期待するシーケンス番号を取得
    pub fn expected_sequence(&self) -> u32 {
        self.expected_seq
    }

    /// パケットを受信
    ///
    /// 損失が検出された場合、損失リストを返す
    pub fn receive(&mut self, packet: DataPacket, now: Timestamp) -> Option<Vec<u32>> {
        let seq = packet.sequence_number;

        // 重複チェック
        if self.packets.contains_key(&seq) {
            self.total_duplicates += 1;
            return None;
        }

        // 古すぎるパケットは無視
        if sequence_less_than(seq, self.expected_seq) {
            return None;
        }

        self.total_received += 1;
        self.packets_since_ack += 1;

        // 帯域推定のためにパケット到着を記録
        let packet_size = packet.payload.len() + 16; // SRT ヘッダサイズを加算
        self.total_bytes_received += packet_size as u64;
        self.rate_estimator.on_packet_received(now, packet_size);
        self.link_capacity_estimator.on_packet_received(now);

        // ジッター計算 (RFC 3550 方式)
        // transit = 受信時刻 - パケットタイムスタンプ
        let transit = now.as_micros() as i64 - packet.timestamp as i64;
        if let Some(last) = self.last_transit {
            // d = |transit - last_transit|
            let d = (transit - last).unsigned_abs() as u32;
            // jitter = jitter + (d - jitter) / 16
            self.jitter = self
                .jitter
                .saturating_add((d.saturating_sub(self.jitter)) / 16);
        }
        self.last_transit = Some(transit);

        // TSBPD ラップアラウンド期間の開始判定
        // 終了判定は pop_ready() に移動 (仕様: "is delivered (read from the buffer)")
        if self.tsbpd_enabled {
            let ts = packet.timestamp as u64;
            if ts >= WRAPPING_PERIOD_START && !self.wrapping_period_active {
                self.wrapping_period_active = true;
            }
        }

        // TSBPD 配信時刻を計算
        let delivery_time = if self.tsbpd_enabled {
            // ラップ後パケット (wrapping_period_active かつ ts < WRAPPING_PERIOD_START) の
            // 配信時刻は MAX_TIMESTAMP + 1 を加算して補正する。
            // ラップ前パケットの ts は WRAPPING_PERIOD_START 以上であり衝突しない。
            let pkt_time = self.tsbpd_time_base
                + packet.timestamp as u64
                + if self.wrapping_period_active
                    && (packet.timestamp as u64) < WRAPPING_PERIOD_START
                {
                    MAX_TIMESTAMP + 1
                } else {
                    0
                };
            Timestamp::from_micros(pkt_time + self.tsbpd_delay_us)
        } else {
            now
        };

        // バッファに追加
        self.packets.insert(
            seq,
            ReceivedPacket {
                packet,
                recv_time: now,
                delivery_time,
            },
        );

        // 損失検出
        let mut new_losses = Vec::new();
        if sequence_greater_than(seq, self.expected_seq) {
            // ギャップがある = 損失の可能性
            let mut s = self.expected_seq;
            while sequence_less_than(s, seq) {
                if !self.packets.contains_key(&s) && !self.loss_list.contains(&s) {
                    new_losses.push(s);
                    self.loss_list.push(s);
                    self.total_lost += 1;
                }
                s = s.wrapping_add(1) & 0x7FFF_FFFF;
            }
        }

        // expected_seq を更新
        while self.packets.contains_key(&self.expected_seq) {
            self.expected_seq = self.expected_seq.wrapping_add(1) & 0x7FFF_FFFF;
        }

        // 損失リストから回復したパケットを削除
        self.loss_list.retain(|&s| s != seq);

        if new_losses.is_empty() {
            None
        } else {
            Some(new_losses)
        }
    }

    /// 配信可能なパケットを取得 (TSBPD)
    pub fn pop_ready(&mut self, now: Timestamp) -> Option<DataPacket> {
        // 配信可能なシーケンス番号を探す
        let delivery_seq = self.find_deliverable_seq(now)?;

        let entry = self.packets.remove(&delivery_seq)?;

        // TSBPD ラップアラウンド期間の終了判定
        // 仕様 (draft-sharabayko-srt.md の #tsbpd-time-base 節):
        // "ends once the packet with timestamp within (30, 60) seconds interval is delivered"
        if self.tsbpd_enabled && self.wrapping_period_active {
            let ts = entry.packet.timestamp as u64;
            if (WRAPPING_PERIOD_END_MIN..WRAPPING_PERIOD_END_MAX).contains(&ts) {
                self.tsbpd_time_base += MAX_TIMESTAMP + 1;
                self.wrapping_period_active = false;
            }
        }

        Some(entry.packet)
    }

    /// 配信可能なシーケンス番号を検索
    ///
    /// `packets` の BTreeMap は u32 の数値順でイテレートするが、31-bit シーケンス番号の
    /// 循環順 (`sequence_less_than`) とラップアラウンド境界 (0x7FFF_FFFF -> 0) で食い違う。
    /// SRT 仕様 (draft-sharabayko-srt.md の Live Streaming セクション) は TSBPD 配信を
    /// 「deliver packets in order, but based on the timestamps」と定めており、配送順は
    /// 数値順ではなく循環順に従う必要がある (節構成・行番号は将来変更される可能性がある)。
    /// 数値順で最初に見つけた候補を返すと境界をまたぐ連続パケットの配送順序が逆転するため、
    /// 配信候補の中から循環順で最小の seq を選ぶ。
    ///
    /// 早期 return せず全候補を走査するのは、循環順最小が数値順最小と食い違うのはラップ境界を
    /// またぐ場合のみだが、その判定に全候補の循環順比較が要るためである。堅牢性を優先し、
    /// 配信ポインタ等の最適化は導入しない。
    fn find_deliverable_seq(&self, now: Timestamp) -> Option<u32> {
        let mut best: Option<u32> = None;
        for (&seq, entry) in &self.packets {
            let time_ok = !self.tsbpd_enabled || entry.delivery_time <= now;
            let has_gap = self.loss_list.iter().any(|&s| sequence_less_than(s, seq));
            if time_ok && !has_gap {
                // 既存 best が seq より循環順で前なら保持、そうでなければ seq に更新する
                best = match best {
                    Some(b) if sequence_less_than(b, seq) => Some(b),
                    _ => Some(seq),
                };
            }
        }
        best
    }

    /// ACK を生成すべきかチェック
    pub fn should_send_ack(&self, now: Timestamp) -> bool {
        // Light ACK: 64 パケット受信ごと
        if self.packets_since_ack >= LIGHT_ACK_INTERVAL {
            return true;
        }

        // 定期 ACK: 10ms ごと
        let elapsed = now
            .as_micros()
            .saturating_sub(self.last_ack_time.as_micros());
        elapsed >= ACK_INTERVAL_US
    }

    /// ACK を生成
    pub fn generate_ack(&mut self, now: Timestamp) -> AckPacket {
        let is_light = self.packets_since_ack >= LIGHT_ACK_INTERVAL
            && now
                .as_micros()
                .saturating_sub(self.last_ack_time.as_micros())
                < ACK_INTERVAL_US;

        self.last_ack_time = now;
        self.last_ack_seq = self.expected_seq;
        self.packets_since_ack = 0;

        if !is_light {
            self.ack_number = self.ack_number.wrapping_add(1);
            self.ack_timestamps.record(self.ack_number, now);
        }

        // 受信レートとリンク容量を計算
        let (receiving_rate, recv_rate) = self.rate_estimator.calculate_rates(now);
        let link_capacity = self.link_capacity_estimator.calculate_capacity();

        AckPacket {
            ack_seq: self.expected_seq,
            rtt: self.rtt,
            rtt_var: self.rtt_var,
            available_buffer: (self.max_buffer_size - self.packets.len() as u32),
            receiving_rate,
            link_capacity,
            recv_rate,
            is_light,
        }
    }

    /// Periodic NAK を生成
    pub fn generate_periodic_nak(&self) -> Option<NakPacket> {
        if self.loss_list.is_empty() {
            return None;
        }

        Some(NakPacket {
            loss_list: self.loss_list.clone(),
        })
    }

    /// NAK 送信間隔を計算 (RTT + 4*RTTVar) / 2
    pub fn nak_interval(&self) -> u64 {
        let interval = (self.rtt as u64 + 4 * self.rtt_var as u64) / 2;
        interval.max(20_000) // 最低 20ms
    }

    /// ACKACK を処理して RTT を更新
    pub fn handle_ackack(&mut self, ack_number: u32, now: Timestamp) {
        // ACK 送信時刻をマッピングから取得
        let send_time = match self.ack_timestamps.get(ack_number) {
            Some(t) => t,
            None => return, // 対応する ACK が見つからない場合は無視
        };

        // RTT を計算
        let rtt = (now.as_micros().saturating_sub(send_time.as_micros())) as u32;

        // RTT が妥当な範囲かチェック (1us - 30sec)
        if rtt == 0 || rtt > 30_000_000 {
            return;
        }

        // EWMA で平滑化: RTT = 7/8 * RTT + 1/8 * rtt
        self.rtt = (self.rtt * 7 / 8) + (rtt / 8);

        // RTTVar = 3/4 * RTTVar + 1/4 * |RTT - rtt|
        let diff = self.rtt.abs_diff(rtt);
        self.rtt_var = (self.rtt_var * 3 / 4) + (diff / 4);
    }

    /// 期限切れパケットを削除 (TLPKTDROP)
    pub fn drop_too_late(&mut self, now: Timestamp) -> Vec<u32> {
        if !self.tsbpd_enabled {
            return Vec::new();
        }

        let tlpktdrop_threshold = ((self.tsbpd_delay_us as u128 * 125 / 100) as u64).max(1_000_000); // 最低 1 秒

        let mut dropped = Vec::new();

        // 欠損パケットの推定配信時刻を計算する。
        // 各欠損 seq に対して、循環順で次側の受信パケットの delivery_time を推定値として使用する。
        // 次側の delivery_time は欠損パケットの真の配信時刻以上であるため、この推定は過大評価側になる。
        // 次側の受信パケットが存在しない場合は、防御的にフォールバック値を使用する。
        let expired: Vec<u32> = self
            .loss_list
            .iter()
            .copied()
            .filter(|&seq| {
                let estimated_delivery = self
                    .packets
                    .get(&seq)
                    .map(|p| p.delivery_time.as_micros())
                    .unwrap_or_else(|| {
                        // 循環順で seq より大きい最小の受信パケットを探す。
                        // BTreeMap の数値順で seq より大きい最初の要素を取得し、なければ最小の要素を取る。
                        let next_seq = self
                            .packets
                            .range(seq.wrapping_add(1)..)
                            .next()
                            .or_else(|| self.packets.iter().next());
                        match next_seq {
                            Some((_, entry)) => entry.delivery_time.as_micros(),
                            // 次側の受信パケットが存在しない場合のフォールバック。
                            // wrapping_period_active が有効中は MAX_TIMESTAMP + 1 を加算する (0021 の修正を継承)。
                            None => {
                                let base = self.tsbpd_time_base + self.tsbpd_delay_us;
                                if self.wrapping_period_active {
                                    base + MAX_TIMESTAMP + 1
                                } else {
                                    base
                                }
                            }
                        }
                    });
                now.as_micros() > estimated_delivery + tlpktdrop_threshold
            })
            .collect();

        for seq in expired {
            self.loss_list.retain(|&s| s != seq);
            dropped.push(seq);
        }

        dropped
    }

    /// 現在の ACK シーケンス番号を取得
    pub fn ack_number(&self) -> u32 {
        self.ack_number
    }

    /// RTT を取得
    pub fn rtt(&self) -> u32 {
        self.rtt
    }

    /// RTT Variance を取得
    pub fn rtt_var(&self) -> u32 {
        self.rtt_var
    }

    /// 統計情報を取得
    pub fn stats(&self) -> ReceiverStats {
        // パケットロス率を計算 (パーセント * 100)
        // total_received には回復したパケットも含まれるため、
        // 損失率 = total_lost / (total_received + total_lost) * 100 * 100
        let total = self.total_received + self.total_lost;
        let loss_rate_percent_x100 =
            (self.total_lost * 10000).checked_div(total).unwrap_or(0) as u32;

        ReceiverStats {
            packets_in_buffer: self.packets.len() as u32,
            packets_in_loss_list: self.loss_list.len() as u32,
            total_received: self.total_received,
            total_lost: self.total_lost,
            total_duplicates: self.total_duplicates,
            rtt: self.rtt,
            rtt_var: self.rtt_var,
            total_bytes_received: self.total_bytes_received,
            loss_rate_percent_x100,
            jitter: self.jitter,
        }
    }
}

/// 受信統計
#[derive(Debug, Clone, Copy, Default)]
pub struct ReceiverStats {
    /// バッファ内のパケット数
    pub packets_in_buffer: u32,
    /// 損失リストのパケット数
    pub packets_in_loss_list: u32,
    /// 受信パケット総数
    pub total_received: u64,
    /// 損失パケット総数
    pub total_lost: u64,
    /// 重複パケット総数
    pub total_duplicates: u64,
    /// RTT (マイクロ秒)
    pub rtt: u32,
    /// RTT Variance (マイクロ秒)
    pub rtt_var: u32,
    /// 受信バイト総数
    pub total_bytes_received: u64,
    /// パケットロス率 (パーセント * 100、例: 123 = 1.23%)
    pub loss_rate_percent_x100: u32,
    /// ジッター (マイクロ秒)
    pub jitter: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_packet(seq: u32, timestamp: u32) -> DataPacket {
        DataPacket {
            sequence_number: seq,
            position: crate::srt_packet::PacketPosition::Single,
            order_flag: false,
            encryption_flag: 0,
            retransmitted: false,
            message_number: 1,
            timestamp,
            dest_socket_id: 1,
            payload: vec![1, 2, 3],
        }
    }

    #[test]
    fn test_receiver_buffer_new() {
        let start = Timestamp::from_micros(0);
        let buf = ReceiverBuffer::new(1000, 120, start, 0);
        assert_eq!(buf.expected_sequence(), 1000);
    }

    #[test]
    fn test_receiver_buffer_receive_in_order() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        // 順序通りに受信
        let losses = buf.receive(make_packet(1000, 100), now);
        assert!(losses.is_none());
        assert_eq!(buf.expected_sequence(), 1001);

        let losses = buf.receive(make_packet(1001, 200), now);
        assert!(losses.is_none());
        assert_eq!(buf.expected_sequence(), 1002);
    }

    #[test]
    fn test_receiver_buffer_loss_detection() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        // 1000 を受信
        buf.receive(make_packet(1000, 100), now);
        // 1001 をスキップして 1002 を受信
        let losses = buf.receive(make_packet(1002, 300), now);

        assert!(losses.is_some());
        let lost = losses.expect("欠落パケットは Some になる想定");
        assert_eq!(lost, vec![1001]);
    }

    #[test]
    fn test_receiver_buffer_duplicate() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        buf.receive(make_packet(1000, 100), now);
        // 同じパケットを再度受信
        let losses = buf.receive(make_packet(1000, 100), now);
        assert!(losses.is_none());

        let stats = buf.stats();
        assert_eq!(stats.total_duplicates, 1);
    }

    #[test]
    fn test_receiver_buffer_pop_ready() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        buf.receive(make_packet(1000, 100), now);
        buf.receive(make_packet(1001, 200), now);

        let pkt = buf.pop_ready(now);
        assert!(pkt.is_some());
        assert_eq!(
            pkt.expect("配信可能パケットは Some になる想定")
                .sequence_number,
            1000
        );

        let pkt = buf.pop_ready(now);
        assert!(pkt.is_some());
        assert_eq!(
            pkt.expect("配信可能パケットは Some になる想定")
                .sequence_number,
            1001
        );
    }

    #[test]
    fn test_receiver_buffer_ack_generation() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);

        let now = Timestamp::from_micros(1000);

        buf.receive(make_packet(1000, 100), now);
        buf.receive(make_packet(1001, 200), now);

        let ack = buf.generate_ack(now);
        assert_eq!(ack.ack_seq, 1002);
    }

    #[test]
    fn test_receiver_buffer_nak_generation() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        buf.receive(make_packet(1000, 100), now);
        buf.receive(make_packet(1002, 300), now); // 1001 欠落

        let nak = buf.generate_periodic_nak();
        assert!(nak.is_some());
        assert_eq!(
            nak.expect("欠落パケットは NAK が生成される想定").loss_list,
            vec![1001]
        );
    }

    #[test]
    fn test_ack_timestamp_tracker() {
        let mut tracker = AckTimestampTracker::new();

        // ACK 送信時刻を記録
        let t1 = Timestamp::from_micros(1000);
        let t2 = Timestamp::from_micros(2000);
        let t3 = Timestamp::from_micros(3000);

        tracker.record(1, t1);
        tracker.record(2, t2);
        tracker.record(3, t3);

        // 記録した時刻を取得できる
        assert_eq!(tracker.get(1), Some(t1));
        assert_eq!(tracker.get(2), Some(t2));
        assert_eq!(tracker.get(3), Some(t3));

        // 存在しない ACK 番号は None
        assert_eq!(tracker.get(99), None);
    }

    #[test]
    fn test_ack_timestamp_tracker_max_entries() {
        let mut tracker = AckTimestampTracker::new();

        // MAX_ENTRIES (16) を超えるエントリを追加
        for i in 0..20 {
            tracker.record(i, Timestamp::from_micros(i as u64 * 1000));
        }

        // 古いエントリは削除される (0-3 が削除される)
        assert_eq!(tracker.get(0), None);
        assert_eq!(tracker.get(3), None);

        // 新しいエントリは残る
        assert!(tracker.get(4).is_some());
        assert!(tracker.get(19).is_some());
    }

    #[test]
    fn test_receiving_rate_estimator() {
        let start = Timestamp::from_micros(0);
        let mut estimator = ReceivingRateEstimator::new(start);

        // 1ms 間隔でパケットを受信 (1000 packets/sec 相当)
        for i in 0..100 {
            let now = Timestamp::from_micros(i * 1000); // 1ms 間隔
            estimator.on_packet_received(now, 1500); // 1500 bytes
        }

        let now = Timestamp::from_micros(100_000); // 100ms 経過
        let (packet_rate, byte_rate) = estimator.calculate_rates(now);

        // 約 1000 packets/sec を期待
        // EWMA により初回は 1/8 の重みなので、完全な値にはならない
        assert!(packet_rate > 0, "パケットレートが 0 より大きいこと");

        // バイトレートも計算される
        assert!(byte_rate > 0, "バイトレートが 0 より大きいこと");
    }

    #[test]
    fn test_receiving_rate_estimator_no_packets() {
        let start = Timestamp::from_micros(0);
        let mut estimator = ReceivingRateEstimator::new(start);

        // パケットを受信しない状態でレート計算
        let now = Timestamp::from_micros(100_000);
        let (packet_rate, byte_rate) = estimator.calculate_rates(now);

        // サンプルがないので 0 のまま
        assert_eq!(packet_rate, 0);
        assert_eq!(byte_rate, 0);
    }

    #[test]
    fn test_link_capacity_estimator() {
        let mut estimator = LinkCapacityEstimator::new();

        // 100μs 間隔でパケットを受信 (10000 packets/sec 相当)
        for i in 0..20 {
            let now = Timestamp::from_micros(i * 100);
            estimator.on_packet_received(now);
        }

        let capacity = estimator.calculate_capacity();

        // 約 10000 packets/sec を期待
        // Packet Pair Technique で下位 25% を使うため、値は変動する
        assert!(capacity > 0, "容量が 0 より大きいこと");
    }

    #[test]
    fn test_link_capacity_estimator_no_samples() {
        let mut estimator = LinkCapacityEstimator::new();

        // パケット受信がない場合
        let capacity = estimator.calculate_capacity();

        // サンプルなしで 0 を返す
        assert_eq!(capacity, 0);
    }

    #[test]
    fn test_link_capacity_estimator_single_interval() {
        let mut estimator = LinkCapacityEstimator::new();

        // 2 つのパケットで 1 つの間隔を記録
        estimator.on_packet_received(Timestamp::from_micros(0));
        estimator.on_packet_received(Timestamp::from_micros(100)); // 100μs 間隔

        let capacity = estimator.calculate_capacity();

        // 1 つのサンプルでも計算される (1,000,000 / 100 = 10000)
        assert_eq!(capacity, 10000);
    }

    #[test]
    fn test_rtt_calculation_with_ack_timestamps() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        // パケットを受信
        let now = Timestamp::from_micros(1000);
        buf.receive(make_packet(1000, 100), now);

        // ACK を生成 (これにより ACK 送信時刻が記録される)
        let ack_time = Timestamp::from_micros(2000);
        let ack = buf.generate_ack(ack_time);
        let ack_number = ack.ack_seq;

        // ACKACK を受信 (RTT = 50ms)
        let ackack_time = Timestamp::from_micros(52000); // 50ms 後
        buf.handle_ackack(ack_number, ackack_time);

        // RTT が計算される (EWMA: 1/8 の重み)
        let stats = buf.stats();
        // 初期 RTT は 100ms (100000μs) なので、
        // RTT = 7/8 * 100000 + 1/8 * 50000 = 87500 + 6250 = 93750
        assert!(stats.rtt > 0, "RTT が計算されていること");
    }

    #[test]
    fn test_ack_includes_recv_rate() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        // パケットを複数回受信
        for i in 0..10 {
            let now = Timestamp::from_micros(i * 1000);
            buf.receive(make_packet(1000 + i as u32, 100 + i as u32 * 100), now);
        }

        // ACK 生成
        let now = Timestamp::from_micros(10000);
        let ack = buf.generate_ack(now);

        // AckPacket のフィールドが設定される
        // 初回なので値は小さいが、フィールドが存在することを確認
        let _receiving_rate = ack.receiving_rate;
        let _link_capacity = ack.link_capacity;
        let _recv_rate = ack.recv_rate;

        // Full ACK として生成される (Light ACK ではない)
        assert!(!ack.is_light);
    }

    #[test]
    fn test_total_bytes_received() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        // 3 バイトのペイロード + 16 バイトのヘッダ = 19 バイト
        buf.receive(make_packet(1000, 100), now);
        let stats = buf.stats();
        assert_eq!(stats.total_bytes_received, 19);

        // さらに 1 パケット受信
        buf.receive(make_packet(1001, 200), now);
        let stats = buf.stats();
        assert_eq!(stats.total_bytes_received, 38);
    }

    #[test]
    fn test_loss_rate_calculation() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        // 10 パケット受信、2 パケット損失 (1001, 1004)
        buf.receive(make_packet(1000, 100), now);
        buf.receive(make_packet(1002, 200), now); // 1001 損失
        buf.receive(make_packet(1003, 300), now);
        buf.receive(make_packet(1005, 400), now); // 1004 損失
        buf.receive(make_packet(1006, 500), now);
        buf.receive(make_packet(1007, 600), now);
        buf.receive(make_packet(1008, 700), now);
        buf.receive(make_packet(1009, 800), now);

        let stats = buf.stats();
        // total_received = 8, total_lost = 2
        // loss_rate = 2 / (8 + 2) * 100 * 100 = 2000 (= 20.00%)
        assert_eq!(stats.total_received, 8);
        assert_eq!(stats.total_lost, 2);
        assert_eq!(stats.loss_rate_percent_x100, 2000);
    }

    #[test]
    fn test_loss_rate_zero() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        // 損失なしで受信
        buf.receive(make_packet(1000, 100), now);
        buf.receive(make_packet(1001, 200), now);
        buf.receive(make_packet(1002, 300), now);

        let stats = buf.stats();
        assert_eq!(stats.total_received, 3);
        assert_eq!(stats.total_lost, 0);
        assert_eq!(stats.loss_rate_percent_x100, 0);
    }

    #[test]
    fn test_jitter_calculation() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        // タイムスタンプと到着時刻の差が一定の場合、ジッターは小さい
        buf.receive(make_packet(1000, 1000), Timestamp::from_micros(2000)); // transit = 1000
        buf.receive(make_packet(1001, 2000), Timestamp::from_micros(3000)); // transit = 1000, d = 0

        let stats = buf.stats();
        // d = 0 なので jitter は増えない
        assert_eq!(stats.jitter, 0);

        // タイムスタンプと到着時刻の差が変動する場合、ジッターが増加
        buf.receive(make_packet(1002, 3000), Timestamp::from_micros(4500)); // transit = 1500, d = 500
        let stats = buf.stats();
        // jitter = 0 + (500 - 0) / 16 = 31
        assert_eq!(stats.jitter, 31);

        // さらに変動
        buf.receive(make_packet(1003, 4000), Timestamp::from_micros(5000)); // transit = 1000, d = 500
        let stats = buf.stats();
        // jitter = 31 + (500 - 31) / 16 = 31 + 29 = 60
        assert_eq!(stats.jitter, 60);
    }

    #[test]
    fn test_jitter_no_packets() {
        let start = Timestamp::from_micros(0);
        let buf = ReceiverBuffer::new(1000, 120, start, 0);

        let stats = buf.stats();
        // パケットを受信していない場合、jitter は 0
        assert_eq!(stats.jitter, 0);
    }

    #[test]
    fn test_tsbpd_delivery_time_uses_tsbpd_time_base() {
        let start = Timestamp::from_micros(1_000_000); // T=1 秒
        // TsbpdTimeBase = 500_000μs (RTT_0/2 = 250ms 相当)
        let tsbpd_time_base = 500_000;
        let mut buf = ReceiverBuffer::new(1000, 120, start, tsbpd_time_base);

        let now = Timestamp::from_micros(1_000_000);
        let pkt = make_packet(1000, 200_000); // PKT_TIMESTAMP = 200ms
        buf.receive(pkt, now);

        let delivered = buf.pop_ready(Timestamp::from_micros(1_500_000));
        // delivery_time = tsbpd_time_base + PKT_TIMESTAMP + tsbpd_delay
        //                = 500_000 + 200_000 + 120_000 = 820_000μs
        // now=1_500_000 > 820_000 なので配送される
        assert!(delivered.is_some());
        assert_eq!(
            delivered
                .expect("配信可能パケットは Some になる想定")
                .sequence_number,
            1000
        );
    }

    #[test]
    fn test_tsbpd_delivery_not_ready_before_delay() {
        let start = Timestamp::from_micros(1_000_000);
        let tsbpd_time_base = 500_000;
        let mut buf = ReceiverBuffer::new(1000, 120, start, tsbpd_time_base);

        let now = Timestamp::from_micros(1_000_000);
        let pkt = make_packet(1000, 200_000);
        buf.receive(pkt, now);

        // PKT_TIMESTAMP=200_000 + tsbpd_time_base=500_000 = 700_000
        // tsbpd_delay=120_000, delivery_time = 820_000
        // now=700_000 < 820_000 なので配送されない
        let delivered = buf.pop_ready(Timestamp::from_micros(700_000));
        assert!(delivered.is_none());
    }

    #[test]
    fn test_drop_too_late_uses_tsbpd_time_base() {
        let start = Timestamp::from_micros(1_000_000);
        let tsbpd_time_base = 500_000;
        let mut buf = ReceiverBuffer::new(1000, 120, start, tsbpd_time_base);

        let now = Timestamp::from_micros(1_000_000);
        // 1001 を受信して 1000 が損失として登録される
        buf.receive(make_packet(1001, 200_000), now);

        assert_eq!(buf.loss_list, vec![1000]);

        // TLPKTDROP = max(1.25 * 120_000, 1_000_000) = 1_000_000μs
        // 次側パケット seq 1001 の delivery_time = 500_000 + 200_000 + 120_000 = 820_000
        // now = 2_000_000 > 820_000 + 1_000_000 = 1_820_000 なので削除される
        let dropped = buf.drop_too_late(Timestamp::from_micros(2_000_000));
        assert_eq!(dropped, vec![1000]);
    }

    #[test]
    fn test_drop_too_late_individual_delivery() {
        let start = Timestamp::from_micros(1_000_000);
        let tsbpd_time_base = 500_000;
        let mut buf = ReceiverBuffer::new(1000, 500, start, tsbpd_time_base);
        // tsbpd_delay = 500ms, tsbpd_delay_us = 500_000

        let now = Timestamp::from_micros(1_000_000);
        // 1000 を受信 (delivery_time = 500_000 + 100_000 + 500_000 = 1_100_000)
        buf.receive(make_packet(1000, 100_000), now);
        // 1002 を受信して 1001 が損失として登録される
        // 1001 の推定配信時刻は次側パケット seq 1002 の delivery_time
        // = 500_000 + 300_000 + 500_000 = 1_300_000
        buf.receive(make_packet(1002, 300_000), now);

        // TLPKTDROP = max(1.25 * 500_000, 1_000_000) = 1_000_000
        // 1000: delivery = 1_100_000, 1_100_000 + 1_000_000 = 2_100_000
        // 1001: estimated = 1_300_000, 1_300_000 + 1_000_000 = 2_300_000
        // now = 2_200_000: 1000 は超過、1001 は未到達 → 1000 のみ削除 (1000 は loss_list にないので削除されない)
        // now = 2_400_000: 両方超過 → 1001 が削除される
        let dropped = buf.drop_too_late(Timestamp::from_micros(2_400_000));
        assert_eq!(dropped, vec![1001]);
        assert_eq!(buf.loss_list, Vec::<u32>::new());
    }

    #[test]
    fn test_light_ack_does_not_increment_ack_number() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        // 64 パケット受信して Light ACK 条件を満たす
        let now = Timestamp::from_micros(0);
        for i in 0..64 {
            buf.receive(make_packet(1000 + i as u32, i as u32 * 100), now);
        }

        let ack_number_before = buf.ack_number();
        let ack = buf.generate_ack(now);
        assert!(ack.is_light);

        // Light ACK では ack_number がインクリメントされない
        assert_eq!(buf.ack_number(), ack_number_before);
    }

    #[test]
    fn test_full_ack_increments_ack_number() {
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(1000, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        // 数パケット受信 (Light ACK 条件を満たさない)
        let now = Timestamp::from_micros(0);
        for i in 0..10 {
            buf.receive(make_packet(1000 + i as u32, i as u32 * 100), now);
        }

        let ack_number_before = buf.ack_number();
        // 定期 ACK 間隔 (10ms) 経過させる
        let ack = buf.generate_ack(Timestamp::from_micros(10_000));
        assert!(!ack.is_light);

        // Full ACK では ack_number がインクリメントされる
        assert_eq!(buf.ack_number(), ack_number_before + 1);
    }

    #[test]
    fn test_pop_ready_blocks_on_loss_across_wrap_boundary() {
        // ラップ境界をまたぐ区間で先頭の 0x7FFF_FFFE が欠損しているとき、loss_list による
        // HoL ブロッキングが後続 (循環順で 0x7FFF_FFFE より後ろ) の配信を止め続けることを検証する。
        // 欠損なしの配送順序は PBT が網羅するため、ここでは PBT で作りにくい損失ありの境界ケースを置く。
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(0x7FFF_FFFE, 120, start, 0);
        buf.set_tsbpd_enabled(false);

        let now = Timestamp::from_micros(1000);

        // 0x7FFF_FFFE を欠損させ、循環順で後続の 0x7FFF_FFFF, 0, 1 を受信する。
        // 0x7FFF_FFFE が loss_list に残る。
        buf.receive(make_packet(0x7FFF_FFFF, 100), now);
        buf.receive(make_packet(0, 100), now);
        buf.receive(make_packet(1, 100), now);

        // 0x7FFF_FFFE が損失として残る間は、循環順で後ろの候補は配信されない。
        assert!(buf.pop_ready(now).is_none());
    }

    #[test]
    fn test_pop_ready_skips_hole_after_drop_across_wrap_boundary() {
        // ラップ境界をまたぐ区間で 0x7FFF_FFFE が欠損したまま drop_too_late で穴が loss_list から
        // 除去されたら、後続 0x7FFF_FFFF, 0, 1 が循環順で配信されること (穴スキップ維持) を検証する。
        // drop_too_late は tsbpd 有効が前提のため、ここでは tsbpd を無効化しない。
        let start = Timestamp::from_micros(0);
        let mut buf = ReceiverBuffer::new(0x7FFF_FFFE, 120, start, 0);

        let recv_now = Timestamp::from_micros(1000);
        buf.receive(make_packet(0x7FFF_FFFF, 100), recv_now);
        buf.receive(make_packet(0, 100), recv_now);
        buf.receive(make_packet(1, 100), recv_now);

        // 穴 (0x7FFF_FFFE) が残る間は HoL ブロッキングで配信されない。
        // tlpktdrop 閾値 (最低 1 秒) を確実に超える時刻で評価する。
        let late_now = Timestamp::from_micros(10_000_000);
        assert!(buf.pop_ready(late_now).is_none());

        // drop_too_late で期限切れの穴 (0x7FFF_FFFE) を loss_list から除去する。
        let dropped = buf.drop_too_late(late_now);
        assert_eq!(dropped, vec![0x7FFF_FFFE]);

        // 穴が消えた後、後続が循環順で配信される。
        let mut popped = Vec::new();
        while let Some(pkt) = buf.pop_ready(late_now) {
            popped.push(pkt.sequence_number);
        }
        assert_eq!(popped, vec![0x7FFF_FFFF, 0, 1]);
    }

    #[test]
    fn test_wrapping_period_delivery_time_compensation() {
        // ラップ後パケットの配信時刻に MAX_TIMESTAMP + 1 が加算されることを検証する。
        let start = Timestamp::from_micros(0);
        let tsbpd_time_base = 500_000;
        let mut buf = ReceiverBuffer::new(1000, 120, start, tsbpd_time_base);

        // ラップ前窗口のパケットを受信して wrapping_period_active を有効化する。
        // WRAPPING_PERIOD_START = MAX_TIMESTAMP - 30_000_000
        let wrap_start_ts = WRAPPING_PERIOD_START as u32;
        let now = Timestamp::from_micros(1_000_000);
        buf.receive(make_packet(1000, wrap_start_ts), now);
        assert!(buf.wrapping_period_active);

        // ラップ後パケット (ts = 10_000_000 < WRAPPING_PERIOD_START) の配信時刻は
        // tsbpd_time_base + ts + MAX_TIMESTAMP + 1 + tsbpd_delay_us になる。
        let post_wrap_ts: u32 = 10_000_000;
        buf.receive(make_packet(1001, post_wrap_ts), now);

        // 配信時刻が正しく補正されていることを確認する。
        // 補正あり: delivery_time = 500_000 + 10_000_000 + (MAX_TIMESTAMP + 1) + 120_000
        //          = 500_000 + 10_000_000 + 4_294_967_296 + 120_000
        //          = 4_305_587_296 μs
        // 補正なし: delivery_time = 500_000 + 10_000_000 + 120_000 = 10_620_000 μs
        // 補正なしの時刻では即時配信されるが、補正ありの時刻では配信されないことを確認する。
        let early = Timestamp::from_micros(10_620_000);
        assert!(
            buf.pop_ready(early).is_none(),
            "補正後の配信時刻は未来のはず"
        );

        // 補正後の配信時刻を超えると配信される。
        // 1000 (ラップ前) と 1001 (ラップ後、補正あり) の両方が配信される。
        let late = Timestamp::from_micros(4_305_588_000);
        let pkt0 = buf.pop_ready(late);
        assert!(pkt0.is_some(), "ラップ前パケットが配信されるはず");
        assert_eq!(pkt0.expect("配信パケット").sequence_number, 1000);

        let pkt1 = buf.pop_ready(late);
        assert!(pkt1.is_some(), "ラップ後パケットが配信されるはず");
        assert_eq!(pkt1.expect("配信パケット").sequence_number, 1001);
    }

    #[test]
    fn test_wrapping_period_drop_too_late_fallback() {
        // wrapping_period_active が有効な場合、drop_too_late のフォールバック式に
        // MAX_TIMESTAMP + 1 が加算されることを検証する。
        let start = Timestamp::from_micros(0);
        let tsbpd_time_base = 500_000;
        let mut buf = ReceiverBuffer::new(1000, 120, start, tsbpd_time_base);

        // ラップ前窗口のパケットを受信して wrapping_period_active を有効化する。
        let wrap_start_ts = WRAPPING_PERIOD_START as u32;
        let now = Timestamp::from_micros(1_000_000);
        buf.receive(make_packet(1000, wrap_start_ts), now);

        // 1002 を受信して 1001 を損失として登録する。
        buf.receive(make_packet(1002, 200_000), now);
        assert!(buf.loss_list.contains(&1001));

        // wrapping_period_active が有効な場合、次側パケット seq 1002 の delivery_time が
        // 推定配信時刻として使用される。seq 1002 の delivery_time はラップ補正付きで
        // = 500_000 + 200_000 + MAX_TIMESTAMP + 1 + 120_000 = 4_295_787_296 μs
        // TLPKTDROP = max(1.25 * 120_000, 1_000_000) = 1_000_000
        // now = 4_295_787_296 + 1_000_000 = 4_296_787_296 で削除される。
        // それより前の時刻では削除されない。
        let before = Timestamp::from_micros(4_295_787_000);
        let dropped = buf.drop_too_late(before);
        assert!(dropped.is_empty(), "閾値未満では削除されないはず");

        let after = Timestamp::from_micros(4_296_788_000);
        let dropped = buf.drop_too_late(after);
        assert_eq!(dropped, vec![1001], "閾値超過で削除されるはず");
    }

    #[test]
    fn test_wrapping_period_end_in_pop_ready() {
        // pop_ready() 内で終了窗口パケット (ts が 30〜60 秒の範囲) が配信されたときに
        // wrapping_period_active が false になり tsbpd_time_base が更新されることを検証する。
        let start = Timestamp::from_micros(0);
        let tsbpd_time_base = 0;
        let mut buf = ReceiverBuffer::new(1000, 120, start, tsbpd_time_base);

        // ラップ前窗口のパケットを受信して wrapping_period_active を有効化する。
        let wrap_start_ts = WRAPPING_PERIOD_START as u32;
        let now = Timestamp::from_micros(1_000_000);
        buf.receive(make_packet(1000, wrap_start_ts), now);
        assert!(buf.wrapping_period_active);

        // 終了窗口パケット (ts = 40_000_000, 40 秒) を受信する。
        // これは (WRAPPING_PERIOD_END_MIN..WRAPPING_PERIOD_END_MAX) の範囲内。
        // ラップ後パケットのため配信時刻に MAX_TIMESTAMP + 1 が加算される。
        let end_ts: u32 = 40_000_000;
        buf.receive(make_packet(1001, end_ts), now);

        // 両パケットの配信時刻:
        // 1000: delivery_time = 0 + WRAPPING_PERIOD_START + 120_000 = 4_265_087_295
        // 1001: delivery_time = 0 + 40_000_000 + MAX_TIMESTAMP + 1 + 120_000 = 4_335_087_296
        // 両パケットの配信時刻を超える時刻で配信する。
        // 1000 が配信された後、1001 が配信されると pop_ready() 内で終了判定が発火する。
        let late = Timestamp::from_micros(4_335_088_000);
        let pkt0 = buf.pop_ready(late);
        assert!(pkt0.is_some(), "1000 が配信されるはず");
        assert_eq!(pkt0.expect("配信パケット").sequence_number, 1000);

        // 1001 を配信する。終了判定が発火し wrapping_period_active が false になる。
        // tsbpd_time_base は private フィールドのため直接検証できないが、
        // 終了判定のコード (self.tsbpd_time_base += MAX_TIMESTAMP + 1) と
        // wrapping_period_active の変化で間接的に検証する。
        let pkt1 = buf.pop_ready(late);
        assert!(pkt1.is_some(), "1001 が配信されるはず");
        assert_eq!(pkt1.expect("配信パケット").sequence_number, 1001);
        assert!(
            !buf.wrapping_period_active,
            "終了判定が発火し wrapping_period_active が false になるはず"
        );
    }

    #[test]
    fn test_wrapping_period_no_end_in_pop_ready_when_tsbpd_disabled() {
        // TSBPD 無効時は pop_ready() 内の終了判定が発火しないことを検証する。
        // TSBPD 無効時は delivery_time = now なので、終了窗口パケット (ts が 30〜60 秒) も
        // 即時配信可能になる。その配信時に終了判定が発火しないことを確認する。
        let start = Timestamp::from_micros(0);
        let tsbpd_time_base = 500_000;
        let mut buf = ReceiverBuffer::new(1000, 120, start, tsbpd_time_base);

        // ラップ前窗口のパケットを受信して wrapping_period_active を有効化する。
        let wrap_start_ts = WRAPPING_PERIOD_START as u32;
        let now = Timestamp::from_micros(1_000_000);
        buf.receive(make_packet(1000, wrap_start_ts), now);
        assert!(buf.wrapping_period_active);

        // 終了窗口パケット (ts = 40_000_000) を受信する。
        let end_ts: u32 = 40_000_000;
        buf.receive(make_packet(1001, end_ts), now);

        // TSBPD を無効化する。これにより delivery_time = now となり即時配信可能になる。
        buf.set_tsbpd_enabled(false);

        // 1000 を配信する。終了判定は発火しない。
        let pkt0 = buf.pop_ready(now);
        assert!(pkt0.is_some(), "1000 が配信されるはず");
        assert!(
            buf.wrapping_period_active,
            "TSBPD 無効時は終了判定が発火しないはず"
        );

        // 1001 (終了窗口パケット) を配信する。TSBPD 無効時は終了判定が発火しない。
        let pkt1 = buf.pop_ready(now);
        assert!(pkt1.is_some(), "1001 が配信されるはず");
        assert!(
            buf.wrapping_period_active,
            "TSBPD 無効時は終了窗口パケットでも終了判定が発火しないはず"
        );
    }
}
