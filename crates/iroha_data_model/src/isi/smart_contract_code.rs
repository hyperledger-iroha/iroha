use super::*;
use crate::smart_contract::manifest::ContractManifest;
/// Maximum number of contract artifact bytes carried by one native upload chunk.
pub const SMART_CONTRACT_CODE_CHUNK_BYTES: usize = 65_536;
isi! {
    /// Register a smart contract manifest keyed by `code_hash` into the WSV.
    ///
    /// The authority must hold `CanRegisterSmartContractCode`. The corresponding
    /// verified bytecode must already be present under the manifest's `code_hash`.
    pub struct RegisterSmartContractCode {
        /// Manifest containing `code_hash` (required) and `abi_hash`.
        pub manifest: ContractManifest,
    }
}
impl crate::seal::Instruction for RegisterSmartContractCode {}
isi! {
    /// Deactivate a contract instance by removing the `contract_address` binding.
    ///
    /// The authority must hold `CanRegisterSmartContractCode`. Deactivation acts as a kill-switch
    /// for compromised deployments. The address becomes unavailable immediately, while provenance
    /// information (caller and optional reason) is emitted via the data event stream.
    pub struct DeactivateContractInstance {
        /// Canonical contract address.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// Optional audit reason describing why the instance was deactivated.
        #[norito(default)]
        pub reason: Option<String>,
    }
}
impl crate::seal::Instruction for DeactivateContractInstance {}
isi! {
    /// Activate or perform `kaizen`/`改善` on a contract instance by binding
    /// `contract_address` to a `code_hash`.
    ///
    /// The authority must hold `CanRegisterSmartContractCode`. A new binding stages its declared
    /// `hajimari`/`始まり` hook. Rebinding an active address to different code additionally requires
    /// `CanEnactGovernance`; it is an in-place `kaizen`/`改善` and stages the new artifact's declared
    /// `kaizen`/`改善` hook. Until that exact hook succeeds, ordinary calls are rejected.
    pub struct ActivateContractInstance {
        /// Canonical contract address.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// Domain-separated canonical hash of the complete `.to` artifact to bind.
        pub code_hash: iroha_crypto::Hash,
    }
}
impl crate::seal::Instruction for ActivateContractInstance {}
isi! {
    /// Atomically deploy a new contract address and move its stable alias.
    ///
    /// `expected_deploy_nonce` and `expected_previous_contract_address` are compare-and-swap
    /// guards. The executor derives `contract_address` from the authority, nonce, exact
    /// genesis-derived `NetworkId`, and alias dataspace, then either creates the first binding or
    /// replaces the exact active alias target. The authority must already exist and hold
    /// `CanRegisterSmartContractCode`;
    /// protected namespaces additionally require governance authority.
    pub struct CommitContractDeployment {
        /// Exact next deployment nonce expected in the authority's reserved metadata.
        pub expected_deploy_nonce: u64,
        /// Newly derived canonical contract address.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// Domain-separated canonical hash of the complete `.to` artifact to activate.
        pub code_hash: iroha_crypto::Hash,
        /// Stable alias to bind to the new address.
        pub contract_alias: crate::smart_contract::ContractAlias,
        /// Optional alias lease expiry in Unix milliseconds.
        pub lease_expiry_ms: Option<u64>,
        /// Exact previous live alias target expected by the submitter, or `None` for first deploy.
        pub expected_previous_contract_address: Option<crate::smart_contract::ContractAddress>,
    }
}
impl crate::seal::Instruction for CommitContractDeployment {}
isi! {
    /// Register compiled contract bytecode on-chain keyed by its `code_hash`.
    ///
    /// The bytecode is the full compiled `.to` image including the IVM header.
    /// Nodes verify that `code_hash` equals the domain-separated canonical hash of the
    /// complete deployable `.to` artifact, including the execution header, `CNTR`,
    /// literals, and code, before storing. The authority must hold
    /// `CanRegisterSmartContractCode`.
    pub struct RegisterSmartContractBytes {
        /// Domain-separated canonical hash of the complete deployable `.to` artifact.
        pub code_hash: iroha_crypto::Hash,
        /// Full compiled `.to` image (including IVM header).
        pub code: Vec<u8>,
    }
}
impl crate::seal::Instruction for RegisterSmartContractBytes {}
isi! {
    /// Upload one bounded chunk of a compiled smart-contract artifact.
    ///
    /// Chunks are staged under `(authority, code_hash)` until an explicit
    /// [`FinalizeSmartContractCodeUpload`] verifies and atomically registers the
    /// complete artifact. Chunks may arrive out of order.
    pub struct UploadSmartContractCodeChunk {
        /// Domain-separated canonical hash of the complete deployable `.to` artifact.
        pub code_hash: iroha_crypto::Hash,
        /// Declared byte length of the complete artifact.
        pub total_size: u64,
        /// Zero-based position of this chunk in the complete artifact.
        pub chunk_index: u32,
        /// Declared total number of chunks in the complete artifact.
        pub chunk_count: u32,
        /// Artifact bytes for `chunk_index`.
        pub chunk: Vec<u8>,
    }
}
impl crate::seal::Instruction for UploadSmartContractCodeChunk {}
isi! {
    /// Verify and atomically register a completely staged smart-contract artifact.
    ///
    /// Failed finalization leaves the pending upload intact so the owner can
    /// retry it or cancel it explicitly.
    pub struct FinalizeSmartContractCodeUpload {
        /// Domain-separated canonical hash of the complete deployable `.to` artifact.
        pub code_hash: iroha_crypto::Hash,
        /// Declared byte length of the complete artifact.
        pub total_size: u64,
        /// Declared total number of chunks in the complete artifact.
        pub chunk_count: u32,
    }
}
impl crate::seal::Instruction for FinalizeSmartContractCodeUpload {}
isi! {
    /// Cancel the authority's pending upload for `code_hash`.
    ///
    /// Cancellation is owner-scoped and idempotent.
    pub struct CancelSmartContractCodeUpload {
        /// Hash identifying the pending artifact upload to discard.
        pub code_hash: iroha_crypto::Hash,
    }
}
impl crate::seal::Instruction for CancelSmartContractCodeUpload {}
isi! {
    /// Remove compiled contract bytecode from on-chain storage.
    ///
    /// The authority must hold `CanRegisterSmartContractCode`. Removal succeeds only when no
    /// manifests or active instances reference the supplied `code_hash`. An optional audit reason
    /// surfaces alongside the emitted removal event.
    pub struct RemoveSmartContractBytes {
        /// Canonical hash of the complete deployable `.to` artifact to delete.
        pub code_hash: iroha_crypto::Hash,
        /// Optional audit reason explaining why the bytecode was removed.
        #[norito(default)]
        pub reason: Option<String>,
    }
}
impl crate::seal::Instruction for RemoveSmartContractBytes {}
fn smart_contract_code_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}
impl<'a> norito::core::DecodeFromSlice<'a> for RegisterSmartContractCode {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = smart_contract_code_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let manifest = super::decode_aos_canonical_field::<ContractManifest>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { manifest }, offset))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for DeactivateContractInstance {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = smart_contract_code_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let contract_address = super::decode_aos_canonical_field::<
            crate::smart_contract::ContractAddress,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let reason = if offset < bytes.len() {
            super::decode_aos_canonical_field::<Option<String>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?
        } else {
            None
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                contract_address,
                reason,
            },
            offset,
        ))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for ActivateContractInstance {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = smart_contract_code_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let contract_address = super::decode_aos_canonical_field::<
            crate::smart_contract::ContractAddress,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let code_hash = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                contract_address,
                code_hash,
            },
            offset,
        ))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for CommitContractDeployment {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = smart_contract_code_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let expected_deploy_nonce = super::decode_aos_canonical_field::<u64>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let contract_address = super::decode_aos_canonical_field::<
            crate::smart_contract::ContractAddress,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let code_hash = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let contract_alias = super::decode_aos_canonical_field::<
            crate::smart_contract::ContractAlias,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let lease_expiry_ms = super::decode_aos_canonical_field::<Option<u64>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let expected_previous_contract_address =
            super::decode_aos_canonical_field::<Option<crate::smart_contract::ContractAddress>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                expected_deploy_nonce,
                contract_address,
                code_hash,
                contract_alias,
                lease_expiry_ms,
                expected_previous_contract_address,
            },
            offset,
        ))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for RegisterSmartContractBytes {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = smart_contract_code_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let code_hash = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let code = super::decode_aos_slice_field::<Vec<u8>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { code_hash, code }, offset))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for UploadSmartContractCodeChunk {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = smart_contract_code_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let code_hash = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let total_size = super::decode_aos_canonical_field::<u64>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let chunk_index = super::decode_aos_canonical_field::<u32>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let chunk_count = super::decode_aos_canonical_field::<u32>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let chunk = super::decode_aos_slice_field::<Vec<u8>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                code_hash,
                total_size,
                chunk_index,
                chunk_count,
                chunk,
            },
            offset,
        ))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for FinalizeSmartContractCodeUpload {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = smart_contract_code_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let code_hash = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let total_size = super::decode_aos_canonical_field::<u64>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let chunk_count = super::decode_aos_canonical_field::<u32>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                code_hash,
                total_size,
                chunk_count,
            },
            offset,
        ))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for CancelSmartContractCodeUpload {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = smart_contract_code_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let code_hash = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { code_hash }, offset))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for RemoveSmartContractBytes {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = smart_contract_code_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let code_hash = super::decode_aos_canonical_field::<iroha_crypto::Hash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let reason = if offset < bytes.len() {
            super::decode_aos_canonical_field::<Option<String>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?
        } else {
            None
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { code_hash, reason }, offset))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::AccountId,
        nexus::DataSpaceId,
        smart_contract::{ContractAddress, ContractAlias},
    };
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use norito::core::DecodeFromSlice;
    fn account() -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![0xD1; 32], Algorithm::Ed25519)
            .expect("derive checked smart-contract-code fixture account keypair");
        AccountId::new(key_pair.public_key().clone())
    }
    fn contract_address() -> ContractAddress {
        ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &account(),
            7,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address")
    }
    fn code_hash() -> Hash {
        Hash::new(b"contract-code")
    }
    fn contract_alias() -> ContractAlias {
        "payments::universal".parse().expect("contract alias")
    }
    fn manifest() -> ContractManifest {
        ContractManifest {
            seiyaku_name: None,
            code_hash: Some(code_hash()),
            abi_hash: Some(Hash::new(b"abi-policy")),
            compiler_fingerprint: Some("kotodama-1.2.3".to_owned()),
            features_bitmap: Some(0),
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
    }
    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }
    fn assert_registry_decodes<T>(registry: &crate::isi::InstructionRegistry, value: T)
    where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let wire_id = std::any::type_name::<T>();
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }
    #[test]
    fn smart_contract_code_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(RegisterSmartContractCode {
            manifest: manifest(),
        });
        assert_slice_roundtrip(DeactivateContractInstance {
            contract_address: contract_address(),
            reason: Some("governance pause".to_owned()),
        });
        assert_slice_roundtrip(ActivateContractInstance {
            contract_address: contract_address(),
            code_hash: code_hash(),
        });
        assert_slice_roundtrip(CommitContractDeployment {
            expected_deploy_nonce: 7,
            contract_address: contract_address(),
            code_hash: code_hash(),
            contract_alias: contract_alias(),
            lease_expiry_ms: Some(42_000),
            expected_previous_contract_address: Some(contract_address()),
        });
        assert_slice_roundtrip(RegisterSmartContractBytes {
            code_hash: code_hash(),
            code: vec![0x01, 0x02, 0x03],
        });
        assert_slice_roundtrip(UploadSmartContractCodeChunk {
            code_hash: code_hash(),
            total_size: 3,
            chunk_index: 0,
            chunk_count: 1,
            chunk: vec![0x01, 0x02, 0x03],
        });
        assert_slice_roundtrip(FinalizeSmartContractCodeUpload {
            code_hash: code_hash(),
            total_size: 3,
            chunk_count: 1,
        });
        assert_slice_roundtrip(CancelSmartContractCodeUpload {
            code_hash: code_hash(),
        });
        assert_slice_roundtrip(RemoveSmartContractBytes {
            code_hash: code_hash(),
            reason: Some("superseded".to_owned()),
        });
    }
    #[test]
    fn commit_contract_deployment_rejects_missing_trailing_fields() {
        let encoded = CommitContractDeployment {
            expected_deploy_nonce: 7,
            contract_address: contract_address(),
            code_hash: code_hash(),
            contract_alias: contract_alias(),
            lease_expiry_ms: None,
            expected_previous_contract_address: None,
        }
        .encode();
        let flags = norito::core::default_encode_flags();
        assert_eq!(
            flags & norito::core::header_flags::PACKED_STRUCT,
            0,
            "truncation fixture requires the canonical AoS layout"
        );
        let mut offset = 0usize;
        for _ in 0..4 {
            crate::isi::read_aos_field(&encoded, &mut offset, flags).expect("required field");
        }
        assert!(
            CommitContractDeployment::decode_from_slice(&encoded[..offset]).is_err(),
            "wire payload missing both optional-valued fields must be rejected"
        );
        crate::isi::read_aos_field(&encoded, &mut offset, flags).expect("lease field");
        assert!(
            CommitContractDeployment::decode_from_slice(&encoded[..offset]).is_err(),
            "wire payload missing expected_previous_contract_address must be rejected"
        );
    }
    #[test]
    fn smart_contract_code_registry_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<RegisterSmartContractCode>()
            .register_slice::<DeactivateContractInstance>()
            .register_slice::<ActivateContractInstance>()
            .register_slice::<CommitContractDeployment>()
            .register_slice::<RegisterSmartContractBytes>()
            .register_slice::<UploadSmartContractCodeChunk>()
            .register_slice::<FinalizeSmartContractCodeUpload>()
            .register_slice::<CancelSmartContractCodeUpload>()
            .register_slice::<RemoveSmartContractBytes>();
        assert_registry_decodes(
            &registry,
            RegisterSmartContractCode {
                manifest: manifest(),
            },
        );
        assert_registry_decodes(
            &registry,
            DeactivateContractInstance {
                contract_address: contract_address(),
                reason: Some("governance pause".to_owned()),
            },
        );
        assert_registry_decodes(
            &registry,
            ActivateContractInstance {
                contract_address: contract_address(),
                code_hash: code_hash(),
            },
        );
        assert_registry_decodes(
            &registry,
            CommitContractDeployment {
                expected_deploy_nonce: 7,
                contract_address: contract_address(),
                code_hash: code_hash(),
                contract_alias: contract_alias(),
                lease_expiry_ms: Some(42_000),
                expected_previous_contract_address: Some(contract_address()),
            },
        );
        assert_registry_decodes(
            &registry,
            RegisterSmartContractBytes {
                code_hash: code_hash(),
                code: vec![0x01, 0x02, 0x03],
            },
        );
        assert_registry_decodes(
            &registry,
            UploadSmartContractCodeChunk {
                code_hash: code_hash(),
                total_size: 3,
                chunk_index: 0,
                chunk_count: 1,
                chunk: vec![0x01, 0x02, 0x03],
            },
        );
        assert_registry_decodes(
            &registry,
            FinalizeSmartContractCodeUpload {
                code_hash: code_hash(),
                total_size: 3,
                chunk_count: 1,
            },
        );
        assert_registry_decodes(
            &registry,
            CancelSmartContractCodeUpload {
                code_hash: code_hash(),
            },
        );
        assert_registry_decodes(
            &registry,
            RemoveSmartContractBytes {
                code_hash: code_hash(),
                reason: Some("superseded".to_owned()),
            },
        );
    }
    #[test]
    fn default_instruction_registry_decodes_native_upload_instructions() {
        let registry = crate::instruction_registry::default();
        assert_registry_decodes(
            &registry,
            UploadSmartContractCodeChunk {
                code_hash: code_hash(),
                total_size: 3,
                chunk_index: 0,
                chunk_count: 1,
                chunk: vec![0x01, 0x02, 0x03],
            },
        );
        assert_registry_decodes(
            &registry,
            FinalizeSmartContractCodeUpload {
                code_hash: code_hash(),
                total_size: 3,
                chunk_count: 1,
            },
        );
        assert_registry_decodes(
            &registry,
            CancelSmartContractCodeUpload {
                code_hash: code_hash(),
            },
        );
        assert_registry_decodes(
            &registry,
            CommitContractDeployment {
                expected_deploy_nonce: 7,
                contract_address: contract_address(),
                code_hash: code_hash(),
                contract_alias: contract_alias(),
                lease_expiry_ms: None,
                expected_previous_contract_address: None,
            },
        );
    }
}
