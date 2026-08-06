//! SRT Listener サンプル
//!
//! MPEG2-TS データを受信してファイルとして保存
//!
//! Usage:
//!   cargo run -p srt-listener -- --port 9000
//!   cargo run -p srt-listener -- --port 9000 --mp4
//!   cargo run -p srt-listener -- --port 9000 --passphrase secret

use shiguredo_srt::{
    ConnectionEvent, ConnectionOptions, ConnectionOutput, ConnectionState, KeyLength,
    SrtConnection, TimerId, Timestamp,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::time::Instant;
use tracing::{error, info, warn};

mod ts_to_mp4;

/// 統計表示間隔 (秒)
const STATS_INTERVAL_SECS: u64 = 5;

/// UDP 受信バッファサイズ
const UDP_RECV_BUF_SIZE: usize = 1500;

struct Args {
    host: String,
    port: u16,
    passphrase: Option<String>,
    mp4: bool,
    debug: bool,
}

fn parse_args() -> noargs::Result<Option<Args>> {
    let mut args = noargs::raw_args();
    args.metadata_mut().app_name = "srt-listener";
    args.metadata_mut().app_description = "SRT Listener - Receives MPEG2-TS and saves as MP4";

    noargs::HELP_FLAG.take_help(&mut args);

    let host: String = noargs::opt("host")
        .doc("Bind address")
        .default("0.0.0.0")
        .take(&mut args)
        .then(|o| o.value().parse())?;

    let port: u16 = noargs::opt("port")
        .short('p')
        .doc("Bind port")
        .default("9000")
        .take(&mut args)
        .then(|o| o.value().parse())?;

    let passphrase: Option<String> = noargs::opt("passphrase")
        .doc("Encryption passphrase")
        .take(&mut args)
        .present_and_then(|o| o.value().parse())?;

    let mp4: bool = noargs::flag("mp4")
        .doc("Also save as MP4 (in addition to TS)")
        .take(&mut args)
        .present()
        .is_some();

    let debug: bool = noargs::flag("debug")
        .doc("Enable debug output")
        .take(&mut args)
        .present()
        .is_some();

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(None);
    }

    Ok(Some(Args {
        host,
        port,
        passphrase,
        mp4,
        debug,
    }))
}

/// 統計情報を表示
fn print_receiver_stats(conn: &SrtConnection) {
    if let Some(stats) = conn.receiver_stats() {
        let loss_rate = stats.loss_rate_percent_x100 as f64 / 100.0;
        let rtt_ms = stats.rtt as f64 / 1000.0;
        let jitter_ms = stats.jitter as f64 / 1000.0;
        info!(
            "stats: recv: {} pkts ({} bytes), lost: {}, loss: {:.2}%, RTT: {:.1}ms, jitter: {:.1}ms",
            stats.total_received,
            stats.total_bytes_received,
            stats.total_lost,
            loss_rate,
            rtt_ms,
            jitter_ms
        );
    }
}

/// 出力ファイル名のベースを生成
fn generate_output_basename() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    format!("srt_{}", timestamp)
}

/// 受信データを保存
fn save_received_data(ts_buffer: &[u8], save_mp4: bool) {
    if ts_buffer.is_empty() {
        warn!("no data received, skipping save");
        return;
    }

    let basename = generate_output_basename();

    // TS ファイルを保存 (デフォルト)
    let ts_path = PathBuf::from(format!("{}.ts", basename));
    match std::fs::write(&ts_path, ts_buffer) {
        Ok(()) => {
            info!(
                "ts: saved {} bytes to {}",
                ts_buffer.len(),
                ts_path.display()
            );
        }
        Err(e) => {
            error!("ts: failed to save: {}", e);
        }
    }

    // MP4 に変換して保存 (オプション)
    if save_mp4 {
        let mp4_path = PathBuf::from(format!("{}.mp4", basename));
        info!("mp4: converting to MP4...");
        match ts_to_mp4::convert_ts_to_mp4(ts_buffer, &mp4_path) {
            Ok(()) => {
                info!("mp4: saved to {}", mp4_path.display());
            }
            Err(e) => {
                error!("mp4: conversion failed: {}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(args) = parse_args().map_err(|e| format!("{e:?}"))? else {
        return Ok(());
    };

    // ライブラリのデバッグ出力は tracing-subscriber で制御する。
    // RUST_LOG が設定されていればそれを優先する。ライブラリのログは debug レベルのため、
    // --debug 指定時のみ shiguredo_srt を debug まで出す (既定の info ではライブラリログは出ない)。
    let default_filter = if args.debug {
        "shiguredo_srt=debug"
    } else {
        "info"
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
        )
        .init();

    let bind_addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    let save_mp4 = args.mp4;

    info!("listening on {}", bind_addr);
    if args.passphrase.is_some() {
        info!("encryption: enabled (AES-128)");
    }
    if save_mp4 {
        info!("output: TS + MP4");
    } else {
        info!("output: TS");
    }

    let socket = UdpSocket::bind(bind_addr).await?;
    let mut buf = vec![0u8; UDP_RECV_BUF_SIZE];

    // 乱数を生成
    fn rand_u32() -> u32 {
        let mut bytes = [0u8; 4];
        aws_lc_rs::rand::fill(&mut bytes).expect("failed to generate random bytes");
        u32::from_le_bytes(bytes)
    }

    // SRT 接続オプション
    let options = ConnectionOptions {
        socket_id: rand_u32() & 0x7FFF_FFFF,
        initial_seq: Some(rand_u32() & 0x7FFF_FFFF),
        syn_cookie: Some(rand_u32()),
        passphrase: args.passphrase,
        key_length: KeyLength::Aes128,
        ..Default::default()
    };

    let mut conn = SrtConnection::new_listener(options);
    let mut peer_addr: Option<SocketAddr> = None;

    // タイマー管理
    let mut timers: HashMap<TimerId, Instant> = HashMap::new();
    let base_time = Instant::now();

    fn now_timestamp(base: Instant) -> Timestamp {
        let elapsed = base.elapsed();
        Timestamp::from_micros(elapsed.as_micros() as u64)
    }

    let mut connected = false;

    // MPEG2-TS データバッファ
    let mut ts_buffer: Vec<u8> = Vec::new();

    // 統計表示用インターバル
    let mut stats_interval = tokio::time::interval(Duration::from_secs(STATS_INTERVAL_SECS));
    stats_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        // 次のタイマー期限を計算
        let next_timer = timers
            .iter()
            .min_by_key(|&(_, deadline)| *deadline)
            .map(|(&id, &deadline)| (id, deadline));

        let timeout_duration = next_timer
            .map(|(_, deadline)| {
                deadline
                    .checked_duration_since(Instant::now())
                    .unwrap_or(Duration::ZERO)
            })
            .unwrap_or(Duration::from_secs(60));

        tokio::select! {
            // UDP パケット受信
            result = socket.recv_from(&mut buf) => {
                let (len, addr) = result?;

                // 初回接続時にピアアドレスを記録
                if peer_addr.is_none() {
                    peer_addr = Some(addr);
                    info!("connection from {}", addr);
                }

                let now = now_timestamp(base_time);
                if let Err(e) = conn.feed_recv_buf(&buf[..len], now) {
                    error!("recv error: {}", e);
                }
            }

            // タイマー発火
            _ = tokio::time::sleep(timeout_duration), if next_timer.is_some() => {
                let (timer_id, _) = next_timer
                    .expect("next timer should be present when the timer guard is active");
                timers.remove(&timer_id);

                let now = now_timestamp(base_time);
                if let Err(e) = conn.handle_timer(timer_id, now) {
                    error!("timer error: {}", e);
                }
            }

            // 統計表示
            _ = stats_interval.tick(), if connected => {
                print_receiver_stats(&conn);
            }
        }

        // イベント処理
        while let Some(event) = conn.poll_event() {
            match event {
                ConnectionEvent::Connected => {
                    info!("connected");
                    connected = true;
                    ts_buffer.clear();
                }
                ConnectionEvent::DataReceived { payload, .. } => {
                    // 受信データをバッファに追加
                    ts_buffer.extend_from_slice(&payload);
                }
                ConnectionEvent::StateChanged(state) => {
                    if state == ConnectionState::Disconnected {
                        info!("disconnected");
                        save_received_data(&ts_buffer, save_mp4);
                        return Ok(());
                    }
                }
                ConnectionEvent::Error(msg) => {
                    error!("connection error: {}", msg);
                }
                ConnectionEvent::Disconnected { reason } => {
                    info!("disconnected: {}", reason);
                    save_received_data(&ts_buffer, save_mp4);
                    return Ok(());
                }
                ConnectionEvent::KeyRefreshNeeded { .. } => {
                    // Listener は受信側のため、キーリフレッシュは Caller から行われる
                }
            }
        }

        // 出力処理
        while let Some(output) = conn.poll_output() {
            match output {
                ConnectionOutput::SendPacket(data) => {
                    if let Some(addr) = peer_addr {
                        socket.send_to(&data, addr).await?;
                    }
                }
                ConnectionOutput::SetTimer {
                    id,
                    duration_micros,
                } => {
                    let deadline = Instant::now() + Duration::from_micros(duration_micros);
                    timers.insert(id, deadline);
                }
                ConnectionOutput::ClearTimer { id } => {
                    timers.remove(&id);
                }
            }
        }
    }
}
