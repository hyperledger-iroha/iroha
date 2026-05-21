use super::*;
use crate::smart_contract::manifest::ContractManifest;

isi! {
    /// Register a smart contract manifest keyed by `code_hash` into the WSV.
    ///
    /// Note: current implementation stores only the manifest. Large code
    /// artifacts may be referenced off-chain by `code_hash`.
    pub struct RegisterSmartContractCode {
        /// Manifest containing `code_hash` (required) and `abi_hash`.
        pub manifest: ContractManifest,
    }
}

impl crate::seal::Instruction for RegisterSmartContractCode {}

isi! {
    /// Deactivate a contract instance by removing the `contract_address` binding.
    ///
    /// Deactivation acts as a governance kill-switch for compromised deployments. The address
    /// becomes unavailable immediately, while provenance information (caller and optional reason)
    /// is emitted via the data event stream.
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
    /// Activate a contract instance by binding `contract_address` to a `code_hash`.
    ///
    /// This creates or updates the canonical routing for a contract address. Nodes use this
    /// mapping to resolve which bytecode to execute for calls into that address.
    pub struct ActivateContractInstance {
        /// Canonical contract address.
        pub contract_address: crate::smart_contract::ContractAddress,
        /// Content-addressed code hash (Blake2b-32) of the `.to` bytecode to bind.
        pub code_hash: iroha_crypto::Hash,
    }
}

impl crate::seal::Instruction for ActivateContractInstance {}

isi! {
    /// Register compiled contract bytecode on-chain keyed by its `code_hash`.
    ///
    /// The bytecode is the full compiled `.to` image including the IVM header.
    /// Nodes verify that `code_hash` equals the Blake2b-32 digest of the program body
    /// (bytes after the IVM header) before storing.
    pub struct RegisterSmartContractBytes {
        /// Hash of the program body bytes (after IVM header).
        pub code_hash: iroha_crypto::Hash,
        /// Full compiled `.to` image (including IVM header).
        pub code: Vec<u8>,
    }
}

impl crate::seal::Instruction for RegisterSmartContractBytes {}

isi! {
    /// Remove compiled contract bytecode from on-chain storage.
    ///
    /// Removal succeeds only when no manifests or active instances reference the supplied
    /// `code_hash`. Governance operators can provide an optional audit reason that surfaces
    /// alongside the emitted removal event.
    pub struct RemoveSmartContractBytes {
        /// Hash of the program body bytes (after the IVM header) identifying the artifact to delete.
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
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{account::AccountId, nexus::DataSpaceId, smart_contract::ContractAddress};

    fn account() -> AccountId {
        let key_pair = KeyPair::from_seed(vec![0xD1; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn contract_address() -> ContractAddress {
        ContractAddress::derive(0x1234, &account(), 7, DataSpaceId::UNIVERSAL)
            .expect("contract address")
    }

    fn code_hash() -> Hash {
        Hash::new(b"contract-code")
    }

    fn manifest() -> ContractManifest {
        ContractManifest {
            code_hash: Some(code_hash()),
            abi_hash: Some(Hash::new(b"abi-policy")),
            compiler_fingerprint: Some("kotodama-1.2.3".to_owned()),
            features_bitmap: Some(0),
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
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
        assert_slice_roundtrip(RegisterSmartContractBytes {
            code_hash: code_hash(),
            code: vec![0x01, 0x02, 0x03],
        });
        assert_slice_roundtrip(RemoveSmartContractBytes {
            code_hash: code_hash(),
            reason: Some("superseded".to_owned()),
        });
    }

    #[test]
    fn smart_contract_code_registry_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<RegisterSmartContractCode>()
            .register_slice::<DeactivateContractInstance>()
            .register_slice::<ActivateContractInstance>()
            .register_slice::<RegisterSmartContractBytes>()
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
            RegisterSmartContractBytes {
                code_hash: code_hash(),
                code: vec![0x01, 0x02, 0x03],
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
}
