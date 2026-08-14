// ISO 20022 lifecycle wrong-family regression.
#[test]
fn lifecycle_wrong_message_family_fails_parser_validation() {
    let err = parse_message(
        "pacs.004",
        b"MsgId=m1\nIntrBkSttlmAmt=10\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nDbtrAgt=DEUTDEFF\nCdtrAgt=DEUTDEFF",
    )
    .expect_err("pacs.008 fields must not satisfy pacs.004 endpoint");
    assert!(matches!(err, MsgError::MissingField(_)));
}
