//! Property-based tests for Buf (read / write functions)

use proptest::prelude::*;
use shiguredo_srt::{
    read_bytes, read_u16, read_u32, read_u64, write_bytes, write_u16, write_u32, write_u64,
};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn test_u16_roundtrip(value in 0u16..=u16::MAX) {
        let mut buf = Vec::new();
        write_u16(&mut buf, value);
        let mut slice = buf.as_slice();
        let read = read_u16(&mut slice).expect("書き込んだ値の読み取りは成功する想定");
        prop_assert_eq!(value, read);
        prop_assert!(slice.is_empty());
    }

    #[test]
    fn test_u32_roundtrip(value in 0u32..=u32::MAX) {
        let mut buf = Vec::new();
        write_u32(&mut buf, value);
        let mut slice = buf.as_slice();
        let read = read_u32(&mut slice).expect("書き込んだ値の読み取りは成功する想定");
        prop_assert_eq!(value, read);
        prop_assert!(slice.is_empty());
    }

    #[test]
    fn test_u64_roundtrip(value in 0u64..=u64::MAX) {
        let mut buf = Vec::new();
        write_u64(&mut buf, value);
        let mut slice = buf.as_slice();
        let read = read_u64(&mut slice).expect("書き込んだ値の読み取りは成功する想定");
        prop_assert_eq!(value, read);
        prop_assert!(slice.is_empty());
    }

    #[test]
    fn test_bytes_roundtrip(data in prop::collection::vec(any::<u8>(), 0..256)) {
        let mut buf = Vec::new();
        write_bytes(&mut buf, &data);
        let mut slice = buf.as_slice();
        let read = read_bytes(&mut slice, data.len()).expect("書き込んだバイト列の読み取りは成功する想定");
        prop_assert_eq!(data, read);
        prop_assert!(slice.is_empty());
    }
}
