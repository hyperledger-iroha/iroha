//! On-chain smart contract registry helpers backed by the world state.
//!
//! This module exposes thin helpers that wrap the canonical ISI instructions
//! for registering manifests, storing bytecode, and binding contract
//! instances. Read APIs query the authenticated world-state view so callers
//! never rely on process-local caches. This replaces the historical
//! process-global map and ensures every node observes the same registry
//! contents.

use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    isi::smart_contract_code::{
        ActivateContractInstance, RegisterSmartContractBytes, RegisterSmartContractCode,
    },
    smart_contract::ContractAddress,
    smart_contract::manifest::ContractManifest,
};
use mv::storage::StorageReadOnly;
use thiserror::Error;

use crate::{
    smartcontracts::Execute,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};

/// Smart contract registry errors.
#[derive(Debug, Error)]
pub enum RegistryError {
    /// Underlying instruction execution failed.
    #[error("instruction failed: {0}")]
    Instruction(#[from] crate::smartcontracts::Error),
    /// Contract manifest must declare `code_hash`.
    #[error("manifest.code_hash missing")]
    MissingCodeHash,
    /// Bytecode image does not include a valid IVM header.
    #[error("invalid contract bytecode: {0}")]
    InvalidCode(&'static str),
}

/// Record combining a contract manifest with optional bytecode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractCodeRecord {
    /// Manifest stored under the `code_hash` key.
    pub manifest: ContractManifest,
    /// Optional compiled bytecode bytes (entire `.to` image).
    pub code_bytes: Option<Vec<u8>>,
}

/// Register a smart contract manifest on-chain via the canonical ISI.
///
/// Manifest registration is public. Networks can still protect specific
/// namespaces at activation time via `gov_protected_namespaces`. The manifest
/// must include `code_hash`; other fields are optional.
///
/// # Errors
///
/// Returns [`RegistryError`] when the manifest is missing a `code_hash` or the
/// underlying `RegisterSmartContractCode` instruction fails during execution.
pub fn register_manifest(
    authority: &AccountId,
    manifest: ContractManifest,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), RegistryError> {
    if manifest.code_hash.is_none() {
        return Err(RegistryError::MissingCodeHash);
    }
    RegisterSmartContractCode { manifest }.execute(authority, state_transaction)?;
    Ok(())
}

/// Register compiled contract bytecode on-chain and return its `code_hash`.
///
/// The helper computes the canonical hash (bytes after the IVM header) and
/// submits the [`RegisterSmartContractBytes`] instruction. Bytecode
/// registration is public; namespace protection applies when instances are
/// activated.
///
/// # Errors
///
/// Returns [`RegistryError`] when the bytecode header is invalid or when the
/// underlying instruction execution fails.
pub fn register_code_bytes(
    authority: &AccountId,
    code: Vec<u8>,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<Hash, RegistryError> {
    let parsed = ivm::ProgramMetadata::parse(&code)
        .map_err(|_| RegistryError::InvalidCode("missing or malformed IVM header"))?;
    if parsed.header_len > code.len() {
        return Err(RegistryError::InvalidCode(
            "header length exceeds code size",
        ));
    }
    let body = &code[parsed.header_len..];
    let code_hash = Hash::new(body);
    RegisterSmartContractBytes { code_hash, code }.execute(authority, state_transaction)?;
    Ok(code_hash)
}

/// Bind `contract_address` to a `code_hash` to activate an instance.
///
/// The binding is idempotent: calling this helper with the same mapping is a
/// no-op, while conflicting mappings result in an error from the underlying ISI.
///
/// # Errors
///
/// Returns [`RegistryError`] when the activation instruction fails during
/// execution.
pub fn activate_instance(
    authority: &AccountId,
    contract_address: ContractAddress,
    code_hash: Hash,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), RegistryError> {
    ActivateContractInstance {
        contract_address,
        code_hash,
    }
    .execute(authority, state_transaction)?;
    Ok(())
}

/// Fetch the manifest stored for `code_hash`, if any.
pub fn fetch_manifest(state: &impl StateReadOnly, code_hash: &Hash) -> Option<ContractManifest> {
    state.world().contract_manifests().get(code_hash).cloned()
}

/// Fetch the stored bytecode for `code_hash`, if any.
pub fn fetch_code_bytes(state: &impl StateReadOnly, code_hash: &Hash) -> Option<Vec<u8>> {
    state.world().contract_code().get(code_hash).cloned()
}

/// Retrieve a combined record (manifest + optional bytecode) for `code_hash`.
pub fn fetch_record(state: &impl StateReadOnly, code_hash: &Hash) -> Option<ContractCodeRecord> {
    let manifest = fetch_manifest(state, code_hash)?;
    let code_bytes = fetch_code_bytes(state, code_hash);
    Some(ContractCodeRecord {
        manifest,
        code_bytes,
    })
}

/// Batched contract lookup combining manifest, bytecode, and optional binding lookup.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContractArtifacts {
    /// Stored manifest for `code_hash`, if any.
    pub manifest: Option<ContractManifest>,
    /// Stored bytecode for `code_hash`, if any.
    pub code_bytes: Option<Vec<u8>>,
    /// Code hash bound to `contract_address`, if a binding exists.
    pub bound_code_hash: Option<Hash>,
}

/// Fetch manifest, code bytes, and instance binding in a single pass.
#[must_use]
pub fn fetch_artifacts(
    state: &impl StateReadOnly,
    code_hash: &Hash,
    binding: Option<&ContractAddress>,
) -> ContractArtifacts {
    let manifest = fetch_manifest(state, code_hash);
    let code_bytes = fetch_code_bytes(state, code_hash);
    let bound_code_hash = binding.and_then(|contract_address| {
        state
            .world()
            .contract_instances()
            .get(contract_address)
            .copied()
    });

    ContractArtifacts {
        manifest,
        code_bytes,
        bound_code_hash,
    }
}

/// Return the code hash bound to `contract_address`, if any.
pub fn fetch_instance_binding(
    state: &impl StateReadOnly,
    contract_address: &ContractAddress,
) -> Option<Hash> {
    state
        .world()
        .contract_instances()
        .get(contract_address)
        .copied()
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{
        isi::SetParameter,
        parameter::custom::{CustomParameter, CustomParameterId},
        prelude::*,
    };

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn minimal_ivm_program(abi_version: u8) -> Vec<u8> {
        let mut code = Vec::new();
        code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let meta = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 1,
            abi_version,
        };
        let mut out = meta.encode();
        out.extend_from_slice(&code);
        out
    }

    fn test_state() -> (State, AccountId, iroha_crypto::KeyPair) {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let kp = iroha_crypto::KeyPair::random();
        let (pubkey, _) = kp.clone().into_parts();
        let dom: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let auth = AccountId::of(pubkey);
        let domain = Domain::new(dom.clone()).build(&auth);
        let account = Account::new(auth.clone()).build(&auth);
        let world = World::with([domain], [account], std::iter::empty::<AssetDefinition>());
        let state = State::new_for_testing(world, kura, query);
        (state, auth, kp)
    }

    fn default_header(height: u64) -> iroha_data_model::block::BlockHeader {
        iroha_data_model::block::BlockHeader::new(
            core::num::NonZeroU64::new(height).expect("block height must be non-zero"),
            None,
            None,
            None,
            0,
            0,
        )
    }

    #[test]
    fn registry_roundtrip_manifest_and_code() {
        let (state, authority, kp) = test_state();
        let mut block = state.block(default_header(1));
        let mut stx = block.transaction();
        let contract_address = ContractAddress::derive(0, &authority, 0, DataSpaceId::GLOBAL)
            .expect("contract address");

        // Register bytecode and manifest, then activate a contract address binding.
        let code = minimal_ivm_program(1);
        let code_hash =
            register_code_bytes(&authority, code.clone(), &mut stx).expect("register bytecode");

        let manifest = ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: Some(Hash::new(b"abi-placeholder")),
            compiler_fingerprint: Some("kotodama-1.0".into()),
            features_bitmap: Some(0),
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&kp);
        register_manifest(&authority, manifest.clone(), &mut stx).expect("register manifest");

        activate_instance(&authority, contract_address.clone(), code_hash, &mut stx)
            .expect("activate instance");

        stx.apply();
        block.commit().expect("commit block");

        let view = state.view();
        // Manifest fetch
        let got_manifest = fetch_manifest(&view, &code_hash).expect("manifest stored");
        assert_eq!(got_manifest, manifest);
        // Bytecode fetch
        let got_code = fetch_code_bytes(&view, &code_hash).expect("code stored");
        assert_eq!(got_code, code);
        // Combined record fetch
        let record = fetch_record(&view, &code_hash).expect("record exists");
        assert_eq!(record.manifest, manifest);
        assert_eq!(record.code_bytes.as_deref(), Some(code.as_slice()));
        // Instance binding fetch
        let bound = fetch_instance_binding(&view, &contract_address).expect("binding exists");
        assert_eq!(bound, code_hash);
    }

    #[test]
    fn activate_instance_is_idempotent_for_same_contract_address() {
        let (state, authority, kp) = test_state();
        let mut block = state.block(default_header(1));
        let mut stx = block.transaction();
        let contract_address = ContractAddress::derive(0, &authority, 0, DataSpaceId::GLOBAL)
            .expect("contract address");

        let code = minimal_ivm_program(1);
        let code_hash =
            register_code_bytes(&authority, code.clone(), &mut stx).expect("register bytecode");
        let manifest = ContractManifest {
            code_hash: Some(code_hash),
            abi_hash: Some(Hash::new(b"abi-placeholder")),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
        .signed(&kp);
        register_manifest(&authority, manifest, &mut stx).expect("register manifest");
        activate_instance(&authority, contract_address.clone(), code_hash, &mut stx)
            .expect("initial activation");
        activate_instance(&authority, contract_address.clone(), code_hash, &mut stx)
            .expect("re-activating the same binding should be a no-op");
        assert_eq!(
            stx.world.contract_instances.get(&contract_address),
            Some(&code_hash),
            "contract address should stay bound to the same code hash"
        );
    }

    #[test]
    fn register_code_obeys_size_cap() {
        let (state, authority, _kp) = test_state();
        let mut block = state.block(default_header(1));
        let mut stx = block.transaction();

        // Set very small cap via custom parameter to ensure registration fails.
        let id = CustomParameterId("max_contract_code_bytes".parse().unwrap());
        let cap = CustomParameter::new(id, iroha_primitives::json::Json::new(8u64));
        SetParameter::new(Parameter::Custom(cap))
            .execute(&authority, &mut stx)
            .expect("set cap");

        let code = minimal_ivm_program(1);
        let err = register_code_bytes(&authority, code, &mut stx).unwrap_err();
        match err {
            RegistryError::Instruction(inner) => {
                let msg = inner.to_string();
                assert!(
                    msg.contains("code bytes exceed cap"),
                    "unexpected instruction error: {msg}"
                );
            }
            other => panic!("expected instruction error, got {other:?}"),
        }
    }

    #[test]
    fn register_manifest_requires_code_hash() {
        let (state, authority, _kp) = test_state();
        let mut block = state.block(default_header(1));
        let mut stx = block.transaction();

        let manifest = ContractManifest {
            code_hash: None,
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        };
        let err = register_manifest(&authority, manifest, &mut stx).unwrap_err();
        matches!(err, RegistryError::MissingCodeHash);
    }
}
