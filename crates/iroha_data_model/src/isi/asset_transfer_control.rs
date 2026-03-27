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
