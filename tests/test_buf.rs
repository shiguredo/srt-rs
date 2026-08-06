use shiguredo_srt::read_utf8;

#[test]
fn test_read_utf8_invalid() {
    let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
    let mut slice = invalid_utf8.as_slice();
    let result = read_utf8(&mut slice, 3);
    assert!(result.is_err());
}

#[test]
fn test_read_utf8_valid() {
    let data = b"hello";
    let mut slice = data.as_slice();
    let result = read_utf8(&mut slice, 5).expect("有効な UTF-8 の読み取りは成功する想定");
    assert_eq!(result, "hello");
}
