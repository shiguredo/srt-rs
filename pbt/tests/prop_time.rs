//! Property-based tests for Timestamp

use proptest::prelude::*;
use shiguredo_srt::Timestamp;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn test_from_micros_roundtrip(micros in 0u64..=u64::MAX) {
        let ts = Timestamp::from_micros(micros);
        prop_assert_eq!(ts.as_micros(), micros);
    }

    #[test]
    fn test_add_micros(a in 0u64..=u64::MAX, b in 0u64..1_000_000u64) {
        let ts = Timestamp::from_micros(a);
        let result = ts.add_micros(b);
        if a.checked_add(b).is_some() {
            prop_assert_eq!(result.as_micros(), a + b);
        }
    }

    #[test]
    fn test_saturating_sub(a in 0u64..1_000_000u64, b in 0u64..1_000_000u64) {
        let ts_a = Timestamp::from_micros(a);
        let ts_b = Timestamp::from_micros(b);
        let diff = ts_a.saturating_sub(ts_b);
        prop_assert_eq!(diff, a.saturating_sub(b));
    }
}
