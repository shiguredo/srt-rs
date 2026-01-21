//! SRT 輻輳制御
//!
//! LiveCC (ライブストリーミング向け輻輳制御) を実装する。

use crate::time::Timestamp;

/// ACK 情報
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckInfo {
    /// 確認応答されたパケットシーケンス番号
    pub ack_seq_number: u32,
    /// RTT (マイクロ秒)
    pub rtt: u32,
    /// RTT 分散 (マイクロ秒)
    pub rtt_var: u32,
    /// 利用可能バッファサイズ
    pub available_buffer: u32,
    /// パケット受信レート (packets/sec)
    pub packet_recv_rate: u32,
    /// 推定リンク容量 (packets/sec)
    pub estimated_link_capacity: u32,
    /// 受信レート (bytes/sec)
    pub recv_rate: u32,
}

/// 輻輳制御トレイト
pub trait CongestionControl: std::fmt::Debug + Send {
    /// データパケット送信時
    fn on_packet_sent(&mut self, packet_size: usize, now: Timestamp);

    /// ACK 受信時
    fn on_ack_received(&mut self, ack_info: &AckInfo, now: Timestamp);

    /// NAK 受信時
    fn on_nak_received(&mut self, lost_packets: &[u32], now: Timestamp);

    /// タイムアウト時
    fn on_timeout(&mut self, now: Timestamp);

    /// 次のパケット送信までの待機時間 (マイクロ秒)
    fn packet_send_period(&self) -> u64;

    /// 輻輳ウィンドウサイズ (パケット数)
    fn congestion_window(&self) -> u32;

    /// 最大帯域幅を設定 (bytes/sec)
    fn set_max_bandwidth(&mut self, max_bw: u64);

    /// 入力帯域幅を設定 (bytes/sec)
    fn set_input_bandwidth(&mut self, input_bw: u64);

    /// オーバーヘッド率を設定 (パーセント)
    fn set_overhead(&mut self, overhead: u32);
}

/// LiveCC 帯域幅設定モード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BandwidthMode {
    /// 最大帯域幅を明示的に設定
    MaxBwSet,
    /// 入力帯域幅 + オーバーヘッドから計算
    InputBwSet,
    /// 入力帯域幅を推定 + オーバーヘッドから計算
    #[default]
    InputBwEstimated,
}

/// Live Congestion Control (LiveCC)
///
/// ライブストリーミング向けの輻輳制御。
/// リアルタイム性を重視し、バッファオーバーフロー/アンダーフローを防ぐ。
#[derive(Debug)]
pub struct LiveCc {
    /// 帯域幅モード
    bandwidth_mode: BandwidthMode,
    /// 最大帯域幅 (bytes/sec)
    max_bandwidth: u64,
    /// 入力帯域幅 (bytes/sec)
    input_bandwidth: u64,
    /// 推定入力帯域幅 (bytes/sec)
    estimated_input_bandwidth: u64,
    /// オーバーヘッド率 (パーセント)
    overhead: u32,
    /// 平均ペイロードサイズ
    avg_payload_size: u64,
    /// パケット送信間隔 (マイクロ秒)
    pkt_snd_period: u64,
    /// 輻輳ウィンドウサイズ
    cwnd: u32,
    /// RTT (マイクロ秒)
    rtt: u32,
    /// RTT 分散 (マイクロ秒)
    rtt_var: u32,
    /// 推定リンク容量 (packets/sec)
    estimated_link_capacity: u32,
    /// NAK による損失カウント (輻輳検出用)
    nak_loss_count: u32,
    /// 最後の NAK 受信時刻
    last_nak_time: Option<Timestamp>,
    /// 輻輳による送信間隔増加係数 (1.0 = 通常)
    congestion_factor: f64,
    /// 入力レート測定用: 送信バイト数
    sent_bytes: u64,
    /// 入力レート測定用: 計測開始時刻 (マイクロ秒)
    measurement_start_time: Option<u64>,
    /// 入力レート測定用: 計測期間 (マイクロ秒)
    measurement_period: u64,
}

impl LiveCc {
    /// SRT ヘッダサイズ
    const SRT_HEADER_SIZE: u64 = 16;

    /// デフォルトの最大帯域幅 (1 Gbps)
    const DEFAULT_MAX_BW: u64 = 1_000_000_000 / 8; // bytes/sec

    /// デフォルトオーバーヘッド率 (25%)
    const DEFAULT_OVERHEAD: u32 = 25;

    /// デフォルト輻輳ウィンドウサイズ
    const DEFAULT_CWND: u32 = 8192;

    /// 最小輻輳ウィンドウサイズ
    const MIN_CWND: u32 = 16;

    /// 最大輻輳ウィンドウサイズ
    const MAX_CWND: u32 = 8192;

    /// 初期 RTT (100ms)
    const INITIAL_RTT: u32 = 100_000;

    /// 初期 RTT 分散 (50ms)
    const INITIAL_RTT_VAR: u32 = 50_000;

    /// 輻輳検出の閾値 (この数以上の損失で輻輳と判断)
    const CONGESTION_THRESHOLD: u32 = 10;

    /// 輻輳係数の最大値
    const MAX_CONGESTION_FACTOR: f64 = 2.0;

    /// 輻輳係数の回復速度 (ACK 毎に減少)
    const CONGESTION_RECOVERY_RATE: f64 = 0.99;

    /// 輻輳リセットの間隔 (マイクロ秒)
    const CONGESTION_RESET_INTERVAL: u64 = 1_000_000;

    /// 入力レート測定期間 (1 秒)
    const INPUT_RATE_MEASUREMENT_PERIOD: u64 = 1_000_000;

    /// 新しい LiveCC を作成
    pub fn new() -> Self {
        Self {
            bandwidth_mode: BandwidthMode::InputBwEstimated,
            max_bandwidth: Self::DEFAULT_MAX_BW,
            input_bandwidth: 0,
            estimated_input_bandwidth: 0,
            overhead: Self::DEFAULT_OVERHEAD,
            avg_payload_size: 1316, // 典型的な TS パケットサイズ
            pkt_snd_period: 1,      // 初期値
            cwnd: Self::DEFAULT_CWND,
            rtt: Self::INITIAL_RTT,
            rtt_var: Self::INITIAL_RTT_VAR,
            estimated_link_capacity: 0,
            nak_loss_count: 0,
            last_nak_time: None,
            congestion_factor: 1.0,
            sent_bytes: 0,
            measurement_start_time: None,
            measurement_period: Self::INPUT_RATE_MEASUREMENT_PERIOD,
        }
    }

    /// 最大帯域幅を計算
    fn calculate_max_bandwidth(&self) -> u64 {
        let base_bw = match self.bandwidth_mode {
            BandwidthMode::MaxBwSet => self.max_bandwidth,
            BandwidthMode::InputBwSet => self.input_bandwidth * (100 + self.overhead as u64) / 100,
            BandwidthMode::InputBwEstimated => {
                self.estimated_input_bandwidth * (100 + self.overhead as u64) / 100
            }
        };

        // リンク容量推定がある場合、それを考慮して自動調整
        // リンク容量 (packets/sec) をバイト/秒に変換
        if self.estimated_link_capacity > 0 && self.bandwidth_mode != BandwidthMode::MaxBwSet {
            let link_capacity_bytes = self.estimated_link_capacity as u64
                * (self.avg_payload_size + Self::SRT_HEADER_SIZE);
            // リンク容量の 90% を上限とする (余裕を持たせる)
            let link_limit = link_capacity_bytes * 90 / 100;
            base_bw.min(link_limit).max(1)
        } else {
            base_bw.max(1)
        }
    }

    /// パケット送信間隔を更新
    fn update_pkt_snd_period(&mut self) {
        let max_bw = self.calculate_max_bandwidth();
        if max_bw > 0 {
            let pkt_size = self.avg_payload_size + Self::SRT_HEADER_SIZE;
            // PKT_SND_PERIOD = pkt_size * 1000000 / max_bw (マイクロ秒)
            self.pkt_snd_period = pkt_size * 1_000_000 / max_bw;
            if self.pkt_snd_period == 0 {
                self.pkt_snd_period = 1;
            }
        }
    }

    /// 入力帯域幅を推定
    fn update_estimated_input_bandwidth(&mut self, recv_rate: u64) {
        if self.estimated_input_bandwidth == 0 {
            self.estimated_input_bandwidth = recv_rate;
        } else {
            // 指数移動平均: 7/8 * old + 1/8 * new
            self.estimated_input_bandwidth = (self.estimated_input_bandwidth * 7 + recv_rate) / 8;
        }
    }

    /// 輻輳ウィンドウサイズを更新
    ///
    /// CWND = (RTT * link_capacity) / 1,000,000 + 16
    /// RTT はマイクロ秒、link_capacity は packets/sec
    fn update_cwnd(&mut self) {
        if self.estimated_link_capacity > 0 {
            // BDP (Bandwidth Delay Product) を計算
            // RTT (μs) * link_capacity (packets/sec) / 1,000,000 = packets in flight
            let bdp = (self.rtt as u64 * self.estimated_link_capacity as u64) / 1_000_000;
            // バッファ余裕として 16 パケット追加
            let cwnd = (bdp + 16) as u32;
            self.cwnd = cwnd.clamp(Self::MIN_CWND, Self::MAX_CWND);
        }
    }

    /// 入力レートを更新 (送信時に呼ばれる)
    fn update_input_rate(&mut self, packet_size: usize, now: Timestamp) {
        let now_micros = now.as_micros();

        match self.measurement_start_time {
            None => {
                // 計測開始
                self.measurement_start_time = Some(now_micros);
                self.sent_bytes = packet_size as u64;
            }
            Some(start_time) => {
                let elapsed = now_micros.saturating_sub(start_time);
                self.sent_bytes += packet_size as u64;

                // 計測期間が経過したらレートを計算
                if elapsed >= self.measurement_period {
                    // bytes/sec = sent_bytes * 1,000,000 / elapsed
                    let input_rate = self.sent_bytes * 1_000_000 / elapsed.max(1);

                    // InputBwEstimated モードの場合のみ更新
                    if self.bandwidth_mode == BandwidthMode::InputBwEstimated {
                        self.update_estimated_input_bandwidth(input_rate);
                        self.update_pkt_snd_period();
                    }

                    // 計測をリセット
                    self.measurement_start_time = Some(now_micros);
                    self.sent_bytes = 0;
                }
            }
        }
    }
}

impl Default for LiveCc {
    fn default() -> Self {
        Self::new()
    }
}

impl CongestionControl for LiveCc {
    fn on_packet_sent(&mut self, packet_size: usize, now: Timestamp) {
        // 平均ペイロードサイズを更新
        // AvgPayloadSize = 7/8 * AvgPayloadSize + 1/8 * PacketPayloadSize
        self.avg_payload_size = (self.avg_payload_size * 7 + packet_size as u64) / 8;

        // 入力レート測定を更新
        self.update_input_rate(packet_size, now);
    }

    fn on_ack_received(&mut self, ack_info: &AckInfo, _now: Timestamp) {
        // RTT 更新 (指数移動平均)
        if ack_info.rtt > 0 {
            self.rtt = ((self.rtt as u64 * 7 + ack_info.rtt as u64) / 8) as u32;
        }

        // RTT 分散更新
        if ack_info.rtt_var > 0 {
            self.rtt_var = ((self.rtt_var as u64 * 3 + ack_info.rtt_var as u64) / 4) as u32;
        }

        // 入力帯域幅推定を更新
        if ack_info.recv_rate > 0 {
            self.update_estimated_input_bandwidth(ack_info.recv_rate as u64);
        }

        // リンク容量推定を更新
        if ack_info.estimated_link_capacity > 0 {
            if self.estimated_link_capacity == 0 {
                self.estimated_link_capacity = ack_info.estimated_link_capacity;
            } else {
                // 指数移動平均: 7/8 * old + 1/8 * new
                self.estimated_link_capacity = ((self.estimated_link_capacity as u64 * 7
                    + ack_info.estimated_link_capacity as u64)
                    / 8) as u32;
            }
        }

        // 輻輳ウィンドウサイズを更新
        self.update_cwnd();

        // 輻輳係数を徐々に回復
        if self.congestion_factor > 1.0 {
            self.congestion_factor *= Self::CONGESTION_RECOVERY_RATE;
            if self.congestion_factor < 1.01 {
                self.congestion_factor = 1.0;
            }
        }

        // 損失カウントも減少
        self.nak_loss_count = self.nak_loss_count.saturating_sub(1);

        // パケット送信間隔を再計算
        self.update_pkt_snd_period();
    }

    fn on_nak_received(&mut self, lost_packets: &[u32], now: Timestamp) {
        let loss_count = lost_packets.len() as u32;

        // 前回の NAK から一定時間経過していれば損失カウントをリセット
        if let Some(last_time) = self.last_nak_time
            && now.as_micros().saturating_sub(last_time.as_micros())
                > Self::CONGESTION_RESET_INTERVAL
        {
            self.nak_loss_count = 0;
        }

        self.nak_loss_count = self.nak_loss_count.saturating_add(loss_count);
        self.last_nak_time = Some(now);

        // 損失カウントが閾値を超えたら輻輳と判断
        if self.nak_loss_count >= Self::CONGESTION_THRESHOLD {
            // 輻輳係数を増加 (送信間隔を伸ばす)
            let increase = 1.0 + (self.nak_loss_count as f64 / 100.0);
            self.congestion_factor =
                (self.congestion_factor * increase).min(Self::MAX_CONGESTION_FACTOR);
        }
    }

    fn on_timeout(&mut self, _now: Timestamp) {
        // パケット送信間隔を再計算
        self.update_pkt_snd_period();
    }

    fn packet_send_period(&self) -> u64 {
        // 輻輳係数を考慮した送信間隔
        (self.pkt_snd_period as f64 * self.congestion_factor) as u64
    }

    fn congestion_window(&self) -> u32 {
        self.cwnd
    }

    fn set_max_bandwidth(&mut self, max_bw: u64) {
        self.max_bandwidth = max_bw;
        self.bandwidth_mode = BandwidthMode::MaxBwSet;
        self.update_pkt_snd_period();
    }

    fn set_input_bandwidth(&mut self, input_bw: u64) {
        self.input_bandwidth = input_bw;
        self.bandwidth_mode = BandwidthMode::InputBwSet;
        self.update_pkt_snd_period();
    }

    fn set_overhead(&mut self, overhead: u32) {
        self.overhead = overhead;
        self.update_pkt_snd_period();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_live_cc_new() {
        let cc = LiveCc::new();
        assert_eq!(cc.bandwidth_mode, BandwidthMode::InputBwEstimated);
        assert!(cc.packet_send_period() > 0);
        assert!(cc.congestion_window() > 0);
    }

    #[test]
    fn test_live_cc_set_max_bandwidth() {
        let mut cc = LiveCc::new();
        cc.set_max_bandwidth(10_000_000); // 10 MB/s
        assert_eq!(cc.bandwidth_mode, BandwidthMode::MaxBwSet);
        assert!(cc.packet_send_period() > 0);
    }

    #[test]
    fn test_live_cc_on_packet_sent() {
        let mut cc = LiveCc::new();
        let now = Timestamp::from_micros(0);
        cc.on_packet_sent(1000, now);
        cc.on_packet_sent(1200, now);
        // 平均ペイロードサイズが更新されていることを確認
        assert!(cc.avg_payload_size > 0);
    }

    #[test]
    fn test_live_cc_on_ack_received() {
        let mut cc = LiveCc::new();
        let now = Timestamp::from_micros(1000);

        let ack_info = AckInfo {
            ack_seq_number: 100,
            rtt: 50_000,
            rtt_var: 10_000,
            available_buffer: 1000,
            packet_recv_rate: 1000,
            estimated_link_capacity: 10000,
            recv_rate: 1_000_000,
        };

        cc.on_ack_received(&ack_info, now);

        // RTT が更新されていることを確認
        assert!(cc.rtt != LiveCc::INITIAL_RTT);
    }

    #[test]
    fn test_live_cc_on_nak_received() {
        let mut cc = LiveCc::new();
        // 帯域幅を設定して pkt_snd_period を適切な値にする
        cc.set_max_bandwidth(10_000_000); // 10 MB/s
        let now = Timestamp::from_micros(1000);

        let initial_factor = cc.congestion_factor;

        // 少数の損失では輻輳係数は変わらない
        let lost_packets: Vec<u32> = (100..105).collect();
        cc.on_nak_received(&lost_packets, now);
        assert_eq!(cc.congestion_factor, 1.0);

        // 閾値を超える損失で輻輳係数が増加
        let lost_packets: Vec<u32> = (200..220).collect();
        cc.on_nak_received(&lost_packets, now);
        assert!(cc.congestion_factor > initial_factor);
    }

    #[test]
    fn test_live_cc_congestion_recovery() {
        let mut cc = LiveCc::new();
        let now = Timestamp::from_micros(1000);

        // 輻輳状態を作る
        let lost_packets: Vec<u32> = (100..150).collect();
        cc.on_nak_received(&lost_packets, now);
        let congested_factor = cc.congestion_factor;
        assert!(congested_factor > 1.0);

        // ACK を受信すると輻輳係数が徐々に回復
        let ack_info = AckInfo {
            ack_seq_number: 100,
            rtt: 50_000,
            rtt_var: 10_000,
            available_buffer: 1000,
            packet_recv_rate: 1000,
            estimated_link_capacity: 10000,
            recv_rate: 1_000_000,
        };

        for _ in 0..100 {
            cc.on_ack_received(&ack_info, now);
        }

        // 輻輳係数が回復していることを確認
        assert!(cc.congestion_factor < congested_factor);
    }

    #[test]
    fn test_live_cc_cwnd_dynamic_adjustment() {
        let mut cc = LiveCc::new();
        let now = Timestamp::from_micros(1000);

        // 初期 CWND はデフォルト値
        assert_eq!(cc.congestion_window(), LiveCc::DEFAULT_CWND);

        // リンク容量を設定した ACK を受信
        let ack_info = AckInfo {
            ack_seq_number: 100,
            rtt: 50_000, // 50ms
            rtt_var: 10_000,
            available_buffer: 1000,
            packet_recv_rate: 1000,
            estimated_link_capacity: 10_000, // 10000 packets/sec
            recv_rate: 1_000_000,
        };

        cc.on_ack_received(&ack_info, now);

        // CWND が計算されていることを確認
        // BDP = 50_000 * 10_000 / 1_000_000 = 500 packets
        // CWND = 500 + 16 = 516
        let cwnd = cc.congestion_window();
        assert!(cwnd >= LiveCc::MIN_CWND);
        assert!(cwnd <= LiveCc::MAX_CWND);
        // 計算値に近いことを確認 (EWMA の影響があるため厳密ではない)
        assert!(cwnd > 100 && cwnd < 1000);
    }

    #[test]
    fn test_live_cc_input_rate_measurement() {
        let mut cc = LiveCc::new();

        // InputBwEstimated モードであることを確認
        assert_eq!(cc.bandwidth_mode, BandwidthMode::InputBwEstimated);

        // 初期状態では推定入力帯域幅は 0
        assert_eq!(cc.estimated_input_bandwidth, 0);

        // 1 秒間に 1MB のデータを送信するシミュレーション
        // 1000 パケット x 1000 バイト = 1MB
        let packet_size = 1000;
        let packets_per_second = 1000;
        let measurement_period = LiveCc::INPUT_RATE_MEASUREMENT_PERIOD;

        // 計測期間中にパケットを送信
        for i in 0..packets_per_second {
            let time_offset = (measurement_period * i as u64) / packets_per_second as u64;
            let now = Timestamp::from_micros(time_offset);
            cc.on_packet_sent(packet_size, now);
        }

        // 計測期間終了後にもう 1 パケット送信してレート計算をトリガー
        let now = Timestamp::from_micros(measurement_period + 1000);
        cc.on_packet_sent(packet_size, now);

        // 推定入力帯域幅が計算されていることを確認
        // 期待値: 約 1MB/s = 1,000,000 bytes/sec
        assert!(cc.estimated_input_bandwidth > 0);
        // 誤差を考慮して範囲チェック
        assert!(
            cc.estimated_input_bandwidth > 500_000 && cc.estimated_input_bandwidth < 2_000_000,
            "estimated_input_bandwidth: {}",
            cc.estimated_input_bandwidth
        );
    }

    #[test]
    fn test_live_cc_max_bw_auto_adjustment() {
        let mut cc = LiveCc::new();
        let now = Timestamp::from_micros(1000);

        // 高い入力帯域幅を設定
        cc.set_input_bandwidth(100_000_000); // 100 MB/s
        let period_before = cc.packet_send_period();

        // リンク容量が低い ACK を受信
        let ack_info = AckInfo {
            ack_seq_number: 100,
            rtt: 50_000,
            rtt_var: 10_000,
            available_buffer: 1000,
            packet_recv_rate: 1000,
            estimated_link_capacity: 1000, // 1000 packets/sec (~1.3 MB/s)
            recv_rate: 1_000_000,
        };

        // InputBwSet モードなのでリンク容量による制限は適用されない
        assert_eq!(cc.bandwidth_mode, BandwidthMode::InputBwSet);

        // InputBwEstimated モードに切り替え
        cc.bandwidth_mode = BandwidthMode::InputBwEstimated;
        cc.estimated_input_bandwidth = 100_000_000; // 100 MB/s
        cc.on_ack_received(&ack_info, now);

        // リンク容量による制限が適用されて送信間隔が長くなる
        let period_after = cc.packet_send_period();
        assert!(
            period_after > period_before,
            "period_before: {}, period_after: {}",
            period_before,
            period_after
        );
    }
}
