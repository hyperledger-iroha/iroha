//! Authenticated-worker core for production-shaped zk-X.509 actions.
//!
//! The surrounding process boundary owns authentication, file custody, and
//! resource isolation.  This module owns the cryptographic and transaction
//! boundary: it accepts public ledger state plus in-process secret bytes and
//! returns only an ordinary signed transaction.  It deliberately calls the
//! production compiled-profile accessor first, so the worker cannot turn
//! release-candidate material into a signable transaction while any X.509
//! readiness pin remains absent.

use crate::{
    privacy_engines::zk_x509::{
        engine::{
            prepare_zk_x509_prover_input_v1, prove_zk_x509_credential_proof_v1_with_rng,
            verify_zk_x509_credential_proof_v1,
        },
        profile::{
            ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1,
            ZK_X509_NATIVE_RELEASE_EXPECTATIONS_JSON_SHA256_V1,
            ZK_X509_NATIVE_RELEASE_EXPECTATIONS_NORITO_SHA256_V1,
            ZK_X509_RELEASE_KAT_EXPECTED_PROOF_BYTES_V1,
            ZK_X509_RELEASE_KAT_EXPECTED_PROOF_SHA256_V1, ZK_X509_RESOURCE_CERTIFICATE_SHA256_V1,
            ZK_X509_SOUNDNESS_CERTIFICATE_SHA256_V1,
        },
    },
    privacy_profiles::{
        CompiledPrivacyProfileV1, compiled_privacy_profile_v1,
        zk_x509_release_candidate_profile_material_v1,
    },
    privacy_state::{
        assemble_privacy_zk_x509_authoritative_state_for_worker_v1,
        validate_privacy_zk_x509_statement_state_v1,
    },
};
use core::{num::NonZeroU32, time::Duration};
use iroha_crypto::{Algorithm, PrivateKey, PublicKey};
use iroha_data_model::{
    isi::privacy::SubmitPrivacyProofV1,
    metadata::Metadata,
    prelude::{AccountId, NetworkId},
    privacy::{
        IrohaZkX509StarkP256StatementV1, PrivacyCompiledProfileSnapshotV1,
        PrivacyConsensusLimitsV1, PrivacyProofBytesV1, PrivacyProofEnvelopeV1, PrivacyProofV1,
        PrivacyProtocolIdV1, PrivacyRootV1, PrivacyStatementDigestV1, PrivacyStatementV1,
        PrivacyTransactionIntentDigestV1, PrivacyZkX509CertificatePolicyRecordV1,
        PrivacyZkX509CrlRecordV1, PrivacyZkX509TrustAnchorRecordV1,
        TAIRA_PRIVACY_MAX_ACTION_BYTES_V1,
    },
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionPayload},
};
use iroha_version::codec::EncodeVersioned;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

/// Canonical public-request schema accepted by the first X.509 worker.
pub const PRIVACY_ZK_X509_WORKER_PUBLIC_REQUEST_VERSION_V1: u8 = 1;
/// Exact authenticated framing and release-evidence contract version.
pub const PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1: u8 = 1;
/// Sole action index supported by the one-action worker transaction.
pub const PRIVACY_ZK_X509_WORKER_ACTION_INDEX_V1: u32 = 0;
const PRIVACY_ZK_X509_WORKER_RELEASE_EVIDENCE_DOMAIN_V1: &[u8] =
    b"iroha.privacy.zk-x509.worker-release-evidence.v1";

/// Exact source-installed pins reported by the authenticated worker identity.
///
/// Zero source pins remain visible here so candidate tooling can name the
/// blockers precisely. `release_evidence_sha256` is populated only when every
/// constituent is nonzero, mutually consistent, and within the canonical
/// proof bound. A production package must additionally carry the equal
/// compiled-profile digest and qualified-isolation package pin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrivacyZkX509WorkerReleasePinsV1 {
    /// Hash of the deterministic candidate protocol-profile snapshot.
    pub protocol_profile_sha256: [u8; 32],
    /// Hash of the production compiled profile, present only after activation readiness closes.
    pub compiled_profile_sha256: Option<[u8; 32]>,
    /// Exact byte length of the reviewed deterministic release KAT proof.
    pub kat_proof_bytes: u32,
    /// Source pin for the reviewed deterministic release KAT proof.
    pub kat_proof_sha256: [u8; 32],
    /// Source pin for the canonical native-expectations Norito fixture.
    pub expectations_norito_sha256: [u8; 32],
    /// Source pin for the typed-equal native-expectations JSON projection.
    pub expectations_json_sha256: [u8; 32],
    /// Independent source pin for the reviewed soundness certificate.
    pub soundness_certificate_sha256: [u8; 32],
    /// Independent source pin for the reviewed native-resource certificate.
    pub resource_certificate_sha256: [u8; 32],
    /// Aggregate domain-separated digest of every complete release-evidence pin.
    pub release_evidence_sha256: Option<[u8; 32]>,
}

const fn digest_is_nonzero_v1(digest: [u8; 32]) -> bool {
    let mut index = 0;
    while index < digest.len() {
        if digest[index] != 0 {
            return true;
        }
        index += 1;
    }
    false
}

fn compiled_profile_sha256_v1(
    profile: CompiledPrivacyProfileV1,
) -> Result<[u8; 32], PrivacyZkX509WorkerErrorV1> {
    let snapshot = PrivacyCompiledProfileSnapshotV1::from(profile);
    let encoded = norito::to_bytes(&snapshot)
        .map_err(|_| PrivacyZkX509WorkerErrorV1::TransactionFinalization)?;
    Ok(Sha256::digest(encoded).into())
}

fn release_evidence_sha256_v1(
    protocol_profile_sha256: [u8; 32],
    kat_proof_bytes: u32,
    kat_proof_sha256: [u8; 32],
    expectations_norito_sha256: [u8; 32],
    expectations_json_sha256: [u8; 32],
    soundness_certificate_sha256: [u8; 32],
    resource_certificate_sha256: [u8; 32],
) -> Option<[u8; 32]> {
    if !digest_is_nonzero_v1(protocol_profile_sha256)
        || kat_proof_bytes == 0
        || kat_proof_bytes > ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1
        || !digest_is_nonzero_v1(kat_proof_sha256)
        || !digest_is_nonzero_v1(expectations_norito_sha256)
        || !digest_is_nonzero_v1(expectations_json_sha256)
        || expectations_norito_sha256 == expectations_json_sha256
        || !digest_is_nonzero_v1(soundness_certificate_sha256)
        || !digest_is_nonzero_v1(resource_certificate_sha256)
    {
        return None;
    }
    let protocol_label = PrivacyProtocolIdV1::IrohaZkX509StarkP256V0.canonical_label();
    let protocol_label_bytes = protocol_label.as_bytes();
    let protocol_label_length = u16::try_from(protocol_label_bytes.len()).ok()?;
    let mut digest = Sha256::new();
    digest.update(PRIVACY_ZK_X509_WORKER_RELEASE_EVIDENCE_DOMAIN_V1);
    digest.update(protocol_label_length.to_be_bytes());
    digest.update(protocol_label_bytes);
    digest.update([PRIVACY_ZK_X509_WORKER_PROTOCOL_VERSION_V1]);
    digest.update([PRIVACY_ZK_X509_WORKER_PUBLIC_REQUEST_VERSION_V1]);
    digest.update(protocol_profile_sha256);
    digest.update(kat_proof_bytes.to_be_bytes());
    digest.update(kat_proof_sha256);
    digest.update(expectations_norito_sha256);
    digest.update(expectations_json_sha256);
    digest.update(soundness_certificate_sha256);
    digest.update(resource_certificate_sha256);
    Some(digest.finalize().into())
}

/// Return the exact protocol and release-evidence pins compiled into the worker.
///
/// The candidate profile is always hashed independently of activation
/// readiness. If the production profile is available, its snapshot must be
/// byte-identical to that candidate. Missing KAT, expectation, or resource
/// evidence is represented by zero source fields and a `None` aggregate, never
/// by a synthesized digest.
///
/// # Errors
///
/// Returns a stable worker error if the candidate profile cannot be derived or
/// encoded, or if a supposedly available production profile differs from it.
pub fn privacy_zk_x509_worker_release_pins_v1()
-> Result<PrivacyZkX509WorkerReleasePinsV1, PrivacyZkX509WorkerErrorV1> {
    let candidate = zk_x509_release_candidate_profile_material_v1()
        .map_err(|_| PrivacyZkX509WorkerErrorV1::ProfileUnavailable)?;
    let protocol_profile_sha256 = compiled_profile_sha256_v1(candidate)?;
    let compiled_profile_sha256 =
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkX509StarkP256V0)
            .ok()
            .map(compiled_profile_sha256_v1)
            .transpose()?;
    if compiled_profile_sha256.is_some_and(|digest| digest != protocol_profile_sha256) {
        return Err(PrivacyZkX509WorkerErrorV1::ProfileUnavailable);
    }
    let release_evidence_sha256 = release_evidence_sha256_v1(
        protocol_profile_sha256,
        ZK_X509_RELEASE_KAT_EXPECTED_PROOF_BYTES_V1,
        ZK_X509_RELEASE_KAT_EXPECTED_PROOF_SHA256_V1,
        ZK_X509_NATIVE_RELEASE_EXPECTATIONS_NORITO_SHA256_V1,
        ZK_X509_NATIVE_RELEASE_EXPECTATIONS_JSON_SHA256_V1,
        ZK_X509_SOUNDNESS_CERTIFICATE_SHA256_V1,
        ZK_X509_RESOURCE_CERTIFICATE_SHA256_V1,
    );
    Ok(PrivacyZkX509WorkerReleasePinsV1 {
        protocol_profile_sha256,
        compiled_profile_sha256,
        kat_proof_bytes: ZK_X509_RELEASE_KAT_EXPECTED_PROOF_BYTES_V1,
        kat_proof_sha256: ZK_X509_RELEASE_KAT_EXPECTED_PROOF_SHA256_V1,
        expectations_norito_sha256: ZK_X509_NATIVE_RELEASE_EXPECTATIONS_NORITO_SHA256_V1,
        expectations_json_sha256: ZK_X509_NATIVE_RELEASE_EXPECTATIONS_JSON_SHA256_V1,
        soundness_certificate_sha256: ZK_X509_SOUNDNESS_CERTIFICATE_SHA256_V1,
        resource_certificate_sha256: ZK_X509_RESOURCE_CERTIFICATE_SHA256_V1,
        release_evidence_sha256,
    })
}

/// Public state and transaction plan for one isolated X.509 proof.
///
/// The statement must contain an all-zero transaction-intent digest.  The
/// worker derives and installs the final digest only after it has reconstructed
/// the exact signature-bound payload.  No certificate, CRL DER, accumulator
/// path, disclosure opening, or signer seed belongs in this value.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct PrivacyZkX509WorkerPublicRequestV1 {
    /// Exact schema version.
    pub schema_version: u8,
    /// Genesis-derived transaction security domain.
    pub network_id: NetworkId,
    /// Direct single-key transaction authority.
    pub authority: AccountId,
    /// Canonical transaction creation time in milliseconds.
    pub creation_time_millis: u64,
    /// Optional canonical transaction lifetime in milliseconds.
    pub time_to_live_millis: Option<u64>,
    /// Optional nonzero transaction nonce.
    pub nonce: Option<u32>,
    /// Exact signature-bound fee intent.
    pub fee_payment: FeePaymentIntent,
    /// Exact signature-bound metadata.
    pub metadata: Metadata,
    /// Consensus hash of the canonical genesis header.
    pub canonical_genesis_hash: [u8; 32],
    /// Finalized block timestamp used by the closed presentation window.
    pub trusted_block_timestamp_ms: u64,
    /// Complete public statement with an unresolved all-zero intent digest.
    pub statement_draft: IrohaZkX509StarkP256StatementV1,
    /// Finalized active trust-anchor revision.
    pub trust_anchor: PrivacyZkX509TrustAnchorRecordV1,
    /// Finalized active certificate-policy revision.
    pub certificate_policy: PrivacyZkX509CertificatePolicyRecordV1,
    /// Finalized active signed-CRL revision.
    pub crl_record: PrivacyZkX509CrlRecordV1,
    /// Finalized current CA-membership root epoch.
    pub ca_membership_root_epoch: u64,
    /// Finalized current CA-membership root.
    pub ca_membership_root: PrivacyRootV1,
}

/// Public result emitted after proof construction and signing.
#[derive(Clone, Debug)]
pub struct PrivacyZkX509WorkerActionV1 {
    /// Ordinary production transaction ready for authenticated Torii submit.
    pub signed_transaction: SignedTransaction,
    /// Exact intent-bound statement contained by the transaction.
    pub statement: IrohaZkX509StarkP256StatementV1,
    /// Hash of the canonical X5S1 proof bytes.
    pub proof_sha256: [u8; 32],
}

/// Stable, non-secret error classes returned by the isolated worker core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyZkX509WorkerErrorV1 {
    /// Public request or finalized governance state is not canonical.
    #[error("zk-X509 worker public request failed at {0}")]
    InvalidPublicRequest(&'static str),
    /// Production profile is not fully pinned and available.
    #[error("zk-X509 worker production profile is unavailable")]
    ProfileUnavailable,
    /// Owner-only signer seed is invalid or does not control the authority.
    #[error("zk-X509 worker signer custody failed")]
    SignerCustody,
    /// Private witness failed exact decoding or the native reference relation.
    #[error("zk-X509 worker witness preparation failed")]
    WitnessPreparation,
    /// Native proof construction or its independent self-check failed.
    #[error("zk-X509 worker proof construction failed")]
    ProofConstruction,
    /// Final proof envelope or transaction could not be encoded canonically.
    #[error("zk-X509 worker transaction finalization failed")]
    TransactionFinalization,
}

fn transaction_payload_v1(
    request: &PrivacyZkX509WorkerPublicRequestV1,
    envelope: PrivacyProofEnvelopeV1,
) -> Result<TransactionPayload, PrivacyZkX509WorkerErrorV1> {
    let mut builder = TransactionBuilder::new(
        request.network_id,
        request.authority.clone(),
        request.fee_payment.clone(),
    )
    .with_instructions([SubmitPrivacyProofV1::new(envelope)])
    .with_metadata(request.metadata.clone());
    builder.set_creation_time(Duration::from_millis(request.creation_time_millis));
    if let Some(ttl) = request.time_to_live_millis {
        builder.set_ttl(Duration::from_millis(ttl));
    }
    if let Some(nonce) = request.nonce {
        builder.set_nonce(
            NonZeroU32::new(nonce)
                .ok_or(PrivacyZkX509WorkerErrorV1::InvalidPublicRequest("nonce"))?,
        );
    }
    builder
        .into_payload()
        .map_err(|_| PrivacyZkX509WorkerErrorV1::InvalidPublicRequest("transaction-context"))
}

fn draft_envelope_v1(
    profile: CompiledPrivacyProfileV1,
    statement: IrohaZkX509StarkP256StatementV1,
) -> PrivacyProofEnvelopeV1 {
    PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest: PrivacyStatementDigestV1::new([0; 32]),
        statement: PrivacyStatementV1::IrohaZkX509StarkP256V0(statement),
        proof: PrivacyProofV1::IrohaZkX509StarkP256V0(PrivacyProofBytesV1::new(Vec::new())),
    }
}

fn validate_draft_v1(
    request: &PrivacyZkX509WorkerPublicRequestV1,
    profile: CompiledPrivacyProfileV1,
) -> Result<(), PrivacyZkX509WorkerErrorV1> {
    if request.schema_version != PRIVACY_ZK_X509_WORKER_PUBLIC_REQUEST_VERSION_V1 {
        return Err(PrivacyZkX509WorkerErrorV1::InvalidPublicRequest(
            "schema-version",
        ));
    }
    if request.canonical_genesis_hash == [0; 32]
        || request.network_id.as_bytes() != &request.canonical_genesis_hash
        || request.trusted_block_timestamp_ms == 0
        || request.creation_time_millis == 0
        || request.time_to_live_millis == Some(0)
        || request.nonce == Some(0)
    {
        return Err(PrivacyZkX509WorkerErrorV1::InvalidPublicRequest(
            "transaction-domain",
        ));
    }
    if request.statement_draft.wallet_account != request.authority
        || request.statement_draft.context.network_id != request.network_id
        || request.statement_draft.context.action_index != PRIVACY_ZK_X509_WORKER_ACTION_INDEX_V1
        || !request
            .statement_draft
            .context
            .transaction_intent_digest
            .is_zero()
        || request.statement_draft.context.parameter_id != profile.parameter_id
        || request.statement_draft.context.parameter_digest != profile.parameter_digest
        || request.statement_draft.context.verifier_digest != profile.verifier_digest
        || request.statement_draft.context.statement_schema_digest
            != profile.statement_schema_digest
        || request.statement_draft.context.engine_manifest_digest != profile.engine_manifest_digest
    {
        return Err(PrivacyZkX509WorkerErrorV1::InvalidPublicRequest(
            "statement-context",
        ));
    }
    // Structural validation requires the derived intent to be nonzero.  This
    // temporary value is never proved or emitted; it validates every other
    // public field before any witness byte is decoded.
    let mut structural = request.statement_draft.clone();
    structural.context.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new([1; 32]);
    PrivacyStatementV1::IrohaZkX509StarkP256V0(structural)
        .validate(&PrivacyConsensusLimitsV1::taira_default())
        .map_err(|_| PrivacyZkX509WorkerErrorV1::InvalidPublicRequest("statement"))
}

/// Validate one owner bundle against its exact public request without proving or signing.
///
/// This admission path is intended for an authenticated isolated worker. It validates the
/// production profile, governed public state, signer custody, canonical transaction intent, and
/// complete private reference relation, but deliberately performs no proof construction and emits
/// no secret-derived value.
///
/// # Errors
///
/// Returns the same stable public-request, profile, custody, and witness classes used by the
/// one-shot proof-and-sign operation.
pub fn validate_privacy_zk_x509_worker_inputs_v1(
    request: &PrivacyZkX509WorkerPublicRequestV1,
    signer_seed: &[u8; 32],
    encoded_witness: &[u8],
) -> Result<(), PrivacyZkX509WorkerErrorV1> {
    let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkX509StarkP256V0)
        .map_err(|_| PrivacyZkX509WorkerErrorV1::ProfileUnavailable)?;
    validate_draft_v1(request, profile)?;

    let state = assemble_privacy_zk_x509_authoritative_state_for_worker_v1(
        request.trust_anchor,
        request.certificate_policy.clone(),
        request.crl_record,
        request.ca_membership_root_epoch,
        request.ca_membership_root,
    )
    .map_err(|_| PrivacyZkX509WorkerErrorV1::InvalidPublicRequest("governance-state"))?;

    let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, signer_seed)
        .map_err(|_| PrivacyZkX509WorkerErrorV1::SignerCustody)?;
    let public_key = PublicKey::from(private_key);
    if request.authority.try_signatory() != Some(&public_key) {
        return Err(PrivacyZkX509WorkerErrorV1::SignerCustody);
    }

    let intent = transaction_payload_v1(
        request,
        draft_envelope_v1(profile, request.statement_draft.clone()),
    )?
    .privacy_transaction_intent_digest_v1()
    .map_err(|_| PrivacyZkX509WorkerErrorV1::InvalidPublicRequest("transaction-intent"))?;
    if intent.is_zero() {
        return Err(PrivacyZkX509WorkerErrorV1::InvalidPublicRequest(
            "transaction-intent",
        ));
    }
    let mut statement = request.statement_draft.clone();
    statement.context.transaction_intent_digest = intent;
    validate_privacy_zk_x509_statement_state_v1(
        &statement,
        &state,
        request.trusted_block_timestamp_ms,
        &PrivacyConsensusLimitsV1::taira_default(),
    )
    .map_err(|_| PrivacyZkX509WorkerErrorV1::InvalidPublicRequest("statement-state"))?;
    prepare_zk_x509_prover_input_v1(
        &statement,
        &state,
        request.trusted_block_timestamp_ms,
        &PrivacyConsensusLimitsV1::taira_default(),
        encoded_witness,
    )
    .map_err(|_| PrivacyZkX509WorkerErrorV1::WitnessPreparation)?;
    Ok(())
}

/// Build and sign one production-profile X.509 action without releasing any
/// secret material from the native worker process.
///
/// `signer_seed` and `encoded_witness` are borrowed from zeroizing owner-only
/// buffers.  The worker process must erase those buffers after this call on
/// every success or failure path.
pub fn build_signed_privacy_zk_x509_worker_action_v1(
    request: PrivacyZkX509WorkerPublicRequestV1,
    signer_seed: &[u8; 32],
    encoded_witness: &[u8],
) -> Result<PrivacyZkX509WorkerActionV1, PrivacyZkX509WorkerErrorV1> {
    // This is intentionally the first cryptographic gate.  Candidate profile
    // material can be inspected offline but can never reach signing here.
    let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkX509StarkP256V0)
        .map_err(|_| PrivacyZkX509WorkerErrorV1::ProfileUnavailable)?;
    validate_draft_v1(&request, profile)?;

    let state = assemble_privacy_zk_x509_authoritative_state_for_worker_v1(
        request.trust_anchor,
        request.certificate_policy.clone(),
        request.crl_record,
        request.ca_membership_root_epoch,
        request.ca_membership_root,
    )
    .map_err(|_| PrivacyZkX509WorkerErrorV1::InvalidPublicRequest("governance-state"))?;

    let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, signer_seed)
        .map_err(|_| PrivacyZkX509WorkerErrorV1::SignerCustody)?;
    let public_key = PublicKey::from(private_key.clone());
    if request.authority.try_signatory() != Some(&public_key) {
        return Err(PrivacyZkX509WorkerErrorV1::SignerCustody);
    }

    let intent = transaction_payload_v1(
        &request,
        draft_envelope_v1(profile, request.statement_draft.clone()),
    )?
    .privacy_transaction_intent_digest_v1()
    .map_err(|_| PrivacyZkX509WorkerErrorV1::InvalidPublicRequest("transaction-intent"))?;
    if intent.is_zero() {
        return Err(PrivacyZkX509WorkerErrorV1::InvalidPublicRequest(
            "transaction-intent",
        ));
    }
    let mut statement = request.statement_draft.clone();
    statement.context.transaction_intent_digest = intent;
    validate_privacy_zk_x509_statement_state_v1(
        &statement,
        &state,
        request.trusted_block_timestamp_ms,
        &PrivacyConsensusLimitsV1::taira_default(),
    )
    .map_err(|_| PrivacyZkX509WorkerErrorV1::InvalidPublicRequest("statement-state"))?;

    let proof = prove_zk_x509_credential_proof_v1_with_rng(
        &statement,
        &state,
        request.trusted_block_timestamp_ms,
        &PrivacyConsensusLimitsV1::taira_default(),
        request.canonical_genesis_hash,
        encoded_witness,
        &mut rand::rngs::OsRng,
    )
    .map_err(|error| {
        use crate::privacy_engines::zk_x509::engine::ZkX509EngineErrorV1;
        match error {
            ZkX509EngineErrorV1::WitnessCodec(_)
            | ZkX509EngineErrorV1::WitnessRoundTripMismatch
            | ZkX509EngineErrorV1::ReferenceRelation(_)
            | ZkX509EngineErrorV1::InvalidAuthoritativeState(_) => {
                PrivacyZkX509WorkerErrorV1::WitnessPreparation
            }
            _ => PrivacyZkX509WorkerErrorV1::ProofConstruction,
        }
    })?;
    verify_zk_x509_credential_proof_v1(&statement, &state, request.canonical_genesis_hash, &proof)
        .map_err(|_| PrivacyZkX509WorkerErrorV1::ProofConstruction)?;

    let typed_statement = PrivacyStatementV1::IrohaZkX509StarkP256V0(statement.clone());
    let statement_digest = typed_statement
        .digest()
        .map_err(|_| PrivacyZkX509WorkerErrorV1::TransactionFinalization)?;
    let proof_sha256 = Sha256::digest(&proof).into();
    let envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest,
        statement: typed_statement,
        proof: PrivacyProofV1::IrohaZkX509StarkP256V0(PrivacyProofBytesV1::new(proof)),
    };
    envelope
        .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
        .map_err(|_| PrivacyZkX509WorkerErrorV1::TransactionFinalization)?;
    let payload = transaction_payload_v1(&request, envelope)
        .map_err(|_| PrivacyZkX509WorkerErrorV1::TransactionFinalization)?;
    let final_intent = payload
        .validate_privacy_transaction_intent_binding_v1()
        .map_err(|_| PrivacyZkX509WorkerErrorV1::TransactionFinalization)?;
    if final_intent != intent {
        return Err(PrivacyZkX509WorkerErrorV1::TransactionFinalization);
    }
    let signed_transaction = TransactionBuilder::from_payload(payload)
        .map_err(|_| PrivacyZkX509WorkerErrorV1::TransactionFinalization)?
        .try_sign(&private_key)
        .map_err(|_| PrivacyZkX509WorkerErrorV1::TransactionFinalization)?;
    if signed_transaction
        .privacy_transaction_intent_digest_v1()
        .map_err(|_| PrivacyZkX509WorkerErrorV1::TransactionFinalization)?
        != intent
        || signed_transaction.encode_versioned().len() > TAIRA_PRIVACY_MAX_ACTION_BYTES_V1 as usize
    {
        return Err(PrivacyZkX509WorkerErrorV1::TransactionFinalization);
    }
    Ok(PrivacyZkX509WorkerActionV1 {
        signed_transaction,
        statement,
        proof_sha256,
    })
}
