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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::AssetDefinitionAlias;
    use crate::isi::test_support::{assert_registry_decodes, assert_slice_roundtrip};
    fn asset_definition() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "rose".parse().expect("asset name"),
        )
    }
    fn asset_alias() -> AssetDefinitionAlias {
        AssetDefinitionAlias::from_components("rose", Some("wonderland"), "universal")
            .expect("asset alias")
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
        let _ = definition;
    }
    #[test]
    fn asset_alias_registry_decodes_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<SetAssetDefinitionAlias>(SetAssetDefinitionAlias::WIRE_ID);
        let definition = asset_definition();
        assert_registry_decodes(
            &registry,
            SetAssetDefinitionAlias::WIRE_ID,
            SetAssetDefinitionAlias::bind(definition.clone(), asset_alias(), Some(7)),
        );
    }
}
