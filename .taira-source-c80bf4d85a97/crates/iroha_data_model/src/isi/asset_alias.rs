//! Asset-definition alias binding instructions.

use super::*;

isi! {
    /// Bind, update, or clear an alias for an existing asset definition.
    ///
    /// `alias = None` clears the current binding.
    /// `alias = Some(...)` sets/updates the binding and optionally refreshes lease metadata.
    pub struct SetAssetDefinitionAlias {
        /// Asset definition that should be updated.
        pub asset_definition_id: AssetDefinitionId,
        /// Alias literal (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`). `None` clears
        /// the binding.
        #[norito(default)]
        pub alias: Option<AssetDefinitionAlias>,
        /// Optional lease expiry timestamp (unix ms). When absent, the binding is treated as non-expiring.
        #[norito(default)]
        pub lease_expiry_ms: Option<u64>,
    }
}

isi! {
    /// Change the balance partition policy for an existing asset definition.
    ///
    /// Transitioning from `Global` to `DataspaceRestricted` may migrate existing global balance
    /// buckets into one dataspace-scoped bucket. Transitioning from `DataspaceRestricted` back to
    /// `Global` is intentionally rejected by the executor.
    pub struct SetAssetDefinitionBalancePolicy {
        /// Asset definition that should be updated.
        pub asset_definition_id: AssetDefinitionId,
        /// New balance partition policy.
        pub balance_scope_policy: crate::asset::AssetBalancePolicy,
        /// Dataspace that should receive existing global buckets when restricting a legacy asset.
        #[norito(default)]
        pub migrate_global_balances_to_dataspace: Option<crate::nexus::DataSpaceId>,
    }
}

impl SetAssetDefinitionAlias {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.asset_definition.alias.set";

    /// Create a binding/update instruction.
    #[must_use]
    pub fn bind(
        asset_definition_id: AssetDefinitionId,
        alias: AssetDefinitionAlias,
        lease_expiry_ms: Option<u64>,
    ) -> Self {
        Self {
            asset_definition_id,
            alias: Some(alias),
            lease_expiry_ms,
        }
    }

    /// Create an instruction that clears the current alias binding.
    #[must_use]
    pub fn clear(asset_definition_id: AssetDefinitionId) -> Self {
        Self {
            asset_definition_id,
            alias: None,
            lease_expiry_ms: None,
        }
    }
}

impl crate::seal::Instruction for SetAssetDefinitionAlias {}

impl SetAssetDefinitionBalancePolicy {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.asset_definition.balance_policy.set";

    /// Build a policy update instruction.
    #[must_use]
    pub const fn new(
        asset_definition_id: AssetDefinitionId,
        balance_scope_policy: crate::asset::AssetBalancePolicy,
        migrate_global_balances_to_dataspace: Option<crate::nexus::DataSpaceId>,
    ) -> Self {
        Self {
            asset_definition_id,
            balance_scope_policy,
            migrate_global_balances_to_dataspace,
        }
    }
}

impl crate::seal::Instruction for SetAssetDefinitionBalancePolicy {}

impl<'a> norito::core::DecodeFromSlice<'a> for SetAssetDefinitionAlias {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let asset_definition_id = super::decode_aos_canonical_field::<AssetDefinitionId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let alias = if offset < bytes.len() {
            super::decode_aos_canonical_field::<Option<AssetDefinitionAlias>>(
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
                asset_definition_id,
                alias,
                lease_expiry_ms,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SetAssetDefinitionBalancePolicy {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let asset_definition_id = super::decode_aos_canonical_field::<AssetDefinitionId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let balance_scope_policy = super::decode_aos_canonical_field::<
            crate::asset::AssetBalancePolicy,
        >(
            super::read_aos_field(bytes, &mut offset, flags)?, flags
        )?;
        let migrate_global_balances_to_dataspace = if offset < bytes.len() {
            super::decode_aos_canonical_field::<Option<crate::nexus::DataSpaceId>>(
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
                asset_definition_id,
                balance_scope_policy,
                migrate_global_balances_to_dataspace,
            },
            offset,
        ))
    }
}

#[cfg(test)]
mod tests {
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        asset::{AssetBalancePolicy, AssetDefinitionAlias},
        nexus::DataSpaceId,
    };

    fn asset_definition() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "rose".parse().expect("asset name"),
        )
    }

    fn asset_alias() -> AssetDefinitionAlias {
        AssetDefinitionAlias::from_components("rose", Some("wonderland"), "universal")
            .expect("asset alias")
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

    fn assert_registry_decodes<T>(
        registry: &crate::isi::InstructionRegistry,
        wire_id: &'static str,
        value: T,
    ) where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn asset_alias_decode_from_slice_roundtrips() {
        let definition = asset_definition();
        assert_slice_roundtrip(SetAssetDefinitionAlias::bind(
            definition.clone(),
            asset_alias(),
            Some(42),
        ));
        assert_slice_roundtrip(SetAssetDefinitionAlias::clear(definition.clone()));
        assert_slice_roundtrip(SetAssetDefinitionBalancePolicy::new(
            definition,
            AssetBalancePolicy::DataspaceRestricted,
            Some(DataSpaceId::UNIVERSAL),
        ));
    }

    #[test]
    fn asset_alias_registry_decodes_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<SetAssetDefinitionAlias>(SetAssetDefinitionAlias::WIRE_ID)
            .register_with_id_slice::<SetAssetDefinitionBalancePolicy>(
                SetAssetDefinitionBalancePolicy::WIRE_ID,
            );
        let definition = asset_definition();

        assert_registry_decodes(
            &registry,
            SetAssetDefinitionAlias::WIRE_ID,
            SetAssetDefinitionAlias::bind(definition.clone(), asset_alias(), Some(7)),
        );
        assert_registry_decodes(
            &registry,
            SetAssetDefinitionBalancePolicy::WIRE_ID,
            SetAssetDefinitionBalancePolicy::new(
                definition,
                AssetBalancePolicy::DataspaceRestricted,
                Some(DataSpaceId::UNIVERSAL),
            ),
        );
    }
}
