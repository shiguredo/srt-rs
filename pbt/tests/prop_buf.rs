//! Property-based tests for Buf (ByteSliceExt / VecExt)

use proptest::prelude::*;
use shiguredo_srt::{ByteSliceExt, VecExt};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn test_u16_roundtrip(value in 0u16..=u16::MAX) {
        let mut buf = Vec::new();
        buf.write_u16(value);
        let mut slice = buf.as_slice();
        let read = slice.read_u16().unwrap();
        prop_assert_eq!(value, read);
        prop_assert!(slice.is_empty());
    }

    #[test]
    fn test_u32_roundtrip(value in 0u32..=u32::MAX) {
        let mut buf = Vec::new();
        buf.write_u32(value);
        let mut slice = buf.as_slice();
        let read = slice.read_u32().unwrap();
        prop_assert_eq!(value, read);
        prop_assert!(slice.is_empty());
    }

    #[test]
    fn test_u64_roundtrip(value in 0u64..=u64::MAX) {
        let mut buf = Vec::new();
        buf.write_u64(value);
        let mut slice = buf.as_slice();
        let read = slice.read_u64().unwrap();
        prop_assert_eq!(value, read);
        prop_assert!(slice.is_empty());
    }

    #[test]
    fn test_bytes_roundtrip(data in prop::collection::vec(any::<u8>(), 0..256)) {
        let mut buf = Vec::new();
        buf.write_bytes(&data);
        let mut slice = buf.as_slice();
        let read = slice.read_bytes(data.len()).unwrap();
        prop_assert_eq!(data, read);
        prop_assert!(slice.is_empty());
    }
}
