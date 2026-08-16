//! Typed, deterministic owner for the shared V1 alias-setup fixture.
use eyre::{Result, WrapErr as _, bail, eyre};
use iroha::client::{
    ACCOUNT_ONBOARDING_RECEIPT_HASH_DOMAIN_V1, AccountOnboardingPlanBodyV1,
    AccountOnboardingPlanReceiptV1, account_onboarding_test_fixture,
    decode_and_verify_account_onboarding_plan_for_request,
};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    alias_setup::{
        ALIAS_LIFECYCLE_TRANSACTION_PLAN_HASH_DOMAIN_V1, ALIAS_TRANSACTION_PLAN_HASH_DOMAIN_V1,
        AccountAliasName, AccountAliasRoleV1, AccountProvisionV1, AliasAccountIntentV1,
        AliasAssetTotalV1, AliasAutoRenewConfigV1, AliasFramedInstructionV1, AliasIntentV1,
        AliasLeaseAcquisitionV1, AliasLeaseQuoteV1, AliasLifecycleOperationV1,
        AliasLifecyclePlanDispositionV1, AliasLifecycleTransactionPlanBodyV1,
        AliasLifecycleTransactionPlanV1, AliasPlanAnchorV1, AliasPlanDispositionV1,
        AliasPlanResourceV1, AliasQuoteGuardV1, AliasSetupDiagnosticV1, AliasSetupReportV1,
        AliasSetupSeverityV1, AliasSetupStatusV1, AliasSetupValidationPhaseV1,
        AliasTransactionPlanBodyV1, AliasTransactionPlanV1, ResolvedAccountAliasV1,
        ResolvedDataSpaceV1, ResolvedDomainV1,
    },
    asset::AssetDefinitionId,
    domain::DomainId,
    isi::{
        InstructionBox,
        alias_setup::{
            CompareAndSetPrimaryAccountAlias, ConfigureAliasAutoRenew, EnsureAlias,
            RebindAccountAlias, RenewAliasLease,
        },
        framed_instruction_payload,
    },
    nexus::DataSpaceId,
};
use iroha_primitives::numeric::{Numeric, Quantity};
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
    json,
};
use norito_codegen_exporter::AliasSetupFixtureBytes;
const PLAN_NETWORK_ID_V1_JSON: &str =
    r#""hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0""#;
const FIXTURE_PAYMENT_ASSET: &str = "4rPeAP6jAjiLVZThZYwwPRBuQagt";
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AliasSetupFixtureV1 {
    schema_version: u8,
    account_alias_cases: Vec<AccountAliasCaseV1>,
    resolved_name_json_vectors: ResolvedNameVectorsV1,
    quote_guard_json_vector: AliasQuoteGuardV1,
    permission_scope_json_vector: AliasPermissionScopeVectorV1,
    account_onboarding_receipt_vector: AccountOnboardingReceiptVectorV1,
    plan_hash_vectors: Vec<PlanHashVectorV1>,
    instruction_frame_vectors: Vec<InstructionFrameVectorV1>,
    report_json_vector: AliasSetupReportV1,
}
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AccountAliasCaseV1 {
    input: String,
    canonical: String,
    label: String,
    domain: Option<String>,
    dataspace: String,
}
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ResolvedNameVectorsV1 {
    dataspace: ResolvedDataSpaceV1,
    domain: ResolvedDomainV1,
    account_alias: ResolvedAccountAliasV1,
}
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AliasPermissionScopeVectorV1 {
    scope: String,
    value: ResolvedAccountAliasV1,
}
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AccountOnboardingReceiptVectorV1 {
    name: String,
    domain: String,
    canonical_body_norito_hex: String,
    canonical_plan_hash_hex: String,
    authority: String,
    signature_hex: String,
    receipt_json: AccountOnboardingPlanReceiptV1,
}
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PlanHashVectorV1 {
    name: String,
    domain: String,
    canonical_body_norito_hex: String,
    canonical_plan_hash_hex: String,
}
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct InstructionFrameVectorV1 {
    name: String,
    wire_id: String,
    framed_payload_hex: String,
}
pub(super) fn render() -> Result<AliasSetupFixtureBytes> {
    let fixture = build_fixture()?;
    validate_fixture(&fixture)?;
    let mut rendered =
        json::to_string_pretty(&fixture).wrap_err("serialize deterministic alias-setup fixture")?;
    rendered.push('\n');
    let decoded: AliasSetupFixtureV1 =
        json::from_str(&rendered).wrap_err("decode rendered alias-setup fixture")?;
    if decoded != fixture {
        bail!("rendered alias-setup fixture failed its typed JSON round trip");
    }
    AliasSetupFixtureBytes::try_new(rendered.into_bytes())
}
fn build_fixture() -> Result<AliasSetupFixtureV1> {
    let dataspace_id = DataSpaceId::new(7);
    let resolved_alias = resolved_alias()?;
    let payment_asset = payment_asset()?;
    let quote_guard = AliasQuoteGuardV1 {
        expected_policy_version: 2,
        expected_payment_asset: payment_asset.clone(),
        max_amount: amount(10)?,
        valid_until_ms: 50_000,
    };
    let first = account(0xC1)?;
    let second = account(0xC2)?;
    let intent = AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
        alias: resolved_alias.clone(),
        target_account: first.clone(),
        provision: AccountProvisionV1::Create,
        role: AccountAliasRoleV1::Primary,
    });
    let acquisition = AliasLeaseAcquisitionV1::new(1, None);
    let ensure = EnsureAlias::new(intent.clone(), acquisition, quote_guard.clone());
    let (ensure_vector, ensure_frame) = instruction_frame("ensure_account_alias", ensure.into())?;
    let target = iroha_data_model::alias_setup::AliasTargetV1::AccountAlias(resolved_alias.clone());
    let renewal = RenewAliasLease::new(target.clone(), 1_000, 2_000, quote_guard.clone());
    let (renewal_vector, renewal_frame) =
        instruction_frame("renew_account_alias", renewal.clone().into())?;
    let auto_renew = AliasAutoRenewConfigV1 {
        term_years: 1,
        policy_version: 2,
        payment_asset: payment_asset.clone(),
        max_amount: amount(9)?,
        renew_before_expiry_ms: 100,
        retry_backoff_ms: 50,
        max_failures: 5,
    };
    let (enable_vector, _) = instruction_frame(
        "configure_auto_renew_enable",
        ConfigureAliasAutoRenew::new(target.clone(), 4, Some(auto_renew)).into(),
    )?;
    let (disable_vector, _) = instruction_frame(
        "configure_auto_renew_disable",
        ConfigureAliasAutoRenew::new(target.clone(), 5, None).into(),
    )?;
    let (rebind_vector, _) = instruction_frame(
        "rebind_account_alias",
        RebindAccountAlias::new(resolved_alias.clone(), first.clone(), second).into(),
    )?;
    let (primary_vector, _) = instruction_frame(
        "compare_and_set_primary_account_alias",
        CompareAndSetPrimaryAccountAlias::new(first.clone(), None, Some(resolved_alias.clone()))
            .into(),
    )?;
    let network_id = json::from_str::<NetworkId>(PLAN_NETWORK_ID_V1_JSON)
        .wrap_err("parse canonical alias-plan NetworkId")?;
    let setup_plan = AliasTransactionPlanV1::new(AliasTransactionPlanBodyV1 {
        version: AliasTransactionPlanBodyV1::VERSION,
        authority: first.clone(),
        network_id,
        anchor: AliasPlanAnchorV1 {
            block_height: 9,
            block_hash: Hash::new(b"alias-setup-fixture-anchor"),
        },
        resources: vec![AliasPlanResourceV1 {
            intent: intent.clone(),
            disposition: AliasPlanDispositionV1::Create,
            quote: Some(AliasLeaseQuoteV1 {
                target: intent.target(),
                pricing_class: 1,
                exact_amount: amount(3)?,
                guard: quote_guard.clone(),
                expires_at_ms: 1_000,
                grace_expires_at_ms: 2_000,
                redemption_expires_at_ms: 3_000,
            }),
            instruction_index: Some(0),
        }],
        instructions: vec![ensure_frame],
        totals_by_asset: vec![AliasAssetTotalV1 {
            payment_asset: payment_asset.clone(),
            amount: amount(3)?,
        }],
        warnings: Vec::new(),
        blockers: Vec::new(),
        valid_until_ms: 50_000,
    });
    let lifecycle_plan =
        AliasLifecycleTransactionPlanV1::new(AliasLifecycleTransactionPlanBodyV1 {
            version: AliasLifecycleTransactionPlanBodyV1::VERSION,
            authority: first,
            network_id,
            anchor: AliasPlanAnchorV1 {
                block_height: 10,
                block_hash: Hash::new(b"alias-lifecycle-fixture-anchor"),
            },
            operation: AliasLifecycleOperationV1::RenewLease(renewal),
            disposition: AliasLifecyclePlanDispositionV1::Apply,
            instruction: Some(renewal_frame),
            quote: Some(AliasLeaseQuoteV1 {
                target,
                pricing_class: 1,
                exact_amount: amount(3)?,
                guard: quote_guard.clone(),
                expires_at_ms: 2_000,
                grace_expires_at_ms: 2_100,
                redemption_expires_at_ms: 2_200,
            }),
            totals_by_asset: vec![AliasAssetTotalV1 {
                payment_asset: payment_asset.clone(),
                amount: amount(3)?,
            }],
            warnings: Vec::new(),
            blockers: Vec::new(),
            valid_until_ms: 50_000,
        });
    Ok(AliasSetupFixtureV1 {
        schema_version: 1,
        account_alias_cases: vec![
            alias_case("Merchant@Banka.Paynet")?,
            alias_case("Merchant@Paynet")?,
        ],
        resolved_name_json_vectors: ResolvedNameVectorsV1 {
            dataspace: ResolvedDataSpaceV1::new("paynet".parse()?, dataspace_id),
            domain: ResolvedDomainV1::new(
                DomainId::try_new("banka", "paynet")?,
                dataspace_id,
            ),
            account_alias: resolved_alias.clone(),
        },
        quote_guard_json_vector: quote_guard,
        permission_scope_json_vector: AliasPermissionScopeVectorV1 {
            scope: "alias".to_owned(),
            value: resolved_alias,
        },
        account_onboarding_receipt_vector: onboarding_receipt_vector()?,
        plan_hash_vectors: vec![
            setup_plan_vector(setup_plan)?,
            lifecycle_plan_vector(lifecycle_plan)?,
        ],
        instruction_frame_vectors: vec![
            ensure_vector,
            renewal_vector,
            enable_vector,
            disable_vector,
            rebind_vector,
            primary_vector,
        ],
        report_json_vector: AliasSetupReportV1::new(
            AliasSetupStatusV1::Blocked,
            vec![AliasSetupDiagnosticV1 {
                phase: AliasSetupValidationPhaseV1::Catalog,
                code: "alias.catalog.mapping_conflict".to_owned(),
                severity: AliasSetupSeverityV1::Error,
                resource: Some("dataspace:paynet".to_owned()),
                config_path: None,
                expected: Some("7".to_owned()),
                actual: Some("9".to_owned()),
                remediation: "Make the static catalog and active SNS record map paynet to the same dataspace ID."
                    .to_owned(),
            }],
        ),
    })
}
fn account(seed: u8) -> Result<AccountId> {
    let key_pair = KeyPair::try_from_seed([seed; 32].to_vec(), Algorithm::Ed25519)
        .wrap_err_with(|| format!("derive alias fixture account for seed {seed:#04x}"))?;
    Ok(AccountId::new(key_pair.public_key().clone()))
}
fn amount(value: u32) -> Result<Quantity> {
    Quantity::from_canonical_numeric(Numeric::new(value, 0))
        .map_err(|error| eyre!("invalid alias fixture quantity {value}: {error}"))
}
fn payment_asset() -> Result<AssetDefinitionId> {
    FIXTURE_PAYMENT_ASSET
        .parse()
        .map_err(|error| eyre!("invalid alias fixture payment asset: {error}"))
}
fn resolved_alias() -> Result<ResolvedAccountAliasV1> {
    Ok(ResolvedAccountAliasV1::new(
        "merchant@banka.paynet".parse::<AccountAliasName>()?,
        DataSpaceId::new(7),
    ))
}
fn alias_case(input: &str) -> Result<AccountAliasCaseV1> {
    let parsed = input.parse::<AccountAliasName>()?;
    Ok(AccountAliasCaseV1 {
        input: input.to_owned(),
        canonical: parsed.canonical_text(),
        label: parsed.label.to_string(),
        domain: parsed.domain.as_ref().map(ToString::to_string),
        dataspace: parsed.dataspace.to_string(),
    })
}
fn instruction_frame(
    name: &str,
    instruction: InstructionBox,
) -> Result<(InstructionFrameVectorV1, AliasFramedInstructionV1)> {
    let (wire_id, framed_payload) = framed_instruction_payload(&instruction)
        .ok_or_else(|| eyre!("failed to frame alias fixture instruction {name}"))?;
    Ok((
        InstructionFrameVectorV1 {
            name: name.to_owned(),
            wire_id: wire_id.to_owned(),
            framed_payload_hex: hex::encode(&framed_payload),
        },
        AliasFramedInstructionV1 {
            wire_id: wire_id.to_owned(),
            framed_payload,
        },
    ))
}
fn onboarding_receipt_vector() -> Result<AccountOnboardingReceiptVectorV1> {
    let receipt = account_onboarding_test_fixture::receipt_v1()?;
    let canonical_body = receipt.body.encode();
    Ok(AccountOnboardingReceiptVectorV1 {
        name: "sponsored_account_alias_create".to_owned(),
        domain: String::from_utf8(ACCOUNT_ONBOARDING_RECEIPT_HASH_DOMAIN_V1.to_vec())
            .expect("onboarding receipt hash domain is UTF-8"),
        canonical_body_norito_hex: hex::encode(canonical_body),
        canonical_plan_hash_hex: hex::encode(receipt.plan_hash.as_ref()),
        authority: receipt.body.authority.to_string(),
        signature_hex: hex::encode_upper(receipt.signature.payload()),
        receipt_json: receipt,
    })
}
fn setup_plan_vector(plan: AliasTransactionPlanV1) -> Result<PlanHashVectorV1> {
    if !plan.verify_hash() {
        bail!("deterministic alias setup plan hash is invalid");
    }
    Ok(PlanHashVectorV1 {
        name: "setup_account_alias_create".to_owned(),
        domain: String::from_utf8(ALIAS_TRANSACTION_PLAN_HASH_DOMAIN_V1.to_vec())
            .expect("alias setup plan hash domain is UTF-8"),
        canonical_body_norito_hex: hex::encode(plan.body.encode()),
        canonical_plan_hash_hex: hex::encode(plan.plan_hash.as_ref()),
    })
}
fn lifecycle_plan_vector(plan: AliasLifecycleTransactionPlanV1) -> Result<PlanHashVectorV1> {
    if !plan.verify_hash() {
        bail!("deterministic alias lifecycle plan hash is invalid");
    }
    Ok(PlanHashVectorV1 {
        name: "renew_account_alias".to_owned(),
        domain: String::from_utf8(ALIAS_LIFECYCLE_TRANSACTION_PLAN_HASH_DOMAIN_V1.to_vec())
            .expect("alias lifecycle plan hash domain is UTF-8"),
        canonical_body_norito_hex: hex::encode(plan.body.encode()),
        canonical_plan_hash_hex: hex::encode(plan.plan_hash.as_ref()),
    })
}
fn validate_fixture(fixture: &AliasSetupFixtureV1) -> Result<()> {
    if fixture.schema_version != 1 {
        bail!("alias-setup fixture schema_version must be exactly 1");
    }
    let onboarding = &fixture.account_onboarding_receipt_vector;
    if onboarding.domain.as_bytes() != ACCOUNT_ONBOARDING_RECEIPT_HASH_DOMAIN_V1 {
        bail!("alias-setup onboarding receipt domain is not canonical");
    }
    let expected_network_id = account_onboarding_test_fixture::network_id_v1();
    if onboarding.receipt_json.body.network_id != expected_network_id {
        bail!("alias-setup onboarding receipt uses a foreign genesis identity");
    }
    let decoded_instructions = decode_and_verify_account_onboarding_plan_for_request(
        expected_network_id,
        &onboarding.receipt_json.body.request,
        &onboarding.receipt_json,
    )?;
    if decoded_instructions.len() != 1 {
        bail!("alias-setup onboarding receipt must decode exactly one instruction");
    }
    let body_bytes = decode_lower_hex(
        &onboarding.canonical_body_norito_hex,
        "onboarding canonical body",
    )?;
    let mut body_slice = body_bytes.as_slice();
    let decoded_body = AccountOnboardingPlanBodyV1::decode(&mut body_slice)
        .wrap_err("decode alias-setup onboarding canonical body")?;
    if !body_slice.is_empty() || decoded_body != onboarding.receipt_json.body {
        bail!("alias-setup onboarding canonical body differs from its typed receipt");
    }
    if onboarding.canonical_plan_hash_hex
        != hex::encode(onboarding.receipt_json.body.canonical_hash().as_ref())
        || onboarding.canonical_plan_hash_hex
            != hex::encode(onboarding.receipt_json.plan_hash.as_ref())
        || onboarding.authority != onboarding.receipt_json.body.authority.to_string()
        || onboarding.signature_hex
            != hex::encode_upper(onboarding.receipt_json.signature.payload())
        || !onboarding.receipt_json.verify()
    {
        bail!("alias-setup onboarding hash, authority, or signature summary is inconsistent");
    }
    let expected_plan_domains = [
        (
            "setup_account_alias_create",
            ALIAS_TRANSACTION_PLAN_HASH_DOMAIN_V1,
        ),
        (
            "renew_account_alias",
            ALIAS_LIFECYCLE_TRANSACTION_PLAN_HASH_DOMAIN_V1,
        ),
    ];
    if fixture.plan_hash_vectors.len() != expected_plan_domains.len() {
        bail!("alias-setup fixture must contain exactly two plan hash vectors");
    }
    for (name, domain) in expected_plan_domains {
        let vector = fixture
            .plan_hash_vectors
            .iter()
            .find(|vector| vector.name == name)
            .ok_or_else(|| eyre!("missing alias plan hash vector {name}"))?;
        if vector.domain.as_bytes() != domain {
            bail!("alias plan hash vector {name} uses the wrong domain");
        }
        let body = decode_lower_hex(&vector.canonical_body_norito_hex, name)?;
        let expected_hash = Hash::new_from_chunks(&[domain, body.as_slice()]);
        if vector.canonical_plan_hash_hex != hex::encode(expected_hash.as_ref()) {
            bail!("alias plan hash vector {name} does not commit its canonical body");
        }
        let mut body_slice = body.as_slice();
        match name {
            "setup_account_alias_create" => {
                let decoded = AliasTransactionPlanBodyV1::decode(&mut body_slice)
                    .wrap_err("decode alias setup plan body")?;
                if decoded.network_id.to_string() != PLAN_NETWORK_ID_V1 || decoded.encode() != body
                {
                    bail!("alias setup plan body is not canonical");
                }
            }
            "renew_account_alias" => {
                let decoded = AliasLifecycleTransactionPlanBodyV1::decode(&mut body_slice)
                    .wrap_err("decode alias lifecycle plan body")?;
                if decoded.network_id.to_string() != PLAN_NETWORK_ID_V1 || decoded.encode() != body
                {
                    bail!("alias lifecycle plan body is not canonical");
                }
            }
            _ => unreachable!("closed alias plan vector names"),
        }
        if !body_slice.is_empty() {
            bail!("alias plan hash vector {name} has trailing Norito bytes");
        }
    }
    let expected_frames = [
        ("ensure_account_alias", EnsureAlias::WIRE_ID),
        ("renew_account_alias", RenewAliasLease::WIRE_ID),
        (
            "configure_auto_renew_enable",
            ConfigureAliasAutoRenew::WIRE_ID,
        ),
        (
            "configure_auto_renew_disable",
            ConfigureAliasAutoRenew::WIRE_ID,
        ),
        ("rebind_account_alias", RebindAccountAlias::WIRE_ID),
        (
            "compare_and_set_primary_account_alias",
            CompareAndSetPrimaryAccountAlias::WIRE_ID,
        ),
    ];
    if fixture.instruction_frame_vectors.len() != expected_frames.len() {
        bail!("alias-setup fixture must contain exactly six instruction frames");
    }
    let registry = iroha_data_model::instruction_registry::default();
    for (name, wire_id) in expected_frames {
        let vector = fixture
            .instruction_frame_vectors
            .iter()
            .find(|vector| vector.name == name)
            .ok_or_else(|| eyre!("missing alias instruction frame {name}"))?;
        if vector.wire_id != wire_id {
            bail!("alias instruction frame {name} uses the wrong wire id");
        }
        let frame = decode_lower_hex(&vector.framed_payload_hex, name)?;
        let decoded = registry
            .decode(wire_id, &frame)
            .ok_or_else(|| eyre!("unregistered alias instruction {name}"))?
            .wrap_err_with(|| format!("decode registered alias instruction {name}"))?;
        let (roundtrip_wire_id, roundtrip_frame) = framed_instruction_payload(&decoded)
            .ok_or_else(|| eyre!("failed to re-encode alias instruction {name}"))?;
        if roundtrip_wire_id != wire_id || roundtrip_frame != frame {
            bail!("alias instruction frame {name} is not canonical");
        }
    }
    Ok(())
}
fn decode_lower_hex(value: &str, label: &str) -> Result<Vec<u8>> {
    let decoded = hex::decode(value).wrap_err_with(|| format!("decode {label} hex"))?;
    if hex::encode(&decoded) != value {
        bail!("{label} must use canonical lowercase hex");
    }
    Ok(decoded)
}
#[cfg(test)]
mod tests {
    use super::*;
    use norito::json::{self, Value};
    #[test]
    fn owner_is_deterministic_and_uses_the_real_receipt_type() {
        assert_eq!(account_onboarding_test_fixture::NETWORK_HASH_SEED_V1, 0xA1);
        assert_eq!(
            account_onboarding_test_fixture::TARGET_ACCOUNT_KEY_SEED_V1,
            0x22
        );
        assert_eq!(
            account_onboarding_test_fixture::ONBOARDING_AUTHORITY_KEY_SEED_V1,
            0x51
        );
        let first = render().expect("first alias fixture render");
        let second = render().expect("second alias fixture render");
        assert_eq!(first, second);
        let fixture = build_fixture().expect("typed alias fixture");
        assert_eq!(
            fixture
                .account_onboarding_receipt_vector
                .receipt_json
                .body
                .network_id,
            account_onboarding_test_fixture::network_id_v1()
        );
        assert!(
            fixture
                .account_onboarding_receipt_vector
                .receipt_json
                .verify()
        );
    }
    #[test]
    fn onboarding_json_rejects_genesis_and_retired_identity_aliases() {
        let fixture = build_fixture().expect("typed alias fixture");
        let canonical = json::to_value(&fixture).expect("alias fixture JSON value");
        let canonical_body = canonical
            .as_object()
            .and_then(|root| root.get("account_onboarding_receipt_vector"))
            .and_then(Value::as_object)
            .and_then(|vector| vector.get("receipt_json"))
            .and_then(Value::as_object)
            .and_then(|receipt| receipt.get("body"))
            .and_then(Value::as_object)
            .expect("typed onboarding receipt body");
        assert!(canonical_body.contains_key("network_id"));
        for retired in [
            "chain",
            "chainId",
            "chain_id",
            "genesis",
            "genesisHash",
            "genesis_hash",
        ] {
            let mut alias_only = canonical.clone();
            let body = onboarding_body_mut(&mut alias_only);
            body.remove("network_id");
            body.insert(retired.to_owned(), Value::String("same-label".to_owned()));
            json::from_value::<AliasSetupFixtureV1>(alias_only)
                .expect_err("retired alias must not replace network_id");
            let mut dual = canonical.clone();
            onboarding_body_mut(&mut dual)
                .insert(retired.to_owned(), Value::String("same-label".to_owned()));
            json::from_value::<AliasSetupFixtureV1>(dual)
                .expect_err("retired alias must remain unknown beside network_id");
        }
        let mut genesis = canonical;
        onboarding_body_mut(&mut genesis)
            .insert("network_id".to_owned(), Value::String("genesis".to_owned()));
        json::from_value::<AliasSetupFixtureV1>(genesis)
            .expect_err("genesis shorthand must not decode as a NetworkId");
    }
    fn onboarding_body_mut(value: &mut Value) -> &mut json::Map {
        value
            .as_object_mut()
            .and_then(|root| root.get_mut("account_onboarding_receipt_vector"))
            .and_then(Value::as_object_mut)
            .and_then(|vector| vector.get_mut("receipt_json"))
            .and_then(Value::as_object_mut)
            .and_then(|receipt| receipt.get_mut("body"))
            .and_then(Value::as_object_mut)
            .expect("typed onboarding receipt body")
    }
}
