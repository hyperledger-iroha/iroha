//! This module provides the [`WorldStateView`] — an in-memory representation of the current blockchain
//! state.
#![allow(
    clippy::new_without_default,
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::arithmetic_side_effects
)]

use std::{
    borrow::Borrow, collections::BTreeSet, convert::Infallible, fmt::Debug, sync::Arc,
    time::Duration,
};

use dashmap::{
    mapref::one::{Ref as DashMapRef, RefMut as DashMapRefMut},
    DashSet,
};
use eyre::Result;
use iroha_config::{
    base::proxy::Builder,
    wsv::{Configuration, ConfigurationProxy},
};
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{CommittedBlock, VersionedCommittedBlock},
    isi::error::{InstructionExecutionFailure as Error, MathError},
    parameter::Parameter,
    prelude::*,
    query::error::{FindError, QueryExecutionFailure},
    trigger::action::ActionTrait,
};
use iroha_logger::prelude::*;
use iroha_primitives::small::SmallVec;
use parking_lot::RwLock;

#[cfg(test)]
use crate::validator::MockValidator as Validator;
#[cfg(not(test))]
use crate::validator::Validator;
use crate::{
    kura::Kura,
    prelude::*,
    smartcontracts::{
        triggers::{
            self,
            set::{LoadedExecutable, LoadedWasm, Set as TriggerSet},
        },
        wasm, Execute,
    },
    tx::TransactionValidator,
    DomainsMap, Parameters, PeersIds,
};

/// The global entity consisting of `domains`, `triggers` and etc.
/// For example registration of domain, will have this as an ISI target.
#[derive(Debug, Default)]
pub struct World {
    /// Iroha config parameters.
    pub(crate) parameters: Parameters,
    /// Identifications of discovered trusted peers.
    pub(crate) trusted_peers_ids: PeersIds,
    /// Registered domains.
    pub(crate) domains: DomainsMap,
    /// Roles. [`Role`] pairs.
    pub(crate) roles: crate::RolesMap,
    /// Permission tokens of an account.
    pub(crate) account_permission_tokens: crate::PermissionTokensMap,
    /// Registered permission token ids.
    pub(crate) permission_token_definitions: crate::PermissionTokenDefinitionsMap,
    /// Triggers
    pub(crate) triggers: TriggerSet,
    /// Runtime Validator
    pub(crate) validator: RwLock<Option<Validator>>,
    /// New version of Validator, which will replace `validator` on the next
    /// [`validator_view()`}(WorldStateView::validator_view call.
    pub(crate) upgraded_validator: RwLock<Option<Validator>>,
}

impl Clone for World {
    #[cfg_attr(test, allow(clippy::clone_on_copy))]
    fn clone(&self) -> Self {
        Self {
            parameters: self.parameters.clone(),
            trusted_peers_ids: self.trusted_peers_ids.clone(),
            domains: self.domains.clone(),
            roles: self.roles.clone(),
            account_permission_tokens: self.account_permission_tokens.clone(),
            permission_token_definitions: self.permission_token_definitions.clone(),
            triggers: self.triggers.clone(),
            validator: RwLock::new(self.validator.read().clone()),
            upgraded_validator: RwLock::new(self.upgraded_validator.read().clone()),
        }
    }
}

impl World {
    /// Creates an empty `World`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a [`World`] with these [`Domain`]s and trusted [`PeerId`]s.
    pub fn with<D, P>(domains: D, trusted_peers_ids: P) -> Self
    where
        D: IntoIterator<Item = Domain>,
        P: IntoIterator<Item = PeerId>,
    {
        let domains = domains
            .into_iter()
            .map(|domain| (domain.id().clone(), domain))
            .collect();
        let trusted_peers_ids = trusted_peers_ids.into_iter().collect();
        World {
            domains,
            trusted_peers_ids,
            ..World::new()
        }
    }
}

/// Current state of the blockchain aligned with `Iroha` module.
pub struct WorldStateView {
    /// The world. Contains `domains`, `triggers`, `roles` and other data representing the current state of the blockchain.
    pub world: World,
    /// Configuration of World State View.
    pub config: std::cell::RefCell<Configuration>,
    /// Blockchain.
    pub block_hashes: std::cell::RefCell<Vec<HashOf<VersionedCommittedBlock>>>,
    /// Hashes of transactions
    pub transactions: DashSet<HashOf<VersionedSignedTransaction>>,
    /// Buffer containing events generated during `WorldStateView::apply`. Renewed on every block commit.
    pub events_buffer: std::cell::RefCell<Vec<Event>>,
    /// Accumulated amount of any asset that has been transacted.
    pub metric_tx_amounts: std::cell::Cell<f64>,
    /// Count of how many mints, transfers and burns have happened.
    pub metric_tx_amounts_counter: std::cell::Cell<u64>,

    /// Reference to Kura subsystem.
    kura: Arc<Kura>,
}

impl Clone for WorldStateView {
    fn clone(&self) -> Self {
        Self {
            world: Clone::clone(&self.world),
            config: self.config.clone(),
            block_hashes: self.block_hashes.clone(),
            transactions: self.transactions.clone(),
            events_buffer: std::cell::RefCell::new(Vec::new()),
            metric_tx_amounts: std::cell::Cell::new(0.0_f64),
            metric_tx_amounts_counter: std::cell::Cell::new(0),
            kura: Arc::clone(&self.kura),
        }
    }
}

/// WARNING!!! INTERNAL USE ONLY!!!
impl WorldStateView {
    /// Construct [`WorldStateView`] with given [`World`].
    #[must_use]
    #[inline]
    pub fn new(world: World, kura: Arc<Kura>) -> Self {
        // Added to remain backward compatible with other code primary in tests
        let config = ConfigurationProxy::default()
            .build()
            .expect("Wsv proxy always builds");
        Self::from_configuration(config, world, kura)
    }

    /// Get `Account`'s `Asset`s
    ///
    /// # Errors
    /// Fails if there is no domain or account
    pub fn account_assets(&self, id: &AccountId) -> Result<Vec<Asset>, QueryExecutionFailure> {
        self.map_account(id, |account| account.assets.values().cloned().collect())
    }

    /// Return a set of all permission tokens granted to this account.
    pub fn account_permission_tokens(&self, account: &Account) -> BTreeSet<PermissionToken> {
        let mut tokens: BTreeSet<PermissionToken> =
            self.account_inherent_permission_tokens(account).collect();
        for role_id in &account.roles {
            if let Some(role) = self.world.roles.get(role_id) {
                tokens.append(&mut role.permissions.clone());
            }
        }
        tokens
    }

    /// Return a set of permission tokens granted to this account not as part of any role.
    pub fn account_inherent_permission_tokens(
        &self,
        account: &Account,
    ) -> impl ExactSizeIterator<Item = PermissionToken> {
        self.world
            .account_permission_tokens
            .get(&account.id)
            .map_or_else(Default::default, |permissions_ref| {
                permissions_ref.value().clone()
            })
            .into_iter()
    }

    /// Return `true` if [`Account`] contains a permission token not associated with any role.
    #[inline]
    pub fn account_contains_inherent_permission(
        &self,
        account: &<Account as Identifiable>::Id,
        token: &PermissionToken,
    ) -> bool {
        self.world
            .account_permission_tokens
            .get_mut(account)
            .map_or(false, |permissions| permissions.contains(token))
    }

    /// Add [`permission`](PermissionToken) to the [`Account`] if the account does not have this permission yet.
    ///
    /// Return a Boolean value indicating whether or not the  [`Account`] already had this permission.
    pub fn add_account_permission(
        &self,
        account: &<Account as Identifiable>::Id,
        token: PermissionToken,
    ) -> bool {
        // `match` here instead of `map_or_else` to avoid cloning token into each closure
        match self.world.account_permission_tokens.get_mut(account) {
            None => {
                self.world
                    .account_permission_tokens
                    .insert(account.clone(), BTreeSet::from([token]));
                true
            }
            Some(mut permissions) => {
                if permissions.contains(&token) {
                    return true;
                }
                permissions.insert(token);
                false
            }
        }
    }

    /// Remove a [`permission`](PermissionToken) from the [`Account`] if the account has this permission.
    /// Return a Boolean value indicating whether the [`Account`] had this permission.
    pub fn remove_account_permission(
        &self,
        account: &<Account as Identifiable>::Id,
        token: &PermissionToken,
    ) -> bool {
        self.world
            .account_permission_tokens
            .get_mut(account)
            .map_or(false, |mut permissions| permissions.remove(token))
    }

    fn process_trigger(
        &self,
        id: &TriggerId,
        engine: wasmtime::Engine,
        action: &dyn ActionTrait<Executable = LoadedExecutable>,
        event: Event,
    ) -> Result<()> {
        use triggers::set::LoadedExecutable::*;
        let authority = action.technical_account();

        match action.executable() {
            Instructions(instructions) => {
                self.process_instructions(instructions.iter().cloned(), authority)
            }
            Wasm(LoadedWasm { module, .. }) => {
                let mut wasm_runtime = wasm::RuntimeBuilder::new()
                    .with_configuration(self.config.borrow().wasm_runtime_config)
                    .with_engine(engine)
                    .build()?;
                wasm_runtime
                    .execute_trigger_module(self, id, authority.clone(), module, event)
                    .map_err(Into::into)
            }
        }
    }

    fn process_executable(&self, executable: &Executable, authority: AccountId) -> Result<()> {
        match executable {
            Executable::Instructions(instructions) => {
                self.process_instructions(instructions.iter().cloned(), &authority)
            }
            Executable::Wasm(bytes) => {
                let mut wasm_runtime = wasm::RuntimeBuilder::new()
                    .with_configuration(self.config.borrow().wasm_runtime_config)
                    .build()?;
                wasm_runtime
                    .execute(self, authority, bytes)
                    .map_err(Into::into)
            }
        }
    }

    fn process_instructions(
        &self,
        instructions: impl IntoIterator<Item = InstructionBox>,
        authority: &AccountId,
    ) -> Result<()> {
        instructions.into_iter().try_for_each(|instruction| {
            instruction.execute(authority, self)?;
            Ok::<_, eyre::Report>(())
        })
    }

    /// Apply `CommittedBlock` with changes in form of **Iroha Special
    /// Instructions** to `self`.
    ///
    /// Order of execution:
    /// 1) Transactions
    /// 2) Triggers
    ///
    /// # Errors
    ///
    /// - (RARE) if applying transaction after validation fails.  This
    /// scenario is rare, because the `tx` validation implies applying
    /// instructions directly to a clone of the wsv.  If this happens,
    /// you likely have data corruption.
    /// - If trigger execution fails
    /// - If timestamp conversion to `u64` fails
    #[iroha_logger::log(skip_all, fields(block_height))]
    pub fn apply(&self, block: &VersionedCommittedBlock) -> Result<()> {
        let hash = block.hash();
        let block = block.as_v1();
        iroha_logger::prelude::Span::current().record("block_height", block.header.height);
        trace!("Applying block");
        let time_event = self.create_time_event(block)?;
        self.events_buffer
            .borrow_mut()
            .push(Event::Time(time_event));

        self.execute_transactions(block)?;
        debug!("All block transactions successfully executed");

        self.world.triggers.handle_time_event(time_event);

        let res = self
            .world
            .triggers
            .inspect_matched(|id, engine, action, event| -> Result<()> {
                self.process_trigger(id, engine, action, event)
            });

        if let Err(errors) = res {
            warn!(
                ?errors,
                "The following errors have occurred during trigger execution"
            );
        }

        self.block_hashes.borrow_mut().push(hash);

        self.apply_parameters();

        Ok(())
    }

    fn apply_parameters(&self) {
        use iroha_data_model::parameter::default::*;
        macro_rules! update_params {
            ($ident:ident, $($param:expr => $config:expr),+ $(,)?) => {
                let mut $ident = self.config.borrow_mut();
                $(if let Some(param) = self.query_param($param) {
                    $config = param;
                })+

            };
        }
        update_params! {
            config,
            WSV_ASSET_METADATA_LIMITS => config.asset_metadata_limits,
            WSV_ASSET_DEFINITION_METADATA_LIMITS => config.asset_definition_metadata_limits,
            WSV_ACCOUNT_METADATA_LIMITS => config.account_metadata_limits,
            WSV_DOMAIN_METADATA_LIMITS => config.domain_metadata_limits,
            WSV_IDENT_LENGTH_LIMITS => config.ident_length_limits,
            WASM_FUEL_LIMIT => config.wasm_runtime_config.fuel_limit,
            WASM_MAX_MEMORY => config.wasm_runtime_config.max_memory,
            TRANSACTION_LIMITS => config.transaction_limits,
        }
    }

    /// Get transaction validator
    pub fn transaction_validator(&self) -> TransactionValidator {
        TransactionValidator::new(self.config.borrow().transaction_limits)
    }

    /// Get a reference to the latest block. Returns none if genesis is not committed.
    #[inline]
    pub fn latest_block_ref(&self) -> Option<Arc<VersionedCommittedBlock>> {
        self.kura
            .get_block_by_height(self.block_hashes.borrow().len() as u64)
    }

    /// Create time event using previous and current blocks
    fn create_time_event(&self, block: &CommittedBlock) -> Result<TimeEvent> {
        let prev_interval = self
            .latest_block_ref()
            .map(|latest_block| {
                let header = &latest_block.as_v1().header;
                header.timestamp.try_into().map(|since| TimeInterval {
                    since: Duration::from_millis(since),
                    length: Duration::from_millis(header.consensus_estimation),
                })
            })
            .transpose()?;

        let interval = TimeInterval {
            since: Duration::from_millis(block.header.timestamp.try_into()?),
            length: Duration::from_millis(block.header.consensus_estimation),
        };

        Ok(TimeEvent {
            prev_interval,
            interval,
        })
    }

    /// Execute `block` transactions and store their hashes as well as
    /// `rejected_transactions` hashes
    ///
    /// # Errors
    /// Fails if transaction instruction execution fails
    fn execute_transactions(&self, block: &CommittedBlock) -> Result<()> {
        // TODO: Should this block panic instead?
        for tx in &block.transactions {
            self.process_executable(
                &tx.as_v1().payload.instructions,
                tx.payload().account_id.clone(),
            )?;
            self.transactions.insert(tx.hash());
        }
        for tx in &block.rejected_transactions {
            self.transactions.insert(tx.hash());
        }

        Ok(())
    }

    /// Get `Asset` by its id
    ///
    /// # Errors
    /// - No such [`Asset`]
    /// - The [`Account`] with which the [`Asset`] is associated doesn't exist.
    /// - The [`Domain`] with which the [`Account`] is associated doesn't exist.
    pub fn asset(&self, id: &<Asset as Identifiable>::Id) -> Result<Asset, QueryExecutionFailure> {
        self.map_account(
            &id.account_id,
            |account| -> Result<Asset, QueryExecutionFailure> {
                account
                    .assets
                    .get(id)
                    .ok_or_else(|| {
                        QueryExecutionFailure::from(Box::new(FindError::Asset(id.clone())))
                    })
                    .map(Clone::clone)
            },
        )?
    }

    /// Get asset or inserts new with `default_asset_value`.
    ///
    /// # Errors
    /// - There is no account with such name.
    #[allow(clippy::missing_panics_doc)]
    pub fn asset_or_insert(
        &self,
        id: &<Asset as Identifiable>::Id,
        default_asset_value: impl Into<AssetValue>,
    ) -> Result<Asset, Error> {
        if let Ok(asset) = self.asset(id) {
            return Ok(asset);
        }

        // This function is strictly infallible.
        self.modify_account(&id.account_id, |account| {
            let asset = Asset::new(id.clone(), default_asset_value.into());
            assert!(account.add_asset(asset.clone()).is_none());

            Ok(AccountEvent::Asset(AssetEvent::Created(asset)))
        })
        .map_err(|err| {
            iroha_logger::warn!(?err);
            err
        })?;

        self.asset(id).map_err(Into::into)
    }

    /// Load all blocks in the block chain from disc and clone them for use.
    pub fn all_blocks_by_value(
        &self,
    ) -> impl DoubleEndedIterator<Item = VersionedCommittedBlock> + '_ {
        let block_count = self.block_hashes.borrow().len() as u64;
        (1..=block_count)
            .map(|height| {
                self.kura
                    .get_block_by_height(height)
                    .expect("Failed to load block.")
            })
            .map(|block| VersionedCommittedBlock::clone(&block))
    }

    /// Return a vector of blockchain blocks after the block with the given `hash`
    pub fn block_hashes_after_hash(
        &self,
        hash: Option<HashOf<VersionedCommittedBlock>>,
    ) -> Vec<HashOf<VersionedCommittedBlock>> {
        hash.map_or_else(
            || self.block_hashes.borrow().clone(),
            |block_hash| {
                self.block_hashes
                    .borrow()
                    .iter()
                    .skip_while(|&x| *x != block_hash)
                    .skip(1)
                    .copied()
                    .collect()
            },
        )
    }

    /// The same as [`Self::modify_world_multiple_events`] except closure `f` returns a single [`WorldEvent`].
    ///
    /// # Errors
    /// Forward errors from [`Self::modify_world_multiple_events`]
    pub fn modify_world(
        &self,
        f: impl FnOnce(&World) -> Result<WorldEvent, Error>,
    ) -> Result<(), Error> {
        self.modify_world_multiple_events(move |world| f(world).map(std::iter::once))
    }

    /// Get [`World`] and pass it to `closure` to modify it.
    ///
    /// The function puts events produced by `f` into `events_buffer`.
    /// Events should be produced in the order of expanding scope: from specific to general.
    /// Example: account events before domain events.
    ///
    /// # Errors
    /// Forward errors from `f`
    pub fn modify_world_multiple_events<I: IntoIterator<Item = WorldEvent>>(
        &self,
        f: impl FnOnce(&World) -> Result<I, Error>,
    ) -> Result<(), Error> {
        let world_events = f(&self.world)?;
        let data_events: SmallVec<[DataEvent; 3]> = world_events
            .into_iter()
            .flat_map(WorldEvent::flatten)
            .collect();

        for event in data_events.iter() {
            self.world.triggers.handle_data_event(event.clone());
        }
        self.events_buffer
            .borrow_mut()
            .extend(data_events.into_iter().map(Into::into));

        Ok(())
    }

    /// Returns reference for trusted peer ids
    #[inline]
    pub fn peers_ids(&self) -> &PeersIds {
        &self.world.trusted_peers_ids
    }

    /// Return an iterator over blockchain block hashes starting with the block of the given `height`
    pub fn block_hashes_from_height(&self, height: usize) -> Vec<HashOf<VersionedCommittedBlock>> {
        self.block_hashes
            .borrow()
            .iter()
            .skip(height.saturating_sub(1))
            .copied()
            .collect()
    }

    /// Get `Domain` without an ability to modify it.
    ///
    /// # Errors
    /// Fails if there is no domain
    pub fn domain(
        &self,
        id: &<Domain as Identifiable>::Id,
    ) -> Result<DashMapRef<DomainId, Domain>, FindError> {
        let domain = self
            .world
            .domains
            .get(id)
            .ok_or_else(|| FindError::Domain(id.clone()))?;
        Ok(domain)
    }

    /// Get `Domain` with an ability to modify it.
    ///
    /// # Errors
    /// Fails if there is no domain
    pub fn domain_mut(
        &self,
        id: &<Domain as Identifiable>::Id,
    ) -> Result<DashMapRefMut<DomainId, Domain>, FindError> {
        let domain = self
            .world
            .domains
            .get_mut(id)
            .ok_or_else(|| FindError::Domain(id.clone()))?;
        Ok(domain)
    }

    /// Returns reference for domains map
    #[inline]
    pub fn domains(&self) -> &DomainsMap {
        &self.world.domains
    }

    /// Get `Domain` and pass it to closure.
    ///
    /// # Errors
    /// Fails if there is no domain
    #[allow(clippy::panic_in_result_fn)]
    pub fn map_domain<T>(
        &self,
        id: &<Domain as Identifiable>::Id,
        f: impl FnOnce(&Domain) -> Result<T, Infallible>,
    ) -> Result<T, FindError> {
        let domain = self.domain(id)?;
        let value = f(domain.value()).map_or_else(
            |_infallible| unreachable!("Returning `Infallible` should not be possible"),
            |value| value,
        );
        Ok(value)
    }

    /// The same as [`Self::modify_domain_multiple_events`] except closure `f` returns a single [`DomainEvent`].
    ///
    /// # Errors
    /// Forward errors from [`Self::modify_domain_multiple_events`]
    pub fn modify_domain(
        &self,
        id: &<Domain as Identifiable>::Id,
        f: impl FnOnce(&mut Domain) -> Result<DomainEvent, Error>,
    ) -> Result<(), Error> {
        self.modify_domain_multiple_events(id, move |domain| f(domain).map(std::iter::once))
    }

    /// Get [`Domain`] and pass it to `closure` to modify it
    ///
    /// # Errors
    /// - If there is no domain
    /// - Forward errors from `f`
    pub fn modify_domain_multiple_events<I: IntoIterator<Item = DomainEvent>>(
        &self,
        id: &<Domain as Identifiable>::Id,
        f: impl FnOnce(&mut Domain) -> Result<I, Error>,
    ) -> Result<(), Error> {
        self.modify_world_multiple_events(|world| {
            let mut domain = world
                .domains
                .get_mut(id)
                .ok_or_else(|| FindError::Domain(id.clone()))?;
            f(domain.value_mut()).map(|events| events.into_iter().map(Into::into))
        })
    }

    /// Get all roles
    #[inline]
    pub fn roles(&self) -> &crate::RolesMap {
        &self.world.roles
    }

    /// Get all permission token ids
    #[inline]
    pub fn permission_token_definitions(&self) -> &crate::PermissionTokenDefinitionsMap {
        &self.world.permission_token_definitions
    }

    /// Construct [`WorldStateView`] with specific [`Configuration`].
    #[inline]
    pub fn from_configuration(config: Configuration, world: World, kura: Arc<Kura>) -> Self {
        Self {
            world,
            config: std::cell::RefCell::new(config),
            transactions: DashSet::new(),
            block_hashes: std::cell::RefCell::new(Vec::new()),
            events_buffer: std::cell::RefCell::new(Vec::new()),
            metric_tx_amounts: std::cell::Cell::new(0.0_f64),
            metric_tx_amounts_counter: std::cell::Cell::new(0),
            kura,
        }
    }

    /// Returns [`Some`] milliseconds since the genesis block was
    /// committed, or [`None`] if it wasn't.
    #[inline]
    pub fn genesis_timestamp(&self) -> Option<u128> {
        if self.block_hashes.borrow().is_empty() {
            None
        } else {
            let opt = self
                .kura
                .get_block_by_height(1)
                .map(|genesis_block| genesis_block.as_v1().header.timestamp);

            if opt.is_none() {
                error!("Failed to get genesis block from Kura.");
            }
            opt
        }
    }

    /// Check if this [`VersionedSignedTransaction`] is already committed or rejected.
    #[inline]
    pub fn has_transaction(&self, hash: &HashOf<VersionedSignedTransaction>) -> bool {
        self.transactions.contains(hash)
    }

    /// Height of blockchain
    #[inline]
    pub fn height(&self) -> u64 {
        self.block_hashes.borrow().len() as u64
    }

    /// Return the hash of the latest block
    pub fn latest_block_hash(&self) -> Option<HashOf<VersionedCommittedBlock>> {
        self.block_hashes.borrow().iter().nth_back(0).copied()
    }

    /// Return the view change index of the latest block
    pub fn latest_block_view_change_index(&self) -> u64 {
        self.kura
            .get_block_by_height(self.height())
            .map_or(0, |block| block.as_v1().header.view_change_index)
    }

    /// Return the hash of the block one before the latest block
    pub fn previous_block_hash(&self) -> Option<HashOf<VersionedCommittedBlock>> {
        self.block_hashes.borrow().iter().nth_back(1).copied()
    }

    /// Get `Account` and pass it to closure.
    ///
    /// # Errors
    /// Fails if there is no domain or account
    pub fn map_account<T>(
        &self,
        id: &AccountId,
        f: impl FnOnce(&Account) -> T,
    ) -> Result<T, QueryExecutionFailure> {
        let domain = self.domain(&id.domain_id)?;
        let account = domain
            .accounts
            .get(id)
            .ok_or(QueryExecutionFailure::Unauthorized)?;
        Ok(f(account))
    }

    /// The same as [`Self::modify_account_multiple_events`] except closure `f` returns a single [`AccountEvent`].
    ///
    /// # Errors
    /// Forward errors from [`Self::modify_account_multiple_events`]
    pub fn modify_account(
        &self,
        id: &AccountId,
        f: impl FnOnce(&mut Account) -> Result<AccountEvent, Error>,
    ) -> Result<(), Error> {
        self.modify_account_multiple_events(id, move |account| f(account).map(std::iter::once))
    }

    /// Get [`Account`] and pass it to `closure` to modify it
    ///
    /// # Errors
    /// - If there is no domain or account
    /// - Forward errors from `f`
    pub fn modify_account_multiple_events<I: IntoIterator<Item = AccountEvent>>(
        &self,
        id: &AccountId,
        f: impl FnOnce(&mut Account) -> Result<I, Error>,
    ) -> Result<(), Error> {
        self.modify_domain_multiple_events(&id.domain_id, |domain| {
            let account = domain
                .accounts
                .get_mut(id)
                .ok_or_else(|| FindError::Account(id.clone()))?;
            f(account).map(|events| events.into_iter().map(DomainEvent::Account))
        })
    }

    /// The same as [`Self::modify_asset_multiple_events`] except closure `f` returns a single [`AssetEvent`].
    ///
    /// # Errors
    /// Forward errors from [`Self::modify_asset_multiple_events`]
    pub fn modify_asset(
        &self,
        id: &<Asset as Identifiable>::Id,
        f: impl FnOnce(&mut Asset) -> Result<AssetEvent, Error>,
    ) -> Result<(), Error> {
        self.modify_asset_multiple_events(id, move |asset| f(asset).map(std::iter::once))
    }

    /// Get [`Asset`] and pass it to `closure` to modify it.
    /// If asset value hits 0 after modification, asset is removed from the [`Account`].
    ///
    /// # Errors
    /// - If there are no such asset or account
    /// - Forward errors from `f`
    ///
    /// # Panics
    /// If removing asset from account failed
    pub fn modify_asset_multiple_events<I: IntoIterator<Item = AssetEvent>>(
        &self,
        id: &<Asset as Identifiable>::Id,
        f: impl FnOnce(&mut Asset) -> Result<I, Error>,
    ) -> Result<(), Error> {
        self.modify_account_multiple_events(&id.account_id, |account| {
            let asset = account
                .assets
                .get_mut(id)
                .ok_or_else(|| FindError::Asset(id.clone()))?;

            let events_result = f(asset);
            if asset.value.is_zero_value() {
                assert!(account.remove_asset(id).is_some());
            }

            events_result.map(|events| events.into_iter().map(AccountEvent::Asset))
        })
    }

    /// The same as [`Self::modify_asset_definition_multiple_events`] except closure `f` returns a single [`AssetDefinitionEvent`].
    ///
    /// # Errors
    /// Forward errors from [`Self::modify_asset_definition_multiple_events`]
    pub fn modify_asset_definition(
        &self,
        id: &<AssetDefinition as Identifiable>::Id,
        f: impl FnOnce(&mut AssetDefinition) -> Result<AssetDefinitionEvent, Error>,
    ) -> Result<(), Error> {
        self.modify_asset_definition_multiple_events(id, move |asset_definition| {
            f(asset_definition).map(std::iter::once)
        })
    }

    /// Get [`AssetDefinition`] and pass it to `closure` to modify it
    ///
    /// # Errors
    /// - If asset definition entry does not exist
    /// - Forward errors from `f`
    pub fn modify_asset_definition_multiple_events<I: IntoIterator<Item = AssetDefinitionEvent>>(
        &self,
        id: &<AssetDefinition as Identifiable>::Id,
        f: impl FnOnce(&mut AssetDefinition) -> Result<I, Error>,
    ) -> Result<(), Error> {
        self.modify_domain_multiple_events(&id.domain_id, |domain| {
            let asset_definition = domain
                .asset_definitions
                .get_mut(id)
                .ok_or_else(|| FindError::AssetDefinition(id.clone()))?;
            f(asset_definition).map(|events| events.into_iter().map(DomainEvent::AssetDefinition))
        })
    }

    /// Get all `PeerId`s without an ability to modify them.
    pub fn peers(&self) -> Vec<Peer> {
        let mut vec = self
            .world
            .trusted_peers_ids
            .iter()
            .map(|peer| Peer::new((*peer).clone()))
            .collect::<Vec<Peer>>();
        vec.sort();
        vec
    }

    /// Get all `Parameter`s registered in the world.
    pub fn parameters(&self) -> Vec<Parameter> {
        self.world
            .parameters
            .iter()
            .map(|param| param.clone())
            .collect::<Vec<Parameter>>()
    }

    /// Query parameter and convert it to a proper type
    pub fn query_param<T: TryFrom<Value>, P: core::hash::Hash + Eq + ?Sized>(
        &self,
        param: &P,
    ) -> Option<T>
    where
        Parameter: Borrow<P>,
    {
        self.world
            .parameters
            .get(param)
            .as_ref()
            .and_then(|param| param.key().val.clone().try_into().ok())
    }

    /// Get `AssetDefinition` immutable view.
    ///
    /// # Errors
    /// - Asset definition entry not found
    pub fn asset_definition(
        &self,
        asset_id: &<AssetDefinition as Identifiable>::Id,
    ) -> Result<AssetDefinition, FindError> {
        self.domain(&asset_id.domain_id)?
            .asset_definitions
            .get(asset_id)
            .ok_or_else(|| FindError::AssetDefinition(asset_id.clone()))
            .map(Clone::clone)
    }

    /// Get total amount of [`Asset`].
    ///
    /// # Errors
    /// - Asset definition not found
    pub fn asset_total_amount(
        &self,
        definition_id: &<AssetDefinition as Identifiable>::Id,
    ) -> Result<NumericValue, FindError> {
        self.domain(&definition_id.domain_id)?
            .asset_total_quantities
            .get(definition_id)
            .ok_or_else(|| FindError::AssetDefinition(definition_id.clone()))
            .copied()
    }

    /// Increase [`Asset`] total amount by given value
    ///
    /// # Errors
    /// - [`AssetDefinition`], [`Domain`] not found
    /// - Overflow
    pub fn increase_asset_total_amount<I>(
        &self,
        definition_id: &<AssetDefinition as Identifiable>::Id,
        increment: I,
    ) -> Result<(), Error>
    where
        I: iroha_primitives::CheckedOp + Copy,
        NumericValue: From<I> + TryAsMut<I>,
        eyre::Error: From<<NumericValue as TryAsMut<I>>::Error>,
    {
        self.modify_domain(&definition_id.domain_id, |domain| {
            let asset_total_amount: &mut I = domain
                .asset_total_quantities.get_mut(definition_id)
                .expect("Asset total amount not being found is a bug: check `Register<AssetDefinition>` to insert initial total amount")
                .try_as_mut()
                .map_err(eyre::Error::from)
                .map_err(|e| Error::Conversion(e.to_string()))?;
            *asset_total_amount = asset_total_amount
                .checked_add(increment)
                .ok_or(MathError::Overflow)?;

            Ok(
                DomainEvent::AssetDefinition(
                    AssetDefinitionEvent::TotalQuantityChanged(
                        AssetDefinitionTotalQuantityChanged {
                            asset_definition_id: definition_id.clone(),
                            total_amount: NumericValue::from(*asset_total_amount)
                        }
                    )
                )
            )
        })
    }

    /// Decrease [`Asset`] total amount by given value
    ///
    /// # Errors
    /// - [`AssetDefinition`], [`Domain`] not found
    /// - Not enough quantity
    pub fn decrease_asset_total_amount<I>(
        &self,
        definition_id: &<AssetDefinition as Identifiable>::Id,
        decrement: I,
    ) -> Result<(), Error>
    where
        I: iroha_primitives::CheckedOp + Copy,
        NumericValue: From<I> + TryAsMut<I>,
        eyre::Error: From<<NumericValue as TryAsMut<I>>::Error>,
    {
        self.modify_domain(&definition_id.domain_id, |domain| {
            let asset_total_amount: &mut I = domain
                .asset_total_quantities.get_mut(definition_id)
                .expect("Asset total amount not being found is a bug: check `Register<AssetDefinition>` to insert initial total amount")
                .try_as_mut()
                .map_err(eyre::Error::from)
                .map_err(|e| Error::Conversion(e.to_string()))?;
            *asset_total_amount = asset_total_amount
                .checked_sub(decrement)
                .ok_or(MathError::NotEnoughQuantity)?;

            Ok(
                DomainEvent::AssetDefinition(
                    AssetDefinitionEvent::TotalQuantityChanged(
                        AssetDefinitionTotalQuantityChanged {
                            asset_definition_id: definition_id.clone(),
                            total_amount: NumericValue::from(*asset_total_amount)
                        }
                    )
                )
            )
        })
    }

    /// Get all transactions
    pub fn transaction_values(&self) -> Vec<TransactionQueryResult> {
        let mut txs = self
            .all_blocks_by_value()
            .flat_map(|block| {
                let block = block.as_v1();
                block
                    .rejected_transactions
                    .iter()
                    .cloned()
                    .map(|versioned_rejected_tx| TransactionQueryResult {
                        tx_value: TransactionValue::RejectedTransaction(versioned_rejected_tx),
                        block_hash: Hash::from(block.hash()),
                    })
                    .chain(
                        block
                            .transactions
                            .iter()
                            .cloned()
                            .map(VersionedSignedTransaction::from)
                            .map(|versioned_tx| TransactionQueryResult {
                                tx_value: TransactionValue::Transaction(versioned_tx),
                                block_hash: Hash::from(block.hash()),
                            }),
                    )
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        txs.sort();
        txs
    }

    /// Find a [`VersionedSignedTransaction`] by hash.
    pub fn transaction_value_by_hash(
        &self,
        hash: &HashOf<VersionedSignedTransaction>,
    ) -> Option<TransactionValue> {
        self.all_blocks_by_value().find_map(|b| {
            b.as_v1()
                .rejected_transactions
                .iter()
                .find(|e| e.hash() == *hash)
                .cloned()
                .map(TransactionValue::RejectedTransaction)
                .or_else(|| {
                    b.as_v1()
                        .transactions
                        .iter()
                        .find(|e| e.hash() == *hash)
                        .cloned()
                        .map(VersionedSignedTransaction::from)
                        .map(TransactionValue::Transaction)
                })
        })
    }

    /// Get committed and rejected transaction of the account.
    pub fn transactions_values_by_account_id(
        &self,
        account_id: &AccountId,
    ) -> Vec<TransactionValue> {
        let mut transactions = self
            .all_blocks_by_value()
            .flat_map(|block_entry| {
                let block = block_entry.as_v1();
                block
                    .rejected_transactions
                    .iter()
                    .filter(|transaction| &transaction.payload().account_id == account_id)
                    .cloned()
                    .map(TransactionValue::RejectedTransaction)
                    .chain(
                        block
                            .transactions
                            .iter()
                            .filter(|transaction| &transaction.payload().account_id == account_id)
                            .cloned()
                            .map(VersionedSignedTransaction::from)
                            .map(TransactionValue::Transaction),
                    )
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        transactions.sort();
        transactions
    }

    /// Get an immutable view of the `World`.
    #[must_use]
    #[inline]
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Returns reference for triggers
    #[inline]
    pub fn triggers(&self) -> &TriggerSet {
        &self.world.triggers
    }

    /// The same as [`Self::modify_triggers_multiple_events`] except closure `f` returns a single `TriggerEvent`.
    ///
    /// # Errors
    /// Forward errors from [`Self::modify_triggers_multiple_events`]
    pub fn modify_triggers<F>(&self, f: F) -> Result<(), Error>
    where
        F: FnOnce(&TriggerSet) -> Result<TriggerEvent, Error>,
    {
        self.modify_triggers_multiple_events(move |triggers| f(triggers).map(std::iter::once))
    }

    /// Get [`TriggerSet`] and pass it to `closure` to modify it
    ///
    /// # Errors
    /// Forward errors from `f`
    pub fn modify_triggers_multiple_events<I, F>(&self, f: F) -> Result<(), Error>
    where
        I: IntoIterator<Item = TriggerEvent>,
        F: FnOnce(&TriggerSet) -> Result<I, Error>,
    {
        self.modify_world_multiple_events(|world| {
            f(&world.triggers).map(|events| events.into_iter().map(WorldEvent::Trigger))
        })
    }

    /// Execute trigger with `trigger_id` as id and `authority` as owner
    ///
    /// Produces [`ExecuteTriggerEvent`].
    ///
    /// Trigger execution time:
    /// - If this method is called by ISI inside *transaction*,
    /// then *trigger* will be executed on the **current** block
    /// - If this method is called by ISI inside *trigger*,
    /// then *trigger* will be executed on the **next** block
    pub fn execute_trigger(&self, trigger_id: TriggerId, authority: &AccountId) {
        let event = ExecuteTriggerEvent {
            trigger_id,
            authority: authority.clone(),
        };

        self.world
            .triggers
            .handle_execute_trigger_event(event.clone());
        self.events_buffer.borrow_mut().push(event.into());
    }

    /// Get constant view to a *Runtime Validator*.
    ///
    /// Performs lazy upgrade of the validator if [`Upgrade`] instruction was executed.
    ///
    /// # Panic
    ///
    /// Panics if validator is not initialized.
    /// Possible only before applying genesis.
    pub fn validator_view(&self) -> crate::validator::View {
        {
            let mut upgraded_validator_write = self.world.upgraded_validator.write();
            if let Some(upgraded_validator) = upgraded_validator_write.take() {
                let mut validator_write = self.world.validator.write();
                validator_write.replace(upgraded_validator);
            }
        }

        #[cfg(test)]
        {
            let mut validator_write = self.world.validator.write();
            if validator_write.is_none() {
                let _validator = validator_write.insert(Validator::default());
            }
        }

        let validator_read = self.world.validator.read();
        crate::validator::View::new(validator_read)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::restriction)]

    use super::*;
    use crate::block::PendingBlock;

    #[test]
    fn get_block_hashes_after_hash() {
        const BLOCK_CNT: usize = 10;

        let mut block = PendingBlock::new_dummy().commit_unchecked();
        let kura = Kura::blank_kura_for_testing();
        let wsv = WorldStateView::new(World::default(), kura);

        let mut block_hashes = vec![];
        for i in 1..=BLOCK_CNT {
            block.header.height = i as u64;
            block.header.previous_block_hash = block_hashes.last().copied();
            let block: VersionedCommittedBlock = block.clone().into();
            block_hashes.push(block.hash());
            wsv.apply(&block).unwrap();
        }

        assert!(wsv
            .block_hashes_after_hash(Some(block_hashes[6]))
            .into_iter()
            .eq(block_hashes.into_iter().skip(7)));
    }

    #[test]
    fn get_blocks_from_height() {
        const BLOCK_CNT: usize = 10;

        let mut block = PendingBlock::new_dummy().commit_unchecked();
        let kura = Kura::blank_kura_for_testing();
        let wsv = WorldStateView::new(World::default(), kura.clone());

        for i in 1..=BLOCK_CNT {
            block.header.height = i as u64;
            let block: VersionedCommittedBlock = block.clone().into();
            wsv.apply(&block).unwrap();
            kura.store_block(block);
        }

        assert_eq!(
            &wsv.all_blocks_by_value()
                .skip(7)
                .map(|block| block.as_v1().header.height)
                .collect::<Vec<_>>(),
            &[8, 9, 10]
        );
    }
}
