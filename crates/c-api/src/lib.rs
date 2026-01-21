//! SRT C-API
//!
//! SRT ライブラリの C 言語バインディング

#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::{CStr, c_char};
use std::ptr;
use std::slice;

use shiguredo_srt::{
    ConnectionEvent, ConnectionOptions, ConnectionOutput, ConnectionState, KeyLength,
    SrtConnection, Timestamp,
};

/// バージョン文字列
#[unsafe(no_mangle)]
pub extern "C" fn srt_version() -> *const c_char {
    c"2026.0.0".as_ptr()
}

/// 接続状態
#[repr(C)]
pub enum SrtConnectionState {
    Disconnected = 0,
    Induction = 1,
    Conclusion = 2,
    Listening = 3,
    Connected = 4,
    Closing = 5,
}

impl From<ConnectionState> for SrtConnectionState {
    fn from(state: ConnectionState) -> Self {
        match state {
            ConnectionState::Disconnected => Self::Disconnected,
            ConnectionState::Induction => Self::Induction,
            ConnectionState::Conclusion => Self::Conclusion,
            ConnectionState::Listening => Self::Listening,
            ConnectionState::Connected => Self::Connected,
            ConnectionState::Closing => Self::Closing,
        }
    }
}

/// 接続オプション
#[repr(C)]
pub struct SrtConnectionOptions {
    pub socket_id: u32,
    pub passphrase: *const c_char,
    pub key_length: u8,
    pub tsbpd_delay: u16,
}

/// SRT 接続ハンドル
pub struct SrtConnectionHandle {
    inner: SrtConnection,
}

/// Caller として接続を作成
///
/// # Safety
/// options が非 null の場合、有効なポインタであること
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srt_connection_new_caller(
    options: *const SrtConnectionOptions,
) -> *mut SrtConnectionHandle {
    let options = if options.is_null() {
        ConnectionOptions::default()
    } else {
        let opts = &*options;
        let passphrase = if opts.passphrase.is_null() {
            None
        } else {
            CStr::from_ptr(opts.passphrase)
                .to_str()
                .ok()
                .map(|s| s.to_string())
        };
        let key_length = match opts.key_length {
            32 => KeyLength::Aes256,
            _ => KeyLength::Aes128,
        };
        ConnectionOptions {
            socket_id: opts.socket_id,
            passphrase,
            key_length,
            tsbpd_delay: opts.tsbpd_delay,
            ..ConnectionOptions::default()
        }
    };

    let conn = SrtConnection::new_caller(options);
    Box::into_raw(Box::new(SrtConnectionHandle { inner: conn }))
}

/// Listener として接続を作成
///
/// # Safety
/// options が非 null の場合、有効なポインタであること
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srt_connection_new_listener(
    options: *const SrtConnectionOptions,
) -> *mut SrtConnectionHandle {
    let options = if options.is_null() {
        ConnectionOptions::default()
    } else {
        let opts = &*options;
        let passphrase = if opts.passphrase.is_null() {
            None
        } else {
            CStr::from_ptr(opts.passphrase)
                .to_str()
                .ok()
                .map(|s| s.to_string())
        };
        let key_length = match opts.key_length {
            32 => KeyLength::Aes256,
            _ => KeyLength::Aes128,
        };
        ConnectionOptions {
            socket_id: opts.socket_id,
            passphrase,
            key_length,
            tsbpd_delay: opts.tsbpd_delay,
            ..ConnectionOptions::default()
        }
    };

    let conn = SrtConnection::new_listener(options);
    Box::into_raw(Box::new(SrtConnectionHandle { inner: conn }))
}

/// 接続を解放
///
/// # Safety
/// conn は有効なポインタであること
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srt_connection_free(conn: *mut SrtConnectionHandle) {
    if !conn.is_null() {
        drop(Box::from_raw(conn));
    }
}

/// 接続状態を取得
///
/// # Safety
/// conn は有効なポインタであること
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srt_connection_state(
    conn: *const SrtConnectionHandle,
) -> SrtConnectionState {
    if conn.is_null() {
        return SrtConnectionState::Disconnected;
    }
    (*conn).inner.state().into()
}

/// 接続を開始 (Caller のみ)
///
/// # Safety
/// conn は有効なポインタであること
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srt_connection_connect(
    conn: *mut SrtConnectionHandle,
    now_micros: u64,
) -> i32 {
    if conn.is_null() {
        return -1;
    }
    let now = Timestamp::from_micros(now_micros);
    match (*conn).inner.connect(now) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// 受信データを処理
///
/// # Safety
/// conn, buf は有効なポインタであること
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srt_connection_feed_recv_buf(
    conn: *mut SrtConnectionHandle,
    buf: *const u8,
    len: usize,
    now_micros: u64,
) -> i32 {
    if conn.is_null() || buf.is_null() {
        return -1;
    }
    let data = slice::from_raw_parts(buf, len);
    let now = Timestamp::from_micros(now_micros);
    match (*conn).inner.feed_recv_buf(data, now) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// データを送信
///
/// # Safety
/// conn, payload は有効なポインタであること
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srt_connection_send(
    conn: *mut SrtConnectionHandle,
    payload: *const u8,
    len: usize,
    now_micros: u64,
) -> i32 {
    if conn.is_null() || payload.is_null() {
        return -1;
    }
    let data = slice::from_raw_parts(payload, len);
    let now = Timestamp::from_micros(now_micros);
    match (*conn).inner.send(data, now) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// 切断
///
/// # Safety
/// conn は有効なポインタであること
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srt_connection_disconnect(
    conn: *mut SrtConnectionHandle,
    now_micros: u64,
) {
    if !conn.is_null() {
        let now = Timestamp::from_micros(now_micros);
        (*conn).inner.disconnect(now);
    }
}

/// 出力パケットタイプ
#[repr(C)]
pub enum SrtOutputType {
    None = 0,
    SendPacket = 1,
    SetTimer = 2,
    ClearTimer = 3,
}

/// 出力パケット
#[repr(C)]
pub struct SrtOutput {
    pub output_type: SrtOutputType,
    pub data: *mut u8,
    pub data_len: usize,
    pub timer_id: u32,
    pub duration_micros: u64,
}

/// 出力を取得
///
/// # Safety
/// conn, output は有効なポインタであること
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srt_connection_poll_output(
    conn: *mut SrtConnectionHandle,
    output: *mut SrtOutput,
) -> i32 {
    if conn.is_null() || output.is_null() {
        return -1;
    }

    match (*conn).inner.poll_output() {
        Some(ConnectionOutput::SendPacket(data)) => {
            let boxed = data.into_boxed_slice();
            let len = boxed.len();
            let ptr = Box::into_raw(boxed) as *mut u8;
            (*output).output_type = SrtOutputType::SendPacket;
            (*output).data = ptr;
            (*output).data_len = len;
            (*output).timer_id = 0;
            (*output).duration_micros = 0;
            1
        }
        Some(ConnectionOutput::SetTimer {
            id,
            duration_micros,
        }) => {
            (*output).output_type = SrtOutputType::SetTimer;
            (*output).data = ptr::null_mut();
            (*output).data_len = 0;
            (*output).timer_id = id as u32;
            (*output).duration_micros = duration_micros;
            1
        }
        Some(ConnectionOutput::ClearTimer { id }) => {
            (*output).output_type = SrtOutputType::ClearTimer;
            (*output).data = ptr::null_mut();
            (*output).data_len = 0;
            (*output).timer_id = id as u32;
            (*output).duration_micros = 0;
            1
        }
        None => {
            (*output).output_type = SrtOutputType::None;
            (*output).data = ptr::null_mut();
            (*output).data_len = 0;
            (*output).timer_id = 0;
            (*output).duration_micros = 0;
            0
        }
    }
}

/// 出力データを解放
///
/// # Safety
/// data は srt_connection_poll_output で取得したポインタであること
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srt_output_data_free(data: *mut u8, len: usize) {
    if !data.is_null() && len > 0 {
        drop(Box::from_raw(ptr::slice_from_raw_parts_mut(data, len)));
    }
}

/// イベントタイプ
#[repr(C)]
pub enum SrtEventType {
    None = 0,
    Connected = 1,
    DataReceived = 2,
    StateChanged = 3,
    Error = 4,
    Disconnected = 5,
    KeyRefreshNeeded = 6,
}

/// イベント
#[repr(C)]
pub struct SrtEvent {
    pub event_type: SrtEventType,
    pub data: *mut u8,
    pub data_len: usize,
    pub message_number: u32,
    pub timestamp: u32,
    pub state: SrtConnectionState,
    pub key_length: u8,
}

/// イベントを取得
///
/// # Safety
/// conn, event は有効なポインタであること
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srt_connection_poll_event(
    conn: *mut SrtConnectionHandle,
    event: *mut SrtEvent,
) -> i32 {
    if conn.is_null() || event.is_null() {
        return -1;
    }

    match (*conn).inner.poll_event() {
        Some(ConnectionEvent::Connected) => {
            (*event).event_type = SrtEventType::Connected;
            (*event).data = ptr::null_mut();
            (*event).data_len = 0;
            (*event).message_number = 0;
            (*event).timestamp = 0;
            (*event).state = SrtConnectionState::Connected;
            (*event).key_length = 0;
            1
        }
        Some(ConnectionEvent::DataReceived {
            payload,
            message_number,
            timestamp,
        }) => {
            let boxed = payload.into_boxed_slice();
            let len = boxed.len();
            let ptr = Box::into_raw(boxed) as *mut u8;
            (*event).event_type = SrtEventType::DataReceived;
            (*event).data = ptr;
            (*event).data_len = len;
            (*event).message_number = message_number;
            (*event).timestamp = timestamp;
            (*event).state = SrtConnectionState::Connected;
            (*event).key_length = 0;
            1
        }
        Some(ConnectionEvent::StateChanged(state)) => {
            (*event).event_type = SrtEventType::StateChanged;
            (*event).data = ptr::null_mut();
            (*event).data_len = 0;
            (*event).message_number = 0;
            (*event).timestamp = 0;
            (*event).state = state.into();
            (*event).key_length = 0;
            1
        }
        Some(ConnectionEvent::Error(_)) => {
            (*event).event_type = SrtEventType::Error;
            (*event).data = ptr::null_mut();
            (*event).data_len = 0;
            (*event).message_number = 0;
            (*event).timestamp = 0;
            (*event).state = SrtConnectionState::Disconnected;
            (*event).key_length = 0;
            1
        }
        Some(ConnectionEvent::Disconnected { .. }) => {
            (*event).event_type = SrtEventType::Disconnected;
            (*event).data = ptr::null_mut();
            (*event).data_len = 0;
            (*event).message_number = 0;
            (*event).timestamp = 0;
            (*event).state = SrtConnectionState::Disconnected;
            (*event).key_length = 0;
            1
        }
        Some(ConnectionEvent::KeyRefreshNeeded { key_length }) => {
            (*event).event_type = SrtEventType::KeyRefreshNeeded;
            (*event).data = ptr::null_mut();
            (*event).data_len = 0;
            (*event).message_number = 0;
            (*event).timestamp = 0;
            (*event).state = SrtConnectionState::Connected;
            (*event).key_length = key_length as u8;
            1
        }
        None => {
            (*event).event_type = SrtEventType::None;
            (*event).data = ptr::null_mut();
            (*event).data_len = 0;
            (*event).message_number = 0;
            (*event).timestamp = 0;
            (*event).state = SrtConnectionState::Disconnected;
            (*event).key_length = 0;
            0
        }
    }
}

/// イベントデータを解放
///
/// # Safety
/// data は srt_connection_poll_event で取得したポインタであること
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srt_event_data_free(data: *mut u8, len: usize) {
    if !data.is_null() && len > 0 {
        drop(Box::from_raw(ptr::slice_from_raw_parts_mut(data, len)));
    }
}

/// 新しい SEK を提供 (キーリフレッシュ用)
///
/// # Safety
/// conn, sek は有効なポインタであること
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srt_connection_provide_new_sek(
    conn: *mut SrtConnectionHandle,
    sek: *const u8,
    sek_len: usize,
    now_micros: u64,
) -> i32 {
    if conn.is_null() || sek.is_null() {
        return -1;
    }
    let sek_data = slice::from_raw_parts(sek, sek_len);
    let now = Timestamp::from_micros(now_micros);
    match (*conn).inner.provide_new_sek(sek_data, now) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}
