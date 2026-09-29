//! TRON destination encodings (spec §5.2.5, §7.1, §7.2).
//!
//! The `Transaction.raw` protobuf builder for `TriggerSmartContract` (every destination call:
//! finalize, rotations, controls, voids and burns) and `CreateSmartContract` (deployment), the
//! transaction id `sha256(raw)`, the recoverable signature and the created contract address.
//! TRON keys are secp256k1 keys whose address is `0x41 ‖ keccak256(pubkey)[12..]`, so the owner-only
//! EVM key files ([`super::evm::EvmSigningKey`]) sign TRON transactions too. The calldata of the
//! shared Solidity contract is the EVM calldata ([`super::evm`]).
//!
//! Protobuf fields are written in field order with proto3 defaults omitted, as java-tron and
//! `TronWeb` serialize them; `ref_block_bytes` and `ref_block_hash` bind the transaction to a
//! recent block (`TAPoS`).

use iroha_sccp::v1::hashes::keccak256;
use sha2::{Digest as _, Sha256};

use super::evm::{EvmError, EvmSigningKey};

/// First byte of every TRON address.
pub const TRON_ADDRESS_PREFIX: u8 = 0x41;
/// `Transaction.Contract.ContractType.CreateSmartContract`.
pub const CREATE_SMART_CONTRACT: u64 = 30;
/// `Transaction.Contract.ContractType.TriggerSmartContract`.
pub const TRIGGER_SMART_CONTRACT: u64 = 31;
/// Default lifetime of a built transaction after its reference block.
pub const DEFAULT_EXPIRATION_MS: u64 = 300_000;
/// Default `fee_limit` of a destination call (in sun): 1 000 TRX.
pub const DEFAULT_CALL_FEE_LIMIT_SUN: u64 = 1_000_000_000;
/// Default `fee_limit` of a deployment (in sun): 5 000 TRX.
pub const DEFAULT_CREATE_FEE_LIMIT_SUN: u64 = 5_000_000_000;

const TRIGGER_TYPE_URL: &str = "type.googleapis.com/protocol.TriggerSmartContract";
const CREATE_TYPE_URL: &str = "type.googleapis.com/protocol.CreateSmartContract";

/// The TRON address of an EVM-form address.
#[must_use]
pub fn tron_address(evm: &[u8; 20]) -> [u8; 21] {
    let mut address = [0_u8; 21];
    address[0] = TRON_ADDRESS_PREFIX;
    address[1..].copy_from_slice(evm);
    address
}

/// The contract a `CreateSmartContract` transaction `tx_id` of `owner` creates:
/// `0x41 ‖ keccak256(tx_id ‖ owner)[12..]` (java-tron `generateContractAddress`).
#[must_use]
pub fn created_contract_address(tx_id: &[u8; 32], owner: &[u8; 21]) -> [u8; 21] {
    let hash = keccak256(&[tx_id, owner]);
    let mut evm = [0_u8; 20];
    evm.copy_from_slice(&hash[12..]);
    tron_address(&evm)
}

/// The recent block a transaction references (`TAPoS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TronReferenceBlockV1 {
    /// Block number.
    pub number: u64,
    /// Block id (`blockID`).
    pub block_id: [u8; 32],
    /// Block timestamp (ms).
    pub timestamp_ms: u64,
}

/// A `TriggerSmartContract` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TronCallV1 {
    /// Calling account.
    pub owner: [u8; 21],
    /// Called contract.
    pub contract: [u8; 21],
    /// ABI calldata.
    pub data: Vec<u8>,
    /// Most TRX (in sun) the call may burn for energy.
    pub fee_limit_sun: u64,
}

/// A `CreateSmartContract` deployment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TronCreateV1 {
    /// Deploying account (the contract's origin).
    pub owner: [u8; 21],
    /// Creation bytecode with the ABI-encoded constructor arguments appended.
    pub bytecode: Vec<u8>,
    /// Contract name.
    pub name: String,
    /// Most TRX (in sun) the deployment may burn for energy.
    pub fee_limit_sun: u64,
    /// Energy the origin pays per call at most.
    pub origin_energy_limit: u64,
    /// Share (percent) of call energy the caller pays; 100 makes callers pay everything.
    pub consume_user_resource_percent: u64,
}

fn push_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push(value.to_le_bytes()[0] | 0x80);
        value >>= 7;
    }
    out.push(value.to_le_bytes()[0]);
}

fn push_uint(out: &mut Vec<u8>, field: u64, value: u64) {
    if value != 0 {
        push_varint(out, field << 3);
        push_varint(out, value);
    }
}

fn push_bytes(out: &mut Vec<u8>, field: u64, value: &[u8]) {
    if !value.is_empty() {
        push_varint(out, (field << 3) | 2);
        push_varint(out, u64::try_from(value.len()).unwrap_or(u64::MAX));
        out.extend_from_slice(value);
    }
}

fn any(type_url: &str, value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(type_url.len() + value.len() + 8);
    push_bytes(&mut out, 1, type_url.as_bytes());
    push_bytes(&mut out, 2, value);
    out
}

fn contract(kind: u64, type_url: &str, parameter: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    push_uint(&mut out, 1, kind);
    push_bytes(&mut out, 2, &any(type_url, parameter));
    out
}

/// An unsigned `Transaction.raw`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TronRawTransactionV1 {
    raw: Vec<u8>,
}

impl TronRawTransactionV1 {
    fn build(
        contract: &[u8],
        reference: &TronReferenceBlockV1,
        now_ms: u64,
        fee_limit_sun: u64,
    ) -> Self {
        let mut raw = Vec::with_capacity(contract.len() + 64);
        push_bytes(&mut raw, 1, &reference.number.to_be_bytes()[6..8]);
        push_bytes(&mut raw, 4, &reference.block_id[8..16]);
        push_uint(
            &mut raw,
            8,
            reference.timestamp_ms.saturating_add(DEFAULT_EXPIRATION_MS),
        );
        push_bytes(&mut raw, 11, contract);
        push_uint(&mut raw, 14, now_ms);
        push_uint(&mut raw, 18, fee_limit_sun);
        Self { raw }
    }

    /// A `TriggerSmartContract` transaction of `call` referencing `reference`, created at
    /// `now_ms`.
    #[must_use]
    pub fn trigger(call: &TronCallV1, reference: &TronReferenceBlockV1, now_ms: u64) -> Self {
        let mut parameter = Vec::with_capacity(call.data.len() + 48);
        push_bytes(&mut parameter, 1, &call.owner);
        push_bytes(&mut parameter, 2, &call.contract);
        push_bytes(&mut parameter, 4, &call.data);
        Self::build(
            &contract(TRIGGER_SMART_CONTRACT, TRIGGER_TYPE_URL, &parameter),
            reference,
            now_ms,
            call.fee_limit_sun,
        )
    }

    /// A `CreateSmartContract` transaction of `create` referencing `reference`, created at
    /// `now_ms`.
    #[must_use]
    pub fn create(create: &TronCreateV1, reference: &TronReferenceBlockV1, now_ms: u64) -> Self {
        let mut smart_contract = Vec::with_capacity(create.bytecode.len() + 64);
        push_bytes(&mut smart_contract, 1, &create.owner);
        push_bytes(&mut smart_contract, 4, &create.bytecode);
        push_uint(&mut smart_contract, 6, create.consume_user_resource_percent);
        push_bytes(&mut smart_contract, 7, create.name.as_bytes());
        push_uint(&mut smart_contract, 8, create.origin_energy_limit);
        let mut parameter = Vec::with_capacity(smart_contract.len() + 32);
        push_bytes(&mut parameter, 1, &create.owner);
        push_bytes(&mut parameter, 2, &smart_contract);
        Self::build(
            &contract(CREATE_SMART_CONTRACT, CREATE_TYPE_URL, &parameter),
            reference,
            now_ms,
            create.fee_limit_sun,
        )
    }

    /// The exact `Transaction.raw` bytes.
    #[must_use]
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// The transaction id: `sha256(raw)`.
    #[must_use]
    pub fn id(&self) -> [u8; 32] {
        Sha256::digest(&self.raw).into()
    }

    /// Sign with `key` (`v ∈ {27, 28}`) and return the `Transaction` protobuf to broadcast.
    ///
    /// # Errors
    ///
    /// Returns [`EvmError::Signature`] when signing fails.
    pub fn sign(&self, key: &EvmSigningKey) -> Result<Vec<u8>, EvmError> {
        let signature = key.sign_digest(&self.id())?;
        let mut transaction = Vec::with_capacity(self.raw.len() + 80);
        push_bytes(&mut transaction, 1, &self.raw);
        push_bytes(&mut transaction, 2, &signature);
        Ok(transaction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference() -> TronReferenceBlockV1 {
        let mut block_id = [0_u8; 32];
        block_id[..8].copy_from_slice(&0x0001_2345_u64.to_be_bytes());
        block_id[8..16].copy_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x11, 0x22]);
        TronReferenceBlockV1 {
            number: 0x0001_2345,
            block_id,
            timestamp_ms: 1_700_000_000_000,
        }
    }

    #[test]
    fn trigger_transactions_follow_the_protobuf_field_order() {
        let call = TronCallV1 {
            owner: [0x41; 21],
            contract: tron_address(&[0x22; 20]),
            data: vec![0xde, 0xad],
            fee_limit_sun: 1_000,
        };
        let raw = TronRawTransactionV1::trigger(&call, &reference(), 1_700_000_001_000);
        let bytes = raw.raw();
        // ref_block_bytes = the number's bytes 6..8, ref_block_hash = the id's bytes 8..16.
        assert_eq!(&bytes[..4], &[0x0a, 0x02, 0x23, 0x45]);
        assert_eq!(&bytes[4..6], &[0x22, 0x08]);
        assert_eq!(
            &bytes[6..14],
            &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x11, 0x22]
        );
        assert_eq!(bytes[14], 0x40, "expiration is field 8");
        let type_url = TRIGGER_TYPE_URL.as_bytes();
        assert!(
            bytes
                .windows(type_url.len())
                .any(|window| window == type_url)
        );
        assert!(bytes.windows(2).any(|window| window == [0xde, 0xad]));
        // fee_limit (field 18) is the last field: key 0x90 0x01, value 1000 = 0xe8 0x07.
        assert_eq!(&bytes[bytes.len() - 4..], &[0x90, 0x01, 0xe8, 0x07]);
        assert_eq!(raw.id(), <[u8; 32]>::from(Sha256::digest(bytes)));
    }

    #[test]
    fn signed_transactions_recover_the_owner() {
        let key = EvmSigningKey::from_secret([7; 32]).expect("key");
        let owner = tron_address(&key.address());
        let raw = TronRawTransactionV1::trigger(
            &TronCallV1 {
                owner,
                contract: tron_address(&[3; 20]),
                data: vec![1, 2, 3],
                fee_limit_sun: DEFAULT_CALL_FEE_LIMIT_SUN,
            },
            &reference(),
            5,
        );
        let signed = raw.sign(&key).expect("signs");
        assert_eq!(signed[0], 0x0a);
        let signature: [u8; 65] = signed[signed.len() - 65..].try_into().expect("65 bytes");
        assert!(matches!(signature[64], 27 | 28));
        let recovered =
            iroha_sccp::v1::signature::recover_address(&raw.id(), &signature).expect("recovers");
        assert_eq!(recovered, key.address());
    }

    #[test]
    fn create_transactions_and_contract_addresses_are_deterministic() {
        let create = TronCreateV1 {
            owner: tron_address(&[9; 20]),
            bytecode: vec![0x60, 0x80],
            name: "SccpTairaXor".into(),
            fee_limit_sun: DEFAULT_CREATE_FEE_LIMIT_SUN,
            origin_energy_limit: 10_000_000,
            consume_user_resource_percent: 100,
        };
        let raw = TronRawTransactionV1::create(&create, &reference(), 7);
        let type_url = CREATE_TYPE_URL.as_bytes();
        assert!(
            raw.raw()
                .windows(type_url.len())
                .any(|window| window == type_url)
        );
        let address = created_contract_address(&raw.id(), &create.owner);
        assert_eq!(address[0], TRON_ADDRESS_PREFIX);
        assert_eq!(address, created_contract_address(&raw.id(), &create.owner));
        assert_ne!(
            address,
            created_contract_address(&[0; 32], &create.owner),
            "the address depends on the transaction id"
        );
        let mut varint = Vec::new();
        push_varint(&mut varint, 300);
        assert_eq!(varint, vec![0xac, 0x02]);
    }
}
