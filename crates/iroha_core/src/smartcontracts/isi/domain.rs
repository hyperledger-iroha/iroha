//! This module contains [`Domain`] structure and related implementations and trait implementations.

use eyre::Result;
use iroha_data_model::{account::rekey::AccountRekeyRecord, prelude::*, query::error::FindError};
use iroha_telemetry::metrics;

use super::super::isi::prelude::*;

/// ISI module contains all instructions related to domains:
/// - creating/changing assets
/// - registering/unregistering accounts
/// - update metadata
/// - transfer, etc.
pub mod isi {
    use std::{
        collections::{BTreeSet, btree_map::Entry},
        str::FromStr,
    };

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        ChainId, IntoKeyValue,
        account::{
            AccountController,
            curve::{CurveId, CurveRegistryError},
        },
        isi::error::{InstructionExecutionError, RepetitionError},
        metadata::Metadata,
        name::Name,
        offline::OFFLINE_ASSET_ENABLED_METADATA_KEY,
    };
    use iroha_logger::prelude::*;

    use super::*;
    use crate::state::account_label_is_pii;

    /// Domain-separation tag for deterministic offline escrow derivation.
    const OFFLINE_ESCROW_SEED_LABEL: &str = "iroha.offline.escrow.v1";

    fn asset_definition_offline_enabled(
        metadata: &Metadata,
    ) -> Result<bool, InstructionExecutionError> {
        let key = Name::from_str(OFFLINE_ASSET_ENABLED_METADATA_KEY).map_err(|err| {
            InstructionExecutionError::InvariantViolation(
                format!("invalid metadata key `{OFFLINE_ASSET_ENABLED_METADATA_KEY}`: {err}")
                    .into(),
            )
        })?;
        let Some(value) = metadata.get(&key) else {
            return Ok(false);
        };

        if let Ok(flag) = value.try_into_any::<bool>() {
            return Ok(flag);
        }
        if let Ok(text) = value.try_into_any::<String>() {
            let trimmed = text.trim();
            if trimmed.eq_ignore_ascii_case("true") {
                return Ok(true);
            }
            if trimmed.eq_ignore_ascii_case("false") {
                return Ok(false);
            }
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "metadata entry `{OFFLINE_ASSET_ENABLED_METADATA_KEY}` must be `true` or `false`"
                )
                .into(),
            ));
        }

        Err(InstructionExecutionError::InvariantViolation(
            format!(
                "metadata entry `{OFFLINE_ASSET_ENABLED_METADATA_KEY}` must be a boolean or string"
            )
            .into(),
        ))
    }

    pub(super) fn offline_escrow_account_id(
        chain_id: &ChainId,
        definition_id: &AssetDefinitionId,
    ) -> AccountId {
        let seed_material = format!(
            "{OFFLINE_ESCROW_SEED_LABEL}|{}|{definition_id}",
            chain_id.as_str()
        );
        let seed: [u8; Hash::LENGTH] = Hash::new(seed_material).into();
        let keypair = KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519);
        AccountId::new(definition_id.domain().clone(), keypair.public_key().clone())
    }

    fn ensure_offline_escrow_account(
        asset_definition: &AssetDefinition,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if !asset_definition_offline_enabled(asset_definition.metadata())? {
            return Ok(());
        }

        let definition_id = asset_definition.id();
        let derived = offline_escrow_account_id(state_transaction.chain_id(), definition_id);
        let escrow_account = match state_transaction
            .settlement
            .offline
            .escrow_accounts
            .entry(definition_id.clone())
        {
            Entry::Vacant(entry) => entry.insert(derived.clone()).clone(),
            Entry::Occupied(mut entry) => {
                if entry.get() != &derived {
                    warn!(
                        definition = %definition_id,
                        configured = %entry.get(),
                        derived = %derived,
                        "offline escrow account overridden by deterministic derivation"
                    );
                    entry.insert(derived.clone());
                }
                entry.get().clone()
            }
        };

        ensure_controller_capabilities(
            escrow_account.controller(),
            &state_transaction.crypto.allowed_signing,
            &state_transaction.crypto.allowed_curve_ids,
        )?;

        if state_transaction.world.account(&escrow_account).is_ok() {
            return Ok(());
        }

        let account = Account {
            id: escrow_account.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        };
        let (account_id, account_value) = account.clone().into_key_value();
        state_transaction
            .world
            .accounts
            .insert(account_id, account_value);
        state_transaction
            .world
            .emit_events(Some(DomainEvent::Account(AccountEvent::Created(account))));

        Ok(())
    }

    impl Execute for Register<Account> {
        #[metrics(+"register_account")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let account: Account = self.object().clone().build(authority);
            ensure_controller_capabilities(
                account.controller(),
                &state_transaction.crypto.allowed_signing,
                &state_transaction.crypto.allowed_curve_ids,
            )?;
            let (account_id, account_value) = account.clone().into_key_value();

            if *account_id.domain() == *iroha_genesis::GENESIS_DOMAIN_ID {
                return Err(InstructionExecutionError::InvariantViolation(
                    "Not allowed to register account in genesis domain"
                        .to_owned()
                        .into(),
                ));
            }

            let _domain = state_transaction.world.domain_mut(account_id.domain())?;
            if state_transaction.world.account(&account_id).is_ok() {
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::AccountId(account_id),
                }
                .into());
            }
            if let Some(uaid) = account.uaid() {
                if let Some(existing) = state_transaction.world.uaid_accounts.get(uaid) {
                    return Err(InstructionExecutionError::InvariantViolation(
                        format!("UAID {uaid} already bound to account {existing}").into(),
                    ));
                }
            } else if !account.opaque_ids().is_empty() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "Opaque identifiers require a UAID".to_owned().into(),
                ));
            }

            if let Some(label) = account.label() {
                if account_label_is_pii(label) {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "Account label looks like raw PII; use UAID/opaque identifiers instead"
                            .to_owned()
                            .into(),
                    ));
                }
                if state_transaction.world.account_aliases.get(label).is_some()
                    || state_transaction
                        .world
                        .account_rekey_records
                        .get(label)
                        .is_some()
                {
                    return Err(InstructionExecutionError::InvariantViolation(
                        "Account label already registered".to_owned().into(),
                    ));
                }
            }

            if account.uaid().is_some() {
                let mut seen = BTreeSet::new();
                for opaque in account.opaque_ids() {
                    if !seen.insert(*opaque) {
                        return Err(InstructionExecutionError::InvariantViolation(
                            format!(
                                "Account {account_id} contains duplicate opaque identifier {opaque}"
                            )
                            .into(),
                        ));
                    }
                    if let Some(existing) = state_transaction.world.opaque_uaids.get(opaque) {
                        return Err(InstructionExecutionError::InvariantViolation(
                            format!("Opaque identifier {opaque} already bound to UAID {existing}")
                                .into(),
                        ));
                    }
                }
            }
            state_transaction
                .world
                .accounts
                .insert(account_id.clone(), account_value);

            if let Some(uaid) = account.uaid() {
                state_transaction
                    .world
                    .uaid_accounts
                    .insert(*uaid, account_id.clone());
                state_transaction.rebuild_space_directory_bindings(*uaid);
            }

            if let Some(label) = account.label() {
                if state_transaction
                    .world
                    .account_aliases
                    .insert(label.clone(), account_id.clone())
                    .is_some()
                {
                    state_transaction.world.accounts.remove(account_id.clone());
                    if let Some(uaid) = account.uaid() {
                        state_transaction.world.uaid_accounts.remove(*uaid);
                        state_transaction.rebuild_space_directory_bindings(*uaid);
                    }
                    return Err(InstructionExecutionError::InvariantViolation(
                        "Account label already registered".to_owned().into(),
                    ));
                }
            }

            if let Some(uaid) = account.uaid() {
                for opaque in account.opaque_ids() {
                    if state_transaction
                        .world
                        .opaque_uaids
                        .insert(*opaque, *uaid)
                        .is_some()
                    {
                        state_transaction.world.accounts.remove(account_id.clone());
                        state_transaction.world.uaid_accounts.remove(*uaid);
                        if let Some(label) = account.label() {
                            state_transaction
                                .world
                                .account_aliases
                                .remove(label.clone());
                        }
                        for inserted in account.opaque_ids() {
                            state_transaction.world.opaque_uaids.remove(*inserted);
                        }
                        state_transaction.rebuild_space_directory_bindings(*uaid);
                        return Err(InstructionExecutionError::InvariantViolation(
                            "Opaque identifier already registered".to_owned().into(),
                        ));
                    }
                }
            }

            if let Some(record) = AccountRekeyRecord::from_account(&account)
                && state_transaction
                    .world
                    .account_rekey_records
                    .insert(record.label.clone(), record)
                    .is_some()
            {
                state_transaction.world.accounts.remove(account_id.clone());
                if let Some(uaid) = account.uaid() {
                    state_transaction.world.uaid_accounts.remove(*uaid);
                    state_transaction.rebuild_space_directory_bindings(*uaid);
                }
                if let Some(label) = account.label() {
                    state_transaction
                        .world
                        .account_aliases
                        .remove(label.clone());
                }
                for opaque in account.opaque_ids() {
                    state_transaction.world.opaque_uaids.remove(*opaque);
                }
                return Err(InstructionExecutionError::InvariantViolation(
                    "Account label already registered".to_owned().into(),
                ));
            }

            state_transaction
                .world
                .emit_events(Some(DomainEvent::Account(AccountEvent::Created(account))));

            Ok(())
        }
    }

    impl Execute for Unregister<Account> {
        #[metrics(+"unregister_account")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let account_id = self.object().clone();

            state_transaction
                .world()
                .triggers()
                .inspect_by_action(
                    |action| action.authority() == &account_id,
                    |trigger_id, _| trigger_id.clone(),
                )
                .collect::<Vec<_>>()
                .into_iter()
                .for_each(|trigger_id| {
                    state_transaction
                        .world
                        .triggers
                        .remove(&trigger_id)
                        .then_some(())
                        .expect("should succeed")
                });

            state_transaction
                .world
                .account_permissions
                .remove(account_id.clone());

            state_transaction.world.remove_account_roles(&account_id);

            let remove_assets: Vec<AssetId> = state_transaction
                .world
                .assets_in_account_iter(&account_id)
                .map(|ad| ad.id().clone())
                .collect();
            for asset_id in remove_assets {
                state_transaction.world.remove_asset_and_metadata(&asset_id);
            }

            let remove_nfts: Vec<NftId> = state_transaction
                .world
                .nfts
                .iter()
                .filter(|(_, nft)| nft.owned_by == account_id)
                .map(|(id, _)| id.clone())
                .collect();
            for nft_id in remove_nfts {
                state_transaction.world.nfts.remove(nft_id.clone());
                state_transaction
                    .world
                    .emit_events(Some(DomainEvent::Nft(NftEvent::Deleted(nft_id))));
            }

            let removed = state_transaction.world.accounts.remove(account_id.clone());
            let Some(account_value) = removed else {
                return Err(FindError::Account(account_id).into());
            };

            state_transaction
                .world
                .tx_sequences
                .remove(account_id.clone());

            if let Some(label) = account_value.label().cloned() {
                state_transaction
                    .world
                    .account_rekey_records
                    .remove(label.clone());
                state_transaction.world.account_aliases.remove(label);
            }

            if let Some(uaid) = account_value.uaid().copied() {
                state_transaction.world.uaid_accounts.remove(uaid);
                for opaque in account_value.opaque_ids() {
                    state_transaction.world.opaque_uaids.remove(*opaque);
                }
                state_transaction.rebuild_space_directory_bindings(uaid);
            } else {
                for opaque in account_value.opaque_ids() {
                    state_transaction.world.opaque_uaids.remove(*opaque);
                }
            }

            state_transaction
                .world
                .emit_events(Some(AccountEvent::Deleted(account_id)));

            Ok(())
        }
    }

    impl Execute for Register<AssetDefinition> {
        #[metrics(+"register_asset_definition")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let asset_definition = self.object().clone().build(authority);

            let asset_definition_id = asset_definition.id().clone();
            if state_transaction
                .world
                .asset_definition(&asset_definition_id)
                .is_ok()
            {
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::AssetDefinitionId(asset_definition_id),
                }
                .into());
            }
            let _ = state_transaction
                .world
                .domain(asset_definition_id.domain())?;

            state_transaction
                .world
                .asset_definitions
                .insert(asset_definition_id.clone(), asset_definition.clone());

            ensure_offline_escrow_account(&asset_definition, state_transaction)?;

            state_transaction
                .world
                .emit_events(Some(DomainEvent::AssetDefinition(
                    AssetDefinitionEvent::Created(asset_definition),
                )));

            Ok(())
        }
    }

    impl Execute for Unregister<AssetDefinition> {
        #[metrics(+"unregister_asset_definition")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let asset_definition_id = self.object().clone();

            let mut assets_to_remove = Vec::new();
            assets_to_remove.extend(
                state_transaction
                    .world
                    .assets
                    .iter()
                    .filter(|(asset_id, _)| asset_id.definition() == &asset_definition_id)
                    .map(|(asset_id, _)| asset_id)
                    .cloned(),
            );

            let mut events = Vec::with_capacity(assets_to_remove.len() + 1);
            for asset_id in assets_to_remove {
                if state_transaction
                    .world
                    .remove_asset_and_metadata(&asset_id)
                    .is_none()
                {
                    error!(%asset_id, "asset not found. This is a bug");
                }

                events.push(AssetEvent::Deleted(asset_id).into());
            }

            if state_transaction
                .world
                .asset_definitions
                .remove(asset_definition_id.clone())
                .is_none()
            {
                return Err(FindError::AssetDefinition(asset_definition_id).into());
            }
            let _ = state_transaction
                .world
                .domain(asset_definition_id.domain())?;

            events.push(DataEvent::from(AssetDefinitionEvent::Deleted(
                asset_definition_id,
            )));

            state_transaction.world.emit_events(events);

            Ok(())
        }
    }

    impl Execute for SetKeyValue<AssetDefinition> {
        #[metrics(+"set_key_value_asset_definition")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let SetKeyValue {
                object: asset_definition_id,
                key,
                value,
            } = self;
            crate::smartcontracts::limits::enforce_json_size(
                state_transaction,
                &value,
                "max_metadata_value_bytes",
                crate::smartcontracts::limits::DEFAULT_JSON_LIMIT,
            )?;

            state_transaction
                .world
                .asset_definition_mut(&asset_definition_id)
                .map_err(Error::from)
                .map(|asset_definition| {
                    asset_definition
                        .metadata_mut()
                        .insert(key.clone(), value.clone())
                })?;

            state_transaction
                .world
                .emit_events(Some(AssetDefinitionEvent::MetadataInserted(
                    MetadataChanged {
                        target: asset_definition_id,
                        key,
                        value,
                    },
                )));

            Ok(())
        }
    }

    impl Execute for RemoveKeyValue<AssetDefinition> {
        #[metrics(+"remove_key_value_asset_definition")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let asset_definition_id = self.object().clone();

            let value = state_transaction
                .world
                .asset_definition_mut(&asset_definition_id)
                .and_then(|asset_definition| {
                    asset_definition
                        .metadata_mut()
                        .remove(self.key().as_ref())
                        .ok_or_else(|| FindError::MetadataKey(self.key().clone()))
                })?;

            state_transaction
                .world
                .emit_events(Some(AssetDefinitionEvent::MetadataRemoved(
                    MetadataChanged {
                        target: asset_definition_id,
                        key: self.key().clone(),
                        value,
                    },
                )));

            Ok(())
        }
    }

    impl Execute for SetKeyValue<Domain> {
        #[metrics(+"set_domain_key_value")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let SetKeyValue {
                object: domain_id,
                key,
                value,
            } = self;
            crate::smartcontracts::limits::enforce_json_size(
                state_transaction,
                &value,
                "max_metadata_value_bytes",
                crate::smartcontracts::limits::DEFAULT_JSON_LIMIT,
            )?;

            let domain = state_transaction.world.domain_mut(&domain_id)?;
            domain.metadata_mut().insert(key.clone(), value.clone());

            state_transaction
                .world
                .emit_events(Some(DomainEvent::MetadataInserted(MetadataChanged {
                    target: domain_id,
                    key,
                    value,
                })));

            Ok(())
        }
    }

    // centralized in smartcontracts::limits

    impl Execute for RemoveKeyValue<Domain> {
        #[metrics(+"remove_domain_key_value")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let domain_id = self.object().clone();

            let domain = state_transaction.world.domain_mut(&domain_id)?;
            let value = domain
                .metadata_mut()
                .remove(self.key().as_ref())
                .ok_or_else(|| FindError::MetadataKey(self.key().clone()))?;

            state_transaction
                .world
                .emit_events(Some(DomainEvent::MetadataRemoved(MetadataChanged {
                    target: domain_id,
                    key: self.key().clone(),
                    value,
                })));

            Ok(())
        }
    }

    impl Execute for Transfer<Account, DomainId, Account> {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let Transfer {
                source,
                object,
                destination,
            } = self;

            let _ = state_transaction.world.account(&source)?;
            let _ = state_transaction.world.account(&destination)?;

            let domain = state_transaction.world.domain_mut(&object)?;

            if domain.owned_by() != &source {
                return Err(Error::InvariantViolation(
                    format!("Can't transfer domain {domain} since {source} doesn't own it",).into(),
                ));
            }

            domain.set_owned_by(destination.clone());
            state_transaction
                .world
                .emit_events(Some(DomainEvent::OwnerChanged(DomainOwnerChanged {
                    domain: object,
                    new_owner: destination,
                })));

            Ok(())
        }
    }

    pub(crate) fn ensure_controller_capabilities(
        controller: &AccountController,
        allowed_algorithms: &[Algorithm],
        allowed_curve_ids: &[u8],
    ) -> Result<(), InstructionExecutionError> {
        if let Some(disallowed) = first_disallowed_algorithm(controller, allowed_algorithms) {
            let allowed_summary = if allowed_algorithms.is_empty() {
                "none".to_string()
            } else {
                allowed_algorithms
                    .iter()
                    .copied()
                    .map(Algorithm::as_static_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "account controller uses signing algorithm {disallowed} which is not \
                     permitted by crypto.allowed_signing (allowed: {allowed_summary})"
                )
                .into(),
            ));
        }

        match first_disallowed_curve(controller, allowed_curve_ids) {
            Ok(Some(curve)) => {
                let algo = curve.algorithm();
                let curve_code: u8 = curve.into();
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "account controller uses curve id {curve_code:#04X} ({}) which is not \
                         permitted by crypto.curves.allowed_curve_ids",
                        algo.as_static_str()
                    )
                    .into(),
                ));
            }
            Ok(None) => {}
            Err((algo, err)) => {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "account controller uses signing algorithm {} which is not registered in \
                         the account curve registry: {err}",
                        algo.as_static_str()
                    )
                    .into(),
                ));
            }
        }

        Ok(())
    }

    fn first_disallowed_algorithm(
        controller: &AccountController,
        allowed: &[Algorithm],
    ) -> Option<Algorithm> {
        match controller {
            AccountController::Single(signatory) => {
                algorithm_if_disallowed(signatory.algorithm(), allowed)
            }
            AccountController::Multisig(policy) => policy
                .members()
                .iter()
                .find_map(|member| algorithm_if_disallowed(member.algorithm(), allowed)),
        }
    }

    fn algorithm_if_disallowed(algo: Algorithm, allowed: &[Algorithm]) -> Option<Algorithm> {
        if allowed.contains(&algo) || is_bls_algorithm(algo) {
            None
        } else {
            Some(algo)
        }
    }

    fn first_disallowed_curve(
        controller: &AccountController,
        allowed_curve_ids: &[u8],
    ) -> Result<Option<CurveId>, (Algorithm, CurveRegistryError)> {
        match controller {
            AccountController::Single(signatory) => {
                let algo = signatory.algorithm();
                curve_if_disallowed(algo, allowed_curve_ids).map_err(|err| (algo, err))
            }
            AccountController::Multisig(policy) => {
                for member in policy.members() {
                    let algo = member.algorithm();
                    match curve_if_disallowed(algo, allowed_curve_ids) {
                        Ok(Some(curve)) => return Ok(Some(curve)),
                        Ok(None) => {}
                        Err(err) => return Err((algo, err)),
                    }
                }
                Ok(None)
            }
        }
    }

    fn curve_if_disallowed(
        algo: Algorithm,
        allowed_curve_ids: &[u8],
    ) -> Result<Option<CurveId>, CurveRegistryError> {
        if is_bls_algorithm(algo) {
            // Consensus validators rely on BLS controller keys even when admission is restricted.
            return Ok(None);
        }
        let curve = CurveId::try_from_algorithm(algo)?;
        if allowed_curve_ids.contains(&curve.as_u8()) {
            Ok(None)
        } else {
            Ok(Some(curve))
        }
    }

    fn is_bls_algorithm(algo: Algorithm) -> bool {
        #[cfg(feature = "bls")]
        {
            matches!(algo, Algorithm::BlsNormal | Algorithm::BlsSmall)
        }
        #[cfg(not(feature = "bls"))]
        {
            let _ = algo;
            false
        }
    }
}

/// Implementations for domain queries.
pub mod query {
    use iroha_data_model::{
        domain::Domain,
        query::{
            dsl::{CompoundPredicate, EvaluatePredicate},
            error::QueryExecutionFail,
        },
    };

    use super::*;
    use crate::{smartcontracts::ValidQuery, state::StateReadOnly};

    impl ValidQuery for FindDomains {
        #[metrics(+"find_domains")]
        fn execute(
            self,
            filter: CompoundPredicate<Domain>,
            state_ro: &impl StateReadOnly,
        ) -> std::result::Result<impl Iterator<Item = Domain>, QueryExecutionFail> {
            Ok(state_ro
                .world()
                .domains_iter()
                .filter(move |&v| filter.applies(v))
                .cloned())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        IntoKeyValue,
        account::{NewAccount, OpaqueAccountId, rekey::AccountLabel},
        asset::definition::AssetConfidentialPolicy,
        asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId, Mintable, NewAssetDefinition},
        block::BlockHeader,
        events::data::space_directory::{
            SpaceDirectoryEvent, SpaceDirectoryManifestActivated, SpaceDirectoryManifestRevoked,
        },
        metadata::Metadata,
        name::Name,
        nexus::{AssetPermissionManifest, DataSpaceId, ManifestVersion, UniversalAccountId},
        nft::{Nft, NftId},
        offline::OFFLINE_ASSET_ENABLED_METADATA_KEY,
        prelude::Domain,
    };
    use iroha_primitives::{
        json::Json,
        numeric::{Numeric, NumericSpec},
    };
    use iroha_test_samples::ALICE_ID;
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        kura::Kura,
        nexus::space_directory::{SpaceDirectoryManifestRecord, SpaceDirectoryManifestSet},
        prelude::World,
        query::store::LiveQueryStore,
        state::State,
    };

    fn test_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        State::new_for_testing(World::default(), kura, query)
    }

    fn seed_domain(state: &mut State, domain_id: &DomainId, owner: &AccountId) {
        let domain = Domain {
            id: domain_id.clone(),
            logo: None,
            metadata: Metadata::default(),
            owned_by: owner.clone(),
        };
        state.world.domains.insert(domain_id.clone(), domain);
    }

    fn seed_manifest_record<F>(
        world: &mut World,
        uaid: UniversalAccountId,
        dataspace: DataSpaceId,
        configure: F,
    ) -> Hash
    where
        F: FnOnce(&mut SpaceDirectoryManifestRecord),
    {
        let manifest = AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid,
            dataspace,
            issued_ms: 0,
            activation_epoch: 0,
            expiry_epoch: None,
            entries: Vec::new(),
        };
        let mut record = SpaceDirectoryManifestRecord::new(manifest);
        configure(&mut record);
        let manifest_hash = record.manifest_hash;
        let mut set = SpaceDirectoryManifestSet::default();
        set.upsert(record);
        world.space_directory_manifests.insert(uaid, set);
        manifest_hash
    }

    #[test]
    fn account_label_registration_and_cleanup() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let account_label =
            AccountLabel::new(domain_id.clone(), "primary".parse::<Name>().unwrap());
        let keypair = KeyPair::random();
        let account_id = AccountId::new(domain_id.clone(), keypair.public_key().clone());
        let new_account = Account::new(account_id.clone()).with_label(Some(account_label.clone()));

        // Execute register with label.
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account with label");
        assert!(
            tx.world.account_rekey_records.get(&account_label).is_some(),
            "rekey record should be inserted"
        );
        assert_eq!(
            tx.world.account_aliases.get(&account_label),
            Some(&account_id),
            "alias index should be inserted"
        );

        // Duplicate label should be rejected.
        let second_keypair = KeyPair::random();
        let second_id = AccountId::new(domain_id.clone(), second_keypair.public_key().clone());
        let dup_account = Account::new(second_id).with_label(Some(account_label.clone()));
        let err = Register::account(dup_account).execute(&authority, &mut tx);
        assert!(err.is_err(), "duplicate label must raise error");

        // Unregister removes label mapping.
        Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister account");
        assert!(
            tx.world.accounts.get(&account_id).is_none(),
            "account should be removed from world"
        );
        assert!(
            tx.world.account_rekey_records.get(&account_label).is_none(),
            "label record must be removed on unregister"
        );
        assert!(
            tx.world.account_aliases.get(&account_label).is_none(),
            "alias index must be removed on unregister"
        );
    }

    #[test]
    fn register_account_rejects_phone_like_label() {
        let mut state = test_state();
        let domain_id: DomainId = "label.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let account_label =
            AccountLabel::new(domain_id.clone(), "+819398553445".parse::<Name>().unwrap());
        let keypair = KeyPair::random();
        let account_id = AccountId::new(domain_id, keypair.public_key().clone());
        let new_account = Account::new(account_id).with_label(Some(account_label));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let err = Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect_err("phone-like label should be rejected");
        assert!(
            err.to_string().contains("raw PII"),
            "error should mention raw PII: {err}"
        );
    }

    #[test]
    fn register_account_rejects_opaque_ids_without_uaid() {
        let mut state = test_state();
        let domain_id: DomainId = "opaque.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let account_id = AccountId::new(domain_id, KeyPair::random().public_key().clone());
        let opaque = OpaqueAccountId::from_hash(Hash::new("opaque::missing-uaid"));
        let new_account = NewAccount {
            id: account_id,
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: vec![opaque],
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let err = Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect_err("opaque ids without UAID should be rejected");
        assert!(
            err.to_string()
                .contains("Opaque identifiers require a UAID"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn register_account_rejects_duplicate_opaque_ids() {
        let mut state = test_state();
        let domain_id: DomainId = "opaque.dupes".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let account_id = AccountId::new(domain_id.clone(), KeyPair::random().public_key().clone());
        let uaid = UniversalAccountId::from_hash(Hash::new("uaid::opaque-dupes"));
        let opaque = OpaqueAccountId::from_hash(Hash::new("opaque::dupe"));
        let new_account = NewAccount {
            id: account_id,
            metadata: Metadata::default(),
            label: None,
            uaid: Some(uaid),
            opaque_ids: vec![opaque, opaque],
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let err = Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect_err("duplicate opaque ids should be rejected");
        assert!(
            err.to_string().contains("duplicate opaque identifier"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn register_account_rejects_opaque_id_collisions() {
        let mut state = test_state();
        let domain_id: DomainId = "opaque.collide".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let opaque = OpaqueAccountId::from_hash(Hash::new("opaque::collide"));
        let first_id = AccountId::new(domain_id.clone(), KeyPair::random().public_key().clone());
        let second_id = AccountId::new(domain_id.clone(), KeyPair::random().public_key().clone());
        let first_uaid = UniversalAccountId::from_hash(Hash::new("uaid::opaque-collide-1"));
        let second_uaid = UniversalAccountId::from_hash(Hash::new("uaid::opaque-collide-2"));

        let first_account = NewAccount {
            id: first_id.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: Some(first_uaid),
            opaque_ids: vec![opaque],
        };
        let second_account = NewAccount {
            id: second_id.clone(),
            metadata: Metadata::default(),
            label: None,
            uaid: Some(second_uaid),
            opaque_ids: vec![opaque],
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(first_account)
            .execute(&authority, &mut tx)
            .expect("register first account");

        let err = Register::account(second_account)
            .execute(&authority, &mut tx)
            .expect_err("opaque id collisions should be rejected");
        assert!(
            err.to_string().contains("already bound to UAID"),
            "unexpected error: {err}"
        );
        assert_eq!(
            tx.world.opaque_uaids.get(&opaque),
            Some(&first_uaid),
            "opaque id should remain bound to first UAID"
        );
        assert!(
            tx.world.accounts.get(&second_id).is_none(),
            "colliding account must not be inserted"
        );
    }

    #[test]
    fn register_account_rejects_disallowed_algorithms() {
        let mut state = test_state();
        let domain_id: DomainId = "disallowed.curves".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);
        {
            let mut guard = state.crypto.write();
            let mut cfg = (**guard).clone();
            cfg.allowed_signing = vec![Algorithm::Ed25519];
            cfg.allowed_curve_ids =
                iroha_config::parameters::defaults::crypto::derive_curve_ids_from_algorithms(
                    &cfg.allowed_signing,
                );
            *guard = Arc::new(cfg);
        }

        let secp_pair = KeyPair::random_with_algorithm(Algorithm::Secp256k1);
        let account_id = AccountId::new(domain_id.clone(), secp_pair.public_key().clone());
        let new_account = Account::new(account_id);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let err = Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect_err("registration with disallowed algorithm must fail");
        let err_string = err.to_string();
        assert!(
            err_string.contains("crypto.allowed_signing"),
            "error should reference allowed_signing gating: {err_string}"
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn register_account_allows_bls_even_when_not_in_allowed_signing() {
        let mut state = test_state();
        let domain_id: DomainId = "bls.allowed".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        {
            let mut guard = state.crypto.write();
            let mut cfg = (**guard).clone();
            cfg.allowed_signing = vec![Algorithm::Ed25519];
            cfg.allowed_curve_ids =
                iroha_config::parameters::defaults::crypto::derive_curve_ids_from_algorithms(
                    &cfg.allowed_signing,
                );
            *guard = Arc::new(cfg);
        }

        let bls_pair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let account_id = AccountId::new(domain_id, bls_pair.public_key().clone());
        let new_account = Account::new(account_id);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("BLS controllers should be allowed for consensus accounts");
    }

    #[test]
    fn register_account_rejects_disallowed_curve_ids() {
        let mut state = test_state();
        let domain_id: DomainId = "restricted.curves".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        {
            let mut guard = state.crypto.write();
            let mut cfg = (**guard).clone();
            cfg.allowed_curve_ids.clear();
            *guard = Arc::new(cfg);
        }

        let keypair = KeyPair::random();
        let account_id = AccountId::new(domain_id.clone(), keypair.public_key().clone());
        let new_account = Account::new(account_id);

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let err = Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect_err("registration with disallowed curve ids must fail");
        let err_string = err.to_string();
        assert!(
            err_string.contains("crypto.curves.allowed_curve_ids"),
            "error should reference curve gating: {err_string}"
        );
    }

    #[test]
    fn register_account_updates_space_directory_bindings() {
        let mut state = test_state();
        let domain_id: DomainId = "spaces.bindings".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::register_bindings"));
        let dataspace = DataSpaceId::new(17);
        seed_manifest_record(&mut state.world, uaid, dataspace, |record| {
            record.lifecycle.mark_activated(5);
        });

        let keypair = KeyPair::random();
        let account_id = AccountId::new(domain_id.clone(), keypair.public_key().clone());
        let new_account = NewAccount::new(account_id.clone()).with_uaid(Some(uaid));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account with UAID");
        tx.apply();
        block.commit().unwrap();

        let view = state.view();
        let bindings = view
            .world()
            .uaid_dataspaces()
            .get(&uaid)
            .expect("bindings exist after registration");
        let dataspace_entry = bindings
            .iter()
            .find(|(id, _)| **id == dataspace)
            .expect("dataspace should be present");
        assert!(
            dataspace_entry.1.contains(&account_id),
            "account must be bound to dataspace"
        );
    }

    #[test]
    fn register_account_rejects_duplicate_uaid() {
        let mut state = test_state();
        let domain_id: DomainId = "uaid.duplicates".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::duplicate"));
        let first_keypair = KeyPair::random();
        let first_id = AccountId::new(domain_id.clone(), first_keypair.public_key().clone());
        let first_account = NewAccount::new(first_id.clone()).with_uaid(Some(uaid));

        let second_keypair = KeyPair::random();
        let second_id = AccountId::new(domain_id.clone(), second_keypair.public_key().clone());
        let second_account = NewAccount::new(second_id.clone()).with_uaid(Some(uaid));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(first_account)
            .execute(&authority, &mut tx)
            .expect("register first account");

        let err = Register::account(second_account)
            .execute(&authority, &mut tx)
            .expect_err("duplicate UAID must be rejected");
        let err_string = err.to_string();
        assert!(
            err_string.contains("UAID"),
            "error should reference UAID conflict: {err_string}"
        );
        assert_eq!(
            tx.world.uaid_accounts.get(&uaid),
            Some(&first_id),
            "UAID index must retain the first account"
        );
        assert!(
            tx.world.accounts.get(&second_id).is_none(),
            "duplicate account should not be inserted"
        );

        tx.apply();
        block.commit().expect("commit block");

        let view = state.view();
        assert_eq!(view.world().uaid_accounts().get(&uaid), Some(&first_id));
    }

    #[test]
    fn unregister_account_removes_space_directory_bindings() {
        let mut state = test_state();
        let domain_id: DomainId = "spaces.cleanup".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::unregister_bindings"));
        let dataspace = DataSpaceId::new(21);
        seed_manifest_record(&mut state.world, uaid, dataspace, |record| {
            record.lifecycle.mark_activated(3);
        });

        let keypair = KeyPair::random();
        let account_id = AccountId::new(domain_id.clone(), keypair.public_key().clone());
        let new_account = NewAccount::new(account_id.clone()).with_uaid(Some(uaid));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account");
        Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister account");
        tx.apply();
        block.commit().unwrap();

        let view = state.view();
        assert!(
            view.world().uaid_dataspaces().get(&uaid).is_none(),
            "bindings should be removed after account deletion"
        );
        assert!(
            view.world().uaid_accounts().get(&uaid).is_none(),
            "UAID index should be cleared after account deletion"
        );
    }

    #[test]
    fn unregister_account_removes_owned_nfts_and_asset_metadata() {
        let mut state = test_state();
        let domain_id: DomainId = "cleanup.world".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let other_domain_id: DomainId = "other.world".parse().expect("domain id");
        seed_domain(&mut state, &other_domain_id, &authority);

        let keypair = KeyPair::random();
        let account_id = AccountId::new(domain_id.clone(), keypair.public_key().clone());
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(NewAccount::new(account_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register account");

        let asset_def_id: AssetDefinitionId =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().unwrap());
        Register::asset_definition(AssetDefinition::numeric(asset_def_id.clone()))
            .execute(&authority, &mut tx)
            .expect("register asset definition");
        let asset_id = AssetId::new(asset_def_id.clone(), account_id.clone());
        let asset = Asset::new(asset_id.clone(), Numeric::new(5, 0));
        let (asset_id, asset_value) = asset.into_key_value();
        tx.world.assets.insert(asset_id.clone(), asset_value);

        let key: Name = "tag".parse().unwrap();
        let value = Json::from(norito::json!("owned"));
        let mut metadata = Metadata::default();
        metadata.insert(key, value);
        tx.world.asset_metadata.insert(asset_id.clone(), metadata);

        let nft_id = NftId::new(other_domain_id.clone(), "dragon".parse().unwrap());
        let nft = Nft {
            id: nft_id.clone(),
            content: Metadata::default(),
            owned_by: account_id.clone(),
        };
        let (nft_id, nft_value) = nft.into_key_value();
        tx.world.nfts.insert(nft_id.clone(), nft_value);

        Unregister::account(account_id.clone())
            .execute(&authority, &mut tx)
            .expect("unregister account");

        assert!(
            tx.world.assets.get(&asset_id).is_none(),
            "asset should be removed"
        );
        assert!(
            tx.world.asset_metadata.get(&asset_id).is_none(),
            "asset metadata should be removed with asset"
        );
        assert!(
            tx.world.nfts.get(&nft_id).is_none(),
            "owned NFT should be removed"
        );
    }

    #[test]
    fn space_directory_events_drive_bindings() {
        let mut state = test_state();
        let domain_id: DomainId = "spaces.events".parse().expect("domain id");
        let authority = (*ALICE_ID).clone();
        seed_domain(&mut state, &domain_id, &authority);

        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::events"));
        let dataspace = DataSpaceId::new(33);
        let manifest_hash = seed_manifest_record(&mut state.world, uaid, dataspace, |_| {});

        let keypair = KeyPair::random();
        let account_id = AccountId::new(domain_id.clone(), keypair.public_key().clone());
        let new_account = NewAccount::new(account_id.clone()).with_uaid(Some(uaid));

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::account(new_account)
            .execute(&authority, &mut tx)
            .expect("register account");

        assert!(
            tx.world.uaid_dataspaces.get(&uaid).is_none(),
            "inactive manifest should not bind accounts"
        );

        tx.world
            .emit_events(Some(SpaceDirectoryEvent::ManifestActivated(
                SpaceDirectoryManifestActivated {
                    dataspace,
                    uaid,
                    manifest_hash,
                    activation_epoch: 10,
                    expiry_epoch: None,
                },
            )));

        let bindings = tx
            .world
            .uaid_dataspaces
            .get(&uaid)
            .expect("bindings must exist after activation");
        assert!(
            bindings
                .iter()
                .any(|(id, accounts)| *id == dataspace && accounts.contains(&account_id))
        );
        let manifest_record = tx
            .world
            .space_directory_manifests
            .get(&uaid)
            .and_then(|set| set.get(&dataspace))
            .expect("manifest record present");
        assert_eq!(
            manifest_record.lifecycle.activated_epoch,
            Some(10),
            "activation epoch recorded"
        );

        tx.world
            .emit_events(Some(SpaceDirectoryEvent::ManifestRevoked(
                SpaceDirectoryManifestRevoked {
                    dataspace,
                    uaid,
                    manifest_hash,
                    revoked_epoch: 25,
                    reason: Some("operator request".to_string()),
                },
            )));

        assert!(
            tx.world.uaid_dataspaces.get(&uaid).is_none(),
            "bindings cleared after revocation"
        );
        let manifest_record = tx
            .world
            .space_directory_manifests
            .get(&uaid)
            .and_then(|set| set.get(&dataspace))
            .expect("manifest record still present");
        assert!(
            manifest_record.lifecycle.revocation.is_some(),
            "revocation metadata recorded"
        );
        assert_eq!(
            manifest_record.lifecycle.revocation.as_ref().unwrap().epoch,
            25
        );

        tx.apply();
        block.commit().unwrap();
    }

    #[test]
    fn register_asset_definition_auto_creates_offline_escrow_account() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = "offline".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);

        let asset_name: Name = "usd".parse().expect("asset name");
        let definition_id = AssetDefinitionId::new(domain_id.clone(), asset_name);
        let mut metadata = Metadata::default();
        metadata.insert(
            OFFLINE_ASSET_ENABLED_METADATA_KEY
                .parse()
                .expect("metadata key"),
            Json::new(true),
        );

        let new_definition = NewAssetDefinition {
            id: definition_id.clone(),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition(new_definition)
            .execute(&authority, &mut tx)
            .expect("register asset definition");

        let escrow_account = tx
            .settlement
            .offline
            .escrow_accounts
            .get(&definition_id)
            .expect("escrow mapping created")
            .clone();
        let expected = super::isi::offline_escrow_account_id(tx.chain_id(), &definition_id);
        assert_eq!(escrow_account, expected);
        assert!(
            tx.world.account(&escrow_account).is_ok(),
            "escrow account should exist"
        );
    }

    #[test]
    fn register_asset_definition_without_offline_flag_skips_escrow() {
        let mut state = test_state();
        let authority = (*ALICE_ID).clone();
        let domain_id: DomainId = "offline2".parse().expect("domain id");
        seed_domain(&mut state, &domain_id, &authority);

        let asset_name: Name = "eur".parse().expect("asset name");
        let definition_id = AssetDefinitionId::new(domain_id.clone(), asset_name);
        let new_definition = NewAssetDefinition {
            id: definition_id.clone(),
            spec: NumericSpec::integer(),
            mintable: Mintable::Infinitely,
            logo: None,
            metadata: Metadata::default(),
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Register::asset_definition(new_definition)
            .execute(&authority, &mut tx)
            .expect("register asset definition");

        assert!(
            tx.settlement
                .offline
                .escrow_accounts
                .get(&definition_id)
                .is_none(),
            "escrow mapping should not be created"
        );
    }
}
