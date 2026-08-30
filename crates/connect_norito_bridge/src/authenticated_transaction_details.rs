//! Native construction and verification for the authenticated transaction-details mobile flow.
//!
//! Managed callers are deliberately limited to an opaque signing callback. Native code
//! constructs the complete `QueryRequestWithAuthority`, returns only its 32-byte signing digest,
//! verifies the detached signature, and emits the canonical versioned `SignedQuery` body. The
//! response projection accepts only an exact canonical committed result for the same external
//! transaction and authority, with a specialized rejection projection retained for callers that
//! require its typed rejection code. Those legacy projections verify the external transaction
//! signature and every binding available in the endpoint payload, but the endpoint alone has no
//! signed block header or finality certificate. The additive finalized Kagemusha projector below
//! therefore binds that exact response to a caller-pinned checkpoint, a bounded sequence of native
//! `BridgeFinalityProof`s, and the exact canonical executed-block wire before exposing either
//! `Applied` or `Rejected` as independently verifiable terminal evidence.

use iroha_crypto::{Hash, HashOf, SignatureOf};
use iroha_data_model::{
    NetworkId, ValidationFail,
    account::AccountId,
    block::{
        SignedBlock,
        consensus_v2::{HeightContext, HeightContextId},
        decode_versioned_signed_block,
        proofs::{AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1, TrustedBlockProofAnchor},
    },
    bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
    isi::{
        Instruction as _,
        error::InstructionExecutionError,
        offline::{
            RedeemKagemushaRecursiveV4, RegisterOfflineDeviceAttestation, TopUpKagemushaRecursiveV4,
        },
    },
    offline::{
        KagemushaRecursiveSpendRedeemRequestV4, KagemushaRecursiveSpendTopUpRequestV4,
        OfflineDeviceEligibilityDecisionV1, OfflineDeviceEligibilityOutcomeV1,
    },
    privacy::{PrivacyLedgerEffectKindV1, PrivacyOperationSchemaV1, PrivacyProtocolIdV1},
    query::{
        CommittedTransaction, CommittedTxFilters, Query as _, QueryItemKind, QueryRequest,
        QueryRequestWithAuthority, QuerySignature, QueryWithParams, SignedQuery,
        dsl::{CompoundPredicate, SelectorTuple},
        parameters::QueryParams,
        transaction::prelude::FindTransactions,
    },
    transaction::{Executable, TransactionEntrypoint, error::TransactionRejectionReason},
};
use iroha_torii_shared::PipelineTransactionDetailsResponse;
use iroha_version::codec::EncodeVersioned as _;
use norito::{NoritoDeserialize, NoritoSerialize, codec::Encode as _};
use std::{error::Error as _, num::NonZeroU64, str::FromStr as _};

use super::{
    connect_signature_from_algorithm_bytes, network_id_from_raw_bytes,
    privacy_exact12_action::{
        inspect_signed_privacy_exact12_action_v1,
        operation_from_index as exact12_operation_from_index,
    },
};

pub(super) const AUTHENTICATED_TRANSACTION_DETAILS_PREPARATION_MAX_BYTES_V1: usize = 64 * 1024;
pub(super) const AUTHENTICATED_TRANSACTION_DETAILS_RESPONSE_MAX_BYTES_V1: usize = 64 * 1024 * 1024;
pub(super) const AUTHENTICATED_TRANSACTION_DETAILS_SIGNATURE_MAX_BYTES_V1: usize = 16 * 1024;
pub(super) const AUTHENTICATED_OFFLINE_DEVICE_REGISTRATION_RESULT_MAX_BYTES_V1: usize = 128 * 1024;
const AUTHENTICATED_TRANSACTION_DETAILS_SIGNED_QUERY_MAX_BYTES_V1: usize = 64 * 1024;
pub(super) const AUTHENTICATED_TRANSACTION_DETAILS_QUERY_TTL_MS_V1: u64 = 100_000;
const AUTHENTICATED_TRANSACTION_DETAILS_PREPARATION_VERSION_V1: u8 = 1;
const AUTHENTICATED_TRANSACTION_DETAILS_PREPARATION_VERSION_V2: u8 = 2;
pub(super) const AUTHENTICATED_TRANSACTION_DETAILS_AUTHORITY_MAX_BYTES_V1: usize =
    2 * iroha_data_model::musubi::MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1;
const AUTHENTICATED_TRANSACTION_DETAILS_REJECTION_CODE_MAX_BYTES_V1: usize = 128;
const AUTHENTICATED_TRANSACTION_DETAILS_REJECTION_MESSAGE_MAX_BYTES_V1: usize = 1_024;
pub(super) const AUTHENTICATED_FINALITY_PAGE_MAX_PROOFS_V1: usize = 64;
pub(super) const AUTHENTICATED_FINALITY_PROOF_MAX_BYTES_V1: usize = 9 * 1024 * 1024;
pub(super) const AUTHENTICATED_FINALITY_PAGE_MAX_BYTES_V1: usize = 64 * 1024 * 1024;
pub(super) const AUTHENTICATED_FINALITY_CHECKPOINT_BYTES_V1: usize = 8 + Hash::LENGTH;
pub(super) const AUTHENTICATED_FINALITY_MOBILE_MAX_HEIGHT_V1: u64 = i64::MAX as u64;
const AUTHENTICATED_FINALITY_PAGE_VERSION_V1: u8 = 1;
const AUTHENTICATED_FINALIZED_KAGEMUSHA_EVIDENCE_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:finalized-outcome:v1\0";
const AUTHENTICATED_FINALIZED_PRIVACY_ACTION_REJECTION_EVIDENCE_DOMAIN_V1: &[u8] =
    b"iroha:privacy:finalized-action-rejection:v1\0";
pub(super) const AUTHENTICATED_FINALIZED_PRIVACY_ACTION_BINDING_BYTES_V1: usize = 3 * Hash::LENGTH;

#[derive(NoritoSerialize, NoritoDeserialize)]
struct AuthenticatedTransactionDetailsPreparationV1 {
    version: u8,
    expected_entrypoint_hash: HashOf<TransactionEntrypoint>,
    authority_literal: String,
    payload: QueryRequestWithAuthority,
}

/// Additive authority-split preparation. The signed query authority is allowed to inspect a
/// transaction issued by a separately pinned transaction authority; both identities remain
/// private native bindings and neither is inferred from the Torii response.
#[derive(NoritoSerialize, NoritoDeserialize)]
struct AuthenticatedTransactionDetailsPreparationV2 {
    version: u8,
    expected_entrypoint_hash: HashOf<TransactionEntrypoint>,
    query_authority_literal: String,
    expected_transaction_authority_literal: String,
    payload: QueryRequestWithAuthority,
}

/// Canonical content-addressed carrier for one bounded contiguous finality page.
#[derive(NoritoSerialize, NoritoDeserialize)]
struct AuthenticatedFinalityProofPageV1 {
    version: u8,
    proofs: Vec<BridgeFinalityProof>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AuthenticatedFinalityProofPageProjectionV1 {
    pub archive: Vec<u8>,
    pub hash_hex: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AuthenticatedFinalityCheckpointProjectionV1 {
    pub height: u64,
    pub context_id: [u8; Hash::LENGTH],
}

impl AuthenticatedFinalityCheckpointProjectionV1 {
    pub(super) fn encode(
        self,
    ) -> Result<[u8; AUTHENTICATED_FINALITY_CHECKPOINT_BYTES_V1], &'static str> {
        authenticated_finality_mobile_height_v1(self.height)?;
        let mut encoded = [0_u8; AUTHENTICATED_FINALITY_CHECKPOINT_BYTES_V1];
        encoded[..8].copy_from_slice(&self.height.to_be_bytes());
        encoded[8..].copy_from_slice(&self.context_id);
        Ok(encoded)
    }
}

/// Keep every height crossing the ABI-22 Java/Kotlin boundary inside the positive signed-mobile
/// domain. The ledger uses `u64`, but both mobile namespaces deliberately expose `long` and the
/// checkpoint encoding is therefore closed to `1..=Long.MAX_VALUE`.
pub(super) fn authenticated_finality_mobile_height_v1(height: u64) -> Result<u64, &'static str> {
    if height == 0 || height > AUTHENTICATED_FINALITY_MOBILE_MAX_HEIGHT_V1 {
        return Err("finality height must fit the positive signed-mobile long domain");
    }
    Ok(height)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AuthenticatedFinalizedKagemushaTerminalStateV1 {
    Applied,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AuthenticatedFinalizedKagemushaOutcomeProjectionV1 {
    pub terminal_state: AuthenticatedFinalizedKagemushaTerminalStateV1,
    pub operation_id: [u8; 32],
    pub operation_kind: String,
    pub transaction_hash_hex: String,
    pub query_authority: String,
    pub transaction_authority: String,
    pub block_hash_hex: String,
    pub result_hash_hex: String,
    pub committed_block_height: u64,
    pub finalized_checkpoint: AuthenticatedFinalityCheckpointProjectionV1,
    pub executed_block_wire_hash_hex: String,
    pub rejection_code: Option<String>,
    pub rejection_message: Option<String>,
    pub evidence_id_hex: String,
    pub transaction_details_hash_hex: String,
    pub finality_page_hash_hex: String,
}

/// Closed, stable classification of a committed Exact12 transaction rejection.
///
/// The human-readable message remains evidence, but mobile callers must branch only on this
/// exhaustive union. New ledger rejection families require a new ABI projection version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AuthenticatedPrivacyActionRejectionCodeV1 {
    AccountDoesNotExist,
    LimitCheck,
    Validation,
    InstructionExecution,
    IvmExecution,
    TriggerExecution,
}

impl AuthenticatedPrivacyActionRejectionCodeV1 {
    pub(super) const fn canonical_label(self) -> &'static str {
        match self {
            Self::AccountDoesNotExist => "account_does_not_exist",
            Self::LimitCheck => "limit_check",
            Self::Validation => "validation",
            Self::InstructionExecution => "instruction_execution",
            Self::IvmExecution => "ivm_execution",
            Self::TriggerExecution => "trigger_execution",
        }
    }
}

/// Independently finalized rejection for one exact signed Exact12 action.
///
/// Unlike the legacy committed-details projection, this value authenticates the exact executed
/// block and its entry/result inclusion through a caller-pinned Sumeragi-v2 checkpoint and a
/// native-verified contiguous QC chain. It carries the same action bindings as the applied typed
/// receipt, while deliberately omitting execution-manifest fields because rejected execution did
/// not atomically persist a native receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AuthenticatedFinalizedPrivacyActionRejectionProjectionV1 {
    pub network_id_hex: String,
    pub protocol_id: PrivacyProtocolIdV1,
    pub operation_schema: PrivacyOperationSchemaV1,
    pub ledger_effect_kind: PrivacyLedgerEffectKindV1,
    pub transaction_hash_hex: String,
    pub action_index: u32,
    pub transaction_intent_digest_hex: String,
    pub statement_digest_hex: String,
    pub proof_envelope_hash_hex: String,
    pub query_authority: String,
    pub transaction_authority: String,
    pub block_hash_hex: String,
    pub result_hash_hex: String,
    pub rejection_code: AuthenticatedPrivacyActionRejectionCodeV1,
    pub rejection_message: String,
    pub committed_block_height: u64,
    pub finalized_checkpoint: AuthenticatedFinalityCheckpointProjectionV1,
    pub executed_block_wire_hash_hex: String,
    pub evidence_id_hex: String,
    pub transaction_details_hash_hex: String,
    pub finality_page_hash_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AuthenticatedCommittedRejectionProjectionV1 {
    pub transaction_hash_hex: String,
    pub transaction_authority: String,
    pub block_hash_hex: String,
    pub result_hash_hex: String,
    pub rejection_code: &'static str,
    pub rejection_message: String,
    pub committed_block_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AuthenticatedCommittedTransactionResultProjectionV1 {
    pub transaction_hash_hex: String,
    pub transaction_authority: String,
    pub block_hash_hex: String,
    pub result_hash_hex: String,
    pub result_ok: bool,
    pub rejection_message: Option<String>,
    pub committed_block_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AuthenticatedCommittedRejectionProjectionV2 {
    pub transaction_hash_hex: String,
    pub query_authority: String,
    pub transaction_authority: String,
    pub block_hash_hex: String,
    pub result_hash_hex: String,
    pub rejection_code: &'static str,
    pub rejection_message: String,
    pub committed_block_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AuthenticatedCommittedTransactionResultProjectionV2 {
    pub transaction_hash_hex: String,
    pub query_authority: String,
    pub transaction_authority: String,
    pub block_hash_hex: String,
    pub result_hash_hex: String,
    pub result_ok: bool,
    pub rejection_message: Option<String>,
    pub committed_block_height: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfflineDeviceRegistrationTerminalStateV1 {
    Applied,
    EligibilityRejected,
    OtherRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AuthenticatedOfflineDeviceRegistrationResultProjectionV1 {
    pub transaction_hash_hex: String,
    pub transaction_authority: String,
    pub block_hash_hex: String,
    pub result_hash_hex: String,
    pub committed_block_height: u64,
    pub terminal_state: OfflineDeviceRegistrationTerminalStateV1,
    pub eligibility_decision: Option<OfflineDeviceEligibilityDecisionV1>,
    pub rejection_code: Option<String>,
    pub rejection_message: Option<String>,
}

fn canonical_lower_hash_of_entrypoint(
    transaction_hash_hex: &str,
) -> Result<HashOf<TransactionEntrypoint>, &'static str> {
    if transaction_hash_hex.len() != Hash::LENGTH * 2
        || !transaction_hash_hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("transactionHashHex must be exactly 64 lowercase hexadecimal characters");
    }
    let hash = transaction_hash_hex
        .parse::<Hash>()
        .map_err(|_| "transactionHashHex must be a canonical marked Iroha hash")?;
    Ok(HashOf::from_untyped_unchecked(hash))
}

pub(super) fn canonical_authority(authority_literal: &str) -> Result<AccountId, &'static str> {
    if authority_literal.is_empty()
        || authority_literal.len() > AUTHENTICATED_TRANSACTION_DETAILS_AUTHORITY_MAX_BYTES_V1
        || authority_literal.trim() != authority_literal
    {
        return Err("authorityAccountId must be bounded exact canonical I105");
    }
    let parsed = AccountId::parse_encoded(authority_literal)
        .map_err(|_| "authorityAccountId must be canonical I105")?;
    if parsed.canonical() != authority_literal {
        return Err("authorityAccountId must use its exact canonical I105 representation");
    }
    let account = parsed.into_account_id();
    if account.try_signatory().is_none() {
        return Err("authorityAccountId must be a single-key account");
    }
    Ok(account)
}

#[cfg(test)]
mod authority_tests {
    use super::canonical_authority;

    const CANONICAL_I105: &str = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";

    #[test]
    fn shared_authority_parser_accepts_exact_unicode_i105_only() {
        canonical_authority(CANONICAL_I105).expect("canonical I105 authority must parse");
        assert!(canonical_authority(&format!(" {CANONICAL_I105}")).is_err());
        assert!(canonical_authority("alice@wonderland").is_err());
    }
}

fn decode_exact_codec<T>(bytes: &[u8]) -> Option<T>
where
    T: norito::codec::Decode + norito::codec::Encode,
{
    let mut cursor = bytes;
    let value = T::decode(&mut cursor).ok()?;
    (cursor.is_empty() && value.encode() == bytes).then_some(value)
}

fn exact_query_hash(payload: &QueryRequestWithAuthority) -> Option<HashOf<TransactionEntrypoint>> {
    let QueryRequest::Start(query) = payload.request() else {
        return None;
    };
    if query.params() != &QueryParams::default() {
        return None;
    }
    let (item_kind, predicate_bytes, selector_bytes, query_payload) = query.parts();
    let expected_query = FindTransactions::new();
    if item_kind != QueryItemKind::CommittedTransaction
        || query_payload != expected_query.dyn_encode()
    {
        return None;
    }
    let predicate = decode_exact_codec::<CompoundPredicate<CommittedTransaction>>(predicate_bytes)?;
    let selector = decode_exact_codec::<SelectorTuple<CommittedTransaction>>(selector_bytes)?;
    if selector != SelectorTuple::<CommittedTransaction>::default() {
        return None;
    }
    let filters = predicate.committed_tx_filters()?;
    let entry_eq = filters.entry_eq?;
    (filters
        == CommittedTxFilters {
            entry_eq: Some(entry_eq),
            ..CommittedTxFilters::default()
        })
    .then_some(entry_eq)
}

fn exact_query(entrypoint_hash: HashOf<TransactionEntrypoint>) -> QueryWithParams {
    let query = FindTransactions::new();
    QueryWithParams {
        query: (),
        query_payload: query.dyn_encode(),
        item: query.query_item_kind(),
        predicate_bytes: CompoundPredicate::from_filters(CommittedTxFilters {
            entry_eq: Some(entrypoint_hash),
            ..CommittedTxFilters::default()
        })
        .encode(),
        selector_bytes: SelectorTuple::<CommittedTransaction>::default().encode(),
        params: QueryParams::default(),
    }
}

fn validate_preparation(
    preparation: &AuthenticatedTransactionDetailsPreparationV1,
) -> Result<(), &'static str> {
    if preparation.version != AUTHENTICATED_TRANSACTION_DETAILS_PREPARATION_VERSION_V1 {
        return Err("unsupported authenticated transaction-details preparation version");
    }
    let authority = canonical_authority(&preparation.authority_literal)?;
    if preparation.payload.authority() != &authority
        || preparation.payload.creation_time_ms() == 0
        || preparation.payload.time_to_live_ms().get()
            != AUTHENTICATED_TRANSACTION_DETAILS_QUERY_TTL_MS_V1
        || preparation.payload.nonce() == &[0; 32]
        || exact_query_hash(&preparation.payload).as_ref()
            != Some(&preparation.expected_entrypoint_hash)
    {
        return Err("authenticated transaction-details preparation binding is invalid");
    }
    Ok(())
}

fn decode_preparation(
    preparation: &[u8],
) -> Result<AuthenticatedTransactionDetailsPreparationV1, &'static str> {
    if preparation.is_empty()
        || preparation.len() > AUTHENTICATED_TRANSACTION_DETAILS_PREPARATION_MAX_BYTES_V1
    {
        return Err("preparation archive is outside its closed byte bound");
    }
    let decoded =
        norito::decode_canonical::<AuthenticatedTransactionDetailsPreparationV1>(preparation)
            .map_err(|_| "preparation archive is not canonical Norito")?;
    validate_preparation(&decoded)?;
    Ok(decoded)
}

fn validate_preparation_v2(
    preparation: &AuthenticatedTransactionDetailsPreparationV2,
) -> Result<(), &'static str> {
    if preparation.version != AUTHENTICATED_TRANSACTION_DETAILS_PREPARATION_VERSION_V2 {
        return Err("unsupported authenticated transaction-details V2 preparation version");
    }
    let query_authority = canonical_authority(&preparation.query_authority_literal)?;
    // Parse independently even when both literals are equal. This prevents a malformed or
    // non-single-key expected transaction authority from being smuggled through the query signer.
    canonical_authority(&preparation.expected_transaction_authority_literal)?;
    if preparation.payload.authority() != &query_authority
        || preparation.payload.creation_time_ms() == 0
        || preparation.payload.time_to_live_ms().get()
            != AUTHENTICATED_TRANSACTION_DETAILS_QUERY_TTL_MS_V1
        || preparation.payload.nonce() == &[0; 32]
        || exact_query_hash(&preparation.payload).as_ref()
            != Some(&preparation.expected_entrypoint_hash)
    {
        return Err("authenticated transaction-details V2 preparation binding is invalid");
    }
    Ok(())
}

fn decode_preparation_v2(
    preparation: &[u8],
) -> Result<AuthenticatedTransactionDetailsPreparationV2, &'static str> {
    if preparation.is_empty()
        || preparation.len() > AUTHENTICATED_TRANSACTION_DETAILS_PREPARATION_MAX_BYTES_V1
    {
        return Err("V2 preparation archive is outside its closed byte bound");
    }
    let decoded =
        norito::decode_canonical::<AuthenticatedTransactionDetailsPreparationV2>(preparation)
            .map_err(|_| "V2 preparation archive is not canonical Norito")?;
    validate_preparation_v2(&decoded)?;
    Ok(decoded)
}

pub(super) fn authenticated_transaction_details_prepare_v2(
    network_id: &[u8],
    query_authority_literal: &str,
    expected_transaction_authority_literal: &str,
    transaction_hash_hex: &str,
    creation_time_ms: u64,
    nonce: [u8; 32],
) -> Result<(Vec<u8>, [u8; 32]), &'static str> {
    if creation_time_ms == 0 {
        return Err("creationTimeMs must be positive");
    }
    if nonce == [0; 32] {
        return Err("nonce must be exactly 32 nonzero random bytes");
    }
    let network_id: NetworkId = network_id_from_raw_bytes(network_id)?;
    let query_authority = canonical_authority(query_authority_literal)?;
    canonical_authority(expected_transaction_authority_literal)?;
    let expected_entrypoint_hash = canonical_lower_hash_of_entrypoint(transaction_hash_hex)?;
    let payload = QueryRequest::Start(exact_query(expected_entrypoint_hash)).with_authority(
        network_id,
        query_authority,
        creation_time_ms,
        NonZeroU64::new(AUTHENTICATED_TRANSACTION_DETAILS_QUERY_TTL_MS_V1)
            .expect("authenticated query TTL is nonzero"),
        nonce,
    );
    let preparation = AuthenticatedTransactionDetailsPreparationV2 {
        version: AUTHENTICATED_TRANSACTION_DETAILS_PREPARATION_VERSION_V2,
        expected_entrypoint_hash,
        query_authority_literal: query_authority_literal.to_owned(),
        expected_transaction_authority_literal: expected_transaction_authority_literal.to_owned(),
        payload,
    };
    validate_preparation_v2(&preparation)?;
    let digest = *HashOf::new(&preparation.payload).as_ref();
    let archive = norito::encode_canonical(&preparation)
        .map_err(|_| "failed to encode canonical transaction-details V2 preparation")?;
    if archive.len() > AUTHENTICATED_TRANSACTION_DETAILS_PREPARATION_MAX_BYTES_V1 {
        return Err("encoded V2 preparation archive exceeds its closed byte bound");
    }
    Ok((archive, digest))
}

pub(super) fn authenticated_transaction_details_finalize_v2(
    preparation: &[u8],
    signature_bytes: &[u8],
) -> Result<Vec<u8>, &'static str> {
    if signature_bytes.is_empty()
        || signature_bytes.len() > AUTHENTICATED_TRANSACTION_DETAILS_SIGNATURE_MAX_BYTES_V1
    {
        return Err("query signature is outside its closed byte bound");
    }
    let preparation = decode_preparation_v2(preparation)?;
    let signatory = preparation
        .payload
        .authority()
        .try_signatory()
        .ok_or("query authority must be single-key")?;
    let algorithm = signatory
        .try_algorithm()
        .map_err(|_| "query authority signature algorithm is invalid")?;
    let signature = connect_signature_from_algorithm_bytes(algorithm, signature_bytes)
        .ok_or("query signature material is malformed")?;
    let signature = SignatureOf::<QueryRequestWithAuthority>::from_signature(signature);
    signature
        .verify(signatory, &preparation.payload)
        .map_err(|_| "query signature does not authenticate the native V2 payload")?;
    let signed = SignedQuery {
        signature: QuerySignature(signature),
        payload: preparation.payload,
    };
    signed
        .verify_signature()
        .map_err(|_| "final signed V2 query failed native verification")?;
    let body = signed.encode_versioned();
    if body.is_empty() || body.len() > AUTHENTICATED_TRANSACTION_DETAILS_SIGNED_QUERY_MAX_BYTES_V1 {
        return Err("final signed V2 query violates its closed byte bound");
    }
    Ok(body)
}

pub(super) fn authenticated_transaction_details_prepare_v1(
    network_id: &[u8],
    authority_literal: &str,
    transaction_hash_hex: &str,
    creation_time_ms: u64,
    nonce: [u8; 32],
) -> Result<(Vec<u8>, [u8; 32]), &'static str> {
    if creation_time_ms == 0 {
        return Err("creationTimeMs must be positive");
    }
    if nonce == [0; 32] {
        return Err("nonce must be exactly 32 nonzero random bytes");
    }
    let network_id: NetworkId = network_id_from_raw_bytes(network_id)?;
    let authority = canonical_authority(authority_literal)?;
    let expected_entrypoint_hash = canonical_lower_hash_of_entrypoint(transaction_hash_hex)?;
    let payload = QueryRequest::Start(exact_query(expected_entrypoint_hash)).with_authority(
        network_id,
        authority,
        creation_time_ms,
        NonZeroU64::new(AUTHENTICATED_TRANSACTION_DETAILS_QUERY_TTL_MS_V1)
            .expect("authenticated query TTL is nonzero"),
        nonce,
    );
    let preparation = AuthenticatedTransactionDetailsPreparationV1 {
        version: AUTHENTICATED_TRANSACTION_DETAILS_PREPARATION_VERSION_V1,
        expected_entrypoint_hash,
        authority_literal: authority_literal.to_owned(),
        payload,
    };
    validate_preparation(&preparation)?;
    let digest = *HashOf::new(&preparation.payload).as_ref();
    let archive = norito::encode_canonical(&preparation)
        .map_err(|_| "failed to encode canonical transaction-details preparation")?;
    if archive.len() > AUTHENTICATED_TRANSACTION_DETAILS_PREPARATION_MAX_BYTES_V1 {
        return Err("encoded preparation archive exceeds its closed byte bound");
    }
    Ok((archive, digest))
}

pub(super) fn authenticated_transaction_details_finalize_v1(
    preparation: &[u8],
    signature_bytes: &[u8],
) -> Result<Vec<u8>, &'static str> {
    if signature_bytes.is_empty()
        || signature_bytes.len() > AUTHENTICATED_TRANSACTION_DETAILS_SIGNATURE_MAX_BYTES_V1
    {
        return Err("query signature is outside its closed byte bound");
    }
    let preparation = decode_preparation(preparation)?;
    let signatory = preparation
        .payload
        .authority()
        .try_signatory()
        .ok_or("query authority must be single-key")?;
    let algorithm = signatory
        .try_algorithm()
        .map_err(|_| "query authority signature algorithm is invalid")?;
    let signature = connect_signature_from_algorithm_bytes(algorithm, signature_bytes)
        .ok_or("query signature material is malformed")?;
    let signature = SignatureOf::<QueryRequestWithAuthority>::from_signature(signature);
    signature
        .verify(signatory, &preparation.payload)
        .map_err(|_| "query signature does not authenticate the native payload")?;
    let signed = SignedQuery {
        signature: QuerySignature(signature),
        payload: preparation.payload,
    };
    signed
        .verify_signature()
        .map_err(|_| "final signed query failed native verification")?;
    let body = signed.encode_versioned();
    if body.is_empty() || body.len() > AUTHENTICATED_TRANSACTION_DETAILS_SIGNED_QUERY_MAX_BYTES_V1 {
        return Err("final signed query violates its closed byte bound");
    }
    Ok(body)
}

fn rejection_code(reason: &TransactionRejectionReason) -> &'static str {
    match reason {
        TransactionRejectionReason::AccountDoesNotExist(_) => "account_does_not_exist",
        TransactionRejectionReason::LimitCheck(_) => "limit_check",
        TransactionRejectionReason::Validation(_) => "validation",
        TransactionRejectionReason::InstructionExecution(_) => "instruction_execution",
        TransactionRejectionReason::IvmExecution(_) => "ivm_execution",
        TransactionRejectionReason::TriggerExecution(_) => "trigger_execution",
    }
}

fn privacy_action_rejection_code(
    reason: &TransactionRejectionReason,
) -> AuthenticatedPrivacyActionRejectionCodeV1 {
    match reason {
        TransactionRejectionReason::AccountDoesNotExist(_) => {
            AuthenticatedPrivacyActionRejectionCodeV1::AccountDoesNotExist
        }
        TransactionRejectionReason::LimitCheck(_) => {
            AuthenticatedPrivacyActionRejectionCodeV1::LimitCheck
        }
        TransactionRejectionReason::Validation(_) => {
            AuthenticatedPrivacyActionRejectionCodeV1::Validation
        }
        TransactionRejectionReason::InstructionExecution(_) => {
            AuthenticatedPrivacyActionRejectionCodeV1::InstructionExecution
        }
        TransactionRejectionReason::IvmExecution(_) => {
            AuthenticatedPrivacyActionRejectionCodeV1::IvmExecution
        }
        TransactionRejectionReason::TriggerExecution(_) => {
            AuthenticatedPrivacyActionRejectionCodeV1::TriggerExecution
        }
    }
}

fn validated_rejection_message(
    reason: &TransactionRejectionReason,
) -> Result<String, &'static str> {
    let message = match reason {
        TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            InstructionExecutionError::OfflineDeviceEligibility(rejection),
        )) => rejection.detail.clone(),
        _ => {
            let mut message = reason.to_string();
            let mut source = reason.source();
            while let Some(current) = source {
                message = current.to_string();
                source = current.source();
            }
            message
        }
    };
    if message.is_empty()
        || message.len() > AUTHENTICATED_TRANSACTION_DETAILS_REJECTION_MESSAGE_MAX_BYTES_V1
        || message.trim() != message
        || message.chars().any(char::is_control)
    {
        return Err("committed rejection message violates its closed text contract");
    }
    Ok(message)
}

pub(super) fn authenticated_transaction_details_project_rejection_v1(
    preparation: &[u8],
    response: &[u8],
) -> Result<AuthenticatedCommittedRejectionProjectionV1, &'static str> {
    let projection = authenticated_transaction_details_project_result_v1(preparation, response)?;
    if projection.result_ok {
        return Err("transaction-details response is not a terminal committed rejection");
    }
    let response = norito::decode_canonical::<PipelineTransactionDetailsResponse>(response)
        .map_err(|_| "transaction-details response is not canonical Norito")?;
    let Err(reason) = &response.transaction.result().0 else {
        return Err("transaction-details response is not a terminal committed rejection");
    };
    let code = rejection_code(reason);
    if code.is_empty() || code.len() > AUTHENTICATED_TRANSACTION_DETAILS_REJECTION_CODE_MAX_BYTES_V1
    {
        return Err("committed rejection code violates its closed text contract");
    }
    Ok(AuthenticatedCommittedRejectionProjectionV1 {
        transaction_hash_hex: projection.transaction_hash_hex,
        transaction_authority: projection.transaction_authority,
        block_hash_hex: projection.block_hash_hex,
        result_hash_hex: projection.result_hash_hex,
        rejection_code: code,
        rejection_message: projection
            .rejection_message
            .ok_or("committed rejection omitted its canonical message")?,
        committed_block_height: projection.committed_block_height,
    })
}

pub(super) fn authenticated_transaction_details_project_result_v1(
    preparation: &[u8],
    response: &[u8],
) -> Result<AuthenticatedCommittedTransactionResultProjectionV1, &'static str> {
    let preparation = decode_preparation(preparation)?;
    if response.is_empty()
        || response.len() > AUTHENTICATED_TRANSACTION_DETAILS_RESPONSE_MAX_BYTES_V1
    {
        return Err("transaction-details response is outside its closed byte bound");
    }
    let response = norito::decode_canonical::<PipelineTransactionDetailsResponse>(response)
        .map_err(|_| "transaction-details response is not canonical Norito")?;
    let expected_hash = preparation.expected_entrypoint_hash.to_string();
    let transaction = &response.transaction;
    if response.hash != expected_hash
        || transaction.entrypoint_hash() != &preparation.expected_entrypoint_hash
        || transaction.entrypoint().hash() != preparation.expected_entrypoint_hash
        || transaction.result_hash() != &transaction.result().hash()
        || transaction.entrypoint_proof().leaf_index() != transaction.result_proof().leaf_index()
        || authenticated_finality_mobile_height_v1(response.block_height).is_err()
    {
        return Err("transaction-details response hash/result/block binding is invalid");
    }
    let TransactionEntrypoint::External(signed_transaction) = transaction.entrypoint() else {
        return Err("transaction-details response is not an external signed transaction");
    };
    let expected_network_id = preparation.payload.network_id();
    if signed_transaction.network_id() != Some(&expected_network_id) {
        return Err(
            "transaction-details response NetworkId differs from the signed query NetworkId",
        );
    }
    if signed_transaction.authority() != preparation.payload.authority() {
        return Err(
            "transaction-details response authority differs from the signed query authority",
        );
    }
    signed_transaction
        .verify_signature()
        .map_err(|_| "transaction-details response carries an invalid transaction signature")?;
    let (result_ok, rejection_message) = match &transaction.result().0 {
        Ok(_) => (true, None),
        Err(reason) => (false, Some(validated_rejection_message(reason)?)),
    };
    Ok(AuthenticatedCommittedTransactionResultProjectionV1 {
        transaction_hash_hex: expected_hash,
        transaction_authority: preparation.authority_literal,
        block_hash_hex: transaction.block_hash().to_string(),
        result_hash_hex: transaction.result_hash().to_string(),
        result_ok,
        rejection_message,
        committed_block_height: response.block_height,
    })
}

pub(super) fn authenticated_transaction_details_project_result_v2(
    preparation: &[u8],
    response: &[u8],
) -> Result<AuthenticatedCommittedTransactionResultProjectionV2, &'static str> {
    let preparation = decode_preparation_v2(preparation)?;
    if response.is_empty()
        || response.len() > AUTHENTICATED_TRANSACTION_DETAILS_RESPONSE_MAX_BYTES_V1
    {
        return Err("transaction-details response is outside its closed byte bound");
    }
    let response = norito::decode_canonical::<PipelineTransactionDetailsResponse>(response)
        .map_err(|_| "transaction-details response is not canonical Norito")?;
    let expected_hash = preparation.expected_entrypoint_hash.to_string();
    let transaction = &response.transaction;
    if response.hash != expected_hash
        || transaction.entrypoint_hash() != &preparation.expected_entrypoint_hash
        || transaction.entrypoint().hash() != preparation.expected_entrypoint_hash
        || transaction.result_hash() != &transaction.result().hash()
        || transaction.entrypoint_proof().leaf_index() != transaction.result_proof().leaf_index()
        || authenticated_finality_mobile_height_v1(response.block_height).is_err()
    {
        return Err("transaction-details response hash/result/block binding is invalid");
    }
    let TransactionEntrypoint::External(signed_transaction) = transaction.entrypoint() else {
        return Err("transaction-details response is not an external signed transaction");
    };
    let expected_network_id = preparation.payload.network_id();
    if signed_transaction.network_id() != Some(&expected_network_id) {
        return Err(
            "transaction-details response NetworkId differs from the signed query NetworkId",
        );
    }
    let expected_transaction_authority =
        canonical_authority(&preparation.expected_transaction_authority_literal)?;
    if signed_transaction.authority() != &expected_transaction_authority {
        return Err(
            "transaction-details response authority differs from the expected transaction authority",
        );
    }
    signed_transaction
        .verify_signature()
        .map_err(|_| "transaction-details response carries an invalid transaction signature")?;
    let (result_ok, rejection_message) = match &transaction.result().0 {
        Ok(_) => (true, None),
        Err(reason) => (false, Some(validated_rejection_message(reason)?)),
    };
    Ok(AuthenticatedCommittedTransactionResultProjectionV2 {
        transaction_hash_hex: expected_hash,
        query_authority: preparation.query_authority_literal,
        transaction_authority: preparation.expected_transaction_authority_literal,
        block_hash_hex: transaction.block_hash().to_string(),
        result_hash_hex: transaction.result_hash().to_string(),
        result_ok,
        rejection_message,
        committed_block_height: response.block_height,
    })
}

pub(super) fn authenticated_transaction_details_project_rejection_v2(
    preparation: &[u8],
    response: &[u8],
) -> Result<AuthenticatedCommittedRejectionProjectionV2, &'static str> {
    let projection = authenticated_transaction_details_project_result_v2(preparation, response)?;
    if projection.result_ok {
        return Err("transaction-details response is not a terminal committed rejection");
    }
    let response = norito::decode_canonical::<PipelineTransactionDetailsResponse>(response)
        .map_err(|_| "transaction-details response is not canonical Norito")?;
    let Err(reason) = &response.transaction.result().0 else {
        return Err("transaction-details response is not a terminal committed rejection");
    };
    let code = rejection_code(reason);
    if code.is_empty() || code.len() > AUTHENTICATED_TRANSACTION_DETAILS_REJECTION_CODE_MAX_BYTES_V1
    {
        return Err("committed rejection code violates its closed text contract");
    }
    Ok(AuthenticatedCommittedRejectionProjectionV2 {
        transaction_hash_hex: projection.transaction_hash_hex,
        query_authority: projection.query_authority,
        transaction_authority: projection.transaction_authority,
        block_hash_hex: projection.block_hash_hex,
        result_hash_hex: projection.result_hash_hex,
        rejection_code: code,
        rejection_message: projection
            .rejection_message
            .ok_or("committed rejection omitted its canonical message")?,
        committed_block_height: projection.committed_block_height,
    })
}

/// Project a committed rejection only when the authenticated transaction contains exactly the
/// caller-retained Kagemusha request. The local request bytes close the gap left by treating HTTP
/// 202 acceptance or its operation-reference body as transaction authentication.
pub(super) fn authenticated_kagemusha_rejection_project_v2(
    preparation: &[u8],
    response: &[u8],
    expected_operation_id: [u8; 32],
    expected_kind: &str,
    expected_request: &[u8],
) -> Result<AuthenticatedCommittedRejectionProjectionV2, &'static str> {
    let projection = authenticated_transaction_details_project_rejection_v2(preparation, response)?;
    let response = norito::decode_canonical::<PipelineTransactionDetailsResponse>(response)
        .map_err(|_| "transaction-details response is not canonical Norito")?;
    validate_exact_kagemusha_request_v2(
        &response,
        expected_operation_id,
        expected_kind,
        expected_request,
    )?;
    Ok(projection)
}

fn validate_exact_kagemusha_request_v2(
    response: &PipelineTransactionDetailsResponse,
    expected_operation_id: [u8; 32],
    expected_kind: &str,
    expected_request: &[u8],
) -> Result<(), &'static str> {
    if expected_operation_id == [0; 32] {
        return Err("expected Kagemusha operation id must be nonzero");
    }
    let TransactionEntrypoint::External(signed_transaction) = response.transaction.entrypoint()
    else {
        return Err("Kagemusha outcome is not an external signed transaction");
    };
    let Executable::Instructions(instructions) = signed_transaction.instructions() else {
        return Err("Kagemusha outcome does not contain explicit instructions");
    };
    if instructions.len() != 1 {
        return Err("Kagemusha outcome must contain exactly one instruction");
    }
    let instruction = instructions.iter().next().expect("one instruction");
    match expected_kind {
        "top_up" => {
            let expected =
                norito::decode_canonical::<KagemushaRecursiveSpendTopUpRequestV4>(expected_request)
                    .map_err(|_| "expected Kagemusha top-up request is not canonical Norito")?;
            if expected.operation_id != expected_operation_id {
                return Err("expected Kagemusha top-up request carries another operation id");
            }
            let actual = instruction
                .as_any()
                .downcast_ref::<TopUpKagemushaRecursiveV4>()
                .ok_or("Kagemusha outcome instruction kind differs from top_up")?;
            if actual.request() != &expected {
                return Err("Kagemusha outcome transaction carries another top-up request");
            }
        }
        "redeem" => {
            let expected = norito::decode_canonical::<KagemushaRecursiveSpendRedeemRequestV4>(
                expected_request,
            )
            .map_err(|_| "expected Kagemusha redemption request is not canonical Norito")?;
            if expected.operation_id != expected_operation_id {
                return Err("expected Kagemusha redemption request carries another operation id");
            }
            let actual = instruction
                .as_any()
                .downcast_ref::<RedeemKagemushaRecursiveV4>()
                .ok_or("Kagemusha outcome instruction kind differs from redeem")?;
            if actual.request() != &expected {
                return Err("Kagemusha outcome transaction carries another redemption request");
            }
        }
        _ => return Err("expected Kagemusha operation kind is invalid"),
    }
    Ok(())
}

pub(super) fn authenticated_finality_proof_page_bind_v1(
    proof_archives: &[Vec<u8>],
) -> Result<AuthenticatedFinalityProofPageProjectionV1, &'static str> {
    let proof_lengths = proof_archives.iter().map(Vec::len).collect::<Vec<_>>();
    validate_finality_proof_archive_lengths_v1(&proof_lengths)?;
    let mut proofs = Vec::with_capacity(proof_archives.len());
    for proof_archive in proof_archives {
        proofs.push(decode_canonical_finality_proof_v1(proof_archive)?);
    }
    let page = AuthenticatedFinalityProofPageV1 {
        version: AUTHENTICATED_FINALITY_PAGE_VERSION_V1,
        proofs,
    };
    let archive = norito::encode_canonical(&page)
        .map_err(|_| "failed to encode canonical finality proof page")?;
    if archive.is_empty() || archive.len() > AUTHENTICATED_FINALITY_PAGE_MAX_BYTES_V1 {
        return Err("canonical finality proof page exceeds its closed byte bound");
    }
    Ok(AuthenticatedFinalityProofPageProjectionV1 {
        hash_hex: Hash::new(&archive).to_string(),
        archive,
    })
}

fn validate_finality_proof_archive_lengths_v1(
    proof_lengths: &[usize],
) -> Result<usize, &'static str> {
    if proof_lengths.is_empty() || proof_lengths.len() > AUTHENTICATED_FINALITY_PAGE_MAX_PROOFS_V1 {
        return Err("finality proof page must contain 1..64 proofs");
    }
    proof_lengths.iter().try_fold(0_usize, |total, length| {
        if *length == 0 || *length > AUTHENTICATED_FINALITY_PROOF_MAX_BYTES_V1 {
            return Err("one finality proof is outside its closed byte bound");
        }
        total
            .checked_add(*length)
            .filter(|sum| *sum <= AUTHENTICATED_FINALITY_PAGE_MAX_BYTES_V1)
            .ok_or("finality proof page exceeds its aggregate byte bound")
    })
}

pub(super) fn authenticated_finality_page_verify_v1(
    network_id: &[u8],
    trusted_checkpoint_height: u64,
    trusted_checkpoint_context_id: &[u8],
    page_archive: &[u8],
) -> Result<AuthenticatedFinalityCheckpointProjectionV1, &'static str> {
    let network_id = network_id_from_raw_bytes(network_id)?;
    let trusted_context_id = parse_height_context_id_v1(trusted_checkpoint_context_id)?;
    let page = decode_canonical_finality_page_v1(page_archive)?;
    verify_finality_page_v1(
        network_id,
        trusted_checkpoint_height,
        trusted_context_id,
        &page,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn authenticated_finalized_kagemusha_outcome_project_v1(
    preparation: &[u8],
    response_archive: &[u8],
    expected_operation_id: [u8; 32],
    expected_kind: &str,
    expected_request: &[u8],
    network_id: &[u8],
    trusted_checkpoint_height: u64,
    trusted_checkpoint_context_id: &[u8],
    finality_page_archive: &[u8],
    executed_block_wire: &[u8],
) -> Result<AuthenticatedFinalizedKagemushaOutcomeProjectionV1, &'static str> {
    let expected_network_id = network_id_from_raw_bytes(network_id)?;
    let preparation_decoded = decode_preparation_v2(preparation)?;
    if preparation_decoded.payload.network_id() != expected_network_id {
        return Err("finality NetworkId differs from the signed transaction-details query");
    }
    let result_projection =
        authenticated_transaction_details_project_result_v2(preparation, response_archive)?;
    if trusted_checkpoint_height >= result_projection.committed_block_height {
        return Err("trusted checkpoint must predate the committed Kagemusha outcome");
    }
    let response = norito::decode_canonical::<PipelineTransactionDetailsResponse>(response_archive)
        .map_err(|_| "transaction-details response is not canonical Norito")?;
    validate_exact_kagemusha_request_v2(
        &response,
        expected_operation_id,
        expected_kind,
        expected_request,
    )?;
    if response.transaction.merge_inclusion().is_some() {
        return Err("Kagemusha issuer outcome must not use certified merge-sidecar inclusion");
    }

    let trusted_context_id = parse_height_context_id_v1(trusted_checkpoint_context_id)?;
    let finality_page = decode_canonical_finality_page_v1(finality_page_archive)?;
    let finalized_checkpoint = verify_finality_page_v1(
        expected_network_id,
        trusted_checkpoint_height,
        trusted_context_id,
        &finality_page,
    )?;
    if finalized_checkpoint.height != result_projection.committed_block_height {
        return Err("terminal finality page does not end at the committed transaction height");
    }
    let finality = finality_page
        .proofs
        .last()
        .ok_or("terminal finality page is empty")?;

    let block = decode_canonical_executed_block_wire_v1(executed_block_wire)?;
    let block_height = authenticated_finality_mobile_height_v1(block.header().height().get())?;
    if block_height != result_projection.committed_block_height
        || block.hash() != *response.transaction.block_hash()
    {
        return Err("executed block wire differs from the committed transaction carrier");
    }
    let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
        &block,
        &finality.finality_artifact,
        response.transaction.entrypoint_hash(),
    )
    .map_err(|_| "finality artifact does not authenticate the exact executed block wire")?;
    let anchor_height = authenticated_finality_mobile_height_v1(anchor.block_height().get())?;
    if anchor_height != result_projection.committed_block_height
        || anchor.block_hash() != *response.transaction.block_hash()
        || anchor.entry_hash() != *response.transaction.entrypoint_hash()
        || !response.transaction.verify_inclusion_in_block(&block)
    {
        return Err(
            "committed transaction entry/result inclusion is invalid for the finalized block",
        );
    }

    let (terminal_state, rejection_code, rejection_message) = match &response.transaction.result().0
    {
        Ok(_) => (
            AuthenticatedFinalizedKagemushaTerminalStateV1::Applied,
            None,
            None,
        ),
        Err(reason) => (
            AuthenticatedFinalizedKagemushaTerminalStateV1::Rejected,
            Some(rejection_code(reason).to_owned()),
            Some(validated_rejection_message(reason)?),
        ),
    };
    if result_projection.result_ok
        != matches!(
            terminal_state,
            AuthenticatedFinalizedKagemushaTerminalStateV1::Applied
        )
    {
        return Err("transaction result projection changed during finality verification");
    }

    let transaction_details_hash = Hash::new(response_archive);
    let finality_page_hash = Hash::new(finality_page_archive);
    let executed_block_wire_hash = anchor.executed_block_wire_hash();
    let evidence_id = finalized_kagemusha_evidence_id_v1(
        &expected_network_id,
        trusted_checkpoint_height,
        trusted_checkpoint_context_id,
        finalized_checkpoint,
        terminal_state,
        expected_operation_id,
        expected_kind,
        expected_request,
        &response,
        &result_projection.query_authority,
        &result_projection.transaction_authority,
        Hash::new(preparation),
        transaction_details_hash,
        finality_page_hash,
        executed_block_wire_hash,
    );
    Ok(AuthenticatedFinalizedKagemushaOutcomeProjectionV1 {
        terminal_state,
        operation_id: expected_operation_id,
        operation_kind: expected_kind.to_owned(),
        transaction_hash_hex: result_projection.transaction_hash_hex,
        query_authority: result_projection.query_authority,
        transaction_authority: result_projection.transaction_authority,
        block_hash_hex: result_projection.block_hash_hex,
        result_hash_hex: result_projection.result_hash_hex,
        committed_block_height: result_projection.committed_block_height,
        finalized_checkpoint,
        executed_block_wire_hash_hex: executed_block_wire_hash.to_string(),
        rejection_code,
        rejection_message,
        evidence_id_hex: evidence_id.to_string(),
        transaction_details_hash_hex: transaction_details_hash.to_string(),
        finality_page_hash_hex: finality_page_hash.to_string(),
    })
}

struct RequestedPrivacyActionBindingV1 {
    transaction_intent_digest: [u8; 32],
    statement_digest: [u8; 32],
    proof_envelope_hash: [u8; 32],
}

fn requested_privacy_action_binding_v1(
    bytes: &[u8],
) -> Result<RequestedPrivacyActionBindingV1, &'static str> {
    if bytes.len() != AUTHENTICATED_FINALIZED_PRIVACY_ACTION_BINDING_BYTES_V1 {
        return Err("requested Exact12 action binding must contain exactly 96 bytes");
    }
    let intent = <[u8; 32]>::try_from(&bytes[..32])
        .map_err(|_| "transaction-intent digest must contain exactly 32 bytes")?;
    let statement = <[u8; 32]>::try_from(&bytes[32..64])
        .map_err(|_| "statement digest must contain exactly 32 bytes")?;
    let envelope = <[u8; 32]>::try_from(&bytes[64..])
        .map_err(|_| "proof-envelope hash must contain exactly 32 bytes")?;
    if intent == [0; 32] || statement == [0; 32] || envelope == [0; 32] {
        return Err("requested Exact12 action binding digests must be nonzero");
    }
    Ok(RequestedPrivacyActionBindingV1 {
        transaction_intent_digest: intent,
        statement_digest: statement,
        proof_envelope_hash: envelope,
    })
}

/// Verify a committed Exact12 rejection against the exact signed action, transaction-details
/// query authorities, result-bearing block wire, and an independent Sumeragi-v2 finality chain.
///
/// The caller-retained three-digest binding is compared to a fresh native inspection of the exact
/// transaction carried by the finalized block. This prevents a transaction hash or public status
/// from being reused to terminalize a different privacy operation.
#[allow(clippy::too_many_arguments)]
pub(super) fn authenticated_finalized_privacy_action_rejection_project_v1(
    preparation: &[u8],
    response_archive: &[u8],
    operation_index: i32,
    action_index: u32,
    requested_action_binding: &[u8],
    network_id: &[u8],
    trusted_checkpoint_height: u64,
    trusted_checkpoint_context_id: &[u8],
    finality_page_archive: &[u8],
    executed_block_wire: &[u8],
) -> Result<AuthenticatedFinalizedPrivacyActionRejectionProjectionV1, &'static str> {
    if action_index != 0 {
        return Err("first-release Exact12 rejection projection requires action index zero");
    }
    let operation = exact12_operation_from_index(operation_index)
        .ok_or("Exact12 operation discriminant is outside the closed union")?;
    let requested_binding = requested_privacy_action_binding_v1(requested_action_binding)?;
    let expected_intent = requested_binding.transaction_intent_digest;
    let expected_statement = requested_binding.statement_digest;
    let expected_envelope = requested_binding.proof_envelope_hash;
    let expected_network_id = network_id_from_raw_bytes(network_id)?;
    let preparation_decoded = decode_preparation_v2(preparation)?;
    if preparation_decoded.payload.network_id() != expected_network_id {
        return Err("finality NetworkId differs from the signed transaction-details query");
    }
    let result_projection =
        authenticated_transaction_details_project_result_v2(preparation, response_archive)?;
    if result_projection.result_ok {
        return Err("Exact12 rejection projection received an applied transaction result");
    }
    if trusted_checkpoint_height >= result_projection.committed_block_height {
        return Err("trusted checkpoint must predate the committed Exact12 rejection");
    }

    let response = norito::decode_canonical::<PipelineTransactionDetailsResponse>(response_archive)
        .map_err(|_| "transaction-details response is not canonical Norito")?;
    if response.transaction.merge_inclusion().is_some() {
        return Err("Exact12 action must not use certified merge-sidecar inclusion");
    }
    let TransactionEntrypoint::External(signed_transaction) = response.transaction.entrypoint()
    else {
        return Err("Exact12 rejection is not an external signed transaction");
    };
    let signed_wire = signed_transaction.encode_versioned();
    let inspected = inspect_signed_privacy_exact12_action_v1(
        &signed_wire,
        expected_network_id.as_bytes(),
        &result_projection.transaction_authority,
        operation_index,
    )?;
    if inspected.transaction_hash.as_slice() != response.transaction.entrypoint_hash().as_ref()
        || inspected.transaction_intent_digest != expected_intent
        || inspected.statement_digest != expected_statement
        || inspected.proof_envelope_hash != expected_envelope
    {
        return Err("finalized transaction differs from the requested Exact12 action binding");
    }

    let trusted_context_id = parse_height_context_id_v1(trusted_checkpoint_context_id)?;
    let finality_page = decode_canonical_finality_page_v1(finality_page_archive)?;
    let finalized_checkpoint = verify_finality_page_v1(
        expected_network_id,
        trusted_checkpoint_height,
        trusted_context_id,
        &finality_page,
    )?;
    if finalized_checkpoint.height != result_projection.committed_block_height {
        return Err("terminal finality page does not end at the committed Exact12 height");
    }
    let finality = finality_page
        .proofs
        .last()
        .ok_or("terminal finality page is empty")?;

    let block = decode_canonical_executed_block_wire_v1(executed_block_wire)?;
    let block_height = authenticated_finality_mobile_height_v1(block.header().height().get())?;
    if block_height != result_projection.committed_block_height
        || block.hash() != *response.transaction.block_hash()
    {
        return Err("executed block wire differs from the committed Exact12 carrier");
    }
    let anchor = TrustedBlockProofAnchor::from_untrusted_finality_artifact(
        &block,
        &finality.finality_artifact,
        response.transaction.entrypoint_hash(),
    )
    .map_err(|_| "finality artifact does not authenticate the exact executed block wire")?;
    let anchor_height = authenticated_finality_mobile_height_v1(anchor.block_height().get())?;
    if anchor_height != result_projection.committed_block_height
        || anchor.block_hash() != *response.transaction.block_hash()
        || anchor.entry_hash() != *response.transaction.entrypoint_hash()
        || !response.transaction.verify_inclusion_in_block(&block)
    {
        return Err(
            "committed Exact12 transaction entry/result inclusion is invalid for the finalized block",
        );
    }

    let Err(reason) = &response.transaction.result().0 else {
        return Err("Exact12 rejection projection received an applied transaction result");
    };
    let rejection_code = privacy_action_rejection_code(reason);
    let rejection_message = validated_rejection_message(reason)?;
    let transaction_details_hash = Hash::new(response_archive);
    let finality_page_hash = Hash::new(finality_page_archive);
    let executed_block_wire_hash = anchor.executed_block_wire_hash();
    let evidence_id = finalized_privacy_action_rejection_evidence_id_v1(
        &expected_network_id,
        trusted_checkpoint_height,
        trusted_checkpoint_context_id,
        finalized_checkpoint,
        operation,
        action_index,
        expected_intent,
        expected_statement,
        expected_envelope,
        &response,
        &result_projection.query_authority,
        &result_projection.transaction_authority,
        rejection_code,
        &rejection_message,
        Hash::new(preparation),
        transaction_details_hash,
        finality_page_hash,
        executed_block_wire_hash,
    );
    Ok(AuthenticatedFinalizedPrivacyActionRejectionProjectionV1 {
        network_id_hex: hex::encode(expected_network_id.as_bytes()),
        protocol_id: operation.protocol_id(),
        operation_schema: operation,
        ledger_effect_kind: operation.ledger_effect_kind(),
        transaction_hash_hex: result_projection.transaction_hash_hex,
        action_index,
        transaction_intent_digest_hex: hex::encode(expected_intent),
        statement_digest_hex: hex::encode(expected_statement),
        proof_envelope_hash_hex: hex::encode(expected_envelope),
        query_authority: result_projection.query_authority,
        transaction_authority: result_projection.transaction_authority,
        block_hash_hex: result_projection.block_hash_hex,
        result_hash_hex: result_projection.result_hash_hex,
        rejection_code,
        rejection_message,
        committed_block_height: result_projection.committed_block_height,
        finalized_checkpoint,
        executed_block_wire_hash_hex: executed_block_wire_hash.to_string(),
        evidence_id_hex: evidence_id.to_string(),
        transaction_details_hash_hex: transaction_details_hash.to_string(),
        finality_page_hash_hex: finality_page_hash.to_string(),
    })
}

fn decode_canonical_finality_proof_v1(
    proof_archive: &[u8],
) -> Result<BridgeFinalityProof, &'static str> {
    let limits = authenticated_finality_decode_limits_v1(proof_archive.len());
    norito::decode_canonical_with_limits(proof_archive, limits)
        .map_err(|_| "finality proof is not bounded canonical Norito")
}

fn decode_canonical_finality_page_v1(
    page_archive: &[u8],
) -> Result<AuthenticatedFinalityProofPageV1, &'static str> {
    if page_archive.is_empty() || page_archive.len() > AUTHENTICATED_FINALITY_PAGE_MAX_BYTES_V1 {
        return Err("finality proof page archive is outside its closed byte bound");
    }
    let page: AuthenticatedFinalityProofPageV1 = norito::decode_canonical_with_limits(
        page_archive,
        authenticated_finality_decode_limits_v1(page_archive.len()),
    )
    .map_err(|_| "finality proof page is not bounded canonical Norito")?;
    if page.version != AUTHENTICATED_FINALITY_PAGE_VERSION_V1
        || page.proofs.is_empty()
        || page.proofs.len() > AUTHENTICATED_FINALITY_PAGE_MAX_PROOFS_V1
    {
        return Err("finality proof page version or proof count is invalid");
    }
    let mut total_bytes = 0_usize;
    for proof in &page.proofs {
        let proof_archive = norito::encode_canonical(proof)
            .map_err(|_| "failed to re-encode finality proof canonically")?;
        if proof_archive.is_empty()
            || proof_archive.len() > AUTHENTICATED_FINALITY_PROOF_MAX_BYTES_V1
        {
            return Err("one decoded finality proof is outside its closed byte bound");
        }
        total_bytes = total_bytes
            .checked_add(proof_archive.len())
            .filter(|total| *total <= AUTHENTICATED_FINALITY_PAGE_MAX_BYTES_V1)
            .ok_or("decoded finality proof page exceeds its aggregate byte bound")?;
    }
    Ok(page)
}

fn verify_finality_page_v1(
    network_id: NetworkId,
    trusted_checkpoint_height: u64,
    trusted_context_id: HeightContextId,
    page: &AuthenticatedFinalityProofPageV1,
) -> Result<AuthenticatedFinalityCheckpointProjectionV1, &'static str> {
    authenticated_finality_mobile_height_v1(trusted_checkpoint_height)?;
    let first = page.proofs.first().ok_or("finality proof page is empty")?;
    if first.finality_artifact.height != trusted_checkpoint_height
        || first.finality_artifact.context_id() != trusted_context_id
    {
        return Err("first finality proof does not equal the trusted checkpoint");
    }
    let mut verifier = BridgeFinalityVerifier::with_context(network_id, trusted_context_id);
    for proof in &page.proofs {
        authenticated_finality_mobile_height_v1(proof.block_header.height().get())?;
        authenticated_finality_mobile_height_v1(proof.finality_artifact.height)?;
        verifier
            .verify(proof)
            .map_err(|_| "finality proof page failed network/context/successor/QC verification")?;
    }
    let last = page.proofs.last().ok_or("finality proof page is empty")?;
    let finalized_height = authenticated_finality_mobile_height_v1(last.finality_artifact.height)?;
    Ok(AuthenticatedFinalityCheckpointProjectionV1 {
        height: finalized_height,
        context_id: *last.finality_artifact.context_id().0.as_ref(),
    })
}

fn parse_height_context_id_v1(bytes: &[u8]) -> Result<HeightContextId, &'static str> {
    let exact: [u8; Hash::LENGTH] = bytes
        .try_into()
        .map_err(|_| "trusted finality context id must contain exactly 32 bytes")?;
    let hash = Hash::from_str(&hex::encode(exact))
        .map_err(|_| "trusted finality context id must be a marked Iroha hash")?;
    Ok(HeightContextId(
        HashOf::<HeightContext>::from_untyped_unchecked(hash),
    ))
}

fn decode_canonical_executed_block_wire_v1(bytes: &[u8]) -> Result<SignedBlock, &'static str> {
    if bytes.is_empty() || bytes.len() > AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1 {
        return Err("executed block wire is outside its closed byte bound");
    }
    let limits = authenticated_finality_decode_limits_v1(bytes.len());
    let block = norito::core::with_decode_limits(limits, || {
        decode_versioned_signed_block(bytes)
            .map_err(|error| norito::core::Error::Message(error.to_string()))
    })
    .map_err(|_| "executed block wire is not a bounded SignedBlockWire")?;
    let canonical = block
        .encode_wire()
        .map_err(|_| "executed block wire could not be canonically re-encoded")?;
    if canonical != bytes {
        return Err("executed block wire is not its exact canonical re-encoding");
    }
    Ok(block)
}

fn authenticated_finality_decode_limits_v1(encoded_len: usize) -> norito::DecodeLimits {
    let canonical = norito::canonical_decode_limits(encoded_len);
    norito::DecodeLimits::new(
        canonical.max_sequence_elements(),
        canonical.max_field_bytes(),
        canonical.max_total_elements(),
        encoded_len.saturating_mul(12).saturating_add(1024 * 1024),
        128,
    )
}

#[allow(clippy::too_many_arguments)]
fn finalized_kagemusha_evidence_id_v1(
    network_id: &NetworkId,
    trusted_checkpoint_height: u64,
    trusted_checkpoint_context_id: &[u8],
    finalized_checkpoint: AuthenticatedFinalityCheckpointProjectionV1,
    terminal_state: AuthenticatedFinalizedKagemushaTerminalStateV1,
    operation_id: [u8; 32],
    operation_kind: &str,
    expected_request: &[u8],
    response: &PipelineTransactionDetailsResponse,
    query_authority: &str,
    transaction_authority: &str,
    preparation_hash: Hash,
    transaction_details_hash: Hash,
    finality_page_hash: Hash,
    executed_block_wire_hash: Hash,
) -> Hash {
    let trusted_height = trusted_checkpoint_height.to_be_bytes();
    let finalized_height = finalized_checkpoint.height.to_be_bytes();
    let terminal_tag = [match terminal_state {
        AuthenticatedFinalizedKagemushaTerminalStateV1::Applied => 0,
        AuthenticatedFinalizedKagemushaTerminalStateV1::Rejected => 1,
    }];
    let operation_tag = [match operation_kind {
        "top_up" => 0,
        "redeem" => 1,
        _ => u8::MAX,
    }];
    let request_hash = Hash::new(expected_request);
    let query_authority_hash = Hash::new(query_authority.as_bytes());
    let transaction_authority_hash = Hash::new(transaction_authority.as_bytes());
    Hash::new_from_chunks(&[
        AUTHENTICATED_FINALIZED_KAGEMUSHA_EVIDENCE_DOMAIN_V1,
        network_id.as_bytes(),
        &trusted_height,
        trusted_checkpoint_context_id,
        &finalized_height,
        &finalized_checkpoint.context_id,
        &terminal_tag,
        &operation_tag,
        &operation_id,
        response.transaction.entrypoint_hash().as_ref(),
        response.transaction.block_hash().as_ref(),
        response.transaction.result_hash().as_ref(),
        request_hash.as_ref(),
        query_authority_hash.as_ref(),
        transaction_authority_hash.as_ref(),
        preparation_hash.as_ref(),
        transaction_details_hash.as_ref(),
        finality_page_hash.as_ref(),
        executed_block_wire_hash.as_ref(),
    ])
}

#[allow(clippy::too_many_arguments)]
fn finalized_privacy_action_rejection_evidence_id_v1(
    network_id: &NetworkId,
    trusted_checkpoint_height: u64,
    trusted_checkpoint_context_id: &[u8],
    finalized_checkpoint: AuthenticatedFinalityCheckpointProjectionV1,
    operation: PrivacyOperationSchemaV1,
    action_index: u32,
    transaction_intent_digest: [u8; 32],
    statement_digest: [u8; 32],
    proof_envelope_hash: [u8; 32],
    response: &PipelineTransactionDetailsResponse,
    query_authority: &str,
    transaction_authority: &str,
    rejection_code: AuthenticatedPrivacyActionRejectionCodeV1,
    rejection_message: &str,
    preparation_hash: Hash,
    transaction_details_hash: Hash,
    finality_page_hash: Hash,
    executed_block_wire_hash: Hash,
) -> Hash {
    let trusted_height = trusted_checkpoint_height.to_be_bytes();
    let finalized_height = finalized_checkpoint.height.to_be_bytes();
    let terminal_tag = [1_u8];
    let protocol_hash = Hash::new(operation.protocol_id().canonical_label().as_bytes());
    let operation_hash = Hash::new(operation.canonical_label().as_bytes());
    let ledger_effect_hash = Hash::new(operation.ledger_effect_kind().canonical_label().as_bytes());
    let action_index = action_index.to_be_bytes();
    let rejection_code_hash = Hash::new(rejection_code.canonical_label().as_bytes());
    let rejection_message_hash = Hash::new(rejection_message.as_bytes());
    let query_authority_hash = Hash::new(query_authority.as_bytes());
    let transaction_authority_hash = Hash::new(transaction_authority.as_bytes());
    Hash::new_from_chunks(&[
        AUTHENTICATED_FINALIZED_PRIVACY_ACTION_REJECTION_EVIDENCE_DOMAIN_V1,
        network_id.as_bytes(),
        &trusted_height,
        trusted_checkpoint_context_id,
        &finalized_height,
        &finalized_checkpoint.context_id,
        &terminal_tag,
        protocol_hash.as_ref(),
        operation_hash.as_ref(),
        ledger_effect_hash.as_ref(),
        &action_index,
        &transaction_intent_digest,
        &statement_digest,
        &proof_envelope_hash,
        response.transaction.entrypoint_hash().as_ref(),
        response.transaction.block_hash().as_ref(),
        response.transaction.result_hash().as_ref(),
        rejection_code_hash.as_ref(),
        rejection_message_hash.as_ref(),
        query_authority_hash.as_ref(),
        transaction_authority_hash.as_ref(),
        preparation_hash.as_ref(),
        transaction_details_hash.as_ref(),
        finality_page_hash.as_ref(),
        executed_block_wire_hash.as_ref(),
    ])
}

pub(super) fn authenticated_offline_device_registration_result_project_v1(
    preparation: &[u8],
    response: &[u8],
) -> Result<AuthenticatedOfflineDeviceRegistrationResultProjectionV1, &'static str> {
    let projection = authenticated_transaction_details_project_result_v1(preparation, response)?;
    let response = norito::decode_canonical::<PipelineTransactionDetailsResponse>(response)
        .map_err(|_| "transaction-details response is not canonical Norito")?;
    let TransactionEntrypoint::External(signed_transaction) = response.transaction.entrypoint()
    else {
        return Err("device-registration result is not an external signed transaction");
    };
    let Executable::Instructions(instructions) = signed_transaction.instructions() else {
        return Err("device-registration result does not contain explicit instructions");
    };
    if instructions.len() != 1
        || instructions
            .iter()
            .next()
            .and_then(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<RegisterOfflineDeviceAttestation>()
            })
            .is_none()
    {
        return Err("device-registration result must contain exactly one registration instruction");
    }

    let (terminal_state, eligibility_decision, rejection_code, rejection_message) =
        match &response.transaction.result().0 {
            Ok(_) => (
                OfflineDeviceRegistrationTerminalStateV1::Applied,
                None,
                None,
                None,
            ),
            Err(TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
                InstructionExecutionError::OfflineDeviceEligibility(rejection),
            ))) => {
                rejection
                    .validate_v1()
                    .map_err(|_| "committed device-eligibility rejection is invalid")?;
                if rejection.decision.outcome == OfflineDeviceEligibilityOutcomeV1::Eligible {
                    return Err("committed device-eligibility rejection carries eligible state");
                }
                (
                    OfflineDeviceRegistrationTerminalStateV1::EligibilityRejected,
                    Some(rejection.decision.clone()),
                    Some("offline_device_eligibility".to_owned()),
                    Some(rejection.detail.clone()),
                )
            }
            Err(reason) => (
                OfflineDeviceRegistrationTerminalStateV1::OtherRejected,
                None,
                Some(rejection_code(reason).to_owned()),
                projection.rejection_message.clone(),
            ),
        };
    Ok(AuthenticatedOfflineDeviceRegistrationResultProjectionV1 {
        transaction_hash_hex: projection.transaction_hash_hex,
        transaction_authority: projection.transaction_authority,
        block_hash_hex: projection.block_hash_hex,
        result_hash_hex: projection.result_hash_hex,
        committed_block_height: projection.committed_block_height,
        terminal_state,
        eligibility_decision,
        rejection_code,
        rejection_message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        Level, ValidationFail,
        block::consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PayloadEncoding, QuorumCertificate,
            ValidatorPower, finality::V2FinalityArtifact,
        },
        bridge::BRIDGE_FINALITY_PROOF_VERSION_V2,
        isi::{InstructionBox, Log, error::InstructionExecutionError},
        offline::{
            KagemushaDevicePublicKeyV2, OfflineDeviceAttestationRegistration,
            OfflineDeviceEligibilityDecisionV1, OfflineDeviceEligibilityOutcomeV1,
            OfflineDeviceEligibilityReasonV1, OfflineDeviceEligibilityRejectionV1,
        },
        peer::PeerId,
        privacy::{PrivacyOperationSchemaV1, privacy_exact12_fixture_bundle_v1},
        query::SignedQuery,
        transaction::{
            FeePaymentIntent, TransactionBuilder, TransactionResult, TransactionResultInner,
        },
    };
    use iroha_version::codec::DecodeVersioned as _;
    use libc::c_ulong;
    use norito::json::Value as JsonValue;
    use std::{collections::BTreeSet, ptr, slice, time::Duration};

    fn fixture() -> (KeyPair, NetworkId, String, String, [u8; 32]) {
        let key_pair = KeyPair::random();
        let authority = AccountId::new(key_pair.public_key().clone()).to_string();
        let mut network_bytes = [0x91; 32];
        network_bytes[31] |= 1;
        let network_id = network_id_from_raw_bytes(&network_bytes).expect("network id");
        let mut transaction_hash = [0xAB; 32];
        transaction_hash[31] |= 1;
        (
            key_pair,
            network_id,
            authority,
            hex::encode(transaction_hash),
            [0xA7; 32],
        )
    }

    fn signed_preparation() -> (Vec<u8>, Vec<u8>, KeyPair, NetworkId, String) {
        let (key_pair, network_id, authority, transaction_hash, nonce) = fixture();
        let (preparation, digest) = authenticated_transaction_details_prepare_v1(
            network_id.as_bytes(),
            &authority,
            &transaction_hash,
            1_900_000_000_000,
            nonce,
        )
        .expect("prepare query");
        let signature =
            Signature::try_new(key_pair.private_key(), &digest).expect("sign query digest");
        (
            preparation,
            signature.payload().to_vec(),
            key_pair,
            network_id,
            authority,
        )
    }

    #[test]
    fn native_query_preparation_and_external_signature_are_exact_and_bound() {
        let (preparation, signature, _key_pair, network_id, authority) = signed_preparation();
        let body = authenticated_transaction_details_finalize_v1(&preparation, &signature)
            .expect("finalize signed query");
        let query = SignedQuery::decode_all_versioned(&body).expect("decode signed query");
        query.verify_signature().expect("verify signed query");
        assert_eq!(query.payload.network_id(), network_id);
        assert_eq!(query.payload.authority().to_string(), authority);
        assert_eq!(query.payload.time_to_live_ms().get(), 100_000);
        assert_ne!(query.payload.nonce(), &[0; 32]);
        assert!(exact_query_hash(&query.payload).is_some());
    }

    #[test]
    fn version_two_separates_signed_query_and_expected_transaction_authorities() {
        let (query_key, network_id, query_authority, _, nonce) = fixture();
        let transaction_key = KeyPair::random();
        let transaction_authority =
            AccountId::new(transaction_key.public_key().clone()).to_string();
        assert_ne!(query_authority, transaction_authority);
        let (response, transaction_hash) =
            committed_response(&transaction_key, network_id, &transaction_authority);
        let transaction_hash =
            std::str::from_utf8(&transaction_hash).expect("transaction hash is UTF-8");
        let (preparation, digest) = authenticated_transaction_details_prepare_v2(
            network_id.as_bytes(),
            &query_authority,
            &transaction_authority,
            transaction_hash,
            1_900_000_000_000,
            nonce,
        )
        .expect("prepare authority-split query");
        let signature =
            Signature::try_new(query_key.private_key(), &digest).expect("sign query digest");
        let body = authenticated_transaction_details_finalize_v2(&preparation, signature.payload())
            .expect("finalize authority-split query");
        let query = SignedQuery::decode_all_versioned(&body).expect("decode signed query");
        assert_eq!(query.payload.authority().to_string(), query_authority);
        let projection =
            authenticated_transaction_details_project_rejection_v2(&preparation, &response)
                .expect("project issuer transaction through wallet query authority");
        assert_eq!(projection.query_authority, query_authority);
        assert_eq!(projection.transaction_authority, transaction_authority);

        let (wrong_preparation, _) = authenticated_transaction_details_prepare_v2(
            network_id.as_bytes(),
            &query_authority,
            &query_authority,
            transaction_hash,
            1_900_000_000_000,
            [0xB7; 32],
        )
        .expect("prepare deliberately wrong expected authority");
        assert!(
            authenticated_transaction_details_project_rejection_v2(&wrong_preparation, &response,)
                .is_err()
        );
    }

    #[test]
    fn authority_split_jni_inventory_is_explicit_for_both_mobile_namespaces() {
        let source = include_str!("platform_jni/part_3.rs");
        for method in [
            "nativePrepareExactTransactionQueryV2",
            "nativeFinalizeExactTransactionQueryV2",
            "nativeProjectExactCommittedRejectionV2",
            "nativeProjectExactKagemushaCommittedRejectionV2",
            "nativeProjectExactCommittedTransactionResultV2",
            "nativeBindFinalityProofPageV1",
            "nativeVerifyFinalityPageV1",
            "nativeProjectFinalizedKagemushaOutcomeV1",
        ] {
            assert!(
                source.contains(&format!(
                    "Java_org_hyperledger_iroha_sdk_client_AuthenticatedTransactionDetailsNativeBridge_{method}"
                )),
                "missing SDK JNI export for {method}",
            );
            assert!(
                source.contains(&format!(
                    "Java_org_hyperledger_iroha_android_client_AuthenticatedTransactionDetailsNativeBridge_{method}"
                )),
                "missing Android JNI export for {method}",
            );
        }
    }

    struct FinalityFixtureAuthority {
        keys: Vec<KeyPair>,
        roster: Vec<ValidatorPower>,
        quorum: DualQuorum,
        pops: Vec<Vec<u8>>,
    }

    fn finality_fixture_authority() -> FinalityFixtureAuthority {
        let mut keys = (0_u8..4)
            .map(|index| {
                KeyPair::try_from_seed(
                    vec![0xD0_u8.saturating_add(index); 32],
                    Algorithm::BlsNormal,
                )
                .expect("derive deterministic finality validator")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| {
            PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
        });
        let roster = keys
            .iter()
            .map(|key| ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let quorum = DualQuorum::from_roster(&roster).expect("valid finality roster");
        let pops = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("derive finality validator PoP")
            })
            .collect::<Vec<_>>();
        FinalityFixtureAuthority {
            keys,
            roster,
            quorum,
            pops,
        }
    }

    fn finality_artifact_fixture(
        authority: &FinalityFixtureAuthority,
        network_id: NetworkId,
        header: iroha_data_model::block::BlockHeader,
        parent: Option<&V2FinalityArtifact>,
        payload_hash: Hash,
        execution_commitment: ExecutionCommitment,
    ) -> V2FinalityArtifact {
        let height = header.height().get();
        let context = HeightContext {
            network_id,
            protocol_version: iroha_data_model::block::consensus_v2::PROTOCOL_VERSION,
            height,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Npos,
            parent_commit_qc: parent.map(|artifact| artifact.commit_qc.clone()),
            snapshot_bootstrap: None,
            quorum: authority.quorum,
            roster: authority.roster.clone(),
            nexus_amx_context_hash: Hash::new(b"finalized outcome test nexus context"),
            execution_policy_hash: Hash::new(b"finalized outcome test execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1_024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4_096,
                max_chunk_count: 8,
            },
            leader_seed: [0xD5; 32],
        };
        let subject = BlockSubject {
            parent_block_hash: header.prev_block_hash(),
            block_hash: header.hash(),
            payload_hash,
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height,
            view: header.view_change_index(),
        };
        let mut commit_qc = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let preimage = commit_qc
            .signer_preimage(&context, 0)
            .expect("valid finality signer preimage");
        let signatures = commit_qc
            .signers
            .iter()
            .map(|index| {
                Signature::try_new(
                    authority.keys[usize::try_from(*index).expect("validator index")].private_key(),
                    &preimage,
                )
                .expect("sign finality vote")
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate finality votes");
        let artifact = V2FinalityArtifact::new(context, subject, commit_qc, authority.pops.clone());
        artifact.verify().expect("finality artifact verifies");
        artifact
            .validate_for_header(&header)
            .expect("finality artifact authenticates its block header");
        artifact
    }

    fn finality_chain_fixture(
        network_id: NetworkId,
        proof_count: usize,
    ) -> Vec<BridgeFinalityProof> {
        let authority = finality_fixture_authority();
        let mut proofs = Vec::with_capacity(proof_count);
        for offset in 0..proof_count {
            let height = u64::try_from(offset).expect("proof offset fits u64") + 1;
            let parent = proofs
                .last()
                .map(|proof: &BridgeFinalityProof| &proof.finality_artifact);
            let header = iroha_data_model::block::BlockHeader::new(
                NonZeroU64::new(height).expect("nonzero finality height"),
                parent.map(|artifact| artifact.block_hash),
                None,
                None,
                1_900_000_000_000 + height,
                0,
            );
            let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"finalized outcome test parent state"),
                Hash::new(b"finalized outcome test post state"),
                Hash::new(b"finalized outcome test ordinary writes"),
                1,
                Hash::new(b"finalized outcome test executed wire"),
            );
            let artifact = finality_artifact_fixture(
                &authority,
                network_id,
                header,
                parent,
                Hash::new(b"finalized outcome test payload"),
                execution_commitment,
            );
            proofs.push(BridgeFinalityProof {
                version: BRIDGE_FINALITY_PROOF_VERSION_V2,
                block_header: header,
                finality_artifact: artifact,
            });
        }
        proofs
    }

    #[test]
    fn finalized_page_is_canonical_content_addressed_and_checkpoint_pinned() {
        let (_, network_id, _, _, _) = fixture();
        let proofs = finality_chain_fixture(network_id, 2);
        let proof_archives = proofs
            .iter()
            .map(|proof| norito::encode_canonical(proof).expect("encode finality proof"))
            .collect::<Vec<_>>();
        let page = authenticated_finality_proof_page_bind_v1(&proof_archives)
            .expect("bind canonical finality page");
        assert_eq!(page.hash_hex, Hash::new(&page.archive).to_string());
        let trusted = &proofs[0].finality_artifact;
        let promoted = authenticated_finality_page_verify_v1(
            network_id.as_bytes(),
            trusted.height,
            trusted.context_id().0.as_ref(),
            &page.archive,
        )
        .expect("verify checkpoint-pinned page");
        assert_eq!(promoted.height, 2);
        assert_eq!(
            promoted.context_id,
            *proofs[1].finality_artifact.context_id().0.as_ref()
        );

        assert!(authenticated_finality_proof_page_bind_v1(&[]).is_err());
        assert!(
            authenticated_finality_page_verify_v1(
                network_id.as_bytes(),
                trusted.height + 1,
                trusted.context_id().0.as_ref(),
                &page.archive,
            )
            .is_err()
        );
        let mut corrupted = page.archive;
        let index = corrupted.len() / 2;
        corrupted[index] ^= 1;
        assert!(
            authenticated_finality_page_verify_v1(
                network_id.as_bytes(),
                trusted.height,
                trusted.context_id().0.as_ref(),
                &corrupted,
            )
            .is_err()
        );
    }

    #[test]
    fn finalized_page_rejects_self_mismatch_gap_reorder_duplicate_and_rollback() {
        let (_, network_id, _, _, _) = fixture();
        let proofs = finality_chain_fixture(network_id, 3);
        let archive = |proof: &BridgeFinalityProof| {
            norito::encode_canonical(proof).expect("encode finality proof")
        };
        let page = |selected: &[usize]| {
            authenticated_finality_proof_page_bind_v1(
                &selected
                    .iter()
                    .map(|index| archive(&proofs[*index]))
                    .collect::<Vec<_>>(),
            )
            .expect("bind structurally canonical proof page")
            .archive
        };
        let trusted = &proofs[0].finality_artifact;
        let verify = |page_archive: &[u8]| {
            authenticated_finality_page_verify_v1(
                network_id.as_bytes(),
                trusted.height,
                trusted.context_id().0.as_ref(),
                page_archive,
            )
        };

        assert!(verify(&page(&[0, 2])).is_err(), "height gap was accepted");
        assert!(
            verify(&page(&[0, 1, 0])).is_err(),
            "reordered proof was accepted"
        );
        assert!(
            verify(&page(&[0, 0])).is_err(),
            "duplicate proof was accepted"
        );
        assert!(
            authenticated_finality_page_verify_v1(
                network_id.as_bytes(),
                trusted.height,
                proofs[1].finality_artifact.context_id().0.as_ref(),
                &page(&[0, 1]),
            )
            .is_err(),
            "checkpoint context substitution was accepted",
        );
        assert!(
            authenticated_finality_page_verify_v1(
                network_id.as_bytes(),
                0,
                trusted.context_id().0.as_ref(),
                &page(&[0]),
            )
            .is_err(),
            "checkpoint rollback to height zero was accepted",
        );

        let mut self_mismatched = proofs[0].clone();
        self_mismatched.block_header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(2).expect("nonzero mismatched height"),
            None,
            None,
            None,
            1_900_000_000_002,
            0,
        );
        let self_mismatched_page =
            authenticated_finality_proof_page_bind_v1(&[archive(&self_mismatched)])
                .expect("bind canonical but internally mismatched proof");
        assert!(
            verify(&self_mismatched_page.archive).is_err(),
            "proof whose header/artifact heights disagree was accepted",
        );
    }

    #[test]
    fn finalized_page_enforces_count_individual_and_aggregate_byte_budgets() {
        assert!(
            validate_finality_proof_archive_lengths_v1(&vec![1; 65]).is_err(),
            "more than 64 proof bodies was accepted",
        );
        assert!(
            validate_finality_proof_archive_lengths_v1(&[
                AUTHENTICATED_FINALITY_PROOF_MAX_BYTES_V1 + 1,
            ])
            .is_err(),
            "an individually oversized proof was accepted",
        );
        assert!(
            validate_finality_proof_archive_lengths_v1(
                &[AUTHENTICATED_FINALITY_PROOF_MAX_BYTES_V1; 8]
            )
            .is_err(),
            "a page larger than 64 MiB was accepted",
        );
    }

    #[test]
    fn finalized_mobile_height_domain_is_positive_and_signed_long_bounded() {
        assert!(authenticated_finality_mobile_height_v1(0).is_err());
        assert_eq!(
            authenticated_finality_mobile_height_v1(AUTHENTICATED_FINALITY_MOBILE_MAX_HEIGHT_V1),
            Ok(AUTHENTICATED_FINALITY_MOBILE_MAX_HEIGHT_V1),
        );
        assert!(
            authenticated_finality_mobile_height_v1(
                AUTHENTICATED_FINALITY_MOBILE_MAX_HEIGHT_V1 + 1
            )
            .is_err()
        );
        assert!(
            AuthenticatedFinalityCheckpointProjectionV1 {
                height: AUTHENTICATED_FINALITY_MOBILE_MAX_HEIGHT_V1 + 1,
                context_id: *Hash::new(b"oversized mobile checkpoint").as_ref(),
            }
            .encode()
            .is_err(),
            "checkpoint encoding emitted a height that Java/Kotlin cannot represent",
        );
    }

    #[test]
    fn finalized_kagemusha_mobile_surface_keeps_verification_and_transport_explicit() {
        let verifier = include_str!("authenticated_transaction_details.rs");
        for required in [
            "BridgeFinalityVerifier::with_context",
            "TrustedBlockProofAnchor::from_untrusted_finality_artifact",
            "verify_inclusion_in_block",
            "merge_inclusion().is_some()",
            "trusted_checkpoint_height >= result_projection.committed_block_height",
            "AUTHENTICATED_FINALIZED_KAGEMUSHA_EVIDENCE_DOMAIN_V1",
        ] {
            assert!(
                verifier.contains(required),
                "missing finalized verifier binding: {required}"
            );
        }

        let android_bridge = include_str!(
            "../../../java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/AuthenticatedTransactionDetailsNativeBridge.java"
        );
        let sdk_bridge = include_str!(
            "../../../kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/AuthenticatedTransactionDetailsNativeBridge.kt"
        );
        for required in [
            "bindFinalityProofPageV1",
            "verifyFinalityPageV1",
            "bindTransactionDetailsCarrierV2",
            "projectFinalizedKagemushaOutcomeV1",
            "requireKagemushaTopUpFinalityAgreementV1",
        ] {
            assert!(
                android_bridge.contains(required),
                "missing Android API: {required}"
            );
            assert!(sdk_bridge.contains(required), "missing SDK API: {required}");
        }

        let android_transport = include_str!(
            "../../../java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransport.java"
        );
        let sdk_transport = include_str!(
            "../../../kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/HttpClientTransport.kt"
        );
        for required in [
            "getBridgeFinalityProofV1",
            "getAuthenticatedTransactionDetailsCarrierV2",
            "fetchExactNoritoBytesAllowingNotFound",
        ] {
            assert!(
                android_transport.contains(required),
                "missing Android transport: {required}"
            );
            assert!(
                sdk_transport.contains(required),
                "missing SDK transport: {required}"
            );
        }

        let c_header = include_str!("../include/connect_norito_bridge.h");
        for required in [
            "iroha_privacy_authenticated_transaction_details_prepare_v2",
            "iroha_privacy_authenticated_transaction_details_finalize_v2",
            "iroha_privacy_authenticated_finality_proof_page_bind_v1",
            "iroha_privacy_authenticated_finality_page_verify_v1",
            "iroha_privacy_authenticated_finalized_kagemusha_outcome_project_v1",
            "iroha_privacy_kagemusha_topup_finality_project_v4",
        ] {
            assert!(
                c_header.contains(required),
                "missing C ABI surface: {required}"
            );
        }

        let swift_bridge = include_str!(
            "../../../IrohaSwift/Sources/IrohaSwift/AuthenticatedFinalizedKagemushaOutcomeV1.swift"
        );
        for required in [
            "AuthenticatedFinalityCheckpointV1",
            "bindFinalityProofPageV1",
            "verifyFinalityPageV1",
            "projectFinalizedKagemushaOutcomeV1",
            "requireKagemushaTopUpFinalityAgreementV1",
        ] {
            assert!(
                swift_bridge.contains(required),
                "missing Swift API: {required}"
            );
        }
        let swift_transport =
            include_str!("../../../IrohaSwift/Sources/IrohaSwift/ToriiClient.swift");
        for required in [
            "getLedgerExecutedBlockWire",
            "getBridgeFinalityProofV1",
            "getAuthenticatedTransactionDetailsCarrierV2",
            "allowNotFound: true",
        ] {
            assert!(
                swift_transport.contains(required),
                "missing Swift exact transport: {required}"
            );
        }
    }

    #[test]
    fn finalized_exact12_rejection_surface_is_closed_across_mobile_sdks() {
        let verifier = include_str!("authenticated_transaction_details.rs");
        for required in [
            "action_index != 0",
            "result_projection.result_ok",
            "inspect_signed_privacy_exact12_action_v1",
            "verify_finality_page_v1",
            "finalized_checkpoint.height != result_projection.committed_block_height",
            "TrustedBlockProofAnchor::from_untrusted_finality_artifact",
            "verify_inclusion_in_block",
            "AUTHENTICATED_FINALIZED_PRIVACY_ACTION_REJECTION_EVIDENCE_DOMAIN_V1",
        ] {
            assert!(
                verifier.contains(required),
                "missing finalized Exact12 rejection binding: {required}"
            );
        }

        let c_header = include_str!("../include/connect_norito_bridge.h");
        let c_bridge = include_str!("lib.rs");
        for source in [c_header, c_bridge] {
            assert!(
                source
                    .contains("iroha_privacy_authenticated_finalized_action_rejection_project_v1"),
                "missing finalized Exact12 rejection C surface"
            );
        }

        let jni = include_str!("platform_jni/part_3.rs");
        for namespace in ["iroha_android_client", "iroha_sdk_client"] {
            assert!(
                jni.contains(&format!(
                    "Java_org_hyperledger_{namespace}_AuthenticatedPrivacyActionReceiptNativeBridge_nativeProjectFinalizedPrivacyActionRejectionV1"
                )),
                "missing finalized Exact12 rejection JNI surface for {namespace}"
            );
        }

        let swift = include_str!(
            "../../../IrohaSwift/Sources/IrohaSwift/AuthenticatedFinalizedKagemushaOutcomeV1.swift"
        );
        let kotlin = include_str!(
            "../../../kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/AuthenticatedPrivacyActionReceiptNativeBridge.kt"
        );
        let java = include_str!(
            "../../../java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/AuthenticatedPrivacyActionReceiptNativeBridge.java"
        );
        let java_codes = include_str!(
            "../../../java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/AuthenticatedPrivacyActionRejectionCodeV1.java"
        );
        for source in [swift, kotlin, java] {
            for required in [
                "AuthenticatedFinalizedPrivacyActionRejectionV1",
                "AuthenticatedPrivacyActionRejectionCodeV1",
                "projectFinalizedPrivacyActionRejectionV1",
            ] {
                assert!(
                    source.contains(required),
                    "missing finalized Exact12 rejection SDK surface: {required}"
                );
            }
        }
        for source in [swift, kotlin, java_codes] {
            for label in [
                "account_does_not_exist",
                "limit_check",
                "validation",
                "instruction_execution",
                "ivm_execution",
                "trigger_execution",
            ] {
                assert!(
                    source.contains(label),
                    "missing finalized Exact12 rejection label: {label}"
                );
            }
        }
    }

    #[test]
    fn native_query_finalizer_rejects_signature_and_preparation_mutations() {
        let (preparation, mut signature, _, _, _) = signed_preparation();
        signature[17] ^= 0x80;
        assert!(authenticated_transaction_details_finalize_v1(&preparation, &signature).is_err());
        assert!(
            authenticated_transaction_details_finalize_v1(
                &preparation,
                &vec![0xA5; AUTHENTICATED_TRANSACTION_DETAILS_SIGNATURE_MAX_BYTES_V1 + 1],
            )
            .is_err()
        );

        let (_, valid_signature, _, _, _) = signed_preparation();
        let mut changed_preparation = preparation;
        let index = changed_preparation.len() / 2;
        changed_preparation[index] ^= 0x01;
        assert!(
            authenticated_transaction_details_finalize_v1(&changed_preparation, &valid_signature,)
                .is_err()
        );
    }

    #[test]
    fn native_query_preparation_rejects_noncanonical_inputs() {
        let (_, network_id, authority, transaction_hash, nonce) = fixture();
        let uppercase_transaction_hash = transaction_hash.to_uppercase();
        assert_ne!(uppercase_transaction_hash, transaction_hash);
        assert!(
            authenticated_transaction_details_prepare_v1(
                network_id.as_bytes(),
                &authority,
                &uppercase_transaction_hash,
                1,
                nonce,
            )
            .is_err()
        );
        assert!(
            authenticated_transaction_details_prepare_v1(
                network_id.as_bytes(),
                &authority,
                &transaction_hash,
                1,
                [0; 32],
            )
            .is_err()
        );
    }

    fn committed_response_with_instruction(
        key_pair: &KeyPair,
        network_id: NetworkId,
        authority: &str,
        instructions: Vec<InstructionBox>,
        result: TransactionResultInner,
    ) -> (Vec<u8>, Vec<u8>) {
        let mut builder = TransactionBuilder::new(
            network_id,
            AccountId::parse_encoded(authority)
                .expect("authority")
                .into_account_id(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions);
        builder.set_creation_time(Duration::from_millis(1_900_000_000_000));
        builder.set_ttl(Duration::from_secs(30));
        let signature = Signature::try_new(key_pair.private_key(), &builder.payload_hash_bytes())
            .expect("sign external transaction payload hash");
        let signed = builder.build_with_signature(signature);
        let entrypoint = TransactionEntrypoint::External(signed);
        let header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(7).expect("height"),
            None,
            None,
            None,
            1_900_000_000_100,
            0,
        );
        let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
        let TransactionEntrypoint::External(signed) = entrypoint else {
            unreachable!()
        };
        builder.push_transaction(signed);
        builder.push_result(result);
        let block = builder.build(BTreeSet::new());
        committed_response_from_single_transaction_block(&block)
    }

    fn committed_response_from_single_transaction_block(block: &SignedBlock) -> (Vec<u8>, Vec<u8>) {
        let transaction = CommittedTransaction {
            block_hash: block.hash(),
            entrypoint_hash: block.entrypoint_hashes().next().expect("entrypoint hash"),
            entrypoint_proof: block.entrypoint_proofs().next().expect("entrypoint proof"),
            entrypoint: block.entrypoints_cloned().next().expect("entrypoint"),
            result_hash: block.result_hashes().next().expect("result hash"),
            result_proof: block.result_proofs().next().expect("result proof"),
            result: block.results().next().cloned().expect("result"),
            merge_inclusion: None,
        };
        let hash = transaction.entrypoint_hash().to_string();
        let response = PipelineTransactionDetailsResponse {
            hash: hash.clone(),
            block_height: block.header().height().get(),
            transaction,
            trigger_completions: Vec::new(),
        };
        (
            norito::encode_canonical(&response).expect("canonical response"),
            hash.into_bytes(),
        )
    }

    fn committed_response(
        key_pair: &KeyPair,
        network_id: NetworkId,
        authority: &str,
    ) -> (Vec<u8>, Vec<u8>) {
        committed_response_with_instruction(
            key_pair,
            network_id,
            authority,
            vec![Log::new(Level::INFO, "fixture".to_owned()).into()],
            Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted("policy denied fixture".to_owned()),
            )),
        )
    }

    struct FinalizedExact12RejectionFixture {
        preparation: Vec<u8>,
        response: Vec<u8>,
        requested_action_binding: [u8; AUTHENTICATED_FINALIZED_PRIVACY_ACTION_BINDING_BYTES_V1],
        network_id: [u8; Hash::LENGTH],
        trusted_checkpoint: [u8; AUTHENTICATED_FINALITY_CHECKPOINT_BYTES_V1],
        finality_page: Vec<u8>,
        executed_block_wire: Vec<u8>,
        expected_transaction_hash_hex: String,
        expected_transaction_authority: String,
    }

    fn finalized_exact12_rejection_fixture() -> FinalizedExact12RejectionFixture {
        let bundle = privacy_exact12_fixture_bundle_v1().expect("build canonical Exact12 bundle");
        let row = bundle.rows.first().expect("bundle contains ZK-ACE row");
        assert_eq!(
            row.protocol_id,
            PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1.protocol_id(),
        );
        let signed = iroha_data_model::transaction::SignedTransaction::decode_all_versioned(
            &row.signed_transaction_versioned_norito,
        )
        .expect("decode canonical signed ZK-ACE transaction");
        signed
            .verify_signature()
            .expect("canonical ZK-ACE transaction signature verifies");
        let network_id = *signed
            .network_id()
            .expect("Exact12 transaction has NetworkId");
        let transaction_authority = signed.authority().to_string();
        let inspected = inspect_signed_privacy_exact12_action_v1(
            &row.signed_transaction_versioned_norito,
            network_id.as_bytes(),
            &transaction_authority,
            0,
        )
        .expect("native inspection authenticates the Exact12 transaction");
        let inspected_bytes = inspected.to_fixed_bytes();
        let requested_action_binding = inspected_bytes[Hash::LENGTH..]
            .try_into()
            .expect("inspection contains the fixed 96-byte action binding");

        let finality_authority = finality_fixture_authority();
        let checkpoint_header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(1).expect("checkpoint height is nonzero"),
            None,
            None,
            None,
            1_900_000_000_001,
            0,
        );
        let checkpoint_execution = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"Exact12 checkpoint parent state"),
            Hash::new(b"Exact12 checkpoint post state"),
            Hash::new(b"Exact12 checkpoint ordinary writes"),
            1,
            Hash::new(b"Exact12 checkpoint executed wire"),
        );
        let checkpoint_artifact = finality_artifact_fixture(
            &finality_authority,
            network_id,
            checkpoint_header,
            None,
            Hash::new(b"Exact12 checkpoint proposal"),
            checkpoint_execution,
        );
        let checkpoint_proof = BridgeFinalityProof {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            block_header: checkpoint_header,
            finality_artifact: checkpoint_artifact,
        };

        let carrier_header = iroha_data_model::block::BlockHeader::new(
            NonZeroU64::new(2).expect("carrier height is nonzero"),
            Some(checkpoint_proof.finality_artifact.block_hash),
            None,
            None,
            1_900_000_000_002,
            0,
        );
        let mut carrier_builder =
            iroha_data_model::block::builder::BlockBuilder::new(carrier_header);
        carrier_builder.push_transaction(signed);
        carrier_builder.push_result(Err(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted("policy denied finalized Exact12 fixture".to_owned()),
        )));
        let carrier = carrier_builder.build(BTreeSet::new());
        let executed_block_wire = carrier
            .encode_wire()
            .expect("encode canonical Exact12 executed block wire");
        let carrier_execution = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"Exact12 carrier parent state"),
            Hash::new(b"Exact12 carrier post state"),
            Hash::new(b"Exact12 carrier ordinary writes"),
            u64::try_from(executed_block_wire.len()).expect("block wire length fits u64"),
            Hash::new(&executed_block_wire),
        );
        let carrier_artifact = finality_artifact_fixture(
            &finality_authority,
            network_id,
            carrier.header(),
            Some(&checkpoint_proof.finality_artifact),
            carrier
                .canonical_proposal_wire_hash()
                .expect("hash canonical Exact12 proposal wire"),
            carrier_execution,
        );
        let carrier_proof = BridgeFinalityProof {
            version: BRIDGE_FINALITY_PROOF_VERSION_V2,
            block_header: carrier.header(),
            finality_artifact: carrier_artifact,
        };
        let proof_archives = [&checkpoint_proof, &carrier_proof]
            .into_iter()
            .map(|proof| norito::encode_canonical(proof).expect("encode finality proof"))
            .collect::<Vec<_>>();
        let finality_page = authenticated_finality_proof_page_bind_v1(&proof_archives)
            .expect("bind exact finality page")
            .archive;
        let trusted_checkpoint = AuthenticatedFinalityCheckpointProjectionV1 {
            height: checkpoint_proof.finality_artifact.height,
            context_id: *checkpoint_proof.finality_artifact.context_id().0.as_ref(),
        }
        .encode()
        .expect("encode trusted checkpoint");

        let (response, transaction_hash) =
            committed_response_from_single_transaction_block(&carrier);
        let transaction_hash =
            String::from_utf8(transaction_hash).expect("transaction hash is UTF-8");
        assert_eq!(transaction_hash, hex::encode(inspected.transaction_hash));
        let (preparation, _) = authenticated_transaction_details_prepare_v2(
            network_id.as_bytes(),
            &transaction_authority,
            &transaction_authority,
            &transaction_hash,
            1_900_000_000_000,
            [0xC7; 32],
        )
        .expect("prepare authority-bound Exact12 transaction query");

        FinalizedExact12RejectionFixture {
            preparation,
            response,
            requested_action_binding,
            network_id: *network_id.as_bytes(),
            trusted_checkpoint,
            finality_page,
            executed_block_wire,
            expected_transaction_hash_hex: transaction_hash,
            expected_transaction_authority: transaction_authority,
        }
    }

    fn project_finalized_exact12_rejection_through_c(
        fixture: &FinalizedExact12RejectionFixture,
        requested_action_binding: &[u8],
        executed_block_wire: &[u8],
    ) -> (i32, *mut u8, c_ulong) {
        let mut output = ptr::null_mut();
        let mut output_len = 0;
        let status = unsafe {
            crate::iroha_privacy_authenticated_finalized_action_rejection_project_v1(
                fixture.preparation.as_ptr(),
                fixture.preparation.len() as c_ulong,
                fixture.response.as_ptr(),
                fixture.response.len() as c_ulong,
                0,
                0,
                requested_action_binding.as_ptr(),
                requested_action_binding.len() as c_ulong,
                fixture.network_id.as_ptr(),
                fixture.network_id.len() as c_ulong,
                fixture.trusted_checkpoint.as_ptr(),
                fixture.trusted_checkpoint.len() as c_ulong,
                fixture.finality_page.as_ptr(),
                fixture.finality_page.len() as c_ulong,
                executed_block_wire.as_ptr(),
                executed_block_wire.len() as c_ulong,
                &mut output,
                &mut output_len,
            )
        };
        (status, output, output_len)
    }

    #[test]
    fn finalized_exact12_rejection_c_abi_executes_positive_and_mutation_fixtures() {
        let fixture = finalized_exact12_rejection_fixture();
        let (status, output, output_len) = project_finalized_exact12_rejection_through_c(
            &fixture,
            &fixture.requested_action_binding,
            &fixture.executed_block_wire,
        );
        assert_eq!(status, 0, "real finalized Exact12 projection failed");
        assert!(!output.is_null());
        let json = unsafe { slice::from_raw_parts(output, output_len as usize) };
        let JsonValue::Object(fields) =
            norito::json::from_slice::<JsonValue>(json).expect("decode Exact12 projection JSON")
        else {
            panic!("Exact12 projection must be a JSON object");
        };
        assert_eq!(fields.len(), 22);
        assert_eq!(fields.get("version"), Some(&JsonValue::from(1_u64)));
        assert_eq!(
            fields.get("network_id_hex"),
            Some(&JsonValue::from(hex::encode(fixture.network_id))),
        );
        assert_eq!(
            fields.get("operation_schema"),
            Some(&JsonValue::from(
                PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1.canonical_label(),
            )),
        );
        assert_eq!(
            fields.get("protocol_id"),
            Some(&JsonValue::from(
                PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1
                    .protocol_id()
                    .canonical_label(),
            )),
        );
        assert_eq!(
            fields.get("ledger_effect_kind"),
            Some(&JsonValue::from(
                PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1
                    .ledger_effect_kind()
                    .canonical_label(),
            )),
        );
        assert_eq!(
            fields.get("transaction_hash_hex"),
            Some(&JsonValue::from(
                fixture.expected_transaction_hash_hex.clone(),
            )),
        );
        assert_eq!(fields.get("action_index"), Some(&JsonValue::from(0_u64)));
        assert_eq!(
            fields.get("transaction_intent_digest_hex"),
            Some(&JsonValue::from(hex::encode(
                &fixture.requested_action_binding[..Hash::LENGTH],
            ))),
        );
        assert_eq!(
            fields.get("statement_digest_hex"),
            Some(&JsonValue::from(hex::encode(
                &fixture.requested_action_binding[Hash::LENGTH..2 * Hash::LENGTH],
            ))),
        );
        assert_eq!(
            fields.get("proof_envelope_hash_hex"),
            Some(&JsonValue::from(hex::encode(
                &fixture.requested_action_binding[2 * Hash::LENGTH..],
            ))),
        );
        assert_eq!(
            fields.get("transaction_authority"),
            Some(&JsonValue::from(
                fixture.expected_transaction_authority.clone(),
            )),
        );
        assert_eq!(
            fields.get("query_authority"),
            Some(&JsonValue::from(
                fixture.expected_transaction_authority.clone(),
            )),
        );
        assert_eq!(
            fields.get("rejection_code"),
            Some(&JsonValue::from("validation")),
        );
        assert_eq!(
            fields.get("rejection_message"),
            Some(&JsonValue::from(
                "Operation is not permitted: policy denied finalized Exact12 fixture",
            )),
        );
        assert_eq!(
            fields.get("committed_block_height"),
            Some(&JsonValue::from("2")),
        );
        crate::iroha_privacy_free_buffer(output);

        let mut substituted_binding = fixture.requested_action_binding;
        substituted_binding[Hash::LENGTH] ^= 0x80;
        let (status, output, output_len) = project_finalized_exact12_rejection_through_c(
            &fixture,
            &substituted_binding,
            &fixture.executed_block_wire,
        );
        assert_ne!(status, 0, "statement-digest substitution was accepted");
        assert!(
            output.is_null(),
            "failed projection leaked an output buffer"
        );
        assert_eq!(output_len, 0, "failed projection retained an output length");

        let mut corrupted_wire = fixture.executed_block_wire.clone();
        let mutation_index = corrupted_wire.len() / 2;
        corrupted_wire[mutation_index] ^= 0x01;
        let (status, output, output_len) = project_finalized_exact12_rejection_through_c(
            &fixture,
            &fixture.requested_action_binding,
            &corrupted_wire,
        );
        assert_ne!(status, 0, "executed-block mutation was accepted");
        assert!(
            output.is_null(),
            "failed projection leaked an output buffer"
        );
        assert_eq!(output_len, 0, "failed projection retained an output length");
    }

    fn registration_instruction(authority: &str) -> InstructionBox {
        let assertion_key =
            p256::ecdsa::SigningKey::from_slice(&[0x5a; 32]).expect("fixture assertion key");
        let assertion_public_key = assertion_key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes()
            .to_vec();
        let public_key = KagemushaDevicePublicKeyV2::from_sec1_bytes(&assertion_public_key)
            .expect("fixture device key");
        let report = b"projection-only registration report".to_vec();
        let evidence = b"projection-only registration evidence".to_vec();
        RegisterOfflineDeviceAttestation::new(OfflineDeviceAttestationRegistration {
            version: 2,
            platform: "android-keymint".to_owned(),
            key_id: "projection-key".to_owned(),
            device_id: "projection-device".to_owned(),
            account_id: AccountId::parse_encoded(authority)
                .expect("fixture authority")
                .into_account_id(),
            asset_definition_id: None,
            ios_team_id: None,
            ios_bundle_id: None,
            ios_environment: None,
            android_package_name: Some("com.example.fixture".to_owned()),
            android_signing_certificate_sha256: Some(vec![0x11; 32]),
            android_attested_device_properties: None,
            public_key,
            assertion_scheme: "android-keymint-usage-count-1-v1".to_owned(),
            assertion_key_algorithm: "ecdsa-p256-sha256".to_owned(),
            assertion_public_key,
            assertion_usage_count_limit: Some(1),
            one_use: true,
            challenge_hash: Hash::new(b"projection challenge"),
            attestation_report_hash: Hash::new(&report),
            attestation_report: report,
            evidence_hash: Hash::new(&evidence),
            evidence,
            recent_block_height: 1,
            recent_block_hash: Hash::new(b"projection recent block"),
            expires_at_ms: 1_900_000_060_000,
        })
        .into()
    }

    #[test]
    fn response_projection_requires_exact_terminal_rejection_and_bindings() {
        let (key_pair, network_id, authority, _, nonce) = fixture();
        let (response, hash) = committed_response(&key_pair, network_id, &authority);
        let hash = String::from_utf8(hash).expect("hash utf8");
        let (preparation, _) = authenticated_transaction_details_prepare_v1(
            network_id.as_bytes(),
            &authority,
            &hash,
            1_900_000_000_000,
            nonce,
        )
        .expect("prepare exact query");
        let projection =
            authenticated_transaction_details_project_rejection_v1(&preparation, &response)
                .expect("project committed rejection");
        assert_eq!(projection.transaction_hash_hex, hash);
        assert_eq!(projection.transaction_authority, authority);
        assert_eq!(projection.rejection_code, "validation");
        assert!(
            projection
                .rejection_message
                .contains("policy denied fixture")
        );
        assert_eq!(projection.committed_block_height, 7);
        assert_eq!(projection.block_hash_hex.len(), 64);
        assert_eq!(projection.result_hash_hex.len(), 64);

        let canonical = norito::decode_canonical::<PipelineTransactionDetailsResponse>(&response)
            .expect("canonical response fixture");
        let reject_mutation = |mutated: PipelineTransactionDetailsResponse| {
            let encoded = norito::encode_canonical(&mutated).expect("canonical mutation");
            assert!(
                authenticated_transaction_details_project_rejection_v1(&preparation, &encoded,)
                    .is_err()
            );
        };

        let mut wrong_outer_hash = canonical.clone();
        wrong_outer_hash.hash = Hash::new(b"wrong outer transaction hash").to_string();
        reject_mutation(wrong_outer_hash);

        let mut zero_height = canonical.clone();
        zero_height.block_height = 0;
        reject_mutation(zero_height);

        let mut beyond_mobile_height = canonical.clone();
        beyond_mobile_height.block_height = AUTHENTICATED_FINALITY_MOBILE_MAX_HEIGHT_V1 + 1;
        reject_mutation(beyond_mobile_height);

        let mut wrong_result_hash = canonical.clone();
        wrong_result_hash.transaction.result_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"wrong committed transaction result"));
        reject_mutation(wrong_result_hash);

        let mut mismatched_proof_indexes = canonical.clone();
        mismatched_proof_indexes.transaction.result_proof =
            iroha_crypto::MerkleProof::from_audit_path(1, Vec::new());
        reject_mutation(mismatched_proof_indexes);

        let mut successful = canonical;
        successful.transaction.result = TransactionResult(Ok(Vec::new()), Vec::new());
        successful.transaction.result_hash = successful.transaction.result.hash();
        reject_mutation(successful);

        let (other_key_pair, _, other_authority, _, _) = fixture();
        let (other_response, other_hash) =
            committed_response(&other_key_pair, network_id, &other_authority);
        let other_hash = String::from_utf8(other_hash).expect("other hash utf8");
        let (wrong_authority_preparation, _) = authenticated_transaction_details_prepare_v1(
            network_id.as_bytes(),
            &authority,
            &other_hash,
            1_900_000_000_000,
            nonce,
        )
        .expect("prepare wrong-authority query");
        assert!(
            authenticated_transaction_details_project_rejection_v1(
                &wrong_authority_preparation,
                &other_response,
            )
            .is_err()
        );

        let mut other_network_bytes = [0xD3; 32];
        other_network_bytes[31] |= 1;
        let other_network =
            network_id_from_raw_bytes(&other_network_bytes).expect("other network id");
        let (wrong_network_response, wrong_network_hash) =
            committed_response(&key_pair, other_network, &authority);
        let wrong_network_hash =
            String::from_utf8(wrong_network_hash).expect("wrong-network hash utf8");
        let (wrong_network_preparation, _) = authenticated_transaction_details_prepare_v1(
            network_id.as_bytes(),
            &authority,
            &wrong_network_hash,
            1_900_000_000_000,
            nonce,
        )
        .expect("prepare wrong-network query");
        assert!(
            authenticated_transaction_details_project_rejection_v1(
                &wrong_network_preparation,
                &wrong_network_response,
            )
            .is_err()
        );

        let (wrong_signer, _, _, _, _) = fixture();
        let (invalid_signature_response, invalid_signature_hash) =
            committed_response(&wrong_signer, network_id, &authority);
        let invalid_signature_hash =
            String::from_utf8(invalid_signature_hash).expect("invalid-signature hash utf8");
        let (invalid_signature_preparation, _) = authenticated_transaction_details_prepare_v1(
            network_id.as_bytes(),
            &authority,
            &invalid_signature_hash,
            1_900_000_000_000,
            nonce,
        )
        .expect("prepare invalid-signature query");
        assert!(
            authenticated_transaction_details_project_rejection_v1(
                &invalid_signature_preparation,
                &invalid_signature_response,
            )
            .is_err()
        );

        assert!(
            authenticated_transaction_details_project_rejection_v1(
                &preparation,
                &response[..response.len() - 1],
            )
            .is_err()
        );
        let mut corrupted = response;
        let index = corrupted.len() - 1;
        corrupted[index] ^= 1;
        assert!(
            authenticated_transaction_details_project_rejection_v1(&preparation, &corrupted,)
                .is_err()
        );
    }

    fn project_registration_fixture(
        instructions: Vec<InstructionBox>,
        result: TransactionResultInner,
    ) -> Result<AuthenticatedOfflineDeviceRegistrationResultProjectionV1, &'static str> {
        let (key_pair, network_id, authority, _, nonce) = fixture();
        let (response, hash) = committed_response_with_instruction(
            &key_pair,
            network_id,
            &authority,
            instructions,
            result,
        );
        let hash = String::from_utf8(hash).expect("registration hash utf8");
        let (preparation, _) = authenticated_transaction_details_prepare_v1(
            network_id.as_bytes(),
            &authority,
            &hash,
            1_900_000_000_000,
            nonce,
        )
        .expect("prepare registration query");
        authenticated_offline_device_registration_result_project_v1(&preparation, &response)
    }

    #[test]
    fn offline_device_registration_projection_preserves_typed_terminal_decisions() {
        let (_, _, authority, _, _) = fixture();
        let drain_only = OfflineDeviceEligibilityRejectionV1::new_v1(
            OfflineDeviceEligibilityDecisionV1 {
                outcome: OfflineDeviceEligibilityOutcomeV1::DrainOnly,
                reason: OfflineDeviceEligibilityReasonV1::VulnerableFirmware,
                matched_rule_ids: vec!["samsung-reviewed-floor".to_owned()],
            },
            "reviewed firmware floor is not satisfied",
        )
        .expect("typed drain-only rejection");
        let projection = project_registration_fixture(
            vec![registration_instruction(&authority)],
            Err(TransactionRejectionReason::Validation(
                ValidationFail::InstructionFailed(
                    InstructionExecutionError::OfflineDeviceEligibility(drain_only.clone()),
                ),
            )),
        )
        .expect("project typed drain-only result");
        assert_eq!(
            projection.terminal_state,
            OfflineDeviceRegistrationTerminalStateV1::EligibilityRejected
        );
        assert_eq!(projection.eligibility_decision, Some(drain_only.decision));
        assert_eq!(
            projection.rejection_code.as_deref(),
            Some("offline_device_eligibility")
        );
        assert_eq!(
            projection.rejection_message.as_deref(),
            Some("reviewed firmware floor is not satisfied")
        );

        let cryptographic = OfflineDeviceEligibilityRejectionV1::new_v1(
            iroha_data_model::offline::OfflineDeviceAttestationPolicy::cryptographic_rejection_v1(),
            "verified boot was not authenticated",
        )
        .expect("typed cryptographic rejection");
        let projection = project_registration_fixture(
            vec![registration_instruction(&authority)],
            Err(TransactionRejectionReason::Validation(
                ValidationFail::InstructionFailed(
                    InstructionExecutionError::OfflineDeviceEligibility(cryptographic.clone()),
                ),
            )),
        )
        .expect("project typed cryptographic result");
        assert_eq!(
            projection.eligibility_decision,
            Some(cryptographic.decision)
        );

        let maximum_detail = "x".repeat(
            iroha_data_model::offline::OFFLINE_DEVICE_ELIGIBILITY_REJECTION_DETAIL_MAX_BYTES_V1,
        );
        let maximum_rejection = OfflineDeviceEligibilityRejectionV1::new_v1(
            iroha_data_model::offline::OfflineDeviceAttestationPolicy::cryptographic_rejection_v1(),
            maximum_detail.clone(),
        )
        .expect("maximum-size typed rejection detail");
        let projection = project_registration_fixture(
            vec![registration_instruction(&authority)],
            Err(TransactionRejectionReason::Validation(
                ValidationFail::InstructionFailed(
                    InstructionExecutionError::OfflineDeviceEligibility(maximum_rejection),
                ),
            )),
        )
        .expect("project maximum-size typed rejection detail");
        assert_eq!(projection.rejection_message, Some(maximum_detail));
    }

    #[test]
    fn offline_device_registration_projection_closes_instruction_and_result_shapes() {
        let (_, _, authority, _, _) = fixture();
        let applied = project_registration_fixture(
            vec![registration_instruction(&authority)],
            Ok(Vec::new()),
        )
        .expect("project applied registration");
        assert_eq!(
            applied.terminal_state,
            OfflineDeviceRegistrationTerminalStateV1::Applied
        );
        assert!(applied.eligibility_decision.is_none());
        assert!(applied.rejection_message.is_none());

        let other = project_registration_fixture(
            vec![registration_instruction(&authority)],
            Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted("controller permission is absent".to_owned()),
            )),
        )
        .expect("project unrelated rejection");
        assert_eq!(
            other.terminal_state,
            OfflineDeviceRegistrationTerminalStateV1::OtherRejected
        );
        assert_eq!(other.rejection_code.as_deref(), Some("validation"));
        assert!(other.eligibility_decision.is_none());

        assert!(
            project_registration_fixture(
                vec![Log::new(Level::INFO, "wrong instruction".to_owned()).into()],
                Ok(Vec::new()),
            )
            .is_err()
        );
        assert!(
            project_registration_fixture(
                vec![
                    registration_instruction(&authority),
                    Log::new(Level::INFO, "extra instruction".to_owned()).into(),
                ],
                Ok(Vec::new()),
            )
            .is_err()
        );

        let invalid = OfflineDeviceEligibilityRejectionV1 {
            decision: OfflineDeviceEligibilityDecisionV1 {
                outcome: OfflineDeviceEligibilityOutcomeV1::Eligible,
                reason: OfflineDeviceEligibilityReasonV1::PolicySatisfied,
                matched_rule_ids: Vec::new(),
            },
            detail: "eligible cannot be rejected".to_owned(),
        };
        assert!(
            project_registration_fixture(
                vec![registration_instruction(&authority)],
                Err(TransactionRejectionReason::Validation(
                    ValidationFail::InstructionFailed(
                        InstructionExecutionError::OfflineDeviceEligibility(invalid),
                    ),
                )),
            )
            .is_err()
        );
    }

    #[test]
    fn offline_device_registration_c_abi_emits_exact_closed_json() {
        let (key_pair, network_id, authority, _, nonce) = fixture();
        let rejection = OfflineDeviceEligibilityRejectionV1::new_v1(
            OfflineDeviceEligibilityDecisionV1 {
                outcome: OfflineDeviceEligibilityOutcomeV1::DrainOnly,
                reason: OfflineDeviceEligibilityReasonV1::VulnerableFirmware,
                matched_rule_ids: vec!["samsung-reviewed-floor".to_owned()],
            },
            "reviewed firmware floor is not satisfied",
        )
        .expect("typed registration rejection");
        let (response, hash) = committed_response_with_instruction(
            &key_pair,
            network_id,
            &authority,
            vec![registration_instruction(&authority)],
            Err(TransactionRejectionReason::Validation(
                ValidationFail::InstructionFailed(
                    InstructionExecutionError::OfflineDeviceEligibility(rejection),
                ),
            )),
        );
        let hash = String::from_utf8(hash).expect("registration hash utf8");
        let (preparation, _) = authenticated_transaction_details_prepare_v1(
            network_id.as_bytes(),
            &authority,
            &hash,
            1_900_000_000_000,
            nonce,
        )
        .expect("prepare registration query");
        let mut output = ptr::null_mut();
        let mut output_len = 0;
        assert_eq!(
            unsafe {
                crate::iroha_privacy_authenticated_offline_device_registration_result_project_v1(
                    preparation.as_ptr(),
                    preparation.len() as c_ulong,
                    response.as_ptr(),
                    response.len() as c_ulong,
                    &mut output,
                    &mut output_len,
                )
            },
            0
        );
        let json = unsafe { slice::from_raw_parts(output, output_len as usize) };
        let JsonValue::Object(fields) =
            norito::json::from_slice::<JsonValue>(json).expect("registration projection JSON")
        else {
            panic!("registration projection must be an object");
        };
        assert_eq!(fields.len(), 12);
        assert_eq!(fields.get("version"), Some(&JsonValue::from(1_u64)));
        assert_eq!(
            fields.get("terminal_state"),
            Some(&JsonValue::from("eligibility_rejected"))
        );
        assert_eq!(
            fields.get("eligibility_outcome"),
            Some(&JsonValue::from("drain_only"))
        );
        assert_eq!(
            fields.get("eligibility_reason"),
            Some(&JsonValue::from("vulnerable_firmware"))
        );
        assert_eq!(
            fields.get("matched_rule_ids"),
            Some(&JsonValue::Array(vec![JsonValue::from(
                "samsung-reviewed-floor"
            )]))
        );
        assert_eq!(
            fields.get("rejection_code"),
            Some(&JsonValue::from("offline_device_eligibility"))
        );
        assert_eq!(
            fields.get("rejection_message"),
            Some(&JsonValue::from("reviewed firmware floor is not satisfied"))
        );
        assert_eq!(
            fields.get("committed_block_height"),
            Some(&JsonValue::from("7"))
        );
        crate::iroha_privacy_free_buffer(output);
    }

    #[test]
    fn offline_device_registration_json_covers_valid_worst_case_escaping() {
        let (key_pair, network_id, authority, _, nonce) = fixture();
        let rule_count =
            iroha_data_model::offline::OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_VULNERABILITY_RULES_V2;
        let rule_bytes =
            iroha_data_model::offline::OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_RULE_ID_BYTES_V2;
        let matched_rule_ids = (0..rule_count)
            .map(|index| format!("{index:03}{}", "\\".repeat(rule_bytes - 3)))
            .collect();
        let detail = "\"".repeat(
            iroha_data_model::offline::OFFLINE_DEVICE_ELIGIBILITY_REJECTION_DETAIL_MAX_BYTES_V1,
        );
        let rejection = OfflineDeviceEligibilityRejectionV1::new_v1(
            OfflineDeviceEligibilityDecisionV1 {
                outcome: OfflineDeviceEligibilityOutcomeV1::DrainOnly,
                reason: OfflineDeviceEligibilityReasonV1::VulnerableFirmware,
                matched_rule_ids,
            },
            detail,
        )
        .expect("maximum valid escaping-heavy rejection");
        let (response, hash) = committed_response_with_instruction(
            &key_pair,
            network_id,
            &authority,
            vec![registration_instruction(&authority)],
            Err(TransactionRejectionReason::Validation(
                ValidationFail::InstructionFailed(
                    InstructionExecutionError::OfflineDeviceEligibility(rejection),
                ),
            )),
        );
        let hash = String::from_utf8(hash).expect("registration hash utf8");
        let (preparation, _) = authenticated_transaction_details_prepare_v1(
            network_id.as_bytes(),
            &authority,
            &hash,
            1_900_000_000_000,
            nonce,
        )
        .expect("prepare registration query");
        let json = crate::authenticated_offline_device_registration_result_json_v1(
            &preparation,
            &response,
        )
        .expect("project escaping-heavy registration result");
        assert!(json.len() > 64 * 1024);
        assert!(json.len() <= AUTHENTICATED_OFFLINE_DEVICE_REGISTRATION_RESULT_MAX_BYTES_V1);
        let JsonValue::Object(fields) =
            norito::json::from_slice::<JsonValue>(&json).expect("bounded registration JSON")
        else {
            panic!("registration projection must be an object");
        };
        assert!(matches!(
            fields.get("matched_rule_ids"),
            Some(JsonValue::Array(rules)) if rules.len() == rule_count
        ));
    }

    #[test]
    fn c_abi_prepares_finalizes_and_projects_exact_version_one_json() {
        let (key_pair, network_id, authority, _, nonce) = fixture();
        let (response, hash) = committed_response(&key_pair, network_id, &authority);
        let mut preparation_ptr = ptr::null_mut();
        let mut preparation_len = 0;
        let mut digest_ptr = ptr::null_mut();
        let mut digest_len = 0;
        assert_eq!(
            unsafe {
                crate::iroha_privacy_authenticated_transaction_details_prepare_v1(
                    network_id.as_bytes().as_ptr(),
                    network_id.as_bytes().len() as c_ulong,
                    authority.as_ptr(),
                    authority.len() as c_ulong,
                    hash.as_ptr(),
                    hash.len() as c_ulong,
                    1_900_000_000_000,
                    nonce.as_ptr(),
                    nonce.len() as c_ulong,
                    &mut preparation_ptr,
                    &mut preparation_len,
                    &mut digest_ptr,
                    &mut digest_len,
                )
            },
            0
        );
        assert!(!preparation_ptr.is_null());
        assert_eq!(digest_len as usize, Hash::LENGTH);
        let digest = unsafe { slice::from_raw_parts(digest_ptr, digest_len as usize) };
        let signature = Signature::try_new(key_pair.private_key(), digest).expect("sign digest");
        let mut signed_query_ptr = ptr::null_mut();
        let mut signed_query_len = 0;
        assert_eq!(
            unsafe {
                crate::iroha_privacy_authenticated_transaction_details_finalize_v1(
                    preparation_ptr,
                    preparation_len,
                    signature.payload().as_ptr(),
                    signature.payload().len() as c_ulong,
                    &mut signed_query_ptr,
                    &mut signed_query_len,
                )
            },
            0
        );
        let signed_query =
            unsafe { slice::from_raw_parts(signed_query_ptr, signed_query_len as usize) };
        SignedQuery::decode_all_versioned(signed_query)
            .expect("decode C ABI signed query")
            .verify_signature()
            .expect("verify C ABI signed query");

        let mut projection_ptr = ptr::null_mut();
        let mut projection_len = 0;
        assert_eq!(
            unsafe {
                crate::iroha_privacy_authenticated_transaction_details_project_result_v1(
                    preparation_ptr,
                    preparation_len,
                    response.as_ptr(),
                    response.len() as c_ulong,
                    &mut projection_ptr,
                    &mut projection_len,
                )
            },
            0
        );
        let projection = unsafe { slice::from_raw_parts(projection_ptr, projection_len as usize) };
        let JsonValue::Object(projection) =
            norito::json::from_slice::<JsonValue>(projection).expect("projection JSON")
        else {
            panic!("projection must be an object");
        };
        assert_eq!(projection.len(), 8);
        assert_eq!(projection.get("version"), Some(&JsonValue::from(1_u64)));
        assert_eq!(
            projection.get("transaction_authority"),
            Some(&JsonValue::from(authority))
        );
        assert_eq!(projection.get("result_ok"), Some(&JsonValue::from(false)));
        assert_eq!(
            projection.get("committed_block_height"),
            Some(&JsonValue::from("7"))
        );
        assert!(matches!(
            projection.get("rejection_message"),
            Some(JsonValue::String(message)) if message.contains("policy denied fixture")
        ));

        crate::iroha_privacy_free_buffer(projection_ptr);
        crate::iroha_privacy_free_buffer(signed_query_ptr);
        crate::iroha_privacy_free_buffer(digest_ptr);
        crate::iroha_privacy_free_buffer(preparation_ptr);
    }
}
