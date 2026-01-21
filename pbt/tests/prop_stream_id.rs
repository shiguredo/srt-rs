//! Property-based tests for SRT Stream ID

use proptest::prelude::*;
use shiguredo_srt::stream_id::{AccessControl, AccessControlBuilder, StreamMode, StreamType};

/// StreamType の任意生成
fn arb_stream_type() -> impl Strategy<Value = StreamType> {
    prop::sample::select(vec![StreamType::Stream, StreamType::File, StreamType::Auth])
}

/// StreamMode の任意生成
fn arb_stream_mode() -> impl Strategy<Value = StreamMode> {
    prop::sample::select(vec![
        StreamMode::Request,
        StreamMode::Publish,
        StreamMode::Bidirectional,
    ])
}

/// 有効なキー/バリュー文字列 (カンマと等号を除く)
fn arb_key_value_string() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_/.-]{1,32}".prop_filter("no commas or equals", |s| {
        !s.contains(',') && !s.contains('=')
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn test_stream_type_as_str_from_str_roundtrip(stream_type in arb_stream_type()) {
        let s = stream_type.as_str();
        let parsed = StreamType::from_str(s);
        prop_assert_eq!(parsed, Some(stream_type));
    }

    #[test]
    fn test_stream_mode_as_str_from_str_roundtrip(stream_mode in arb_stream_mode()) {
        let s = stream_mode.as_str();
        let parsed = StreamMode::from_str(s);
        prop_assert_eq!(parsed, Some(stream_mode));
    }

    #[test]
    fn test_stream_type_from_str_invalid(s in "[a-z]{1,10}".prop_filter("not valid type", |s| {
        s != "stream" && s != "file" && s != "auth"
    })) {
        let parsed = StreamType::from_str(&s);
        prop_assert!(parsed.is_none());
    }

    #[test]
    fn test_stream_mode_from_str_invalid(s in "[a-z]{1,15}".prop_filter("not valid mode", |s| {
        s != "request" && s != "publish" && s != "bidirectional"
    })) {
        let parsed = StreamMode::from_str(&s);
        prop_assert!(parsed.is_none());
    }

    #[test]
    fn test_access_control_roundtrip(
        user_name in prop::option::of(arb_key_value_string()),
        resource_name in prop::option::of(arb_key_value_string()),
        host_name in prop::option::of(arb_key_value_string()),
        session_id in prop::option::of(arb_key_value_string()),
        stream_type in arb_stream_type(),
        stream_mode in arb_stream_mode(),
    ) {
        let mut builder = AccessControlBuilder::new()
            .stream_type(stream_type)
            .stream_mode(stream_mode);

        if let Some(ref u) = user_name {
            builder = builder.user_name(u);
        }
        if let Some(ref r) = resource_name {
            builder = builder.resource_name(r);
        }
        if let Some(ref h) = host_name {
            builder = builder.host_name(h);
        }
        if let Some(ref s) = session_id {
            builder = builder.session_id(s);
        }

        let original = builder.build();
        let encoded = original.encode();

        // エンコードは必ず #!:: で始まる
        prop_assert!(encoded.starts_with("#!::"));

        let parsed = AccessControl::parse(&encoded).expect("parse should succeed");

        prop_assert_eq!(parsed.user_name(), original.user_name());
        prop_assert_eq!(parsed.resource_name(), original.resource_name());
        prop_assert_eq!(parsed.host_name(), original.host_name());
        prop_assert_eq!(parsed.session_id(), original.session_id());
        prop_assert_eq!(parsed.stream_type(), original.stream_type());
        prop_assert_eq!(parsed.stream_mode(), original.stream_mode());
    }

    #[test]
    fn test_access_control_with_custom_keys(
        custom_key in "[a-z_]{1,16}".prop_filter("not standard key", |s| {
            s != "u" && s != "r" && s != "h" && s != "s" && s != "t" && s != "m"
        }),
        custom_value in arb_key_value_string(),
    ) {
        let original = AccessControlBuilder::new()
            .custom(&custom_key, &custom_value)
            .build();

        let encoded = original.encode();
        let parsed = AccessControl::parse(&encoded).expect("parse should succeed");

        prop_assert_eq!(parsed.custom(&custom_key), Some(custom_value.as_str()));
    }

    #[test]
    fn test_access_control_parse_without_prefix(s in "[a-zA-Z0-9_/.-]{1,64}".prop_filter("no prefix", |s| {
        !s.starts_with("#!::")
    })) {
        let parsed = AccessControl::parse(&s);
        prop_assert!(parsed.is_none());
    }

    #[test]
    fn test_access_control_parse_with_invalid_prefix(
        prefix in "[#!:]{1,3}".prop_filter("not correct prefix", |s| s != "#!::"),
        content in "[a-zA-Z0-9_=,]{1,32}",
    ) {
        let input = format!("{prefix}{content}");
        let parsed = AccessControl::parse(&input);
        prop_assert!(parsed.is_none());
    }
}
