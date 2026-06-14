//! Dedicated instructions for real-world asset lots.

use iroha_primitives::numeric::Numeric;

use super::*;
use crate::rwa::{NewRwa, Rwa, RwaControlPolicy, RwaId, RwaParentRef};

isi! {
    /// Issue a new canonical RWA lot.
    pub struct RegisterRwa {
        /// Registration payload for the new lot.
        pub rwa: NewRwa,
    }
}

isi! {
    /// Transfer quantity from an existing lot into a destination account.
    pub struct TransferRwa {
        /// Current owner recorded on the source lot.
        pub source: AccountId,
        /// Lot being transferred.
        pub rwa: RwaId,
        /// Quantity to transfer.
        pub quantity: Numeric,
        /// Destination account that will own the transferred lot.
        pub destination: AccountId,
    }
}

isi! {
    /// Create a derived lot by merging one or more parent lots.
    pub struct MergeRwas {
        /// Parent lots and quantities contributed into the merged result.
        pub parents: Vec<RwaParentRef>,
        /// Primary reference for the derived lot.
        pub primary_reference: String,
        /// Optional status label for the derived lot.
        pub status: Option<Name>,
        /// Metadata for the derived lot.
        pub metadata: Metadata,
    }
}

isi! {
    /// Redeem a quantity from an existing lot.
    pub struct RedeemRwa {
        /// Lot being redeemed.
        pub rwa: RwaId,
        /// Quantity to redeem.
        pub quantity: Numeric,
    }
}

isi! {
    /// Freeze an existing lot.
    pub struct FreezeRwa {
        /// Lot being frozen.
        pub rwa: RwaId,
    }
}

isi! {
    /// Unfreeze an existing lot.
    pub struct UnfreezeRwa {
        /// Lot being unfrozen.
        pub rwa: RwaId,
    }
}

isi! {
    /// Reserve a quantity on an existing lot.
    pub struct HoldRwa {
        /// Lot receiving the hold.
        pub rwa: RwaId,
        /// Quantity to reserve.
        pub quantity: Numeric,
    }
}

isi! {
    /// Release a reserved quantity on an existing lot.
    pub struct ReleaseRwa {
        /// Lot receiving the release.
        pub rwa: RwaId,
        /// Quantity to release.
        pub quantity: Numeric,
    }
}

isi! {
    /// Perform a controller-driven transfer from an existing lot.
    pub struct ForceTransferRwa {
        /// Lot being moved.
        pub rwa: RwaId,
        /// Quantity to move.
        pub quantity: Numeric,
        /// Destination account that will own the transferred lot.
        pub destination: AccountId,
    }
}

isi! {
    /// Replace the control policy on an existing lot.
    pub struct SetRwaControls {
        /// Lot whose controls are being replaced.
        pub rwa: RwaId,
        /// Full replacement policy.
        pub controls: RwaControlPolicy,
    }
}

impl core::fmt::Display for RegisterRwa {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "REGISTER_RWA `{}`", self.rwa.primary_reference)
    }
}

impl core::fmt::Display for TransferRwa {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "TRANSFER_RWA `{}` {} `{}` -> `{}`",
            self.rwa, self.quantity, self.source, self.destination
        )
    }
}

impl core::fmt::Display for MergeRwas {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "MERGE_RWAS {}", self.parents.len())
    }
}

impl core::fmt::Display for RedeemRwa {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "REDEEM_RWA `{}` {}", self.rwa, self.quantity)
    }
}

impl core::fmt::Display for FreezeRwa {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "FREEZE_RWA `{}`", self.rwa)
    }
}

impl core::fmt::Display for UnfreezeRwa {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "UNFREEZE_RWA `{}`", self.rwa)
    }
}

impl core::fmt::Display for HoldRwa {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "HOLD_RWA `{}` {}", self.rwa, self.quantity)
    }
}

impl core::fmt::Display for ReleaseRwa {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "RELEASE_RWA `{}` {}", self.rwa, self.quantity)
    }
}

impl core::fmt::Display for ForceTransferRwa {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "FORCE_TRANSFER_RWA `{}` {} -> `{}`",
            self.rwa, self.quantity, self.destination
        )
    }
}

impl core::fmt::Display for SetRwaControls {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "SET_RWA_CONTROLS `{}`", self.rwa)
    }
}

impl RegisterRwa {
    /// Stable wire identifier for RWA instruction grouping.
    pub const WIRE_ID: &'static str = "iroha.rwa.register";
}

impl TransferRwa {
    /// Stable wire identifier for lot transfers.
    pub const WIRE_ID: &'static str = "iroha.rwa.transfer";
}

impl MergeRwas {
    /// Stable wire identifier for lot merges.
    pub const WIRE_ID: &'static str = "iroha.rwa.merge";
}

impl RedeemRwa {
    /// Stable wire identifier for lot redemption.
    pub const WIRE_ID: &'static str = "iroha.rwa.redeem";
}

impl FreezeRwa {
    /// Stable wire identifier for lot freezing.
    pub const WIRE_ID: &'static str = "iroha.rwa.freeze";
}

impl UnfreezeRwa {
    /// Stable wire identifier for lot unfreezing.
    pub const WIRE_ID: &'static str = "iroha.rwa.unfreeze";
}

impl HoldRwa {
    /// Stable wire identifier for lot holds.
    pub const WIRE_ID: &'static str = "iroha.rwa.hold";
}

impl ReleaseRwa {
    /// Stable wire identifier for releasing holds.
    pub const WIRE_ID: &'static str = "iroha.rwa.release";
}

impl ForceTransferRwa {
    /// Stable wire identifier for controller-driven transfers.
    pub const WIRE_ID: &'static str = "iroha.rwa.force_transfer";
}

impl SetRwaControls {
    /// Stable wire identifier for control-policy replacement.
    pub const WIRE_ID: &'static str = "iroha.rwa.set_controls";
}

impl crate::seal::Instruction for RegisterRwa {}
impl crate::seal::Instruction for TransferRwa {}
impl crate::seal::Instruction for MergeRwas {}
impl crate::seal::Instruction for RedeemRwa {}
impl crate::seal::Instruction for FreezeRwa {}
impl crate::seal::Instruction for UnfreezeRwa {}
impl crate::seal::Instruction for HoldRwa {}
impl crate::seal::Instruction for ReleaseRwa {}
impl crate::seal::Instruction for ForceTransferRwa {}
impl crate::seal::Instruction for SetRwaControls {}
impl crate::seal::Instruction for SetKeyValue<Rwa> {}
impl crate::seal::Instruction for RemoveKeyValue<Rwa> {}

isi_box! {
    /// Grouping enum for RWA-related instructions.
    pub enum RwaInstructionBox {
        /// Register a new lot.
        Register(RegisterRwa),
        /// Transfer quantity out of an existing lot.
        Transfer(TransferRwa),
        /// Merge multiple lots into a derived lot.
        Merge(MergeRwas),
        /// Redeem quantity from a lot.
        Redeem(RedeemRwa),
        /// Freeze a lot.
        Freeze(FreezeRwa),
        /// Unfreeze a lot.
        Unfreeze(UnfreezeRwa),
        /// Hold quantity on a lot.
        Hold(HoldRwa),
        /// Release held quantity on a lot.
        Release(ReleaseRwa),
        /// Force transfer quantity from a lot.
        ForceTransfer(ForceTransferRwa),
        /// Replace control policy on a lot.
        SetControls(SetRwaControls),
        /// Set lot metadata.
        SetKeyValue(SetKeyValue<Rwa>),
        /// Remove lot metadata.
        RemoveKeyValue(RemoveKeyValue<Rwa>),
    }
}

impl RwaInstructionBox {
    /// Stable wire identifier for the grouped RWA instruction family.
    pub const WIRE_ID: &'static str = "iroha.rwa";
}

impl_into_box! {
    RegisterRwa |
    TransferRwa |
    MergeRwas |
    RedeemRwa |
    FreezeRwa |
    UnfreezeRwa |
    HoldRwa |
    ReleaseRwa |
    ForceTransferRwa |
    SetRwaControls |
    SetKeyValue<Rwa> |
    RemoveKeyValue<Rwa>
=> RwaInstructionBox
}

impl crate::seal::Instruction for RwaInstructionBox {}

fn rwa_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_rwa_decode_from_slice {
    ($ty:ty { $($field:ident : $field_ty:ty),+ $(,)? }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = rwa_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                $(
                    let $field = super::decode_aos_canonical_field::<$field_ty>(
                        super::read_aos_field(bytes, &mut offset, flags)?,
                        flags,
                    )?;
                )+
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $($field),+ }, offset))
            }
        }
    };
}

impl_rwa_decode_from_slice!(RegisterRwa { rwa: NewRwa });

impl_rwa_decode_from_slice!(TransferRwa {
    source: AccountId,
    rwa: RwaId,
    quantity: Numeric,
    destination: AccountId,
});

impl_rwa_decode_from_slice!(MergeRwas {
    parents: Vec<RwaParentRef>,
    primary_reference: String,
    status: Option<Name>,
    metadata: Metadata,
});

impl_rwa_decode_from_slice!(RedeemRwa {
    rwa: RwaId,
    quantity: Numeric,
});

impl_rwa_decode_from_slice!(FreezeRwa { rwa: RwaId });

impl_rwa_decode_from_slice!(UnfreezeRwa { rwa: RwaId });

impl_rwa_decode_from_slice!(HoldRwa {
    rwa: RwaId,
    quantity: Numeric,
});

impl_rwa_decode_from_slice!(ReleaseRwa {
    rwa: RwaId,
    quantity: Numeric,
});

impl_rwa_decode_from_slice!(ForceTransferRwa {
    rwa: RwaId,
    quantity: Numeric,
    destination: AccountId,
});

impl_rwa_decode_from_slice!(SetRwaControls {
    rwa: RwaId,
    controls: RwaControlPolicy,
});

impl<'a> norito::core::DecodeFromSlice<'a> for RwaInstructionBox {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = rwa_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let tag_bytes = bytes.get(..4).ok_or(norito::core::Error::LengthMismatch)?;
        let tag = u32::from_le_bytes(
            tag_bytes
                .try_into()
                .map_err(|_| norito::core::Error::LengthMismatch)?,
        );
        let mut offset = 4usize;
        let value = match tag {
            0 => Self::Register(super::decode_aos_slice_field::<RegisterRwa>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            1 => Self::Transfer(super::decode_aos_slice_field::<TransferRwa>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            2 => Self::Merge(super::decode_aos_slice_field::<MergeRwas>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            3 => Self::Redeem(super::decode_aos_slice_field::<RedeemRwa>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            4 => Self::Freeze(super::decode_aos_slice_field::<FreezeRwa>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            5 => Self::Unfreeze(super::decode_aos_slice_field::<UnfreezeRwa>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            6 => Self::Hold(super::decode_aos_slice_field::<HoldRwa>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            7 => Self::Release(super::decode_aos_slice_field::<ReleaseRwa>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            8 => Self::ForceTransfer(super::decode_aos_slice_field::<ForceTransferRwa>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            9 => Self::SetControls(super::decode_aos_slice_field::<SetRwaControls>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            10 => Self::SetKeyValue(super::decode_aos_slice_field::<SetKeyValue<Rwa>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            11 => Self::RemoveKeyValue(super::decode_aos_slice_field::<RemoveKeyValue<Rwa>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            _ => {
                return Err(norito::core::Error::Message(format!(
                    "invalid RwaInstructionBox tag {tag}"
                )));
            }
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((value, offset))
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_primitives::{json::Json, numeric::NumericSpec};
    use norito::core::DecodeFromSlice;

    use super::*;

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked RWA fixture account keypair");
        AccountId::new(keypair.public_key().clone())
    }

    fn domain() -> DomainId {
        DomainId::try_new("wonderland", "universal").expect("domain")
    }

    fn rwa_id(seed: &'static str) -> RwaId {
        RwaId::generated(domain(), Hash::new(seed))
    }

    fn controls() -> RwaControlPolicy {
        RwaControlPolicy {
            controller_accounts: vec![account(0xCA)],
            controller_roles: Vec::new(),
            freeze_enabled: true,
            hold_enabled: true,
            force_transfer_enabled: true,
            redeem_enabled: true,
        }
    }

    fn parent_ref() -> RwaParentRef {
        RwaParentRef::new(rwa_id("rwa-parent"), Numeric::from(25_u64))
    }

    fn new_rwa() -> NewRwa {
        NewRwa::new(
            domain(),
            Numeric::from(100_u64),
            NumericSpec::fractional(2),
            "warehouse-lot-7".to_owned(),
            Some("active".parse::<Name>().expect("status")),
            Metadata::default(),
            vec![parent_ref()],
            controls(),
        )
    }

    fn key() -> Name {
        "quality".parse().expect("metadata key")
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
        wire_id: &str,
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
    #[allow(clippy::too_many_lines)]
    fn rwa_decode_from_slice_roundtrips() {
        let source = account(0xA1);
        let destination = account(0xB2);
        let rwa = rwa_id("rwa-slice");

        assert_slice_roundtrip(RegisterRwa { rwa: new_rwa() });
        assert_slice_roundtrip(TransferRwa {
            source: source.clone(),
            rwa: rwa.clone(),
            quantity: Numeric::from(5_u64),
            destination: destination.clone(),
        });
        assert_slice_roundtrip(MergeRwas {
            parents: vec![parent_ref()],
            primary_reference: "merged-lot".to_owned(),
            status: Some("merged".parse::<Name>().expect("status")),
            metadata: Metadata::default(),
        });
        assert_slice_roundtrip(RedeemRwa {
            rwa: rwa.clone(),
            quantity: Numeric::from(2_u64),
        });
        assert_slice_roundtrip(FreezeRwa { rwa: rwa.clone() });
        assert_slice_roundtrip(UnfreezeRwa { rwa: rwa.clone() });
        assert_slice_roundtrip(HoldRwa {
            rwa: rwa.clone(),
            quantity: Numeric::from(3_u64),
        });
        assert_slice_roundtrip(ReleaseRwa {
            rwa: rwa.clone(),
            quantity: Numeric::from(1_u64),
        });
        assert_slice_roundtrip(ForceTransferRwa {
            rwa: rwa.clone(),
            quantity: Numeric::from(4_u64),
            destination,
        });
        assert_slice_roundtrip(SetRwaControls {
            rwa: rwa.clone(),
            controls: controls(),
        });
        assert_slice_roundtrip(RwaInstructionBox::SetKeyValue(SetKeyValue::rwa(
            rwa.clone(),
            key(),
            Json::new(1_u32),
        )));
        assert_slice_roundtrip(RwaInstructionBox::RemoveKeyValue(RemoveKeyValue::rwa(
            rwa,
            key(),
        )));
    }

    #[test]
    fn rwa_instruction_box_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(RwaInstructionBox::Register(RegisterRwa { rwa: new_rwa() }));
    }

    #[test]
    fn rwa_instruction_box_registry_decodes_type_name_and_stable_id() {
        let registry = crate::isi::registry::default();
        assert_registry_decodes(
            &registry,
            std::any::type_name::<RwaInstructionBox>(),
            RwaInstructionBox::Register(RegisterRwa { rwa: new_rwa() }),
        );
        assert_registry_decodes(
            &registry,
            RwaInstructionBox::WIRE_ID,
            RwaInstructionBox::Transfer(TransferRwa {
                source: account(0xA1),
                rwa: rwa_id("rwa-registry"),
                quantity: Numeric::from(5_u64),
                destination: account(0xB2),
            }),
        );
    }
}
