// Included by `soracloud::tests`; keeping the test here preserves its original
// module path while separating exact-quantity boundary coverage from routes.

#[test]
fn wallet_quantity_boundary_preserves_subnano_and_wide_values() {
    let sub_nano: Quantity = "0.0000000001".parse().expect("bounded quantity");
    let overwide: Quantity = "340282366920938463463374607431768211456.0000000001"
        .parse()
        .expect("quantity exceeds the legacy u128 range");
    for amount in [sub_nano, overwide] {
        let payload = AgentWalletSpendPayload {
            apartment_name: "ops_agent".to_owned(),
            asset_definition: "asset".to_owned(),
            amount: amount.clone(),
        };
        let encoded = encode_agent_wallet_spend_signature_payload(&payload)
            .expect("encode exact quantity payload");
        let expected = norito::to_bytes(&(
            payload.apartment_name.as_str(),
            payload.asset_definition.as_str(),
            amount,
        ))
        .expect("encode canonical tuple");
        assert_eq!(encoded, expected);

        let value = norito::json::to_value(&payload).expect("serialize exact wallet payload");
        let object = value.as_object().expect("wallet payload object");
        let amount_text = payload.amount.to_string();
        assert_eq!(
            object.get("amount").and_then(norito::json::Value::as_str),
            Some(amount_text.as_str())
        );
        assert!(!object.contains_key("amount_nanos"));
    }

    for amount in ["-1", "+1", "01", "1.0", "0.00000000000000000000000000001"] {
        let raw = format!(
            "{{\"apartment_name\":\"ops_agent\",\"asset_definition\":\"asset\",\"amount\":\"{amount}\"}}"
        );
        assert!(
            norito::json::from_str::<AgentWalletSpendPayload>(&raw).is_err(),
            "accepted hostile wallet quantity `{amount}`"
        );
    }
    for hostile in [
        r#"{"apartment_name":"ops_agent","asset_definition":"asset","amount":1}"#,
        r#"{"apartment_name":"ops_agent","asset_definition":"asset","amount_nanos":1}"#,
    ] {
        assert!(
            norito::json::from_str::<AgentWalletSpendPayload>(hostile).is_err(),
            "accepted noncanonical or retired wallet payload `{hostile}`"
        );
    }
}
