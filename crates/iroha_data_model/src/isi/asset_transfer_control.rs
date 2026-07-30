//! Built-in instructions for account- and asset-scoped transfer controls.

use std::{format, string::String, vec::Vec};

use super::*;
use crate::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetTransferAvailability, AssetTransferLimit},
};

isi! {
    /// Atomically update incoming and outgoing account-to-account transfer availability.
    ///
    /// Supply operations such as mint and burn are deliberately outside this
    /// transfer policy. Use a holding limit to constrain all native credits.
    pub struct SetAssetTransferAvailability {
        /// Controlled account.
        pub account_id: AccountId,
        /// Controlled asset definition.
        pub asset_definition_id: AssetDefinitionId,
        /// Availability revision that must currently be stored.
        pub expected_revision: u64,
        /// Desired incoming-movement state.
        pub incoming: AssetTransferAvailability,
        /// Desired outgoing-movement state.
        pub outgoing: AssetTransferAvailability,
        /// Optional operator reason persisted with this transition.
        #[norito(default)]
        pub reason: Option<String>,
    }
}

isi! {
    /// Set or clear the post-credit balance limit for an account and asset definition.
    ///
    /// The limit applies to every future native credit, including transfers, mints,
    /// settlements, and offline escrow movements. It is evaluated independently for each
    /// concrete routed balance bucket. The limit may be set below the current balance; a
    /// zero limit therefore closes inbound credit without altering existing funds.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SetAssetHoldingLimit {
        /// Controlled account.
        pub account_id: AccountId,
        /// Controlled asset definition.
        pub asset_definition_id: AssetDefinitionId,
        /// Maximum post-credit balance, or `None` to clear the limit.
        pub holding_limit: Option<iroha_primitives::numeric::Quantity>,
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

impl SetAssetTransferAvailability {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.asset.transfer.availability.set";

    /// Convenience constructor.
    #[must_use]
    pub fn new(
        account_id: AccountId,
        asset_definition_id: AssetDefinitionId,
        expected_revision: u64,
        incoming: AssetTransferAvailability,
        outgoing: AssetTransferAvailability,
        reason: Option<String>,
    ) -> Self {
        Self {
            account_id,
            asset_definition_id,
            expected_revision,
            incoming,
            outgoing,
            reason,
        }
    }
}

impl SetAssetHoldingLimit {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.asset.holding_limit.set";

    /// Convenience constructor.
    #[must_use]
    pub fn new(
        account_id: AccountId,
        asset_definition_id: AssetDefinitionId,
        holding_limit: Option<iroha_primitives::numeric::Quantity>,
    ) -> Self {
        Self {
            account_id,
            asset_definition_id,
            holding_limit,
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

impl crate::seal::Instruction for SetAssetTransferAvailability {}
impl crate::seal::Instruction for SetAssetHoldingLimit {}
impl crate::seal::Instruction for SetAssetTransferBlacklist {}
impl crate::seal::Instruction for SetAssetTransferControl {}

impl<'a> norito::core::DecodeFromSlice<'a> for SetAssetTransferAvailability {
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
        let expected_revision = super::decode_aos_canonical_field::<u64>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let incoming = super::decode_aos_canonical_field::<AssetTransferAvailability>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let outgoing = super::decode_aos_canonical_field::<AssetTransferAvailability>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let reason = super::decode_aos_canonical_field::<Option<String>>(
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
                expected_revision,
                incoming,
                outgoing,
                reason,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SetAssetHoldingLimit {
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
        let holding_limit = super::decode_aos_canonical_field::<
            Option<iroha_primitives::numeric::Quantity>,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                account_id,
                asset_definition_id,
                holding_limit,
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

        assert_slice_roundtrip(SetAssetTransferAvailability::new(
            account_id.clone(),
            asset_definition_id.clone(),
            4,
            AssetTransferAvailability::Disabled,
            AssetTransferAvailability::Enabled,
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
        assert_slice_roundtrip(SetAssetHoldingLimit::new(
            account(0x93),
            asset_definition(),
            Some(50_u32.into()),
        ));
    }

    #[test]
    fn asset_transfer_control_registry_decodes_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<SetAssetTransferAvailability>(
                SetAssetTransferAvailability::WIRE_ID,
            )
            .register_with_id_slice::<SetAssetTransferBlacklist>(SetAssetTransferBlacklist::WIRE_ID)
            .register_with_id_slice::<SetAssetTransferControl>(SetAssetTransferControl::WIRE_ID)
            .register_with_id_slice::<SetAssetHoldingLimit>(SetAssetHoldingLimit::WIRE_ID);
        let account_id = account(0x92);
        let asset_definition_id = asset_definition();

        assert_registry_decodes(
            &registry,
            SetAssetTransferAvailability::WIRE_ID,
            SetAssetTransferAvailability::new(
                account_id.clone(),
                asset_definition_id.clone(),
                0,
                AssetTransferAvailability::Disabled,
                AssetTransferAvailability::Disabled,
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
        assert_registry_decodes(
            &registry,
            SetAssetHoldingLimit::WIRE_ID,
            SetAssetHoldingLimit::new(account(0x94), asset_definition(), Some(25_u32.into())),
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn structured_json_availability_decodes_exact_cas_instruction() {
        use crate::isi::{Instruction as _, InstructionBox};

        let account_id = account(0x96);
        let asset_definition_id = asset_definition();
        let payload = format!(
            r#"{{"name":"SetAssetTransferAvailability","params":{{"account_id":"{account_id}","asset_definition_id":"{asset_definition_id}","expected_revision":7,"incoming":"Disabled","outgoing":"Enabled","reason":"operator review"}}}}"#
        );

        let decoded =
            norito::json::from_str::<InstructionBox>(&payload).expect("decode structured ISI JSON");
        let instruction = decoded
            .as_any()
            .downcast_ref::<SetAssetTransferAvailability>()
            .expect("native availability instruction");
        assert_eq!(instruction.account_id, account_id);
        assert_eq!(instruction.asset_definition_id, asset_definition_id);
        assert_eq!(instruction.expected_revision, 7);
        assert_eq!(instruction.incoming, AssetTransferAvailability::Disabled);
        assert_eq!(instruction.outgoing, AssetTransferAvailability::Enabled);
        assert_eq!(instruction.reason.as_deref(), Some("operator review"));

        for invalid_params in [
            r#""expected_revision":7,"incoming":"disabled","outgoing":"Enabled""#,
            r#""expected_revision":7,"incoming":"Disabled","outgoing":"Enabled","reason":" padded""#,
            r#""expected_revision":7,"incoming":"Disabled","outgoing":"Enabled","legacy":true"#,
        ] {
            let invalid = format!(
                r#"{{"name":"SetAssetTransferAvailability","params":{{"account_id":"{account_id}","asset_definition_id":"{asset_definition_id}",{invalid_params}}}}}"#
            );
            norito::json::from_str::<InstructionBox>(&invalid)
                .expect_err("non-canonical availability JSON must be rejected");
        }
    }

    #[cfg(feature = "json")]
    #[test]
    fn structured_json_holding_limit_decodes_to_native_instruction() {
        use crate::isi::{Instruction as _, InstructionBox};

        let account_id = account(0x95);
        let asset_definition_id = asset_definition();
        let payload = format!(
            r#"{{"name":"SetAssetHoldingLimit","params":{{"account_id":"{account_id}","asset_definition_id":"{asset_definition_id}","holding_limit":"0"}}}}"#
        );

        let decoded =
            norito::json::from_str::<InstructionBox>(&payload).expect("decode structured ISI JSON");
        let instruction = decoded
            .as_any()
            .downcast_ref::<SetAssetHoldingLimit>()
            .expect("native holding-limit instruction");
        assert_eq!(instruction.account_id, account_id);
        assert_eq!(instruction.asset_definition_id, asset_definition_id);
        assert_eq!(
            instruction.holding_limit,
            Some(iroha_primitives::numeric::Quantity::zero())
        );
    }
}
