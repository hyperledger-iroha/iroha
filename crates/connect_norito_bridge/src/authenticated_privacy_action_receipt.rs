//! Native signed-query construction and finalized Exact12 receipt inspection for mobile SDKs.
//!
//! Managed callers receive only an opaque preparation plus its signing digest. Native code
//! validates the authority signature, emits the exact ID105 signed query, and accepts only the
//! canonical typed receipt bound to the requested action and the local inspection digests.

use iroha_crypto::{Hash, HashOf, SignatureOf};
use iroha_data_model::{
    account::AccountId,
    privacy::{PrivacyActionExecutionReceiptViewV1, PrivacyOperationSchemaV1, PrivacyProtocolIdV1},
    query::{
        QueryRequest, QueryRequestWithAuthority, QueryResponse, QuerySignature, SignedQuery,
        SingularQueryBox, SingularQueryOutputBox,
        privacy::prelude::FindPrivacyActionExecutionReceiptV1,
    },
    transaction::TransactionEntrypoint,
};
use iroha_version::codec::EncodeVersioned as _;
use norito::{NoritoDeserialize, NoritoSerialize};
use std::{num::NonZeroU64, str::FromStr as _};

use super::{
    authenticated_transaction_details::canonical_authority, connect_signature_from_algorithm_bytes,
    network_id_from_raw_bytes,
};

pub(super) const AUTHENTICATED_ACTION_RECEIPT_PREPARATION_MAX_BYTES_V1: usize = 64 * 1024;
pub(super) const AUTHENTICATED_ACTION_RECEIPT_RESPONSE_MAX_BYTES_V1: usize = 256 * 1024;
pub(super) const AUTHENTICATED_ACTION_RECEIPT_SIGNATURE_MAX_BYTES_V1: usize = 16 * 1024;
const AUTHENTICATED_ACTION_RECEIPT_SIGNED_QUERY_MAX_BYTES_V1: usize = 64 * 1024;
pub(super) const AUTHENTICATED_ACTION_RECEIPT_QUERY_TTL_MS_V1: u64 = 100_000;
pub(super) const AUTHENTICATED_ACTION_RECEIPT_BINDING_BYTES_V1: usize = 96;
const AUTHENTICATED_ACTION_RECEIPT_PREPARATION_VERSION_V1: u8 = 1;
type RequestedActionBindingV1 = ([u8; 32], [u8; 32], [u8; 32]);

#[derive(NoritoSerialize, NoritoDeserialize)]
struct AuthenticatedPrivacyActionReceiptPreparationV1 {
    version: u8,
    authority_literal: String,
    operation: PrivacyOperationSchemaV1,
    expected_transaction_hash: [u8; 32],
    action_index: u32,
    transaction_intent_digest: [u8; 32],
    statement_digest: [u8; 32],
    proof_envelope_hash: [u8; 32],
    payload: QueryRequestWithAuthority,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AuthenticatedPrivacyActionReceiptProjectionV1 {
    pub version: u16,
    pub network_id_hex: String,
    pub protocol_id: &'static str,
    pub operation_schema: &'static str,
    pub ledger_effect_kind: &'static str,
    pub transaction_hash_hex: String,
    pub action_index: u32,
    pub transaction_intent_digest_hex: String,
    pub statement_digest_hex: String,
    pub proof_envelope_hash_hex: String,
    pub capability_manifest_digest_hex: String,
    pub capability_committed_height: u64,
    pub admitted_at_height: u64,
    pub finalized_height: u64,
    pub finalized_block_hash_hex: String,
}

fn operation_from_index(index: i32) -> Option<PrivacyOperationSchemaV1> {
    Some(match index {
        0 => PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1,
        1 => PrivacyOperationSchemaV1::AnonymousPgcPaymentActionV1,
        2 => PrivacyOperationSchemaV1::VeRangeRangeProofV1,
        3 => PrivacyOperationSchemaV1::ZkAmsBatchAdmissionActionV1,
        4 => PrivacyOperationSchemaV1::ZkAmsProvisionAccountActionV1,
        5 => PrivacyOperationSchemaV1::VegaCredentialPresentationV1,
        6 => PrivacyOperationSchemaV1::ZkX509IdentityPresentationV1,
        7 => PrivacyOperationSchemaV1::JindoPolynomialEvaluationV1,
        8 => PrivacyOperationSchemaV1::BootleLanternCredentialPresentationV1,
        9 => PrivacyOperationSchemaV1::OrchardNoteActionV1,
        10 => PrivacyOperationSchemaV1::FcmpMembershipPaymentV1,
        11 => PrivacyOperationSchemaV1::IvmPrivateNoteActionV1,
        12 => PrivacyOperationSchemaV1::PqMaspNoteActionV1,
        _ => return None,
    })
}

fn canonical_transaction_hash(transaction_hash_hex: &str) -> Result<[u8; 32], &'static str> {
    if transaction_hash_hex.len() != Hash::LENGTH * 2
        || !transaction_hash_hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("transactionHashHex must be exactly 64 lowercase hexadecimal characters");
    }
    let hash = Hash::from_str(transaction_hash_hex)
        .map_err(|_| "transactionHashHex must be a canonical marked Iroha hash")?;
    Ok(*HashOf::<TransactionEntrypoint>::from_untyped_unchecked(hash).as_ref())
}

fn requested_action_binding(bytes: &[u8]) -> Result<RequestedActionBindingV1, &'static str> {
    if bytes.len() != AUTHENTICATED_ACTION_RECEIPT_BINDING_BYTES_V1 {
        return Err("requested action binding must contain exactly 96 bytes");
    }
    let intent = <[u8; 32]>::try_from(&bytes[0..32])
        .map_err(|_| "transaction-intent digest must contain exactly 32 bytes")?;
    let statement = <[u8; 32]>::try_from(&bytes[32..64])
        .map_err(|_| "statement digest must contain exactly 32 bytes")?;
    let envelope = <[u8; 32]>::try_from(&bytes[64..96])
        .map_err(|_| "proof-envelope hash must contain exactly 32 bytes")?;
    if intent == [0; 32] || statement == [0; 32] || envelope == [0; 32] {
        return Err("requested action binding digests must be nonzero");
    }
    Ok((intent, statement, envelope))
}

fn exact_query(
    operation: PrivacyOperationSchemaV1,
    transaction_hash: [u8; 32],
    action_index: u32,
) -> QueryRequest {
    QueryRequest::Singular(
        FindPrivacyActionExecutionReceiptV1::new(
            operation.protocol_id(),
            transaction_hash,
            action_index,
        )
        .into(),
    )
}

fn exact_query_binding(request: &QueryRequest) -> Option<(PrivacyProtocolIdV1, [u8; 32], u32)> {
    let QueryRequest::Singular(SingularQueryBox::FindPrivacyActionExecutionReceiptV1(query)) =
        request
    else {
        return None;
    };
    Some((
        query.protocol_id(),
        query.transaction_hash(),
        query.action_index(),
    ))
}

fn validate_preparation(
    preparation: &AuthenticatedPrivacyActionReceiptPreparationV1,
) -> Result<(), &'static str> {
    if preparation.version != AUTHENTICATED_ACTION_RECEIPT_PREPARATION_VERSION_V1 {
        return Err("unsupported authenticated action-receipt preparation version");
    }
    let authority: AccountId = canonical_authority(&preparation.authority_literal)?;
    if preparation.payload.authority() != &authority
        || preparation.payload.creation_time_ms() == 0
        || preparation.payload.time_to_live_ms().get()
            != AUTHENTICATED_ACTION_RECEIPT_QUERY_TTL_MS_V1
        || preparation.payload.nonce() == &[0; 32]
        || preparation.action_index != 0
        || preparation.transaction_intent_digest == [0; 32]
        || preparation.statement_digest == [0; 32]
        || preparation.proof_envelope_hash == [0; 32]
        || exact_query_binding(preparation.payload.request())
            != Some((
                preparation.operation.protocol_id(),
                preparation.expected_transaction_hash,
                preparation.action_index,
            ))
    {
        return Err("authenticated action-receipt preparation binding is invalid");
    }
    Ok(())
}

fn decode_preparation(
    preparation: &[u8],
) -> Result<AuthenticatedPrivacyActionReceiptPreparationV1, &'static str> {
    if preparation.is_empty()
        || preparation.len() > AUTHENTICATED_ACTION_RECEIPT_PREPARATION_MAX_BYTES_V1
    {
        return Err("preparation archive is outside its closed byte bound");
    }
    let decoded =
        norito::decode_canonical::<AuthenticatedPrivacyActionReceiptPreparationV1>(preparation)
            .map_err(|_| "preparation archive is not canonical Norito")?;
    validate_preparation(&decoded)?;
    Ok(decoded)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the receipt preparation keeps every signed and inspected binding explicit"
)]
pub(super) fn authenticated_privacy_action_receipt_prepare_v1(
    network_id: &[u8],
    authority_literal: &str,
    operation_index: i32,
    transaction_hash_hex: &str,
    action_index: u32,
    requested_binding: &[u8],
    creation_time_ms: u64,
    nonce: [u8; 32],
) -> Result<(Vec<u8>, [u8; 32]), &'static str> {
    if creation_time_ms == 0 || nonce == [0; 32] {
        return Err("query freshness must be positive and nonzero");
    }
    let network_id = network_id_from_raw_bytes(network_id)?;
    let authority = canonical_authority(authority_literal)?;
    let operation = operation_from_index(operation_index)
        .ok_or("Exact12 operation discriminant is outside the closed union")?;
    let expected_transaction_hash = canonical_transaction_hash(transaction_hash_hex)?;
    let (transaction_intent_digest, statement_digest, proof_envelope_hash) =
        requested_action_binding(requested_binding)?;
    let payload = exact_query(operation, expected_transaction_hash, action_index).with_authority(
        network_id,
        authority,
        creation_time_ms,
        NonZeroU64::new(AUTHENTICATED_ACTION_RECEIPT_QUERY_TTL_MS_V1)
            .expect("authenticated query TTL is nonzero"),
        nonce,
    );
    let preparation = AuthenticatedPrivacyActionReceiptPreparationV1 {
        version: AUTHENTICATED_ACTION_RECEIPT_PREPARATION_VERSION_V1,
        authority_literal: authority_literal.to_owned(),
        operation,
        expected_transaction_hash,
        action_index,
        transaction_intent_digest,
        statement_digest,
        proof_envelope_hash,
        payload,
    };
    validate_preparation(&preparation)?;
    let signing_digest = *HashOf::new(&preparation.payload).as_ref();
    let archive = norito::encode_canonical(&preparation)
        .map_err(|_| "failed to encode canonical action-receipt preparation")?;
    if archive.len() > AUTHENTICATED_ACTION_RECEIPT_PREPARATION_MAX_BYTES_V1 {
        return Err("encoded preparation archive exceeds its closed byte bound");
    }
    Ok((archive, signing_digest))
}

pub(super) fn authenticated_privacy_action_receipt_finalize_v1(
    preparation: &[u8],
    signature_bytes: &[u8],
) -> Result<Vec<u8>, &'static str> {
    if signature_bytes.is_empty()
        || signature_bytes.len() > AUTHENTICATED_ACTION_RECEIPT_SIGNATURE_MAX_BYTES_V1
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
    if body.is_empty() || body.len() > AUTHENTICATED_ACTION_RECEIPT_SIGNED_QUERY_MAX_BYTES_V1 {
        return Err("final signed query violates its closed byte bound");
    }
    Ok(body)
}

fn project_receipt(
    receipt: PrivacyActionExecutionReceiptViewV1,
) -> AuthenticatedPrivacyActionReceiptProjectionV1 {
    AuthenticatedPrivacyActionReceiptProjectionV1 {
        version: receipt.version,
        network_id_hex: hex::encode(receipt.network_id.as_bytes()),
        protocol_id: receipt.protocol_id.canonical_label(),
        operation_schema: receipt.operation_schema.canonical_label(),
        ledger_effect_kind: receipt.ledger_effect_kind.canonical_label(),
        transaction_hash_hex: hex::encode(receipt.transaction_hash),
        action_index: receipt.action_index,
        transaction_intent_digest_hex: hex::encode(receipt.transaction_intent_digest.as_bytes()),
        statement_digest_hex: hex::encode(receipt.statement_digest.as_bytes()),
        proof_envelope_hash_hex: hex::encode(receipt.proof_envelope_hash),
        capability_manifest_digest_hex: hex::encode(receipt.capability_manifest_digest.as_bytes()),
        capability_committed_height: receipt.capability_committed_height,
        admitted_at_height: receipt.admitted_at_height,
        finalized_height: receipt.finalized_height,
        finalized_block_hash_hex: receipt.finalized_block_hash.to_string(),
    }
}

pub(super) fn authenticated_privacy_action_receipt_project_result_v1(
    preparation: &[u8],
    response: &[u8],
) -> Result<AuthenticatedPrivacyActionReceiptProjectionV1, &'static str> {
    let preparation = decode_preparation(preparation)?;
    if response.is_empty() || response.len() > AUTHENTICATED_ACTION_RECEIPT_RESPONSE_MAX_BYTES_V1 {
        return Err("action-receipt response is outside its closed byte bound");
    }
    let decoded: QueryResponse = norito::decode_canonical_with_limits(
        response,
        norito::canonical_decode_limits(response.len()),
    )
    .map_err(|_| "action-receipt response is not canonical Norito")?;
    let canonical = norito::to_bytes(&decoded)
        .map_err(|_| "action-receipt response could not be re-encoded")?;
    if canonical != response {
        return Err("action-receipt response is not its exact canonical wire");
    }
    let QueryResponse::Singular(SingularQueryOutputBox::PrivacyActionExecutionReceiptViewV1(
        receipt,
    )) = decoded
    else {
        return Err("action-receipt query returned an unexpected typed response");
    };
    receipt
        .validate()
        .map_err(|_| "action receipt failed its native validation")?;
    if receipt.network_id != preparation.payload.network_id()
        || receipt.protocol_id != preparation.operation.protocol_id()
        || receipt.operation_schema != preparation.operation
        || receipt.ledger_effect_kind != preparation.operation.ledger_effect_kind()
        || receipt.transaction_hash != preparation.expected_transaction_hash
        || receipt.action_index != preparation.action_index
        || receipt.transaction_intent_digest.as_bytes() != &preparation.transaction_intent_digest
        || receipt.statement_digest.as_bytes() != &preparation.statement_digest
        || receipt.proof_envelope_hash != preparation.proof_envelope_hash
    {
        return Err("action receipt differs from the requested Exact12 action");
    }
    Ok(project_receipt(receipt))
}
