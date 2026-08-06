use crate::error::Error;

/// 入力バイト列から 8-bit 値を読み取り、カーソルを進める
#[track_caller]
pub fn read_u8(slice: &mut &[u8]) -> Result<u8, Error> {
    Error::check_buffer_size(1, slice)?;
    let v = slice[0];
    *slice = &slice[1..];
    Ok(v)
}

/// 入力バイト列から 16-bit 値 (ビッグエンディアン) を読み取り、カーソルを進める
#[track_caller]
pub fn read_u16(slice: &mut &[u8]) -> Result<u16, Error> {
    Error::check_buffer_size(2, slice)?;
    let bytes = [slice[0], slice[1]];
    *slice = &slice[2..];
    Ok(u16::from_be_bytes(bytes))
}

/// 入力バイト列から 32-bit 値 (ビッグエンディアン) を読み取り、カーソルを進める
#[track_caller]
pub fn read_u32(slice: &mut &[u8]) -> Result<u32, Error> {
    Error::check_buffer_size(4, slice)?;
    let bytes = [slice[0], slice[1], slice[2], slice[3]];
    *slice = &slice[4..];
    Ok(u32::from_be_bytes(bytes))
}

/// 入力バイト列から 64-bit 値 (ビッグエンディアン) を読み取り、カーソルを進める
#[track_caller]
pub fn read_u64(slice: &mut &[u8]) -> Result<u64, Error> {
    Error::check_buffer_size(8, slice)?;
    let bytes = [
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ];
    *slice = &slice[8..];
    Ok(u64::from_be_bytes(bytes))
}

/// 入力バイト列から指定長のバイト列を読み取り、カーソルを進める
#[track_caller]
pub fn read_bytes(slice: &mut &[u8], len: usize) -> Result<Vec<u8>, Error> {
    Error::check_buffer_size(len, slice)?;
    let buf = slice[..len].to_vec();
    *slice = &slice[len..];
    Ok(buf)
}

/// 入力バイト列から指定長の UTF-8 文字列を読み取り、カーソルを進める
#[track_caller]
pub fn read_utf8(slice: &mut &[u8], len: usize) -> Result<String, Error> {
    let buf = read_bytes(slice, len)?;
    String::from_utf8(buf).map_err(|e| Error::invalid_data(format!("invalid UTF-8 bytes: {e}")))
}

/// 出力バイト列に 8-bit 値を追記する
pub fn write_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}

/// 出力バイト列に 16-bit 値 (ビッグエンディアン) を追記する
pub fn write_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_be_bytes());
}

/// 出力バイト列に 32-bit 値 (ビッグエンディアン) を追記する
pub fn write_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_be_bytes());
}

/// 出力バイト列に 64-bit 値 (ビッグエンディアン) を追記する
pub fn write_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_be_bytes());
}

/// 出力バイト列にバイト列を追記する
pub fn write_bytes(buf: &mut Vec<u8>, v: &[u8]) {
    buf.extend_from_slice(v);
}
