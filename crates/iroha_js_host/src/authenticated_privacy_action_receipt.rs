//! Native signed-query construction and finalized Exact12 receipt inspection for JavaScript.
//!
//! A successful pipeline result is not sufficient evidence that the requested privacy action
//! executed its native semantics. This boundary accepts only the finalized typed receipt written
//! by Core in the same state transaction as the action's ledger effect.

use iroha_crypto::HashOf;
use iroha_data_model::{
    NetworkId,
    privacy::{
        PrivacyActionExecutionReceiptViewV1, PrivacyExact12ActionOperationV1,
        PrivacyLedgerEffectKindV1, PrivacyProtocolIdV1,
    },
    query::{
        QueryRequest, QueryResponse, SingularQueryOutputBox,
        privacy::prelude::FindPrivacyActionExecutionReceiptV1,
    },
    transaction::TransactionEntrypoint,
};

use super::{
    authenticated_transaction_details::{build_signed_request_v1, canonical_lower_hash},
    parse_transaction_network_id_bytes,
    privacy_exact12_action::operation_from_index,
};

pub(crate) const RECEIPT_RESPONSE_MAX_BYTES_V1: usize = 256 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExecutionReceiptProjectionV1 {
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

fn expected_operation(operation_index: u32) -> Result<PrivacyExact12ActionOperationV1, String> {
    operation_from_index(operation_index)
        .ok_or_else(|| "Exact12 operation discriminant is outside the closed union".to_owned())
}

fn exact_receipt_query(
    operation: PrivacyExact12ActionOperationV1,
    transaction_hash: HashOf<TransactionEntrypoint>,
    action_index: u32,
) -> QueryRequest {
    QueryRequest::Singular(
        FindPrivacyActionExecutionReceiptV1::new(
            operation.protocol_id(),
            *transaction_hash.as_ref(),
            action_index,
        )
        .into(),
    )
}

fn exact_nonzero_32(bytes: &[u8], label: &str) -> Result<[u8; 32], String> {
    let exact: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("{label} must contain exactly 32 bytes"))?;
    if exact.iter().all(|byte| *byte == 0) {
        return Err(format!("{label} must not be all zero"));
    }
    Ok(exact)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RequestedReceiptBindingV1 {
    transaction_intent_digest: [u8; 32],
    statement_digest: [u8; 32],
    proof_envelope_hash: [u8; 32],
}

fn exact_requested_receipt_binding_v1(bytes: &[u8]) -> Result<RequestedReceiptBindingV1, String> {
    if bytes.len() != 96 {
        return Err("requested Exact12 receipt binding must contain exactly 96 bytes".to_owned());
    }
    Ok(RequestedReceiptBindingV1 {
        transaction_intent_digest: exact_nonzero_32(
            &bytes[0..32],
            "requested transaction-intent digest",
        )?,
        statement_digest: exact_nonzero_32(&bytes[32..64], "requested statement digest")?,
        proof_envelope_hash: exact_nonzero_32(&bytes[64..96], "requested proof-envelope hash")?,
    })
}

pub(crate) fn build_signed_query_v1(
    authority_literal: &str,
    private_key_bytes: &[u8],
    network_id_bytes: &[u8],
    operation_index: u32,
    transaction_hash_hex: &str,
    action_index: u32,
) -> Result<Vec<u8>, String> {
    let operation = expected_operation(operation_index)?;
    let transaction_hash = canonical_lower_hash(transaction_hash_hex)?;
    build_signed_request_v1(
        authority_literal,
        private_key_bytes,
        network_id_bytes,
        exact_receipt_query(operation, transaction_hash, action_index),
    )
}

fn project_receipt(receipt: PrivacyActionExecutionReceiptViewV1) -> ExecutionReceiptProjectionV1 {
    ExecutionReceiptProjectionV1 {
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

pub(crate) fn inspect_receipt_v1(
    network_id_bytes: &[u8],
    operation_index: u32,
    transaction_hash_hex: &str,
    action_index: u32,
    requested_action_binding: &[u8],
    response: &[u8],
) -> Result<ExecutionReceiptProjectionV1, String> {
    let expected_network_id: NetworkId = parse_transaction_network_id_bytes(network_id_bytes)
        .map_err(|error| error.reason.clone())?;
    let expected_operation = expected_operation(operation_index)?;
    let expected_protocol: PrivacyProtocolIdV1 = expected_operation.protocol_id();
    let expected_effect: PrivacyLedgerEffectKindV1 = expected_operation.ledger_effect_kind();
    let expected_transaction_hash = canonical_lower_hash(transaction_hash_hex)?;
    let expected_binding = exact_requested_receipt_binding_v1(requested_action_binding)?;
    if response.is_empty() || response.len() > RECEIPT_RESPONSE_MAX_BYTES_V1 {
        return Err(
            "Exact12 execution-receipt response is outside its closed byte bound".to_owned(),
        );
    }
    let decoded: QueryResponse = norito::decode_canonical_with_limits(
        response,
        norito::canonical_decode_limits(response.len()),
    )
    .map_err(|_| "Exact12 execution-receipt response is not canonical Norito".to_owned())?;
    let canonical = norito::to_bytes(&decoded)
        .map_err(|_| "Exact12 execution-receipt response could not be re-encoded".to_owned())?;
    if canonical != response {
        return Err(
            "Exact12 execution-receipt response is not its exact canonical wire".to_owned(),
        );
    }
    let QueryResponse::Singular(SingularQueryOutputBox::PrivacyActionExecutionReceiptViewV1(
        receipt,
    )) = decoded
    else {
        return Err(
            "Exact12 execution-receipt query returned an unexpected typed response".to_owned(),
        );
    };
    receipt
        .validate()
        .map_err(|_| "Exact12 execution receipt failed its native validation".to_owned())?;
    if receipt.network_id != expected_network_id
        || receipt.protocol_id != expected_protocol
        || receipt.operation_schema != expected_operation
        || receipt.ledger_effect_kind != expected_effect
        || receipt.transaction_hash != *expected_transaction_hash.as_ref()
        || receipt.action_index != action_index
        || receipt.transaction_intent_digest.as_bytes()
            != &expected_binding.transaction_intent_digest
        || receipt.statement_digest.as_bytes() != &expected_binding.statement_digest
        || receipt.proof_envelope_hash != expected_binding.proof_envelope_hash
    {
        return Err("Exact12 execution receipt differs from the requested action".to_owned());
    }
    Ok(project_receipt(receipt))
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        NetworkId,
        block::BlockHeader,
        privacy::{
            PRIVACY_ACTION_EXECUTION_RECEIPT_VIEW_VERSION_V1, PrivacyActionExecutionReceiptViewV1,
            PrivacyExact12CapabilityManifestDigestV1, PrivacyOperationSchemaV1,
            PrivacyStatementDigestV1, PrivacyTransactionIntentDigestV1,
        },
        query::{QueryResponse, SingularQueryOutputBox},
        transaction::TransactionEntrypoint,
    };

    use super::inspect_receipt_v1;

    fn receipt_fixture() -> (
        NetworkId,
        HashOf<TransactionEntrypoint>,
        PrivacyActionExecutionReceiptViewV1,
    ) {
        let operation = PrivacyOperationSchemaV1::AnonymousPgcPaymentActionV1;
        let genesis_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x71; 32]));
        let network_id = NetworkId::from_genesis_hash(genesis_hash);
        let transaction_hash =
            HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed([0x11; 32]));
        let receipt = PrivacyActionExecutionReceiptViewV1 {
            version: PRIVACY_ACTION_EXECUTION_RECEIPT_VIEW_VERSION_V1,
            network_id,
            protocol_id: operation.protocol_id(),
            operation_schema: operation,
            ledger_effect_kind: operation.ledger_effect_kind(),
            transaction_hash: *transaction_hash.as_ref(),
            action_index: 0,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0x22; 32]),
            statement_digest: PrivacyStatementDigestV1::new([0x33; 32]),
            proof_envelope_hash: [0x44; 32],
            capability_manifest_digest: PrivacyExact12CapabilityManifestDigestV1::new([0x55; 32]),
            capability_committed_height: 40,
            admitted_at_height: 41,
            finalized_height: 42,
            finalized_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [0x72; 32],
            )),
        };
        (network_id, transaction_hash, receipt)
    }

    fn requested_binding(receipt: &PrivacyActionExecutionReceiptViewV1) -> [u8; 96] {
        let mut binding = [0_u8; 96];
        binding[0..32].copy_from_slice(receipt.transaction_intent_digest.as_bytes());
        binding[32..64].copy_from_slice(receipt.statement_digest.as_bytes());
        binding[64..96].copy_from_slice(&receipt.proof_envelope_hash);
        binding
    }

    #[test]
    fn inspector_accepts_only_the_exact_finalized_typed_receipt() {
        let (network_id, transaction_hash, receipt) = receipt_fixture();
        let response = QueryResponse::Singular(
            SingularQueryOutputBox::PrivacyActionExecutionReceiptViewV1(receipt),
        );
        let wire = norito::to_bytes(&response).expect("encode receipt query response");
        let binding = requested_binding(&receipt);
        let projection = inspect_receipt_v1(
            network_id.as_bytes(),
            1,
            &transaction_hash.to_string(),
            0,
            &binding,
            &wire,
        )
        .expect("inspect exact finalized receipt");
        assert_eq!(
            projection.operation_schema,
            "anonymous_pgc_payment_action_v1"
        );
        assert_eq!(
            projection.ledger_effect_kind,
            "anonymous_pgc_account_state_transition"
        );
        assert_eq!(
            projection.transaction_hash_hex,
            transaction_hash.to_string()
        );
        assert_eq!(projection.admitted_at_height, 41);
        assert_eq!(projection.finalized_height, 42);

        assert!(
            inspect_receipt_v1(
                network_id.as_bytes(),
                0,
                &transaction_hash.to_string(),
                0,
                &binding,
                &wire,
            )
            .is_err(),
            "another Exact12 operation must not accept the receipt",
        );
        let mut trailing = wire.clone();
        trailing.push(0);
        assert!(
            inspect_receipt_v1(
                network_id.as_bytes(),
                1,
                &transaction_hash.to_string(),
                0,
                &binding,
                &trailing,
            )
            .is_err(),
            "trailing response bytes must fail closed",
        );
        let mut another_statement = binding;
        another_statement[32..64].fill(0x99);
        assert!(
            inspect_receipt_v1(
                network_id.as_bytes(),
                1,
                &transaction_hash.to_string(),
                0,
                &another_statement,
                &wire,
            )
            .is_err(),
            "another requested statement must not accept the receipt",
        );
    }
}
