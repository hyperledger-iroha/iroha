//! This module contains [`Asset`] structure, it's implementation and related traits and
//! instructions implementations.

use iroha_data_model::{
    asset::definition::ConfidentialPolicyMode,
    fastpq::TransferDeltaTranscript,
    isi::error::{InstructionExecutionError, MathError, Mismatch, TypeError},
    prelude::*,
    query::error::FindError,
};
use iroha_logger::prelude::*;
use iroha_telemetry::metrics;

use super::prelude::*;
/// ISI module contains all instructions related to assets:
/// - minting/burning assets
/// - update metadata
/// - transfer, etc.
pub mod isi {
    use iroha_data_model::{
        events::data::prelude::{AccountEvent, AssetEvent, MetadataChanged},
        isi::{RemoveAssetKeyValue, SetAssetKeyValue, error::MintabilityError},
    };
    use iroha_primitives::numeric::NumericSpec;

    use super::*;
    use crate::{
        smartcontracts::isi::account_admission::ensure_receiving_account, state::WorldTransaction,
    };

    // Use elided lifetimes to avoid single-use lifetime warnings in this inherent impl.
    impl WorldTransaction<'_, '_> {
        /// Decrease a numeric asset balance; removes the asset entry if it reaches zero.
        /// Does not emit events; callers remain responsible for event emission.
        pub(crate) fn withdraw_numeric_asset(
            &mut self,
            id: &AssetId,
            amount: &Numeric,
        ) -> Result<(), Error> {
            ensure_non_negative(amount)?;
            let asset = self
                .assets
                .get_mut(id)
                .ok_or_else(|| FindError::Asset(id.clone().into()))?;
            let quantity: &mut Numeric = &mut *asset;
            ensure_non_negative(quantity)?;
            let current = quantity.clone();
            let candidate = current
                .checked_sub(amount.clone())
                .ok_or(MathError::NotEnoughQuantity)?;
            if candidate.mantissa().is_negative() {
                return Err(MathError::NotEnoughQuantity.into());
            }
            *quantity = candidate;
            if (**asset).is_zero() {
                assert!(self.remove_asset_and_metadata(id).is_some());
            }
            Ok(())
        }

        /// Increase a numeric asset balance, creating it if missing.
        /// Does not emit events; callers remain responsible for event emission.
        pub(crate) fn deposit_numeric_asset(
            &mut self,
            id: &AssetId,
            amount: &Numeric,
        ) -> Result<(), Error> {
            ensure_non_negative(amount)?;
            let dst = self.asset_or_insert(id, Numeric::zero())?;
            let q: &mut Numeric = &mut *dst;
            ensure_non_negative(q)?;
            *q = q
                .clone()
                .checked_add(amount.clone())
                .ok_or(MathError::Overflow)?;
            Ok(())
        }
    }

    /// Assert that `object` matches the provided `asset_spec`.
    pub(crate) fn assert_numeric_spec_with(
        object: &Numeric,
        asset_spec: NumericSpec,
    ) -> Result<NumericSpec, Error> {
        let object_spec = NumericSpec::fractional(object.scale());
        asset_spec.check(object).map_err(|_| {
            TypeError::from(Mismatch {
                expected: asset_spec,
                actual: object_spec,
            })
        })?;
        Ok(asset_spec)
    }

    /// Reject negative numeric values to keep asset balances non-negative.
    pub(super) fn ensure_non_negative(value: &Numeric) -> Result<(), Error> {
        if value.mantissa().is_negative() {
            return Err(MathError::NegativeValue.into());
        }
        Ok(())
    }

    fn ensure_transparent_allowed(
        state_transaction: &StateTransaction<'_, '_>,
        asset_def_id: &AssetDefinitionId,
        violation_message: &'static str,
    ) -> Result<(), Error> {
        let policy_mode = state_transaction
            .world
            .asset_definition(asset_def_id)
            .map_err(Error::from)?
            .confidential_policy()
            .effective_mode(state_transaction.block_height());
        if matches!(policy_mode, ConfidentialPolicyMode::ShieldedOnly) {
            return Err(InstructionExecutionError::InvariantViolation(
                violation_message.into(),
            ));
        }
        Ok(())
    }

    fn apply_transfer_delta(
        state_transaction: &mut StateTransaction<'_, '_>,
        source_id: &AssetId,
        destination_id: &AssetId,
        amount: &Numeric,
    ) -> Result<TransferDeltaTranscript, Error> {
        let spec = state_transaction
            .numeric_spec_for(source_id.definition())
            .map_err(Error::from)?;
        assert_numeric_spec_with(amount, spec)?;
        ensure_non_negative(amount)?;
        ensure_transparent_allowed(
            state_transaction,
            source_id.definition(),
            "transparent transfer not permitted by policy",
        )?;

        let remove_source_asset;
        let from_balance_before;
        let from_balance_after;
        {
            let asset = state_transaction.world.asset_mut(source_id)?;
            let current = asset.clone().into_inner();
            ensure_non_negative(&current)?;
            from_balance_before = current.clone();
            let candidate = current
                .checked_sub(amount.clone())
                .ok_or(MathError::NotEnoughQuantity)?;
            if candidate.mantissa().is_negative() {
                return Err(MathError::NotEnoughQuantity.into());
            }
            from_balance_after = candidate;
            remove_source_asset = from_balance_after.is_zero();
            **asset = from_balance_after.clone();
        }
        if remove_source_asset {
            assert!(
                state_transaction
                    .world
                    .remove_asset_and_metadata(source_id)
                    .is_some()
            );
        }

        let to_balance_before;
        let to_balance_after;
        {
            let dst = state_transaction
                .world
                .asset_or_insert(destination_id, Numeric::zero())?;
            let current = dst.clone().into_inner();
            ensure_non_negative(&current)?;
            to_balance_before = current.clone();
            to_balance_after = current
                .checked_add(amount.clone())
                .ok_or(MathError::Overflow)?;
            ensure_non_negative(&to_balance_after)?;
            **dst = to_balance_after.clone();
        }

        Ok(TransferDeltaTranscript {
            from_account: source_id.account().clone(),
            to_account: destination_id.account().clone(),
            asset_definition: source_id.definition().clone(),
            amount: amount.clone(),
            from_balance_before,
            from_balance_after,
            to_balance_before,
            to_balance_after,
            from_merkle_proof: None,
            to_merkle_proof: None,
        })
    }

    impl Execute for Mint<Numeric, Asset> {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let asset_id = self.destination().clone();

            let amount = self.object().clone();
            let _created = ensure_receiving_account(
                authority,
                asset_id.account(),
                Some((asset_id.definition(), &amount)),
                state_transaction,
            )?;
            let spec = state_transaction
                .numeric_spec_for(asset_id.definition())
                .map_err(Error::from)?;
            assert_numeric_spec_with(&amount, spec)?;
            ensure_transparent_allowed(
                state_transaction,
                asset_id.definition(),
                "transparent mint not permitted by policy",
            )?;

            let flipped = assert_can_mint_cached(state_transaction, asset_id.definition())?;
            // Deposit into destination asset balance, creating if needed
            #[cfg(feature = "telemetry")]
            let amount_f64 = amount.clone().to_f64();
            state_transaction
                .world
                .deposit_numeric_asset(&asset_id, &amount)?;

            #[allow(clippy::float_arithmetic)]
            {
                #[cfg(feature = "telemetry")]
                state_transaction.telemetry.observe_tx_amount(amount_f64);
                state_transaction
                    .world
                    .increase_asset_total_amount(asset_id.definition(), &amount)?;
            }

            state_transaction
                .world
                .emit_events(Some(AssetEvent::Added(AssetChanged {
                    asset: asset_id.clone(),
                    amount: amount.clone(),
                })));

            if flipped {
                state_transaction.world.emit_events([DataEvent::from(
                    AssetDefinitionEvent::MintabilityChangedDetailed(
                        AssetDefinitionMintabilityChanged {
                            asset_definition: asset_id.definition().clone(),
                            minted_amount: amount.clone(),
                            authority: authority.clone(),
                        },
                    ),
                )]);
            }

            Ok(())
        }
    }

    impl Execute for Burn<Numeric, Asset> {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let asset_id = self.destination().clone();

            let spec = state_transaction
                .numeric_spec_for(asset_id.definition())
                .map_err(Error::from)?;
            assert_numeric_spec_with(self.object(), spec)?;

            // Withdraw from source asset balance and remove if it reaches zero
            let amount = self.object().clone();
            state_transaction
                .world
                .withdraw_numeric_asset(&asset_id, &amount)?;

            #[allow(clippy::float_arithmetic)]
            {
                #[cfg(feature = "telemetry")]
                state_transaction
                    .telemetry
                    .observe_tx_amount(amount.clone().to_f64());
                state_transaction
                    .world
                    .decrease_asset_total_amount(asset_id.definition(), &amount)?;
            }

            state_transaction
                .world
                .emit_events(Some(AssetEvent::Removed(AssetChanged {
                    asset: asset_id.clone(),
                    amount,
                })));

            Ok(())
        }
    }

    impl Execute for Transfer<Asset, Numeric, Account> {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let source_id = self.source().clone();
            let destination_id =
                AssetId::new(source_id.definition().clone(), self.destination().clone());
            let amount = self.object().clone();
            let _created = ensure_receiving_account(
                authority,
                destination_id.account(),
                Some((destination_id.definition(), &amount)),
                state_transaction,
            )?;
            let delta =
                apply_transfer_delta(state_transaction, &source_id, &destination_id, &amount)?;
            state_transaction.record_transfer_transcript(authority, delta);

            #[allow(clippy::float_arithmetic)]
            #[cfg(feature = "telemetry")]
            state_transaction
                .telemetry
                .observe_tx_amount(amount.clone().to_f64());

            state_transaction.world.emit_events([
                AssetEvent::Removed(AssetChanged {
                    asset: source_id,
                    amount: amount.clone(),
                }),
                AssetEvent::Added(AssetChanged {
                    asset: destination_id,
                    amount,
                }),
            ]);

            Ok(())
        }
    }

    impl Execute for TransferAssetBatch {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            if self.entries().is_empty() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "transfer asset batch requires at least one entry".into(),
                ));
            }
            let mut deltas = Vec::with_capacity(self.entries().len());
            for entry in self.entries() {
                let source_id =
                    AssetId::new(entry.asset_definition().clone(), entry.from().clone());
                let destination_id =
                    AssetId::new(entry.asset_definition().clone(), entry.to().clone());
                let amount = entry.amount().clone();
                let _created = ensure_receiving_account(
                    authority,
                    destination_id.account(),
                    Some((destination_id.definition(), &amount)),
                    state_transaction,
                )?;
                let delta =
                    apply_transfer_delta(state_transaction, &source_id, &destination_id, &amount)?;
                deltas.push(delta);
                #[allow(clippy::float_arithmetic)]
                #[cfg(feature = "telemetry")]
                state_transaction
                    .telemetry
                    .observe_tx_amount(amount.to_f64());
                state_transaction.world.emit_events([
                    AssetEvent::Removed(AssetChanged {
                        asset: source_id,
                        amount: amount.clone(),
                    }),
                    AssetEvent::Added(AssetChanged {
                        asset: destination_id,
                        amount,
                    }),
                ]);
            }
            state_transaction.record_transfer_transcripts(authority, deltas);
            Ok(())
        }
    }

    impl Execute for SetAssetKeyValue {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let SetAssetKeyValue { asset, key, value } = self;

            crate::smartcontracts::limits::enforce_json_size(
                state_transaction,
                &value,
                "max_metadata_value_bytes",
                crate::smartcontracts::limits::DEFAULT_JSON_LIMIT,
            )?;

            state_transaction
                .world
                .asset_metadata_mut_or_default(&asset)
                .map_err(Error::from)?
                .insert(key.clone(), value.clone());

            state_transaction
                .world
                .emit_events(Some(AccountEvent::Asset(AssetEvent::MetadataInserted(
                    MetadataChanged {
                        target: asset,
                        key,
                        value,
                    },
                ))));

            Ok(())
        }
    }

    impl Execute for RemoveAssetKeyValue {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let RemoveAssetKeyValue { asset, key } = self;

            let removed = state_transaction
                .world
                .remove_asset_metadata_key(&asset, &key)
                .map_err(Error::from)?;

            state_transaction
                .world
                .emit_events(Some(AccountEvent::Asset(AssetEvent::MetadataRemoved(
                    MetadataChanged {
                        target: asset,
                        key,
                        value: removed,
                    },
                ))));

            Ok(())
        }
    }

    /// Assert that this asset is `mintable`.
    // Internal helper retained alongside cached variant; keep for clarity and potential reuse.
    #[allow(dead_code)]
    fn assert_can_mint(
        asset_definition: &AssetDefinition,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<bool, Error> {
        match asset_definition.mintable() {
            Mintable::Infinitely => Ok(false),
            Mintable::Not => Err(Error::Mintability(MintabilityError::MintUnmintable)),
            Mintable::Once | Mintable::Limited(_) => {
                let asset_definition_id = asset_definition.id().clone();
                let asset_definition = state_transaction
                    .world
                    .asset_definition_mut(&asset_definition_id)?;
                asset_definition
                    .consume_mintability()
                    .map_err(Error::Mintability)
            }
        }
    }

    /// Cached variant of `assert_can_mint` using `StateTransaction` caches.
    fn assert_can_mint_cached(
        state_transaction: &mut StateTransaction<'_, '_>,
        def_id: &AssetDefinitionId,
    ) -> Result<bool, Error> {
        let mintable = state_transaction
            .mintable_for(def_id)
            .map_err(Error::from)?;

        match mintable {
            Mintable::Infinitely => Ok(false),
            Mintable::Not => Err(Error::Mintability(MintabilityError::MintUnmintable)),
            Mintable::Once | Mintable::Limited(_) => {
                let def_mut = state_transaction.world.asset_definition_mut(def_id)?;
                let flipped = def_mut.consume_mintability().map_err(Error::Mintability)?;
                let updated = def_mut.mintable();
                state_transaction
                    .mintable_cache
                    .insert(def_id.clone(), updated);
                if let Some((ref id, _)) = state_transaction.last_mintable
                    && id == def_id
                {
                    state_transaction.last_mintable = Some((id.clone(), updated));
                }
                Ok(flipped)
            }
        }
    }
}

/// Asset-related query implementations.
pub mod query {
    use std::collections::BTreeSet;

    use eyre::Result;
    use iroha_data_model::{
        asset::{Asset, AssetDefinition, AssetEntry},
        query::{
            asset::FindAssetById,
            dsl::{CompoundPredicate, EvaluatePredicate},
            error::QueryExecutionFail as Error,
            json::{EqualsCondition, InCondition, PredicateJson},
        },
    };
    use norito::json::Value;

    use super::*;
    use crate::{
        smartcontracts::{ValidQuery, ValidSingularQuery},
        state::StateReadOnly,
    };

    #[derive(Debug, Default, Clone)]
    struct AssetPredicateView {
        accounts: BTreeSet<AccountId>,
        definitions: BTreeSet<AssetDefinitionId>,
        domains: BTreeSet<DomainId>,
    }

    impl AssetPredicateView {
        fn from_predicate(predicate: &CompoundPredicate<Asset>) -> Self {
            let mut view = Self::default();
            let Some(raw) = predicate.json_payload() else {
                return view;
            };
            let Ok(value) = norito::json::from_str(raw) else {
                return view;
            };

            if let Some(parsed) = Self::parse_predicate_value(value) {
                view.ingest_predicate(parsed);
            }

            view
        }

        fn parse_predicate_value(value: Value) -> Option<PredicateJson> {
            PredicateJson::try_from_value(&value).ok().or_else(|| {
                if let Value::Object(map) = value {
                    let mut predicate = PredicateJson::default();
                    for (field, raw_value) in map {
                        match raw_value {
                            Value::String(raw) => {
                                predicate
                                    .equals
                                    .push(EqualsCondition::new(field, Value::String(raw)));
                            }
                            Value::Array(values) if !values.is_empty() => {
                                predicate.r#in.push(InCondition::new(field, values));
                            }
                            _ => {}
                        }
                    }
                    Some(predicate)
                } else {
                    None
                }
            })
        }

        fn ingest_predicate(&mut self, predicate: PredicateJson) {
            for condition in predicate.equals {
                self.push_field_value(&condition.field, &condition.value);
            }
            for membership in predicate.r#in {
                for value in membership.values {
                    self.push_field_value(&membership.field, &value);
                }
            }
        }

        fn push_field_value(&mut self, field: &str, value: &Value) {
            let Some(raw) = Self::value_as_str(value) else {
                return;
            };

            match field {
                "account" | "account_id" | "owner" => {
                    if let Ok(account_id) = raw.parse() {
                        self.accounts.insert(account_id);
                    }
                }
                "definition" | "asset_definition" | "asset_definition_id" | "definition_id" => {
                    if let Ok(definition_id) = raw.parse() {
                        self.definitions.insert(definition_id);
                    }
                }
                "domain" => {
                    if let Ok(domain_id) = raw.parse() {
                        self.domains.insert(domain_id);
                    }
                }
                "id" => {
                    if let Ok(asset_id) = raw.parse::<AssetId>() {
                        self.accounts.insert(asset_id.account().clone());
                        self.definitions.insert(asset_id.definition().clone());
                        self.domains.insert(asset_id.account().domain().clone());
                    }
                }
                _ => {}
            }
        }

        fn value_as_str(value: &Value) -> Option<&str> {
            if let Value::String(raw) = value {
                Some(raw.as_str())
            } else {
                None
            }
        }

        fn plan(&self) -> AssetQueryPlan {
            let domains = self.domains.clone();
            let mut accounts: Vec<_> = if domains.is_empty() {
                self.accounts.iter().cloned().collect()
            } else {
                self.accounts
                    .iter()
                    .filter(|account| domains.contains(account.domain()))
                    .cloned()
                    .collect()
            };
            accounts.sort();

            let mut definitions: Vec<_> = self.definitions.iter().cloned().collect();
            definitions.sort();

            let mut domains: Vec<_> = domains.into_iter().collect();
            domains.sort();

            if !accounts.is_empty() && !definitions.is_empty() {
                let asset_ids = accounts
                    .iter()
                    .flat_map(|account| {
                        definitions
                            .iter()
                            .cloned()
                            .map(|definition| AssetId::new(definition, account.clone()))
                    })
                    .collect();
                return AssetQueryPlan::AssetIds(asset_ids);
            }

            if !accounts.is_empty() {
                return AssetQueryPlan::Accounts {
                    accounts,
                    definitions: None,
                };
            }

            if !domains.is_empty() && !definitions.is_empty() {
                return AssetQueryPlan::Domains {
                    domains,
                    definitions: Some(definitions),
                };
            }

            if !domains.is_empty() {
                return AssetQueryPlan::Domains {
                    domains,
                    definitions: None,
                };
            }

            if !definitions.is_empty() {
                return AssetQueryPlan::Definitions(definitions);
            }

            AssetQueryPlan::Full
        }

        fn matches(&self, asset: &Asset) -> bool {
            if !self.accounts.is_empty() && !self.accounts.contains(asset.id().account()) {
                return false;
            }
            if !self.definitions.is_empty() && !self.definitions.contains(asset.id().definition()) {
                return false;
            }
            if !self.domains.is_empty() && !self.domains.contains(asset.id().account().domain()) {
                return false;
            }
            true
        }
    }

    fn predicate_value_at_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
        if path.is_empty() {
            return None;
        }
        let mut current = value;
        for segment in path.split('.') {
            if segment.is_empty() {
                return None;
            }
            match current {
                Value::Object(map) => {
                    current = map.get(segment)?;
                }
                _ => return None,
            }
        }
        Some(current)
    }

    fn asset_alias_value(asset: &Asset, field: &str) -> Option<String> {
        match field {
            "account" | "account_id" | "owner" | "id.account" => {
                Some(asset.id().account().to_string())
            }
            "definition"
            | "asset_definition"
            | "asset_definition_id"
            | "definition_id"
            | "id.definition" => Some(asset.id().definition().to_string()),
            "domain" | "account.domain" | "id.account.domain" => {
                Some(asset.id().account().domain().to_string())
            }
            _ => None,
        }
    }

    fn predicate_value_equals_str(value: &Value, expected: &str) -> bool {
        matches!(value, Value::String(raw) if raw == expected)
    }

    fn predicate_values_contain_str(values: &[Value], expected: &str) -> bool {
        values
            .iter()
            .any(|value| matches!(value, Value::String(raw) if raw == expected))
    }

    fn predicate_matches_asset(predicate: &PredicateJson, asset: &Asset) -> bool {
        let asset_json = norito::json::to_value(asset).ok();

        for cond in &predicate.equals {
            if let Some(alias) = asset_alias_value(asset, &cond.field) {
                if !predicate_value_equals_str(&cond.value, &alias) {
                    return false;
                }
                continue;
            }
            let Some(value) = asset_json.as_ref() else {
                continue;
            };
            let Some(actual) = predicate_value_at_path(value, &cond.field) else {
                return false;
            };
            if actual != &cond.value {
                return false;
            }
        }

        for cond in &predicate.r#in {
            if let Some(alias) = asset_alias_value(asset, &cond.field) {
                if !predicate_values_contain_str(&cond.values, &alias) {
                    return false;
                }
                continue;
            }
            let Some(value) = asset_json.as_ref() else {
                continue;
            };
            let Some(actual) = predicate_value_at_path(value, &cond.field) else {
                return false;
            };
            if !cond.values.iter().any(|candidate| candidate == actual) {
                return false;
            }
        }

        for field in &predicate.exists {
            if asset_alias_value(asset, field).is_some() {
                continue;
            }
            let Some(value) = asset_json.as_ref() else {
                continue;
            };
            let Some(actual) = predicate_value_at_path(value, field) else {
                return false;
            };
            if actual.is_null() {
                return false;
            }
        }

        true
    }

    #[derive(Debug)]
    enum AssetQueryPlan {
        AssetIds(Vec<AssetId>),
        Accounts {
            accounts: Vec<AccountId>,
            definitions: Option<Vec<AssetDefinitionId>>,
        },
        Domains {
            domains: Vec<DomainId>,
            definitions: Option<Vec<AssetDefinitionId>>,
        },
        Definitions(Vec<AssetDefinitionId>),
        Full,
    }

    impl ValidQuery for FindAssets {
        #[metrics(+"find_assets")]
        fn execute(
            self,
            filter: CompoundPredicate<Asset>,
            state_ro: &impl StateReadOnly,
        ) -> Result<impl Iterator<Item = Asset>, Error> {
            let predicate_view = AssetPredicateView::from_predicate(&filter);
            let predicate_json = filter
                .json_payload()
                .and_then(|raw| norito::json::from_str(raw).ok())
                .and_then(AssetPredicateView::parse_predicate_value);
            let plan = predicate_view.plan();
            let world = state_ro.world();

            let entry_to_asset = |entry: AssetEntry<'_>| Asset {
                id: entry.id().clone(),
                value: entry.value().clone().into_inner(),
            };

            let iter: Box<dyn Iterator<Item = Asset> + '_> = match plan {
                AssetQueryPlan::AssetIds(ids) => {
                    if ids.is_empty() {
                        Box::new(std::iter::empty())
                    } else {
                        Box::new(ids.into_iter().filter_map(|asset_id| {
                            world
                                .assets()
                                .get(&asset_id)
                                .map(|value| Asset::new(asset_id, value.clone().into_inner()))
                        }))
                    }
                }
                AssetQueryPlan::Accounts {
                    accounts,
                    definitions,
                } => {
                    let mut assets = Vec::new();
                    match definitions {
                        Some(definitions) => {
                            for account_id in accounts {
                                for definition in &definitions {
                                    let asset_id =
                                        AssetId::new(definition.clone(), account_id.clone());
                                    if let Some(value) = world.assets().get(&asset_id) {
                                        assets
                                            .push(Asset::new(asset_id, value.clone().into_inner()));
                                    }
                                }
                            }
                        }
                        None => {
                            for account_id in accounts {
                                assets.extend(
                                    world
                                        .assets_in_account_iter(&account_id)
                                        .map(entry_to_asset),
                                );
                            }
                        }
                    }
                    Box::new(assets.into_iter())
                }
                AssetQueryPlan::Domains {
                    domains,
                    definitions,
                } => {
                    let mut assets = Vec::new();
                    match definitions {
                        Some(definitions) => {
                            for domain_id in domains {
                                for account in world.accounts_in_domain_iter(&domain_id) {
                                    let account_id = account.id().clone();
                                    for definition in &definitions {
                                        let asset_id =
                                            AssetId::new(definition.clone(), account_id.clone());
                                        if let Some(value) = world.assets().get(&asset_id) {
                                            assets.push(Asset::new(
                                                asset_id,
                                                value.clone().into_inner(),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        None => {
                            for domain_id in domains {
                                assets.extend(
                                    world.assets_in_domain_iter(&domain_id).map(entry_to_asset),
                                );
                            }
                        }
                    }
                    Box::new(assets.into_iter())
                }
                AssetQueryPlan::Definitions(definitions) => {
                    let mut assets = Vec::new();
                    for account in world.accounts_iter() {
                        let account_id = account.id().clone();
                        for definition in &definitions {
                            let asset_id = AssetId::new(definition.clone(), account_id.clone());
                            if let Some(value) = world.assets().get(&asset_id) {
                                assets.push(Asset::new(asset_id, value.clone().into_inner()));
                            }
                        }
                    }
                    Box::new(assets.into_iter())
                }
                AssetQueryPlan::Full => Box::new(world.assets_iter().map(entry_to_asset)),
            };

            Ok(iter.filter(move |asset| {
                if !predicate_view.matches(asset) {
                    return false;
                }
                if let Some(predicate) = predicate_json.as_ref() {
                    return predicate_matches_asset(predicate, asset);
                }
                filter.applies(asset)
            }))
        }
    }

    impl ValidSingularQuery for FindAssetById {
        #[metrics(+"find_asset_by_id")]
        fn execute(&self, state_ro: &impl StateReadOnly) -> Result<Asset, Error> {
            let entry = state_ro.world().asset(self.asset_id())?;
            Ok(Asset {
                id: entry.id().clone(),
                value: entry.value().clone().into_inner(),
            })
        }
    }
    impl ValidQuery for FindAssetsDefinitions {
        #[metrics(+"find_asset_definitions")]
        fn execute(
            self,
            filter: CompoundPredicate<AssetDefinition>,
            state_ro: &impl StateReadOnly,
        ) -> Result<impl Iterator<Item = AssetDefinition>, Error> {
            Ok(state_ro
                .world()
                .asset_definitions_iter()
                .filter(move |&asset_definition| filter.applies(asset_definition))
                .map(|asset_definition| {
                    // Ensure `total_quantity` reflects current state even if storage was not incrementally updated.
                    let mut ad = asset_definition.clone();
                    match state_ro.world().asset_total_amount(ad.id()) {
                        Ok(total) => {
                            debug!(
                                target: "iroha::state::asset_totals",
                                "FindAssetsDefinitions recomputed total for {} as {}",
                                ad.id(),
                                total
                            );
                            ad.total_quantity = total;
                        }
                        Err(error) => {
                            debug!(
                                target: "iroha::state::asset_totals",
                                "FindAssetsDefinitions failed to recompute total for {}: {}",
                                ad.id(),
                                error
                            );
                        }
                    }
                    ad
                }))
        }
    }

    #[cfg(test)]
    mod tests {
        use iroha_data_model::query::json::{EqualsCondition, PredicateJson};
        use iroha_primitives::{json::Json, numeric::Numeric};
        use iroha_test_samples::{ALICE_ID, BOB_ID};
        use nonzero_ext::nonzero;
        use norito::json::Value;

        use super::*;
        use crate::{
            kura::Kura,
            query::store::LiveQueryStore,
            smartcontracts::{ValidQuery, isi::asset::isi::ensure_non_negative},
            state::{State, World},
        };

        #[test]
        fn find_assets_returns_registered_balances() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
            let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
            let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);
            let asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
            let asset = Asset::new(asset_id.clone(), Numeric::new(13, 0));

            let world = World::with_assets([domain], [account], [asset_def], [asset], []);
            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_store);
            let view = state.view();

            let iter = ValidQuery::execute(FindAssets, CompoundPredicate::PASS, &view)
                .expect("query execution succeeds");
            let assets: Vec<_> = iter.collect();

            assert_eq!(assets.len(), 1, "expected the pre-registered asset");
            let fetched = &assets[0];
            assert_eq!(fetched.id(), &asset_id);
            assert_eq!(*fetched.value(), Numeric::new(13, 0));
        }

        #[test]
        fn find_assets_filters_by_account_predicate() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
            let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
            let bob_account = Account::new(bob_id.clone()).build(&ALICE_ID);
            let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
            let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);
            let alice_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
            let bob_asset_id = AssetId::new(asset_def_id.clone(), bob_id.clone());
            let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(13, 0));
            let bob_asset = Asset::new(bob_asset_id, Numeric::new(7, 0));

            let world = World::with_assets(
                [domain],
                [alice_account, bob_account],
                [asset_def],
                [alice_asset, bob_asset],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_store);
            let view = state.view();

            let mut predicate = PredicateJson::default();
            predicate.equals.push(EqualsCondition::new(
                "account",
                Value::String(ALICE_ID.to_string()),
            ));
            let filter = predicate
                .into_compound::<Asset>()
                .expect("predicate is valid JSON");

            let assets: Vec<_> = ValidQuery::execute(FindAssets, filter, &view)
                .expect("query execution succeeds")
                .collect();

            assert_eq!(assets.len(), 1);
            assert_eq!(assets[0].id(), &alice_asset_id);
            assert_eq!(*assets[0].value(), Numeric::new(13, 0));
        }

        #[test]
        fn transfer_removes_metadata_when_balance_zero() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id).build(&ALICE_ID);
            let alice_account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
            let bob_account = Account::new(BOB_ID.clone()).build(&ALICE_ID);
            let asset_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("asset def id");
            let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&ALICE_ID);
            let alice_asset_id = AssetId::new(asset_def_id, ALICE_ID.clone());
            let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(1, 0));

            let world = World::with_assets(
                [domain],
                [alice_account, bob_account],
                [asset_def],
                [alice_asset],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_store);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();

            let key: Name = "tag".parse().expect("metadata key");
            let value = Json::from(norito::json!("seed"));
            SetAssetKeyValue::new(alice_asset_id.clone(), key, value)
                .execute(&ALICE_ID, &mut stx)
                .expect("set metadata");

            Transfer::asset_numeric(alice_asset_id.clone(), 1_u32, BOB_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("transfer succeeds");

            assert!(stx.world.assets.get(&alice_asset_id).is_none());
            assert!(stx.world.asset_metadata.get(&alice_asset_id).is_none());
        }

        #[test]
        fn find_assets_filters_by_definition_predicate() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
            let accounts = [
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                Account::new(bob_id.clone()).build(&ALICE_ID),
            ];
            let rose_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("rose def id");
            let tulip_def_id: AssetDefinitionId = "tulip#wonderland".parse().expect("tulip def id");
            let definitions = [
                AssetDefinition::numeric(rose_def_id.clone()).build(&ALICE_ID),
                AssetDefinition::numeric(tulip_def_id.clone()).build(&ALICE_ID),
            ];
            let assets = [
                Asset::new(
                    AssetId::new(rose_def_id.clone(), ALICE_ID.clone()),
                    Numeric::new(13, 0),
                ),
                Asset::new(
                    AssetId::new(rose_def_id.clone(), bob_id.clone()),
                    Numeric::new(7, 0),
                ),
                Asset::new(
                    AssetId::new(tulip_def_id, ALICE_ID.clone()),
                    Numeric::new(3, 0),
                ),
            ];

            let world =
                World::with_assets([domain], accounts, definitions, assets, /*nfts*/ []);
            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_store);
            let view = state.view();

            let mut predicate = PredicateJson::default();
            predicate.equals.push(EqualsCondition::new(
                "definition",
                Value::String(rose_def_id.to_string()),
            ));
            let filter = predicate
                .into_compound::<Asset>()
                .expect("predicate is valid JSON");

            let assets: Vec<_> = ValidQuery::execute(FindAssets, filter, &view)
                .expect("query execution succeeds")
                .collect();

            assert_eq!(assets.len(), 2);
            assert!(
                assets
                    .iter()
                    .all(|asset| asset.id().definition() == &rose_def_id)
            );
            let mut ids: Vec<_> = assets.into_iter().map(|asset| asset.id().clone()).collect();
            ids.sort();
            let mut expected = vec![
                AssetId::new(rose_def_id.clone(), ALICE_ID.clone()),
                AssetId::new(rose_def_id, bob_id),
            ];
            expected.sort();
            assert_eq!(ids, expected);
        }

        #[test]
        fn find_assets_filters_by_domain_predicate() {
            let wonderland_id: DomainId = "wonderland".parse().expect("domain id");
            let oasis_id: DomainId = "oasis".parse().expect("domain id");
            let domains = [
                Domain::new(wonderland_id.clone()).build(&ALICE_ID),
                Domain::new(oasis_id.clone()).build(&ALICE_ID),
            ];
            let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
            let (dune_id, _) = iroha_test_samples::gen_account_in("oasis");
            let accounts = [
                Account::new(ALICE_ID.clone()).build(&ALICE_ID),
                Account::new(bob_id.clone()).build(&ALICE_ID),
                Account::new(dune_id.clone()).build(&ALICE_ID),
            ];
            let rose_def_id: AssetDefinitionId = "rose#wonderland".parse().expect("rose def id");
            let spice_def_id: AssetDefinitionId = "spice#oasis".parse().expect("spice def id");
            let definitions = [
                AssetDefinition::numeric(rose_def_id.clone()).build(&ALICE_ID),
                AssetDefinition::numeric(spice_def_id.clone()).build(&ALICE_ID),
            ];
            let assets = [
                Asset::new(
                    AssetId::new(rose_def_id.clone(), ALICE_ID.clone()),
                    Numeric::new(5, 0),
                ),
                Asset::new(
                    AssetId::new(rose_def_id.clone(), bob_id.clone()),
                    Numeric::new(11, 0),
                ),
                Asset::new(
                    AssetId::new(spice_def_id, dune_id.clone()),
                    Numeric::new(42, 0),
                ),
            ];

            let world =
                World::with_assets(domains, accounts, definitions, assets, /*nfts*/ []);
            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_store);
            let view = state.view();

            let mut predicate = PredicateJson::default();
            predicate.equals.push(EqualsCondition::new(
                "domain",
                Value::String(wonderland_id.to_string()),
            ));
            let filter = predicate
                .into_compound::<Asset>()
                .expect("predicate is valid JSON");

            let assets: Vec<_> = ValidQuery::execute(FindAssets, filter, &view)
                .expect("query execution succeeds")
                .collect();

            assert_eq!(assets.len(), 2);
            for asset in &assets {
                assert_eq!(asset.id().account().domain(), &wonderland_id);
            }
            let mut ids: Vec<_> = assets.into_iter().map(|asset| asset.id().clone()).collect();
            ids.sort();
            let mut expected = vec![
                AssetId::new(rose_def_id.clone(), ALICE_ID.clone()),
                AssetId::new(rose_def_id, bob_id),
            ];
            expected.sort();
            assert_eq!(ids, expected);
        }

        #[test]
        fn ensure_non_negative_blocks_negative_amounts() {
            let err = ensure_non_negative(&Numeric::new(-1, 0))
                .expect_err("negative numeric balances must be rejected");
            assert!(matches!(
                err,
                InstructionExecutionError::Math(MathError::NegativeValue)
            ));
        }
    }
}
