//! Asset-related instruction type aliases.

use iroha_primitives::numeric::Numeric;

use super::{definition::AssetDefinition, id::AssetDefinitionId, value::Asset};
use crate::{
    account::Account,
    isi::{Burn, Mint, Register, Transfer, Unregister},
};

/// Register a new [`AssetDefinition`].
pub type RegisterAssetDefinition = Register<AssetDefinition>;

/// Unregister an [`AssetDefinition`].
pub type UnregisterAssetDefinition = Unregister<AssetDefinition>;

/// Transfer an [`AssetDefinition`] to another [`Account`].
pub type TransferAssetDefinition = Transfer<Account, AssetDefinitionId, Account>;

/// Transfer a quantity of [`Asset`] to another [`Account`].
pub type TransferAsset = Transfer<Asset, Numeric, Account>;

/// Mint a quantity of [`Asset`].
pub type MintAsset = Mint<Numeric, Asset>;

/// Burn a quantity of [`Asset`].
pub type BurnAsset = Burn<Numeric, Asset>;
