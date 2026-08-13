//! Inert native semantic contract for the future Taira privacy-governance signer.
//!
//! This module deliberately has no service, key, signature, provisioning, rotation,
//! transport, journal, or public re-export.  It only defines the canonical request
//! and transaction checks that a separately provisioned retained-genesis-key service
//! must perform in the future.  No production module calls it.  Keeping the module in
//! normal builds makes release compilation type-check the contract while the retained
//! external genesis signer and its same-key `FinalizePrivacyGenesis` transition remain
//! unavailable.
//!
//! Request account fields use the data model's canonical JSON address representation:
//! bounded lowercase `0x` canonical bytes.  They never accept aliases, legacy
//! `key@domain` literals, or Unicode I105 text at this ASCII JSON boundary.
use std::num::NonZeroU32;
use iroha_crypto::{Algorithm, PublicKey, sha256};
use iroha_data_model::{
    NetworkId,
    account::{AccountAddress, AccountId},
    isi::privacy::RegisterPrivacyProtocolActivationV1,
    privacy::{
        MIN_PRIVACY_POLICY_DELAY_BLOCKS_V1, PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1,
        PrivacyProtocolLifecycleV1,
    },
    transaction::{Executable, FeePaymentIntent, TransactionBuilder, TransactionDomain},
};
use norito::json::{Map, Value};
pub(super) const PRIVACY_GOVERNANCE_PROVISIONING_BLOCKER_V1: &str =
    "MissingRetainedGenesisSignerFinalizePrivacyGenesisV1";
pub(super) const PRIVACY_GOVERNANCE_REQUEST_SCHEMA_V1: &str =
    "iroha.taira.privacy_governance_authority_request";
pub(super) const PRIVACY_GOVERNANCE_AUTHORITY_ENVELOPE_SCHEMA_V1: &str =
    "iroha.taira.privacy_governance_authority.v1";
pub(super) const PRIVACY_GOVERNANCE_REPLAY_NAMESPACE_V1: &str =
    "iroha.taira.privacy_governance_authority_replay.v1";
const REQUEST_SCHEMA_VERSION_V1: u64 = 1;
const REQUEST_OPERATION_V1: &str = "sign-exact12-privacy-governance-transaction-v1";
const REQUEST_ID_DOMAIN_V1: &[u8] = b"iroha.taira.privacy_governance_authority_request.v1\0";
const OPERATION_ID_DOMAIN_V1: &[u8] = b"iroha.taira.privacy_governance_authority.operation_id.v1\0";
const REPLAY_ID_DOMAIN_V1: &[u8] = b"iroha.taira.privacy_governance_authority.replay_id.v1\0";
const TRANSACTION_PAYLOAD_CODEC_V1: &str = "iroha.TransactionPayload/norito-adaptive-default-v1";
const TRANSACTION_PAYLOAD_PREHASH_V1: &str = "iroha.HashOf<TransactionPayload>/v1";
const TRANSACTION_DOMAIN_V1: &str = "network-from-reset-genesis-header-hash-v1";
const TRANSACTION_EXECUTABLE_KIND_V1: &str = "single-direct-instruction-v1";
const AUTHORIZED_ROOT_PEER_UID_V1: u32 = 0;
const MAX_ACTIVATION_BYTES_V1: usize = 4 * 1024 * 1024;
const MAX_REQUEST_BYTES_V1: usize = 8 * 1024 * 1024;
const MAX_TRANSACTION_PAYLOAD_BYTES_V1: usize = 8 * 1024 * 1024;
const MAX_TEXT_BYTES_V1: usize = 4_096;
const MAX_REQUEST_LIFETIME_MILLIS_V1: u64 = 15 * 60 * 1_000;
const MAX_TRANSACTION_CREATION_TIME_MILLIS_V1: u64 = i64::MAX as u64;
const REQUEST_FIELDS_V1: &[&str] = &[
    "activation",
    "authority_envelope_schema",
    "candidate",
    "controller",
    "fleet",
    "genesis",
    "operation",
    "request_id",
    "run",
    "schema",
    "schema_version",
    "transaction",
];
const ACTIVATION_FIELDS_V1: &[&str] = &[
    "activate_at_height",
    "compiled_profile_sha256",
    "instruction_norito_base64",
    "instruction_sha256",
    "proposed_at_height",
    "protocol",
];
const CANDIDATE_FIELDS_V1: &[&str] = &[
    "candidate_binding_sha256",
    "cargo_lock_sha256",
    "dpn_validator_release_commit",
    "source_commit",
    "workspace_source_manifest_sha256",
];
const CONTROLLER_FIELDS_V1: &[&str] = &["digest", "host_id", "installation_id"];
const FLEET_FIELDS_V1: &[&str] = &["four_peer_binding_sha256", "supervisor_binding_sha256"];
const GENESIS_FIELDS_V1: &[&str] = &[
    "authority_account_id",
    "expected_hash",
    "network_id_hex",
    "public_key",
    "reset_manifest_sha256",
    "signed_genesis_sha256",
    "unsigned_genesis_sha256",
];
const RUN_FIELDS_V1: &[&str] = &[
    "expires_at_unix_millis",
    "issued_at_unix_millis",
    "nonce",
    "replay_namespace",
];
const TRANSACTION_FIELDS_V1: &[&str] = &[
    "attachments",
    "authority_account_id",
    "creation_time_millis",
    "domain",
    "executable_kind",
    "fee_payment",
    "instruction_norito_sha256",
    "metadata",
    "network_id_hex",
    "nonce",
    "payload_codec",
    "payload_hash_hex",
    "payload_norito_base64",
    "payload_prehash",
    "payload_sha256",
    "time_to_live_millis",
];
const FEE_FIELDS_V1: &[&str] = &["charge_limits", "gas_limit", "payer"];
/// Independently pinned semantic inputs which a future retained-key service must own.
///
/// These values are not accepted from the request.  The future service must populate
/// them from its finalized genesis binding, installed controller binding, and the
/// authenticated root closer's run assignment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PrivacyGovernanceExpectedContextV1 {
    pub(super) candidate_binding_sha256: [u8; 32],
    pub(super) cargo_lock_sha256: [u8; 32],
    pub(super) dpn_validator_release_commit: [u8; 20],
    pub(super) source_commit: [u8; 20],
    pub(super) workspace_source_manifest_sha256: [u8; 32],
    pub(super) controller_digest: [u8; 32],
    pub(super) controller_host_id: String,
    pub(super) controller_installation_id: String,
    pub(super) four_peer_binding_sha256: [u8; 32],
    pub(super) supervisor_binding_sha256: [u8; 32],
    pub(super) reset_manifest_sha256: [u8; 32],
    pub(super) signed_genesis_sha256: [u8; 32],
    pub(super) unsigned_genesis_sha256: [u8; 32],
    pub(super) network_id: NetworkId,
    pub(super) genesis_public_key: PublicKey,
    pub(super) genesis_authority: AccountId,
    pub(super) compiled_profile_sha256: [u8; 32],
    pub(super) activation: PrivacyProtocolActivationRecordV1,
    pub(super) run_nonce_sha256: [u8; 32],
    pub(super) issued_at_unix_millis: u64,
    pub(super) expires_at_unix_millis: u64,
    pub(super) transaction_nonce: NonZeroU32,
}
/// Exact predecessor supplied by the authenticated live audit state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PrivacyGovernanceAuditPredecessorV1 {
    pub(super) sequence: u64,
    pub(super) head_sha256: [u8; 32],
}
/// One receipt's claimed newly committed audit position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PrivacyGovernanceAuditCommitV1 {
    pub(super) sequence: u64,
    pub(super) previous_head_sha256: [u8; 32],
    pub(super) committed_head_sha256: [u8; 32],
}
/// Post-commit journal head independently reread from authenticated live state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PrivacyGovernanceAuthenticatedLiveAuditV1 {
    pub(super) sequence: u64,
    pub(super) head_sha256: [u8; 32],
}
/// Result of the pure semantic request validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ValidatedPrivacyGovernanceRequestV1 {
    pub(super) request_id: [u8; 32],
    pub(super) request_sha256: [u8; 32],
    pub(super) operation_id: [u8; 32],
    pub(super) replay_id: [u8; 32],
    pub(super) transaction_payload_sha256: [u8; 32],
    pub(super) transaction_payload_hash: [u8; 32],
    pub(super) issued_at_unix_millis: u64,
    pub(super) expires_at_unix_millis: u64,
}
/// Closed reason returned by the inert semantic contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PrivacyGovernanceSemanticErrorV1 {
    KernelPeer,
    InvalidExpectedContext,
    NonCanonicalRequest,
    RequestContract(&'static str),
    BoundAxis(&'static str),
    TimeWindow,
    Replay,
    TransactionPayload,
    TransactionIntent(&'static str),
    Activation,
    AuditPredecessor,
}
/// Validate one exact request without performing I/O or exposing a signing path.
///
/// The authenticated kernel peer UID is checked before request parsing.  The caller
/// must eventually obtain it from the accepted Unix peer credentials, never from the
/// request body.  This function has no production caller until the separately built
/// retained genesis signer and same-key finalization transition are source-closed.
pub(super) fn validate_privacy_governance_request_v1(
    request_bytes: &[u8],
    authenticated_kernel_peer_uid: u32,
    now_unix_millis: u64,
    expected: &PrivacyGovernanceExpectedContextV1,
) -> Result<ValidatedPrivacyGovernanceRequestV1, PrivacyGovernanceSemanticErrorV1> {
    if authenticated_kernel_peer_uid != AUTHORIZED_ROOT_PEER_UID_V1 {
        return Err(PrivacyGovernanceSemanticErrorV1::KernelPeer);
    }
    validate_expected_context(expected)?;
    let value = parse_canonical_request(request_bytes)?;
    let request = exact_object(&value, REQUEST_FIELDS_V1)?;
    if required_u64(request, "schema_version")? != REQUEST_SCHEMA_VERSION_V1
        || required_str(request, "schema")? != PRIVACY_GOVERNANCE_REQUEST_SCHEMA_V1
        || required_str(request, "authority_envelope_schema")?
            != PRIVACY_GOVERNANCE_AUTHORITY_ENVELOPE_SCHEMA_V1
        || required_str(request, "operation")? != REQUEST_OPERATION_V1
    {
        return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(
            "request schema",
        ));
    }
    let request_id = required_sha256(request, "request_id")?;
    let mut body = (*request).clone();
    body.remove("request_id")
        .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(
            "request id",
        ))?;
    let body_bytes = canonical_json_bytes(&Value::Object(body))?;
    let mut request_id_preimage = Vec::with_capacity(REQUEST_ID_DOMAIN_V1.len() + body_bytes.len());
    request_id_preimage.extend_from_slice(REQUEST_ID_DOMAIN_V1);
    request_id_preimage.extend_from_slice(&body_bytes);
    if sha256(request_id_preimage) != request_id {
        return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(
            "request id",
        ));
    }
    let activation = nested_object(request, "activation", ACTIVATION_FIELDS_V1)?;
    let candidate = nested_object(request, "candidate", CANDIDATE_FIELDS_V1)?;
    let controller = nested_object(request, "controller", CONTROLLER_FIELDS_V1)?;
    let fleet = nested_object(request, "fleet", FLEET_FIELDS_V1)?;
    let genesis = nested_object(request, "genesis", GENESIS_FIELDS_V1)?;
    let run = nested_object(request, "run", RUN_FIELDS_V1)?;
    let transaction = nested_object(request, "transaction", TRANSACTION_FIELDS_V1)?;
    let fee = nested_object(transaction, "fee_payment", FEE_FIELDS_V1)?;
    require_sha256_axis(
        candidate,
        "candidate_binding_sha256",
        expected.candidate_binding_sha256,
        "candidate binding",
    )?;
    require_sha256_axis(
        candidate,
        "cargo_lock_sha256",
        expected.cargo_lock_sha256,
        "Cargo.lock",
    )?;
    require_commit_axis(
        candidate,
        "dpn_validator_release_commit",
        expected.dpn_validator_release_commit,
        "DPN validator release commit",
    )?;
    require_commit_axis(
        candidate,
        "source_commit",
        expected.source_commit,
        "source commit",
    )?;
    require_sha256_axis(
        candidate,
        "workspace_source_manifest_sha256",
        expected.workspace_source_manifest_sha256,
        "workspace source manifest",
    )?;
    require_sha256_axis(
        controller,
        "digest",
        expected.controller_digest,
        "controller digest",
    )?;
    require_text_axis(
        controller,
        "host_id",
        &expected.controller_host_id,
        "controller host",
    )?;
    require_text_axis(
        controller,
        "installation_id",
        &expected.controller_installation_id,
        "controller installation",
    )?;
    require_sha256_axis(
        fleet,
        "four_peer_binding_sha256",
        expected.four_peer_binding_sha256,
        "four-peer binding",
    )?;
    require_sha256_axis(
        fleet,
        "supervisor_binding_sha256",
        expected.supervisor_binding_sha256,
        "supervisor binding",
    )?;
    require_sha256_axis(
        genesis,
        "reset_manifest_sha256",
        expected.reset_manifest_sha256,
        "reset manifest",
    )?;
    require_sha256_axis(
        genesis,
        "signed_genesis_sha256",
        expected.signed_genesis_sha256,
        "signed genesis",
    )?;
    require_sha256_axis(
        genesis,
        "unsigned_genesis_sha256",
        expected.unsigned_genesis_sha256,
        "unsigned genesis",
    )?;
    let network_hash = *expected.network_id.as_bytes();
    for (object, field) in [
        (genesis, "expected_hash"),
        (genesis, "network_id_hex"),
        (transaction, "network_id_hex"),
    ] {
        if required_iroha_hash(object, field)? != network_hash {
            return Err(PrivacyGovernanceSemanticErrorV1::BoundAxis(
                "reset NetworkId",
            ));
        }
    }
    validate_genesis_identity(genesis, expected)?;
    let authority_text = required_text(transaction, "authority_account_id")?;
    if authority_text != required_text(genesis, "authority_account_id")? {
        return Err(PrivacyGovernanceSemanticErrorV1::BoundAxis(
            "transaction authority",
        ));
    }
    if required_str(transaction, "domain")? != TRANSACTION_DOMAIN_V1
        || required_str(transaction, "executable_kind")? != TRANSACTION_EXECUTABLE_KIND_V1
        || required_str(transaction, "payload_codec")? != TRANSACTION_PAYLOAD_CODEC_V1
        || required_str(transaction, "payload_prehash")? != TRANSACTION_PAYLOAD_PREHASH_V1
    {
        return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(
            "transaction contract",
        ));
    }
    if !required_null(transaction, "attachments")?
        || !required_empty_object(transaction, "metadata")?
        || required_str(fee, "payer")? != "authority"
        || !required_empty_array(fee, "charge_limits")?
        || !required_null(fee, "gas_limit")?
    {
        return Err(PrivacyGovernanceSemanticErrorV1::TransactionIntent(
            "request fee, metadata, or attachments",
        ));
    }
    let issued_at = required_positive_u64(run, "issued_at_unix_millis")?;
    let expires_at = required_positive_u64(run, "expires_at_unix_millis")?;
    let run_nonce = required_sha256(run, "nonce")?;
    if required_str(run, "replay_namespace")? != PRIVACY_GOVERNANCE_REPLAY_NAMESPACE_V1
        || run_nonce != expected.run_nonce_sha256
    {
        return Err(PrivacyGovernanceSemanticErrorV1::Replay);
    }
    if issued_at != expected.issued_at_unix_millis
        || expires_at != expected.expires_at_unix_millis
        || issued_at > MAX_TRANSACTION_CREATION_TIME_MILLIS_V1
        || expires_at <= issued_at
        || expires_at - issued_at > MAX_REQUEST_LIFETIME_MILLIS_V1
        || now_unix_millis < issued_at
        || now_unix_millis >= expires_at
    {
        return Err(PrivacyGovernanceSemanticErrorV1::TimeWindow);
    }
    let transaction_creation = required_positive_u64(transaction, "creation_time_millis")?;
    let transaction_ttl = required_positive_u64(transaction, "time_to_live_millis")?;
    let transaction_nonce = required_positive_u64(transaction, "nonce")?;
    if transaction_creation != issued_at
        || transaction_ttl != expires_at - issued_at
        || transaction_ttl > MAX_REQUEST_LIFETIME_MILLIS_V1
        || transaction_nonce != u64::from(expected.transaction_nonce.get())
    {
        return Err(PrivacyGovernanceSemanticErrorV1::TimeWindow);
    }
    let protocol_label = required_text(activation, "protocol")?;
    let protocol_id = PrivacyProtocolIdV1::from_canonical_label(protocol_label)
        .ok_or(PrivacyGovernanceSemanticErrorV1::Activation)?;
    if protocol_id != expected.activation.protocol_id {
        return Err(PrivacyGovernanceSemanticErrorV1::BoundAxis(
            "privacy protocol",
        ));
    }
    let proposed_at = required_positive_u64(activation, "proposed_at_height")?;
    let activate_at = required_positive_u64(activation, "activate_at_height")?;
    require_minimum_activation_delay(proposed_at, activate_at)?;
    require_sha256_axis(
        activation,
        "compiled_profile_sha256",
        expected.compiled_profile_sha256,
        "compiled profile",
    )?;
    let instruction_bytes = required_canonical_base64(
        activation,
        "instruction_norito_base64",
        MAX_ACTIVATION_BYTES_V1,
    )?;
    let instruction_sha256 = required_sha256(activation, "instruction_sha256")?;
    if sha256(&instruction_bytes) != instruction_sha256
        || required_sha256(transaction, "instruction_norito_sha256")? != instruction_sha256
    {
        return Err(PrivacyGovernanceSemanticErrorV1::Activation);
    }
    let transaction_payload = required_canonical_base64(
        transaction,
        "payload_norito_base64",
        MAX_TRANSACTION_PAYLOAD_BYTES_V1,
    )?;
    let transaction_payload_sha256 = required_sha256(transaction, "payload_sha256")?;
    if sha256(&transaction_payload) != transaction_payload_sha256 {
        return Err(PrivacyGovernanceSemanticErrorV1::TransactionPayload);
    }
    let declared_payload_hash = required_iroha_hash(transaction, "payload_hash_hex")?;
    let builder = TransactionBuilder::decode_payload(&transaction_payload)
        .map_err(|_| PrivacyGovernanceSemanticErrorV1::TransactionPayload)?;
    if builder.payload_hash_bytes() != declared_payload_hash {
        return Err(PrivacyGovernanceSemanticErrorV1::TransactionPayload);
    }
    validate_transaction_payload(
        builder.payload(),
        expected,
        proposed_at,
        activate_at,
        &instruction_bytes,
    )?;
    let operation_id = derive_operation_id_v1(request_id, run_nonce);
    let replay_id = derive_replay_id_v1(run_nonce);
    Ok(ValidatedPrivacyGovernanceRequestV1 {
        request_id,
        request_sha256: sha256(request_bytes),
        operation_id,
        replay_id,
        transaction_payload_sha256,
        transaction_payload_hash: declared_payload_hash,
        issued_at_unix_millis: issued_at,
        expires_at_unix_millis: expires_at,
    })
}
/// Authoritative native operation identity for a future replay journal.
///
/// The barriered Python scaffold only checks that a receipt field is digest-shaped;
/// it does not derive an operation identity.  This length-framed native derivation
/// therefore binds both the canonical request identity and the independently pinned
/// run nonce under the fixed replay namespace.
fn derive_operation_id_v1(request_id: [u8; 32], run_nonce: [u8; 32]) -> [u8; 32] {
    let replay_namespace = PRIVACY_GOVERNANCE_REPLAY_NAMESPACE_V1.as_bytes();
    let mut preimage = Vec::with_capacity(
        OPERATION_ID_DOMAIN_V1.len()
            + 4
            + replay_namespace.len()
            + 4
            + request_id.len()
            + 4
            + run_nonce.len(),
    );
    preimage.extend_from_slice(OPERATION_ID_DOMAIN_V1);
    preimage.extend_from_slice(
        &u32::try_from(replay_namespace.len())
            .expect("fixed replay namespace length fits u32")
            .to_be_bytes(),
    );
    preimage.extend_from_slice(replay_namespace);
    preimage.extend_from_slice(
        &u32::try_from(request_id.len())
            .expect("fixed request-id length fits u32")
            .to_be_bytes(),
    );
    preimage.extend_from_slice(&request_id);
    preimage.extend_from_slice(
        &u32::try_from(run_nonce.len())
            .expect("fixed nonce length fits u32")
            .to_be_bytes(),
    );
    preimage.extend_from_slice(&run_nonce);
    sha256(preimage)
}
/// Stable namespace-and-nonce identity which a future journal must reserve before signing.
fn derive_replay_id_v1(run_nonce: [u8; 32]) -> [u8; 32] {
    let replay_namespace = PRIVACY_GOVERNANCE_REPLAY_NAMESPACE_V1.as_bytes();
    let mut preimage = Vec::with_capacity(
        REPLAY_ID_DOMAIN_V1.len() + 4 + replay_namespace.len() + 4 + run_nonce.len(),
    );
    preimage.extend_from_slice(REPLAY_ID_DOMAIN_V1);
    preimage.extend_from_slice(
        &u32::try_from(replay_namespace.len())
            .expect("fixed replay namespace length fits u32")
            .to_be_bytes(),
    );
    preimage.extend_from_slice(replay_namespace);
    preimage.extend_from_slice(
        &u32::try_from(run_nonce.len())
            .expect("fixed nonce length fits u32")
            .to_be_bytes(),
    );
    preimage.extend_from_slice(&run_nonce);
    sha256(preimage)
}
/// Validate that a receipt commit is the exact fresh successor of a live predecessor
/// and equals a separately authenticated post-commit journal head.
pub(super) fn validate_privacy_governance_audit_successor_v1(
    predecessor: PrivacyGovernanceAuditPredecessorV1,
    committed: PrivacyGovernanceAuditCommitV1,
    authenticated_live: PrivacyGovernanceAuthenticatedLiveAuditV1,
) -> Result<(), PrivacyGovernanceSemanticErrorV1> {
    let expected_sequence = predecessor
        .sequence
        .checked_add(1)
        .ok_or(PrivacyGovernanceSemanticErrorV1::AuditPredecessor)?;
    if predecessor.sequence == 0
        || is_zero(&predecessor.head_sha256)
        || is_zero(&committed.previous_head_sha256)
        || is_zero(&committed.committed_head_sha256)
        || is_zero(&authenticated_live.head_sha256)
        || committed.sequence != expected_sequence
        || authenticated_live.sequence != committed.sequence
        || committed.previous_head_sha256 != predecessor.head_sha256
        || committed.committed_head_sha256 != authenticated_live.head_sha256
        || committed.committed_head_sha256 == predecessor.head_sha256
    {
        return Err(PrivacyGovernanceSemanticErrorV1::AuditPredecessor);
    }
    Ok(())
}
fn validate_expected_context(
    expected: &PrivacyGovernanceExpectedContextV1,
) -> Result<(), PrivacyGovernanceSemanticErrorV1> {
    let required_digests = [
        expected.candidate_binding_sha256,
        expected.cargo_lock_sha256,
        expected.workspace_source_manifest_sha256,
        expected.controller_digest,
        expected.four_peer_binding_sha256,
        expected.supervisor_binding_sha256,
        expected.reset_manifest_sha256,
        expected.signed_genesis_sha256,
        expected.unsigned_genesis_sha256,
        expected.compiled_profile_sha256,
        expected.run_nonce_sha256,
    ];
    if required_digests.iter().any(|digest| is_zero(digest))
        || is_zero(&expected.dpn_validator_release_commit)
        || is_zero(&expected.source_commit)
        || is_zero(expected.network_id.as_bytes())
        || expected.network_id.as_bytes()[31] & 1 == 0
        || !bounded_ascii(&expected.controller_host_id)
        || !bounded_ascii(&expected.controller_installation_id)
        || !matches!(
            expected.genesis_public_key.try_algorithm(),
            Ok(Algorithm::Ed25519)
        )
        || expected.genesis_authority.try_signatory() != Some(&expected.genesis_public_key)
        || expected.issued_at_unix_millis == 0
        || expected.issued_at_unix_millis > MAX_TRANSACTION_CREATION_TIME_MILLIS_V1
        || expected.expires_at_unix_millis <= expected.issued_at_unix_millis
        || expected.expires_at_unix_millis - expected.issued_at_unix_millis
            > MAX_REQUEST_LIFETIME_MILLIS_V1
        || expected.activation.validate().is_err()
    {
        return Err(PrivacyGovernanceSemanticErrorV1::InvalidExpectedContext);
    }
    let PrivacyProtocolLifecycleV1::Proposed(lifecycle) = expected.activation.lifecycle else {
        return Err(PrivacyGovernanceSemanticErrorV1::InvalidExpectedContext);
    };
    require_minimum_activation_delay(lifecycle.proposed_at_height, lifecycle.activate_at_height)
        .map_err(|_| PrivacyGovernanceSemanticErrorV1::InvalidExpectedContext)
}
fn validate_genesis_identity(
    genesis: &Map,
    expected: &PrivacyGovernanceExpectedContextV1,
) -> Result<(), PrivacyGovernanceSemanticErrorV1> {
    let public_key_text = required_text(genesis, "public_key")?;
    let public_key = public_key_text
        .parse::<PublicKey>()
        .map_err(|_| PrivacyGovernanceSemanticErrorV1::BoundAxis("genesis public key"))?;
    if !matches!(public_key.try_algorithm(), Ok(Algorithm::Ed25519))
        || public_key.to_string() != public_key_text
        || public_key != expected.genesis_public_key
    {
        return Err(PrivacyGovernanceSemanticErrorV1::BoundAxis(
            "genesis public key",
        ));
    }
    let authority_text = required_text(genesis, "authority_account_id")?;
    let parsed = parse_canonical_account_address(authority_text)?;
    let parsed_account = parsed
        .to_account_id()
        .map_err(|_| PrivacyGovernanceSemanticErrorV1::BoundAxis("genesis authority"))?;
    if parsed_account != expected.genesis_authority
        || parsed_account.try_signatory() != Some(&public_key)
    {
        return Err(PrivacyGovernanceSemanticErrorV1::BoundAxis(
            "genesis authority",
        ));
    }
    Ok(())
}
fn parse_canonical_account_address(
    text: &str,
) -> Result<AccountAddress, PrivacyGovernanceSemanticErrorV1> {
    let encoded = text
        .strip_prefix("0x")
        .ok_or(PrivacyGovernanceSemanticErrorV1::BoundAxis(
            "genesis authority",
        ))?;
    if encoded.is_empty()
        || encoded.len() % 2 != 0
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(PrivacyGovernanceSemanticErrorV1::BoundAxis(
            "genesis authority",
        ));
    }
    let mut canonical_bytes = Vec::with_capacity(encoded.len() / 2);
    for pair in encoded.as_bytes().chunks_exact(2) {
        let high = lower_hex_nibble(pair[0]).ok_or(PrivacyGovernanceSemanticErrorV1::BoundAxis(
            "genesis authority",
        ))?;
        let low = lower_hex_nibble(pair[1]).ok_or(PrivacyGovernanceSemanticErrorV1::BoundAxis(
            "genesis authority",
        ))?;
        canonical_bytes.push((high << 4) | low);
    }
    let address = AccountAddress::from_canonical_bytes(&canonical_bytes)
        .map_err(|_| PrivacyGovernanceSemanticErrorV1::BoundAxis("genesis authority"))?;
    if address
        .canonical_hex()
        .map_err(|_| PrivacyGovernanceSemanticErrorV1::BoundAxis("genesis authority"))?
        != text
    {
        return Err(PrivacyGovernanceSemanticErrorV1::BoundAxis(
            "genesis authority",
        ));
    }
    Ok(address)
}
fn validate_transaction_payload(
    payload: &iroha_data_model::transaction::TransactionPayload,
    expected: &PrivacyGovernanceExpectedContextV1,
    proposed_at: u64,
    activate_at: u64,
    declared_instruction_bytes: &[u8],
) -> Result<(), PrivacyGovernanceSemanticErrorV1> {
    if payload.domain != TransactionDomain::Network(expected.network_id)
        || payload.authority != expected.genesis_authority
    {
        return Err(PrivacyGovernanceSemanticErrorV1::TransactionIntent(
            "domain or authority",
        ));
    }
    if payload.creation_time_ms != expected.issued_at_unix_millis
        || payload.time_to_live_ms.map(|ttl| ttl.get())
            != Some(expected.expires_at_unix_millis - expected.issued_at_unix_millis)
        || payload.nonce != Some(expected.transaction_nonce)
    {
        return Err(PrivacyGovernanceSemanticErrorV1::TransactionIntent(
            "creation time, TTL, or nonce",
        ));
    }
    if !matches!(
        &payload.fee_payment,
        FeePaymentIntent::Authority(payment)
            if payment.charge_limits.is_empty() && payment.gas_limit.is_none()
    ) || !payload.metadata.is_empty()
        || payload.attachments.is_some()
    {
        return Err(PrivacyGovernanceSemanticErrorV1::TransactionIntent(
            "fee, metadata, or attachments",
        ));
    }
    let Executable::Instructions(instructions) = &payload.instructions else {
        return Err(PrivacyGovernanceSemanticErrorV1::TransactionIntent(
            "executable kind",
        ));
    };
    if instructions.len() != 1 {
        return Err(PrivacyGovernanceSemanticErrorV1::TransactionIntent(
            "instruction count",
        ));
    }
    let instruction = &instructions[0];
    let registration = instruction
        .as_any()
        .downcast_ref::<RegisterPrivacyProtocolActivationV1>()
        .ok_or(PrivacyGovernanceSemanticErrorV1::TransactionIntent(
            "instruction type",
        ))?;
    if registration.activation != expected.activation
        || registration.activation.validate().is_err()
        || instruction.dyn_encode() != declared_instruction_bytes
    {
        return Err(PrivacyGovernanceSemanticErrorV1::Activation);
    }
    let PrivacyProtocolLifecycleV1::Proposed(lifecycle) = registration.activation.lifecycle else {
        return Err(PrivacyGovernanceSemanticErrorV1::Activation);
    };
    if lifecycle.proposed_at_height != proposed_at || lifecycle.activate_at_height != activate_at {
        return Err(PrivacyGovernanceSemanticErrorV1::Activation);
    }
    require_minimum_activation_delay(proposed_at, activate_at)
}
fn require_minimum_activation_delay(
    proposed_at: u64,
    activate_at: u64,
) -> Result<(), PrivacyGovernanceSemanticErrorV1> {
    let earliest = proposed_at
        .checked_add(MIN_PRIVACY_POLICY_DELAY_BLOCKS_V1)
        .ok_or(PrivacyGovernanceSemanticErrorV1::Activation)?;
    if proposed_at == 0 || activate_at < earliest {
        return Err(PrivacyGovernanceSemanticErrorV1::Activation);
    }
    Ok(())
}
fn parse_canonical_request(bytes: &[u8]) -> Result<Value, PrivacyGovernanceSemanticErrorV1> {
    if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES_V1 || !bytes.is_ascii() {
        return Err(PrivacyGovernanceSemanticErrorV1::NonCanonicalRequest);
    }
    let value: Value = norito::json::from_slice(bytes)
        .map_err(|_| PrivacyGovernanceSemanticErrorV1::NonCanonicalRequest)?;
    if canonical_json_bytes(&value)?.as_slice() != bytes {
        return Err(PrivacyGovernanceSemanticErrorV1::NonCanonicalRequest);
    }
    Ok(value)
}
fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, PrivacyGovernanceSemanticErrorV1> {
    let mut bytes = norito::json::to_json(value)
        .map_err(|_| PrivacyGovernanceSemanticErrorV1::NonCanonicalRequest)?
        .into_bytes();
    if !bytes.is_ascii() {
        return Err(PrivacyGovernanceSemanticErrorV1::NonCanonicalRequest);
    }
    bytes.push(b'\n');
    Ok(bytes)
}
fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Result<&'a Map, PrivacyGovernanceSemanticErrorV1> {
    let object = value
        .as_object()
        .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(
            "expected object",
        ))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(
            "unexpected fields",
        ));
    }
    Ok(object)
}
fn nested_object<'a>(
    object: &'a Map,
    field: &'static str,
    fields: &[&str],
) -> Result<&'a Map, PrivacyGovernanceSemanticErrorV1> {
    exact_object(
        object
            .get(field)
            .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(field))?,
        fields,
    )
}
fn required_str<'a>(
    object: &'a Map,
    field: &'static str,
) -> Result<&'a str, PrivacyGovernanceSemanticErrorV1> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(field))
}
fn required_text<'a>(
    object: &'a Map,
    field: &'static str,
) -> Result<&'a str, PrivacyGovernanceSemanticErrorV1> {
    let value = required_str(object, field)?;
    if !bounded_ascii(value) {
        return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(field));
    }
    Ok(value)
}
fn bounded_ascii(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_TEXT_BYTES_V1 && value.is_ascii()
}
fn required_u64(
    object: &Map,
    field: &'static str,
) -> Result<u64, PrivacyGovernanceSemanticErrorV1> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(field))
}
fn required_positive_u64(
    object: &Map,
    field: &'static str,
) -> Result<u64, PrivacyGovernanceSemanticErrorV1> {
    let value = required_u64(object, field)?;
    if value == 0 {
        return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(field));
    }
    Ok(value)
}
fn required_null(
    object: &Map,
    field: &'static str,
) -> Result<bool, PrivacyGovernanceSemanticErrorV1> {
    object
        .get(field)
        .map(Value::is_null)
        .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(field))
}
fn required_empty_array(
    object: &Map,
    field: &'static str,
) -> Result<bool, PrivacyGovernanceSemanticErrorV1> {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(|value| value.is_empty())
        .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(field))
}
fn required_empty_object(
    object: &Map,
    field: &'static str,
) -> Result<bool, PrivacyGovernanceSemanticErrorV1> {
    object
        .get(field)
        .and_then(Value::as_object)
        .map(|value| value.is_empty())
        .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(field))
}
fn required_sha256(
    object: &Map,
    field: &'static str,
) -> Result<[u8; 32], PrivacyGovernanceSemanticErrorV1> {
    let digest = required_lower_hex::<32>(object, field)?;
    if is_zero(&digest) {
        return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(field));
    }
    Ok(digest)
}
fn required_iroha_hash(
    object: &Map,
    field: &'static str,
) -> Result<[u8; 32], PrivacyGovernanceSemanticErrorV1> {
    let hash = required_sha256(object, field)?;
    if hash[31] & 1 == 0 {
        return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(field));
    }
    Ok(hash)
}
fn required_commit(
    object: &Map,
    field: &'static str,
) -> Result<[u8; 20], PrivacyGovernanceSemanticErrorV1> {
    let commit = required_lower_hex::<20>(object, field)?;
    if is_zero(&commit) {
        return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(field));
    }
    Ok(commit)
}
fn required_lower_hex<const N: usize>(
    object: &Map,
    field: &'static str,
) -> Result<[u8; N], PrivacyGovernanceSemanticErrorV1> {
    let text = required_str(object, field)?;
    if text.len() != N * 2
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(field));
    }
    let mut decoded = [0_u8; N];
    for (index, output) in decoded.iter_mut().enumerate() {
        let high = lower_hex_nibble(text.as_bytes()[index * 2])
            .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(field))?;
        let low = lower_hex_nibble(text.as_bytes()[index * 2 + 1])
            .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(field))?;
        *output = (high << 4) | low;
    }
    Ok(decoded)
}
fn lower_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
fn require_sha256_axis(
    object: &Map,
    field: &'static str,
    expected: [u8; 32],
    label: &'static str,
) -> Result<(), PrivacyGovernanceSemanticErrorV1> {
    if required_sha256(object, field)? != expected {
        return Err(PrivacyGovernanceSemanticErrorV1::BoundAxis(label));
    }
    Ok(())
}
fn require_commit_axis(
    object: &Map,
    field: &'static str,
    expected: [u8; 20],
    label: &'static str,
) -> Result<(), PrivacyGovernanceSemanticErrorV1> {
    if required_commit(object, field)? != expected {
        return Err(PrivacyGovernanceSemanticErrorV1::BoundAxis(label));
    }
    Ok(())
}
fn require_text_axis(
    object: &Map,
    field: &'static str,
    expected: &str,
    label: &'static str,
) -> Result<(), PrivacyGovernanceSemanticErrorV1> {
    if required_text(object, field)? != expected {
        return Err(PrivacyGovernanceSemanticErrorV1::BoundAxis(label));
    }
    Ok(())
}
fn required_canonical_base64(
    object: &Map,
    field: &'static str,
    maximum_decoded_bytes: usize,
) -> Result<Vec<u8>, PrivacyGovernanceSemanticErrorV1> {
    let encoded = required_str(object, field)?.as_bytes();
    if encoded.is_empty() || encoded.len() % 4 != 0 {
        return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(field));
    }
    let padding = match encoded.get(encoded.len().saturating_sub(2)..) {
        Some([b'=', b'=']) => 2,
        Some([_, b'=']) => 1,
        _ => 0,
    };
    let decoded_len = encoded
        .len()
        .checked_div(4)
        .and_then(|groups| groups.checked_mul(3))
        .and_then(|bytes| bytes.checked_sub(padding))
        .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(field))?;
    if decoded_len == 0 || decoded_len > maximum_decoded_bytes {
        return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(field));
    }
    let mut decoded = Vec::with_capacity(decoded_len);
    let group_count = encoded.len() / 4;
    for (index, group) in encoded.chunks_exact(4).enumerate() {
        let last = index + 1 == group_count;
        let a = base64_sextet(group[0])
            .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(field))?;
        let b = base64_sextet(group[1])
            .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(field))?;
        if group[2] == b'=' {
            if !last || group[3] != b'=' || b & 0x0f != 0 {
                return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(field));
            }
            decoded.push((a << 2) | (b >> 4));
            continue;
        }
        let c = base64_sextet(group[2])
            .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(field))?;
        let d = if group[3] == b'=' {
            if !last || c & 0x03 != 0 {
                return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(field));
            }
            None
        } else {
            Some(
                base64_sextet(group[3])
                    .ok_or(PrivacyGovernanceSemanticErrorV1::RequestContract(field))?,
            )
        };
        decoded.push((a << 2) | (b >> 4));
        decoded.push((b << 4) | (c >> 2));
        if let Some(d) = d {
            decoded.push((c << 6) | d);
        }
    }
    if decoded.len() != decoded_len {
        return Err(PrivacyGovernanceSemanticErrorV1::RequestContract(field));
    }
    Ok(decoded)
}
fn base64_sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
fn is_zero<const N: usize>(bytes: &[u8; N]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}
#[cfg(test)]
mod tests {
    use std::{num::NonZeroU64, time::Duration};
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        block::BlockHeader,
        isi::InstructionBox,
        name::Name,
        privacy::{
            PrivacyAssuranceV1, PrivacyEngineManifestDigestV1, PrivacyParameterDigestV1,
            PrivacyParameterIdV1, PrivacyProposedLifecycleV1, PrivacyProtocolActivationLimitsV1,
            PrivacyStatementSchemaDigestV1, PrivacyVerifierDigestV1,
            VERANGE_TAIRA_MAX_AGGREGATION_COUNT_V1, VeRangeActivationLimitsV1,
        },
        proof::{ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyId},
        transaction::{ExecutableBatchItem, TransactionPayload},
    };
    use iroha_primitives::json::Json;
    use super::*;
    const TEST_PUBLIC_KEY: &str =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
    const FOREIGN_PUBLIC_KEY: &str =
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4";
    fn digest(label: &[u8]) -> [u8; 32] {
        sha256(label)
    }
    fn commit(byte: u8) -> [u8; 20] {
        [byte; 20]
    }
    fn fixture_context() -> PrivacyGovernanceExpectedContextV1 {
        let public_key = TEST_PUBLIC_KEY
            .parse::<PublicKey>()
            .expect("test key parses");
        let authority = AccountId::new(public_key.clone());
        let network_id =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"privacy-governance-test-genesis",
            )));
        let protocol_id = PrivacyProtocolIdV1::VeRangeTransparentRangeV1;
        let activation = PrivacyProtocolActivationRecordV1 {
            protocol_id,
            proof_system_id: protocol_id.expected_proof_system(),
            engine_id: protocol_id.expected_engine(),
            parameter_id: PrivacyParameterIdV1::new(digest(b"parameter-id")),
            parameter_digest: PrivacyParameterDigestV1::new(digest(b"parameter")),
            verifier_digest: PrivacyVerifierDigestV1::new(digest(b"verifier")),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new(digest(b"statement")),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new(digest(b"engine")),
            lifecycle: PrivacyProtocolLifecycleV1::Proposed(PrivacyProposedLifecycleV1 {
                proposed_at_height: 100,
                activate_at_height: 400,
            }),
            protocol_limits: PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
                VeRangeActivationLimitsV1 {
                    max_aggregation_count: VERANGE_TAIRA_MAX_AGGREGATION_COUNT_V1,
                },
            ),
            pending_protocol_limits_tightening: None,
            assurance: PrivacyAssuranceV1::Experimental,
        };
        PrivacyGovernanceExpectedContextV1 {
            candidate_binding_sha256: digest(b"candidate"),
            cargo_lock_sha256: digest(b"Cargo.lock"),
            dpn_validator_release_commit: commit(0x11),
            source_commit: commit(0x22),
            workspace_source_manifest_sha256: digest(b"workspace"),
            controller_digest: digest(b"controller"),
            controller_host_id: "taira-controller-host-a".to_owned(),
            controller_installation_id: "taira-controller-installation-a".to_owned(),
            four_peer_binding_sha256: digest(b"four-peer"),
            supervisor_binding_sha256: digest(b"supervisor"),
            reset_manifest_sha256: digest(b"reset"),
            signed_genesis_sha256: digest(b"signed-genesis"),
            unsigned_genesis_sha256: digest(b"unsigned-genesis"),
            network_id,
            genesis_public_key: public_key,
            genesis_authority: authority,
            compiled_profile_sha256: digest(b"compiled-profile"),
            activation,
            run_nonce_sha256: digest(b"run-nonce"),
            issued_at_unix_millis: 1_800_000_000_000,
            expires_at_unix_millis: 1_800_000_060_000,
            transaction_nonce: NonZeroU32::new(7).expect("nonzero nonce"),
        }
    }
    fn valid_payload(context: &PrivacyGovernanceExpectedContextV1) -> TransactionPayload {
        let mut builder = TransactionBuilder::new(
            context.network_id,
            context.genesis_authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([RegisterPrivacyProtocolActivationV1::new(context.activation)]);
        builder
            .set_creation_time(Duration::from_millis(context.issued_at_unix_millis))
            .set_ttl(Duration::from_millis(
                context.expires_at_unix_millis - context.issued_at_unix_millis,
            ))
            .set_nonce(context.transaction_nonce);
        builder.into_payload().expect("valid payload")
    }
    fn encode_payload(payload: TransactionPayload) -> Vec<u8> {
        TransactionBuilder::from_payload(payload)
            .expect("test payload remains structurally encodable")
            .encode_payload()
    }
    fn activation_bytes(context: &PrivacyGovernanceExpectedContextV1) -> Vec<u8> {
        let instruction =
            InstructionBox::from(RegisterPrivacyProtocolActivationV1::new(context.activation));
        instruction.dyn_encode()
    }
    fn object(entries: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
        Value::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        )
    }
    fn hex(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            out.push(char::from(DIGITS[usize::from(byte >> 4)]));
            out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
        }
        out
    }
    fn base64(bytes: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let a = chunk[0];
            let b = chunk.get(1).copied().unwrap_or(0);
            let c = chunk.get(2).copied().unwrap_or(0);
            out.push(char::from(TABLE[usize::from(a >> 2)]));
            out.push(char::from(TABLE[usize::from(((a & 0x03) << 4) | (b >> 4))]));
            if chunk.len() > 1 {
                out.push(char::from(TABLE[usize::from(((b & 0x0f) << 2) | (c >> 6))]));
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(char::from(TABLE[usize::from(c & 0x3f)]));
            } else {
                out.push('=');
            }
        }
        out
    }
    fn proposed_heights(context: &PrivacyGovernanceExpectedContextV1) -> (u64, u64) {
        let PrivacyProtocolLifecycleV1::Proposed(lifecycle) = context.activation.lifecycle else {
            panic!("fixture activation is proposed");
        };
        (lifecycle.proposed_at_height, lifecycle.activate_at_height)
    }
    fn request_value_with_payload(
        context: &PrivacyGovernanceExpectedContextV1,
        payload_bytes: &[u8],
    ) -> Value {
        let payload_builder =
            TransactionBuilder::decode_payload(payload_bytes).expect("test payload");
        let instruction_bytes = activation_bytes(context);
        let instruction_digest = sha256(&instruction_bytes);
        let (proposed_at, activate_at) = proposed_heights(context);
        let authority = context
            .genesis_authority
            .to_canonical_hex()
            .expect("fixture authority encodes");
        let network_hash = *context.network_id.as_bytes();
        object([
            (
                "activation",
                object([
                    ("activate_at_height", Value::from(activate_at)),
                    (
                        "compiled_profile_sha256",
                        Value::from(hex(&context.compiled_profile_sha256)),
                    ),
                    (
                        "instruction_norito_base64",
                        Value::from(base64(&instruction_bytes)),
                    ),
                    ("instruction_sha256", Value::from(hex(&instruction_digest))),
                    ("proposed_at_height", Value::from(proposed_at)),
                    (
                        "protocol",
                        Value::from(context.activation.protocol_id.canonical_label()),
                    ),
                ]),
            ),
            (
                "authority_envelope_schema",
                Value::from(PRIVACY_GOVERNANCE_AUTHORITY_ENVELOPE_SCHEMA_V1),
            ),
            (
                "candidate",
                object([
                    (
                        "candidate_binding_sha256",
                        Value::from(hex(&context.candidate_binding_sha256)),
                    ),
                    (
                        "cargo_lock_sha256",
                        Value::from(hex(&context.cargo_lock_sha256)),
                    ),
                    (
                        "dpn_validator_release_commit",
                        Value::from(hex(&context.dpn_validator_release_commit)),
                    ),
                    ("source_commit", Value::from(hex(&context.source_commit))),
                    (
                        "workspace_source_manifest_sha256",
                        Value::from(hex(&context.workspace_source_manifest_sha256)),
                    ),
                ]),
            ),
            (
                "controller",
                object([
                    ("digest", Value::from(hex(&context.controller_digest))),
                    ("host_id", Value::from(context.controller_host_id.clone())),
                    (
                        "installation_id",
                        Value::from(context.controller_installation_id.clone()),
                    ),
                ]),
            ),
            (
                "fleet",
                object([
                    (
                        "four_peer_binding_sha256",
                        Value::from(hex(&context.four_peer_binding_sha256)),
                    ),
                    (
                        "supervisor_binding_sha256",
                        Value::from(hex(&context.supervisor_binding_sha256)),
                    ),
                ]),
            ),
            (
                "genesis",
                object([
                    ("authority_account_id", Value::from(authority.clone())),
                    ("expected_hash", Value::from(hex(&network_hash))),
                    ("network_id_hex", Value::from(hex(&network_hash))),
                    (
                        "public_key",
                        Value::from(context.genesis_public_key.to_string()),
                    ),
                    (
                        "reset_manifest_sha256",
                        Value::from(hex(&context.reset_manifest_sha256)),
                    ),
                    (
                        "signed_genesis_sha256",
                        Value::from(hex(&context.signed_genesis_sha256)),
                    ),
                    (
                        "unsigned_genesis_sha256",
                        Value::from(hex(&context.unsigned_genesis_sha256)),
                    ),
                ]),
            ),
            ("operation", Value::from(REQUEST_OPERATION_V1)),
            (
                "run",
                object([
                    (
                        "expires_at_unix_millis",
                        Value::from(context.expires_at_unix_millis),
                    ),
                    (
                        "issued_at_unix_millis",
                        Value::from(context.issued_at_unix_millis),
                    ),
                    ("nonce", Value::from(hex(&context.run_nonce_sha256))),
                    (
                        "replay_namespace",
                        Value::from(PRIVACY_GOVERNANCE_REPLAY_NAMESPACE_V1),
                    ),
                ]),
            ),
            ("schema", Value::from(PRIVACY_GOVERNANCE_REQUEST_SCHEMA_V1)),
            ("schema_version", Value::from(REQUEST_SCHEMA_VERSION_V1)),
            (
                "transaction",
                object([
                    ("attachments", Value::Null),
                    ("authority_account_id", Value::from(authority)),
                    (
                        "creation_time_millis",
                        Value::from(context.issued_at_unix_millis),
                    ),
                    ("domain", Value::from(TRANSACTION_DOMAIN_V1)),
                    (
                        "executable_kind",
                        Value::from(TRANSACTION_EXECUTABLE_KIND_V1),
                    ),
                    (
                        "fee_payment",
                        object([
                            ("charge_limits", Value::Array(Vec::new())),
                            ("gas_limit", Value::Null),
                            ("payer", Value::from("authority")),
                        ]),
                    ),
                    (
                        "instruction_norito_sha256",
                        Value::from(hex(&instruction_digest)),
                    ),
                    ("metadata", Value::Object(Map::new())),
                    ("network_id_hex", Value::from(hex(&network_hash))),
                    (
                        "nonce",
                        Value::from(u64::from(context.transaction_nonce.get())),
                    ),
                    ("payload_codec", Value::from(TRANSACTION_PAYLOAD_CODEC_V1)),
                    (
                        "payload_hash_hex",
                        Value::from(hex(&payload_builder.payload_hash_bytes())),
                    ),
                    ("payload_norito_base64", Value::from(base64(payload_bytes))),
                    (
                        "payload_prehash",
                        Value::from(TRANSACTION_PAYLOAD_PREHASH_V1),
                    ),
                    ("payload_sha256", Value::from(hex(&sha256(payload_bytes)))),
                    (
                        "time_to_live_millis",
                        Value::from(context.expires_at_unix_millis - context.issued_at_unix_millis),
                    ),
                ]),
            ),
        ])
    }
    fn reseal_request_id(value: &mut Value) -> Vec<u8> {
        value
            .as_object_mut()
            .expect("request object")
            .remove("request_id");
        let body = canonical_json_bytes(value).expect("canonical body");
        let mut preimage = REQUEST_ID_DOMAIN_V1.to_vec();
        preimage.extend_from_slice(&body);
        value
            .as_object_mut()
            .expect("request object")
            .insert("request_id".to_owned(), Value::from(hex(&sha256(preimage))));
        canonical_json_bytes(value).expect("canonical request")
    }
    fn valid_request(context: &PrivacyGovernanceExpectedContextV1) -> Vec<u8> {
        let payload = encode_payload(valid_payload(context));
        let mut value = request_value_with_payload(context, &payload);
        reseal_request_id(&mut value)
    }
    fn nested_mut<'a>(value: &'a mut Value, field: &str) -> &'a mut Map {
        value
            .as_object_mut()
            .expect("object")
            .get_mut(field)
            .expect("field")
            .as_object_mut()
            .expect("nested object")
    }
    fn replace_payload(value: &mut Value, bytes: &[u8]) {
        let transaction = nested_mut(value, "transaction");
        transaction.insert(
            "payload_norito_base64".to_owned(),
            Value::from(base64(bytes)),
        );
        transaction.insert(
            "payload_sha256".to_owned(),
            Value::from(hex(&sha256(bytes))),
        );
        if let Ok(builder) = TransactionBuilder::decode_payload(bytes) {
            transaction.insert(
                "payload_hash_hex".to_owned(),
                Value::from(hex(&builder.payload_hash_bytes())),
            );
        }
    }
    fn validate(
        bytes: &[u8],
        context: &PrivacyGovernanceExpectedContextV1,
    ) -> Result<ValidatedPrivacyGovernanceRequestV1, PrivacyGovernanceSemanticErrorV1> {
        validate_privacy_governance_request_v1(
            bytes,
            AUTHORIZED_ROOT_PEER_UID_V1,
            context.issued_at_unix_millis + 1,
            context,
        )
    }
    #[test]
    fn exact_request_and_transaction_are_bound_without_authority_side_effects() {
        let context = fixture_context();
        let bytes = valid_request(&context);
        let validated = validate(&bytes, &context).expect("exact request validates");
        assert_eq!(validated.request_sha256, sha256(&bytes));
        assert_eq!(
            validated.issued_at_unix_millis,
            context.issued_at_unix_millis
        );
        assert_eq!(
            validated.expires_at_unix_millis,
            context.expires_at_unix_millis
        );
        assert!(!is_zero(&validated.request_id));
        assert!(!is_zero(&validated.operation_id));
        assert!(!is_zero(&validated.replay_id));
        assert!(!is_zero(&validated.transaction_payload_sha256));
        assert_eq!(validated.transaction_payload_hash[31] & 1, 1);
    }
    #[test]
    fn root_peer_is_checked_before_any_request_decode() {
        let context = fixture_context();
        assert_eq!(
            validate_privacy_governance_request_v1(
                b"not-json",
                50_000,
                context.issued_at_unix_millis + 1,
                &context,
            ),
            Err(PrivacyGovernanceSemanticErrorV1::KernelPeer)
        );
        assert_eq!(
            validate_privacy_governance_request_v1(
                b"not-json",
                AUTHORIZED_ROOT_PEER_UID_V1,
                context.issued_at_unix_millis + 1,
                &context,
            ),
            Err(PrivacyGovernanceSemanticErrorV1::NonCanonicalRequest)
        );
    }
    #[test]
    fn canonical_request_rejects_coercion_duplicates_and_recomputed_self_hashes() {
        let context = fixture_context();
        let request = valid_request(&context);
        let mut spaced = request.clone();
        spaced.insert(0, b' ');
        assert_eq!(
            validate(&spaced, &context),
            Err(PrivacyGovernanceSemanticErrorV1::NonCanonicalRequest)
        );
        assert!(request.ends_with(b"}\n"));
        let mut duplicate = request[..request.len() - 2].to_vec();
        duplicate.extend_from_slice(b",\"schema_version\":1}\n");
        assert_eq!(
            validate(&duplicate, &context),
            Err(PrivacyGovernanceSemanticErrorV1::NonCanonicalRequest)
        );
        for non_integer in [Value::Bool(true), Value::from(1.0_f64)] {
            let mut value: Value = norito::json::from_slice(&request).expect("request JSON");
            value
                .as_object_mut()
                .expect("object")
                .insert("schema_version".to_owned(), non_integer);
            let coercion = reseal_request_id(&mut value);
            assert!(matches!(
                validate(&coercion, &context),
                Err(PrivacyGovernanceSemanticErrorV1::RequestContract(
                    "schema_version"
                ))
            ));
        }
        for non_integer in [Value::Bool(true), Value::from(60_000.0_f64)] {
            let mut value: Value = norito::json::from_slice(&request).expect("request JSON");
            nested_mut(&mut value, "transaction")
                .insert("time_to_live_millis".to_owned(), non_integer);
            let coercion = reseal_request_id(&mut value);
            assert!(matches!(
                validate(&coercion, &context),
                Err(PrivacyGovernanceSemanticErrorV1::RequestContract(
                    "time_to_live_millis"
                ))
            ));
        }
        let foreign_network = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"foreign-request-network")),
        );
        let splice_cases = [
            (
                "candidate",
                "candidate_binding_sha256",
                hex(&digest(b"other-candidate")),
            ),
            (
                "candidate",
                "cargo_lock_sha256",
                hex(&digest(b"other-lock")),
            ),
            ("candidate", "source_commit", hex(&commit(0x33))),
            ("controller", "host_id", "other-controller".to_owned()),
            (
                "fleet",
                "four_peer_binding_sha256",
                hex(&digest(b"other-fleet")),
            ),
            (
                "genesis",
                "signed_genesis_sha256",
                hex(&digest(b"other-genesis")),
            ),
            ("genesis", "public_key", FOREIGN_PUBLIC_KEY.to_owned()),
            ("genesis", "expected_hash", hex(foreign_network.as_bytes())),
        ];
        for (section, field, replacement) in splice_cases {
            let mut value: Value = norito::json::from_slice(&request).expect("request JSON");
            nested_mut(&mut value, section).insert(field.to_owned(), Value::from(replacement));
            let hostile = reseal_request_id(&mut value);
            assert!(
                matches!(
                    validate(&hostile, &context),
                    Err(PrivacyGovernanceSemanticErrorV1::BoundAxis(_))
                ),
                "splice {section}.{field} was admitted"
            );
        }
        let canonical_authority = context
            .genesis_authority
            .to_canonical_hex()
            .expect("fixture authority encodes");
        let mut odd_length_authority = canonical_authority.clone();
        odd_length_authority.pop();
        let foreign_authority = AccountId::new(
            FOREIGN_PUBLIC_KEY
                .parse::<PublicKey>()
                .expect("foreign public key"),
        )
        .to_canonical_hex()
        .expect("foreign authority encodes");
        for hostile_authority in [
            canonical_authority.to_ascii_uppercase(),
            odd_length_authority,
            "0x00".to_owned(),
            "taira_genesis_authority@genesis".to_owned(),
            foreign_authority,
        ] {
            let mut value: Value = norito::json::from_slice(&request).expect("request JSON");
            nested_mut(&mut value, "genesis").insert(
                "authority_account_id".to_owned(),
                Value::from(hostile_authority.clone()),
            );
            nested_mut(&mut value, "transaction").insert(
                "authority_account_id".to_owned(),
                Value::from(hostile_authority),
            );
            let hostile = reseal_request_id(&mut value);
            assert!(matches!(
                validate(&hostile, &context),
                Err(PrivacyGovernanceSemanticErrorV1::BoundAxis(
                    "genesis authority"
                ))
            ));
        }
    }
    #[test]
    fn canonical_payload_rejects_trailing_bytes_and_noncanonical_base64() {
        let context = fixture_context();
        let request = valid_request(&context);
        let mut value: Value = norito::json::from_slice(&request).expect("request JSON");
        let transaction = nested_mut(&mut value, "transaction");
        transaction.insert("payload_norito_base64".to_owned(), Value::from("AB=="));
        transaction.insert(
            "payload_sha256".to_owned(),
            Value::from(hex(&digest(b"unused"))),
        );
        let hostile = reseal_request_id(&mut value);
        assert!(matches!(
            validate(&hostile, &context),
            Err(PrivacyGovernanceSemanticErrorV1::RequestContract(
                "payload_norito_base64"
            ))
        ));
        let mut trailing = encode_payload(valid_payload(&context));
        trailing.push(0);
        let mut value: Value = norito::json::from_slice(&request).expect("request JSON");
        replace_payload(&mut value, &trailing);
        let hostile = reseal_request_id(&mut value);
        assert_eq!(
            validate(&hostile, &context),
            Err(PrivacyGovernanceSemanticErrorV1::TransactionPayload)
        );
    }
    #[test]
    fn transaction_semantics_reject_domain_authority_time_fee_and_metadata_splices() {
        let context = fixture_context();
        let request = valid_request(&context);
        let original = valid_payload(&context);
        let foreign_key: PublicKey =
            "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
                .parse()
                .expect("foreign key");
        let foreign_network = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"foreign-network")),
        );
        let mut attacks = Vec::new();
        let mut payload = original.clone();
        payload.domain = TransactionDomain::Genesis;
        attacks.push(payload);
        let mut payload = original.clone();
        payload.domain = TransactionDomain::Network(foreign_network);
        attacks.push(payload);
        let mut payload = original.clone();
        payload.authority = AccountId::new(foreign_key);
        attacks.push(payload);
        let mut payload = original.clone();
        payload.creation_time_ms += 1;
        attacks.push(payload);
        let mut payload = original.clone();
        payload.time_to_live_ms = NonZeroU64::new(1);
        attacks.push(payload);
        let mut payload = original.clone();
        payload.nonce = NonZeroU32::new(8);
        attacks.push(payload);
        let mut payload = original.clone();
        payload.fee_payment = FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(1));
        attacks.push(payload);
        let mut payload = original.clone();
        payload
            .metadata
            .insert("hostile".parse::<Name>().expect("name"), Json::new("value"));
        attacks.push(payload);
        for payload in attacks {
            let bytes = encode_payload(payload);
            let mut value: Value = norito::json::from_slice(&request).expect("request JSON");
            replace_payload(&mut value, &bytes);
            let hostile = reseal_request_id(&mut value);
            assert!(matches!(
                validate(&hostile, &context),
                Err(PrivacyGovernanceSemanticErrorV1::TransactionIntent(_))
            ));
        }
    }
    #[test]
    fn transaction_semantics_reject_attachments_batches_and_instruction_splices() {
        let context = fixture_context();
        let request = valid_request(&context);
        let original = valid_payload(&context);
        let attachment = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
            VerifyingKeyId::new("halo2/ipa", "vk_1"),
        );
        let mut attached = original.clone();
        attached.attachments =
            Some(ProofAttachmentList::try_from(vec![attachment]).expect("one attachment"));
        let registration = RegisterPrivacyProtocolActivationV1::new(context.activation);
        let mut batched = original.clone();
        batched.instructions = Executable::Batch(
            vec![ExecutableBatchItem::Instruction(
                registration.clone().into(),
            )]
            .into(),
        );
        let mut doubled = original;
        doubled.instructions =
            Executable::Instructions(vec![registration.clone().into(), registration.into()].into());
        for payload in [attached, batched, doubled] {
            let bytes = encode_payload(payload);
            let mut value: Value = norito::json::from_slice(&request).expect("request JSON");
            replace_payload(&mut value, &bytes);
            let hostile = reseal_request_id(&mut value);
            assert!(matches!(
                validate(&hostile, &context),
                Err(PrivacyGovernanceSemanticErrorV1::TransactionIntent(_))
            ));
        }
    }
    #[test]
    fn activation_and_time_replay_contracts_fail_closed() {
        let context = fixture_context();
        let request = valid_request(&context);
        assert_eq!(
            validate_privacy_governance_request_v1(
                &request,
                AUTHORIZED_ROOT_PEER_UID_V1,
                context.issued_at_unix_millis - 1,
                &context,
            ),
            Err(PrivacyGovernanceSemanticErrorV1::TimeWindow)
        );
        assert_eq!(
            validate_privacy_governance_request_v1(
                &request,
                AUTHORIZED_ROOT_PEER_UID_V1,
                context.expires_at_unix_millis,
                &context,
            ),
            Err(PrivacyGovernanceSemanticErrorV1::TimeWindow)
        );
        let mut replay: Value = norito::json::from_slice(&request).expect("request JSON");
        nested_mut(&mut replay, "run").insert(
            "replay_namespace".to_owned(),
            Value::from("candidate-controlled-replay"),
        );
        let replay = reseal_request_id(&mut replay);
        assert_eq!(
            validate(&replay, &context),
            Err(PrivacyGovernanceSemanticErrorV1::Replay)
        );
        let mut activation: Value = norito::json::from_slice(&request).expect("request JSON");
        nested_mut(&mut activation, "activation")
            .insert("activate_at_height".to_owned(), Value::from(399_u64));
        let activation = reseal_request_id(&mut activation);
        assert_eq!(
            validate(&activation, &context),
            Err(PrivacyGovernanceSemanticErrorV1::Activation)
        );
        let mut instruction: Value = norito::json::from_slice(&request).expect("request JSON");
        let replacement = b"candidate-generated-activation";
        let replacement_sha256 = hex(&sha256(replacement));
        {
            let activation = nested_mut(&mut instruction, "activation");
            activation.insert(
                "instruction_norito_base64".to_owned(),
                Value::from(base64(replacement)),
            );
            activation.insert(
                "instruction_sha256".to_owned(),
                Value::from(replacement_sha256.clone()),
            );
        }
        nested_mut(&mut instruction, "transaction").insert(
            "instruction_norito_sha256".to_owned(),
            Value::from(replacement_sha256),
        );
        let instruction = reseal_request_id(&mut instruction);
        assert_eq!(
            validate(&instruction, &context),
            Err(PrivacyGovernanceSemanticErrorV1::Activation)
        );
        let first = validate(&request, &context).expect("first request");
        let mut second_context = context.clone();
        second_context.candidate_binding_sha256 = digest(b"second-candidate");
        let second =
            validate(&valid_request(&second_context), &second_context).expect("second request");
        assert_ne!(first.request_id, second.request_id);
        assert_ne!(
            first.operation_id, second.operation_id,
            "one run nonce must not make distinct canonical requests alias"
        );
        assert_eq!(
            first.replay_id, second.replay_id,
            "one run nonce must retain one journal replay identity"
        );
    }
    #[test]
    fn audit_commit_requires_exact_fresh_live_successor() {
        let predecessor = PrivacyGovernanceAuditPredecessorV1 {
            sequence: 8,
            head_sha256: digest(b"previous"),
        };
        let committed = PrivacyGovernanceAuditCommitV1 {
            sequence: 9,
            previous_head_sha256: predecessor.head_sha256,
            committed_head_sha256: digest(b"committed"),
        };
        let authenticated_live = PrivacyGovernanceAuthenticatedLiveAuditV1 {
            sequence: committed.sequence,
            head_sha256: digest(b"committed"),
        };
        validate_privacy_governance_audit_successor_v1(predecessor, committed, authenticated_live)
            .expect("fresh successor");
        let attacks = [
            PrivacyGovernanceAuditCommitV1 {
                sequence: 8,
                ..committed
            },
            PrivacyGovernanceAuditCommitV1 {
                previous_head_sha256: digest(b"spliced"),
                ..committed
            },
            PrivacyGovernanceAuditCommitV1 {
                committed_head_sha256: predecessor.head_sha256,
                ..committed
            },
        ];
        for attack in attacks {
            assert_eq!(
                validate_privacy_governance_audit_successor_v1(
                    predecessor,
                    attack,
                    authenticated_live,
                ),
                Err(PrivacyGovernanceSemanticErrorV1::AuditPredecessor)
            );
        }
        for hostile_live_head in [[0_u8; 32], digest(b"stale-live")] {
            assert_eq!(
                validate_privacy_governance_audit_successor_v1(
                    predecessor,
                    committed,
                    PrivacyGovernanceAuthenticatedLiveAuditV1 {
                        sequence: committed.sequence,
                        head_sha256: hostile_live_head,
                    },
                ),
                Err(PrivacyGovernanceSemanticErrorV1::AuditPredecessor)
            );
        }
        for hostile_live_sequence in [committed.sequence - 1, committed.sequence + 1] {
            assert_eq!(
                validate_privacy_governance_audit_successor_v1(
                    predecessor,
                    committed,
                    PrivacyGovernanceAuthenticatedLiveAuditV1 {
                        sequence: hostile_live_sequence,
                        head_sha256: committed.committed_head_sha256,
                    },
                ),
                Err(PrivacyGovernanceSemanticErrorV1::AuditPredecessor)
            );
        }
        assert_eq!(
            validate_privacy_governance_audit_successor_v1(
                PrivacyGovernanceAuditPredecessorV1 {
                    sequence: u64::MAX,
                    head_sha256: predecessor.head_sha256,
                },
                committed,
                authenticated_live,
            ),
            Err(PrivacyGovernanceSemanticErrorV1::AuditPredecessor)
        );
        assert_eq!(
            validate_privacy_governance_audit_successor_v1(
                PrivacyGovernanceAuditPredecessorV1 {
                    sequence: 0,
                    head_sha256: predecessor.head_sha256,
                },
                PrivacyGovernanceAuditCommitV1 {
                    sequence: 1,
                    previous_head_sha256: predecessor.head_sha256,
                    committed_head_sha256: committed.committed_head_sha256,
                },
                PrivacyGovernanceAuthenticatedLiveAuditV1 {
                    sequence: 1,
                    head_sha256: committed.committed_head_sha256,
                },
            ),
            Err(PrivacyGovernanceSemanticErrorV1::AuditPredecessor)
        );
    }
    #[test]
    fn production_surface_has_no_role_service_or_signing_caller() {
        let parent = include_str!("../external_software_signer.rs");
        assert!(parent.contains("#[allow(dead_code)]\nmod privacy_governance;"));
        for source in [
            include_str!("adapter.rs"),
            include_str!("envelope.rs"),
            include_str!("journal.rs"),
            include_str!("protocol.rs"),
            include_str!("runtime_adapters.rs"),
            include_str!("runtime_backends.rs"),
            include_str!("service.rs"),
            include_str!("unix.rs"),
            include_str!("typed_payload.rs"),
            include_str!("../bin/sorafs_external_software_signer.rs"),
        ] {
            assert!(!source.contains("privacy_governance::"));
            assert!(!source.contains("PrivacyGovernance"));
        }
        let this_source = include_str!("privacy_governance.rs");
        let production_contract = this_source
            .split("#[cfg(test)]")
            .next()
            .expect("production contract prefix");
        for forbidden in [
            "SoftwareSignerRoleV1",
            "SoftwareSignerServiceV1",
            "PrivateKey",
            "fn sign",
            "fn provision",
            "fn rotate",
            "std::fs",
            "UnixStream",
        ] {
            assert!(
                !production_contract.contains(forbidden),
                "forbidden authority surface: {forbidden}"
            );
        }
        assert!(this_source.contains(PRIVACY_GOVERNANCE_PROVISIONING_BLOCKER_V1));
    }
}
