//! SRT Caller サンプル
//!
//! MP4 ファイルを MPEG2-TS に変換して SRT で送信する
//!
//! Usage:
//!   cargo run -p srt-caller -- --host 127.0.0.1 --port 9000 --input video.mp4
//!   cargo run -p srt-caller -- --host 127.0.0.1 --port 9000 --input video.mp4 --passphrase secret
//!   cat video.ts | cargo run -p srt-caller -- --host 127.0.0.1 --port 9000

use std::collections::HashMap;
use std::io::Read;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use shiguredo_srt::{
    ConnectionEvent, ConnectionOptions, ConnectionOutput, ConnectionState, KeyLength,
    SrtConnection, TimerId, Timestamp,
};
use tokio::net::UdpSocket;
use tokio::time::Instant;
use tracing::{error, info};

mod mp4_to_ts;

/// 統計表示間隔 (秒)
const STATS_INTERVAL_SECS: u64 = 5;

/// UDP 受信バッファサイズ
const UDP_RECV_BUF_SIZE: usize = 1500;

/// SRT ペイロードサイズ (7 TS パケット)
const SRT_PAYLOAD_SIZE: usize = 1316;

struct Args {
    host: String,
    port: u16,
    passphrase: Option<String>,
    input: Option<PathBuf>,
    save_ts: Option<PathBuf>,
    debug: bool,
}

fn parse_args() -> noargs::Result<Option<Args>> {
    let mut args = noargs::raw_args();
    args.metadata_mut().app_name = "srt-caller";
    args.metadata_mut().app_description =
        "SRT Caller - Reads MP4/TS data and sends MPEG2-TS to the SRT listener";

    noargs::HELP_FLAG.take_help(&mut args);

    let host: String = noargs::opt("host")
        .doc("Target host")
        .default("127.0.0.1")
        .take(&mut args)
        .then(|o| o.value().parse())?;

    let port: u16 = noargs::opt("port")
        .short('p')
        .doc("Target port")
        .default("9000")
        .take(&mut args)
        .then(|o| o.value().parse())?;

    let passphrase: Option<String> = noargs::opt("passphrase")
        .doc("Encryption passphrase")
        .take(&mut args)
        .present_and_then(|o| o.value().parse())?;

    let input: Option<PathBuf> = noargs::opt("input")
        .short('i')
        .doc("Input MP4 file (if not specified, reads from stdin)")
        .take(&mut args)
        .present_and_then(|o| o.value().parse())?;

    let save_ts: Option<PathBuf> = noargs::opt("save-ts")
        .doc("Save generated TS to file (for debugging)")
        .take(&mut args)
        .present_and_then(|o| o.value().parse())?;

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
        input,
        save_ts,
        debug,
    }))
}

/// 統計情報を表示
fn print_sender_stats(conn: &SrtConnection) {
    if let Some(stats) = conn.sender_stats() {
        info!(
            "stats: sent: {} pkts ({} bytes), retransmits: {}, in_buffer: {}, in_loss_list: {}",
            stats.total_sent,
            stats.total_bytes_sent,
            stats.total_retransmits,
            stats.packets_in_buffer,
            stats.packets_in_loss_list
        );
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

    let target_addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    info!("connecting to {}", target_addr);
    if args.passphrase.is_some() {
        info!("encryption: enabled (AES-128)");
    }

    // ローカルポートを自動割り当て
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect(target_addr).await?;

    let mut buf = vec![0u8; UDP_RECV_BUF_SIZE];

    // 乱数を生成
    fn rand_u32() -> u32 {
        let mut bytes = [0u8; 4];
        getrandom::fill(&mut bytes).expect("failed to generate random bytes");
        u32::from_le_bytes(bytes)
    }

    // 暗号化用の乱数を生成
    let (crypto_salt, crypto_sek) = if args.passphrase.is_some() {
        let mut salt = [0u8; 16];
        let mut sek = vec![0u8; KeyLength::Aes128.len()];
        getrandom::fill(&mut salt).expect("failed to generate random salt");
        getrandom::fill(&mut sek).expect("failed to generate random SEK");
        (Some(salt), Some(sek))
    } else {
        (None, None)
    };

    // SRT 接続オプション
    let options = ConnectionOptions {
        socket_id: rand_u32() & 0x7FFF_FFFF,
        initial_seq: Some(rand_u32() & 0x7FFF_FFFF),
        passphrase: args.passphrase,
        crypto_salt,
        crypto_sek,
        key_length: KeyLength::Aes128,
        ..Default::default()
    };

    let mut conn = SrtConnection::new_caller(options);

    // タイマー管理
    let mut timers: HashMap<TimerId, Instant> = HashMap::new();
    let base_time = Instant::now();

    fn now_timestamp(base: Instant) -> Timestamp {
        let elapsed = base.elapsed();
        Timestamp::from_micros(elapsed.as_micros() as u64)
    }

    // 接続開始
    let now = now_timestamp(base_time);
    conn.connect(now)?;

    // 初期パケット (INDUCTION) を送信
    while let Some(output) = conn.poll_output() {
        match output {
            ConnectionOutput::SendPacket(data) => {
                socket.send(&data).await?;
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

    // データソースからデータを読み込むチャンネル
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(16);

    if let Some(input_path) = args.input {
        // MP4 ファイルから MPEG2-TS に変換して送信
        info!("input: {} (MP4 -> MPEG2-TS)", input_path.display());

        let tx = tx.clone();
        let save_ts = args.save_ts.clone();
        std::thread::spawn(move || {
            match mp4_to_ts::Mp4ToTsConverter::from_file(&input_path) {
                Ok(mut converter) => match converter.convert() {
                    Ok(ts_data) => {
                        info!(
                            "mp4->ts: converted {} bytes of MPEG2-TS data",
                            ts_data.len()
                        );

                        // デバッグ用: TS ファイルを保存
                        if let Some(ref ts_path) = save_ts {
                            match std::fs::write(ts_path, &ts_data) {
                                Ok(()) => info!("mp4->ts: saved to: {}", ts_path.display()),
                                Err(e) => error!("mp4->ts: failed to save: {}", e),
                            }
                        }

                        // TS パケット単位 (188 バイト) で送信
                        // SRT ペイロードサイズ (1316 バイト = 7 TS パケット)
                        const TS_PACKET_SIZE: usize = 188;
                        const PACKETS_PER_SEND: usize = 7;
                        const CHUNK_SIZE: usize = TS_PACKET_SIZE * PACKETS_PER_SEND;

                        for chunk in ts_data.chunks(CHUNK_SIZE) {
                            if tx.blocking_send(chunk.to_vec()).is_err() {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("mp4->ts: conversion error: {}", e);
                    }
                },
                Err(e) => {
                    error!("mp4->ts: failed to open file: {}", e);
                }
            }
        });
    } else {
        // 標準入力からデータを読み込む
        info!("input: stdin (MPEG2-TS)");

        std::thread::spawn(move || {
            let mut stdin = std::io::stdin();
            let mut buf = vec![0u8; SRT_PAYLOAD_SIZE];
            loop {
                match stdin.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if tx.blocking_send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("stdin read error: {}", e);
                        break;
                    }
                }
            }
        });
    }

    let mut connected = false;

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
            result = socket.recv(&mut buf) => {
                let len = result?;
                let now = now_timestamp(base_time);
                if let Err(e) = conn.feed_recv_buf(&buf[..len], now) {
                    error!("recv error: {}", e);
                }
            }

            // データ受信
            result = rx.recv(), if connected && conn.can_send() => {
                match result {
                    Some(data) => {
                        let now = now_timestamp(base_time);
                        if let Err(e) = conn.send(&data, now) {
                            error!("send error: {}", e);
                        }
                    }
                    None => {
                        // チャンネルが閉じた = 全データ送信完了
                        info!("all data sent, closing connection...");
                        let now = now_timestamp(base_time);
                        conn.disconnect(now);
                    }
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
                print_sender_stats(&conn);
            }
        }

        // イベント処理
        while let Some(event) = conn.poll_event() {
            match event {
                ConnectionEvent::Connected => {
                    info!("connected");
                    connected = true;
                }
                ConnectionEvent::DataReceived { .. } => {
                    // Caller は送信専用のため、受信データは無視
                }
                ConnectionEvent::StateChanged(state) => {
                    if state == ConnectionState::Disconnected {
                        info!("disconnected");
                        return Ok(());
                    }
                }
                ConnectionEvent::Error(msg) => {
                    error!("connection error: {}", msg);
                }
                ConnectionEvent::Disconnected { reason } => {
                    info!("disconnected: {}", reason);
                    return Ok(());
                }
                ConnectionEvent::KeyRefreshNeeded { key_length } => {
                    // 新しい SEK を生成してキーリフレッシュを実行
                    let mut new_sek = vec![0u8; key_length];
                    getrandom::fill(&mut new_sek).expect("failed to generate random SEK");
                    let now = now_timestamp(base_time);
                    if let Err(e) = conn.provide_new_sek(&new_sek, now) {
                        error!("key refresh failed: {}", e);
                    }
                }
            }
        }

        // 出力処理
        while let Some(output) = conn.poll_output() {
            match output {
                ConnectionOutput::SendPacket(data) => {
                    socket.send(&data).await?;
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
