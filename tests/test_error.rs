use shiguredo_srt::{ErrorKind, read_u32};

#[test]
fn test_read_with_exact_buffer() {
    let buf = [0u8; 4];
    let mut slice = buf.as_slice();
    let result = read_u32(&mut slice);
    assert!(result.is_ok());
}

#[test]
fn test_read_with_larger_buffer() {
    let buf = [0u8; 8];
    let mut slice = buf.as_slice();
    let result = read_u32(&mut slice);
    assert!(result.is_ok());
}

#[test]
fn test_read_with_too_small_buffer() {
    let buf = [0u8; 3];
    let mut slice = buf.as_slice();
    let result = read_u32(&mut slice);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind, ErrorKind::InsufficientBuffer);
}

#[test]
fn test_read_with_empty_buffer() {
    let buf: [u8; 0] = [];
    let mut slice = buf.as_slice();
    let result = read_u32(&mut slice);
    assert!(result.is_err());
}
