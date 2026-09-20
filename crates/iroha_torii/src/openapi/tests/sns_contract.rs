#[test]
fn sns_name_absence_openapi_is_typed_and_selector_bound() {
    use iroha_data_model::sns::NameSelectorV1;
    use iroha_torii_shared::sns::{SNS_REGISTRATION_NOT_FOUND_CODE, SnsRegistrationNotFoundV1};
    use iroha_torii_shared::{ErrorDetails, ErrorEnvelope};

    let document = canonical_document();
    let operation = openapi_operation(&document, "/v1/sns/names/{namespace}/{literal}", "get");
    let missing = operation
        .get("responses")
        .and_then(|responses| responses.get("404"))
        .expect("SNS missing-registration response");
    let content = missing
        .get("content")
        .and_then(Value::as_object)
        .expect("SNS not-found media types");
    assert_eq!(
        content.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from(["application/json", "application/x-norito"])
    );
    for media in ["application/json", "application/x-norito"] {
        assert_eq!(
            content[media].get("schema"),
            Some(&schema_ref("ErrorEnvelope"))
        );
    }
    let description = missing.get("description").and_then(Value::as_str).unwrap();
    for distinction in [
        "exact requested",
        SNS_REGISTRATION_NOT_FOUND_CODE,
        "details.sns_registration_not_found",
        "404 alone",
    ] {
        assert!(
            description.contains(distinction),
            "missing distinction: {distinction}"
        );
    }

    let schemas = component_schemas(&document);
    let schema = &schemas["SnsRegistrationNotFoundV1"];
    assert_eq!(schema.get("type").and_then(Value::as_str), Some("object"));
    assert_eq!(
        schema.get("additionalProperties").and_then(Value::as_bool),
        Some(false)
    );
    let required = schema
        .get("required")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<BTreeSet<_>>();
    let selector = NameSelectorV1::new(4099, "dpn").unwrap();
    let absence = SnsRegistrationNotFoundV1::new(selector.suffix_id, selector.label.clone());
    let details_schema = component_properties(schemas, "ErrorDetails");
    assert_eq!(
        details_schema.get("sns_registration_not_found"),
        Some(&schema_ref("SnsRegistrationNotFoundV1"))
    );
    let envelope = ErrorEnvelope::new(
        SNS_REGISTRATION_NOT_FOUND_CODE,
        "The requested SNS registration does not exist.",
    )
    .with_details(ErrorDetails {
        sns_registration_not_found: Some(absence.clone()),
        ..ErrorDetails::default()
    });
    let envelope_json = norito::json::to_vec(&envelope).unwrap();
    let envelope_value: Value = norito::json::from_slice(&envelope_json).unwrap();
    assert_eq!(
        envelope_value["code"].as_str(),
        Some(SNS_REGISTRATION_NOT_FOUND_CODE)
    );
    let encoded = norito::json::to_vec(&absence).unwrap();
    let native: Value = norito::json::from_slice(&encoded).unwrap();
    let native_keys = native
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(required, native_keys);
    let properties = component_properties(schemas, "SnsRegistrationNotFoundV1");
    assert_eq!(
        properties
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        native_keys
    );
    assert!(!properties.contains_key("code"));
    assert_eq!(
        envelope_value["details"]["sns_registration_not_found"],
        native
    );
    assert_eq!(
        properties["suffix_id"].get("type").and_then(Value::as_str),
        Some("integer")
    );
    assert_eq!(
        properties["suffix_id"]
            .get("minimum")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        properties["suffix_id"]
            .get("maximum")
            .and_then(Value::as_u64),
        Some(u64::from(u16::MAX))
    );
    assert_eq!(
        properties["label"].get("type").and_then(Value::as_str),
        Some("string")
    );
    assert!(absence.matches_selector(&selector));
    assert!(!absence.matches_selector(&NameSelectorV1::new(4099, "other").unwrap()));
}

#[test]
fn alias_errors_openapi_match_native_reports_and_bound_absence() {
    use iroha_data_model::alias_setup::*;
    use iroha_torii_shared::aliases::*;

    fn keys(value: &Value) -> BTreeSet<&str> {
        value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect()
    }
    fn required(schema: &Value) -> BTreeSet<&str> {
        schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect()
    }
    let document = canonical_document();
    let schemas = component_schemas(&document);
    let diagnostic = AliasSetupDiagnosticV1 {
        phase: AliasSetupValidationPhaseV1::Planning,
        code: "alias.plan.conflict".into(),
        severity: AliasSetupSeverityV1::Error,
        resource: Some("admin@dpn".into()),
        config_path: None,
        expected: None,
        actual: None,
        remediation: "request a corrected plan".into(),
    };
    let report = AliasSetupReportV1::new(AliasSetupStatusV1::Blocked, vec![diagnostic.clone()]);
    let native_report = norito::json::to_value(&report).unwrap();
    let native_diagnostic = norito::json::to_value(&diagnostic).unwrap();
    let alias = norito::json::to_value(&AccountAliasNotFoundV1 {
        alias: "admin@dpn".into(),
    })
    .unwrap();
    let account = norito::json::to_value(&AccountAliasesByAccountNotFoundV1 {
        account_id: "exact-canonical-account-selector".into(),
        dataspace: Some("dpn".into()),
        domain: None,
    })
    .unwrap();
    for (name, native) in [
        ("AliasSetupReportV1", &native_report),
        ("AliasSetupDiagnosticV1", &native_diagnostic),
        ("AccountAliasNotFoundV1", &alias),
        ("AccountAliasesByAccountNotFoundV1", &account),
    ] {
        let schema = &schemas[name];
        assert_eq!(schema["type"].as_str(), Some("object"));
        assert_eq!(schema["additionalProperties"].as_bool(), Some(false));
        assert_eq!(keys(&schema["properties"]), keys(native), "{name}");
        let expected_required = if name == "AliasSetupDiagnosticV1" {
            BTreeSet::from(["phase", "code", "severity", "remediation"])
        } else {
            keys(native)
        };
        assert_eq!(required(schema), expected_required, "{name}");
    }
    let report_properties = component_properties(schemas, "AliasSetupReportV1");
    assert_eq!(
        report_properties["status"],
        schema_ref("AliasSetupStatusV1")
    );
    assert_eq!(
        report_properties["diagnostics"]["items"],
        schema_ref("AliasSetupDiagnosticV1")
    );
    assert_eq!(
        report_properties["version"]["enum"].as_array().unwrap(),
        &vec![Value::from(report.version)]
    );
    let diagnostic_properties = component_properties(schemas, "AliasSetupDiagnosticV1");
    assert_eq!(
        diagnostic_properties["phase"],
        schema_ref("AliasSetupValidationPhaseV1")
    );
    assert_eq!(
        diagnostic_properties["severity"],
        schema_ref("AliasSetupSeverityV1")
    );
    for field in ["resource", "config_path", "expected", "actual"] {
        let types = diagnostic_properties[field]["type"].as_array().unwrap();
        assert_eq!(
            types
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["string", "null"])
        );
    }
    for field in ["dataspace", "domain"] {
        let types = schemas["AccountAliasesByAccountNotFoundV1"]["properties"][field]["type"]
            .as_array()
            .unwrap();
        assert_eq!(
            types
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["string", "null"])
        );
    }
    for (name, tag, variants) in [
        (
            "AliasSetupStatusV1",
            "status",
            [
                AliasSetupStatusV1::Ready,
                AliasSetupStatusV1::Pending,
                AliasSetupStatusV1::Blocked,
            ]
            .iter()
            .map(|v| norito::json::to_value(v).unwrap())
            .collect::<Vec<_>>(),
        ),
        (
            "AliasSetupValidationPhaseV1",
            "phase",
            [
                AliasSetupValidationPhaseV1::Config,
                AliasSetupValidationPhaseV1::Catalog,
                AliasSetupValidationPhaseV1::Bootstrap,
                AliasSetupValidationPhaseV1::WorldState,
                AliasSetupValidationPhaseV1::Planning,
            ]
            .iter()
            .map(|v| norito::json::to_value(v).unwrap())
            .collect(),
        ),
        (
            "AliasSetupSeverityV1",
            "severity",
            [
                AliasSetupSeverityV1::Info,
                AliasSetupSeverityV1::Warning,
                AliasSetupSeverityV1::Error,
            ]
            .iter()
            .map(|v| norito::json::to_value(v).unwrap())
            .collect(),
        ),
    ] {
        let schema = &schemas[name];
        assert_eq!(schema["type"].as_str(), Some("object"));
        assert_eq!(schema["additionalProperties"].as_bool(), Some(false));
        assert_eq!(required(schema), BTreeSet::from([tag, "value"]));
        assert_eq!(keys(&schema["properties"]), BTreeSet::from([tag, "value"]));
        assert_eq!(schema["properties"]["value"]["type"].as_str(), Some("null"));
        let tags = variants
            .iter()
            .map(|v| {
                assert_eq!(keys(v), BTreeSet::from([tag, "value"]));
                assert_eq!(v["value"], Value::Null);
                v[tag].as_str().unwrap()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            schema["properties"][tag]["enum"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            tags
        );
    }
    let details = component_properties(schemas, "ErrorDetails");
    for (field, name) in [
        ("alias_setup_report", "AliasSetupReportV1"),
        ("account_alias_not_found", "AccountAliasNotFoundV1"),
        (
            "account_aliases_by_account_not_found",
            "AccountAliasesByAccountNotFoundV1",
        ),
    ] {
        assert_eq!(details[field], schema_ref(name));
    }
    for (path, status, code, detail) in [
        (
            "/v1/aliases/setup/plan",
            "400",
            ALIAS_SETUP_REJECTED_CODE,
            "alias_setup_report",
        ),
        (
            "/v1/aliases/setup/plan",
            "403",
            ALIAS_SETUP_REJECTED_CODE,
            "alias_setup_report",
        ),
        (
            "/v1/aliases/setup/plan",
            "409",
            ALIAS_SETUP_REJECTED_CODE,
            "alias_setup_report",
        ),
        (
            "/v1/aliases/setup/plan",
            "503",
            ALIAS_SETUP_PENDING_CODE,
            "alias_setup_report",
        ),
        (
            "/v1/aliases/resolve",
            "404",
            ACCOUNT_ALIAS_NOT_FOUND_CODE,
            "account_alias_not_found",
        ),
        (
            "/v1/aliases/by-account",
            "404",
            ACCOUNT_ALIASES_BY_ACCOUNT_NOT_FOUND_CODE,
            "account_aliases_by_account_not_found",
        ),
    ] {
        let operation = openapi_operation(&document, path, "post");
        let response = &operation["responses"][status];
        assert_eq!(
            keys(&response["content"]),
            BTreeSet::from(["application/json", "application/x-norito"])
        );
        for media in ["application/json", "application/x-norito"] {
            assert_eq!(
                response["content"][media]["schema"],
                schema_ref("ErrorEnvelope")
            );
        }
        let description = response["description"].as_str().unwrap();
        assert!(description.contains(code));
        assert!(description.contains(&format!("details.{detail}")));
        if status == "404" {
            assert!(description.contains("exact requested"));
            assert!(description.contains("404 alone"));
        }
    }
}
