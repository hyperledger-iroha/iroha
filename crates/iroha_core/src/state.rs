//! This module provides the [`State`] — an in-memory representation of the current blockchain state.
use std::{
    collections::BTreeSet, marker::PhantomData, num::NonZeroUsize, sync::Arc, time::Duration,
};

use eyre::Result;
use iroha_crypto::HashOf;
use iroha_data_model::{
    account::AccountId,
    block::{BlockHeader, SignedBlock},
    events::{
        pipeline::BlockEvent,
        time::TimeEvent,
        trigger_completed::{TriggerCompletedEvent, TriggerCompletedOutcome},
        EventBox,
    },
    executor::ExecutorDataModel,
    isi::error::{InstructionExecutionError as Error, MathError},
    parameter::Parameters,
    permission::Permissions,
    prelude::*,
    query::error::{FindError, QueryExecutionFail},
    role::RoleId,
};
use iroha_logger::prelude::*;
use iroha_primitives::numeric::Numeric;
use mv::{
    cell::{Block as CellBlock, Cell, Transaction as CellTransaction, View as CellView},
    storage::{
        Block as StorageBlock, RangeIter, Storage, StorageReadOnly,
        Transaction as StorageTransaction, View as StorageView,
    },
};
use nonzero_ext::nonzero;
use range_bounds::*;
use serde::{
    de::{DeserializeSeed, MapAccess, Visitor},
    Deserializer, Serialize,
};

#[cfg(feature = "telemetry")]
use crate::telemetry::StateTelemetry;
use crate::{
    block::CommittedBlock,
    executor::Executor,
    kura::Kura,
    query::store::LiveQueryStoreHandle,
    role::RoleIdWithOwner,
    smartcontracts::{
        triggers::{
            set::{
                ExecutableRef, Set as TriggerSet, SetBlock as TriggerSetBlock,
                SetReadOnly as TriggerSetReadOnly, SetTransaction as TriggerSetTransaction,
                SetView as TriggerSetView,
            },
            specialized::{LoadedAction, LoadedActionTrait},
        },
        wasm, Execute,
    },
    state::storage_transactions::{
        TransactionsBlock, TransactionsReadOnly, TransactionsStorage, TransactionsView,
    },
    Peers,
};

pub(crate) mod storage_transactions;

/// The global entity consisting of `domains`, `triggers` and etc.
/// For example registration of domain, will have this as an ISI target.
#[derive(Default, Serialize)]
pub struct World {
    /// Iroha on-chain parameters.
    pub(crate) parameters: Cell<Parameters>,
    /// Identifications of discovered peers.
    pub(crate) peers: Cell<Peers>,
    /// Registered domains.
    pub(crate) domains: Storage<DomainId, Domain>,
    /// Registered accounts.
    pub(crate) accounts: Storage<AccountId, Account>,
    /// Registered asset definitions.
    pub(crate) asset_definitions: Storage<AssetDefinitionId, AssetDefinition>,
    /// Registered assets.
    pub(crate) assets: Storage<AssetId, Asset>,
    /// Non fungible assets.
    pub(crate) nfts: Storage<NftId, Nft>,
    /// Roles. [`Role`] pairs.
    pub(crate) roles: Storage<RoleId, Role>,
    /// Permission tokens of an account.
    pub(crate) account_permissions: Storage<AccountId, Permissions>,
    /// Roles of an account.
    pub(crate) account_roles: Storage<RoleIdWithOwner, ()>,
    /// Triggers
    pub(crate) triggers: TriggerSet,
    /// Runtime Executor
    pub(crate) executor: Cell<Executor>,
    /// Executor-defined data model
    pub(crate) executor_data_model: Cell<ExecutorDataModel>,
    /// Required here for formal correctness, even though it is only used below block level.
    external_event_buf: Cell<Vec<EventBox>>,
}

/// Struct for block's aggregated changes
pub struct WorldBlock<'world> {
    /// Iroha on-chain parameters.
    pub parameters: CellBlock<'world, Parameters>,
    /// Identifications of discovered peers.
    pub(crate) peers: CellBlock<'world, Peers>,
    /// Registered domains.
    pub(crate) domains: StorageBlock<'world, DomainId, Domain>,
    /// Registered accounts.
    pub(crate) accounts: StorageBlock<'world, AccountId, Account>,
    /// Registered asset definitions.
    pub(crate) asset_definitions: StorageBlock<'world, AssetDefinitionId, AssetDefinition>,
    /// Registered assets.
    pub(crate) assets: StorageBlock<'world, AssetId, Asset>,
    /// Registered NFTs.
    pub(crate) nfts: StorageBlock<'world, NftId, Nft>,
    /// Roles. [`Role`] pairs.
    pub(crate) roles: StorageBlock<'world, RoleId, Role>,
    /// Permission tokens of an account.
    pub(crate) account_permissions: StorageBlock<'world, AccountId, Permissions>,
    /// Roles of an account.
    pub(crate) account_roles: StorageBlock<'world, RoleIdWithOwner, ()>,
    /// Triggers
    pub(crate) triggers: TriggerSetBlock<'world>,
    /// Runtime Executor
    pub(crate) executor: CellBlock<'world, Executor>,
    /// Executor-defined data model
    pub(crate) executor_data_model: CellBlock<'world, ExecutorDataModel>,
    /// Buffer of events pending publication to external subscribers.
    external_event_buf: CellBlock<'world, Vec<EventBox>>,
}

/// Struct for single transaction's aggregated changes
pub struct WorldTransaction<'block, 'world> {
    /// Iroha on-chain parameters.
    pub(crate) parameters: CellTransaction<'block, 'world, Parameters>,
    /// Identifications of discovered peers.
    pub(crate) peers: CellTransaction<'block, 'world, Peers>,
    /// Registered domains.
    pub(crate) domains: StorageTransaction<'block, 'world, DomainId, Domain>,
    /// Registered accounts.
    pub(crate) accounts: StorageTransaction<'block, 'world, AccountId, Account>,
    /// Registered asset definitions.
    pub(crate) asset_definitions:
        StorageTransaction<'block, 'world, AssetDefinitionId, AssetDefinition>,
    /// Registered assets.
    pub(crate) assets: StorageTransaction<'block, 'world, AssetId, Asset>,
    /// Registered NFTs.
    pub(crate) nfts: StorageTransaction<'block, 'world, NftId, Nft>,
    /// Roles. [`Role`] pairs.
    pub(crate) roles: StorageTransaction<'block, 'world, RoleId, Role>,
    /// Permission tokens of an account.
    pub(crate) account_permissions: StorageTransaction<'block, 'world, AccountId, Permissions>,
    /// Roles of an account.
    pub(crate) account_roles: StorageTransaction<'block, 'world, RoleIdWithOwner, ()>,
    /// Triggers
    pub(crate) triggers: TriggerSetTransaction<'block, 'world>,
    /// Runtime Executor
    pub(crate) executor: CellTransaction<'block, 'world, Executor>,
    /// Executor-defined data model
    pub(crate) executor_data_model: CellTransaction<'block, 'world, ExecutorDataModel>,
    /// Buffer of events pending publication to external subscribers.
    external_event_buf: CellTransaction<'block, 'world, Vec<EventBox>>,
    /// Data events buffered during a single execution step
    /// -- either the initial step (transaction or time trigger) or a subsequent step (data trigger).
    internal_event_buf: Vec<DataEvent>,
}

/// Consistent point in time view of the [`World`]
pub struct WorldView<'world> {
    /// Iroha on-chain parameters.
    pub(crate) parameters: CellView<'world, Parameters>,
    /// Identifications of discovered peers.
    pub(crate) peers: CellView<'world, Peers>,
    /// Registered domains.
    pub(crate) domains: StorageView<'world, DomainId, Domain>,
    /// Registered accounts.
    pub(crate) accounts: StorageView<'world, AccountId, Account>,
    /// Registered asset definitions.
    pub(crate) asset_definitions: StorageView<'world, AssetDefinitionId, AssetDefinition>,
    /// Registered assets.
    pub(crate) assets: StorageView<'world, AssetId, Asset>,
    /// Registered NFTs.
    pub(crate) nfts: StorageView<'world, NftId, Nft>,
    /// Roles. [`Role`] pairs.
    pub(crate) roles: StorageView<'world, RoleId, Role>,
    /// Permission tokens of an account.
    pub(crate) account_permissions: StorageView<'world, AccountId, Permissions>,
    /// Roles of an account.
    pub(crate) account_roles: StorageView<'world, RoleIdWithOwner, ()>,
    /// Triggers
    pub(crate) triggers: TriggerSetView<'world>,
    /// Runtime Executor
    pub(crate) executor: CellView<'world, Executor>,
    /// Executor-defined data model
    pub(crate) executor_data_model: CellView<'world, ExecutorDataModel>,
}

/// Current state of the blockchain
#[derive(Serialize)]
pub struct State {
    /// The world. Contains `domains`, `triggers`, `roles` and other data representing the current state of the blockchain.
    pub world: World,
    /// Blockchain.
    // TODO: Cell is redundant here since block_hashes is very easy to rollback by just popping the last element
    pub block_hashes: Cell<Vec<HashOf<BlockHeader>>>,
    /// Hashes of transactions mapped onto block height where they stored
    pub transactions: TransactionsStorage,
    /// Topology used to commit latest block
    pub commit_topology: Cell<Vec<PeerId>>,
    /// Topology used to commit previous block
    pub prev_commit_topology: Cell<Vec<PeerId>>,
    /// Engine for WASM [`Runtime`](wasm::Runtime) to execute triggers.
    #[serde(skip)]
    pub engine: wasmtime::Engine,

    /// Reference to Kura subsystem.
    #[serde(skip)]
    kura: Arc<Kura>,
    /// Handle to the [`LiveQueryStore`](crate::query::store::LiveQueryStore).
    #[serde(skip)]
    pub query_handle: LiveQueryStoreHandle,
    /// State telemetry
    // TODO: this should be done through events
    #[cfg(feature = "telemetry")]
    #[serde(skip)]
    pub telemetry: StateTelemetry,
    /// Lock to prevent getting inconsistent view of the state
    #[serde(skip)]
    view_lock: parking_lot::RwLock<()>,
}

/// Struct for block's aggregated changes
pub struct StateBlock<'state> {
    /// The world. Contains `domains`, `triggers`, `roles` and other data representing the current state of the blockchain.
    pub world: WorldBlock<'state>,
    /// Blockchain.
    pub block_hashes: CellBlock<'state, Vec<HashOf<BlockHeader>>>,
    /// Hashes of transactions mapped onto block height where they stored
    pub transactions: TransactionsBlock<'state>,
    /// Topology used to commit latest block
    pub commit_topology: CellBlock<'state, Vec<PeerId>>,
    /// Topology used to commit previous block
    pub prev_commit_topology: CellBlock<'state, Vec<PeerId>>,
    /// Engine for WASM [`Runtime`](wasm::Runtime) to execute triggers.
    pub engine: &'state wasmtime::Engine,

    /// Reference to Kura subsystem.
    kura: &'state Kura,
    /// Handle to the [`LiveQueryStore`](crate::query::store::LiveQueryStore).
    pub query_handle: &'state LiveQueryStoreHandle,
    /// State telemetry
    #[cfg(feature = "telemetry")]
    pub telemetry: &'state StateTelemetry,
    /// Lock to prevent getting inconsistent view of the state
    view_lock: &'state parking_lot::RwLock<()>,

    pub(crate) curr_block: BlockHeader,
}

/// Struct for single transaction's aggregated changes
pub struct StateTransaction<'block, 'state> {
    /// The world. Contains `domains`, `triggers`, `roles` and other data representing the current state of the blockchain.
    pub world: WorldTransaction<'block, 'state>,
    /// Blockchain.
    pub block_hashes: CellTransaction<'block, 'state, Vec<HashOf<BlockHeader>>>,
    /// Topology used to commit latest block
    pub commit_topology: CellTransaction<'block, 'state, Vec<PeerId>>,
    /// Topology used to commit previous block
    pub prev_commit_topology: CellTransaction<'block, 'state, Vec<PeerId>>,
    /// Engine for WASM [`Runtime`](wasm::Runtime) to execute triggers.
    pub engine: &'state wasmtime::Engine,

    /// Reference to Kura subsystem.
    kura: &'state Kura,
    /// Handle to the [`LiveQueryStore`](crate::query::store::LiveQueryStore).
    pub query_handle: &'state LiveQueryStoreHandle,
    /// State telemetry
    #[cfg(feature = "telemetry")]
    pub telemetry: &'state StateTelemetry,

    pub(crate) curr_block: BlockHeader,
}

/// Consistent point in time view of the [`State`]
pub struct StateView<'state> {
    /// The world. Contains `domains`, `triggers`, `roles` and other data representing the current state of the blockchain.
    pub world: WorldView<'state>,
    /// Blockchain.
    pub block_hashes: CellView<'state, Vec<HashOf<BlockHeader>>>,
    /// Hashes of transactions mapped onto block height where they stored
    pub transactions: TransactionsView<'state>,
    /// Topology used to commit latest block
    pub commit_topology: CellView<'state, Vec<PeerId>>,
    /// Topology used to commit previous block
    pub prev_commit_topology: CellView<'state, Vec<PeerId>>,
    /// Engine for WASM [`Runtime`](wasm::Runtime) to execute triggers.
    pub engine: &'state wasmtime::Engine,

    /// Reference to Kura subsystem.
    kura: &'state Kura,
    /// Handle to the [`LiveQueryStore`](crate::query::store::LiveQueryStore).
    pub query_handle: &'state LiveQueryStoreHandle,
    /// State telemetry
    #[cfg(feature = "telemetry")]
    pub telemetry: &'state StateTelemetry,
}

impl World {
    /// Creates an empty `World`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a [`World`] with these [`Domain`]s and [`Peer`]s.
    pub fn with<D, A, Ad>(domains: D, accounts: A, asset_definitions: Ad) -> Self
    where
        D: IntoIterator<Item = Domain>,
        A: IntoIterator<Item = Account>,
        Ad: IntoIterator<Item = AssetDefinition>,
    {
        Self::with_assets(domains, accounts, asset_definitions, [])
    }

    /// Creates a [`World`] with these [`Domain`]s and [`Peer`]s.
    pub fn with_assets<D, A, Ad, As>(
        domains: D,
        accounts: A,
        asset_definitions: Ad,
        assets: As,
    ) -> Self
    where
        D: IntoIterator<Item = Domain>,
        A: IntoIterator<Item = Account>,
        Ad: IntoIterator<Item = AssetDefinition>,
        As: IntoIterator<Item = Asset>,
    {
        let domains = domains
            .into_iter()
            .map(|domain| (domain.id().clone(), domain))
            .collect();
        let accounts = accounts
            .into_iter()
            .map(|account| (account.id().clone(), account))
            .collect();
        let asset_definitions = asset_definitions
            .into_iter()
            .map(|ad| (ad.id().clone(), ad))
            .collect();
        let assets = assets.into_iter().map(|ad| (ad.id().clone(), ad)).collect();
        Self {
            domains,
            accounts,
            asset_definitions,
            assets,
            ..Self::new()
        }
    }

    /// Create struct to apply block's changes
    pub fn block(&self) -> WorldBlock {
        WorldBlock {
            parameters: self.parameters.block(),
            peers: self.peers.block(),
            domains: self.domains.block(),
            accounts: self.accounts.block(),
            asset_definitions: self.asset_definitions.block(),
            assets: self.assets.block(),
            nfts: self.nfts.block(),
            roles: self.roles.block(),
            account_permissions: self.account_permissions.block(),
            account_roles: self.account_roles.block(),
            triggers: self.triggers.block(),
            executor: self.executor.block(),
            executor_data_model: self.executor_data_model.block(),
            external_event_buf: self.external_event_buf.block(),
        }
    }

    /// Create struct to apply block's changes while reverting changes made in the latest block
    pub fn block_and_revert(&self) -> WorldBlock {
        WorldBlock {
            parameters: self.parameters.block_and_revert(),
            peers: self.peers.block_and_revert(),
            domains: self.domains.block_and_revert(),
            accounts: self.accounts.block_and_revert(),
            asset_definitions: self.asset_definitions.block_and_revert(),
            assets: self.assets.block_and_revert(),
            nfts: self.nfts.block_and_revert(),
            roles: self.roles.block_and_revert(),
            account_permissions: self.account_permissions.block_and_revert(),
            account_roles: self.account_roles.block_and_revert(),
            triggers: self.triggers.block_and_revert(),
            executor: self.executor.block_and_revert(),
            executor_data_model: self.executor_data_model.block_and_revert(),
            external_event_buf: self.external_event_buf.block_and_revert(),
        }
    }

    /// Create point in time view of the [`Self`]
    pub fn view(&self) -> WorldView {
        WorldView {
            parameters: self.parameters.view(),
            peers: self.peers.view(),
            domains: self.domains.view(),
            accounts: self.accounts.view(),
            asset_definitions: self.asset_definitions.view(),
            assets: self.assets.view(),
            nfts: self.nfts.view(),
            roles: self.roles.view(),
            account_permissions: self.account_permissions.view(),
            account_roles: self.account_roles.view(),
            triggers: self.triggers.view(),
            executor: self.executor.view(),
            executor_data_model: self.executor_data_model.view(),
        }
    }
}

/// Trait to perform read-only operations on [`WorldBlock`], [`WorldTransaction`] and [`WorldView`]
#[allow(missing_docs)]
pub trait WorldReadOnly {
    fn parameters(&self) -> &Parameters;
    fn peers(&self) -> &Peers;
    fn domains(&self) -> &impl StorageReadOnly<DomainId, Domain>;
    fn accounts(&self) -> &impl StorageReadOnly<AccountId, Account>;
    fn asset_definitions(&self) -> &impl StorageReadOnly<AssetDefinitionId, AssetDefinition>;
    fn assets(&self) -> &impl StorageReadOnly<AssetId, Asset>;
    fn nfts(&self) -> &impl StorageReadOnly<NftId, Nft>;
    fn roles(&self) -> &impl StorageReadOnly<RoleId, Role>;
    fn account_permissions(&self) -> &impl StorageReadOnly<AccountId, Permissions>;
    fn account_roles(&self) -> &impl StorageReadOnly<RoleIdWithOwner, ()>;
    fn triggers(&self) -> &impl TriggerSetReadOnly;
    fn executor(&self) -> &Executor;
    fn executor_data_model(&self) -> &ExecutorDataModel;

    // Domain-related methods

    /// Get `Domain` without an ability to modify it.
    ///
    /// # Errors
    /// Fails if there is no domain
    fn domain(&self, id: &DomainId) -> Result<&Domain, FindError> {
        let domain = self
            .domains()
            .get(id)
            .ok_or_else(|| FindError::Domain(id.clone()))?;
        Ok(domain)
    }

    /// Get `Domain` and pass it to closure.
    ///
    /// # Errors
    /// Fails if there is no domain
    fn map_domain<'slf, T>(
        &'slf self,
        id: &DomainId,
        f: impl FnOnce(&'slf Domain) -> T,
    ) -> Result<T, FindError> {
        let domain = self.domain(id)?;
        let value = f(domain);
        Ok(value)
    }

    /// Returns reference for domains map
    #[inline]
    fn domains_iter(&self) -> impl Iterator<Item = &Domain> {
        self.domains().iter().map(|(_, domain)| domain)
    }

    /// Iterate accounts in domain
    #[allow(clippy::type_complexity)]
    fn accounts_in_domain_iter<'slf>(
        &'slf self,
        id: &DomainId,
    ) -> core::iter::Map<
        RangeIter<'slf, AccountId, Account>,
        fn((&'slf AccountId, &'slf Account)) -> &'slf Account,
    > {
        self.accounts()
            .range::<dyn AsAccountIdDomainCompare>(AccountByDomainBounds::new(id))
            .map(|(_, account)| account)
    }

    /// Returns reference for accounts map
    #[inline]
    fn accounts_iter(&self) -> impl Iterator<Item = &Account> {
        self.accounts().iter().map(|(_, account)| account)
    }

    /// Iterate asset definitions in domain
    #[allow(clippy::type_complexity)]
    fn asset_definitions_in_domain_iter<'slf>(
        &'slf self,
        id: &DomainId,
    ) -> core::iter::Map<
        RangeIter<'slf, AssetDefinitionId, AssetDefinition>,
        fn((&'slf AssetDefinitionId, &'slf AssetDefinition)) -> &'slf AssetDefinition,
    > {
        self.asset_definitions()
            .range::<dyn AsAssetDefinitionIdDomainCompare>(AssetDefinitionByDomainBounds::new(id))
            .map(|(_, ad)| ad)
    }

    /// Returns reference for asset definitions map
    #[inline]
    fn asset_definitions_iter(&self) -> impl Iterator<Item = &AssetDefinition> {
        self.asset_definitions().iter().map(|(_, ad)| ad)
    }

    /// Iterate assets in account
    #[allow(clippy::type_complexity)]
    fn assets_in_account_iter<'slf>(
        &'slf self,
        id: &AccountId,
    ) -> core::iter::Map<
        RangeIter<'slf, AssetId, Asset>,
        fn((&'slf AssetId, &'slf Asset)) -> &'slf Asset,
    > {
        self.assets()
            .range::<dyn AsAssetIdAccountCompare>(AssetByAccountBounds::new(id))
            .map(|(_, a)| a)
    }

    /// Returns reference for asset definitions map
    #[inline]
    fn assets_iter(&self) -> impl Iterator<Item = &Asset> {
        self.assets().iter().map(|(_, a)| a)
    }

    // Account-related methods

    /// Get `Account` and return reference to it.
    ///
    /// # Errors
    /// Fails if there is no domain or account
    fn account(&self, id: &AccountId) -> Result<&Account, FindError> {
        self.accounts()
            .get(id)
            .ok_or_else(|| FindError::Account(id.clone()))
    }

    /// Get `Account` and pass it to closure.
    ///
    /// # Errors
    /// Fails if there is no domain or account
    fn map_account<'slf, T>(
        &'slf self,
        id: &AccountId,
        f: impl FnOnce(&'slf Account) -> T,
    ) -> Result<T, QueryExecutionFail> {
        let account = self
            .accounts()
            .get(id)
            .ok_or(FindError::Account(id.clone()))?;
        Ok(f(account))
    }

    /// Get [`Account`]'s [`RoleId`]s
    // NOTE: have to use concreate type because don't want to capture lifetme of `id`
    #[allow(clippy::type_complexity)]
    fn account_roles_iter<'slf>(
        &'slf self,
        id: &AccountId,
    ) -> core::iter::Map<
        RangeIter<'slf, RoleIdWithOwner, ()>,
        fn((&'slf RoleIdWithOwner, &'slf ())) -> &'slf RoleId,
    > {
        self.account_roles()
            .range(RoleIdByAccountBounds::new(id))
            .map(|(role, ())| &role.id)
    }

    /// Return a set of all permission tokens granted to this account.
    ///
    /// # Errors
    ///
    /// - if `account_id` is not found in `self`
    fn account_permissions_iter<'slf>(
        &'slf self,
        account_id: &AccountId,
    ) -> Result<std::collections::btree_set::Iter<'slf, Permission>, FindError> {
        self.account(account_id)?;

        Ok(self.account_inherent_permissions(account_id))
    }

    /// Return a set of permission tokens granted to this account not as part of any role.
    ///
    /// # Errors
    ///
    /// - `account_id` is not found in `self.world`.
    fn account_inherent_permissions<'slf>(
        &'slf self,
        account_id: &AccountId,
    ) -> std::collections::btree_set::Iter<'slf, Permission> {
        self.account_permissions()
            .get(account_id)
            .map_or_else(Default::default, std::collections::BTreeSet::iter)
    }

    /// Return `true` if [`Account`] contains a permission token not associated with any role.
    #[inline]
    fn account_contains_inherent_permission(
        &self,
        account: &AccountId,
        token: &Permission,
    ) -> bool {
        self.account_permissions()
            .get(account)
            .is_some_and(|permissions| permissions.contains(token))
    }

    // Asset-related methods

    /// Get `Asset` by its id
    ///
    /// # Errors
    /// - No such [`Asset`]
    /// - The [`Account`] with which the [`Asset`] is associated doesn't exist.
    /// - The [`Domain`] with which the [`Account`] is associated doesn't exist.
    fn asset(&self, id: &AssetId) -> Result<Asset, QueryExecutionFail> {
        self.map_account(&id.account, |_| ())?;

        self.assets()
            .get(id)
            .ok_or_else(|| QueryExecutionFail::from(FindError::Asset(id.clone().into())))
            .cloned()
    }

    // AssetDefinition-related methods

    /// Get `AssetDefinition` immutable view.
    ///
    /// # Errors
    /// - Asset definition entry not found
    fn asset_definition(&self, asset_id: &AssetDefinitionId) -> Result<AssetDefinition, FindError> {
        self.asset_definitions()
            .get(asset_id)
            .ok_or_else(|| FindError::AssetDefinition(asset_id.clone()))
            .cloned()
    }

    /// Get total amount of [`Asset`].
    ///
    /// # Errors
    /// - Asset definition not found
    fn asset_total_amount(&self, definition_id: &AssetDefinitionId) -> Result<Numeric, FindError> {
        Ok(self.asset_definition(definition_id)?.total_quantity)
    }

    // NFT-related methods

    /// Get `Nft` immutable view.
    ///
    /// # Errors
    /// - NFT entry not found
    fn nft(&self, nft_id: &NftId) -> Result<Nft, FindError> {
        self.nfts()
            .get(nft_id)
            .ok_or_else(|| FindError::Nft(nft_id.clone()))
            .cloned()
    }

    /// Returns reference for NFTs map
    #[inline]
    fn nfts_iter(&self) -> impl Iterator<Item = &Nft> {
        self.nfts().iter().map(|(_, nft)| nft)
    }

    /// Iterate NFTs in domain
    #[allow(clippy::type_complexity)]
    fn nfts_in_domain_iter<'slf>(
        &'slf self,
        id: &DomainId,
    ) -> core::iter::Map<RangeIter<'slf, NftId, Nft>, fn((&'slf NftId, &'slf Nft)) -> &'slf Nft>
    {
        self.nfts()
            .range::<dyn AsNftIdDomainCompare>(NftByDomainBounds::new(id))
            .map(|(_, ad)| ad)
    }

    // Role-related methods

    /// Get `Role` and return reference to it.
    ///
    /// # Errors
    /// Fails if there is no role
    fn role(&self, id: &RoleId) -> Result<&Role, FindError> {
        self.roles()
            .get(id)
            .ok_or_else(|| FindError::Role(id.clone()))
    }
}

macro_rules! impl_world_ro {
    ($($ident:ty),*) => {$(
        impl WorldReadOnly for $ident {
            fn parameters(&self) -> &Parameters {
                &self.parameters
            }
            fn peers(&self) -> &Peers {
                &self.peers
            }
            fn domains(&self) -> &impl StorageReadOnly<DomainId, Domain> {
                &self.domains
            }
            fn accounts(&self) -> &impl StorageReadOnly<AccountId, Account> {
                &self.accounts
            }
            fn asset_definitions(&self) -> &impl StorageReadOnly<AssetDefinitionId, AssetDefinition> {
                &self.asset_definitions
            }
            fn assets(&self) -> &impl StorageReadOnly<AssetId, Asset> {
                &self.assets
            }
            fn nfts(&self) -> &impl StorageReadOnly<NftId, Nft> {
                &self.nfts
            }
            fn roles(&self) -> &impl StorageReadOnly<RoleId, Role> {
                &self.roles
            }
            fn account_permissions(&self) -> &impl StorageReadOnly<AccountId, Permissions> {
                &self.account_permissions
            }
            fn account_roles(&self) -> &impl StorageReadOnly<RoleIdWithOwner, ()> {
                &self.account_roles
            }
            fn triggers(&self) -> &impl TriggerSetReadOnly {
                &self.triggers
            }
            fn executor(&self) -> &Executor {
                &self.executor
            }
            fn executor_data_model(&self) -> &ExecutorDataModel {
                &self.executor_data_model
            }
        }
    )*};
}

impl_world_ro! {
    WorldBlock<'_>, WorldTransaction<'_, '_>, WorldView<'_>
}

impl<'world> WorldBlock<'world> {
    /// Create struct to apply transaction's changes
    pub fn trasaction(&mut self) -> WorldTransaction<'_, 'world> {
        WorldTransaction {
            parameters: self.parameters.transaction(),
            peers: self.peers.transaction(),
            domains: self.domains.transaction(),
            accounts: self.accounts.transaction(),
            asset_definitions: self.asset_definitions.transaction(),
            assets: self.assets.transaction(),
            nfts: self.nfts.transaction(),
            roles: self.roles.transaction(),
            account_permissions: self.account_permissions.transaction(),
            account_roles: self.account_roles.transaction(),
            triggers: self.triggers.transaction(),
            executor: self.executor.transaction(),
            executor_data_model: self.executor_data_model.transaction(),
            external_event_buf: self.external_event_buf.transaction(),
            internal_event_buf: Vec::new(),
        }
    }

    /// Commit block's changes
    pub fn commit(self) {
        // NOTE: intentionally destruct self not to forget commit some fields
        let Self {
            parameters,
            peers,
            domains,
            accounts,
            asset_definitions,
            assets,
            nfts,
            roles,
            account_permissions,
            account_roles,
            triggers,
            executor,
            executor_data_model,
            // Always drop at the block level.
            external_event_buf: _,
        } = self;
        // IMPORTANT!!! Commit fields in reverse order, this way consistent results are insured
        executor_data_model.commit();
        executor.commit();
        triggers.commit();
        account_roles.commit();
        account_permissions.commit();
        roles.commit();
        nfts.commit();
        assets.commit();
        asset_definitions.commit();
        accounts.commit();
        domains.commit();
        peers.commit();
        parameters.commit();
    }
}

impl WorldTransaction<'_, '_> {
    /// Apply transaction's changes
    pub fn apply(self) {
        // NOTE: intentionally destruct self not to forget commit some fields
        let Self {
            parameters,
            peers,
            domains,
            accounts,
            asset_definitions,
            assets,
            nfts,
            roles,
            account_permissions,
            account_roles,
            triggers,
            executor,
            executor_data_model,
            external_event_buf,
            internal_event_buf: _,
        } = self;
        external_event_buf.apply();
        executor_data_model.apply();
        executor.apply();
        triggers.apply();
        account_roles.apply();
        account_permissions.apply();
        roles.apply();
        nfts.apply();
        assets.apply();
        asset_definitions.apply();
        accounts.apply();
        domains.apply();
        peers.apply();
        parameters.apply();
    }

    /// Get `Domain` with an ability to modify it.
    ///
    /// # Errors
    /// Fails if there is no domain
    pub fn domain_mut(&mut self, id: &DomainId) -> Result<&mut Domain, FindError> {
        let domain = self
            .domains
            .get_mut(id)
            .ok_or_else(|| FindError::Domain(id.clone()))?;
        Ok(domain)
    }

    /// Get mutable reference to [`Account`]
    ///
    /// # Errors
    /// Fail if domain or account not found
    pub fn account_mut(&mut self, id: &AccountId) -> Result<&mut Account, FindError> {
        self.accounts
            .get_mut(id)
            .ok_or_else(|| FindError::Account(id.clone()))
    }

    /// Add [`permission`](Permission) to the [`Account`] if the account does not have this permission yet.
    ///
    /// Return a Boolean value indicating whether or not the  [`Account`] already had this permission.
    pub fn add_account_permission(&mut self, account: &AccountId, token: Permission) -> bool {
        // `match` here instead of `map_or_else` to avoid cloning token into each closure
        match self.account_permissions.get_mut(account) {
            None => {
                self.account_permissions
                    .insert(account.clone(), BTreeSet::from([token]));
                true
            }
            Some(permissions) => {
                if permissions.contains(&token) {
                    return true;
                }
                permissions.insert(token);
                false
            }
        }
    }

    /// Remove a [`permission`](Permission) from the [`Account`] if the account has this permission.
    /// Return a Boolean value indicating whether the [`Account`] had this permission.
    pub fn remove_account_permission(&mut self, account: &AccountId, token: &Permission) -> bool {
        self.account_permissions
            .get_mut(account)
            .is_some_and(|permissions| permissions.remove(token))
    }

    /// Remove all [`Role`]s from the [`Account`]
    pub fn remove_account_roles(&mut self, account: &AccountId) {
        let roles_to_remove = self
            .account_roles_iter(account)
            .cloned()
            .map(|role| RoleIdWithOwner::new(account.clone(), role.clone()))
            .collect::<Vec<_>>();

        for role in roles_to_remove {
            self.account_roles.remove(role);
        }
    }

    /// Get mutable reference to [`Asset`]
    ///
    /// # Errors
    /// If domain, account or asset not found
    pub fn asset_mut(&mut self, id: &AssetId) -> Result<&mut Asset, FindError> {
        let _ = self.account(&id.account)?;
        self.assets
            .get_mut(id)
            .ok_or_else(|| FindError::Asset(id.clone().into()))
    }

    /// Get asset or inserts new with `default_asset_value`.
    ///
    /// # Errors
    /// - There is no account with such name.
    #[allow(clippy::missing_panics_doc)]
    pub fn asset_or_insert(
        &mut self,
        asset_id: &AssetId,
        default_asset_value: impl Into<Numeric>,
    ) -> Result<&mut Asset, Error> {
        self.domain(&asset_id.definition.domain)?;
        self.asset_definition(&asset_id.definition)?;
        self.account(&asset_id.account)?;

        if self.assets.get(asset_id).is_none() {
            let asset = Asset::new(asset_id.clone(), default_asset_value.into());

            Self::emit_events_impl(
                &mut self.external_event_buf,
                &mut self.internal_event_buf,
                Some(AssetEvent::Created(asset.clone())),
            );
            self.assets.insert(asset_id.clone(), asset);
        }
        Ok(self
            .assets
            .get_mut(asset_id)
            .expect("Just inserted, cannot fail."))
    }

    /// Get mutable reference to [`AssetDefinition`]
    ///
    /// # Errors
    /// If domain or asset definition not found
    pub fn asset_definition_mut(
        &mut self,
        id: &AssetDefinitionId,
    ) -> Result<&mut AssetDefinition, FindError> {
        self.asset_definitions
            .get_mut(id)
            .ok_or_else(|| FindError::AssetDefinition(id.clone()))
    }

    /// Increase [`Asset`] total amount by given value
    ///
    /// # Errors
    /// - [`AssetDefinition`], [`Domain`] not found
    /// - Overflow
    pub fn increase_asset_total_amount(
        &mut self,
        definition_id: &AssetDefinitionId,
        increment: Numeric,
    ) -> Result<(), Error> {
        let asset_total_amount: &mut Numeric =
            &mut self.asset_definition_mut(definition_id)?.total_quantity;

        *asset_total_amount = asset_total_amount
            .checked_add(increment)
            .ok_or(MathError::Overflow)?;
        let asset_total_amount = *asset_total_amount;

        self.emit_events({
            Some(DomainEvent::AssetDefinition(
                AssetDefinitionEvent::TotalQuantityChanged(AssetDefinitionTotalQuantityChanged {
                    asset_definition: definition_id.clone(),
                    total_amount: asset_total_amount,
                }),
            ))
        });

        Ok(())
    }

    /// Decrease [`Asset`] total amount by given value
    ///
    /// # Errors
    /// - [`AssetDefinition`], [`Domain`] not found
    /// - Not enough quantity
    pub fn decrease_asset_total_amount(
        &mut self,
        definition_id: &AssetDefinitionId,
        decrement: Numeric,
    ) -> Result<(), Error> {
        let asset_total_amount: &mut Numeric =
            &mut self.asset_definition_mut(definition_id)?.total_quantity;

        *asset_total_amount = asset_total_amount
            .checked_sub(decrement)
            .ok_or(MathError::NotEnoughQuantity)?;
        let asset_total_amount = *asset_total_amount;

        self.emit_events({
            Some(DomainEvent::AssetDefinition(
                AssetDefinitionEvent::TotalQuantityChanged(AssetDefinitionTotalQuantityChanged {
                    asset_definition: definition_id.clone(),
                    total_amount: asset_total_amount,
                }),
            ))
        });

        Ok(())
    }

    /// Get mutable reference to [`Nft`]
    ///
    /// # Errors
    /// If NFT not found
    pub fn nft_mut(&mut self, id: &NftId) -> Result<&mut Nft, FindError> {
        self.nfts
            .get_mut(id)
            .ok_or_else(|| FindError::Nft(id.clone()))
    }

    /// Set executor data model.
    pub fn set_executor_data_model(&mut self, executor_data_model: ExecutorDataModel) {
        let prev_executor_data_model =
            core::mem::replace(self.executor_data_model.get_mut(), executor_data_model);

        self.update_parameters(&prev_executor_data_model);
    }

    fn update_parameters(&mut self, prev_executor_data_model: &ExecutorDataModel) {
        let removed_parameters = prev_executor_data_model
            .parameters
            .keys()
            .filter(|param_id| !self.executor_data_model.parameters.contains_key(param_id));
        let new_parameters = self
            .executor_data_model
            .parameters
            .iter()
            .filter(|(param_id, _)| !prev_executor_data_model.parameters.contains_key(param_id));

        for param in removed_parameters {
            iroha_logger::info!("{}: parameter removed", param);
            self.parameters.custom.remove(param);
        }

        for (param_id, param) in new_parameters {
            self.parameters
                .custom
                .insert(param_id.clone(), param.clone());
            iroha_logger::info!("{}: parameter created", param);
        }
    }

    /// The function puts events produced by iterator into event buffers.
    /// Events should be produced in the order of expanding scope: from specific to general.
    /// Example: account events before domain events.
    pub fn emit_events<I: IntoIterator<Item = T>, T: Into<DataEvent>>(&mut self, world_events: I) {
        Self::emit_events_impl(
            &mut self.external_event_buf,
            &mut self.internal_event_buf,
            world_events,
        )
    }

    /// Implementation of [`Self::emit_events()`].
    ///
    /// Usable when you can't call [`Self::emit_events()`] due to mutable reference to self.
    fn emit_events_impl<I: IntoIterator<Item = T>, T: Into<DataEvent>>(
        external_event_buf: &mut CellTransaction<Vec<EventBox>>,
        internal_event_buf: &mut Vec<DataEvent>,
        world_events: I,
    ) {
        let data_events: Vec<DataEvent> = world_events.into_iter().map(Into::into).collect();
        external_event_buf.extend(data_events.iter().cloned().map(EventBox::from));
        internal_event_buf.extend(data_events);
    }
}

impl State {
    #[must_use]
    #[inline]
    fn new_inner(
        world: World,
        kura: Arc<Kura>,
        query_handle: LiveQueryStoreHandle,
        #[cfg(feature = "telemetry")] telemetry: StateTelemetry,
    ) -> Self {
        Self {
            world,
            transactions: TransactionsStorage::new(),
            commit_topology: Cell::new(Vec::new()),
            prev_commit_topology: Cell::new(Vec::new()),
            block_hashes: Cell::new(Vec::new()),
            engine: wasm::create_engine(),
            kura,
            query_handle,
            #[cfg(feature = "telemetry")]
            telemetry,
            view_lock: parking_lot::RwLock::new(()),
        }
    }

    /// Construct [`State`] with given [`World`].
    #[must_use]
    #[inline]
    #[cfg(not(test))]
    pub fn new(
        world: World,
        kura: Arc<Kura>,
        query_handle: LiveQueryStoreHandle,
        #[cfg(feature = "telemetry")] telemetry: StateTelemetry,
    ) -> Self {
        Self::new_inner(
            world,
            kura,
            query_handle,
            #[cfg(feature = "telemetry")]
            telemetry,
        )
    }

    /// _(test only)_ Create state with mock telemetry (depends on features)
    #[must_use]
    #[inline]
    #[cfg(test)]
    pub fn new(world: World, kura: Arc<Kura>, query_handle: LiveQueryStoreHandle) -> Self {
        Self::new_inner(
            world,
            kura,
            query_handle,
            #[cfg(feature = "telemetry")]
            <_>::default(),
        )
    }

    /// _(test only)_ Create state with telemetry
    #[must_use]
    #[inline]
    #[cfg(all(test, feature = "telemetry"))]
    pub fn with_telemetry(
        world: World,
        kura: Arc<Kura>,
        query_handle: LiveQueryStoreHandle,
        telemetry: StateTelemetry,
    ) -> Self {
        Self::new_inner(world, kura, query_handle, telemetry)
    }

    /// Create structure to execute a block
    pub fn block(&self, curr_block: BlockHeader) -> StateBlock<'_> {
        StateBlock {
            world: self.world.block(),
            block_hashes: self.block_hashes.block(),
            transactions: self.transactions.block(),
            commit_topology: self.commit_topology.block(),
            prev_commit_topology: self.prev_commit_topology.block(),
            engine: &self.engine,
            kura: &self.kura,
            query_handle: &self.query_handle,
            #[cfg(feature = "telemetry")]
            telemetry: &self.telemetry,
            view_lock: &self.view_lock,
            curr_block,
        }
    }

    /// Create structure to execute a block while reverting changes made in the latest block
    pub fn block_and_revert(&self, curr_block: BlockHeader) -> StateBlock<'_> {
        StateBlock {
            world: self.world.block_and_revert(),
            block_hashes: self.block_hashes.block_and_revert(),
            transactions: self.transactions.block_and_revert(),
            commit_topology: self.commit_topology.block_and_revert(),
            prev_commit_topology: self.prev_commit_topology.block_and_revert(),
            engine: &self.engine,
            kura: &self.kura,
            query_handle: &self.query_handle,
            #[cfg(feature = "telemetry")]
            telemetry: &self.telemetry,
            view_lock: &self.view_lock,
            curr_block,
        }
    }

    /// Create point in time view of [`State`]
    pub fn view(&self) -> StateView<'_> {
        let _view_lock = self.view_lock.read();
        StateView {
            world: self.world.view(),
            block_hashes: self.block_hashes.view(),
            transactions: self.transactions.view(),
            commit_topology: self.commit_topology.view(),
            prev_commit_topology: self.prev_commit_topology.view(),
            engine: &self.engine,
            kura: &self.kura,
            query_handle: &self.query_handle,
            #[cfg(feature = "telemetry")]
            telemetry: &self.telemetry,
        }
    }
}

/// Trait to perform read-only operations on [`StateBlock`], [`StateTransaction`] and [`StateView`]
#[allow(missing_docs)]
pub trait StateReadOnly {
    fn world(&self) -> &impl WorldReadOnly;
    fn block_hashes(&self) -> &[HashOf<BlockHeader>];
    fn commit_topology(&self) -> &[PeerId];
    fn prev_commit_topology(&self) -> &[PeerId];
    fn engine(&self) -> &wasmtime::Engine;
    fn kura(&self) -> &Kura;
    fn query_handle(&self) -> &LiveQueryStoreHandle;
    #[cfg(feature = "telemetry")]
    fn metrics(&self) -> &StateTelemetry;

    /// Get a reference to the block one before the latest block.
    /// Returns None if at least 2 blocks are not committed.
    ///
    /// If you only need hash of the latest block prefer using [`Self::prev_block_hash`].
    #[inline]
    fn prev_block(&self) -> Option<Arc<SignedBlock>> {
        self.height()
            .checked_sub(1)
            .and_then(NonZeroUsize::new)
            .and_then(|height| self.kura().get_block(height))
    }

    /// Get a reference to the latest block. Returns none if genesis is not committed.
    ///
    /// If you only need hash of the latest block prefer using [`Self::latest_block_hash`]
    #[inline]
    fn latest_block(&self) -> Option<Arc<SignedBlock>> {
        NonZeroUsize::new(self.height()).and_then(|height| self.kura().get_block(height))
    }

    /// Return the hash of the latest block
    fn latest_block_hash(&self) -> Option<HashOf<BlockHeader>> {
        self.block_hashes().iter().nth_back(0).copied()
    }

    /// Return the hash of the block one before the latest block.
    /// Returns None if at least 2 blocks are not committed.
    fn prev_block_hash(&self) -> Option<HashOf<BlockHeader>> {
        self.block_hashes().iter().nth_back(1).copied()
    }

    /// Load all blocks in the block chain from disc
    fn all_blocks(
        &self,
        start: NonZeroUsize,
    ) -> impl DoubleEndedIterator<Item = Arc<SignedBlock>> + '_ {
        (start.get()..=self.height()).map(|height| {
            NonZeroUsize::new(height)
                .and_then(|height| self.kura().get_block(height))
                .expect("INTERNAL BUG: Failed to load block")
        })
    }

    /// Height of blockchain
    #[inline]
    fn height(&self) -> usize {
        self.block_hashes().len()
    }

    /// Returns [`Some`] milliseconds since the genesis block was
    /// committed, or [`None`] if it wasn't.
    #[inline]
    fn genesis_timestamp(&self) -> Option<Duration> {
        if self.block_hashes().is_empty() {
            None
        } else {
            let opt = self
                .kura()
                .get_block(nonzero!(1_usize))
                .map(|genesis_block| genesis_block.header().creation_time());

            if opt.is_none() {
                error!("Failed to get genesis block from Kura.");
            }

            opt
        }
    }
}

macro_rules! impl_state_ro {
    ($($ident:ty),*) => {$(
        impl StateReadOnly for $ident {
            fn world(&self) -> &impl WorldReadOnly {
                &self.world
            }
            fn block_hashes(&self) -> &[HashOf<BlockHeader>] {
                &self.block_hashes
            }
            fn commit_topology(&self) -> &[PeerId] {
                &self.commit_topology
            }
            fn prev_commit_topology(&self) -> &[PeerId] {
                &self.prev_commit_topology
            }
            fn engine(&self) -> &wasmtime::Engine {
                &self.engine
            }
            fn kura(&self) -> &Kura {
                &self.kura
            }
            fn query_handle(&self) -> &LiveQueryStoreHandle {
                &self.query_handle
            }
            #[cfg(feature = "telemetry")]
            fn metrics(&self) -> &StateTelemetry {
                &self.telemetry
            }
        }
    )*};
}

impl_state_ro! {
    StateBlock<'_>, StateTransaction<'_, '_>, StateView<'_>
}

/// Separate trait for either [`State`] or [`StateBlock`].
///
/// `transactions` map consumes >80% RAM of iroha, so they should be optimized.
/// And it would be easier to optimize if less methods are needed to be supported.
/// (`StateTransaction` anyway doesn't need `transactions` map)
pub trait StateReadOnlyWithTransactions: StateReadOnly {
    /// Returns transactions map
    fn transactions(&self) -> &impl TransactionsReadOnly;

    /// Check if [`SignedTransaction`] is already committed
    #[inline]
    fn has_transaction(&self, hash: HashOf<SignedTransaction>) -> bool {
        self.transactions().get(&hash).is_some()
    }
}

impl StateReadOnlyWithTransactions for StateView<'_> {
    fn transactions(&self) -> &impl TransactionsReadOnly {
        &self.transactions
    }
}

impl StateReadOnlyWithTransactions for StateBlock<'_> {
    fn transactions(&self) -> &impl TransactionsReadOnly {
        &self.transactions
    }
}

impl<'state> StateBlock<'state> {
    /// Create struct to store changes during transaction or trigger execution
    pub fn transaction(&mut self) -> StateTransaction<'_, 'state> {
        StateTransaction {
            world: self.world.trasaction(),
            block_hashes: self.block_hashes.transaction(),
            commit_topology: self.commit_topology.transaction(),
            prev_commit_topology: self.prev_commit_topology.transaction(),
            engine: self.engine,
            kura: self.kura,
            query_handle: self.query_handle,
            #[cfg(feature = "telemetry")]
            telemetry: self.telemetry,
            curr_block: self.curr_block,
        }
    }

    /// Commit changes aggregated during application of block
    pub fn commit(self) {
        // NOTE: intentionally destruct self not to forget commit some fields
        let Self {
            world,
            block_hashes,
            transactions,
            commit_topology: committed_topology,
            prev_commit_topology: prev_committed_topology,
            view_lock,
            ..
        } = self;
        let _view_lock = view_lock.write();
        prev_committed_topology.commit();
        committed_topology.commit();
        transactions.commit();
        block_hashes.commit();
        world.commit();
    }

    /// Assuming all transactions in the block have been processed,
    /// apply the remaining block effects outside the world state.
    #[iroha_logger::log(skip_all, fields(block_height = block.as_ref().header().height))]
    #[must_use]
    pub fn apply_without_execution(
        &mut self,
        block: &CommittedBlock,
        topology: Vec<PeerId>,
    ) -> Vec<EventBox> {
        let block_hash = block.as_ref().hash();
        trace!(%block_hash, "Applying block");

        let block_height = block
            .as_ref()
            .header()
            .height
            .try_into()
            .expect("INTERNAL BUG: Block height exceeds usize::MAX");
        let transactions = block
            .as_ref()
            .transactions()
            .map(SignedTransaction::hash)
            .collect();
        self.transactions.insert_block(transactions, block_height);

        self.block_hashes.push(block_hash);

        *self.prev_commit_topology = core::mem::take(&mut self.commit_topology);
        *self.commit_topology = topology;

        self.world.external_event_buf.push(
            BlockEvent {
                header: block.as_ref().header(),
                status: BlockStatus::Applied,
            }
            .into(),
        );
        core::mem::take(&mut self.world.external_event_buf)
    }

    /// Execute all time triggers matching the given block.
    pub(crate) fn execute_time_triggers(&mut self, block: &SignedBlock) {
        let time_event = self.create_time_event(block);
        self.world.external_event_buf.push(time_event.into());
        let matched: Vec<_> = self.world.triggers.match_time_event(time_event).collect();

        for (trg_id, action) in &matched {
            if let Err(error) = self.execute_time_trigger(trg_id, action, &time_event) {
                // TODO(#4968): Record errors in the block alongside transaction errors.
                iroha_logger::warn!(
                    trigger=%trg_id,
                    block=%block.hash(),
                    reason=?error,
                    "Time trigger and its chained data triggers failed to execute"
                );
            }
        }
    }

    fn execute_time_trigger(
        &mut self,
        trg_id: &TriggerId,
        action: &LoadedAction<TimeEventFilter>,
        time_event: &TimeEvent,
    ) -> Result<(), TransactionRejectionReason> {
        let mut transaction = self.transaction();

        transaction
            .execute_trigger(
                trg_id,
                action.authority(),
                action.executable(),
                (*time_event).into(),
            )
            .and_then(|()| transaction.execute_data_triggers_dfs())?;
        transaction
            .world
            .triggers
            .decrease_repeats([trg_id].into_iter());
        transaction.apply();

        Ok(())
    }

    /// Create time event using previous and current blocks.
    fn create_time_event(&self, block: &SignedBlock) -> TimeEvent {
        let to = block.header().creation_time();

        let since = self.latest_block().map_or(to, |latest_block| {
            let header = latest_block.header();
            header.creation_time()
        });

        // NOTE: in case of genesis block only single point in time is matched
        let interval = TimeInterval::new(since, to - since);

        TimeEvent { interval }
    }

    /// Apply a committed block to the world state.
    ///
    /// Execution order:
    /// 1. Transactions (including invoked data triggers)
    /// 2. Time triggers (including invoked data triggers)
    ///
    /// # Panics
    ///
    /// Panics if processing approved transactions or time triggers fails.
    #[cfg(any(test, feature = "bench"))]
    #[iroha_logger::log(skip_all, fields(block_height))]
    pub fn apply(&mut self, block: &CommittedBlock, topology: Vec<PeerId>) -> Vec<EventBox> {
        self.apply_transactions(block);
        debug!(height = %self.height(), "Transactions applied");
        self.execute_time_triggers(block.as_ref());
        debug!(height = %self.height(), "Time triggers executed");
        self.apply_without_execution(block, topology)
    }

    /// Apply all non-erroneous transactions in the given committed block.
    #[cfg(any(test, feature = "bench"))]
    fn apply_transactions(&mut self, block: &CommittedBlock) {
        let block = block.as_ref();

        for (idx, tx) in block.transactions().enumerate() {
            if block.error(idx).is_none() {
                // Execute each transaction in its own transactional state
                let mut transaction = self.transaction();
                transaction.apply_executable(tx.instructions(), tx.authority().clone());
                transaction
                    .execute_data_triggers_dfs()
                    .expect("should be no errors");
                transaction.apply();
            }
        }
    }
}

impl StateTransaction<'_, '_> {
    /// Apply transaction making it's changes visible
    pub fn apply(self) {
        // NOTE: intentionally destruct self not to forget apply some fields
        let Self {
            world,
            block_hashes,
            commit_topology: committed_topology,
            prev_commit_topology: prev_committed_topology,
            ..
        } = self;
        prev_committed_topology.apply();
        committed_topology.apply();
        block_hashes.apply();
        world.apply();
    }

    /// Execute a call-trigger. This function will be deprecated in #5147.
    pub(crate) fn execute_called_trigger(
        &mut self,
        id: &TriggerId,
        event: ExecuteTriggerEvent,
    ) -> Result<(), TransactionRejectionReason> {
        let (authority, executable) = {
            let action = self
                .world
                .triggers
                .by_call_triggers()
                .get(event.trigger_id())
                .ok_or_else(|| FindError::Trigger(id.clone()))
                .map_err(Error::from)
                .map_err(ValidationFail::from)?;
            assert!(
                !action.repeats.is_depleted(),
                "orphaned trigger was not removed"
            );

            (action.authority().clone(), action.executable().clone())
        };
        self.world.external_event_buf.push(event.clone().into());
        self.execute_trigger(id, &authority, &executable, event.into())?;
        self.world.triggers.decrease_repeats([id].into_iter());

        Ok(())
    }

    /// Perform a depth-first traversal of the trigger execution path.
    pub(crate) fn execute_data_triggers_dfs(&mut self) -> Result<(), TransactionRejectionReason> {
        let mut stack: Vec<(DataEvent, TriggerId, u8)> = self
            .capture_data_events()
            .into_iter()
            // Preserve the order of the matched triggers
            .rev()
            .map(|(e, t)| (e, t, 1))
            .collect();

        while let Some((event, trg_id, depth)) = stack.pop() {
            let max_depth = self.world.parameters.smart_contract.execution_depth;
            if max_depth < depth {
                return Err(TriggerExecutionFail::MaxDepthExceeded.into());
            }
            let (authority, executable) = {
                let action = self
                    .world
                    .triggers
                    .data_triggers()
                    .get(&trg_id)
                    .expect("stack should reference existing data trigger IDs");
                assert!(
                    !action.repeats.is_depleted(),
                    "orphaned trigger was not removed"
                );

                (action.authority().clone(), action.executable().clone())
            };

            self.execute_trigger(&trg_id, &authority, &executable, event.clone().into())?;
            let depleted = self.world.triggers.decrease_repeats([&trg_id].into_iter());
            stack.retain(|(_, trg_id, _)| !depleted.contains(trg_id));

            let next_items = self
                .capture_data_events()
                .into_iter()
                .rev()
                .map(|(e, t)| (e, t, depth + 1));
            stack.extend(next_items);
        }

        Ok(())
    }

    /// Flush the internal event buffer and return pairs of __representative__ matched events and trigger IDs.
    // FIXME: Return the triggering event unions instead of the representatives (#5355 as a prerequisite)
    fn capture_data_events(&mut self) -> Vec<(DataEvent, TriggerId)> {
        let drained: Vec<DataEvent> = self.world.internal_event_buf.drain(..).collect();
        self.world
            .triggers
            .data_triggers()
            .iter()
            .filter_map(|(trg_id, action)| {
                drained.iter().find_map(|event| {
                    action
                        .filter
                        .matches(event)
                        .then(|| (event.clone(), trg_id.clone()))
                })
            })
            .collect()
    }

    fn execute_trigger(
        &mut self,
        id: &TriggerId,
        authority: &AccountId,
        executable: &ExecutableRef,
        event: EventBox,
    ) -> Result<(), TransactionRejectionReason> {
        let res = match executable {
            ExecutableRef::Instructions(instructions) => self
                .execute_instructions(instructions.iter().cloned(), authority)
                .map_err(ValidationFail::from),
            ExecutableRef::Wasm(blob_hash) => {
                let module = self
                    .world
                    .triggers
                    .get_compiled_contract(blob_hash)
                    .expect("INTERNAL BUG: contract is not present")
                    .clone();
                wasm::RuntimeBuilder::<wasm::state::Trigger>::new()
                    .with_config(self.world().parameters().smart_contract)
                    .with_engine(self.engine.clone()) // Cloning engine is cheap
                    .build()
                    .and_then(|mut wasm_runtime| {
                        wasm_runtime.execute_trigger_module(
                            self,
                            id,
                            authority.clone(),
                            &module,
                            event,
                        )
                    })
                    .map_err(ValidationFail::from)
            }
        };

        let outcome = match &res {
            Ok(()) => TriggerCompletedOutcome::Success,
            Err(error) => TriggerCompletedOutcome::Failure(error.to_string()),
        };
        let event = TriggerCompletedEvent::new(id.clone(), outcome);
        self.world.external_event_buf.push(event.into());

        res.map_err(Into::into)
    }

    fn execute_instructions(
        &mut self,
        instructions: impl IntoIterator<Item = InstructionBox>,
        authority: &AccountId,
    ) -> Result<(), Error> {
        instructions.into_iter().try_for_each(|instruction| {
            instruction.execute(authority, self)?;
            Ok(())
        })
    }

    /// Apply a non-erroneous executable in the given committed block.
    #[cfg(any(test, feature = "bench"))]
    fn apply_executable(&mut self, executable: &Executable, authority: AccountId) {
        match executable {
            Executable::Instructions(instructions) => {
                self.execute_instructions(instructions.iter().cloned(), &authority)
                    .expect("should be no errors");
            }
            Executable::Wasm(bytes) => {
                let mut wasm_runtime = wasm::RuntimeBuilder::<wasm::state::SmartContract>::new()
                    .with_config(self.world().parameters().smart_contract)
                    .with_engine(self.engine.clone()) // Cloning engine is cheap
                    .build()
                    .expect("failed to create wasm runtime");
                wasm_runtime
                    .execute(self, authority, bytes)
                    .expect("should be no errors");
            }
        }
    }
}

/// Bounds for `range` queries
mod range_bounds {
    use core::ops::{Bound, RangeBounds};

    use iroha_primitives::{cmpext::MinMaxExt, impl_as_dyn_key};

    use super::*;
    use crate::role::RoleIdWithOwner;

    /// Key for range queries over account for roles
    #[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
    pub struct RoleIdByAccount<'role> {
        account_id: &'role AccountId,
        role_id: MinMaxExt<&'role RoleId>,
    }

    /// Bounds for range quired over account for roles
    pub struct RoleIdByAccountBounds<'role> {
        start: RoleIdByAccount<'role>,
        end: RoleIdByAccount<'role>,
    }

    impl<'role> RoleIdByAccountBounds<'role> {
        /// Create range bounds for range quires of roles over account
        pub fn new(account_id: &'role AccountId) -> Self {
            Self {
                start: RoleIdByAccount {
                    account_id,
                    role_id: MinMaxExt::Min,
                },
                end: RoleIdByAccount {
                    account_id,
                    role_id: MinMaxExt::Max,
                },
            }
        }
    }

    impl<'role> RangeBounds<dyn AsRoleIdByAccount + 'role> for RoleIdByAccountBounds<'role> {
        fn start_bound(&self) -> Bound<&(dyn AsRoleIdByAccount + 'role)> {
            Bound::Excluded(&self.start)
        }

        fn end_bound(&self) -> Bound<&(dyn AsRoleIdByAccount + 'role)> {
            Bound::Excluded(&self.end)
        }
    }

    impl AsRoleIdByAccount for RoleIdWithOwner {
        fn as_key(&self) -> RoleIdByAccount<'_> {
            RoleIdByAccount {
                account_id: &self.account,
                role_id: (&self.id).into(),
            }
        }
    }

    impl_as_dyn_key! {
        target: RoleIdWithOwner,
        key: RoleIdByAccount<'_>,
        trait: AsRoleIdByAccount
    }

    /// `DomainId` wrapper for fetching accounts beloning to a domain from the global store
    #[derive(PartialEq, Eq, Ord, PartialOrd, Copy, Clone)]
    pub struct AccountIdDomainCompare<'a> {
        domain_id: &'a DomainId,
        signatory: MinMaxExt<&'a PublicKey>,
    }

    /// Bounds for range quired over accounts by domain
    pub struct AccountByDomainBounds<'a> {
        start: AccountIdDomainCompare<'a>,
        end: AccountIdDomainCompare<'a>,
    }

    impl<'a> AccountByDomainBounds<'a> {
        /// Create range bounds for range quires over accounts by domain
        pub fn new(domain_id: &'a DomainId) -> Self {
            Self {
                start: AccountIdDomainCompare {
                    domain_id,
                    signatory: MinMaxExt::Min,
                },
                end: AccountIdDomainCompare {
                    domain_id,
                    signatory: MinMaxExt::Max,
                },
            }
        }
    }

    impl<'a> RangeBounds<dyn AsAccountIdDomainCompare + 'a> for AccountByDomainBounds<'a> {
        fn start_bound(&self) -> Bound<&(dyn AsAccountIdDomainCompare + 'a)> {
            Bound::Excluded(&self.start)
        }

        fn end_bound(&self) -> Bound<&(dyn AsAccountIdDomainCompare + 'a)> {
            Bound::Excluded(&self.end)
        }
    }

    impl AsAccountIdDomainCompare for AccountId {
        fn as_key(&self) -> AccountIdDomainCompare<'_> {
            AccountIdDomainCompare {
                domain_id: &self.domain,
                signatory: (&self.signatory).into(),
            }
        }
    }

    impl_as_dyn_key! {
        target: AccountId,
        key: AccountIdDomainCompare<'_>,
        trait: AsAccountIdDomainCompare
    }

    /// `DomainId` wrapper for fetching asset definitions beloning to a domain from the global store
    #[derive(PartialEq, Eq, Ord, PartialOrd, Copy, Clone)]
    pub struct AssetDefinitionIdDomainCompare<'a> {
        domain_id: &'a DomainId,
        name: MinMaxExt<&'a Name>,
    }

    /// Bounds for range quired over asset definitions by domain
    pub struct AssetDefinitionByDomainBounds<'a> {
        start: AssetDefinitionIdDomainCompare<'a>,
        end: AssetDefinitionIdDomainCompare<'a>,
    }

    impl<'a> AssetDefinitionByDomainBounds<'a> {
        /// Create range bounds for range quires over asset definitions by domain
        pub fn new(domain_id: &'a DomainId) -> Self {
            Self {
                start: AssetDefinitionIdDomainCompare {
                    domain_id,
                    name: MinMaxExt::Min,
                },
                end: AssetDefinitionIdDomainCompare {
                    domain_id,
                    name: MinMaxExt::Max,
                },
            }
        }
    }

    impl<'a> RangeBounds<dyn AsAssetDefinitionIdDomainCompare + 'a>
        for AssetDefinitionByDomainBounds<'a>
    {
        fn start_bound(&self) -> Bound<&(dyn AsAssetDefinitionIdDomainCompare + 'a)> {
            Bound::Excluded(&self.start)
        }

        fn end_bound(&self) -> Bound<&(dyn AsAssetDefinitionIdDomainCompare + 'a)> {
            Bound::Excluded(&self.end)
        }
    }

    impl AsAssetDefinitionIdDomainCompare for AssetDefinitionId {
        fn as_key(&self) -> AssetDefinitionIdDomainCompare<'_> {
            AssetDefinitionIdDomainCompare {
                domain_id: &self.domain,
                name: (&self.name).into(),
            }
        }
    }

    impl_as_dyn_key! {
        target: AssetDefinitionId,
        key: AssetDefinitionIdDomainCompare<'_>,
        trait: AsAssetDefinitionIdDomainCompare
    }

    /// `DomainId` wrapper for fetching NFTs belonging to a domain from the global store
    #[derive(PartialEq, Eq, Ord, PartialOrd, Copy, Clone)]
    pub struct NftIdDomainCompare<'a> {
        domain_id: &'a DomainId,
        name: MinMaxExt<&'a Name>,
    }

    /// Bounds for range quired over NFTs by domain
    pub struct NftByDomainBounds<'a> {
        start: NftIdDomainCompare<'a>,
        end: NftIdDomainCompare<'a>,
    }

    impl<'a> NftByDomainBounds<'a> {
        /// Create range bounds for range quires over NFTs by domain
        pub fn new(domain_id: &'a DomainId) -> Self {
            Self {
                start: NftIdDomainCompare {
                    domain_id,
                    name: MinMaxExt::Min,
                },
                end: NftIdDomainCompare {
                    domain_id,
                    name: MinMaxExt::Max,
                },
            }
        }
    }

    impl<'a> RangeBounds<dyn AsNftIdDomainCompare + 'a> for NftByDomainBounds<'a> {
        fn start_bound(&self) -> Bound<&(dyn AsNftIdDomainCompare + 'a)> {
            Bound::Excluded(&self.start)
        }

        fn end_bound(&self) -> Bound<&(dyn AsNftIdDomainCompare + 'a)> {
            Bound::Excluded(&self.end)
        }
    }

    impl AsNftIdDomainCompare for NftId {
        fn as_key(&self) -> NftIdDomainCompare<'_> {
            NftIdDomainCompare {
                domain_id: &self.domain,
                name: (&self.name).into(),
            }
        }
    }

    impl_as_dyn_key! {
        target: NftId,
        key: NftIdDomainCompare<'_>,
        trait: AsNftIdDomainCompare
    }

    /// `AccountId` wrapper for fetching assets beloning to an account from the global store
    #[derive(PartialEq, Eq, Ord, PartialOrd, Copy, Clone)]
    pub struct AssetIdAccountCompare<'a> {
        account_id: &'a AccountId,
        definition: MinMaxExt<&'a AssetDefinitionId>,
    }

    /// Bounds for range quired over assets by account
    pub struct AssetByAccountBounds<'a> {
        start: AssetIdAccountCompare<'a>,
        end: AssetIdAccountCompare<'a>,
    }

    impl<'a> AssetByAccountBounds<'a> {
        /// Create range bounds for range quires over assets by account
        pub fn new(account_id: &'a AccountId) -> Self {
            Self {
                start: AssetIdAccountCompare {
                    account_id,
                    definition: MinMaxExt::Min,
                },
                end: AssetIdAccountCompare {
                    account_id,
                    definition: MinMaxExt::Max,
                },
            }
        }
    }

    impl<'a> RangeBounds<dyn AsAssetIdAccountCompare + 'a> for AssetByAccountBounds<'a> {
        fn start_bound(&self) -> Bound<&(dyn AsAssetIdAccountCompare + 'a)> {
            Bound::Excluded(&self.start)
        }

        fn end_bound(&self) -> Bound<&(dyn AsAssetIdAccountCompare + 'a)> {
            Bound::Excluded(&self.end)
        }
    }

    impl AsAssetIdAccountCompare for AssetId {
        fn as_key(&self) -> AssetIdAccountCompare<'_> {
            AssetIdAccountCompare {
                account_id: &self.account,
                definition: (&self.definition).into(),
            }
        }
    }

    impl_as_dyn_key! {
        target: AssetId,
        key: AssetIdAccountCompare<'_>,
        trait: AsAssetIdAccountCompare
    }
}

pub(crate) mod deserialize {
    use mv::serde::CellSeeded;

    use super::*;

    // Loader for [`Set`]
    #[derive(Clone, Copy)]
    pub struct WasmSeed<'e, T> {
        pub engine: &'e wasmtime::Engine,
        _marker: PhantomData<T>,
    }

    impl<'e, T> WasmSeed<'e, T> {
        pub fn cast<U>(&self) -> WasmSeed<'e, U> {
            WasmSeed {
                engine: self.engine,
                _marker: PhantomData,
            }
        }
    }

    impl<'e, 'de, T> DeserializeSeed<'de> for WasmSeed<'e, Option<T>>
    where
        WasmSeed<'e, T>: DeserializeSeed<'de, Value = T>,
    {
        type Value = Option<T>;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct OptionVisitor<'l, T> {
                loader: WasmSeed<'l, T>,
                _marker: PhantomData<T>,
            }

            impl<'e, 'de, T> Visitor<'de> for OptionVisitor<'e, T>
            where
                WasmSeed<'e, T>: DeserializeSeed<'de, Value = T>,
            {
                type Value = Option<T>;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("struct World")
                }

                fn visit_none<E>(self) -> Result<Self::Value, E>
                where
                    E: serde::de::Error,
                {
                    Ok(None)
                }

                fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    Some(self.loader.deserialize(deserializer)).transpose()
                }
            }

            let visitor = OptionVisitor {
                loader: self.cast::<T>(),
                _marker: PhantomData,
            };
            deserializer.deserialize_option(visitor)
        }
    }

    impl<'de> DeserializeSeed<'de> for WasmSeed<'_, World> {
        type Value = World;

        #[allow(clippy::too_many_lines)]
        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            struct WorldVisitor<'l> {
                loader: &'l WasmSeed<'l, World>,
            }

            impl<'de> Visitor<'de> for WorldVisitor<'_> {
                type Value = World;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("struct World")
                }

                fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
                where
                    M: MapAccess<'de>,
                {
                    let mut parameters = None;
                    let mut peers = None;
                    let mut domains = None;
                    let mut accounts = None;
                    let mut asset_definitions = None;
                    let mut assets = None;
                    let mut nfts = None;
                    let mut roles = None;
                    let mut account_permissions = None;
                    let mut account_roles = None;
                    let mut triggers = None;
                    let mut executor = None;
                    let mut executor_data_model = None;
                    let mut external_event_buf = None;

                    while let Some(key) = map.next_key::<String>()? {
                        match key.as_str() {
                            "parameters" => {
                                parameters = Some(map.next_value()?);
                            }
                            "peers" => {
                                peers = Some(map.next_value()?);
                            }
                            "domains" => {
                                domains = Some(map.next_value()?);
                            }
                            "accounts" => {
                                accounts = Some(map.next_value()?);
                            }
                            "asset_definitions" => {
                                asset_definitions = Some(map.next_value()?);
                            }
                            "assets" => {
                                assets = Some(map.next_value()?);
                            }
                            "nfts" => {
                                nfts = Some(map.next_value()?);
                            }
                            "roles" => {
                                roles = Some(map.next_value()?);
                            }
                            "account_permissions" => {
                                account_permissions = Some(map.next_value()?);
                            }
                            "account_roles" => {
                                account_roles = Some(map.next_value()?);
                            }
                            "triggers" => {
                                triggers =
                                    Some(map.next_value_seed(self.loader.cast::<TriggerSet>())?);
                            }
                            "executor" => {
                                executor = Some(map.next_value_seed(CellSeeded {
                                    seed: self.loader.cast::<Executor>(),
                                })?);
                            }
                            "executor_data_model" => {
                                executor_data_model = Some(map.next_value()?);
                            }
                            "external_event_buf" => {
                                external_event_buf = Some(map.next_value()?);
                            }

                            _ => { /* Skip unknown fields */ }
                        }
                    }

                    Ok(World {
                        parameters: parameters
                            .ok_or_else(|| serde::de::Error::missing_field("parameters"))?,
                        peers: peers.ok_or_else(|| serde::de::Error::missing_field("peers"))?,
                        domains: domains
                            .ok_or_else(|| serde::de::Error::missing_field("domains"))?,
                        accounts: accounts
                            .ok_or_else(|| serde::de::Error::missing_field("accounts"))?,
                        asset_definitions: asset_definitions
                            .ok_or_else(|| serde::de::Error::missing_field("asset_definitions"))?,
                        assets: assets.ok_or_else(|| serde::de::Error::missing_field("assets"))?,
                        nfts: nfts.ok_or_else(|| serde::de::Error::missing_field("nfts"))?,
                        roles: roles.ok_or_else(|| serde::de::Error::missing_field("roles"))?,
                        account_permissions: account_permissions.ok_or_else(|| {
                            serde::de::Error::missing_field("account_permissions")
                        })?,
                        account_roles: account_roles
                            .ok_or_else(|| serde::de::Error::missing_field("account_roles"))?,
                        triggers: triggers
                            .ok_or_else(|| serde::de::Error::missing_field("triggers"))?,
                        executor: executor
                            .ok_or_else(|| serde::de::Error::missing_field("executor"))?,
                        executor_data_model: executor_data_model.ok_or_else(|| {
                            serde::de::Error::missing_field("executor_data_model")
                        })?,
                        external_event_buf: external_event_buf
                            .ok_or_else(|| serde::de::Error::missing_field("external_event_buf"))?,
                    })
                }
            }

            deserializer.deserialize_struct(
                "World",
                &[
                    "parameters",
                    "peers",
                    "domains",
                    "roles",
                    "account_permissions",
                    "account_roles",
                    "triggers",
                    "executor",
                    "executor_data_model",
                ],
                WorldVisitor { loader: &self },
            )
        }
    }

    /// Context necessary for deserializing [`State`]
    pub struct KuraSeed {
        /// Kura subsystem reference
        pub kura: Arc<Kura>,
        /// Handle to the [`LiveQueryStore`](crate::query::store::LiveQueryStore).
        pub query_handle: LiveQueryStoreHandle,
        #[cfg(feature = "telemetry")]
        /// Handle to the metrics actor
        pub telemetry: StateTelemetry,
    }

    impl<'de> DeserializeSeed<'de> for KuraSeed {
        type Value = State;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct StateVisitor {
                loader: KuraSeed,
            }

            impl<'de> Visitor<'de> for StateVisitor {
                type Value = State;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("struct WorldState")
                }

                fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
                where
                    M: MapAccess<'de>,
                {
                    let mut world = None;
                    let mut block_hashes = None;
                    let mut transactions = None;
                    let mut commit_topology = None;
                    let mut prev_commit_topology = None;

                    let engine = wasm::create_engine();

                    let wasm_seed: WasmSeed<()> = WasmSeed {
                        engine: &engine,
                        _marker: PhantomData,
                    };

                    while let Some(key) = map.next_key::<String>()? {
                        match key.as_str() {
                            "world" => {
                                world = Some(map.next_value_seed(wasm_seed.cast::<World>())?);
                            }
                            "block_hashes" => {
                                block_hashes = Some(map.next_value()?);
                            }
                            "transactions" => {
                                transactions = Some(map.next_value()?);
                            }
                            "commit_topology" => {
                                commit_topology = Some(map.next_value()?);
                            }
                            "prev_commit_topology" => {
                                prev_commit_topology = Some(map.next_value()?);
                            }
                            _ => { /* Skip unknown fields */ }
                        }
                    }

                    Ok(State {
                        world: world.ok_or_else(|| serde::de::Error::missing_field("world"))?,
                        block_hashes: block_hashes
                            .ok_or_else(|| serde::de::Error::missing_field("block_hashes"))?,
                        transactions: transactions
                            .ok_or_else(|| serde::de::Error::missing_field("transactions"))?,
                        commit_topology: commit_topology
                            .ok_or_else(|| serde::de::Error::missing_field("commit_topology"))?,
                        prev_commit_topology: prev_commit_topology.ok_or_else(|| {
                            serde::de::Error::missing_field("prev_commit_topology")
                        })?,
                        kura: self.loader.kura,
                        query_handle: self.loader.query_handle,
                        #[cfg(feature = "telemetry")]
                        telemetry: self.loader.telemetry,
                        engine,
                        view_lock: parking_lot::RwLock::new(()),
                    })
                }
            }

            deserializer.deserialize_struct(
                "WorldState",
                &[
                    "world",
                    "block_hashes",
                    "transactions",
                    "commit_topology",
                    "prev_commit_topology",
                ],
                StateVisitor { loader: self },
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;

    use iroha_test_samples::gen_account_in;

    use super::*;
    use crate::{
        block::ValidBlock, query::store::LiveQueryStore, role::RoleIdWithOwner,
        sumeragi::network_topology::Topology,
    };

    /// Used to inject faulty payload for testing
    fn new_dummy_block_with_payload(f: impl FnOnce(&mut BlockHeader)) -> CommittedBlock {
        let (leader_public_key, leader_private_key) = iroha_crypto::KeyPair::random().into_parts();
        let peer_id = PeerId::new(leader_public_key);
        let topology = Topology::new(vec![peer_id]);

        ValidBlock::new_dummy_and_modify_header(&leader_private_key, f)
            .commit(&topology)
            .unpack(|_| {})
            .unwrap()
    }

    #[tokio::test]
    async fn get_block_hashes_after_hash() {
        const BLOCK_CNT: usize = 10;

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);

        let mut block_hashes = vec![];
        for i in 1..=BLOCK_CNT {
            let block = new_dummy_block_with_payload(|header| {
                header.height = NonZeroU64::new(i as u64).unwrap();
                header.prev_block_hash = block_hashes.last().copied();
            });

            let mut state_block = state.block(block.as_ref().header());
            block_hashes.push(block.as_ref().hash());
            let _events = state_block.apply(&block, Vec::new());
            state_block.commit();
        }

        assert!(state
            .view()
            .block_hashes()
            .iter()
            .skip_while(|&x| *x != block_hashes[6])
            .skip(1)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .eq(block_hashes.into_iter().skip(7)));
    }

    #[tokio::test]
    async fn get_blocks_from_height() {
        const BLOCK_CNT: usize = 10;

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura.clone(), query_handle);

        for i in 1..=BLOCK_CNT {
            let block = new_dummy_block_with_payload(|header| {
                header.height = NonZeroU64::new(i as u64).unwrap();
            });

            let mut state_block = state.block(block.as_ref().header());
            let _events = state_block.apply(&block, Vec::new());
            state_block.commit();
            kura.store_block(block);
        }

        assert_eq!(
            &state
                .view()
                .all_blocks(nonzero!(8_usize))
                .map(|block| block.header().height().get())
                .collect::<Vec<_>>(),
            &[8, 9, 10]
        );
    }

    #[test]
    fn role_account_range() {
        let (account_id, _account_keypair) = gen_account_in("wonderland");
        let roles = [
            RoleIdWithOwner::new(account_id.clone(), "1".parse().unwrap()),
            RoleIdWithOwner::new(account_id.clone(), "2".parse().unwrap()),
            RoleIdWithOwner::new(gen_account_in("wonderland").0, "3".parse().unwrap()),
            RoleIdWithOwner::new(gen_account_in("wonderland").0, "4".parse().unwrap()),
            RoleIdWithOwner::new(gen_account_in("0").0, "5".parse().unwrap()),
            RoleIdWithOwner::new(gen_account_in("1").0, "6".parse().unwrap()),
        ]
        .map(|role| (role, ()));
        let map = Storage::from_iter(roles);

        let view = map.view();
        let range = view
            .range(RoleIdByAccountBounds::new(&account_id))
            .collect::<Vec<_>>();
        assert_eq!(range.len(), 2);
        for (role, ()) in range {
            assert_eq!(&role.account, &account_id);
        }
    }

    #[test]
    fn account_domain_range() {
        let accounts = [
            gen_account_in("wonderland").0,
            gen_account_in("wonderland").0,
            gen_account_in("a").0,
            gen_account_in("b").0,
            gen_account_in("z").0,
            gen_account_in("z").0,
        ]
        .map(|account| (account, ()));
        let map = Storage::from_iter(accounts);

        let domain_id = "kingdom".parse().unwrap();
        let view = map.view();
        let range = view.range(AccountByDomainBounds::new(&domain_id));
        assert_eq!(range.count(), 0);

        let domain_id = "wonderland".parse().unwrap();
        let view = map.view();
        let range = view
            .range(AccountByDomainBounds::new(&domain_id))
            .collect::<Vec<_>>();
        assert_eq!(range.len(), 2);
        for (account, ()) in range {
            assert_eq!(&account.domain, &domain_id);
        }
    }

    #[test]
    fn asset_account_range() {
        let domain_id: DomainId = "wonderland".parse().unwrap();

        let account_id = gen_account_in("wonderland").0;

        let accounts = [
            account_id.clone(),
            account_id.clone(),
            gen_account_in("a").0,
            gen_account_in("b").0,
            gen_account_in("z").0,
            gen_account_in("z").0,
        ];
        let asset_definitions = [
            AssetDefinitionId::new(domain_id.clone(), "a".parse().unwrap()),
            AssetDefinitionId::new(domain_id.clone(), "f".parse().unwrap()),
            AssetDefinitionId::new(domain_id.clone(), "b".parse().unwrap()),
            AssetDefinitionId::new(domain_id.clone(), "c".parse().unwrap()),
            AssetDefinitionId::new(domain_id.clone(), "d".parse().unwrap()),
            AssetDefinitionId::new(domain_id.clone(), "e".parse().unwrap()),
        ];

        let assets = accounts
            .into_iter()
            .zip(asset_definitions)
            .map(|(account, asset_definition)| AssetId::new(asset_definition, account))
            .map(|asset| (asset, ()));

        let map: Storage<_, _> = assets.collect();
        let view = map.view();
        let range = view.range(AssetByAccountBounds::new(&account_id));
        assert_eq!(range.count(), 2);
    }
}
