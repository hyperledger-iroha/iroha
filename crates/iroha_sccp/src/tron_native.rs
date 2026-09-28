//! Native TRON `DPoS` header-finality verification for SCCP.
//!
//! TRON witnesses sign only `BlockHeader.raw_data`.  There is no native quorum
//! signature over a target block and there is no witness-set handover seal.
//! This module therefore replays the protocol's scheduled-producer and
//! solid-height state machine from an exact governed checkpoint.  Proofs that
//! reach a maintenance boundary fail closed because block headers do not commit
//! the post-maintenance active-witness roster or witness permission mapping.
//!
//! TODO(ws39): the governed `DPoS` anchor (with its `BLAKE2b` hash), the anchor-based schedule
//! replay and 27-header window, the proof work estimate and the retired SCCP transfer-call and
//! replay-witness binding were cut. The v1 TRON light client (`light_client::tron`) rebuilds
//! solidity from the header, signature and transaction-inclusion primitives kept here.
#![allow(
    dead_code,
    reason = "TODO(ws39): the kept TRON primitives are rewired by the v1 TRON light client"
)]
use super::H256;
use alloc::vec::Vec;
use iroha_crypto::EcdsaSecp256k1Sha256;
use sha2::{Digest, Sha256};
const TRON_TRIGGER_SMART_CONTRACT_TYPE_URL_V1: &[u8] =
    b"type.googleapis.com/protocol.TriggerSmartContract";
const TRON_ADDRESS_BYTES: usize = 21;
const TRON_SIGNATURE_BYTES: usize = 65;
const TRON_BLOCK_INTERVAL_MS: u64 = 3_000;
const TRON_MAINTENANCE_SKIP_SLOTS: u32 = 2;
const TRON_SINGLE_REPEAT: u32 = 1;
const TRON_SOLIDIFIED_THRESHOLD_PERCENT: u8 = 70;
const TRON_ACTIVE_WITNESS_COUNT: usize = 27;
const TRON_MAX_RAW_HEADER_BYTES: usize = 16 * 1024;
const TRON_MAX_TRANSACTION_BYTES: usize = 512 * 1024;
const TRON_MAX_TRANSACTION_SIGNATURES: usize = 32;
const TRON_MAX_TRANSACTION_MERKLE_DEPTH: usize = 64;
/// Maximum post-anchor headers before the selected TRON target.
///
/// V1 requires a governed checkpoint no more than one complete 27-witness
/// scheduling round before the target.
pub const TRON_NATIVE_MAX_TARGET_HEADERS: usize = TRON_ACTIVE_WITNESS_COUNT;
/// Maximum headers after a TRON target before it becomes solid.
///
/// One complete active-witness round is enough to include all 27 producers in
/// a healthy schedule and therefore the required 19 distinct producers for
/// the native 70% solid-height order statistic.
pub const TRON_NATIVE_MAX_FINALITY_SUFFIX_HEADERS: usize = TRON_ACTIVE_WITNESS_COUNT;
/// Maximum headers in one canonical native TRON finality continuation.
pub const TRON_NATIVE_MAX_FINALITY_HEADERS: usize =
    TRON_NATIVE_MAX_TARGET_HEADERS + TRON_NATIVE_MAX_FINALITY_SUFFIX_HEADERS;
const SECP256K1_SCALAR_ORDER_BE: H256 = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
    0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41,
];
const SECP256K1_SCALAR_HALF_ORDER_BE: H256 = [
    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d, 0xdf, 0xe9, 0x2f, 0x46, 0x68, 0x1b, 0x20, 0xa0,
];
fn protobuf_varint_len(mut value: u64) -> usize {
    let mut len = 1usize;
    while value >= 0x80 {
        len += 1;
        value >>= 7;
    }
    len
}
/// Read one minimally encoded protobuf varint at `cursor`, advancing it.
fn read_protobuf_varint_at(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
    let start = *cursor;
    let mut value = 0u64;
    let mut shift = 0u32;
    for index in 0..10 {
        let byte = *bytes.get(*cursor)?;
        *cursor = (*cursor).checked_add(1)?;
        let chunk = u64::from(byte & 0x7f);
        if index == 9 && chunk > 1 {
            return None;
        }
        value |= chunk.checked_shl(shift)?;
        if byte & 0x80 == 0 {
            let consumed = (*cursor).checked_sub(start)?;
            return (consumed == protobuf_varint_len(value)).then_some(value);
        }
        shift = shift.checked_add(7)?;
    }
    None
}
fn h256_is_nonzero(value: &H256) -> bool {
    value.iter().any(|byte| *byte != 0)
}
/// Return the recoverable signature with an Ethereum-style `27 + recid` byte when it has a
/// canonical recovery id, a valid `r` and a low `s`.
fn tron_recoverable_signature_for_recovery(signature: &[u8; 65]) -> Option<[u8; 65]> {
    let mut r = [0u8; 32];
    r.copy_from_slice(&signature[..32]);
    let mut s = [0u8; 32];
    s.copy_from_slice(&signature[32..64]);
    let canonical = matches!(signature[64], 0..=3)
        && h256_is_nonzero(&r)
        && r < SECP256K1_SCALAR_ORDER_BE
        && h256_is_nonzero(&s)
        && s <= SECP256K1_SCALAR_HALF_ORDER_BE;
    if !canonical {
        return None;
    }
    let mut normalized = *signature;
    normalized[64] = signature[64].checked_add(27)?;
    Some(normalized)
}
fn sha256_bytes(payload: &[u8]) -> H256 {
    Sha256::digest(payload).into()
}
/// One active TRON super representative at a governed checkpoint.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::tron_native::TronNativeWitnessV1")]
pub struct TronNativeWitnessV1 {
    /// Canonical TRON account address, including the `0x41` network prefix.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub account_address: Vec<u8>,
    /// Canonical address recovered from the account's active witness permission.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub signing_address: Vec<u8>,
    /// Latest block number produced by this witness at the checkpoint.
    #[norito(with = "crate::json_utils::u64_string")]
    pub latest_block_number: u64,
}
/// One native TRON header and its producer signature.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::tron_native::TronNativeSignedHeaderV1")]
pub struct TronNativeSignedHeaderV1 {
    /// Exact deterministic protobuf serialization of `BlockHeader.raw`.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub raw_data: Vec<u8>,
    /// Java-TRON recoverable secp256k1 signature (`r || s || recid`).
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub witness_signature: Vec<u8>,
}
/// Inclusion proof for one full native TRON transaction protobuf.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::tron_native::TronNativeTransactionProofV1")]
pub struct TronNativeTransactionProofV1 {
    /// Zero-based transaction position in the native block transaction list.
    #[norito(with = "crate::json_utils::u64_string")]
    pub transaction_index: u64,
    /// Total number of transactions committed by the block.
    #[norito(with = "crate::json_utils::u64_string")]
    pub transaction_count: u64,
    /// Exact full serialized `protocol.Transaction`, including `ret` fields.
    #[norito(with = "crate::json_utils::bytes_hex")]
    pub transaction_bytes: Vec<u8>,
    /// Consumed native Merkle siblings, bottom-up; odd final nodes are promoted.
    #[norito(with = "crate::json_utils::vec_bytes_hex")]
    pub merkle_branch: Vec<Vec<u8>>,
}
/// Fail-closed reason returned by native TRON finality verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TronNativeFinalityError {
    /// The proof or checkpoint schema version is unsupported.
    UnsupportedVersion,
    /// The checkpoint names a non-TRON network.
    WrongNetwork,
    /// The governed anchor hash does not match the canonical checkpoint.
    AnchorHashMismatch,
    /// Static or stateful checkpoint fields are malformed or inconsistent.
    InvalidAnchor,
    /// The proof contains no headers, too many headers, or a bad target index.
    InvalidProofShape,
    /// A raw header protobuf is noncanonical, incomplete, duplicated, or unknown.
    InvalidHeaderEncoding,
    /// A header does not continue the checkpoint's native parent/number chain.
    HeaderChainMismatch,
    /// A header timestamp violates the native absolute-slot rules.
    InvalidTimestamp,
    /// Verifying the sequence would cross an unauthenticated maintenance update.
    MaintenanceBoundary,
    /// The scheduled witness does not match the header producer.
    WrongScheduledWitness,
    /// The native producer signature is malformed or resolves to the wrong key.
    InvalidWitnessSignature,
    /// The supplied continuation does not make the target block solid.
    TargetNotSolid,
    /// The target became solid before the last supplied continuation header.
    NonMinimalContinuation,
}
/// Fail-closed reason returned by native TRON transaction verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TronNativeTransactionError {
    /// The proof shape, transaction count, index, or byte bounds are invalid.
    InvalidProofShape,
    /// The protobuf transaction or one of its nested messages is noncanonical.
    InvalidTransactionEncoding,
    /// The authenticated transaction result is absent or not `SUCCESS`.
    TransactionFailed,
    /// The transaction does not call the exact governed source contract.
    WrongContract,
    /// The successful call sender differed from the canonical payload sender.
    WrongCaller,
    /// The recipient, scaled amount, or nonce differs from the payload-derived call.
    WrongCallData,
    /// The native transaction Merkle branch does not reconstruct the header root.
    InvalidMerkleProof,
}
/// Fail-closed reason returned by complete native TRON source verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TronNativeSourceError {
    /// The typed identity is malformed, belongs to another family, or names another lane.
    InvalidSourceIdentity,
    /// The canonical typed identity does not match the governed registry hash.
    SourceIdentityHashMismatch,
    /// Native `DPoS` finality verification failed.
    Finality(TronNativeFinalityError),
    /// Native transaction decoding or inclusion verification failed.
    Transaction(TronNativeTransactionError),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ParsedTronRawHeaderV1 {
    number: u64,
    timestamp_ms: u64,
    transaction_root: Option<H256>,
    parent_block_id: H256,
    _witness_id: Option<u64>,
    witness_address: [u8; TRON_ADDRESS_BYTES],
    _header_version: u32,
    account_state_root: Option<H256>,
}
fn is_tron_address(address: &[u8]) -> bool {
    address.len() == TRON_ADDRESS_BYTES
        && address.first() == Some(&0x41)
        && address[1..].iter().any(|byte| *byte != 0)
}
fn block_id_number(block_id: &H256) -> u64 {
    u64::from_be_bytes([
        block_id[0],
        block_id[1],
        block_id[2],
        block_id[3],
        block_id[4],
        block_id[5],
        block_id[6],
        block_id[7],
    ])
}
fn solid_height(latest_block_numbers: &[u64]) -> Option<u64> {
    if latest_block_numbers.len() != TRON_ACTIVE_WITNESS_COUNT {
        return None;
    }
    let mut sorted = latest_block_numbers.to_vec();
    sorted.sort_unstable();
    let position = sorted.len().checked_mul(usize::from(
        100u8.checked_sub(TRON_SOLIDIFIED_THRESHOLD_PERCENT)?,
    ))? / 100;
    sorted.get(position).copied()
}
fn read_bytes_field<'a>(bytes: &'a [u8], cursor: &mut usize) -> Option<&'a [u8]> {
    let len = usize::try_from(read_protobuf_varint_at(bytes, cursor)?).ok()?;
    let end = cursor.checked_add(len)?;
    let value = bytes.get(*cursor..end)?;
    *cursor = end;
    Some(value)
}
fn parse_tron_raw_header(raw_data: &[u8]) -> Option<ParsedTronRawHeaderV1> {
    if raw_data.is_empty() || raw_data.len() > TRON_MAX_RAW_HEADER_BYTES {
        return None;
    }
    let mut cursor = 0usize;
    let mut previous_field = 0u32;
    let mut timestamp_ms = None;
    let mut transaction_root = None;
    let mut parent_block_id = None;
    let mut number = None;
    let mut witness_id = None;
    let mut witness_address = None;
    let mut header_version = None;
    let mut account_state_root = None;
    while cursor < raw_data.len() {
        let key = read_protobuf_varint_at(raw_data, &mut cursor)?;
        let field = u32::try_from(key >> 3).ok()?;
        let wire = u8::try_from(key & 7).ok()?;
        if field <= previous_field {
            return None;
        }
        previous_field = field;
        match (field, wire) {
            (1, 0) => timestamp_ms = Some(read_protobuf_varint_at(raw_data, &mut cursor)?),
            (2, 2) => {
                transaction_root = Some(read_bytes_field(raw_data, &mut cursor)?.try_into().ok()?)
            }
            (3, 2) => {
                parent_block_id = Some(read_bytes_field(raw_data, &mut cursor)?.try_into().ok()?)
            }
            (7, 0) => number = Some(read_protobuf_varint_at(raw_data, &mut cursor)?),
            (8, 0) => {
                let value = read_protobuf_varint_at(raw_data, &mut cursor)?;
                if value == 0 || i64::try_from(value).is_err() {
                    return None;
                }
                witness_id = Some(value);
            }
            (9, 2) => {
                witness_address = Some(read_bytes_field(raw_data, &mut cursor)?.try_into().ok()?)
            }
            (10, 0) => {
                let value = u32::try_from(read_protobuf_varint_at(raw_data, &mut cursor)?).ok()?;
                if value == 0 || i32::try_from(value).is_err() {
                    return None;
                }
                header_version = Some(value);
            }
            (11, 2) => {
                account_state_root = Some(read_bytes_field(raw_data, &mut cursor)?.try_into().ok()?)
            }
            _ => return None,
        }
    }
    let parsed = ParsedTronRawHeaderV1 {
        number: number?,
        timestamp_ms: timestamp_ms?,
        transaction_root,
        parent_block_id: parent_block_id?,
        _witness_id: witness_id,
        witness_address: witness_address?,
        _header_version: header_version.unwrap_or(0),
        account_state_root,
    };
    (parsed.number != 0
        && i64::try_from(parsed.number).is_ok()
        && parsed.timestamp_ms != 0
        && i64::try_from(parsed.timestamp_ms).is_ok()
        && parsed.parent_block_id.iter().any(|byte| *byte != 0)
        && is_tron_address(&parsed.witness_address)
        && parsed
            .transaction_root
            .is_none_or(|root| root.iter().any(|byte| *byte != 0))
        && parsed
            .account_state_root
            .is_none_or(|root| root.iter().any(|byte| *byte != 0)))
    .then_some(parsed)
}
fn tron_block_id(number: u64, raw_hash: H256) -> H256 {
    let mut block_id = raw_hash;
    block_id[..8].copy_from_slice(&number.to_be_bytes());
    block_id
}
fn recover_tron_address(raw_hash: H256, signature: &[u8]) -> Option<[u8; TRON_ADDRESS_BYTES]> {
    let signature: [u8; TRON_SIGNATURE_BYTES] = signature.try_into().ok()?;
    let normalized = tron_recoverable_signature_for_recovery(&signature)?;
    let public_key =
        EcdsaSecp256k1Sha256::recover_public_key_from_prehash(&raw_hash, &normalized).ok()?;
    let evm = EcdsaSecp256k1Sha256::evm_address(&public_key);
    let mut address = [0u8; TRON_ADDRESS_BYTES];
    address[0] = 0x41;
    address[1..].copy_from_slice(&evm);
    Some(address)
}
fn transaction_encoding_error() -> TronNativeTransactionError {
    TronNativeTransactionError::InvalidTransactionEncoding
}
fn read_transaction_bytes_field<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
) -> Result<&'a [u8], TronNativeTransactionError> {
    read_bytes_field(bytes, cursor).ok_or_else(transaction_encoding_error)
}
fn read_transaction_varint(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<u64, TronNativeTransactionError> {
    read_protobuf_varint_at(bytes, cursor).ok_or_else(transaction_encoding_error)
}
fn transaction_field_key(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<(u32, u8), TronNativeTransactionError> {
    let key = read_transaction_varint(bytes, cursor)?;
    let field = u32::try_from(key >> 3).map_err(|_| transaction_encoding_error())?;
    let wire = u8::try_from(key & 7).map_err(|_| transaction_encoding_error())?;
    if field == 0 {
        return Err(transaction_encoding_error());
    }
    Ok((field, wire))
}
fn parse_tron_any(bytes: &[u8]) -> Result<&[u8], TronNativeTransactionError> {
    let mut cursor = 0usize;
    let mut previous = 0u32;
    let mut type_url = None;
    let mut value = None;
    while cursor < bytes.len() {
        let (field, wire) = transaction_field_key(bytes, &mut cursor)?;
        if field <= previous {
            return Err(transaction_encoding_error());
        }
        previous = field;
        match (field, wire) {
            (1, 2) => type_url = Some(read_transaction_bytes_field(bytes, &mut cursor)?),
            (2, 2) => value = Some(read_transaction_bytes_field(bytes, &mut cursor)?),
            _ => return Err(transaction_encoding_error()),
        }
    }
    if type_url != Some(TRON_TRIGGER_SMART_CONTRACT_TYPE_URL_V1) {
        return Err(transaction_encoding_error());
    }
    value.ok_or_else(transaction_encoding_error)
}
fn abi_word_usize(word: &[u8]) -> Option<usize> {
    if word.len() != 32 || word[..24].iter().any(|byte| *byte != 0) {
        return None;
    }
    let mut raw = [0_u8; 8];
    raw.copy_from_slice(&word[24..]);
    usize::try_from(u64::from_be_bytes(raw)).ok()
}
fn verify_tron_transaction_success(bytes: &[u8]) -> Result<(), TronNativeTransactionError> {
    let mut cursor = 0usize;
    let mut previous = 0u32;
    let mut contract_result = None;
    while cursor < bytes.len() {
        let (field, wire) = transaction_field_key(bytes, &mut cursor)?;
        if field <= previous {
            return Err(transaction_encoding_error());
        }
        previous = field;
        match (field, wire) {
            (1, 0) => {
                let fee = read_transaction_varint(bytes, &mut cursor)?;
                if fee == 0 || i64::try_from(fee).is_err() {
                    return Err(transaction_encoding_error());
                }
            }
            // `SUCESS` is protobuf's zero value and is omitted canonically;
            // any serialized `ret` value is therefore a failure or alias.
            (2, 0) => return Err(TronNativeTransactionError::TransactionFailed),
            (3, 0) => contract_result = Some(read_transaction_varint(bytes, &mut cursor)?),
            _ => return Err(transaction_encoding_error()),
        }
    }
    if contract_result != Some(1) {
        return Err(TronNativeTransactionError::TransactionFailed);
    }
    Ok(())
}
fn tron_transaction_merkle_node(left: H256, right: H256) -> H256 {
    let mut preimage = [0u8; 64];
    preimage[..32].copy_from_slice(&left);
    preimage[32..].copy_from_slice(&right);
    sha256_bytes(&preimage)
}
fn tron_transaction_merkle_root(
    leaf: H256,
    transaction_index: u64,
    transaction_count: u64,
    branch: &[Vec<u8>],
) -> Option<H256> {
    if transaction_count == 0
        || transaction_index >= transaction_count
        || branch.len() > TRON_MAX_TRANSACTION_MERKLE_DEPTH
        || branch.iter().any(|node| node.len() != 32)
    {
        return None;
    }
    let mut current = leaf;
    let mut index = transaction_index;
    let mut count = transaction_count;
    let mut branch_index = 0usize;
    while count > 1 {
        if index & 1 == 0 {
            if index + 1 < count {
                let sibling: H256 = branch.get(branch_index)?.as_slice().try_into().ok()?;
                branch_index = branch_index.checked_add(1)?;
                current = tron_transaction_merkle_node(current, sibling);
            }
        } else {
            let sibling: H256 = branch.get(branch_index)?.as_slice().try_into().ok()?;
            branch_index = branch_index.checked_add(1)?;
            current = tron_transaction_merkle_node(sibling, current);
        }
        index >>= 1;
        count = count.checked_add(1)?.checked_div(2)?;
    }
    (branch_index == branch.len()).then_some(current)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn push_varint(out: &mut Vec<u8>, mut value: u64) {
        while value >= 0x80 {
            out.push(u8::try_from(value & 0x7f).expect("varint byte") | 0x80);
            value >>= 7;
        }
        out.push(u8::try_from(value).expect("varint tail"));
    }
    fn push_key(out: &mut Vec<u8>, field: u32, wire: u8) {
        push_varint(out, (u64::from(field) << 3) | u64::from(wire));
    }
    fn push_bytes(out: &mut Vec<u8>, field: u32, value: &[u8]) {
        push_key(out, field, 2);
        push_varint(out, u64::try_from(value.len()).expect("length"));
        out.extend_from_slice(value);
    }
    fn push_int(out: &mut Vec<u8>, field: u32, value: u64) {
        push_key(out, field, 0);
        push_varint(out, value);
    }
    fn raw_header(
        number: u64,
        timestamp_ms: u64,
        parent: H256,
        witness: [u8; TRON_ADDRESS_BYTES],
        transaction_root: Option<H256>,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        push_int(&mut out, 1, timestamp_ms);
        if let Some(root) = transaction_root {
            push_bytes(&mut out, 2, &root);
        }
        push_bytes(&mut out, 3, &parent);
        push_int(&mut out, 7, number);
        push_bytes(&mut out, 9, &witness);
        push_int(&mut out, 10, 31);
        push_bytes(&mut out, 11, &[0xA5; 32]);
        out
    }
    #[test]
    fn raw_header_rejects_unknown_duplicate_reordered_and_overlong_varints() {
        let witness = [0x41; TRON_ADDRESS_BYTES];
        let canonical = raw_header(101, 3_003_000, [0x42; 32], witness, Some([7; 32]));
        let parsed = parse_tron_raw_header(&canonical).expect("canonical raw header parses");
        assert_eq!(parsed.number, 101);
        let mut unknown = canonical.clone();
        push_int(&mut unknown, 12, 1);
        assert!(parse_tron_raw_header(&unknown).is_none());
        let mut duplicate = canonical.clone();
        push_int(&mut duplicate, 10, 31);
        assert!(parse_tron_raw_header(&duplicate).is_none());
        let mut reordered = Vec::new();
        push_int(&mut reordered, 7, 101);
        reordered.extend_from_slice(&canonical);
        assert!(parse_tron_raw_header(&reordered).is_none());
        let mut overlong = canonical.clone();
        overlong[0] = 0x88;
        overlong.insert(1, 0x00);
        assert!(parse_tron_raw_header(&overlong).is_none());
    }
    #[test]
    fn transaction_merkle_root_promotes_odd_leaf_without_duplication() {
        let leaf = sha256_bytes(b"tron transaction");
        let left_pair = tron_transaction_merkle_node([0x10; 32], [0x20; 32]);
        let root = tron_transaction_merkle_node(left_pair, leaf);
        assert_eq!(
            tron_transaction_merkle_root(leaf, 2, 3, &[left_pair.to_vec()]),
            Some(root)
        );
        assert_ne!(
            tron_transaction_merkle_root(leaf, 2, 3, &[left_pair.to_vec()]),
            Some(tron_transaction_merkle_node(
                left_pair,
                tron_transaction_merkle_node(leaf, leaf)
            ))
        );
        assert_eq!(
            tron_transaction_merkle_root(leaf, 2, 3, &[left_pair.to_vec(), [0x99; 32].to_vec()]),
            None
        );
        assert_eq!(tron_transaction_merkle_root(leaf, 3, 3, &[]), None);
        assert_eq!(tron_transaction_merkle_root(leaf, 0, 1, &[]), Some(leaf));
    }
    #[test]
    fn protobuf_varints_and_recoverable_signatures_are_canonical() {
        let mut cursor = 0;
        assert_eq!(
            read_protobuf_varint_at(&[0xac, 0x02], &mut cursor),
            Some(300)
        );
        assert_eq!(cursor, 2);
        let mut cursor = 0;
        assert_eq!(read_protobuf_varint_at(&[0x80, 0x00], &mut cursor), None);
        let mut signature = [0x11; 65];
        signature[64] = 1;
        assert_eq!(
            tron_recoverable_signature_for_recovery(&signature).map(|value| value[64]),
            Some(28)
        );
        signature[64] = 4;
        assert_eq!(tron_recoverable_signature_for_recovery(&signature), None);
        signature[64] = 0;
        signature[32..64].copy_from_slice(&[0xff; 32]);
        assert_eq!(tron_recoverable_signature_for_recovery(&signature), None);
    }
}
