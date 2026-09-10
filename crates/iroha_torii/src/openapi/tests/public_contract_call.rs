// Public contract preparation and certified-admission schema bindings.

#[test]
fn public_contract_call_schema_matches_exact_queue_plan_handoff() {
    for (label, document) in [
        ("package authority", canonical_document()),
        ("compiled spec", generate_spec()),
    ] {
        let schemas = component_schemas(&document);
        let request = &schemas["ContractCallRequest"];
        assert_eq!(
            request["additionalProperties"],
            Value::Bool(false),
            "{label}"
        );
        for field in [
            "metadata",
            "transaction_payload_b64",
            "public_key_hex",
            "signature_b64",
        ] {
            assert!(
                request["properties"].get(field).is_some(),
                "{label}: {field}"
            );
        }
        assert_eq!(
            request["allOf"]
                .as_array()
                .expect("selector and detached groups")
                .len(),
            2
        );
        for name in ["ContractCallResponse", "ContractCallOperationReceipt"] {
            let schema = &schemas[name];
            assert_eq!(
                schema["additionalProperties"],
                Value::Bool(false),
                "{label}: {name}"
            );
            assert_eq!(
                schema["required"].as_array().expect("required").len(),
                15,
                "{label}: {name}"
            );
            assert_eq!(
                schema["properties"].as_object().expect("properties").len(),
                15,
                "{label}: {name}"
            );
        }
        assert_eq!(
            schemas["ContractCallResponse"]["properties"]["pipeline_status"]["type"].as_str(),
            Some("null")
        );
        assert_eq!(
            schemas["ContractCallOperationReceipt"]["properties"]["gas_used"]["type"].as_str(),
            Some("null")
        );
        let canonical = canonical_document();
        let operation = &canonical["paths"]["/v1/contracts/call"]["post"];
        assert_eq!(
            operation["responses"]["200"]["content"]["application/json"]["schema"],
            schema_ref("ContractCallResponse")
        );
        for phrase in [
            "QueuePlanSynced",
            "never re-quoted",
            "durable certified admission",
            "100000ms",
        ] {
            assert!(
                operation["description"]
                    .as_str()
                    .expect("description")
                    .contains(phrase),
                "{label}: {phrase}"
            );
        }
    }
}
