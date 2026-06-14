//! Contract alias binding instructions.

use super::*;

isi! {
    /// Bind, update, or clear an alias for an existing contract address.
    ///
    /// `alias = None` clears the current binding.
    /// `alias = Some(...)` sets or updates the binding and optionally refreshes lease metadata.
    pub struct SetContractAlias {
        /// Contract address that should be updated.
        pub contract_address: ContractAddress,
        /// Alias literal (`<name>::<domain>.<dataspace>` or `<name>::<dataspace>`). `None`
        /// clears the binding.
        #[norito(default)]
        pub alias: Option<ContractAlias>,
        /// Optional lease expiry timestamp (unix ms). When absent, the binding is treated as
        /// non-expiring.
        #[norito(default)]
        pub lease_expiry_ms: Option<u64>,
    }
}

impl SetContractAlias {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.contract.alias.set";

    /// Create a binding or update instruction.
    #[must_use]
    pub fn bind(
        contract_address: ContractAddress,
        alias: ContractAlias,
        lease_expiry_ms: Option<u64>,
    ) -> Self {
        Self {
            contract_address,
            alias: Some(alias),
            lease_expiry_ms,
        }
    }

    /// Create an instruction that clears the current alias binding.
    #[must_use]
    pub fn clear(contract_address: ContractAddress) -> Self {
        Self {
            contract_address,
            alias: None,
            lease_expiry_ms: None,
        }
    }
}

impl crate::seal::Instruction for SetContractAlias {}

impl<'a> norito::core::DecodeFromSlice<'a> for SetContractAlias {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let contract_address = super::decode_aos_canonical_field::<ContractAddress>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let alias = if offset < bytes.len() {
            super::decode_aos_canonical_field::<Option<ContractAlias>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?
        } else {
            None
        };
        let lease_expiry_ms = if offset < bytes.len() {
            super::decode_aos_canonical_field::<Option<u64>>(
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
                alias,
                lease_expiry_ms,
            },
            offset,
        ))
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::nexus::DataSpaceId;

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked contract-alias ISI fixture keypair");
        AccountId::new(key_pair.public_key().clone())
    }

    fn contract_address() -> ContractAddress {
        ContractAddress::derive(0x1234, &account(0xC1), 7, DataSpaceId::UNIVERSAL)
            .expect("contract address")
    }

    fn contract_alias() -> ContractAlias {
        ContractAlias::from_components("router", Some("dex"), "universal").expect("contract alias")
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

    #[test]
    fn contract_alias_decode_from_slice_roundtrips() {
        let address = contract_address();
        assert_slice_roundtrip(SetContractAlias::bind(
            address.clone(),
            contract_alias(),
            Some(5000),
        ));
        assert_slice_roundtrip(SetContractAlias::clear(address));
    }

    #[test]
    fn contract_alias_registry_decodes_stable_id() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<SetContractAlias>(SetContractAlias::WIRE_ID);
        let value = SetContractAlias::bind(contract_address(), contract_alias(), Some(6000));
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<SetContractAlias>(&payload, flags)
                .expect("frame contract alias");
        let decoded =
            crate::isi::InstructionRegistry::decode(&registry, SetContractAlias::WIRE_ID, &framed)
                .expect("registered contract alias")
                .expect("decode contract alias");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }
}
