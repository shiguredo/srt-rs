use shiguredo_srt::ByteSliceExt;

#[test]
fn test_read_utf8_invalid() {
    let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
    let mut slice = invalid_utf8.as_slice();
    let result = slice.read_utf8(3);
    assert!(result.is_err());
}

#[test]
fn test_read_utf8_valid() {
    let data = b"hello";
    let mut slice = data.as_slice();
    let result = slice.read_utf8(5).unwrap();
    assert_eq!(result, "hello");
}
