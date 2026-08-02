//! Secret-free composition of the exact first-release Taira privacy inputs.

use std::{
    collections::BTreeSet,
    io::Write,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use clap::Args as ClapArgs;
use color_eyre::eyre::{WrapErr as _, bail, eyre};
use iroha_core::privacy_engines::bootle_lantern::issuer::{
    TairaBootleLanternBrokerQualificationInputsV1,
    derive_taira_bootle_lantern_broker_qualification_digest_v1,
    taira_bootle_lantern_broker_contract_digest_v1,
    taira_bootle_lantern_issuer_profile_contract_digest_v1,
};
use iroha_crypto::sha256;
use iroha_data_model::{
    isi::{
        InstructionBox,
        privacy::{
            RegisterPrivacyBootleLanternIssuerPolicyV1, RegisterPrivacyProtocolActivationV1,
        },
    },
    privacy::{
        BootleLanternIssuerPolicyLifecycleV1, PRIVACY_RETIRED_PROTOCOL_LABELS_V1,
        PrivacyProtocolIdV1, privacy_exact12_matrix_bytes_v1,
    },
};
use iroha_genesis::RawGenesisTransaction;
use norito::json::{Map as JsonMap, Value as JsonValue};

use super::{
    MAX_INSTRUCTIONS_JSON_BYTES_V1, MAX_REPORT_JSON_BYTES_V1, create_new_file, read_bounded,
    remove_created_file_if_unchanged_v1, resolved_new_output_path_v1,
    validate_taira_privacy_bootstrap_v1,
};

const MAX_TEMPLATE_BYTES_V1: u64 = 8 * 1024 * 1024;
const MAX_BROKER_EXPORT_BYTES_V1: u64 = 4 * 1024 * 1024;
const CHAIN_ID_V1: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
const CHAIN_DISCRIMINANT_V1: u64 = 369;
const GENESIS_AUTHORITY_V1: &str = "testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A";
const PROVIDER_HANDLE_V1: &str = "runtime://privacy/bootle-lantern/taira-primary";
const PROVIDER_REVISION_V1: u64 = 1;
const AUTHORIZATION_LIFETIME_BLOCKS_V1: u64 = 300;
const MAX_INFLIGHT_V1: u64 = 2;
const ISSUER_ID_DOMAIN_V1: &[u8] = b"iroha.taira.privacy.bootle-lantern.issuer.v1";
const POLICY_ID_DOMAIN_V1: &[u8] = b"iroha.taira.privacy.bootle-lantern.policy.v1";
const BROKER_EXPORT_SCHEMA_V1: &str = "iroha.taira.privacy.bootle-lantern-broker-public.v1";
const CANONICAL_PLAN_TEMPLATE_V1: &[u8] =
    include_bytes!("../../../../configs/soranexus/taira/privacy_bootstrap_plan.json");
const CANONICAL_CONFIG_TEMPLATE_V1: &[u8] =
    include_bytes!("../../../../configs/soranexus/taira/config.toml");
const CANONICAL_GENESIS_TEMPLATE_V1: &[u8] =
    include_bytes!("../../../../configs/soranexus/taira/genesis.json");

/// Inputs and fresh output paths for one complete Taira privacy release set.
#[derive(Debug, ClapArgs)]
pub(super) struct RenderTairaReleaseV1Args {
    /// Exact-12 instruction JSON emitted by `emit-taira-v1`.
    #[arg(long)]
    activation_instructions: PathBuf,
    /// Digest report emitted together with the exact-12 instructions.
    #[arg(long)]
    activation_report: PathBuf,
    /// Canonical public JSON emitted by the qualified peer-1 broker.
    #[arg(long)]
    broker_public_export: PathBuf,
    /// Canonical disabled Taira privacy plan template.
    #[arg(long)]
    plan_template: PathBuf,
    /// Canonical disabled peer-1 Taira config template.
    #[arg(long)]
    config_template: PathBuf,
    /// Canonical Taira genesis without privacy bootstrap instructions.
    #[arg(long)]
    genesis_template: PathBuf,
    /// Fresh output path for the complete public release plan.
    #[arg(long)]
    plan_output: PathBuf,
    /// Fresh output path for the complete peer-1 release config.
    #[arg(long)]
    config_output: PathBuf,
    /// Fresh output path for the complete release genesis.
    #[arg(long)]
    genesis_output: PathBuf,
    /// Fresh output path for the verified canonical public broker export.
    #[arg(long)]
    broker_public_output: PathBuf,
}

#[derive(Debug)]
struct BrokerPublicMaterialV1 {
    public_export_sha256: String,
    qualification_policy_digest_hex: String,
    issuer_parameter_id_hex: String,
    issuer_parameter_digest_hex: String,
    policy_record_digest_hex: String,
    instruction_norito_sha256: String,
    instruction_base64: String,
    instruction: InstructionBox,
}

#[derive(Debug)]
struct ReleaseArtifactsV1 {
    plan: Vec<u8>,
    config: Vec<u8>,
    genesis: Vec<u8>,
    broker_public: Vec<u8>,
}

pub(super) fn render_taira_release_v1<T: Write>(
    args: RenderTairaReleaseV1Args,
    writer: &mut std::io::BufWriter<T>,
) -> color_eyre::Result<()> {
    let activation_instructions = read_bounded(
        &args.activation_instructions,
        MAX_INSTRUCTIONS_JSON_BYTES_V1,
        "Taira privacy activation instructions",
    )?;
    let activation_report = read_bounded(
        &args.activation_report,
        MAX_REPORT_JSON_BYTES_V1,
        "Taira privacy activation report",
    )?;
    let broker_export = read_bounded(
        &args.broker_public_export,
        MAX_BROKER_EXPORT_BYTES_V1,
        "Taira Bootle/Lantern broker public export",
    )?;
    let plan_template = read_bounded(
        &args.plan_template,
        MAX_TEMPLATE_BYTES_V1,
        "Taira privacy plan template",
    )?;
    let config_template = read_bounded(
        &args.config_template,
        MAX_TEMPLATE_BYTES_V1,
        "Taira config template",
    )?;
    let genesis_template = read_bounded(
        &args.genesis_template,
        MAX_TEMPLATE_BYTES_V1,
        "Taira genesis template",
    )?;

    let artifacts = compose_release_artifacts_v1(
        &activation_instructions,
        &activation_report,
        &broker_export,
        &plan_template,
        &config_template,
        &genesis_template,
    )?;
    write_new_artifact_set_v1([
        (
            &args.plan_output,
            artifacts.plan.as_slice(),
            "Taira privacy release plan",
        ),
        (
            &args.config_output,
            artifacts.config.as_slice(),
            "Taira peer-1 release config",
        ),
        (
            &args.genesis_output,
            artifacts.genesis.as_slice(),
            "Taira privacy release genesis",
        ),
        (
            &args.broker_public_output,
            artifacts.broker_public.as_slice(),
            "verified Taira broker public export",
        ),
    ])?;
    let status = norito::json!({
        "status": "rendered",
        "plan_path": (args.plan_output.display().to_string()),
        "plan_sha256": (hex::encode(sha256(&artifacts.plan))),
        "config_path": (args.config_output.display().to_string()),
        "config_sha256": (hex::encode(sha256(&artifacts.config))),
        "genesis_path": (args.genesis_output.display().to_string()),
        "genesis_sha256": (hex::encode(sha256(&artifacts.genesis))),
        "broker_public_path": (args.broker_public_output.display().to_string()),
        "broker_public_sha256": (hex::encode(sha256(&artifacts.broker_public))),
        "activation_instruction_count": (PrivacyProtocolIdV1::COUNT as u64),
        "issuer_policy_instruction_count": 1_u64,
    });
    writeln!(writer, "{}", norito::json::to_json(&status)?)?;
    Ok(())
}

fn compose_release_artifacts_v1(
    activation_instructions: &[u8],
    activation_report: &[u8],
    broker_export: &[u8],
    plan_template: &[u8],
    config_template: &[u8],
    genesis_template: &[u8],
) -> color_eyre::Result<ReleaseArtifactsV1> {
    validate_taira_privacy_bootstrap_v1(activation_instructions, activation_report)?;
    let (activation_hashes, activation_base64, activation_boxes) =
        activation_material_v1(activation_report)?;
    let broker = parse_broker_public_export_v1(broker_export)?;
    let plan = render_release_plan_v1(plan_template, &activation_hashes, &broker)?;
    let config = render_release_config_v1(config_template, &broker)?;
    let genesis = render_release_genesis_v1(
        genesis_template,
        &activation_base64,
        &activation_boxes,
        &broker,
    )?;
    Ok(ReleaseArtifactsV1 {
        plan,
        config,
        genesis,
        broker_public: broker_export.to_vec(),
    })
}

fn activation_material_v1(
    report_json: &[u8],
) -> color_eyre::Result<(Vec<String>, Vec<String>, Vec<InstructionBox>)> {
    let report: JsonValue = norito::json::from_slice(report_json)
        .wrap_err("failed to decode validated Taira activation report")?;
    let registration = object_field_v1(
        object_v1(&report, "activation report")?,
        "genesis_registration",
        "activation report",
    )?;
    let hashes = string_array_field_v1(
        registration,
        "instruction_norito_sha256",
        "activation report",
    )?;
    let encoded = string_array_field_v1(
        registration,
        "instruction_norito_base64",
        "activation report",
    )?;
    if hashes.len() != PrivacyProtocolIdV1::COUNT || encoded.len() != PrivacyProtocolIdV1::COUNT {
        bail!("validated activation report did not retain the exact-12 inventory");
    }
    let mut boxes = Vec::with_capacity(encoded.len());
    for (index, value) in encoded.iter().enumerate() {
        let bytes = BASE64_STANDARD
            .decode(value)
            .wrap_err_with(|| format!("activation instruction {index} is not base64"))?;
        let instruction =
            norito::decode_from_bytes::<InstructionBox>(&bytes).wrap_err_with(|| {
                format!("activation instruction {index} is not an instruction box")
            })?;
        if norito::to_bytes(&instruction).wrap_err("failed to re-encode activation instruction")?
            != bytes
        {
            bail!("activation instruction {index} is not canonical Norito");
        }
        boxes.push(instruction);
    }
    Ok((hashes, encoded, boxes))
}

fn parse_broker_public_export_v1(bytes: &[u8]) -> color_eyre::Result<BrokerPublicMaterialV1> {
    let export: JsonValue = norito::json::from_slice(bytes)
        .wrap_err("Taira Bootle/Lantern broker public export is not strict JSON")?;
    let canonical = format!("{}\n", norito::json::to_json(&export)?);
    if canonical.as_bytes() != bytes {
        bail!("Taira Bootle/Lantern broker public export is not in canonical emitted form");
    }
    let fields = object_v1(&export, "broker public export")?;
    expect_exact_keys_v1(
        fields,
        &[
            "authorization_lifetime_blocks",
            "broker_contract_digest_hex",
            "chain_id",
            "issuer_id_hex",
            "issuer_parameter_digest_hex",
            "issuer_parameter_id_hex",
            "issuer_profile_digest_hex",
            "policy_id_hex",
            "policy_record_digest_hex",
            "registration_instruction",
            "registration_instruction_norito_hex",
            "registration_instruction_norito_sha256",
            "runtime_provider_handle",
            "runtime_provider_policy_digest_hex",
            "runtime_provider_revision",
            "schema",
            "stable_principal_digest_hex",
        ],
        "broker public export",
    )?;
    expect_string_v1(
        fields,
        "schema",
        BROKER_EXPORT_SCHEMA_V1,
        "broker public export",
    )?;
    expect_string_v1(fields, "chain_id", CHAIN_ID_V1, "broker public export")?;
    expect_string_v1(
        fields,
        "runtime_provider_handle",
        PROVIDER_HANDLE_V1,
        "broker public export",
    )?;
    expect_u64_v1(
        fields,
        "runtime_provider_revision",
        PROVIDER_REVISION_V1,
        "broker public export",
    )?;
    expect_u64_v1(
        fields,
        "authorization_lifetime_blocks",
        AUTHORIZATION_LIFETIME_BLOCKS_V1,
        "broker public export",
    )?;

    let expected_issuer_id = hex::encode(sha256(ISSUER_ID_DOMAIN_V1));
    let expected_policy_id = hex::encode(sha256(POLICY_ID_DOMAIN_V1));
    expect_string_v1(
        fields,
        "issuer_id_hex",
        &expected_issuer_id,
        "broker public export",
    )?;
    expect_string_v1(
        fields,
        "policy_id_hex",
        &expected_policy_id,
        "broker public export",
    )?;

    for field in [
        "runtime_provider_policy_digest_hex",
        "issuer_parameter_id_hex",
        "issuer_parameter_digest_hex",
        "policy_record_digest_hex",
        "stable_principal_digest_hex",
        "issuer_profile_digest_hex",
        "broker_contract_digest_hex",
        "registration_instruction_norito_sha256",
    ] {
        fixed_nonzero_sha256_v1(
            string_field_v1(fields, field, "broker public export")?,
            field,
        )?;
    }

    let instruction_hex = string_field_v1(
        fields,
        "registration_instruction_norito_hex",
        "broker public export",
    )?;
    if instruction_hex.is_empty()
        || instruction_hex.len() % 2 != 0
        || !instruction_hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("broker registration instruction must be non-empty canonical lowercase hex");
    }
    let instruction_bytes = hex::decode(instruction_hex)
        .wrap_err("failed to decode broker registration instruction hex")?;
    let claimed_instruction_sha256 = string_field_v1(
        fields,
        "registration_instruction_norito_sha256",
        "broker public export",
    )?;
    if hex::encode(sha256(&instruction_bytes)) != claimed_instruction_sha256 {
        bail!("broker registration instruction SHA-256 does not match its boxed Norito bytes");
    }
    let instruction = norito::decode_from_bytes::<InstructionBox>(&instruction_bytes)
        .wrap_err("broker registration instruction is not a canonical instruction box")?;
    if norito::to_bytes(&instruction)
        .wrap_err("failed to re-encode broker registration instruction box")?
        != instruction_bytes
    {
        bail!("broker registration instruction box is not canonical Norito");
    }
    let registration = instruction
        .as_any()
        .downcast_ref::<RegisterPrivacyBootleLanternIssuerPolicyV1>()
        .ok_or_else(|| eyre!("broker export does not contain the issuer-policy registration"))?;
    registration
        .policy
        .validate()
        .map_err(|source| eyre!("broker issuer policy is invalid: {source}"))?;
    if registration.policy.epoch != 1
        || registration.policy.lifecycle != BootleLanternIssuerPolicyLifecycleV1::Active
        || registration.policy.required_disclosure_bitmap != 0
    {
        bail!(
            "broker issuer policy differs from the exact first-release lifecycle and disclosure policy"
        );
    }
    if hex::encode(registration.policy.issuer_id.as_bytes()) != expected_issuer_id
        || hex::encode(registration.policy.policy_id.as_bytes()) != expected_policy_id
    {
        bail!("broker issuer-policy identity differs from the exact Taira identity");
    }
    let issuer_parameter_id_hex = hex::encode(registration.policy.issuer_parameter_id.as_bytes());
    let issuer_parameter_digest_hex =
        hex::encode(registration.policy.issuer_parameter_digest.as_bytes());
    let policy_record_digest_hex = hex::encode(registration.policy.record_digest.as_bytes());
    for (field, expected) in [
        ("issuer_parameter_id_hex", issuer_parameter_id_hex.as_str()),
        (
            "issuer_parameter_digest_hex",
            issuer_parameter_digest_hex.as_str(),
        ),
        (
            "policy_record_digest_hex",
            policy_record_digest_hex.as_str(),
        ),
    ] {
        expect_string_v1(fields, field, expected, "broker public export")?;
    }
    expect_string_v1(
        fields,
        "issuer_profile_digest_hex",
        &hex::encode(taira_bootle_lantern_issuer_profile_contract_digest_v1()),
        "broker public export",
    )?;
    expect_string_v1(
        fields,
        "broker_contract_digest_hex",
        &hex::encode(taira_bootle_lantern_broker_contract_digest_v1()),
        "broker public export",
    )?;
    let stable_principal_digest = decode_sha256_v1(
        string_field_v1(
            fields,
            "stable_principal_digest_hex",
            "broker public export",
        )?,
        "stable_principal_digest_hex",
    )?;
    let qualification = derive_taira_bootle_lantern_broker_qualification_digest_v1(
        &TairaBootleLanternBrokerQualificationInputsV1 {
            chain_id: CHAIN_ID_V1,
            runtime_provider_handle: PROVIDER_HANDLE_V1,
            runtime_provider_revision: PROVIDER_REVISION_V1,
            issuer_id: registration.policy.issuer_id,
            policy_id: registration.policy.policy_id,
            authorization_lifetime_blocks: AUTHORIZATION_LIFETIME_BLOCKS_V1,
            policy: &registration.policy,
            stable_principal_digest,
        },
    )
    .map_err(|source| eyre!("broker provider qualification is invalid: {source}"))?;
    expect_string_v1(
        fields,
        "runtime_provider_policy_digest_hex",
        &hex::encode(qualification),
        "broker public export",
    )?;
    let expected_registration_json = norito::json::to_value(registration)
        .wrap_err("failed to project decoded broker registration to JSON")?;
    if fields.get("registration_instruction") != Some(&expected_registration_json) {
        bail!("broker structured registration differs from its boxed Norito instruction");
    }

    Ok(BrokerPublicMaterialV1 {
        public_export_sha256: hex::encode(sha256(bytes)),
        qualification_policy_digest_hex: string_field_v1(
            fields,
            "runtime_provider_policy_digest_hex",
            "broker public export",
        )?
        .to_owned(),
        issuer_parameter_id_hex,
        issuer_parameter_digest_hex,
        policy_record_digest_hex,
        instruction_norito_sha256: claimed_instruction_sha256.to_owned(),
        instruction_base64: BASE64_STANDARD.encode(&instruction_bytes),
        instruction,
    })
}

fn render_release_plan_v1(
    bytes: &[u8],
    activation_hashes: &[String],
    broker: &BrokerPublicMaterialV1,
) -> color_eyre::Result<Vec<u8>> {
    let mut plan: JsonValue = norito::json::from_slice(bytes)
        .wrap_err("Taira privacy plan template is not strict JSON")?;
    validate_staging_plan_v1(&plan)?;
    expect_canonical_template_bytes_v1(bytes, CANONICAL_PLAN_TEMPLATE_V1, "Taira privacy plan")?;
    let root = object_mut_v1(&mut plan, "privacy plan")?;
    let registration = object_field_mut_v1(root, "genesis_registration", "privacy plan")?;
    registration.insert(
        "instruction_norito_sha256".to_owned(),
        JsonValue::Array(
            activation_hashes
                .iter()
                .cloned()
                .map(JsonValue::String)
                .collect(),
        ),
    );
    let bootle = object_field_mut_v1(root, "bootle_lantern_issuer", "privacy plan")?;
    bootle.insert(
        "public_export_sha256".to_owned(),
        JsonValue::String(broker.public_export_sha256.clone()),
    );
    let provider = object_field_mut_v1(bootle, "runtime_provider", "Bootle/Lantern plan")?;
    provider.insert(
        "qualification_policy_digest_hex".to_owned(),
        JsonValue::String(broker.qualification_policy_digest_hex.clone()),
    );
    let policy = object_field_mut_v1(bootle, "governed_issuer_policy", "Bootle/Lantern plan")?;
    for (field, value) in [
        (
            "instruction_norito_sha256",
            broker.instruction_norito_sha256.as_str(),
        ),
        (
            "issuer_parameter_id_hex",
            broker.issuer_parameter_id_hex.as_str(),
        ),
        (
            "issuer_parameter_digest_hex",
            broker.issuer_parameter_digest_hex.as_str(),
        ),
        (
            "record_digest_hex",
            broker.policy_record_digest_hex.as_str(),
        ),
    ] {
        policy.insert(field.to_owned(), JsonValue::String(value.to_owned()));
    }
    json_pretty_bytes_v1(&plan, "Taira privacy release plan")
}

fn validate_staging_plan_v1(plan: &JsonValue) -> color_eyre::Result<()> {
    let root = object_v1(plan, "privacy plan")?;
    expect_exact_keys_v1(
        root,
        &[
            "bootle_lantern_issuer",
            "chain_discriminant",
            "chain_id",
            "genesis_authority",
            "genesis_registration",
            "governance_permission",
            "privacy_catalog",
            "schema",
            "schema_version",
        ],
        "privacy plan",
    )?;
    expect_string_v1(
        root,
        "schema",
        "iroha.taira.privacy_bootstrap_plan.v1",
        "privacy plan",
    )?;
    expect_u64_v1(root, "schema_version", 1, "privacy plan")?;
    expect_string_v1(root, "chain_id", CHAIN_ID_V1, "privacy plan")?;
    expect_u64_v1(
        root,
        "chain_discriminant",
        CHAIN_DISCRIMINANT_V1,
        "privacy plan",
    )?;
    expect_string_v1(
        root,
        "genesis_authority",
        GENESIS_AUTHORITY_V1,
        "privacy plan",
    )?;
    expect_string_v1(
        root,
        "governance_permission",
        "CanEnactGovernance",
        "privacy plan",
    )?;

    let registration = object_field_v1(root, "genesis_registration", "privacy plan")?;
    expect_exact_keys_v1(
        registration,
        &[
            "activate_at_height",
            "assurance",
            "instruction_encoding",
            "instruction_norito_sha256",
            "lifecycle",
            "minimum_activation_delay_blocks",
            "pending_protocol_limits_tightening",
            "proposed_at_height",
        ],
        "genesis registration",
    )?;
    expect_string_v1(
        registration,
        "lifecycle",
        "Proposed",
        "genesis registration",
    )?;
    expect_u64_v1(
        registration,
        "proposed_at_height",
        1,
        "genesis registration",
    )?;
    expect_u64_v1(
        registration,
        "activate_at_height",
        301,
        "genesis registration",
    )?;
    expect_u64_v1(
        registration,
        "minimum_activation_delay_blocks",
        300,
        "genesis registration",
    )?;
    expect_string_v1(
        registration,
        "assurance",
        "experimental",
        "genesis registration",
    )?;
    expect_string_v1(
        registration,
        "instruction_encoding",
        "norito-instruction-box-base64",
        "genesis registration",
    )?;
    if registration
        .get("pending_protocol_limits_tightening")
        .and_then(JsonValue::as_bool)
        != Some(false)
        || registration
            .get("instruction_norito_sha256")
            .and_then(JsonValue::as_array)
            .is_none_or(|values| !values.is_empty())
    {
        bail!("privacy plan template is not an empty disabled genesis-registration staging plan");
    }

    validate_catalog_inventory_v1(object_field_v1(root, "privacy_catalog", "privacy plan")?)?;
    let bootle = object_field_v1(root, "bootle_lantern_issuer", "privacy plan")?;
    expect_exact_keys_v1(
        bootle,
        &[
            "authorization_lifetime_blocks",
            "designated_validator",
            "edge_routing",
            "governed_issuer_policy",
            "issuer_id_hex",
            "max_inflight",
            "policy_id_hex",
            "protocol_label",
            "provider_operations",
            "public_export_sha256",
            "runtime_provider",
            "secret_material_permitted",
            "state_authority",
        ],
        "Bootle/Lantern plan",
    )?;
    expect_string_v1(
        bootle,
        "protocol_label",
        PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1.canonical_label(),
        "Bootle/Lantern plan",
    )?;
    if !bootle
        .get("public_export_sha256")
        .is_some_and(JsonValue::is_null)
    {
        bail!("Bootle/Lantern broker public-export staging digest must be null");
    }
    expect_string_v1(
        bootle,
        "designated_validator",
        "taira-validator-1",
        "Bootle/Lantern plan",
    )?;
    expect_string_v1(
        bootle,
        "edge_routing",
        "designated-validator-only",
        "Bootle/Lantern plan",
    )?;
    expect_string_v1(
        bootle,
        "issuer_id_hex",
        &hex::encode(sha256(ISSUER_ID_DOMAIN_V1)),
        "Bootle/Lantern plan",
    )?;
    expect_string_v1(
        bootle,
        "policy_id_hex",
        &hex::encode(sha256(POLICY_ID_DOMAIN_V1)),
        "Bootle/Lantern plan",
    )?;
    expect_u64_v1(
        bootle,
        "authorization_lifetime_blocks",
        AUTHORIZATION_LIFETIME_BLOCKS_V1,
        "Bootle/Lantern plan",
    )?;
    expect_u64_v1(
        bootle,
        "max_inflight",
        MAX_INFLIGHT_V1,
        "Bootle/Lantern plan",
    )?;
    expect_string_v1(
        bootle,
        "state_authority",
        "torii-local-one-shot-store",
        "Bootle/Lantern plan",
    )?;
    if bootle
        .get("secret_material_permitted")
        .and_then(JsonValue::as_bool)
        != Some(false)
    {
        bail!("Bootle/Lantern release plan must forbid secret material");
    }
    let operations = string_array_field_v1(bootle, "provider_operations", "Bootle/Lantern plan")?;
    if operations
        != [
            "bearer-principal-authentication-v1".to_owned(),
            "falcon512-native-crypto-v1".to_owned(),
        ]
    {
        bail!("Bootle/Lantern provider operation inventory differs from first release");
    }
    let provider = object_field_v1(bootle, "runtime_provider", "Bootle/Lantern plan")?;
    expect_exact_keys_v1(
        provider,
        &[
            "handle",
            "qualification_policy_digest_hex",
            "revision",
            "slot_wire_id",
            "transport",
        ],
        "Bootle/Lantern provider plan",
    )?;
    expect_string_v1(
        provider,
        "transport",
        "stock-local-runtime-provider-broker-v1",
        "Bootle/Lantern provider plan",
    )?;
    expect_u64_v1(provider, "slot_wire_id", 54, "Bootle/Lantern provider plan")?;
    expect_string_v1(
        provider,
        "handle",
        PROVIDER_HANDLE_V1,
        "Bootle/Lantern provider plan",
    )?;
    expect_u64_v1(
        provider,
        "revision",
        PROVIDER_REVISION_V1,
        "Bootle/Lantern provider plan",
    )?;
    if !provider
        .get("qualification_policy_digest_hex")
        .is_some_and(JsonValue::is_null)
    {
        bail!("Bootle/Lantern provider staging digest must be null");
    }
    let policy = object_field_v1(bootle, "governed_issuer_policy", "Bootle/Lantern plan")?;
    expect_exact_keys_v1(
        policy,
        &[
            "instruction_norito_sha256",
            "issuer_parameter_digest_hex",
            "issuer_parameter_id_hex",
            "record_digest_hex",
        ],
        "Bootle/Lantern issuer policy plan",
    )?;
    if policy.values().any(|value| !value.is_null()) {
        bail!("Bootle/Lantern issuer-policy staging fields must all be null");
    }
    Ok(())
}

fn validate_catalog_inventory_v1(catalog: &JsonMap) -> color_eyre::Result<()> {
    expect_exact_keys_v1(
        catalog,
        &[
            "matrix_file_sha256",
            "matrix_version",
            "protocols",
            "registry_sha256",
            "retired_labels",
        ],
        "privacy catalog",
    )?;
    expect_u64_v1(catalog, "matrix_version", 1, "privacy catalog")?;
    let exact_matrix = privacy_exact12_matrix_bytes_v1()
        .map_err(|source| eyre!("failed to derive the native exact-12 matrix: {source}"))?;
    expect_string_v1(
        catalog,
        "matrix_file_sha256",
        &hex::encode(sha256(&exact_matrix)),
        "privacy catalog",
    )?;
    let mut registry_preimage = Vec::new();
    for protocol in PrivacyProtocolIdV1::ALL {
        registry_preimage.extend_from_slice(protocol.canonical_label().as_bytes());
        registry_preimage.push(b'\n');
    }
    expect_string_v1(
        catalog,
        "registry_sha256",
        &hex::encode(sha256(&registry_preimage)),
        "privacy catalog",
    )?;
    let protocols = catalog
        .get("protocols")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| eyre!("privacy catalog protocols must be an array"))?;
    if protocols.len() != PrivacyProtocolIdV1::COUNT {
        bail!("privacy catalog must contain exactly twelve protocols");
    }
    for (index, (value, protocol)) in protocols.iter().zip(PrivacyProtocolIdV1::ALL).enumerate() {
        let row = object_v1(value, "privacy catalog row")?;
        expect_exact_keys_v1(
            row,
            &["index", "label", "statement_type"],
            "privacy catalog row",
        )?;
        expect_u64_v1(row, "index", index as u64, "privacy catalog row")?;
        expect_string_v1(
            row,
            "label",
            protocol.canonical_label(),
            "privacy catalog row",
        )?;
        expect_string_v1(
            row,
            "statement_type",
            protocol.canonical_typed_variant_label(),
            "privacy catalog row",
        )?;
    }
    let retired = string_array_field_v1(catalog, "retired_labels", "privacy catalog")?;
    let expected = PRIVACY_RETIRED_PROTOCOL_LABELS_V1
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if retired != expected {
        bail!("privacy catalog retirement inventory differs from exact first release");
    }
    Ok(())
}

fn render_release_config_v1(
    bytes: &[u8],
    broker: &BrokerPublicMaterialV1,
) -> color_eyre::Result<Vec<u8>> {
    let text = std::str::from_utf8(bytes).wrap_err("Taira config template is not UTF-8")?;
    let mut config: toml::Value =
        toml::from_str(text).wrap_err("Taira config template is invalid TOML")?;
    validate_secret_free_config_template_v1(&config)?;
    let root = config
        .as_table_mut()
        .ok_or_else(|| eyre!("Taira config root must be a table"))?;
    if root.get("chain").and_then(toml::Value::as_str) != Some(CHAIN_ID_V1)
        || root
            .get("chain_discriminant")
            .and_then(toml::Value::as_integer)
            != Some(i64::try_from(CHAIN_DISCRIMINANT_V1).expect("fixed discriminant fits i64"))
    {
        bail!("Taira config template targets the wrong chain");
    }
    let torii = root
        .get_mut("torii")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| eyre!("Taira config template has no torii table"))?;
    let issuer = torii
        .get_mut("privacy_bootle_lantern_issuer")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| eyre!("Taira config template has no Bootle/Lantern issuer table"))?;
    let actual_keys = issuer.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected_keys = [
        "authorization_lifetime_blocks",
        "enabled",
        "max_inflight",
        "max_records",
        "max_total_bytes",
        "state_dir",
        "terminal_retention_blocks",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if actual_keys != expected_keys
        || issuer.get("enabled").and_then(toml::Value::as_bool) != Some(false)
        || issuer.get("state_dir").and_then(toml::Value::as_str)
            != Some("/var/lib/iroha/taira-validator-1/privacy/bootle-lantern/issuer")
        || issuer.get("max_inflight").and_then(toml::Value::as_integer)
            != Some(i64::try_from(MAX_INFLIGHT_V1).expect("fixed inflight bound fits i64"))
        || issuer
            .get("authorization_lifetime_blocks")
            .and_then(toml::Value::as_integer)
            != Some(300)
        || issuer.get("max_records").and_then(toml::Value::as_integer) != Some(4096)
        || issuer
            .get("max_total_bytes")
            .and_then(toml::Value::as_integer)
            != Some(13_557_760)
        || issuer
            .get("terminal_retention_blocks")
            .and_then(toml::Value::as_integer)
            != Some(4096)
    {
        bail!("Taira Bootle/Lantern config template is not the exact disabled peer-1 contract");
    }
    expect_canonical_template_bytes_v1(bytes, CANONICAL_CONFIG_TEMPLATE_V1, "Taira config")?;
    issuer.insert("enabled".to_owned(), toml::Value::Boolean(true));
    issuer.insert(
        "issuer_id_hex".to_owned(),
        toml::Value::String(hex::encode(sha256(ISSUER_ID_DOMAIN_V1))),
    );
    issuer.insert(
        "policy_id_hex".to_owned(),
        toml::Value::String(hex::encode(sha256(POLICY_ID_DOMAIN_V1))),
    );
    issuer.insert(
        "runtime_provider_registry_handle".to_owned(),
        toml::Value::String(PROVIDER_HANDLE_V1.to_owned()),
    );
    issuer.insert(
        "runtime_provider_registry_revision".to_owned(),
        toml::Value::Integer(
            i64::try_from(PROVIDER_REVISION_V1).expect("fixed provider revision fits i64"),
        ),
    );
    issuer.insert(
        "runtime_provider_registry_policy_digest_hex".to_owned(),
        toml::Value::String(broker.qualification_policy_digest_hex.clone()),
    );
    let mut rendered =
        toml::to_string_pretty(&config).wrap_err("failed to render release config")?;
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    Ok(rendered.into_bytes())
}

fn validate_secret_free_config_template_v1(config: &toml::Value) -> color_eyre::Result<()> {
    let root = config
        .as_table()
        .ok_or_else(|| eyre!("Taira config root must be a table"))?;
    expect_toml_string_v1(
        root,
        "private_key",
        "REPLACE_WITH_VALIDATOR_PRIVATE_KEY",
        "Taira validator private key",
    )?;
    let torii = toml_table_field_v1(root, "torii", "Taira config")?;
    expect_toml_string_v1(
        toml_table_field_v1(torii, "kagemusha_commands", "Taira torii config")?,
        "private_key",
        "REPLACE_WITH_TAIRA_KAGEMUSHA_COMMANDS_PRIVATE_KEY",
        "Taira Kagemusha command private key",
    )?;
    let onboarding = toml_table_field_v1(torii, "account_onboarding", "Taira torii config")?;
    expect_toml_string_v1(
        onboarding,
        "private_key_file",
        "REPLACE_WITH_TAIRA_ONBOARDING_PRIVATE_KEY_FILE",
        "Taira onboarding private-key path",
    )?;
    let credentials = onboarding
        .get("credentials")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| eyre!("Taira onboarding credentials must be an array"))?;
    if credentials.len() != 1 {
        bail!("Taira config template must contain exactly one placeholder onboarding credential");
    }
    let credential = credentials[0]
        .as_table()
        .ok_or_else(|| eyre!("Taira onboarding credential must be a table"))?;
    expect_toml_string_v1(
        credential,
        "token_hash",
        "REPLACE_WITH_TAIRA_ONBOARDING_TOKEN_HASH",
        "Taira onboarding token digest",
    )?;
    expect_toml_string_v1(
        toml_table_field_v1(torii, "faucet", "Taira torii config")?,
        "private_key_file",
        "REPLACE_WITH_TAIRA_FAUCET_PRIVATE_KEY_FILE",
        "Taira faucet private-key path",
    )?;
    expect_toml_string_v1(
        toml_table_field_v1(root, "streaming", "Taira config")?,
        "identity_private_key",
        "REPLACE_WITH_STREAMING_IDENTITY_PRIVATE_KEY",
        "Taira streaming private key",
    )?;
    Ok(())
}

fn toml_table_field_v1<'a>(
    fields: &'a toml::Table,
    field: &str,
    label: &str,
) -> color_eyre::Result<&'a toml::Table> {
    fields
        .get(field)
        .and_then(toml::Value::as_table)
        .ok_or_else(|| eyre!("{label} `{field}` must be a table"))
}

fn expect_toml_string_v1(
    fields: &toml::Table,
    field: &str,
    expected: &str,
    label: &str,
) -> color_eyre::Result<()> {
    if fields.get(field).and_then(toml::Value::as_str) != Some(expected) {
        bail!("{label} must remain the exact secret-free staging placeholder");
    }
    Ok(())
}

fn render_release_genesis_v1(
    bytes: &[u8],
    activation_base64: &[String],
    activation_boxes: &[InstructionBox],
    broker: &BrokerPublicMaterialV1,
) -> color_eyre::Result<Vec<u8>> {
    if activation_base64.len() != PrivacyProtocolIdV1::COUNT
        || activation_boxes.len() != PrivacyProtocolIdV1::COUNT
    {
        bail!("Taira release genesis requires the complete exact-12 activation inventory");
    }
    iroha_genesis::init_instruction_registry();
    let decoded_template: RawGenesisTransaction = norito::json::from_slice(bytes)
        .wrap_err("Taira genesis template cannot be decoded natively")?;
    if decoded_template.chain_id().as_str() != CHAIN_ID_V1
        || u64::from(decoded_template.chain_discriminant()) != CHAIN_DISCRIMINANT_V1
    {
        bail!("Taira genesis template targets the wrong chain");
    }
    for instruction in decoded_template
        .transactions()
        .iter()
        .flat_map(|transaction| transaction.instructions())
    {
        if instruction
            .as_any()
            .downcast_ref::<RegisterPrivacyProtocolActivationV1>()
            .is_some()
            || instruction
                .as_any()
                .downcast_ref::<RegisterPrivacyBootleLanternIssuerPolicyV1>()
                .is_some()
        {
            bail!(
                "Taira genesis staging template already contains a privacy bootstrap instruction"
            );
        }
    }

    let mut genesis: JsonValue =
        norito::json::from_slice(bytes).wrap_err("Taira genesis template is not strict JSON")?;
    let root = object_mut_v1(&mut genesis, "Taira genesis")?;
    expect_string_v1(root, "chain", CHAIN_ID_V1, "Taira genesis")?;
    expect_u64_v1(
        root,
        "chain_discriminant",
        CHAIN_DISCRIMINANT_V1,
        "Taira genesis",
    )?;
    let transactions = root
        .get_mut("transactions")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| eyre!("Taira genesis has no transaction array"))?;
    if transactions.is_empty() {
        bail!("Taira genesis has no transactions");
    }

    let mut authority_registration_count = 0_usize;
    let mut governance_grant_count = 0_usize;
    let mut authority_registration_index = None;
    let mut governance_grant_index = None;
    let mut global_index = 0_usize;
    for transaction in transactions.iter() {
        let instructions = transaction
            .get("instructions")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| eyre!("Taira genesis transaction has no instruction array"))?;
        for instruction in instructions {
            if instruction.is_string() {
                bail!("Taira genesis staging template already contains an encoded instruction");
            }
            if registered_account_id_v1(instruction) == Some(GENESIS_AUTHORITY_V1) {
                authority_registration_count += 1;
                authority_registration_index = Some(global_index);
            }
            if let Some(destination) = governance_grant_destination_v1(instruction)? {
                if destination != GENESIS_AUTHORITY_V1 {
                    bail!("Taira CanEnactGovernance genesis grant targets the wrong authority");
                }
                governance_grant_count += 1;
                governance_grant_index = Some(global_index);
            }
            global_index += 1;
        }
    }
    if authority_registration_count != 1
        || governance_grant_count != 1
        || authority_registration_index >= governance_grant_index
    {
        bail!(
            "Taira genesis does not contain the exact ordered privacy governance authority and grant"
        );
    }
    expect_canonical_template_bytes_v1(bytes, CANONICAL_GENESIS_TEMPLATE_V1, "Taira genesis")?;

    let final_transaction = transactions
        .last_mut()
        .and_then(JsonValue::as_object_mut)
        .ok_or_else(|| eyre!("Taira genesis final transaction is not an object"))?;
    if final_transaction
        .get("parameters")
        .is_some_and(|value| !value.is_null())
        || final_transaction
            .get("ivm_triggers")
            .and_then(JsonValue::as_array)
            .is_some_and(|values| !values.is_empty())
        || final_transaction
            .get("topology")
            .and_then(JsonValue::as_array)
            .is_some_and(|values| !values.is_empty())
    {
        bail!("Taira genesis final transaction is not instruction-only");
    }
    let instructions = final_transaction
        .get_mut("instructions")
        .and_then(JsonValue::as_array_mut)
        .ok_or_else(|| eyre!("Taira genesis final transaction has no instruction array"))?;
    instructions.extend(activation_base64.iter().cloned().map(JsonValue::String));
    instructions.push(JsonValue::String(broker.instruction_base64.clone()));

    let rendered = json_pretty_bytes_v1(&genesis, "Taira privacy release genesis")?;
    iroha_genesis::init_instruction_registry();
    let decoded: RawGenesisTransaction = norito::json::from_slice(&rendered)
        .wrap_err("rendered Taira release genesis cannot be decoded natively")?;
    if decoded.chain_id().as_str() != CHAIN_ID_V1
        || u64::from(decoded.chain_discriminant()) != CHAIN_DISCRIMINANT_V1
    {
        bail!("rendered Taira release genesis changed chain identity");
    }
    let decoded_instructions = decoded
        .transactions()
        .iter()
        .flat_map(|transaction| transaction.instructions())
        .collect::<Vec<_>>();
    let activation_count = decoded_instructions
        .iter()
        .filter(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<RegisterPrivacyProtocolActivationV1>()
                .is_some()
        })
        .count();
    let issuer_policy_count = decoded_instructions
        .iter()
        .filter(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<RegisterPrivacyBootleLanternIssuerPolicyV1>()
                .is_some()
        })
        .count();
    if activation_count != PrivacyProtocolIdV1::COUNT || issuer_policy_count != 1 {
        bail!(
            "rendered Taira release genesis must contain exactly twelve activations and one issuer-policy registration"
        );
    }
    let expected_count = activation_boxes.len() + 1;
    if decoded_instructions.len() < expected_count {
        bail!("rendered Taira release genesis lost privacy instructions");
    }
    let privacy_tail = &decoded_instructions[decoded_instructions.len() - expected_count..];
    for (index, (actual, expected)) in privacy_tail
        .iter()
        .take(activation_boxes.len())
        .zip(activation_boxes)
        .enumerate()
    {
        if *actual != expected {
            bail!("rendered Taira activation instruction {index} changed during genesis decoding");
        }
    }
    if privacy_tail.last().copied() != Some(&broker.instruction) {
        bail!("rendered Taira issuer-policy instruction changed during genesis decoding");
    }
    Ok(rendered)
}

fn registered_account_id_v1(instruction: &JsonValue) -> Option<&str> {
    instruction
        .get("Register")?
        .get("Account")?
        .get("id")?
        .as_str()
}

fn governance_grant_destination_v1(instruction: &JsonValue) -> color_eyre::Result<Option<&str>> {
    let Some(permission) = instruction
        .get("Grant")
        .and_then(|grant| grant.get("Permission"))
    else {
        return Ok(None);
    };
    let Some(object) = permission.get("object").and_then(JsonValue::as_object) else {
        return Ok(None);
    };
    if object.get("name").and_then(JsonValue::as_str) != Some("CanEnactGovernance") {
        return Ok(None);
    }
    if object.len() != 1 {
        bail!("Taira CanEnactGovernance genesis grant must be unscoped");
    }
    let destination = permission
        .get("destination")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| eyre!("Taira CanEnactGovernance genesis grant has no destination"))?;
    Ok(Some(destination))
}

fn json_pretty_bytes_v1(value: &JsonValue, label: &str) -> color_eyre::Result<Vec<u8>> {
    let mut rendered = norito::json::to_json_pretty(value)
        .wrap_err_with(|| format!("failed to render {label}"))?;
    rendered.push('\n');
    Ok(rendered.into_bytes())
}

fn object_v1<'a>(value: &'a JsonValue, label: &str) -> color_eyre::Result<&'a JsonMap> {
    value
        .as_object()
        .ok_or_else(|| eyre!("{label} must be a JSON object"))
}

fn object_mut_v1<'a>(value: &'a mut JsonValue, label: &str) -> color_eyre::Result<&'a mut JsonMap> {
    value
        .as_object_mut()
        .ok_or_else(|| eyre!("{label} must be a JSON object"))
}

fn object_field_v1<'a>(
    fields: &'a JsonMap,
    field: &str,
    label: &str,
) -> color_eyre::Result<&'a JsonMap> {
    fields
        .get(field)
        .and_then(JsonValue::as_object)
        .ok_or_else(|| eyre!("{label} `{field}` must be an object"))
}

fn object_field_mut_v1<'a>(
    fields: &'a mut JsonMap,
    field: &str,
    label: &str,
) -> color_eyre::Result<&'a mut JsonMap> {
    fields
        .get_mut(field)
        .and_then(JsonValue::as_object_mut)
        .ok_or_else(|| eyre!("{label} `{field}` must be an object"))
}

fn string_field_v1<'a>(
    fields: &'a JsonMap,
    field: &str,
    label: &str,
) -> color_eyre::Result<&'a str> {
    fields
        .get(field)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| eyre!("{label} `{field}` must be a string"))
}

fn string_array_field_v1(
    fields: &JsonMap,
    field: &str,
    label: &str,
) -> color_eyre::Result<Vec<String>> {
    fields
        .get(field)
        .and_then(JsonValue::as_array)
        .ok_or_else(|| eyre!("{label} `{field}` must be an array"))?
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| eyre!("{label} `{field}` entry {index} must be a string"))
        })
        .collect()
}

fn expect_string_v1(
    fields: &JsonMap,
    field: &str,
    expected: &str,
    label: &str,
) -> color_eyre::Result<()> {
    if string_field_v1(fields, field, label)? != expected {
        bail!("{label} `{field}` differs from the exact first-release value");
    }
    Ok(())
}

fn expect_u64_v1(
    fields: &JsonMap,
    field: &str,
    expected: u64,
    label: &str,
) -> color_eyre::Result<()> {
    if fields.get(field).and_then(JsonValue::as_u64) != Some(expected) {
        bail!("{label} `{field}` differs from the exact first-release value");
    }
    Ok(())
}

fn expect_exact_keys_v1(
    fields: &JsonMap,
    expected: &[&str],
    label: &str,
) -> color_eyre::Result<()> {
    let actual = fields.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        bail!("{label} has missing or unknown fields");
    }
    Ok(())
}

fn fixed_nonzero_sha256_v1(value: &str, label: &str) -> color_eyre::Result<()> {
    if value.len() != 64
        || value == "0".repeat(64)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("{label} must be one nonzero lowercase SHA-256 digest");
    }
    Ok(())
}

fn decode_sha256_v1(value: &str, label: &str) -> color_eyre::Result<[u8; 32]> {
    fixed_nonzero_sha256_v1(value, label)?;
    let bytes = hex::decode(value).wrap_err_with(|| format!("failed to decode {label}"))?;
    bytes
        .try_into()
        .map_err(|_| eyre!("{label} must decode to exactly 32 bytes"))
}

fn expect_canonical_template_bytes_v1(
    actual: &[u8],
    canonical: &[u8],
    label: &str,
) -> color_eyre::Result<()> {
    if actual != canonical {
        bail!("{label} template differs byte-for-byte from the compiled first-release template");
    }
    Ok(())
}

fn write_new_artifact_set_v1<const N: usize>(
    artifacts: [(&Path, &[u8], &str); N],
) -> color_eyre::Result<()> {
    let unique = artifacts
        .iter()
        .map(|(path, _, _)| resolved_new_output_path_v1(path))
        .collect::<color_eyre::Result<BTreeSet<_>>>()?;
    if unique.len() != N {
        bail!("Taira release output paths must all differ");
    }
    let mut opened = Vec::with_capacity(N);
    for (path, _, description) in &artifacts {
        match create_new_file(path, description) {
            Ok(file) => opened.push(file),
            Err(error) => {
                for ((created_path, _, _), created_file) in artifacts.iter().zip(opened.iter()) {
                    remove_created_file_if_unchanged_v1(created_path, created_file);
                }
                drop(opened);
                return Err(error);
            }
        }
    }
    for index in 0..N {
        let (_, bytes, description) = artifacts[index];
        let result = {
            let file = &mut opened[index];
            file.write_all(bytes)
                .and_then(|()| file.sync_all())
                .wrap_err_with(|| format!("failed to write and sync {description}"))
        };
        if let Err(error) = result {
            for ((path, _, _), opened_file) in artifacts.iter().zip(opened.iter()) {
                remove_created_file_if_unchanged_v1(path, opened_file);
            }
            drop(opened);
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::OnceLock};

    use iroha_core::{
        privacy_engines::bootle_lantern::issuer::{
            BootleLanternIssuerKeyPairV1, BootleLanternIssuerPolicyMetadataV1,
        },
        privacy_profiles::{
            compiled_privacy_profile_v1, zk_x509_release_candidate_profile_material_v1,
        },
    };
    use iroha_data_model::privacy::{
        BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1, BootleLanternAllowedAttributeValuesV1,
        PrivacyIssuerIdV1, PrivacyParameterIdV1, PrivacyPolicyIdV1,
    };

    use super::*;
    use crate::privacy_bootstrap::{
        TairaPrivacyBootstrapArtifactsV1, build_artifacts_from_profiles_v1,
    };

    const PLAN_TEMPLATE_V1: &[u8] =
        include_bytes!("../../../../configs/soranexus/taira/privacy_bootstrap_plan.json");
    const CONFIG_TEMPLATE_V1: &[u8] =
        include_bytes!("../../../../configs/soranexus/taira/config.toml");
    const GENESIS_TEMPLATE_V1: &[u8] =
        include_bytes!("../../../../configs/soranexus/taira/genesis.json");

    fn activation_fixture_v1() -> TairaPrivacyBootstrapArtifactsV1 {
        static FIXTURE: OnceLock<TairaPrivacyBootstrapArtifactsV1> = OnceLock::new();
        FIXTURE
            .get_or_init(|| {
                let profiles = PrivacyProtocolIdV1::ALL
                    .into_iter()
                    .map(|protocol_id| {
                        compiled_privacy_profile_v1(protocol_id).or_else(|error| {
                            if protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 {
                                zk_x509_release_candidate_profile_material_v1()
                            } else {
                                Err(error)
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .expect("derive exact-12 release-composer fixtures");
                build_artifacts_from_profiles_v1(&profiles)
                    .expect("build exact-12 release-composer artifacts")
            })
            .clone()
    }

    fn policy_registration_fixture_v1() -> RegisterPrivacyBootleLanternIssuerPolicyV1 {
        let issuer = BootleLanternIssuerKeyPairV1::generate_from_secret_seed_v1(
            PrivacyParameterIdV1::new(sha256(b"release-composer-parameter-id")),
            &[0x5a; 32],
        )
        .expect("derive deterministic native issuer fixture");
        let policy = issuer
            .active_policy_v1(BootleLanternIssuerPolicyMetadataV1 {
                issuer_id: PrivacyIssuerIdV1::new(sha256(ISSUER_ID_DOMAIN_V1)),
                policy_id: PrivacyPolicyIdV1::new(sha256(POLICY_ID_DOMAIN_V1)),
                epoch: 1,
                required_disclosure_bitmap: 0,
                allowed_values: (0..BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1)
                    .map(|_| BootleLanternAllowedAttributeValuesV1 { values: Vec::new() })
                    .collect(),
            })
            .expect("derive valid governed issuer-policy fixture");
        RegisterPrivacyBootleLanternIssuerPolicyV1::new(policy)
    }

    fn broker_export_fixture_v1() -> Vec<u8> {
        let registration = policy_registration_fixture_v1();
        let instruction = InstructionBox::from(registration.clone());
        let instruction_bytes =
            norito::to_bytes(&instruction).expect("encode boxed policy fixture");
        let stable_principal_digest = sha256(b"iroha.taira.release-composer.stable-principal.v1");
        let qualification = derive_taira_bootle_lantern_broker_qualification_digest_v1(
            &TairaBootleLanternBrokerQualificationInputsV1 {
                chain_id: CHAIN_ID_V1,
                runtime_provider_handle: PROVIDER_HANDLE_V1,
                runtime_provider_revision: PROVIDER_REVISION_V1,
                issuer_id: registration.policy.issuer_id,
                policy_id: registration.policy.policy_id,
                authorization_lifetime_blocks: AUTHORIZATION_LIFETIME_BLOCKS_V1,
                policy: &registration.policy,
                stable_principal_digest,
            },
        )
        .expect("derive provider qualification fixture");
        let mut fields = JsonMap::new();
        fields.insert(
            "schema".to_owned(),
            JsonValue::String(BROKER_EXPORT_SCHEMA_V1.to_owned()),
        );
        fields.insert(
            "chain_id".to_owned(),
            JsonValue::String(CHAIN_ID_V1.to_owned()),
        );
        fields.insert(
            "runtime_provider_handle".to_owned(),
            JsonValue::String(PROVIDER_HANDLE_V1.to_owned()),
        );
        fields.insert(
            "runtime_provider_revision".to_owned(),
            JsonValue::from(PROVIDER_REVISION_V1),
        );
        fields.insert(
            "runtime_provider_policy_digest_hex".to_owned(),
            JsonValue::String(hex::encode(qualification)),
        );
        fields.insert(
            "issuer_id_hex".to_owned(),
            JsonValue::String(hex::encode(sha256(ISSUER_ID_DOMAIN_V1))),
        );
        fields.insert(
            "policy_id_hex".to_owned(),
            JsonValue::String(hex::encode(sha256(POLICY_ID_DOMAIN_V1))),
        );
        fields.insert(
            "authorization_lifetime_blocks".to_owned(),
            JsonValue::from(AUTHORIZATION_LIFETIME_BLOCKS_V1),
        );
        fields.insert(
            "issuer_parameter_id_hex".to_owned(),
            JsonValue::String(hex::encode(
                registration.policy.issuer_parameter_id.as_bytes(),
            )),
        );
        fields.insert(
            "issuer_parameter_digest_hex".to_owned(),
            JsonValue::String(hex::encode(
                registration.policy.issuer_parameter_digest.as_bytes(),
            )),
        );
        fields.insert(
            "policy_record_digest_hex".to_owned(),
            JsonValue::String(hex::encode(registration.policy.record_digest.as_bytes())),
        );
        fields.insert(
            "stable_principal_digest_hex".to_owned(),
            JsonValue::String(hex::encode(stable_principal_digest)),
        );
        fields.insert(
            "issuer_profile_digest_hex".to_owned(),
            JsonValue::String(hex::encode(
                taira_bootle_lantern_issuer_profile_contract_digest_v1(),
            )),
        );
        fields.insert(
            "broker_contract_digest_hex".to_owned(),
            JsonValue::String(hex::encode(taira_bootle_lantern_broker_contract_digest_v1())),
        );
        fields.insert(
            "registration_instruction_norito_hex".to_owned(),
            JsonValue::String(hex::encode(&instruction_bytes)),
        );
        fields.insert(
            "registration_instruction_norito_sha256".to_owned(),
            JsonValue::String(hex::encode(sha256(&instruction_bytes))),
        );
        fields.insert(
            "registration_instruction".to_owned(),
            norito::json::to_value(&registration).expect("project policy fixture"),
        );
        format!(
            "{}\n",
            norito::json::to_json(&JsonValue::Object(fields))
                .expect("encode broker export fixture")
        )
        .into_bytes()
    }

    fn mutate_export_v1(source: &[u8], mutate: impl FnOnce(&mut JsonMap)) -> Vec<u8> {
        let mut value: JsonValue = norito::json::from_slice(source).expect("parse export fixture");
        mutate(value.as_object_mut().expect("export object"));
        format!(
            "{}\n",
            norito::json::to_json(&value).expect("encode mutated export")
        )
        .into_bytes()
    }

    #[test]
    fn complete_release_composition_is_native_deterministic_and_secret_free() {
        let activations = activation_fixture_v1();
        let export = broker_export_fixture_v1();
        if compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkX509StarkP256V0).is_err() {
            let error = compose_release_artifacts_v1(
                &activations.instructions_json,
                &activations.report_json,
                &export,
                PLAN_TEMPLATE_V1,
                CONFIG_TEMPLATE_V1,
                GENESIS_TEMPLATE_V1,
            )
            .expect_err("closed ZK-X509 evidence gate must prevent release composition");
            assert!(
                error.to_string().contains("not governance-available"),
                "unexpected closed-gate error: {error}"
            );
            return;
        }
        let first = compose_release_artifacts_v1(
            &activations.instructions_json,
            &activations.report_json,
            &export,
            PLAN_TEMPLATE_V1,
            CONFIG_TEMPLATE_V1,
            GENESIS_TEMPLATE_V1,
        )
        .expect("compose complete release");
        let second = compose_release_artifacts_v1(
            &activations.instructions_json,
            &activations.report_json,
            &export,
            PLAN_TEMPLATE_V1,
            CONFIG_TEMPLATE_V1,
            GENESIS_TEMPLATE_V1,
        )
        .expect("recompose complete release");
        assert_eq!(first.plan, second.plan);
        assert_eq!(first.config, second.config);
        assert_eq!(first.genesis, second.genesis);
        assert_eq!(first.broker_public, second.broker_public);
        assert_eq!(first.broker_public, export);
        let plan: JsonValue = norito::json::from_slice(&first.plan).expect("parse release plan");
        assert_eq!(
            plan.pointer("/genesis_registration/instruction_norito_sha256")
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(PrivacyProtocolIdV1::COUNT)
        );
        let expected_export_sha256 = hex::encode(sha256(&export));
        assert_eq!(
            plan.pointer("/bootle_lantern_issuer/public_export_sha256")
                .and_then(JsonValue::as_str),
            Some(expected_export_sha256.as_str())
        );
        let config: toml::Value =
            toml::from_str(std::str::from_utf8(&first.config).expect("release config UTF-8"))
                .expect("parse release config");
        assert_eq!(
            config
                .get("torii")
                .and_then(|value| value.get("privacy_bootle_lantern_issuer"))
                .and_then(|value| value.get("enabled"))
                .and_then(toml::Value::as_bool),
            Some(true)
        );
        assert!(!String::from_utf8_lossy(&first.plan).contains("issuer_seed"));
        assert!(!String::from_utf8_lossy(&first.config).contains("bearer_token"));
        assert!(!String::from_utf8_lossy(&first.genesis).contains("principal_seed"));
    }

    #[test]
    fn bare_trailing_digest_and_secret_field_broker_substitutions_are_rejected() {
        let canonical = broker_export_fixture_v1();
        let registration = policy_registration_fixture_v1();
        let bare = norito::to_bytes(&registration).expect("encode bare policy registration");
        let bare_export = mutate_export_v1(&canonical, |fields| {
            fields.insert(
                "registration_instruction_norito_hex".to_owned(),
                JsonValue::String(hex::encode(&bare)),
            );
            fields.insert(
                "registration_instruction_norito_sha256".to_owned(),
                JsonValue::String(hex::encode(sha256(&bare))),
            );
        });
        assert!(parse_broker_public_export_v1(&bare_export).is_err());

        let trailing = mutate_export_v1(&canonical, |fields| {
            let mut bytes = hex::decode(
                fields
                    .get("registration_instruction_norito_hex")
                    .and_then(JsonValue::as_str)
                    .expect("instruction hex"),
            )
            .expect("decode boxed instruction");
            bytes.push(0);
            fields.insert(
                "registration_instruction_norito_hex".to_owned(),
                JsonValue::String(hex::encode(&bytes)),
            );
            fields.insert(
                "registration_instruction_norito_sha256".to_owned(),
                JsonValue::String(hex::encode(sha256(&bytes))),
            );
        });
        assert!(parse_broker_public_export_v1(&trailing).is_err());

        let digest = mutate_export_v1(&canonical, |fields| {
            fields.insert(
                "registration_instruction_norito_sha256".to_owned(),
                JsonValue::String("33".repeat(32)),
            );
        });
        assert!(parse_broker_public_export_v1(&digest).is_err());

        let secret = mutate_export_v1(&canonical, |fields| {
            fields.insert(
                "issuer_seed".to_owned(),
                JsonValue::String("forbidden".to_owned()),
            );
        });
        assert!(parse_broker_public_export_v1(&secret).is_err());
    }

    #[test]
    fn provider_qualification_profile_contract_and_principal_drift_are_rejected() {
        let canonical = broker_export_fixture_v1();
        for field in [
            "runtime_provider_policy_digest_hex",
            "issuer_profile_digest_hex",
            "broker_contract_digest_hex",
            "stable_principal_digest_hex",
        ] {
            let drifted = mutate_export_v1(&canonical, |fields| {
                fields.insert(field.to_owned(), JsonValue::String("33".repeat(32)));
            });
            assert!(
                parse_broker_public_export_v1(&drifted).is_err(),
                "broker public field {field} must be derived rather than shape-checked"
            );
        }
    }

    #[test]
    fn coordinated_governance_authority_substitution_is_rejected_natively() {
        let mut plan: JsonValue =
            norito::json::from_slice(PLAN_TEMPLATE_V1).expect("parse staging plan");
        plan.as_object_mut().expect("plan object").insert(
            "genesis_authority".to_owned(),
            JsonValue::String("attacker@wonderland".to_owned()),
        );
        assert!(validate_staging_plan_v1(&plan).is_err());
    }

    #[test]
    fn catalog_digest_type_and_unknown_field_substitutions_are_rejected_natively() {
        for mutation in 0_u8..3 {
            let mut plan: JsonValue =
                norito::json::from_slice(PLAN_TEMPLATE_V1).expect("parse staging plan");
            let catalog = plan
                .pointer_mut("/privacy_catalog")
                .and_then(JsonValue::as_object_mut)
                .expect("catalog object");
            match mutation {
                0 => {
                    catalog.insert(
                        "matrix_file_sha256".to_owned(),
                        JsonValue::String("11".repeat(32)),
                    );
                }
                1 => {
                    catalog
                        .get_mut("protocols")
                        .and_then(JsonValue::as_array_mut)
                        .expect("protocol rows")[0]
                        .as_object_mut()
                        .expect("protocol row")
                        .insert(
                            "statement_type".to_owned(),
                            JsonValue::String("AttackerStatement".to_owned()),
                        );
                }
                _ => {
                    catalog.insert("future_aliases".to_owned(), JsonValue::Array(Vec::new()));
                }
            }
            assert!(
                validate_staging_plan_v1(&plan).is_err(),
                "catalog mutation {mutation} must fail"
            );
        }
    }

    #[test]
    fn config_template_with_materialized_private_key_is_rejected() {
        let text = std::str::from_utf8(CONFIG_TEMPLATE_V1)
            .expect("config fixture UTF-8")
            .replacen(
                "REPLACE_WITH_VALIDATOR_PRIVATE_KEY",
                "materialized-private-key",
                1,
            );
        let broker = parse_broker_public_export_v1(&broker_export_fixture_v1())
            .expect("parse broker fixture");
        assert!(
            render_release_config_v1(text.as_bytes(), &broker)
                .expect_err("reject materialized private key")
                .to_string()
                .contains("secret-free staging placeholder")
        );
    }

    #[test]
    fn semantically_equivalent_template_byte_drift_is_rejected() {
        let broker = parse_broker_public_export_v1(&broker_export_fixture_v1())
            .expect("parse broker fixture");

        let mut plan = b"\n".to_vec();
        plan.extend_from_slice(PLAN_TEMPLATE_V1);
        assert!(
            render_release_plan_v1(&plan, &[], &broker)
                .expect_err("reject whitespace-drifted plan template")
                .to_string()
                .contains("differs byte-for-byte")
        );

        let mut config = CONFIG_TEMPLATE_V1.to_vec();
        config.extend_from_slice(b"\n# unreviewed but semantically inert\n");
        assert!(
            render_release_config_v1(&config, &broker)
                .expect_err("reject comment-drifted config template")
                .to_string()
                .contains("differs byte-for-byte")
        );

        let activations = activation_fixture_v1();
        let (_, encoded, boxes) =
            activation_material_v1(&activations.report_json).expect("activation material");
        let mut genesis = b"\n".to_vec();
        genesis.extend_from_slice(GENESIS_TEMPLATE_V1);
        assert!(
            render_release_genesis_v1(&genesis, &encoded, &boxes, &broker)
                .expect_err("reject whitespace-drifted genesis template")
                .to_string()
                .contains("differs byte-for-byte")
        );
    }

    #[test]
    fn decoded_privacy_bootstrap_in_genesis_template_is_rejected() {
        let activations = activation_fixture_v1();
        let (_, encoded, boxes) =
            activation_material_v1(&activations.report_json).expect("activation material");
        let broker = parse_broker_public_export_v1(&broker_export_fixture_v1())
            .expect("parse broker fixture");
        let mut genesis: JsonValue =
            norito::json::from_slice(GENESIS_TEMPLATE_V1).expect("parse genesis template");
        let mut one = String::new();
        iroha_genesis::genesis_instructions_json::serialize(&boxes[..1], &mut one);
        let mut decoded: JsonValue =
            norito::json::from_str(&one).expect("parse decoded activation JSON");
        let injected = decoded
            .as_array_mut()
            .expect("activation array")
            .pop()
            .expect("one activation");
        genesis
            .get_mut("transactions")
            .and_then(JsonValue::as_array_mut)
            .and_then(|transactions| transactions.last_mut())
            .and_then(|transaction| transaction.get_mut("instructions"))
            .and_then(JsonValue::as_array_mut)
            .expect("final instructions")
            .push(injected);
        let tampered = json_pretty_bytes_v1(&genesis, "tampered genesis").expect("render tamper");
        assert!(
            render_release_genesis_v1(&tampered, &encoded, &boxes, &broker)
                .expect_err("reject pre-existing decoded privacy instruction")
                .to_string()
                .contains("already contains a privacy bootstrap instruction")
        );
    }

    #[test]
    fn wrong_and_scoped_governance_grants_are_rejected_before_composition() {
        let activations = activation_fixture_v1();
        let (_, encoded, boxes) =
            activation_material_v1(&activations.report_json).expect("activation material");
        let broker = parse_broker_public_export_v1(&broker_export_fixture_v1())
            .expect("parse broker fixture");

        for scoped in [false, true] {
            let mut genesis: JsonValue =
                norito::json::from_slice(GENESIS_TEMPLATE_V1).expect("parse genesis template");
            let transactions = genesis
                .get_mut("transactions")
                .and_then(JsonValue::as_array_mut)
                .expect("transactions");
            let alternative = transactions
                .iter()
                .flat_map(|transaction| {
                    transaction
                        .get("instructions")
                        .and_then(JsonValue::as_array)
                        .into_iter()
                        .flatten()
                })
                .find_map(registered_account_id_v1)
                .filter(|account| *account != GENESIS_AUTHORITY_V1)
                .expect("alternative registered account")
                .to_owned();
            let permission = transactions
                .iter_mut()
                .flat_map(|transaction| {
                    transaction
                        .get_mut("instructions")
                        .and_then(JsonValue::as_array_mut)
                        .into_iter()
                        .flatten()
                })
                .find_map(|instruction| {
                    instruction
                        .get_mut("Grant")?
                        .get_mut("Permission")?
                        .as_object_mut()
                        .filter(|permission| {
                            permission
                                .get("object")
                                .and_then(JsonValue::as_object)
                                .and_then(|object| object.get("name"))
                                .and_then(JsonValue::as_str)
                                == Some("CanEnactGovernance")
                        })
                })
                .expect("governance permission");
            if scoped {
                permission
                    .get_mut("object")
                    .and_then(JsonValue::as_object_mut)
                    .expect("permission object")
                    .insert("payload".to_owned(), norito::json!({"scope": "privacy"}));
            } else {
                permission.insert("destination".to_owned(), JsonValue::String(alternative));
            }
            let tampered =
                json_pretty_bytes_v1(&genesis, "tampered genesis").expect("render tamper");
            let error = render_release_genesis_v1(&tampered, &encoded, &boxes, &broker)
                .expect_err("reject invalid governance grant");
            assert!(
                error.to_string().contains("wrong authority")
                    || error.to_string().contains("must be unscoped")
                    || error.to_string().contains("cannot be decoded natively"),
                "unexpected governance rejection: {error}"
            );
        }
    }

    #[test]
    fn release_output_set_never_overwrites_and_removes_partial_creations() {
        let directory = tempfile::tempdir().expect("temporary release directory");
        let plan = directory.path().join("plan.json");
        let config = directory.path().join("config.toml");
        let genesis = directory.path().join("genesis.json");
        fs::write(&config, b"occupied").expect("occupy second path");
        assert!(
            write_new_artifact_set_v1([
                (plan.as_path(), b"plan".as_slice(), "plan"),
                (config.as_path(), b"config".as_slice(), "config"),
                (genesis.as_path(), b"genesis".as_slice(), "genesis"),
            ])
            .is_err()
        );
        assert!(!plan.exists());
        assert_eq!(
            fs::read(&config).expect("read occupied config"),
            b"occupied"
        );
        assert!(!genesis.exists());
    }
}
