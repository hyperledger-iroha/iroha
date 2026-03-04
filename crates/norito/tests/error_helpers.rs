use norito::json::Error;

#[test]
fn helper_methods_emit_expected_messages() {
    assert_eq!(
        Error::missing_field("foo").to_string(),
        "JSON error: missing field `foo`"
    );
    assert_eq!(
        Error::duplicate_field("bar").to_string(),
        "JSON error: duplicate field `bar`"
    );
    assert_eq!(
        Error::unknown_field("baz").to_string(),
        "JSON error: unknown field `baz`"
    );
}
