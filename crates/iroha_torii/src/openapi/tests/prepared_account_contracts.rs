#[test]
fn prepared_account_transaction_schemas_are_closed_and_exactly_typed() {
    let document = canonical_document();
    let schemas = component_schemas(&document);

    for (name, required) in [
        (
            "AccountOnboardingPlanRequest",
            &["version", "alias", "account_id", "permissions"][..],
        ),
        (
            "AccountOnboardingPlanReceipt",
            &["body", "plan_hash", "signature"][..],
        ),
        (
            "AccountOnboardingPrepareRequest",
            &["schema", "binding", "receipt", "fee_payment"][..],
        ),
        (
            "AccountOnboardingPreparedTransaction",
            &[
                "schema",
                "binding",
                "operation",
                "receipt",
                "semantic_hash_hex",
                "account_id",
                "alias",
                "disposition",
                "transaction_hash_hex",
                "signed_transaction_wire_hex",
                "signed_transaction_wire_sha256",
                "fee_payment",
                "server_signature",
            ][..],
        ),
        (
            "AccountOnboardingProofRequiredPrepareResponse",
            &[
                "schema",
                "binding",
                "operation",
                "outcome",
                "proof_kind",
                "semantic_hash_hex",
                "account_id",
                "alias",
                "disposition",
                "server_signature",
            ][..],
        ),
        (
            "AccountFaucetClaim",
            &["account_id", "pow_anchor_height", "pow_nonce_hex"][..],
        ),
        (
            "AccountFaucetPrepareRequest",
            &["schema", "binding", "claim", "fee_payment"][..],
        ),
        (
            "AccountFaucetPreparedTransaction",
            &[
                "schema",
                "binding",
                "operation",
                "claim",
                "semantic_hash_hex",
                "account_id",
                "asset_definition_id",
                "asset_id",
                "amount",
                "transaction_hash_hex",
                "signed_transaction_wire_hex",
                "signed_transaction_wire_sha256",
                "fee_payment",
                "server_signature",
            ][..],
        ),
        (
            "PreparedOperationBinding",
            &[
                "schema",
                "semantic_hash_hex",
                "kind",
                "request_id",
                "execution_expires_at_unix_ms",
            ][..],
        ),
        (
            "AccountOnboardingPlanBody",
            &[
                "version",
                "request",
                "authority",
                "network_id",
                "anchor",
                "resource",
                "acquisition",
                "quote_guard",
                "instructions",
                "owner_auto_renew_instruction",
                "valid_until_ms",
            ][..],
        ),
        ("AccountOnboardingAliasIntentV1", &["kind", "intent"][..]),
        ("AccountOnboardingAliasTargetV1", &["kind", "resource"][..]),
        ("AccountOnboardingCreateProvisionV1", &["kind", "value"][..]),
        (
            "AccountOnboardingPrimaryAliasRoleV1",
            &["kind", "value"][..],
        ),
        ("AliasPlanAnchorV1", &["block_height", "block_hash"][..]),
        ("AliasPlanDispositionV1", &["kind", "value"][..]),
        (
            "AliasPlanResourceV1",
            &["intent", "disposition", "quote", "instruction_index"][..],
        ),
        (
            "AliasLeaseAcquisitionV1",
            &["term_years", "pricing_class_hint"][..],
        ),
        (
            "AliasLeaseQuoteV1",
            &[
                "target",
                "pricing_class",
                "exact_amount",
                "guard",
                "expires_at_ms",
                "grace_expires_at_ms",
                "redemption_expires_at_ms",
            ][..],
        ),
        (
            "AliasQuoteGuardV1",
            &[
                "expected_policy_version",
                "expected_payment_asset",
                "max_amount",
                "valid_until_ms",
            ][..],
        ),
        (
            "AliasFramedInstructionV1",
            &["wire_id", "framed_payload"][..],
        ),
        (
            "ResolvedAccountAliasV1",
            &["canonical_name", "dataspace_id"][..],
        ),
        ("AccountAliasName", &["label", "domain", "dataspace"][..]),
    ] {
        assert_strict_object_schema(schemas, name, required, &[]);
    }

    let mut pending = [
        "AccountOnboardingPrepareRequest",
        "AccountOnboardingPreparedTransaction",
        "AccountOnboardingProofRequiredPrepareResponse",
        "AccountFaucetPrepareRequest",
        "AccountFaucetPreparedTransaction",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<VecDeque<_>>();
    let mut reachable = BTreeSet::new();
    while let Some(name) = pending.pop_front() {
        assert_ne!(name, "JsonValue", "prepared account schema graph is typed");
        if !reachable.insert(name.clone()) {
            continue;
        }
        let schema = schemas
            .get(&name)
            .unwrap_or_else(|| panic!("prepared account component reference: {name}"));
        let mut references = BTreeSet::new();
        collect_component_refs(schema, &mut references);
        pending.extend(references);
    }

    for (owner, property, target) in [
        (
            "AccountOnboardingPlanReceipt",
            "body",
            "AccountOnboardingPlanBody",
        ),
        ("AccountOnboardingPlanReceipt", "plan_hash", "Hash"),
        (
            "AccountOnboardingPlanReceipt",
            "signature",
            "CanonicalSignature",
        ),
        (
            "AccountOnboardingPrepareRequest",
            "binding",
            "PreparedOnboardingOperationBinding",
        ),
        (
            "AccountOnboardingPrepareRequest",
            "fee_payment",
            "FeePaymentIntent",
        ),
        (
            "AccountOnboardingPreparedTransaction",
            "disposition",
            "AliasPlanDispositionV1",
        ),
        (
            "AccountOnboardingPreparedTransaction",
            "fee_payment",
            "FeePaymentIntent",
        ),
        (
            "AccountOnboardingPreparedTransaction",
            "server_signature",
            "CanonicalSignature",
        ),
        (
            "AccountOnboardingProofRequiredPrepareResponse",
            "disposition",
            "AliasPlanDispositionV1",
        ),
        (
            "AccountOnboardingProofRequiredPrepareResponse",
            "server_signature",
            "CanonicalSignature",
        ),
        (
            "AccountFaucetPrepareRequest",
            "binding",
            "PreparedFaucetOperationBinding",
        ),
        (
            "AccountFaucetPrepareRequest",
            "fee_payment",
            "FeePaymentIntent",
        ),
        ("AccountFaucetPreparedTransaction", "amount", "Quantity"),
        (
            "AccountFaucetPreparedTransaction",
            "fee_payment",
            "FeePaymentIntent",
        ),
        (
            "AccountFaucetPreparedTransaction",
            "server_signature",
            "CanonicalSignature",
        ),
    ] {
        assert_eq!(
            property_ref(schemas, owner, property),
            format!("{COMPONENT_SCHEMA_REF_PREFIX}{target}"),
            "{owner}.{property} must not fall back to JsonValue"
        );
    }

    for (owner, property) in [
        ("AccountAliasName", "domain"),
        ("AliasLeaseAcquisitionV1", "pricing_class_hint"),
        ("AliasPlanResourceV1", "quote"),
        ("AliasPlanResourceV1", "instruction_index"),
        ("AccountOnboardingPlanBody", "owner_auto_renew_instruction"),
    ] {
        assert!(
            component_required(schemas, owner).contains(&property),
            "{owner}.{property} must remain a mandatory V1 slot"
        );
        let variants = component_properties(schemas, owner)[property]["oneOf"]
            .as_array()
            .unwrap_or_else(|| panic!("{owner}.{property} nullable union"));
        assert!(
            variants
                .iter()
                .any(|variant| variant.get("type").and_then(Value::as_str) == Some("null")),
            "{owner}.{property} must admit an explicit null"
        );
    }

    assert_eq!(
        schemas["CanonicalSignature"]["pattern"].as_str(),
        Some("^(?:[0-9A-F]{2})+$")
    );
    for owner in [
        "AccountOnboardingPreparedTransaction",
        "AccountFaucetPreparedTransaction",
    ] {
        assert_eq!(
            component_properties(schemas, owner)["signed_transaction_wire_hex"]["pattern"].as_str(),
            Some("^(?:[0-9a-f]{2})+$"),
            "{owner} wire must be nonempty byte-aligned lowercase hex"
        );
    }
    for field in ["semantic_hash_hex", "request_id"] {
        assert_eq!(
            component_properties(schemas, "PreparedOperationBinding")[field]["pattern"].as_str(),
            Some("^[0-9a-f]{64}$")
        );
    }
    for (name, kind) in [
        ("PreparedOnboardingOperationBinding", "onboarding"),
        ("PreparedFaucetOperationBinding", "faucet"),
    ] {
        let variants = schemas[name]["allOf"]
            .as_array()
            .unwrap_or_else(|| panic!("{name} allOf"));
        assert_eq!(
            variants[0].get("$ref").and_then(Value::as_str),
            Some("#/components/schemas/PreparedOperationBinding")
        );
        assert_eq!(
            variants[1]["properties"]["kind"]
                .get("const")
                .and_then(Value::as_str),
            Some(kind),
            "{name} must bind the operation kind"
        );
    }
}

#[test]
fn faucet_policy_schema_is_exact_public_discovery() {
    let document = canonical_document();
    let schemas = component_schemas(&document);
    assert_strict_object_schema(
        schemas,
        "AccountFaucetAdvertisement",
        &[
            "schema_version",
            "network_id",
            "network_prefix",
            "authority",
            "asset_definition_id",
            "amount",
        ],
        &[],
    );
    let schema = &schemas["AccountFaucetAdvertisement"];
    assert_eq!(
        schema["x-iroha-max-bytes"].as_u64(),
        Some(iroha_torii_shared::account_faucet_policy::ACCOUNT_FAUCET_POLICY_MAX_BYTES as u64)
    );
    assert_eq!(
        schema["properties"]["schema_version"]["const"].as_u64(),
        Some(1)
    );
    for (field, reference) in [
        ("network_id", "NetworkId"),
        ("authority", "CanonicalAccountId"),
        ("amount", "Quantity"),
    ] {
        assert_eq!(
            schema["properties"][field]["$ref"].as_str(),
            Some(format!("#/components/schemas/{reference}").as_str())
        );
    }
    assert_eq!(
        schema["properties"]["network_prefix"]["maximum"].as_u64(),
        Some(u64::from(u16::MAX))
    );
    let operation = &document["paths"]["/v1/accounts/faucet/policy"]["get"];
    assert_eq!(operation["x-iroha-tool-effect"].as_str(), Some("read"));
    assert_eq!(
        operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"].as_str(),
        Some("#/components/schemas/AccountFaucetAdvertisement")
    );
    assert!(operation["responses"].get("503").is_some());
    assert!(operation.get("requestBody").is_none());
    assert_eq!(
        operation["responses"]["200"]["headers"]["Cache-Control"]["schema"]["const"].as_str(),
        Some("no-store")
    );
}
