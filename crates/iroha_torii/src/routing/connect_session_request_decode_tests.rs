#[test]
fn connect_session_request_rejects_retired_key() {
    const RETIRED: &str = r#"{"sid":"x","network_id":"hash:4141414141414141414141414141414141414141414141414141414141414141#7023","app_pk":"x","nonce":"x","chain_id":"retired"}"#;
    assert!(norito::json::from_str::<ConnectSessionRequest>(RETIRED).is_err());
}
