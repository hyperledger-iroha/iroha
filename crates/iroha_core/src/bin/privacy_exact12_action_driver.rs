//! One-shot, non-networked Exact12 action-construction driver.
//!
//! This boundary is designed for a sealed qualification controller to own
//! validator lifecycle, submission, direct peer queries, replay attempts, and
//! outcome validation. This binary deliberately has no endpoint or credential
//! input. Each admitted v1 operation accepts one bounded public network context
//! on stdin and returns one genuine proof-bearing transaction on stdout.
//! Witness and signing material are derived and consumed inside this process
//! and never cross the IPC boundary. Vega remains absent until its exact
//! governed Figure 9 proving artifacts are available; ZK-AMS and ZK-X509 also
//! remain absent until their native release paths are genuinely available, so
//! receipt issuance remains closed.
use iroha_core::{
    privacy::PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1,
    privacy_profiles::compiled_privacy_profile_v1,
    privacy_release_evidence::{
        PrivacyReleaseTransactionContextV1, build_privacy_release_anonymous_pgc_network_action_v1,
        build_privacy_release_bootle_lantern_network_action_v1,
        build_privacy_release_fcmp_network_action_v1,
        build_privacy_release_ivm_private_note_network_action_v1,
        build_privacy_release_jindo_network_action_v1,
        build_privacy_release_orchard_network_action_v1,
        build_privacy_release_pq_masp_network_actions_v1,
        build_privacy_release_verange_network_action_v1,
        build_privacy_release_zk_ace_network_action_v1,
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf, PrivateKey, PublicKey};
use iroha_data_model::{
    block::BlockHeader,
    metadata::Metadata,
    prelude::{AccountId, AssetDefinitionId, NetworkId},
    privacy::{
        PrivacyCompiledProfileSnapshotV1, PrivacyPolicyIdV1, PrivacyPoolIdV1,
        PrivacyProposedLifecycleV1, PrivacyProtocolActivationLimitsV1, PrivacyProtocolIdV1,
        PrivacyProtocolLifecycleV1, TAIRA_PRIVACY_MAX_ACTION_BYTES_V1,
    },
    transaction::{FeePaymentIntent, SignedTransaction},
};
use iroha_version::codec::EncodeVersioned;
use sha2::{Digest as _, Sha256};
use std::{
    env,
    io::{Read as _, Write as _},
    num::NonZeroU32,
    process::ExitCode,
    time::Duration,
};
use zeroize::Zeroizing;
const REQUEST_SCHEMA: &str = "iroha.taira.privacy_action_driver_request";
const RESPONSE_SCHEMA: &str = "iroha.taira.privacy_action_driver_response";
const SCHEMA_VERSION: u8 = 1;
const ZK_ACE_OPERATION: &str = "build-zk-ace-action-v1";
const ANONYMOUS_PGC_OPERATION: &str = "build-anonymous-pgc-action-v1";
const VERANGE_OPERATION: &str = "build-verange-action-v1";
const UNAVAILABLE_VEGA_OPERATION: &str = "build-vega-action-v1";
const JINDO_OPERATION: &str = "build-jindo-action-v1";
const BOOTLE_LANTERN_OPERATION: &str = "build-bootle-lantern-action-v1";
const ORCHARD_OPERATION: &str = "build-orchard-action-v1";
const FCMP_OPERATION: &str = "build-fcmp-action-v1";
const IVM_PRIVATE_NOTE_OPERATION: &str = "build-ivm-private-note-action-v1";
const PQ_MASP_OPERATION: &str = "build-pq-masp-action-v1";
const REQUEST_ID_DOMAIN: &[u8] = b"iroha.taira.privacy_action_driver_request.v1\0";
const SEED_DOMAIN: &[u8] = b"iroha.taira.privacy_action_driver_seed.v1\0";
const VERANGE_SETUP_SEED_DOMAIN: &[u8] = b"iroha.taira.verange_qualification_setup_seed.v1\0";
const VERANGE_SETUP_IDENTITY_BINDING_DOMAIN: &[u8] =
    b"iroha.taira.verange_qualification_setup_identity.v1\0";
const MAX_REQUEST_BYTES: u64 = 16 * 1024;
const MAX_TRANSACTION_BYTES: usize = TAIRA_PRIVACY_MAX_ACTION_BYTES_V1 as usize;
const MAX_RESPONSE_BYTES: usize = 2 * MAX_TRANSACTION_BYTES
    + 2 * MAX_COMPILED_PROFILE_BYTES
    + 2 * MAX_ACTIVATION_TEMPLATE_BYTES
    + MAX_REQUEST_BYTES as usize;
const MAX_TTL_MILLIS: u64 = 2 * 60 * 60 * 1_000;
const MAX_CREATION_TIME_MILLIS: u64 = 9_223_372_036_854_775_807;
const MAX_ASSET_DEFINITION_ID_BYTES: usize = 1024;
const ORCHARD_RELEASE_EXPIRY_HEIGHT: u64 = 1_000_000_000;
const ZK_ACE_RELEASE_AMOUNT: u128 = 19;
const VERANGE_RELEASE_VALUES: [u64; 4] = [0, 1, 17, 4_294_967_295];
const QUALIFICATION_SCOPE: &str = "native-action-construction-only";
const CONSTRUCTION_ONLY_STATUS: &str = "constructible";
const JINDO_EXPERIMENTAL_STATUS: &str = "available-experimental";
const MISSING_CONTROLLER_CASE_EVIDENCE: &str = "MissingSealedControllerProtocolCaseEvidence";
const MISSING_ADMISSION_ARTIFACT_BUNDLE: &str = "MissingCanonicalAdmissionArtifactBundle";
const MISSING_GOVERNED_FIGURE9_PROVER_ARTIFACTS: &str = "MissingGovernedFigure9ProverArtifacts";
const MISSING_JINDO_KNOWLEDGE_SOUNDNESS: &str = "MissingDistributionWideKnowledgeSoundnessEvidence";
const MISSING_VERANGE_SETUP_AUTHORITY: &str =
    "MissingExactGenesisSourceClosedControllerSetupAuthorityIdentity";
const MISSING_VERANGE_SETUP_TRANSACTION_BUNDLE: &str =
    "MissingNativePublicOnlyVeRangePolicyActivationTransactionBundle";
const MISSING_VERANGE_STATE_QUERY_EVIDENCE: &str =
    "MissingFourPeerCanonicalVeRangeCapabilityRowStateQueriesBeforeAfterRestart";
const VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA: &str =
    "iroha.taira.verange_public_admission_artifacts";
const VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA_VERSION: u8 = 1;
const VERANGE_SETUP_REQUIREMENTS_SCHEMA: &str =
    "iroha.taira.verange_qualification_setup_requirements";
const VERANGE_SETUP_REQUIREMENTS_SCHEMA_VERSION: u8 = 1;
const VERANGE_QUALIFICATION_DOMAIN_ID: &str = "privacy.universal";
const VERANGE_ACTIVATION_HEIGHT_RULE: &str =
    "activate_at_height=proposed_at_height+minimum_delay_blocks";
const VERANGE_ACTIVATION_INSTRUCTION: &str = "register-privacy-protocol-activation-v1";
const VERANGE_ACTIVATION_LIFECYCLE: &str = "proposed-relative-height-template-v1";
const VERANGE_GOVERNANCE_PERMISSION: &str = "CanEnactGovernance";
const VERANGE_ACTIVATION_TEMPLATE_PROPOSED_AT_HEIGHT: u64 = 1;
const MAX_COMPILED_PROFILE_BYTES: usize = 64 * 1024;
const MAX_ACTIVATION_TEMPLATE_BYTES: usize = 64 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConstructibleOperationSpecV1 {
    operation: &'static str,
    protocol: &'static str,
}
const CONSTRUCTIBLE_OPERATION_SPECS_V1: [ConstructibleOperationSpecV1; 9] = [
    ConstructibleOperationSpecV1 {
        operation: ZK_ACE_OPERATION,
        protocol: "zk-ace-pq-authorization-v0",
    },
    ConstructibleOperationSpecV1 {
        operation: ANONYMOUS_PGC_OPERATION,
        protocol: "anonymous-pgc-k-out-of-n-v1",
    },
    ConstructibleOperationSpecV1 {
        operation: VERANGE_OPERATION,
        protocol: "verange-transparent-range-v1",
    },
    ConstructibleOperationSpecV1 {
        operation: JINDO_OPERATION,
        protocol: "iroha-jindo-polynomial-commitment-v0",
    },
    ConstructibleOperationSpecV1 {
        operation: BOOTLE_LANTERN_OPERATION,
        protocol: "iroha-bootle-lantern-anoncred-v1",
    },
    ConstructibleOperationSpecV1 {
        operation: ORCHARD_OPERATION,
        protocol: "orchard-halo2-actions-v1",
    },
    ConstructibleOperationSpecV1 {
        operation: FCMP_OPERATION,
        protocol: "monero-fcmp-plus-plus-v1",
    },
    ConstructibleOperationSpecV1 {
        operation: IVM_PRIVATE_NOTE_OPERATION,
        protocol: "iroha-ivm-private-note-stark-v1",
    },
    ConstructibleOperationSpecV1 {
        operation: PQ_MASP_OPERATION,
        protocol: "pq-masp-stark-v0",
    },
];
fn constructible_operation_spec_v1(operation: &str) -> Option<ConstructibleOperationSpecV1> {
    CONSTRUCTIBLE_OPERATION_SPECS_V1
        .iter()
        .copied()
        .find(|spec| spec.operation == operation)
}
fn unavailable_operation_reason_v1(operation: &str) -> Option<&'static str> {
    (operation == UNAVAILABLE_VEGA_OPERATION).then_some(MISSING_GOVERNED_FIGURE9_PROVER_ARTIFACTS)
}
#[derive(Debug, Clone, norito::JsonDeserialize, norito::JsonSerialize)]
#[norito(deny_unknown_fields)]
struct BuildActionRequestV1 {
    asset_definition_id: String,
    candidate_binding_sha256: String,
    creation_time_millis: u64,
    network_id_hex: String,
    nonce: u32,
    operation: String,
    request_id: String,
    schema: String,
    schema_version: u8,
    ttl_millis: u64,
}
#[derive(Debug, Clone, norito::JsonSerialize)]
struct RequestIdBodyV1 {
    asset_definition_id: String,
    candidate_binding_sha256: String,
    creation_time_millis: u64,
    network_id_hex: String,
    nonce: u32,
    operation: String,
    schema: String,
    schema_version: u8,
    ttl_millis: u64,
}
#[derive(Debug, norito::JsonSerialize)]
struct BuildActionResponseV1 {
    availability: String,
    candidate_binding_sha256: String,
    limitations: Vec<String>,
    network_outcome_authoritative: bool,
    operation: String,
    protocol: String,
    public_admission_artifacts: Option<VeRangePublicAdmissionArtifactsV1>,
    qualification_scope: String,
    request_id: String,
    schema: String,
    schema_version: u8,
    transaction_hash_hex: String,
    transaction_norito_hex: String,
    transaction_sha256: String,
}
/// Public-only material a future sealed controller needs to admit the exact
/// deterministic VeRange action identity and compiled verifier profile.
///
/// This deliberately contains neither a setup transaction nor any secret,
/// witness, endpoint, credential, or network outcome.  The explicit response
/// limitations keep qualification closed until the controller owns an
/// already-admitted setup authority and a native policy/activation bundle.
#[derive(Debug, norito::JsonSerialize)]
struct VeRangePublicAdmissionArtifactsV1 {
    action_authority_account_id: String,
    action_authority_public_key_hex: String,
    compiled_profile_norito_hex: String,
    compiled_profile_sha256: String,
    engine_id: String,
    engine_manifest_digest_hex: String,
    max_aggregation_count: u32,
    parameter_digest_hex: String,
    parameter_id_hex: String,
    policy_id_hex: String,
    proof_system_id: String,
    protocol_id: String,
    schema: String,
    schema_version: u8,
    setup_requirements: VeRangeQualificationSetupRequirementsV1,
    setup_requirements_sha256: String,
    statement_schema_digest_hex: String,
    verifier_digest_hex: String,
}
/// Canonical public requirements for a future controller-owned setup bundle.
///
/// The relative-height activation record is a binding template, not a signed
/// instruction and not evidence that the setup authority exists in genesis.
/// All signing material remains internal and absent from this response.
#[derive(Debug, norito::JsonSerialize)]
struct VeRangeQualificationSetupRequirementsV1 {
    action_authority_account_id: String,
    action_authority_public_key_hex: String,
    activation_height_rule: String,
    activation_instruction: String,
    activation_lifecycle: String,
    activation_minimum_delay_blocks: u64,
    activation_template_activate_at_height: u64,
    activation_template_norito_hex: String,
    activation_template_proposed_at_height: u64,
    activation_template_sha256: String,
    asset_definition_id: String,
    candidate_binding_sha256: String,
    compiled_profile_sha256: String,
    domain_id: String,
    governance_permission: String,
    protocol_id: String,
    schema: String,
    schema_version: u8,
    setup_authority_account_id: String,
    setup_authority_public_key_hex: String,
    setup_identity_binding_sha256: String,
}
fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(sha256_bytes(bytes))
}
fn decode_hex_32(value: &str, label: &str, reject_zero: bool) -> Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{label} must be exactly 64 lowercase hexadecimal characters"
        ));
    }
    let decoded = hex::decode(value).map_err(|_| format!("{label} is not hexadecimal"))?;
    let bytes: [u8; 32] = decoded
        .try_into()
        .map_err(|_| format!("{label} does not decode to 32 bytes"))?;
    if reject_zero && bytes == [0; 32] {
        return Err(format!("{label} must be nonzero"));
    }
    Ok(bytes)
}
fn request_id_body(request: &BuildActionRequestV1) -> RequestIdBodyV1 {
    RequestIdBodyV1 {
        asset_definition_id: request.asset_definition_id.clone(),
        candidate_binding_sha256: request.candidate_binding_sha256.clone(),
        creation_time_millis: request.creation_time_millis,
        network_id_hex: request.network_id_hex.clone(),
        nonce: request.nonce,
        operation: request.operation.clone(),
        schema: request.schema.clone(),
        schema_version: request.schema_version,
        ttl_millis: request.ttl_millis,
    }
}
fn operation_availability_v1(protocol: &str) -> &'static str {
    if protocol == "iroha-jindo-polynomial-commitment-v0" {
        JINDO_EXPERIMENTAL_STATUS
    } else {
        CONSTRUCTION_ONLY_STATUS
    }
}
fn operation_limitations_v1(protocol: &str) -> Vec<String> {
    let mut limitations = vec![MISSING_CONTROLLER_CASE_EVIDENCE.to_owned()];
    if protocol == "verange-transparent-range-v1" {
        limitations.extend([
            MISSING_VERANGE_SETUP_AUTHORITY.to_owned(),
            MISSING_VERANGE_SETUP_TRANSACTION_BUNDLE.to_owned(),
            MISSING_VERANGE_STATE_QUERY_EVIDENCE.to_owned(),
        ]);
    } else if protocol != "iroha-jindo-polynomial-commitment-v0" {
        limitations.push(MISSING_ADMISSION_ARTIFACT_BUNDLE.to_owned());
    } else {
        limitations.push(MISSING_JINDO_KNOWLEDGE_SOUNDNESS.to_owned());
    }
    limitations
}
fn compute_request_id(request: &BuildActionRequestV1) -> Result<String, String> {
    let body = norito::json::to_string(&request_id_body(request))
        .map_err(|error| format!("cannot encode request ID body: {error}"))?;
    let mut hash = Sha256::new();
    hash.update(REQUEST_ID_DOMAIN);
    hash.update(body.as_bytes());
    Ok(hex::encode(hash.finalize()))
}
fn derive_nonzero_seed(candidate: &[u8; 32], request_id: &[u8; 32], purpose: u8) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(SEED_DOMAIN);
    hash.update(candidate);
    hash.update(request_id);
    hash.update([purpose]);
    let mut seed: [u8; 32] = hash.finalize().into();
    if seed == [0; 32] {
        seed[0] = 1;
    }
    seed
}
fn derive_nonzero_verange_setup_seed(candidate: &[u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(VERANGE_SETUP_SEED_DOMAIN);
    hash.update(candidate);
    let mut seed: [u8; 32] = hash.finalize().into();
    if seed == [0; 32] {
        seed[0] = 1;
    }
    seed
}
fn bounded_ed25519_authority_v1(
    authority: &AccountId,
    label: &str,
) -> Result<(String, Vec<u8>), String> {
    let public_key = authority
        .try_signatory()
        .ok_or_else(|| format!("VeRange {label} authority is not single-signature"))?;
    let (algorithm, public_key_bytes) = public_key
        .try_to_bytes()
        .map_err(|error| format!("cannot expose VeRange {label} public key: {error}"))?;
    if algorithm != Algorithm::Ed25519
        || public_key_bytes.len() != 32
        || public_key_bytes.iter().all(|byte| *byte == 0)
    {
        return Err(format!(
            "VeRange {label} public key is not one nonzero Ed25519 key"
        ));
    }
    let account_id = authority.to_string();
    if account_id.is_empty()
        || !account_id.is_ascii()
        || account_id.len() > MAX_ASSET_DEFINITION_ID_BYTES
    {
        return Err(format!(
            "VeRange {label} authority account ID is not bounded ASCII"
        ));
    }
    Ok((account_id, public_key_bytes.to_vec()))
}
fn verange_setup_authority_v1(candidate: &[u8; 32]) -> Result<AccountId, String> {
    let seed = Zeroizing::new(derive_nonzero_verange_setup_seed(candidate));
    let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, seed.as_ref())
        .map_err(|error| format!("cannot derive VeRange qualification setup identity: {error}"))?;
    Ok(AccountId::new(PublicKey::from(private_key)))
}
fn verange_setup_identity_binding_v1(
    candidate: &[u8; 32],
    setup_account_id: &str,
    setup_public_key: &[u8],
) -> String {
    let mut hash = Sha256::new();
    hash.update(VERANGE_SETUP_IDENTITY_BINDING_DOMAIN);
    hash.update(candidate);
    hash.update(
        u64::try_from(setup_account_id.len())
            .expect("bounded setup account ID length fits u64")
            .to_le_bytes(),
    );
    hash.update(setup_account_id.as_bytes());
    hash.update(setup_public_key);
    hex::encode(hash.finalize())
}
fn verange_public_admission_artifacts_v1(
    authority: &AccountId,
    policy_id: [u8; 32],
    candidate: &[u8; 32],
    asset_definition_id: &AssetDefinitionId,
) -> Result<VeRangePublicAdmissionArtifactsV1, String> {
    let (authority_account_id, public_key_bytes) =
        bounded_ed25519_authority_v1(authority, "action")?;
    let setup_authority = verange_setup_authority_v1(candidate)?;
    let (setup_authority_account_id, setup_public_key_bytes) =
        bounded_ed25519_authority_v1(&setup_authority, "qualification setup")?;
    let compiled = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
        .map_err(|error| format!("native VeRange compiled profile is unavailable: {error}"))?;
    let compiled_profile = PrivacyCompiledProfileSnapshotV1::from(compiled);
    compiled_profile
        .validate()
        .map_err(|error| format!("native VeRange compiled profile is invalid: {error}"))?;
    let PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(limits) =
        compiled_profile.protocol_limits
    else {
        return Err("native VeRange compiled profile has the wrong limit tag".to_owned());
    };
    let compiled_profile_bytes = norito::to_bytes(&compiled_profile)
        .map_err(|error| format!("cannot encode native VeRange compiled profile: {error}"))?;
    if compiled_profile_bytes.is_empty()
        || compiled_profile_bytes.len() > MAX_COMPILED_PROFILE_BYTES
    {
        return Err("native VeRange compiled profile violates its byte bound".to_owned());
    }
    let activation_template_activate_at_height = VERANGE_ACTIVATION_TEMPLATE_PROPOSED_AT_HEIGHT
        .checked_add(PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1)
        .ok_or_else(|| "VeRange activation template height overflowed".to_owned())?;
    let activation_template = compiled.activation_record(PrivacyProtocolLifecycleV1::Proposed(
        PrivacyProposedLifecycleV1 {
            proposed_at_height: VERANGE_ACTIVATION_TEMPLATE_PROPOSED_AT_HEIGHT,
            activate_at_height: activation_template_activate_at_height,
        },
    ));
    activation_template
        .validate()
        .map_err(|error| format!("native VeRange activation template is invalid: {error}"))?;
    let activation_template_bytes = norito::to_bytes(&activation_template)
        .map_err(|error| format!("cannot encode native VeRange activation template: {error}"))?;
    if activation_template_bytes.is_empty()
        || activation_template_bytes.len() > MAX_ACTIVATION_TEMPLATE_BYTES
    {
        return Err("native VeRange activation template violates its byte bound".to_owned());
    }
    let compiled_profile_sha256 = sha256_hex(&compiled_profile_bytes);
    let setup_identity_binding_sha256 = verange_setup_identity_binding_v1(
        candidate,
        &setup_authority_account_id,
        &setup_public_key_bytes,
    );
    let setup_requirements = VeRangeQualificationSetupRequirementsV1 {
        action_authority_account_id: authority_account_id.clone(),
        action_authority_public_key_hex: hex::encode(&public_key_bytes),
        activation_height_rule: VERANGE_ACTIVATION_HEIGHT_RULE.to_owned(),
        activation_instruction: VERANGE_ACTIVATION_INSTRUCTION.to_owned(),
        activation_lifecycle: VERANGE_ACTIVATION_LIFECYCLE.to_owned(),
        activation_minimum_delay_blocks: PRIVACY_MIN_ACTIVATION_DELAY_BLOCKS_V1,
        activation_template_activate_at_height,
        activation_template_norito_hex: hex::encode(&activation_template_bytes),
        activation_template_proposed_at_height: VERANGE_ACTIVATION_TEMPLATE_PROPOSED_AT_HEIGHT,
        activation_template_sha256: sha256_hex(&activation_template_bytes),
        asset_definition_id: asset_definition_id.to_string(),
        candidate_binding_sha256: hex::encode(candidate),
        compiled_profile_sha256: compiled_profile_sha256.clone(),
        domain_id: VERANGE_QUALIFICATION_DOMAIN_ID.to_owned(),
        governance_permission: VERANGE_GOVERNANCE_PERMISSION.to_owned(),
        protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1
            .canonical_label()
            .to_owned(),
        schema: VERANGE_SETUP_REQUIREMENTS_SCHEMA.to_owned(),
        schema_version: VERANGE_SETUP_REQUIREMENTS_SCHEMA_VERSION,
        setup_authority_account_id,
        setup_authority_public_key_hex: hex::encode(&setup_public_key_bytes),
        setup_identity_binding_sha256,
    };
    let setup_requirements_json = norito::json::to_string(&setup_requirements)
        .map_err(|error| format!("cannot encode VeRange setup requirements: {error}"))?;
    let setup_requirements_sha256 = sha256_hex(setup_requirements_json.as_bytes());
    Ok(VeRangePublicAdmissionArtifactsV1 {
        action_authority_account_id: authority_account_id,
        action_authority_public_key_hex: hex::encode(public_key_bytes),
        compiled_profile_norito_hex: hex::encode(&compiled_profile_bytes),
        compiled_profile_sha256,
        engine_id: "native-verange-p256".to_owned(),
        engine_manifest_digest_hex: hex::encode(compiled_profile.engine_manifest_digest.as_bytes()),
        max_aggregation_count: limits.max_aggregation_count,
        parameter_digest_hex: hex::encode(compiled_profile.parameter_digest.as_bytes()),
        parameter_id_hex: hex::encode(compiled_profile.parameter_id.as_bytes()),
        policy_id_hex: hex::encode(policy_id),
        proof_system_id: "iroha-verange-p256".to_owned(),
        protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1
            .canonical_label()
            .to_owned(),
        schema: VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA.to_owned(),
        schema_version: VERANGE_PUBLIC_ADMISSION_ARTIFACT_SCHEMA_VERSION,
        setup_requirements,
        setup_requirements_sha256,
        statement_schema_digest_hex: hex::encode(
            compiled_profile.statement_schema_digest.as_bytes(),
        ),
        verifier_digest_hex: hex::encode(compiled_profile.verifier_digest.as_bytes()),
    })
}
fn read_request() -> Result<(BuildActionRequestV1, Vec<u8>), String> {
    if env::args_os().count() != 1 {
        return Err("the action driver accepts no command-line arguments".to_owned());
    }
    let mut input = Vec::new();
    std::io::stdin()
        .lock()
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut input)
        .map_err(|error| format!("cannot read action-driver request: {error}"))?;
    if input.is_empty() || input.len() as u64 > MAX_REQUEST_BYTES {
        return Err("action-driver request is empty or exceeds 16384 bytes".to_owned());
    }
    let request: BuildActionRequestV1 = norito::json::from_slice(&input)
        .map_err(|error| format!("cannot decode action-driver request: {error}"))?;
    let canonical = norito::json::to_string(&request)
        .map_err(|error| format!("cannot re-encode action-driver request: {error}"))?
        + "\n";
    if canonical.as_bytes() != input {
        return Err("action-driver request is not the one canonical JSON encoding".to_owned());
    }
    Ok((request, input))
}
fn incremented_context_v1(
    context: &PrivacyReleaseTransactionContextV1,
    nonce_delta: u32,
    creation_time_delta_millis: u64,
) -> Result<PrivacyReleaseTransactionContextV1, String> {
    let nonce = context
        .nonce
        .ok_or_else(|| "action-driver base context omitted its nonce".to_owned())?
        .get()
        .checked_add(nonce_delta)
        .and_then(NonZeroU32::new)
        .ok_or_else(|| "action-driver derived nonce overflowed".to_owned())?;
    let creation_time = context
        .creation_time
        .checked_add(Duration::from_millis(creation_time_delta_millis))
        .ok_or_else(|| "action-driver derived creation time overflowed".to_owned())?;
    Ok(PrivacyReleaseTransactionContextV1 {
        network_id: context.network_id,
        authority: context.authority.clone(),
        creation_time,
        time_to_live: context.time_to_live,
        nonce: Some(nonce),
        fee_payment: context.fee_payment.clone(),
        metadata: context.metadata.clone(),
        genesis_hash: context.genesis_hash,
    })
}
fn build_response(request: BuildActionRequestV1) -> Result<BuildActionResponseV1, String> {
    if request.schema != REQUEST_SCHEMA || request.schema_version != SCHEMA_VERSION {
        return Err("action-driver request selects an unsupported contract".to_owned());
    }
    if let Some(reason) = unavailable_operation_reason_v1(&request.operation) {
        return Err(reason.to_owned());
    }
    let operation = constructible_operation_spec_v1(&request.operation)
        .ok_or_else(|| "action-driver request selects an unsupported contract".to_owned())?;
    if request.creation_time_millis == 0
        || request.creation_time_millis > MAX_CREATION_TIME_MILLIS
        || request.ttl_millis == 0
        || request.ttl_millis > MAX_TTL_MILLIS
    {
        return Err("action-driver time fields are outside the v1 bounds".to_owned());
    }
    if request.asset_definition_id.is_empty()
        || !request.asset_definition_id.is_ascii()
        || request.asset_definition_id.len() > MAX_ASSET_DEFINITION_ID_BYTES
    {
        return Err("action-driver asset definition ID is not bounded ASCII".to_owned());
    }
    let nonce = NonZeroU32::new(request.nonce)
        .ok_or_else(|| "action-driver nonce must be nonzero".to_owned())?;
    let candidate = decode_hex_32(&request.candidate_binding_sha256, "candidate binding", true)?;
    let network_id_bytes = decode_hex_32(&request.network_id_hex, "NetworkId", true)?;
    if network_id_bytes[31] & 1 == 0 {
        return Err("NetworkId must carry the canonical Iroha hash marker bit".to_owned());
    }
    let request_id = decode_hex_32(&request.request_id, "request ID", true)?;
    if compute_request_id(&request)? != request.request_id {
        return Err("action-driver request ID is not derived from the canonical body".to_owned());
    }
    let asset_definition_id: AssetDefinitionId = request
        .asset_definition_id
        .parse()
        .map_err(|error| format!("invalid action-driver asset definition: {error}"))?;
    let signing_seed = Zeroizing::new(derive_nonzero_seed(&candidate, &request_id, 0));
    let fixture_seed = Zeroizing::new(derive_nonzero_seed(&candidate, &request_id, 1));
    let policy_seed = derive_nonzero_seed(&candidate, &request_id, 2);
    let counterparty_seed = Zeroizing::new(derive_nonzero_seed(&candidate, &request_id, 3));
    let proof_seed = Zeroizing::new(derive_nonzero_seed(&candidate, &request_id, 4));
    let pool_seed = derive_nonzero_seed(&candidate, &request_id, 5);
    let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, signing_seed.as_ref())
        .map_err(|error| format!("cannot derive action-driver signing key: {error}"))?;
    let counterparty_key =
        PrivateKey::from_bytes(Algorithm::Ed25519, counterparty_seed.as_ref())
            .map_err(|error| format!("cannot derive action-driver counterparty key: {error}"))?;
    let authority = AccountId::new(PublicKey::from(private_key.clone()));
    let counterparty = AccountId::new(PublicKey::from(counterparty_key));
    let network_id = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed(network_id_bytes),
    ));
    let context = PrivacyReleaseTransactionContextV1 {
        network_id,
        authority: authority.clone(),
        creation_time: Duration::from_millis(request.creation_time_millis),
        time_to_live: Some(Duration::from_millis(request.ttl_millis)),
        nonce: Some(nonce),
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
        metadata: Metadata::default(),
        genesis_hash: network_id_bytes,
    };
    let public_admission_artifacts = (operation.operation == VERANGE_OPERATION)
        .then(|| {
            verange_public_admission_artifacts_v1(
                &authority,
                policy_seed,
                &candidate,
                &asset_definition_id,
            )
        })
        .transpose()?;
    let pool_id = PrivacyPoolIdV1::new(pool_seed);
    let transaction: SignedTransaction = match operation.operation {
        ZK_ACE_OPERATION => {
            build_privacy_release_zk_ace_network_action_v1(
                context,
                authority,
                counterparty,
                asset_definition_id,
                ZK_ACE_RELEASE_AMOUNT,
                *fixture_seed,
                *proof_seed,
                &private_key,
            )
            .map_err(|error| format!("native ZK-ACE action construction failed: {error:?}"))?
            .transaction
        }
        ANONYMOUS_PGC_OPERATION => {
            build_privacy_release_anonymous_pgc_network_action_v1(
                context,
                asset_definition_id,
                pool_id,
                0,
                *fixture_seed,
                &private_key,
            )
            .map_err(|error| format!("native Anonymous-PGC action construction failed: {error:?}"))?
            .transaction
        }
        VERANGE_OPERATION => {
            build_privacy_release_verange_network_action_v1(
                context,
                asset_definition_id,
                PrivacyPolicyIdV1::new(policy_seed),
                VERANGE_RELEASE_VALUES.to_vec(),
                *fixture_seed,
                &private_key,
            )
            .map_err(|error| format!("native VeRange action construction failed: {error:?}"))?
            .transaction
        }
        JINDO_OPERATION => {
            build_privacy_release_jindo_network_action_v1(context, *fixture_seed, &private_key)
                .map_err(|error| format!("native Jindo action construction failed: {error:?}"))?
                .transaction
        }
        BOOTLE_LANTERN_OPERATION => {
            build_privacy_release_bootle_lantern_network_action_v1(
                context,
                *fixture_seed,
                &private_key,
            )
            .map_err(|error| {
                format!("native Bootle/Lantern action construction failed: {error:?}")
            })?
            .transaction
        }
        ORCHARD_OPERATION => {
            build_privacy_release_orchard_network_action_v1(
                context,
                pool_id,
                asset_definition_id,
                counterparty,
                ORCHARD_RELEASE_EXPIRY_HEIGHT,
                *fixture_seed,
                &private_key,
            )
            .map_err(|error| format!("native Orchard action construction failed: {error:?}"))?
            .transaction
        }
        FCMP_OPERATION => {
            build_privacy_release_fcmp_network_action_v1(
                context,
                asset_definition_id,
                pool_id,
                *fixture_seed,
                &private_key,
            )
            .map_err(|error| format!("native FCMP++ action construction failed: {error:?}"))?
            .transaction
        }
        IVM_PRIVATE_NOTE_OPERATION => {
            build_privacy_release_ivm_private_note_network_action_v1(
                context,
                asset_definition_id,
                pool_id,
                counterparty,
                *fixture_seed,
                &private_key,
            )
            .map_err(|error| format!("native private-IVM action construction failed: {error:?}"))?
            .transaction
        }
        PQ_MASP_OPERATION => {
            let preactivation_context = incremented_context_v1(&context, 1, 1)?;
            let replay_context = incremented_context_v1(&context, 2, 2)?;
            let post_restart_replay_context = incremented_context_v1(&context, 3, 3)?;
            // The constructor returns the full preactivation/canonical/replay/
            // post-restart set, but the v1 IPC has no typed artifact bundle.
            // Returning only this canonical transaction therefore remains
            // explicitly construction-only and cannot satisfy a controller case.
            let actions = build_privacy_release_pq_masp_network_actions_v1(
                preactivation_context,
                context,
                replay_context,
                post_restart_replay_context,
                *fixture_seed,
                &private_key,
            )
            .map_err(|error| format!("native PQ-MASP action construction failed: {error:?}"))?;
            if actions.canonical_statement.asset_definition_id != asset_definition_id {
                return Err(
                    "PQ-MASP request asset differs from the native release fixture".to_owned(),
                );
            }
            actions.canonical_transaction
        }
        _ => return Err("action-driver operation table is internally inconsistent".to_owned()),
    };
    let transaction_bytes = transaction.encode_versioned();
    if transaction_bytes.is_empty() || transaction_bytes.len() > MAX_TRANSACTION_BYTES {
        return Err("encoded action-driver transaction violates its byte bound".to_owned());
    }
    let transaction_hash_hex = hex::encode(transaction.hash().as_ref());
    Ok(BuildActionResponseV1 {
        availability: operation_availability_v1(operation.protocol).to_owned(),
        candidate_binding_sha256: request.candidate_binding_sha256,
        limitations: operation_limitations_v1(operation.protocol),
        network_outcome_authoritative: false,
        operation: operation.operation.to_owned(),
        protocol: operation.protocol.to_owned(),
        public_admission_artifacts,
        qualification_scope: QUALIFICATION_SCOPE.to_owned(),
        request_id: request.request_id,
        schema: RESPONSE_SCHEMA.to_owned(),
        schema_version: SCHEMA_VERSION,
        transaction_hash_hex,
        transaction_norito_hex: hex::encode(&transaction_bytes),
        transaction_sha256: sha256_hex(&transaction_bytes),
    })
}
fn run() -> Result<(), String> {
    let (request, mut request_bytes) = read_request()?;
    let response = build_response(request);
    request_bytes.fill(0);
    let response = response?;
    let mut encoded = norito::json::to_string(&response)
        .map_err(|error| format!("cannot encode action-driver response: {error}"))?;
    encoded.push('\n');
    if encoded.len() > MAX_RESPONSE_BYTES {
        return Err("encoded action-driver response violates its byte bound".to_owned());
    }
    std::io::stdout()
        .lock()
        .write_all(encoded.as_bytes())
        .map_err(|error| format!("cannot write action-driver response: {error}"))?;
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("privacy Exact12 action driver refused: {error}");
            ExitCode::FAILURE
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const DRIVER_SOURCE: &str = include_str!("privacy_exact12_action_driver.rs");
    const REQUEST_ID_GOLDEN: &str =
        include_str!("../../../../fixtures/privacy_exact12_action_driver_request_id_v1.json");
    #[derive(Debug, norito::JsonDeserialize)]
    struct RequestIdGoldenV1 {
        canonical_request: String,
        canonical_request_id_body: String,
        request: BuildActionRequestV1,
        request_id: String,
        schema: String,
        schema_version: u8,
    }
    #[test]
    fn python_and_rust_share_one_request_id_golden() {
        let golden: RequestIdGoldenV1 =
            norito::json::from_str(REQUEST_ID_GOLDEN).expect("decode request-ID golden");
        assert_eq!(
            golden.schema,
            "iroha.taira.privacy_action_driver_request_id_golden"
        );
        assert_eq!(golden.schema_version, 1);
        assert_eq!(golden.request.request_id, golden.request_id);
        assert_eq!(
            norito::json::to_string(&request_id_body(&golden.request))
                .expect("encode request-ID body"),
            golden.canonical_request_id_body
        );
        assert_eq!(
            norito::json::to_string(&golden.request).expect("encode full request") + "\n",
            golden.canonical_request
        );
        assert_eq!(
            compute_request_id(&golden.request).expect("derive request ID"),
            golden.request_id
        );
    }
    #[test]
    fn verange_setup_identity_is_candidate_only() {
        let candidate = [0x11; 32];
        let different_candidate = [0x22; 32];
        let network_id_before = [0x23; 32];
        let network_id_after = [0x45; 32];
        assert_ne!(network_id_before, network_id_after);
        let before = verange_setup_authority_v1(&candidate)
            .expect("derive candidate-bound setup identity")
            .to_string();
        let after = verange_setup_authority_v1(&candidate)
            .expect("rederive setup identity after NetworkId change")
            .to_string();
        let substituted = verange_setup_authority_v1(&different_candidate)
            .expect("derive another candidate setup identity")
            .to_string();
        assert_eq!(before, after);
        assert_ne!(before, substituted);
    }
    #[test]
    fn operation_table_contains_only_genuine_release_action_paths() {
        assert_eq!(CONSTRUCTIBLE_OPERATION_SPECS_V1.len(), 9);
        assert_eq!(
            CONSTRUCTIBLE_OPERATION_SPECS_V1.map(|spec| spec.protocol),
            [
                "zk-ace-pq-authorization-v0",
                "anonymous-pgc-k-out-of-n-v1",
                "verange-transparent-range-v1",
                "iroha-jindo-polynomial-commitment-v0",
                "iroha-bootle-lantern-anoncred-v1",
                "orchard-halo2-actions-v1",
                "monero-fcmp-plus-plus-v1",
                "iroha-ivm-private-note-stark-v1",
                "pq-masp-stark-v0",
            ]
        );
        assert!(
            CONSTRUCTIBLE_OPERATION_SPECS_V1
                .iter()
                .all(|spec| !matches!(
                    spec.protocol,
                    "vega-existing-credential-zk-v0"
                        | "iroha-zk-ams-v1"
                        | "iroha-zk-x509-stark-p256-v0"
                ))
        );
    }

    #[test]
    fn vega_is_rejected_before_request_material_derivation_with_exact_reason() {
        let request = BuildActionRequestV1 {
            asset_definition_id: String::new(),
            candidate_binding_sha256: String::new(),
            creation_time_millis: 0,
            network_id_hex: String::new(),
            nonce: 0,
            operation: UNAVAILABLE_VEGA_OPERATION.to_owned(),
            request_id: String::new(),
            schema: REQUEST_SCHEMA.to_owned(),
            schema_version: SCHEMA_VERSION,
            ttl_millis: 0,
        };
        assert!(constructible_operation_spec_v1(UNAVAILABLE_VEGA_OPERATION).is_none());
        assert_eq!(
            build_response(request).expect_err("Vega must remain unavailable"),
            MISSING_GOVERNED_FIGURE9_PROVER_ARTIFACTS
        );
        assert_eq!(
            DRIVER_SOURCE
                .matches("\"MissingGovernedFigure9ProverArtifacts\"")
                .count(),
            1
        );
        let retired_builder = ["build_privacy_release_", "vega_network_action_v1"].concat();
        assert!(!DRIVER_SOURCE.contains(&retired_builder));
        let response_source = DRIVER_SOURCE
            .split_once("fn build_response")
            .expect("build-response boundary")
            .1
            .split_once("fn run()")
            .expect("driver-run boundary")
            .0;
        assert!(
            response_source
                .find("unavailable_operation_reason_v1")
                .expect("explicit unavailable-operation check")
                < response_source
                    .find("let signing_seed")
                    .expect("secret seed derivation")
        );
    }
}
