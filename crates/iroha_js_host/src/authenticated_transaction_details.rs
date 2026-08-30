//! Native signed-query construction and committed-result inspection for JavaScript.
//!
//! The request is a fresh exact-hash `FindTransactions` query. The response inspector accepts
//! only canonical Norito and binds the returned external transaction, authority, `NetworkId`,
//! result hash, and committed height. TLS plus the signed query authenticate the Torii exchange;
//! independent block finality still requires the finalized-block proof API.

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PrivateKey};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    query::{
        CommittedTransaction, Query as _, QueryItemKind, QueryRequest, QueryWithParams,
        dsl::{CommittedTxPredicate, CompoundPredicate, SelectorTuple},
        parameters::QueryParams,
        transaction::prelude::FindTransactions,
    },
    transaction::{TransactionEntrypoint, error::TransactionRejectionReason},
};
use iroha_torii_shared::PipelineTransactionDetailsResponse;
use iroha_version::codec::EncodeVersioned as _;
use norito::codec::Encode as _;
use rand_core_06::{OsRng, RngCore as _};
use std::{
    num::NonZeroU64,
    str::FromStr as _,
    time::{SystemTime, UNIX_EPOCH},
};

use super::parse_transaction_network_id_bytes;

pub(crate) const RESPONSE_MAX_BYTES_V1: usize = 64 * 1024 * 1024;
const SIGNED_QUERY_MAX_BYTES_V1: usize = 64 * 1024;
const QUERY_TTL_MS_V1: u64 = 100_000;
const AUTHORITY_MAX_BYTES_V1: usize =
    2 * iroha_data_model::musubi::MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1;
const REJECTION_MESSAGE_MAX_BYTES_V1: usize = 1_024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommittedResultProjectionV1 {
    pub transaction_hash_hex: String,
    pub block_hash_hex: String,
    pub result_hash_hex: String,
    pub result_ok: bool,
    pub rejection_code: Option<&'static str>,
    pub rejection_message: Option<String>,
    pub committed_block_height: u64,
}

/// Parse one exact lowercase transaction hash used by authenticated query helpers.
pub(crate) fn canonical_lower_hash(
    transaction_hash_hex: &str,
) -> Result<HashOf<TransactionEntrypoint>, String> {
    if transaction_hash_hex.len() != Hash::LENGTH * 2
        || !transaction_hash_hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(
            "transactionHashHex must be exactly 64 lowercase hexadecimal characters".to_owned(),
        );
    }
    let hash = Hash::from_str(transaction_hash_hex)
        .map_err(|_| "transactionHashHex must be a canonical marked Iroha hash".to_owned())?;
    Ok(HashOf::from_untyped_unchecked(hash))
}

/// Parse one bounded canonical single-key authority for native request binding.
pub(crate) fn canonical_authority(authority_literal: &str) -> Result<AccountId, String> {
    if authority_literal.is_empty()
        || authority_literal.len() > AUTHORITY_MAX_BYTES_V1
        || authority_literal.trim() != authority_literal
    {
        return Err("authority must be bounded exact canonical I105".to_owned());
    }
    let parsed = AccountId::parse_encoded(authority_literal)
        .map_err(|_| "authority must be canonical I105".to_owned())?;
    if parsed.canonical() != authority_literal {
        return Err("authority must use its exact canonical I105 representation".to_owned());
    }
    let account = parsed.into_account_id();
    if account.try_signatory().is_none() {
        return Err("authority must be a single-key account".to_owned());
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

fn exact_query(entrypoint_hash: HashOf<TransactionEntrypoint>) -> QueryRequest {
    let predicate = CompoundPredicate::<CommittedTransaction>::from_committed_tx_predicate(
        CommittedTxPredicate::EntryEq(entrypoint_hash),
    );
    QueryRequest::Start(QueryWithParams {
        query: (),
        query_payload: FindTransactions::new().dyn_encode(),
        item: QueryItemKind::CommittedTransaction,
        predicate_bytes: predicate.encode(),
        selector_bytes: SelectorTuple::<CommittedTransaction>::default().encode(),
        params: QueryParams::default(),
    })
}

pub(crate) fn build_signed_query_v1(
    authority_literal: &str,
    private_key_bytes: &[u8],
    network_id_bytes: &[u8],
    transaction_hash_hex: &str,
) -> Result<Vec<u8>, String> {
    let entrypoint_hash = canonical_lower_hash(transaction_hash_hex)?;
    build_signed_request_v1(
        authority_literal,
        private_key_bytes,
        network_id_bytes,
        exact_query(entrypoint_hash),
    )
}

/// Sign one closed query request with the shared fresh nonce/TTL contract.
pub(crate) fn build_signed_request_v1(
    authority_literal: &str,
    private_key_bytes: &[u8],
    network_id_bytes: &[u8],
    request: QueryRequest,
) -> Result<Vec<u8>, String> {
    let network_id = parse_transaction_network_id_bytes(network_id_bytes)
        .map_err(|error| error.reason.clone())?;
    let authority = canonical_authority(authority_literal)?;
    let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, private_key_bytes)
        .map_err(|_| "query private key is not canonical Ed25519 material".to_owned())?;
    let key_pair = KeyPair::from_private_key(private_key)
        .map_err(|_| "query private key could not reconstruct its key pair".to_owned())?;
    if authority.try_signatory() != Some(key_pair.public_key()) {
        return Err("query private key does not match the authority account".to_owned());
    }
    let creation_time_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock precedes the UNIX epoch".to_owned())?
        .as_millis()
        .try_into()
        .map_err(|_| "query creation time exceeds u64".to_owned())?;
    let mut nonce = [0_u8; 32];
    OsRng
        .try_fill_bytes(&mut nonce)
        .map_err(|_| "query nonce OS RNG failed".to_owned())?;
    if nonce == [0; 32] {
        return Err("query nonce OS RNG returned an all-zero value".to_owned());
    }
    let payload = request.with_authority(
        network_id,
        authority,
        creation_time_ms,
        NonZeroU64::new(QUERY_TTL_MS_V1).expect("query TTL is nonzero"),
        nonce,
    );
    let signed = payload
        .try_sign(&key_pair)
        .map_err(|_| "query signing failed".to_owned())?;
    signed
        .verify_signature()
        .map_err(|_| "final signed query failed native verification".to_owned())?;
    let body = signed.encode_versioned();
    if body.is_empty() || body.len() > SIGNED_QUERY_MAX_BYTES_V1 {
        return Err("final signed query violates its closed byte bound".to_owned());
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

pub(crate) fn inspect_committed_result_v1(
    transaction_hash_hex: &str,
    network_id_bytes: &[u8],
    authority_literal: &str,
    response: &[u8],
) -> Result<CommittedResultProjectionV1, String> {
    let expected_hash = canonical_lower_hash(transaction_hash_hex)?;
    let expected_network_id: NetworkId = parse_transaction_network_id_bytes(network_id_bytes)
        .map_err(|error| error.reason.clone())?;
    let expected_authority = canonical_authority(authority_literal)?;
    if response.is_empty() || response.len() > RESPONSE_MAX_BYTES_V1 {
        return Err("transaction-details response is outside its closed byte bound".to_owned());
    }
    let details: PipelineTransactionDetailsResponse = norito::decode_canonical_with_limits(
        response,
        norito::canonical_decode_limits(response.len()),
    )
    .map_err(|_| "transaction-details response is not canonical Norito".to_owned())?;
    let response_hash = canonical_lower_hash(&details.hash)?;
    let transaction = &details.transaction;
    if response_hash != expected_hash
        || details.block_height == 0
        || transaction.entrypoint_hash() != &expected_hash
        || transaction.entrypoint().hash() != expected_hash
        || transaction.result_hash() != &transaction.result().hash()
        || transaction.entrypoint_proof().leaf_index() != transaction.result_proof().leaf_index()
    {
        return Err("transaction-details response hash/result/block binding is invalid".to_owned());
    }
    let TransactionEntrypoint::External(signed_transaction) = transaction.entrypoint() else {
        return Err(
            "transaction-details response is not an external signed transaction".to_owned(),
        );
    };
    if signed_transaction.network_id() != Some(&expected_network_id) {
        return Err("transaction-details response belongs to another NetworkId".to_owned());
    }
    if signed_transaction.authority() != &expected_authority {
        return Err("transaction-details response belongs to another authority".to_owned());
    }
    signed_transaction.verify_signature().map_err(|_| {
        "transaction-details response has an invalid transaction signature".to_owned()
    })?;
    let (result_ok, code, message) = match &transaction.result().0 {
        Ok(_) => (true, None, None),
        Err(reason) => {
            let message = reason.to_string();
            if message.is_empty()
                || message.len() > REJECTION_MESSAGE_MAX_BYTES_V1
                || message.trim() != message
                || message.chars().any(char::is_control)
            {
                return Err(
                    "committed rejection message violates its closed text contract".to_owned(),
                );
            }
            (false, Some(rejection_code(reason)), Some(message))
        }
    };
    Ok(CommittedResultProjectionV1 {
        transaction_hash_hex: transaction_hash_hex.to_owned(),
        block_hash_hex: transaction.block_hash().to_string(),
        result_hash_hex: transaction.result_hash().to_string(),
        result_ok,
        rejection_code: code,
        rejection_message: message,
        committed_block_height: details.block_height,
    })
}
