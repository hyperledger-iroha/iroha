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
