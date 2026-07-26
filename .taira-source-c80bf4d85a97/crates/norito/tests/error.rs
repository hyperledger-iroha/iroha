use norito::Error;

#[test]
fn error_from_str_and_string() {
    let msg = "custom error";
    let err: Error = msg.into();
    assert_eq!(err.to_string(), msg);

    let string_msg = String::from("another error");
    let err: Error = string_msg.clone().into();
    assert_eq!(err.to_string(), string_msg);
}
