//! Built-in instructions for asset-scoped outbound transfer controls.

use std::{format, string::String, vec::Vec};

use super::*;
use crate::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetTransferLimit},
};

isi! {
    /// Freeze or unfreeze outbound transfers for an account on one asset definition.
    pub struct SetAssetTransferFreeze {
        /// Controlled account.
        pub account_id: AccountId,
        /// Controlled asset definition.
        pub asset_definition_id: AssetDefinitionId,
        /// Desired outbound-freeze state.
        pub outgoing_frozen: bool,
        /// Optional operator reason attached to the proposal intent.
        #[norito(default)]
        pub reason: Option<String>,
    }
}

isi! {
    /// Blacklist or clear outbound transfers for an account on one asset definition.
    pub struct SetAssetTransferBlacklist {
        /// Controlled account.
        pub account_id: AccountId,
        /// Controlled asset definition.
        pub asset_definition_id: AssetDefinitionId,
        /// Desired blacklist state.
        pub blacklisted: bool,
    }
}

isi! {
    /// Replace calendar-window outbound transfer caps for an account on one asset definition.
    pub struct SetAssetTransferControl {
        /// Controlled account.
        pub account_id: AccountId,
        /// Controlled asset definition.
        pub asset_definition_id: AssetDefinitionId,
        /// Complete set of day/week/month caps to apply.
        #[norito(default)]
        pub limits: Vec<AssetTransferLimit>,
    }
}

impl SetAssetTransferFreeze {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.asset.transfer.freeze.set";

    /// Convenience constructor.
    #[must_use]
    pub fn new(
        account_id: AccountId,
        asset_definition_id: AssetDefinitionId,
        outgoing_frozen: bool,
        reason: Option<String>,
    ) -> Self {
        Self {
            account_id,
            asset_definition_id,
            outgoing_frozen,
            reason,
        }
    }
}

impl SetAssetTransferBlacklist {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.asset.transfer.blacklist.set";

    /// Convenience constructor.
    #[must_use]
    pub fn new(
        account_id: AccountId,
        asset_definition_id: AssetDefinitionId,
        blacklisted: bool,
    ) -> Self {
        Self {
            account_id,
            asset_definition_id,
            blacklisted,
        }
    }
}

impl SetAssetTransferControl {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.asset.transfer.control.set";

    /// Convenience constructor.
    #[must_use]
    pub fn new(
        account_id: AccountId,
        asset_definition_id: AssetDefinitionId,
        limits: Vec<AssetTransferLimit>,
    ) -> Self {
        Self {
            account_id,
            asset_definition_id,
            limits,
        }
    }
}

impl crate::seal::Instruction for SetAssetTransferFreeze {}
impl crate::seal::Instruction for SetAssetTransferBlacklist {}
impl crate::seal::Instruction for SetAssetTransferControl {}

impl<'a> norito::core::DecodeFromSlice<'a> for SetAssetTransferFreeze {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let account_id = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let asset_definition_id = super::decode_aos_canonical_field::<AssetDefinitionId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let outgoing_frozen = super::decode_aos_canonical_field::<bool>(
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
        Ok((
            Self {
                account_id,
                asset_definition_id,
                outgoing_frozen,
                reason,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SetAssetTransferBlacklist {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let account_id = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let asset_definition_id = super::decode_aos_canonical_field::<AssetDefinitionId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let blacklisted = super::decode_aos_canonical_field::<bool>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                account_id,
                asset_definition_id,
                blacklisted,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SetAssetTransferControl {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let account_id = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let asset_definition_id = super::decode_aos_canonical_field::<AssetDefinitionId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let limits = if offset < bytes.len() {
            super::decode_aos_slice_field::<Vec<AssetTransferLimit>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?
        } else {
            Vec::new()
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                account_id,
                asset_definition_id,
                limits,
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
    use crate::asset::AssetTransferControlWindow;

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked asset-transfer-control fixture keypair");
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset_definition() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "rose".parse().expect("asset name"),
        )
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
    fn asset_transfer_control_decode_from_slice_roundtrips() {
        let account_id = account(0x91);
        let asset_definition_id = asset_definition();

        assert_slice_roundtrip(SetAssetTransferFreeze::new(
            account_id.clone(),
            asset_definition_id.clone(),
            true,
            Some("court order".to_owned()),
        ));
        assert_slice_roundtrip(SetAssetTransferBlacklist::new(
            account_id.clone(),
            asset_definition_id.clone(),
            true,
        ));
        assert_slice_roundtrip(SetAssetTransferControl::new(
            account_id,
            asset_definition_id,
            vec![AssetTransferLimit {
                window: AssetTransferControlWindow::Day,
                cap_amount: Some(10_u32.into()),
            }],
        ));
    }

    #[test]
    fn asset_transfer_control_registry_decodes_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<SetAssetTransferFreeze>(SetAssetTransferFreeze::WIRE_ID)
            .register_with_id_slice::<SetAssetTransferBlacklist>(SetAssetTransferBlacklist::WIRE_ID)
            .register_with_id_slice::<SetAssetTransferControl>(SetAssetTransferControl::WIRE_ID);
        let account_id = account(0x92);
        let asset_definition_id = asset_definition();

        assert_registry_decodes(
            &registry,
            SetAssetTransferFreeze::WIRE_ID,
            SetAssetTransferFreeze::new(
                account_id.clone(),
                asset_definition_id.clone(),
                false,
                None,
            ),
        );
        assert_registry_decodes(
            &registry,
            SetAssetTransferBlacklist::WIRE_ID,
            SetAssetTransferBlacklist::new(account_id.clone(), asset_definition_id.clone(), false),
        );
        assert_registry_decodes(
            &registry,
            SetAssetTransferControl::WIRE_ID,
            SetAssetTransferControl::new(
                account_id,
                asset_definition_id,
                vec![AssetTransferLimit {
                    window: AssetTransferControlWindow::Month,
                    cap_amount: None,
                }],
            ),
        );
    }
}
