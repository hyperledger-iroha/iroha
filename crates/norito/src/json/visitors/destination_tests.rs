//! Caller-owned JSON destinations preserve canonical kernels and typed refusal.
use super::*;

#[derive(Debug)]
enum Failure {
    Json(Error),
    Original(u64),
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self::Json(error)
    }
}

#[test]
fn exact_string_destination_matches_owned_utf8_escapes_and_cursor() {
    for source in [r#""plain""#, r#""\n\u0041\uD83D\uDE80é""#, r#""""#] {
        let mut owned = Parser::new(source);
        let expected = owned.parse_string().unwrap();
        let mut destination = Parser::new(source);
        let actual = destination
            .parse_string_with_buffer(|length| Ok::<_, Failure>(vec![0_u8; length]))
            .unwrap();
        assert_eq!(actual, expected.as_bytes());
        assert_eq!(destination.position(), owned.position());
        assert!(destination.eof());
    }
    for source in [r#""\uD800""#, r#""\q""#, "\"unterminated"] {
        let owned = Parser::new(source).parse_string().unwrap_err();
        let result = Parser::new(source)
            .parse_string_with_buffer(|length| Ok::<_, Failure>(vec![0_u8; length]))
            .unwrap_err();
        assert!(matches!(result, Failure::Json(actual) if actual.to_string()==owned.to_string()));
    }
}

#[test]
fn string_destination_refusal_preserves_original_cause_and_logical_priority() {
    let mut parser = Parser::new(r#""abcdef""#);
    let mut allocated = 0;
    let result = parser.parse_string_with_buffer(|length| {
        allocated += 1;
        assert_eq!(length, 6);
        Err::<Vec<u8>, _>(Failure::Original(73))
    });
    assert!(matches!(result, Err(Failure::Original(73))));
    assert_eq!(allocated, 1);
    let limits = crate::core::DecodeLimits::new(1024, 1024, 1024, 0, 32);
    crate::core::with_decode_limits_scope(limits, || {
        let mut parser = Parser::new(r#""abcdef""#);
        let result = parser.parse_string_with_buffer(|_| {
            allocated += 1;
            Err::<Vec<u8>, _>(Failure::Original(74))
        });
        assert!(matches!(result, Err(Failure::Json(_))));
    });
    assert_eq!(
        allocated, 1,
        "logical string refusal precedes physical owner callback"
    );
    let result =
        Parser::new(r#""abc""#).parse_string_with_buffer(|_| Ok::<_, Failure>(vec![0x55; 2]));
    assert!(matches!(result, Err(Failure::Json(_))));
}

#[test]
fn typed_object_and_sequence_callbacks_preserve_refusal_count_and_delimiters() {
    let mut parser = Parser::new(r#"{"value":[1,2]}"#);
    let mut map = MapVisitor::new(&mut parser).unwrap();
    assert_eq!(map.next_key().unwrap().unwrap().as_str(), "value");
    let result = map.parse_value_with_parser_typed(|parser| {
        let mut values = SeqVisitor::new(parser)?;
        assert_eq!(values.total_entries(), 2);
        assert_eq!(
            values.next_element_with_parser_typed(|parser| {
                Ok::<_, Failure>(parser.parse_u64()?)
            })?,
            Some(1)
        );
        values.next_element_with_parser_typed(|_| Err::<u64, _>(Failure::Original(91)))
    });
    assert!(matches!(result, Err(Failure::Original(91))));
    let mut parser = Parser::new(r#"{"value":[1,2]}"#);
    let mut map = MapVisitor::new(&mut parser).unwrap();
    map.next_key().unwrap();
    let result = map
        .parse_value_with_parser_typed(|parser| {
            let mut values = SeqVisitor::new(parser)?;
            let first = values
                .next_element_with_parser_typed(|parser| Ok::<_, Failure>(parser.parse_u64()?))?;
            let second = values
                .next_element_with_parser_typed(|parser| Ok::<_, Failure>(parser.parse_u64()?))?;
            assert!(values.next_element::<u64>()?.is_none());
            values.finish()?;
            Ok::<_, Failure>((first, second))
        })
        .unwrap();
    assert_eq!(result, (Some(1), Some(2)));
    map.finish().unwrap();
    assert!(parser.eof());
}

#[test]
fn string_destination_exact_preflight_never_indexes_outside_malformed_or_unicode_input() {
    for scalar in [
        0, 1, 0x1f, 0x20, 0x7f, 0x80, 0x7ff, 0x800, 0xd7ff, 0xe000, 0xffff, 0x10000, 0x10ffff,
    ] {
        let ch = char::from_u32(scalar).unwrap();
        let source = crate::json::to_json(&ch.to_string()).unwrap();
        let actual = Parser::new(&source)
            .parse_string_with_buffer(|length| Ok::<_, Failure>(vec![0; length]))
            .unwrap();
        assert_eq!(actual, ch.to_string().as_bytes());
        for end in 0..source.len() {
            if !source.is_char_boundary(end) {
                continue;
            }
            let prefix = &source[..end];
            let result = std::panic::catch_unwind(|| {
                Parser::new(prefix)
                    .parse_string_with_buffer(|length| Ok::<_, Failure>(vec![0; length]))
            });
            assert!(
                result.is_ok(),
                "malformed prefix must return syntax failure"
            );
            assert!(result.unwrap().is_err());
        }
    }
    for source in [
        r#""\u""#,
        r#""\u000G""#,
        r#""\uD800\u0000""#,
        r#""\uDFFF""#,
        r#""\uD800x""#,
        "\"\n\"",
        r#""\uD800\q""#,
    ] {
        let mut calls = 0;
        let result = Parser::new(source).parse_string_with_buffer(|length| {
            calls += 1;
            Ok::<_, Failure>(vec![0; length])
        });
        assert!(matches!(result, Err(Failure::Json(_))));
        assert_eq!(
            calls, 0,
            "preflight syntax failure precedes the destination allocation"
        );
    }
}

#[test]
fn string_destination_active_successful_prefix_is_not_refunded_on_refusal() {
    let limits = crate::core::DecodeLimits::new(1024, 1024, 1024, 3, 32);
    crate::core::with_decode_limits_scope(limits, || {
        let first = Parser::new(r#""abc""#)
            .parse_string_with_buffer(|_| Err::<Vec<u8>, _>(Failure::Original(104)));
        assert!(matches!(first, Err(Failure::Original(104))));
        let mut calls = 0;
        let retry = Parser::new(r#""x""#).parse_string_with_buffer(|length| {
            calls += 1;
            Ok::<_, Failure>(vec![0; length])
        });
        assert!(matches!(retry, Err(Failure::Json(ref error)) if error.is_decode_resource_limit()));
        assert_eq!(calls, 0);
    });
    crate::core::with_decode_limits_scope(limits, || {
        assert_eq!(Parser::new(r#""abc""#).parse_string().unwrap(), "abc");
        assert!(
            Parser::new(r#""x""#)
                .parse_string()
                .unwrap_err()
                .is_decode_resource_limit()
        );
    });
}
