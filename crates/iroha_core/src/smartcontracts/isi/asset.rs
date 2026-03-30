//! This module contains [`Asset`] structure, it's implementation and related traits and
//! instructions implementations.

use iroha_data_model::{
    asset::definition::ConfidentialPolicyMode,
    fastpq::TransferDeltaTranscript,
    isi::error::{InstructionExecutionError, MathError, Mismatch, TypeError},
    prelude::*,
    query::error::FindError,
};
use iroha_telemetry::metrics;

use super::prelude::*;
/// ISI module contains all instructions related to assets:
/// - minting/burning assets
/// - update metadata
/// - transfer, etc.
pub mod isi {
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::LazyLock,
    };

    use iroha_data_model::{
        asset::{
            ASSET_ISSUER_USAGE_POLICY_METADATA_KEY, ASSET_TRANSFER_CONTROL_METADATA_KEY,
            AssetIssuerUsagePolicyV1, AssetSubjectBindingV1, AssetTransferControlRecord,
            AssetTransferControlStoreV1, AssetTransferControlWindow, AssetTransferLimit,
            AssetTransferUsageBucket, DOMAIN_ASSET_USAGE_POLICY_METADATA_KEY,
            DomainAssetUsagePolicyV1,
        },
        events::data::prelude::{AccountEvent, AssetEvent, MetadataChanged},
        isi::{
            RemoveAssetKeyValue, SetAssetKeyValue, SetAssetTransferBlacklist,
            SetAssetTransferControl, SetAssetTransferFreeze, error::MintabilityError,
        },
        nexus::{CapabilityRequest, ManifestVerdict},
    };
    use iroha_primitives::numeric::NumericSpec;
    use iroha_primitives::{json::Json, numeric::Numeric};
    use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time as WallClockTime};

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
            let resolved_id = self.resolve_asset_id_for_current_scope(id)?;
            ensure_non_negative(amount)?;
            let asset = self
                .assets
                .get_mut(&resolved_id)
                .ok_or_else(|| FindError::Asset(resolved_id.clone().into()))?;
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
                assert!(self.remove_asset_and_metadata(&resolved_id).is_some());
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
            let resolved_id = self.resolve_asset_id_for_current_scope(id)?;
            ensure_non_negative(amount)?;
            let dst = self.asset_or_insert(&resolved_id, Numeric::zero())?;
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

    fn ensure_not_offline_escrow_source(
        state_transaction: &StateTransaction<'_, '_>,
        source_id: &AssetId,
    ) -> Result<(), Error> {
        if crate::smartcontracts::isi::offline::is_offline_escrow_source_asset(
            state_transaction,
            source_id,
        )? {
            return Err(InstructionExecutionError::InvariantViolation(
                "direct transfer from offline escrow account is not allowed; use offline settlement instructions".into(),
            ));
        }
        Ok(())
    }

    static ASSET_ISSUER_POLICY_KEY: LazyLock<Name> = LazyLock::new(|| {
        ASSET_ISSUER_USAGE_POLICY_METADATA_KEY
            .parse()
            .expect("asset issuer usage policy metadata key must be a valid Name")
    });

    static DOMAIN_ASSET_POLICY_KEY: LazyLock<Name> = LazyLock::new(|| {
        DOMAIN_ASSET_USAGE_POLICY_METADATA_KEY
            .parse()
            .expect("domain asset usage policy metadata key must be a valid Name")
    });

    static ASSET_TRANSFER_CONTROL_KEY: LazyLock<Name> = LazyLock::new(|| {
        ASSET_TRANSFER_CONTROL_METADATA_KEY
            .parse()
            .expect("asset transfer control metadata key must be a valid Name")
    });

    fn load_asset_transfer_control_store_from_account(
        account_id: &AccountId,
        metadata: &Metadata,
    ) -> Result<AssetTransferControlStoreV1, Error> {
        let Some(raw) = metadata.get(&*ASSET_TRANSFER_CONTROL_KEY) else {
            return Ok(AssetTransferControlStoreV1::default());
        };
        raw.try_into_any_norito::<AssetTransferControlStoreV1>()
            .map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "invalid account metadata `{}` on {}: {err}",
                        ASSET_TRANSFER_CONTROL_METADATA_KEY, account_id
                    )
                    .into(),
                )
            })
    }

    fn load_asset_transfer_control_store(
        state_transaction: &StateTransaction<'_, '_>,
        account_id: &AccountId,
    ) -> Result<AssetTransferControlStoreV1, Error> {
        let account = state_transaction.world.account(account_id)?;
        load_asset_transfer_control_store_from_account(account.id(), account.metadata())
    }

    fn persist_asset_transfer_control_store(
        state_transaction: &mut StateTransaction<'_, '_>,
        account_id: &AccountId,
        store: &AssetTransferControlStoreV1,
    ) -> Result<(), Error> {
        let account = state_transaction.world.account_mut(account_id)?;
        if store.controls.is_empty() {
            if let Some(value) = account.remove(&*ASSET_TRANSFER_CONTROL_KEY) {
                state_transaction
                    .world
                    .emit_events(Some(AccountEvent::MetadataRemoved(MetadataChanged {
                        target: account_id.clone(),
                        key: ASSET_TRANSFER_CONTROL_KEY.clone(),
                        value,
                    })));
            }
            return Ok(());
        }

        let value = Json::new(store.clone());
        account.insert(ASSET_TRANSFER_CONTROL_KEY.clone(), value.clone());
        state_transaction
            .world
            .emit_events(Some(AccountEvent::MetadataInserted(MetadataChanged {
                target: account_id.clone(),
                key: ASSET_TRANSFER_CONTROL_KEY.clone(),
                value,
            })));
        Ok(())
    }

    fn ensure_asset_transfer_control_owner(
        state_transaction: &StateTransaction<'_, '_>,
        authority: &AccountId,
        asset_definition_id: &AssetDefinitionId,
    ) -> Result<(), Error> {
        let owner = state_transaction
            .world
            .asset_definition(asset_definition_id)?
            .owned_by()
            .clone();
        if owner == *authority {
            return Ok(());
        }
        Err(InstructionExecutionError::InvariantViolation(
            format!(
                "account {authority} cannot manage transfer controls for asset definition {asset_definition_id}; owner is {owner}"
            )
            .into(),
        ))
    }

    fn canonicalize_asset_transfer_limits(
        limits: Vec<AssetTransferLimit>,
    ) -> Result<Vec<AssetTransferLimit>, Error> {
        let mut by_window = BTreeMap::<AssetTransferControlWindow, Option<Numeric>>::new();
        for limit in limits {
            if let Some(cap) = &limit.cap_amount {
                ensure_non_negative(cap)?;
            }
            by_window.insert(limit.window, limit.cap_amount);
        }
        Ok(by_window
            .into_iter()
            .filter_map(|(window, cap_amount)| {
                cap_amount.map(|cap_amount| AssetTransferLimit {
                    window,
                    cap_amount: Some(cap_amount),
                })
            })
            .collect())
    }

    fn bucket_start_ms(window: AssetTransferControlWindow, now_ms: u64) -> Result<u64, Error> {
        let now = OffsetDateTime::from_unix_timestamp_nanos(i128::from(now_ms) * 1_000_000)
            .map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                    format!("invalid block timestamp for asset transfer controls: {err}").into(),
                )
            })?;
        let date = now.date();
        let start_date = match window {
            AssetTransferControlWindow::Day => date,
            AssetTransferControlWindow::Week => {
                let offset = i64::from(date.weekday().number_days_from_monday());
                date.checked_sub(time::Duration::days(offset))
                    .ok_or_else(|| {
                        InstructionExecutionError::InvariantViolation(
                            "failed to compute UTC week bucket start".into(),
                        )
                    })?
            }
            AssetTransferControlWindow::Month => Date::from_calendar_date(
                date.year(),
                Month::try_from(u8::from(date.month())).map_err(|err| {
                    InstructionExecutionError::InvariantViolation(
                        format!("failed to compute UTC month bucket start: {err}").into(),
                    )
                })?,
                1,
            )
            .map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                    format!("failed to compute UTC month bucket start: {err}").into(),
                )
            })?,
        };
        let start = PrimitiveDateTime::new(start_date, WallClockTime::MIDNIGHT).assume_utc();
        u64::try_from(start.unix_timestamp_nanos() / 1_000_000).map_err(|_| {
            InstructionExecutionError::InvariantViolation(
                "bucket start timestamp exceeds supported range".into(),
            )
        })
    }

    fn active_control_record(
        state_transaction: &StateTransaction<'_, '_>,
        account_id: &AccountId,
        asset_definition_id: &AssetDefinitionId,
    ) -> Result<Option<AssetTransferControlRecord>, Error> {
        let store = load_asset_transfer_control_store(state_transaction, account_id)?;
        Ok(store.find(asset_definition_id).cloned())
    }

    fn update_control_record(
        state_transaction: &mut StateTransaction<'_, '_>,
        account_id: &AccountId,
        record: AssetTransferControlRecord,
    ) -> Result<(), Error> {
        let mut store = load_asset_transfer_control_store(state_transaction, account_id)?;
        if record.is_empty() {
            store.remove(&record.asset_definition_id);
        } else {
            store.upsert(record);
        }
        persist_asset_transfer_control_store(state_transaction, account_id, &store)
    }

    fn prepare_outbound_asset_transfer_control_update(
        state_transaction: &StateTransaction<'_, '_>,
        source_id: &AssetId,
        amount: &Numeric,
    ) -> Result<Option<AssetTransferControlRecord>, Error> {
        let Some(mut record) = active_control_record(
            state_transaction,
            source_id.account(),
            source_id.definition(),
        )?
        else {
            return Ok(None);
        };

        if record.outgoing_frozen {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "outbound transfers for account {} are frozen on asset definition {}",
                    source_id.account(),
                    source_id.definition()
                )
                .into(),
            ));
        }
        if record.blacklisted {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "account {} is blacklisted for outbound transfers on asset definition {}",
                    source_id.account(),
                    source_id.definition()
                )
                .into(),
            ));
        }

        let now_ms = state_transaction.block_unix_timestamp_ms();
        let mut current_usages =
            BTreeMap::<AssetTransferControlWindow, AssetTransferUsageBucket>::new();
        for usage in record.usages.iter().cloned() {
            current_usages.insert(usage.window, usage);
        }

        let mut next_usages = Vec::new();
        for limit in record.limits.iter().filter_map(|limit| {
            limit
                .cap_amount
                .clone()
                .map(|cap_amount| (limit.window, cap_amount))
        }) {
            let (window, cap_amount) = limit;
            let bucket_start = bucket_start_ms(window, now_ms)?;
            let spent_before = current_usages
                .remove(&window)
                .filter(|usage| usage.bucket_start_ms == bucket_start)
                .map(|usage| usage.spent_amount)
                .unwrap_or_else(Numeric::zero);
            let spent_after = spent_before
                .clone()
                .checked_add(amount.clone())
                .ok_or(MathError::Overflow)?;
            if spent_after > cap_amount {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "outbound transfer cap exceeded for {} on {} {} bucket: {} + {} > {}",
                        source_id.account(),
                        source_id.definition(),
                        window.as_str(),
                        spent_before,
                        amount,
                        cap_amount
                    )
                    .into(),
                ));
            }
            next_usages.push(AssetTransferUsageBucket {
                window,
                bucket_start_ms: bucket_start,
                spent_amount: spent_after,
            });
        }

        record.usages = next_usages;
        record.updated_at_ms = Some(now_ms);
        Ok(Some(record))
    }

    fn load_issuer_usage_policy(
        definition: &AssetDefinition,
    ) -> Result<AssetIssuerUsagePolicyV1, Error> {
        let Some(raw) = definition.metadata().get(&*ASSET_ISSUER_POLICY_KEY) else {
            return Ok(AssetIssuerUsagePolicyV1::default());
        };
        raw.try_into_any_norito::<AssetIssuerUsagePolicyV1>()
            .map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "invalid metadata `{}` on asset definition {}: {err}",
                        ASSET_ISSUER_USAGE_POLICY_METADATA_KEY,
                        definition.id()
                    )
                    .into(),
                )
            })
    }

    fn load_domain_usage_policy(domain: &Domain) -> Result<DomainAssetUsagePolicyV1, Error> {
        let Some(raw) = domain.metadata().get(&*DOMAIN_ASSET_POLICY_KEY) else {
            return Ok(DomainAssetUsagePolicyV1::default());
        };
        raw.try_into_any_norito::<DomainAssetUsagePolicyV1>()
            .map_err(|err| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "invalid metadata `{}` on domain {}: {err}",
                        DOMAIN_ASSET_USAGE_POLICY_METADATA_KEY,
                        domain.id()
                    )
                    .into(),
                )
            })
    }

    fn ensure_domain_binding_allows_asset(
        state_transaction: &StateTransaction<'_, '_>,
        definition_id: &AssetDefinitionId,
        subject: &AccountId,
        binding: &AssetSubjectBindingV1,
    ) -> Result<(), Error> {
        if binding.allowed_domains.is_empty() {
            return Ok(());
        }

        let alias_domains: BTreeSet<_> = state_transaction
            .world
            .alias_domains_for_account(subject)
            .into_iter()
            .collect();

        for domain_id in &binding.allowed_domains {
            if !alias_domains.contains(domain_id) {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "asset subject binding requires account {subject} to hold an alias in domain {domain_id}"
                    )
                    .into(),
                ));
            }

            let domain = state_transaction
                .world
                .domain(domain_id)
                .map_err(Error::from)?;
            let domain_policy = load_domain_usage_policy(domain)?;
            if !domain_policy.allows(definition_id) {
                return Err(InstructionExecutionError::InvariantViolation(
                    format!(
                        "domain policy for {domain_id} denies usage of asset definition {definition_id}"
                    )
                    .into(),
                ));
            }
        }

        Ok(())
    }

    fn ensure_dataspace_binding_allows_asset(
        state_transaction: &StateTransaction<'_, '_>,
        definition_id: &AssetDefinitionId,
        subject: &AccountId,
        amount: Option<&Numeric>,
        dataspace: Option<DataSpaceId>,
        binding: &AssetSubjectBindingV1,
    ) -> Result<(), Error> {
        if binding.allowed_dataspaces.is_empty() {
            return Ok(());
        }

        let current_dataspace = dataspace
            .or(state_transaction.current_dataspace_id)
            .or(state_transaction.world.current_dataspace_id)
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "asset subject binding for {subject} requires dataspace context for {definition_id}"
                    )
                    .into(),
                )
            })?;

        if !binding.allows_dataspace(current_dataspace) {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "asset subject binding for {subject} does not allow dataspace {}",
                    current_dataspace.as_u64()
                )
                .into(),
            ));
        }

        let account = state_transaction
            .world
            .account(subject)
            .map_err(Error::from)?;
        let uaid = account.value().uaid().copied().ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                format!(
                    "asset subject binding for {subject} requires a UAID for dataspace policy checks"
                )
                .into(),
            )
        })?;

        let manifest_record = state_transaction
            .world
            .space_directory_manifests
            .get(&uaid)
            .and_then(|set| set.get(&current_dataspace))
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!(
                        "missing Space Directory manifest for UAID {uaid} in dataspace {}",
                        current_dataspace.as_u64()
                    )
                    .into(),
                )
            })?;

        if !manifest_record.is_active() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "inactive Space Directory manifest for UAID {uaid} in dataspace {}",
                    current_dataspace.as_u64()
                )
                .into(),
            ));
        }

        let request = CapabilityRequest::new(
            current_dataspace,
            None,
            None,
            Some(definition_id),
            None,
            amount.cloned(),
            state_transaction.block_height(),
        );
        match manifest_record.manifest.evaluate(&request) {
            ManifestVerdict::Allowed(_) => Ok(()),
            ManifestVerdict::Denied(reason) => Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "dataspace policy denied asset definition {definition_id} for account {subject} in dataspace {}: {reason:?}",
                    current_dataspace.as_u64()
                )
                .into(),
            )),
        }
    }

    fn ensure_subject_usage_policy(
        state_transaction: &StateTransaction<'_, '_>,
        definition_id: &AssetDefinitionId,
        policy: &AssetIssuerUsagePolicyV1,
        subject: &AccountId,
        amount: Option<&Numeric>,
        dataspace: Option<DataSpaceId>,
    ) -> Result<(), Error> {
        let binding = policy.binding_for(subject);
        if policy.require_subject_binding && binding.is_none() {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "asset definition {definition_id} requires explicit subject binding for account {subject}"
                )
                .into(),
            ));
        }
        let Some(binding) = binding else {
            return Ok(());
        };

        ensure_domain_binding_allows_asset(state_transaction, definition_id, subject, binding)?;
        ensure_dataspace_binding_allows_asset(
            state_transaction,
            definition_id,
            subject,
            amount,
            dataspace,
            binding,
        )?;
        Ok(())
    }

    #[allow(single_use_lifetimes)]
    fn ensure_usage_policy_for_accounts<'a>(
        state_transaction: &StateTransaction<'_, '_>,
        definition_id: &AssetDefinitionId,
        participants: impl IntoIterator<Item = (&'a AccountId, Option<DataSpaceId>)>,
        amount: Option<&Numeric>,
    ) -> Result<(), Error> {
        let definition = state_transaction
            .world
            .asset_definition(definition_id)
            .map_err(Error::from)?;
        let policy = load_issuer_usage_policy(&definition)?;
        for (subject, dataspace) in participants {
            ensure_subject_usage_policy(
                state_transaction,
                definition_id,
                &policy,
                subject,
                amount,
                dataspace,
            )?;
        }
        Ok(())
    }

    fn asset_id_dataspace_hint(
        state_transaction: &StateTransaction<'_, '_>,
        asset_id: &AssetId,
    ) -> Option<DataSpaceId> {
        match asset_id.scope() {
            iroha_data_model::asset::AssetBalanceScope::Dataspace(dataspace) => Some(*dataspace),
            iroha_data_model::asset::AssetBalanceScope::Global => state_transaction
                .current_dataspace_id
                .or(state_transaction.world.current_dataspace_id),
        }
    }

    fn apply_transfer_delta(
        state_transaction: &mut StateTransaction<'_, '_>,
        source_id: &AssetId,
        destination_id: &AssetId,
        amount: &Numeric,
    ) -> Result<TransferDeltaTranscript, Error> {
        let source_id = state_transaction
            .world
            .resolve_asset_id_for_current_scope(source_id)?;
        let destination_dataspace = state_transaction
            .world
            .dataspace_for_account(destination_id.account())
            .or(state_transaction.current_dataspace_id)
            .or(state_transaction.world.current_dataspace_id);
        let destination_id = state_transaction
            .world
            .resolve_asset_id_for_scope_hint(destination_id, destination_dataspace)?;
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
        ensure_usage_policy_for_accounts(
            state_transaction,
            source_id.definition(),
            [
                (
                    source_id.account(),
                    asset_id_dataspace_hint(state_transaction, &source_id),
                ),
                (
                    destination_id.account(),
                    asset_id_dataspace_hint(state_transaction, &destination_id),
                ),
            ],
            Some(amount),
        )?;
        ensure_not_offline_escrow_source(state_transaction, &source_id)?;

        let remove_source_asset;
        let from_balance_before;
        let from_balance_after;
        {
            let asset = state_transaction.world.asset_mut(&source_id)?;
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
                    .remove_asset_and_metadata(&source_id)
                    .is_some()
            );
        }

        let to_balance_before;
        let to_balance_after;
        {
            let dst = state_transaction
                .world
                .asset_or_insert_exact(&destination_id, Numeric::zero())?;
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
            let resolved_asset_id = state_transaction
                .world
                .resolve_asset_id_for_current_scope(&asset_id)?;

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
            ensure_usage_policy_for_accounts(
                state_transaction,
                asset_id.definition(),
                [(
                    resolved_asset_id.account(),
                    asset_id_dataspace_hint(state_transaction, &resolved_asset_id),
                )],
                Some(&amount),
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
            let resolved_asset_id = state_transaction
                .world
                .resolve_asset_id_for_current_scope(&asset_id)?;

            let spec = state_transaction
                .numeric_spec_for(asset_id.definition())
                .map_err(Error::from)?;
            assert_numeric_spec_with(self.object(), spec)?;
            ensure_usage_policy_for_accounts(
                state_transaction,
                asset_id.definition(),
                [(
                    resolved_asset_id.account(),
                    asset_id_dataspace_hint(state_transaction, &resolved_asset_id),
                )],
                Some(self.object()),
            )?;

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
            let control_update = prepare_outbound_asset_transfer_control_update(
                state_transaction,
                &source_id,
                &amount,
            )?;
            let _created = ensure_receiving_account(
                authority,
                destination_id.account(),
                Some((destination_id.definition(), &amount)),
                state_transaction,
            )?;
            let delta =
                apply_transfer_delta(state_transaction, &source_id, &destination_id, &amount)?;
            if let Some(record) = control_update {
                update_control_record(state_transaction, source_id.account(), record)?;
            }
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

    impl Execute for SetAssetTransferFreeze {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            ensure_asset_transfer_control_owner(
                state_transaction,
                authority,
                &self.asset_definition_id,
            )?;
            state_transaction.world.account(&self.account_id)?;

            let now_ms = state_transaction.block_unix_timestamp_ms();
            let mut record = active_control_record(
                state_transaction,
                &self.account_id,
                &self.asset_definition_id,
            )?
            .unwrap_or(AssetTransferControlRecord {
                asset_definition_id: self.asset_definition_id.clone(),
                outgoing_frozen: false,
                blacklisted: false,
                limits: Vec::new(),
                usages: Vec::new(),
                updated_at_ms: None,
            });
            record.outgoing_frozen = self.outgoing_frozen;
            record.updated_at_ms = Some(now_ms);
            update_control_record(state_transaction, &self.account_id, record)
        }
    }

    impl Execute for SetAssetTransferBlacklist {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            ensure_asset_transfer_control_owner(
                state_transaction,
                authority,
                &self.asset_definition_id,
            )?;
            state_transaction.world.account(&self.account_id)?;

            let now_ms = state_transaction.block_unix_timestamp_ms();
            let mut record = active_control_record(
                state_transaction,
                &self.account_id,
                &self.asset_definition_id,
            )?
            .unwrap_or(AssetTransferControlRecord {
                asset_definition_id: self.asset_definition_id.clone(),
                outgoing_frozen: false,
                blacklisted: false,
                limits: Vec::new(),
                usages: Vec::new(),
                updated_at_ms: None,
            });
            record.blacklisted = self.blacklisted;
            record.updated_at_ms = Some(now_ms);
            update_control_record(state_transaction, &self.account_id, record)
        }
    }

    impl Execute for SetAssetTransferControl {
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            ensure_asset_transfer_control_owner(
                state_transaction,
                authority,
                &self.asset_definition_id,
            )?;
            state_transaction.world.account(&self.account_id)?;

            let now_ms = state_transaction.block_unix_timestamp_ms();
            let mut record = active_control_record(
                state_transaction,
                &self.account_id,
                &self.asset_definition_id,
            )?
            .unwrap_or(AssetTransferControlRecord {
                asset_definition_id: self.asset_definition_id.clone(),
                outgoing_frozen: false,
                blacklisted: false,
                limits: Vec::new(),
                usages: Vec::new(),
                updated_at_ms: None,
            });

            let next_limits = canonicalize_asset_transfer_limits(self.limits)?;
            let active_windows = next_limits
                .iter()
                .map(|limit| limit.window)
                .collect::<BTreeSet<_>>();
            record.limits = next_limits;
            record
                .usages
                .retain(|usage| active_windows.contains(&usage.window));
            record.updated_at_ms = Some(now_ms);
            update_control_record(state_transaction, &self.account_id, record)
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
                let control_update = prepare_outbound_asset_transfer_control_update(
                    state_transaction,
                    &source_id,
                    &amount,
                )?;
                let _created = ensure_receiving_account(
                    authority,
                    destination_id.account(),
                    Some((destination_id.definition(), &amount)),
                    state_transaction,
                )?;
                let delta =
                    apply_transfer_delta(state_transaction, &source_id, &destination_id, &amount)?;
                if let Some(record) = control_update {
                    update_control_record(state_transaction, source_id.account(), record)?;
                }
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
    use std::{collections::BTreeSet, sync::Arc};

    use eyre::Result;
    use iroha_data_model::{
        asset::{Asset, AssetDefinition, AssetEntry},
        query::{
            asset::{FindAssetById, FindAssetDefinitionById},
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
        subjects: BTreeSet<AccountId>,
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
                "account" | "account_id" | "owner" | "id.account" => {
                    if let Ok(account_id) = AccountId::parse_encoded(raw)
                        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                    {
                        self.subjects.insert(account_id.subject_id());
                    }
                }
                "definition"
                | "asset_definition"
                | "asset_definition_id"
                | "definition_id"
                | "id.definition" => {
                    if let Ok(definition_id) = raw.parse() {
                        self.definitions.insert(definition_id);
                    }
                }
                "domain" | "definition.domain" | "id.definition.domain" => {
                    if let Ok(domain_id) = raw.parse() {
                        self.domains.insert(domain_id);
                    }
                }
                "id" => {
                    if let Ok(asset_id) = raw.parse::<AssetId>() {
                        self.subjects.insert(asset_id.account().subject_id());
                        self.definitions.insert(asset_id.definition().clone());
                        if !asset_id.definition().is_opaque_canonical() {
                            self.domains.insert(asset_id.definition().domain().clone());
                        }
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
            let mut subjects: Vec<_> = self.subjects.iter().cloned().collect();
            subjects.sort();

            let mut definitions: Vec<_> = self.definitions.iter().cloned().collect();
            definitions.sort();

            let mut domains: Vec<_> = self.domains.iter().cloned().collect();
            domains.sort();

            if !self.subjects.is_empty() {
                return AssetQueryPlan::Subjects {
                    subjects,
                    domains: (!domains.is_empty()).then_some(domains),
                    definitions: (!definitions.is_empty()).then_some(definitions),
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
            if !self.subjects.is_empty()
                && !self.subjects.contains(&asset.id().account().subject_id())
            {
                return false;
            }
            if !self.definitions.is_empty() && !self.definitions.contains(asset.id().definition()) {
                return false;
            }
            if !self.domains.is_empty()
                && (asset.id().definition().is_opaque_canonical()
                    || !self.domains.contains(asset.id().definition().domain()))
            {
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
            "domain" | "definition.domain" | "id.definition.domain" => {
                (!asset.id().definition().is_opaque_canonical())
                    .then(|| asset.id().definition().domain().to_string())
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

    enum AssetSimplePath {
        Definitions(Vec<AssetDefinitionId>),
        Domains(Vec<DomainId>),
        Ids(Vec<AssetId>),
    }

    fn parse_asset_simple_values(field: &str, values: &[Value]) -> Option<AssetSimplePath> {
        match field {
            "account" | "account_id" | "owner" | "id.account" => None,
            "definition"
            | "asset_definition"
            | "asset_definition_id"
            | "definition_id"
            | "id.definition" => {
                let definitions = values
                    .iter()
                    .filter_map(|value| {
                        let Value::String(raw) = value else {
                            return None;
                        };
                        AssetDefinitionId::parse_address_literal(raw).ok()
                    })
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                Some(AssetSimplePath::Definitions(definitions))
            }
            "domain" | "definition.domain" | "id.definition.domain" => {
                let domains = values
                    .iter()
                    .filter_map(|value| {
                        let Value::String(raw) = value else {
                            return None;
                        };
                        raw.parse::<DomainId>().ok()
                    })
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                Some(AssetSimplePath::Domains(domains))
            }
            "id" => {
                let ids = values
                    .iter()
                    .filter_map(|value| {
                        let Value::String(raw) = value else {
                            return None;
                        };
                        raw.parse::<AssetId>().ok()
                    })
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                Some(AssetSimplePath::Ids(ids))
            }
            _ => None,
        }
    }

    fn asset_predicate_simple_path(predicate: &PredicateJson) -> Option<AssetSimplePath> {
        if !predicate.exists.is_empty() {
            return None;
        }

        if predicate.r#in.is_empty() && predicate.equals.len() == 1 {
            let cond = &predicate.equals[0];
            return parse_asset_simple_values(&cond.field, std::slice::from_ref(&cond.value));
        }

        if predicate.equals.is_empty() && predicate.r#in.len() == 1 {
            let cond = &predicate.r#in[0];
            return parse_asset_simple_values(&cond.field, &cond.values);
        }

        None
    }

    fn asset_json_value<'a>(cache: &'a mut Option<Value>, asset: &Asset) -> Option<&'a Value> {
        if cache.is_none() {
            *cache = norito::json::to_value(asset).ok();
        }
        cache.as_ref()
    }

    fn selected_asset_count_for_definitions(
        world: &impl WorldReadOnly,
        definitions: &BTreeSet<AssetDefinitionId>,
    ) -> usize {
        definitions
            .iter()
            .map(|definition| {
                world
                    .asset_definition_assets()
                    .get(definition)
                    .map_or(0, BTreeSet::len)
            })
            .sum()
    }

    fn selected_asset_count_for_domains(
        world: &impl WorldReadOnly,
        domains: &BTreeSet<DomainId>,
    ) -> usize {
        domains
            .iter()
            .flat_map(|domain| {
                world
                    .domain_asset_definitions()
                    .get(domain)
                    .into_iter()
                    .flat_map(BTreeSet::iter)
            })
            .map(|definition| {
                world
                    .asset_definition_assets()
                    .get(definition)
                    .map_or(0, BTreeSet::len)
            })
            .sum()
    }

    fn should_scan_assets_directly(
        world: &impl WorldReadOnly,
        selected_asset_count: usize,
    ) -> bool {
        let total_assets = world.assets().len();
        total_assets != 0 && selected_asset_count.saturating_mul(8) >= total_assets
    }

    fn predicate_matches_asset(predicate: &PredicateJson, asset: &Asset) -> bool {
        let mut asset_json = None;

        for cond in &predicate.equals {
            if let Some(alias) = asset_alias_value(asset, &cond.field) {
                if !predicate_value_equals_str(&cond.value, &alias) {
                    return false;
                }
                continue;
            }
            let Some(value) = asset_json_value(&mut asset_json, asset) else {
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
            let Some(value) = asset_json_value(&mut asset_json, asset) else {
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
            let Some(value) = asset_json_value(&mut asset_json, asset) else {
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
        Subjects {
            subjects: Vec<AccountId>,
            domains: Option<Vec<DomainId>>,
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
            fn entry_to_asset(entry: AssetEntry<'_>) -> Asset {
                Asset {
                    id: entry.id().clone(),
                    value: entry.value().clone().into_inner(),
                }
            }

            let world = state_ro.world();
            let filter_payload = filter.json_payload();
            if filter_payload.is_none() {
                let iter: Box<dyn Iterator<Item = Asset> + '_> =
                    Box::new(world.assets_iter().map(entry_to_asset));
                return Ok(iter);
            }

            let predicate_view = AssetPredicateView::from_predicate(&filter);
            let predicate_json = filter_payload
                .and_then(|raw| norito::json::from_str(raw).ok())
                .and_then(AssetPredicateView::parse_predicate_value);
            let plan = predicate_view.plan();
            let simple_path = predicate_json
                .as_ref()
                .and_then(asset_predicate_simple_path);

            if let Some(path) = simple_path {
                let iter: Box<dyn Iterator<Item = Asset> + '_> = match path {
                    AssetSimplePath::Definitions(definitions) => {
                        let definitions = definitions.into_iter().collect::<BTreeSet<_>>();
                        if should_scan_assets_directly(
                            world,
                            selected_asset_count_for_definitions(world, &definitions),
                        ) {
                            Box::new(
                                world
                                    .assets_iter()
                                    .filter(move |entry| {
                                        definitions.contains(entry.id().definition())
                                    })
                                    .map(entry_to_asset),
                            )
                        } else {
                            Box::new(definitions.into_iter().flat_map(move |definition| {
                                world
                                    .assets_by_definition_iter(&definition)
                                    .collect::<Vec<_>>()
                            }))
                        }
                    }
                    AssetSimplePath::Domains(domains) => {
                        let domains = domains.into_iter().collect::<BTreeSet<_>>();
                        if should_scan_assets_directly(
                            world,
                            selected_asset_count_for_domains(world, &domains),
                        ) {
                            Box::new(
                                world
                                    .assets_iter()
                                    .filter(move |entry| {
                                        let definition = entry.id().definition();
                                        !definition.is_opaque_canonical()
                                            && domains.contains(definition.domain())
                                    })
                                    .map(entry_to_asset),
                            )
                        } else {
                            let mut definitions = BTreeSet::<AssetDefinitionId>::new();
                            for domain in &domains {
                                definitions.extend(
                                    world
                                        .domain_asset_definitions()
                                        .get(domain)
                                        .into_iter()
                                        .flat_map(BTreeSet::iter)
                                        .cloned(),
                                );
                            }
                            Box::new(definitions.into_iter().flat_map(move |definition| {
                                world
                                    .assets_by_definition_iter(&definition)
                                    .collect::<Vec<_>>()
                            }))
                        }
                    }
                    AssetSimplePath::Ids(asset_ids) => {
                        Box::new(asset_ids.into_iter().filter_map(move |asset_id| {
                            world
                                .assets()
                                .get_key_value(&asset_id)
                                .map(|(asset_id, value)| Asset {
                                    id: asset_id.clone(),
                                    value: value.clone().into_inner(),
                                })
                        }))
                    }
                };
                return Ok(iter);
            }

            let iter: Box<dyn Iterator<Item = Asset> + '_> = match plan {
                AssetQueryPlan::Subjects {
                    subjects,
                    domains,
                    definitions,
                } => {
                    let subjects = subjects
                        .into_iter()
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>();
                    let domains = domains
                        .map(|domains| Arc::new(domains.into_iter().collect::<BTreeSet<_>>()));
                    let definitions = definitions.map(|definitions| {
                        Arc::new(definitions.into_iter().collect::<BTreeSet<_>>())
                    });
                    if let Some(definitions) = definitions {
                        let definitions = if let Some(domains) = domains.as_ref() {
                            Arc::new(
                                definitions
                                    .iter()
                                    .filter(|definition| {
                                        !definition.is_opaque_canonical()
                                            && domains.contains(definition.domain())
                                    })
                                    .cloned()
                                    .collect::<BTreeSet<_>>(),
                            )
                        } else {
                            definitions
                        };
                        Box::new(subjects.into_iter().flat_map(move |subject| {
                            let mut assets = Vec::new();
                            for definition in definitions.iter() {
                                assets.extend(
                                    world
                                        .assets_in_account_by_definition_iter(&subject, definition)
                                        .map(entry_to_asset),
                                );
                            }
                            assets
                        }))
                    } else {
                        Box::new(subjects.into_iter().flat_map(move |subject| {
                            let domains = domains.clone();
                            world
                                .assets_in_account_iter(&subject)
                                .filter(move |entry| {
                                    let definition = entry.id().definition();
                                    domains.as_ref().is_none_or(|domains| {
                                        !definition.is_opaque_canonical()
                                            && domains.contains(definition.domain())
                                    })
                                })
                                .map(entry_to_asset)
                                .collect::<Vec<_>>()
                        }))
                    }
                }
                AssetQueryPlan::Domains {
                    domains,
                    definitions,
                } => {
                    let domains: BTreeSet<_> = domains.into_iter().collect();
                    let definitions: BTreeSet<_> = match definitions {
                        Some(definitions) => definitions
                            .into_iter()
                            .filter(|definition| {
                                !definition.is_opaque_canonical()
                                    && domains.contains(definition.domain())
                            })
                            .collect(),
                        None => {
                            let mut definitions = BTreeSet::<AssetDefinitionId>::new();
                            for domain in &domains {
                                definitions.extend(
                                    world
                                        .domain_asset_definitions()
                                        .get(domain)
                                        .into_iter()
                                        .flat_map(BTreeSet::iter)
                                        .cloned(),
                                );
                            }
                            definitions
                        }
                    };
                    Box::new(definitions.into_iter().flat_map(move |definition| {
                        world
                            .assets_by_definition_iter(&definition)
                            .collect::<Vec<_>>()
                    }))
                }
                AssetQueryPlan::Definitions(definitions) => {
                    let definitions: BTreeSet<_> = definitions.into_iter().collect();
                    Box::new(definitions.into_iter().flat_map(move |definition| {
                        world
                            .assets_by_definition_iter(&definition)
                            .collect::<Vec<_>>()
                    }))
                }
                AssetQueryPlan::Full => Box::new(world.assets_iter().map(entry_to_asset)),
            };

            let iter: Box<dyn Iterator<Item = Asset> + '_> = Box::new(iter.filter(move |asset| {
                if !predicate_view.matches(asset) {
                    return false;
                }
                if let Some(predicate) = predicate_json.as_ref() {
                    return predicate_matches_asset(predicate, asset);
                }
                filter.applies(asset)
            }));
            Ok(iter)
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

    impl ValidSingularQuery for FindAssetDefinitionById {
        #[metrics(+"find_asset_definition_by_id")]
        fn execute(&self, state_ro: &impl StateReadOnly) -> Result<AssetDefinition, Error> {
            Ok(state_ro
                .world()
                .asset_definition(self.asset_definition_id())?)
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
                .filter_map(move |asset_definition| {
                    let effective = state_ro
                        .world()
                        .asset_definition(asset_definition.id())
                        .ok()?;
                    filter.applies(&effective).then_some(effective)
                }))
        }
    }

    #[cfg(test)]
    mod tests {
        use std::collections::{BTreeMap, BTreeSet};

        use iroha_data_model::account::NewAccount;
        use iroha_data_model::asset::{
            ASSET_ISSUER_USAGE_POLICY_METADATA_KEY, ASSET_TRANSFER_CONTROL_METADATA_KEY,
            AssetIssuerUsagePolicyV1, AssetSubjectBindingV1, AssetTransferControlStoreV1,
            AssetTransferControlWindow, AssetTransferLimit, DOMAIN_ASSET_USAGE_POLICY_METADATA_KEY,
            DomainAssetUsagePolicyV1,
        };
        use iroha_data_model::nexus::{
            Allowance, AllowanceWindow, AssetPermissionManifest, CapabilityScope, DataSpaceId,
            ManifestEffect, ManifestEntry,
        };
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

        fn build_account_in_domain(account_id: &AccountId, domain_id: &DomainId) -> Account {
            Account::new(account_id.clone()).build(account_id)
        }

        fn build_numeric_asset_definition(
            asset_definition_id: &AssetDefinitionId,
            owner: &AccountId,
        ) -> AssetDefinition {
            let __asset_definition_id = asset_definition_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
                .build(owner)
        }

        fn build_asset_transfer_control_test_state(
            source_balance: u32,
        ) -> (State, AssetDefinitionId, AssetId) {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let alice_account = build_account_in_domain(&ALICE_ID, &domain_id);
            let bob_account = build_account_in_domain(&BOB_ID, &domain_id);
            let asset_definition_id: AssetDefinitionId =
                iroha_data_model::asset::AssetDefinitionId::new(
                    "wonderland".parse().unwrap(),
                    "rose".parse().unwrap(),
                );
            let asset_definition = build_numeric_asset_definition(&asset_definition_id, &ALICE_ID);
            let source_asset_id = AssetId::new(asset_definition_id.clone(), ALICE_ID.clone());
            let source_asset = Asset::new(
                source_asset_id.clone(),
                Numeric::new(i128::from(source_balance), 0),
            );

            let world = World::with_assets(
                [domain],
                [alice_account, bob_account],
                [asset_definition],
                [source_asset],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_store);

            (state, asset_definition_id, source_asset_id)
        }

        fn asset_balance_or_zero(
            state_transaction: &crate::state::StateTransaction<'_, '_>,
            asset_id: &AssetId,
        ) -> Numeric {
            state_transaction
                .world
                .assets
                .get(asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero)
        }

        fn load_asset_transfer_control_store(
            state_transaction: &crate::state::StateTransaction<'_, '_>,
            account_id: &AccountId,
        ) -> AssetTransferControlStoreV1 {
            let metadata_key: Name = ASSET_TRANSFER_CONTROL_METADATA_KEY
                .parse()
                .expect("metadata key");
            let account = state_transaction
                .world
                .account(account_id)
                .expect("controlled account exists");
            let raw = account
                .metadata()
                .get(&metadata_key)
                .cloned()
                .expect("asset transfer control metadata stored");
            raw.try_into_any_norito::<AssetTransferControlStoreV1>()
                .expect("stored control metadata decodes")
        }

        #[test]
        fn find_assets_returns_registered_balances() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let account = build_account_in_domain(&ALICE_ID, &domain_id);
            let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let asset_def = {
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&ALICE_ID);
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
            let alice_account = build_account_in_domain(&ALICE_ID, &domain_id);
            let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
            let bob_account = build_account_in_domain(&bob_id, &domain_id);
            let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let asset_def = {
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&ALICE_ID);
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
        fn asset_predicate_view_extracts_alias_fields_for_planner() {
            let account_filter =
                CompoundPredicate::<Asset>::build(|p| p.equals("id.account", ALICE_ID.to_string()));
            let account_view = AssetPredicateView::from_predicate(&account_filter);
            assert!(
                matches!(account_view.plan(), AssetQueryPlan::Subjects { .. }),
                "id.account should seed subject plan"
            );

            let definition_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let definition_filter =
                CompoundPredicate::<Asset>::build(|p| p.equals("id.definition", definition_id));
            let definition_view = AssetPredicateView::from_predicate(&definition_filter);
            assert!(
                matches!(definition_view.plan(), AssetQueryPlan::Definitions(_)),
                "id.definition should seed definition plan"
            );

            let domain_filter =
                CompoundPredicate::<Asset>::build(|p| p.equals("definition.domain", "wonderland"));
            let domain_view = AssetPredicateView::from_predicate(&domain_filter);
            assert!(
                matches!(domain_view.plan(), AssetQueryPlan::Domains { .. }),
                "definition.domain should seed domain plan"
            );

            let id_domain_filter = CompoundPredicate::<Asset>::build(|p| {
                p.equals("id.definition.domain", "wonderland")
            });
            let id_domain_view = AssetPredicateView::from_predicate(&id_domain_filter);
            assert!(
                matches!(id_domain_view.plan(), AssetQueryPlan::Domains { .. }),
                "id.definition.domain should seed domain plan"
            );
        }

        #[test]
        fn find_assets_filters_by_id_account_alias_predicate() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let alice_account = build_account_in_domain(&ALICE_ID, &domain_id);
            let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
            let bob_account = build_account_in_domain(&bob_id, &domain_id);
            let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let asset_def = {
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&ALICE_ID);
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

            let predicate =
                CompoundPredicate::<Asset>::build(|p| p.equals("id.account", ALICE_ID.to_string()));
            let assets: Vec<_> = ValidQuery::execute(FindAssets, predicate, &view)
                .expect("query execution succeeds")
                .collect();

            assert_eq!(assets.len(), 1);
            assert_eq!(assets[0].id(), &alice_asset_id);
            assert_eq!(*assets[0].value(), Numeric::new(13, 0));
        }

        #[test]
        fn find_assets_filters_by_account_and_domain_predicate() {
            let primary_domain_id: DomainId = "wonderland".parse().expect("domain id");
            let secondary_domain_id: DomainId = "redland".parse().expect("domain id");
            let primary_domain = Domain::new(primary_domain_id.clone()).build(&ALICE_ID);
            let secondary_domain = Domain::new(secondary_domain_id.clone()).build(&ALICE_ID);

            let alice_account = build_account_in_domain(&ALICE_ID, &primary_domain_id);
            let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
            let bob_account = build_account_in_domain(&bob_id, &primary_domain_id);

            let primary_asset_def_id: AssetDefinitionId =
                iroha_data_model::asset::AssetDefinitionId::new(
                    primary_domain_id.clone(),
                    "rose".parse().unwrap(),
                );
            let secondary_asset_def_id: AssetDefinitionId =
                iroha_data_model::asset::AssetDefinitionId::new(
                    secondary_domain_id.clone(),
                    "lily".parse().unwrap(),
                );
            let primary_asset_def = {
                let __asset_definition_id = primary_asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&ALICE_ID);
            let secondary_asset_def = {
                let __asset_definition_id = secondary_asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&ALICE_ID);

            let alice_primary_asset_id =
                AssetId::new(primary_asset_def_id.clone(), ALICE_ID.clone());
            let alice_secondary_asset_id =
                AssetId::new(secondary_asset_def_id.clone(), ALICE_ID.clone());
            let bob_primary_asset_id = AssetId::new(primary_asset_def_id, bob_id.clone());
            let alice_primary_asset =
                Asset::new(alice_primary_asset_id.clone(), Numeric::new(13, 0));
            let alice_secondary_asset = Asset::new(alice_secondary_asset_id, Numeric::new(7, 0));
            let bob_primary_asset = Asset::new(bob_primary_asset_id, Numeric::new(5, 0));

            let world = World::with_assets(
                [primary_domain, secondary_domain],
                [alice_account, bob_account],
                [primary_asset_def, secondary_asset_def],
                [
                    alice_primary_asset,
                    alice_secondary_asset,
                    bob_primary_asset,
                ],
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
            predicate.equals.push(EqualsCondition::new(
                "domain",
                Value::String(primary_domain_id.to_string()),
            ));
            let filter = predicate
                .into_compound::<Asset>()
                .expect("predicate is valid JSON");

            let assets: Vec<_> = ValidQuery::execute(FindAssets, filter, &view)
                .expect("query execution succeeds")
                .collect();

            assert_eq!(assets.len(), 1);
            assert_eq!(assets[0].id(), &alice_primary_asset_id);
            assert_eq!(*assets[0].value(), Numeric::new(13, 0));
        }

        #[test]
        fn transfer_removes_metadata_when_balance_zero() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let alice_account = build_account_in_domain(&ALICE_ID, &domain_id);
            let bob_account = build_account_in_domain(&BOB_ID, &domain_id);
            let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let asset_def = {
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&ALICE_ID);
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
        fn asset_transfer_controls_require_asset_owner_authority() {
            let (state, asset_definition_id, _) = build_asset_transfer_control_test_state(10);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();

            let err = SetAssetTransferFreeze::new(
                ALICE_ID.clone(),
                asset_definition_id.clone(),
                true,
                Some("operator freeze".to_owned()),
            )
            .execute(&BOB_ID, &mut stx)
            .expect_err("non-owner must be rejected");
            assert!(
                err.to_string().contains("owner is"),
                "unexpected error: {err}"
            );

            let metadata_key: Name = ASSET_TRANSFER_CONTROL_METADATA_KEY
                .parse()
                .expect("metadata key");
            let account = stx
                .world
                .account(&ALICE_ID)
                .expect("controlled account exists");
            assert!(
                account.metadata().get(&metadata_key).is_none(),
                "rejected control instruction must not persist metadata"
            );
        }

        #[test]
        fn transfer_rejects_when_outbound_asset_is_frozen() {
            let (state, asset_definition_id, source_asset_id) =
                build_asset_transfer_control_test_state(10);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();

            SetAssetTransferFreeze::new(
                ALICE_ID.clone(),
                asset_definition_id.clone(),
                true,
                Some("compliance hold".to_owned()),
            )
            .execute(&ALICE_ID, &mut stx)
            .expect("freeze succeeds");

            let err = Transfer::asset_numeric(source_asset_id.clone(), 1_u32, BOB_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("frozen outbound transfer must be rejected");
            assert!(
                err.to_string().contains("frozen"),
                "unexpected error: {err}"
            );

            let destination_asset_id = AssetId::new(asset_definition_id.clone(), BOB_ID.clone());
            assert_eq!(
                asset_balance_or_zero(&stx, &source_asset_id),
                Numeric::new(10, 0)
            );
            assert_eq!(
                asset_balance_or_zero(&stx, &destination_asset_id),
                Numeric::zero()
            );

            let store = load_asset_transfer_control_store(&stx, &ALICE_ID);
            let record = store
                .find(&asset_definition_id)
                .expect("frozen record stored");
            assert!(record.outgoing_frozen);
            assert!(!record.blacklisted);
            assert!(record.usages.is_empty());
        }

        #[test]
        fn transfer_rejects_when_account_is_blacklisted_for_asset() {
            let (state, asset_definition_id, source_asset_id) =
                build_asset_transfer_control_test_state(10);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();

            SetAssetTransferBlacklist::new(ALICE_ID.clone(), asset_definition_id.clone(), true)
                .execute(&ALICE_ID, &mut stx)
                .expect("blacklist succeeds");

            let err = Transfer::asset_numeric(source_asset_id.clone(), 1_u32, BOB_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("blacklisted outbound transfer must be rejected");
            assert!(
                err.to_string().contains("blacklisted"),
                "unexpected error: {err}"
            );

            let destination_asset_id = AssetId::new(asset_definition_id.clone(), BOB_ID.clone());
            assert_eq!(
                asset_balance_or_zero(&stx, &source_asset_id),
                Numeric::new(10, 0)
            );
            assert_eq!(
                asset_balance_or_zero(&stx, &destination_asset_id),
                Numeric::zero()
            );

            let store = load_asset_transfer_control_store(&stx, &ALICE_ID);
            let record = store
                .find(&asset_definition_id)
                .expect("blacklist record stored");
            assert!(record.blacklisted);
            assert!(!record.outgoing_frozen);
            assert!(record.usages.is_empty());
        }

        #[test]
        fn transfer_allows_exact_cap_and_preserves_usage_on_rejected_overage() {
            let (state, asset_definition_id, source_asset_id) =
                build_asset_transfer_control_test_state(10);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 86_400_000, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();

            SetAssetTransferControl::new(
                ALICE_ID.clone(),
                asset_definition_id.clone(),
                vec![AssetTransferLimit {
                    window: AssetTransferControlWindow::Day,
                    cap_amount: Some(Numeric::new(5, 0)),
                }],
            )
            .execute(&ALICE_ID, &mut stx)
            .expect("limit update succeeds");

            Transfer::asset_numeric(source_asset_id.clone(), 5_u32, BOB_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("exact-cap transfer must succeed");

            let destination_asset_id = AssetId::new(asset_definition_id.clone(), BOB_ID.clone());
            assert_eq!(
                asset_balance_or_zero(&stx, &source_asset_id),
                Numeric::new(5, 0)
            );
            assert_eq!(
                asset_balance_or_zero(&stx, &destination_asset_id),
                Numeric::new(5, 0)
            );

            let store_after_success = load_asset_transfer_control_store(&stx, &ALICE_ID);
            let record_after_success = store_after_success
                .find(&asset_definition_id)
                .expect("limit record stored after successful transfer");
            assert_eq!(record_after_success.limits.len(), 1);
            assert_eq!(record_after_success.usages.len(), 1);
            let usage = &record_after_success.usages[0];
            assert_eq!(usage.window, AssetTransferControlWindow::Day);
            assert_eq!(usage.bucket_start_ms, 86_400_000);
            assert_eq!(usage.spent_amount, Numeric::new(5, 0));

            let err = Transfer::asset_numeric(source_asset_id.clone(), 1_u32, BOB_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("over-cap transfer must be rejected");
            assert!(
                err.to_string().contains("cap exceeded"),
                "unexpected error: {err}"
            );

            assert_eq!(
                asset_balance_or_zero(&stx, &source_asset_id),
                Numeric::new(5, 0)
            );
            assert_eq!(
                asset_balance_or_zero(&stx, &destination_asset_id),
                Numeric::new(5, 0)
            );

            let store_after_rejection = load_asset_transfer_control_store(&stx, &ALICE_ID);
            let record_after_rejection = store_after_rejection
                .find(&asset_definition_id)
                .expect("limit record retained after rejected transfer");
            assert_eq!(record_after_rejection.usages.len(), 1);
            assert_eq!(
                record_after_rejection.usages[0].spent_amount,
                Numeric::new(5, 0)
            );
            assert_eq!(record_after_rejection.usages[0].bucket_start_ms, 86_400_000);
        }

        #[test]
        fn transfer_rejects_configured_offline_escrow_source() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let alice_account = build_account_in_domain(&ALICE_ID, &domain_id);
            let bob_account = build_account_in_domain(&BOB_ID, &domain_id);
            let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let asset_def = {
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&ALICE_ID);
            let alice_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
            let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(10, 0));

            let world = World::with_assets(
                [domain],
                [alice_account, bob_account],
                [asset_def],
                [alice_asset],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let mut state = State::new(world, kura, query_store);
            state
                .settlement
                .offline
                .escrow_accounts
                .insert(asset_def_id.clone(), ALICE_ID.clone());

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();

            let err = Transfer::asset_numeric(alice_asset_id.clone(), 1_u32, BOB_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("generic transfer from escrow source must be rejected");
            assert!(
                err.to_string().contains("offline escrow account"),
                "unexpected error: {err}"
            );

            let source_balance = stx
                .world
                .assets
                .get(&alice_asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(source_balance, Numeric::new(10, 0));

            let destination_asset = AssetId::new(asset_def_id, BOB_ID.clone());
            assert!(
                stx.world.assets.get(&destination_asset).is_none(),
                "destination account must not be credited"
            );
        }

        #[test]
        fn transfer_rejects_metadata_derived_offline_escrow_source() {
            let chain_id: iroha_data_model::ChainId = "testnet".parse().expect("chain id");
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let escrow_account = crate::smartcontracts::isi::domain::isi::offline_escrow_account_id(
                &chain_id,
                &asset_def_id,
            );
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let escrow_account_model = build_account_in_domain(&escrow_account, &domain_id);
            let bob_account = build_account_in_domain(&BOB_ID, &domain_id);
            let mut asset_def = {
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&ALICE_ID);
            asset_def.metadata_mut().insert(
                iroha_data_model::offline::OFFLINE_ASSET_ENABLED_METADATA_KEY
                    .parse()
                    .expect("metadata key"),
                Json::from(norito::json!(true)),
            );
            let escrow_asset_id = AssetId::new(asset_def_id.clone(), escrow_account.clone());
            let escrow_asset = Asset::new(escrow_asset_id.clone(), Numeric::new(10, 0));
            let world = World::with_assets(
                [domain],
                [escrow_account_model, bob_account],
                [asset_def],
                [escrow_asset],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let state = State::new_with_chain(world, kura, query_store, chain_id);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();

            let err = Transfer::asset_numeric(escrow_asset_id.clone(), 1_u32, BOB_ID.clone())
                .execute(&escrow_account, &mut stx)
                .expect_err("metadata-derived escrow source must be rejected");
            assert!(
                err.to_string().contains("offline escrow account"),
                "unexpected error: {err}"
            );

            let source_balance = stx
                .world
                .assets
                .get(&escrow_asset_id)
                .map(|asset| asset.as_ref().clone())
                .unwrap_or_else(Numeric::zero);
            assert_eq!(source_balance, Numeric::new(10, 0));

            let destination_asset = AssetId::new(asset_def_id, BOB_ID.clone());
            assert!(
                stx.world.assets.get(&destination_asset).is_none(),
                "destination account must not be credited"
            );
        }

        #[test]
        fn find_assets_filters_by_definition_predicate() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
            let accounts = [
                build_account_in_domain(&ALICE_ID, &domain_id),
                build_account_in_domain(&bob_id, &domain_id),
            ];
            let rose_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let tulip_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "tulip".parse().unwrap(),
            );
            let definitions = [
                {
                    let __asset_definition_id = rose_def_id.clone();
                    AssetDefinition::numeric(__asset_definition_id.clone())
                        .with_name(__asset_definition_id.name().to_string())
                }
                .build(&ALICE_ID),
                {
                    let __asset_definition_id = tulip_def_id.clone();
                    AssetDefinition::numeric(__asset_definition_id.clone())
                        .with_name(__asset_definition_id.name().to_string())
                }
                .build(&ALICE_ID),
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
        fn find_assets_filters_by_id_definition_alias_predicate() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
            let accounts = [
                build_account_in_domain(&ALICE_ID, &domain_id),
                build_account_in_domain(&bob_id, &domain_id),
            ];
            let rose_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let tulip_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "tulip".parse().unwrap(),
            );
            let definitions = [
                {
                    let __asset_definition_id = rose_def_id.clone();
                    AssetDefinition::numeric(__asset_definition_id.clone())
                        .with_name(__asset_definition_id.name().to_string())
                }
                .build(&ALICE_ID),
                {
                    let __asset_definition_id = tulip_def_id.clone();
                    AssetDefinition::numeric(__asset_definition_id.clone())
                        .with_name(__asset_definition_id.name().to_string())
                }
                .build(&ALICE_ID),
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

            let predicate = CompoundPredicate::<Asset>::build(|p| {
                p.equals("id.definition", rose_def_id.clone())
            });
            let assets: Vec<_> = ValidQuery::execute(FindAssets, predicate, &view)
                .expect("query execution succeeds")
                .collect();

            assert_eq!(assets.len(), 2);
            assert!(
                assets
                    .iter()
                    .all(|asset| asset.id().definition() == &rose_def_id)
            );
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
                build_account_in_domain(&ALICE_ID, &wonderland_id),
                build_account_in_domain(&bob_id, &wonderland_id),
                build_account_in_domain(&dune_id, &oasis_id),
            ];
            let rose_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let spice_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "oasis".parse().unwrap(),
                "spice".parse().unwrap(),
            );
            let definitions = [
                {
                    let __asset_definition_id = rose_def_id.clone();
                    AssetDefinition::numeric(__asset_definition_id.clone())
                        .with_name(__asset_definition_id.name().to_string())
                }
                .build(&ALICE_ID),
                {
                    let __asset_definition_id = spice_def_id.clone();
                    AssetDefinition::numeric(__asset_definition_id.clone())
                        .with_name(__asset_definition_id.name().to_string())
                }
                .build(&ALICE_ID),
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
                assert_eq!(asset.id().definition().domain(), &wonderland_id);
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
        fn find_assets_filters_by_definition_domain_alias_predicate() {
            let wonderland_id: DomainId = "wonderland".parse().expect("domain id");
            let oasis_id: DomainId = "oasis".parse().expect("domain id");
            let domains = [
                Domain::new(wonderland_id.clone()).build(&ALICE_ID),
                Domain::new(oasis_id.clone()).build(&ALICE_ID),
            ];
            let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
            let (dune_id, _) = iroha_test_samples::gen_account_in("oasis");
            let accounts = [
                build_account_in_domain(&ALICE_ID, &wonderland_id),
                build_account_in_domain(&bob_id, &wonderland_id),
                build_account_in_domain(&dune_id, &oasis_id),
            ];
            let rose_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let spice_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "oasis".parse().unwrap(),
                "spice".parse().unwrap(),
            );
            let definitions = [
                {
                    let __asset_definition_id = rose_def_id.clone();
                    AssetDefinition::numeric(__asset_definition_id.clone())
                        .with_name(__asset_definition_id.name().to_string())
                }
                .build(&ALICE_ID),
                {
                    let __asset_definition_id = spice_def_id.clone();
                    AssetDefinition::numeric(__asset_definition_id.clone())
                        .with_name(__asset_definition_id.name().to_string())
                }
                .build(&ALICE_ID),
            ];
            let assets = [
                Asset::new(
                    AssetId::new(rose_def_id.clone(), ALICE_ID.clone()),
                    Numeric::new(5, 0),
                ),
                Asset::new(
                    AssetId::new(rose_def_id, bob_id.clone()),
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

            let predicate =
                CompoundPredicate::<Asset>::build(|p| p.equals("definition.domain", "wonderland"));
            let assets: Vec<_> = ValidQuery::execute(FindAssets, predicate, &view)
                .expect("query execution succeeds")
                .collect();

            assert_eq!(assets.len(), 2);
            assert!(
                assets
                    .iter()
                    .all(|asset| asset.id().definition().domain() == &wonderland_id)
            );
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

        #[test]
        fn mint_restricted_asset_uses_current_dataspace_bucket() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let account = build_account_in_domain(&ALICE_ID, &domain_id);
            let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let asset_def = {
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .with_balance_scope_policy(
                iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted,
            )
            .build(&ALICE_ID);

            let world = World::with([domain], [account], [asset_def]);
            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_store);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            let dsid = DataSpaceId::new(7);
            stx.current_dataspace_id = Some(dsid);
            stx.world.current_dataspace_id = Some(dsid);

            let mint_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
            Mint::asset_numeric(5_u32, mint_id)
                .execute(&ALICE_ID, &mut stx)
                .expect("mint must succeed in dataspace context");

            let scoped_id = AssetId::with_scope(
                asset_def_id.clone(),
                ALICE_ID.clone(),
                iroha_data_model::asset::AssetBalanceScope::Dataspace(dsid),
            );
            assert!(
                stx.world.assets.get(&scoped_id).is_some(),
                "restricted asset must be stored under dataspace scope"
            );
            assert!(
                stx.world
                    .assets
                    .get(&AssetId::new(asset_def_id, ALICE_ID.clone()))
                    .is_none(),
                "global bucket must stay empty for restricted assets"
            );
        }

        #[test]
        fn transfer_restricted_asset_rejects_cross_dataspace_scope() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let alice_account = build_account_in_domain(&ALICE_ID, &domain_id);
            let bob_account = build_account_in_domain(&BOB_ID, &domain_id);
            let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let asset_def = {
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .with_balance_scope_policy(
                iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted,
            )
            .build(&ALICE_ID);
            let source_asset = Asset::new(
                AssetId::with_scope(
                    asset_def_id.clone(),
                    ALICE_ID.clone(),
                    iroha_data_model::asset::AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
                ),
                Numeric::new(10, 0),
            );

            let world = World::with_assets(
                [domain],
                [alice_account, bob_account],
                [asset_def],
                [source_asset],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_store);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            stx.current_dataspace_id = Some(DataSpaceId::new(8));
            stx.world.current_dataspace_id = Some(DataSpaceId::new(8));

            let source = AssetId::with_scope(
                asset_def_id,
                ALICE_ID.clone(),
                iroha_data_model::asset::AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
            );
            let err = Transfer::asset_numeric(source, 1_u32, BOB_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("cross-dataspace transfer must be rejected");
            assert!(
                matches!(err, InstructionExecutionError::InvariantViolation(_)),
                "unexpected error: {err:?}"
            );
        }

        #[test]
        fn transfer_restricted_asset_uses_destination_dataspace_binding_and_policy() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let source_dataspace = DataSpaceId::new(7);
            let destination_dataspace = DataSpaceId::new(11);
            let uaid_alice = iroha_data_model::nexus::UniversalAccountId::from_hash(
                iroha_crypto::Hash::new(b"uaid::alice-destination-scope"),
            );
            let uaid_bob = iroha_data_model::nexus::UniversalAccountId::from_hash(
                iroha_crypto::Hash::new(b"uaid::bob-destination-scope"),
            );

            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let alice_account = NewAccount::new(ALICE_ID.clone())
                .with_uaid(Some(uaid_alice))
                .build(&ALICE_ID);
            let bob_account = NewAccount::new(BOB_ID.clone())
                .with_uaid(Some(uaid_bob))
                .build(&BOB_ID);

            let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let mut asset_def = {
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .with_balance_scope_policy(
                iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted,
            )
            .build(&ALICE_ID);
            let issuer_policy = AssetIssuerUsagePolicyV1 {
                require_subject_binding: true,
                subject_bindings: BTreeMap::from([
                    (
                        ALICE_ID.clone(),
                        AssetSubjectBindingV1 {
                            allowed_domains: BTreeSet::new(),
                            allowed_dataspaces: BTreeSet::from([source_dataspace]),
                        },
                    ),
                    (
                        BOB_ID.clone(),
                        AssetSubjectBindingV1 {
                            allowed_domains: BTreeSet::new(),
                            allowed_dataspaces: BTreeSet::from([destination_dataspace]),
                        },
                    ),
                ]),
            };
            asset_def.metadata_mut().insert(
                ASSET_ISSUER_USAGE_POLICY_METADATA_KEY
                    .parse()
                    .expect("metadata key"),
                Json::new(issuer_policy),
            );

            let source_asset_id = AssetId::with_scope(
                asset_def_id.clone(),
                ALICE_ID.clone(),
                iroha_data_model::asset::AssetBalanceScope::Dataspace(source_dataspace),
            );
            let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(10, 0));

            let mut world = World::with_assets(
                [domain],
                [alice_account, bob_account],
                [asset_def],
                [source_asset],
                [],
            );
            world.uaid_accounts.insert(uaid_alice, ALICE_ID.clone());
            world.uaid_accounts.insert(uaid_bob, BOB_ID.clone());

            let mut alice_bindings =
                crate::nexus::space_directory::UaidDataspaceBindings::default();
            alice_bindings.bind_account(source_dataspace, ALICE_ID.clone());
            world.uaid_dataspaces.insert(uaid_alice, alice_bindings);
            let mut bob_bindings = crate::nexus::space_directory::UaidDataspaceBindings::default();
            bob_bindings.bind_account(destination_dataspace, BOB_ID.clone());
            world.uaid_dataspaces.insert(uaid_bob, bob_bindings);

            let mut alice_manifest_record =
                crate::nexus::space_directory::SpaceDirectoryManifestRecord::new(
                    AssetPermissionManifest {
                        version: iroha_data_model::nexus::ManifestVersion::default(),
                        uaid: uaid_alice,
                        dataspace: source_dataspace,
                        issued_ms: 1,
                        activation_epoch: 0,
                        expiry_epoch: None,
                        entries: vec![ManifestEntry {
                            scope: CapabilityScope {
                                dataspace: Some(source_dataspace),
                                program: None,
                                method: None,
                                asset: Some(asset_def_id.clone()),
                                role: None,
                            },
                            effect: ManifestEffect::Allow(Allowance {
                                max_amount: None,
                                window: AllowanceWindow::PerDay,
                            }),
                            notes: None,
                        }],
                    },
                );
            alice_manifest_record.lifecycle.mark_activated(0);
            let mut bob_manifest_record =
                crate::nexus::space_directory::SpaceDirectoryManifestRecord::new(
                    AssetPermissionManifest {
                        version: iroha_data_model::nexus::ManifestVersion::default(),
                        uaid: uaid_bob,
                        dataspace: destination_dataspace,
                        issued_ms: 1,
                        activation_epoch: 0,
                        expiry_epoch: None,
                        entries: vec![ManifestEntry {
                            scope: CapabilityScope {
                                dataspace: Some(destination_dataspace),
                                program: None,
                                method: None,
                                asset: Some(asset_def_id.clone()),
                                role: None,
                            },
                            effect: ManifestEffect::Allow(Allowance {
                                max_amount: None,
                                window: AllowanceWindow::PerDay,
                            }),
                            notes: None,
                        }],
                    },
                );
            bob_manifest_record.lifecycle.mark_activated(0);
            let mut alice_set = crate::nexus::space_directory::SpaceDirectoryManifestSet::default();
            alice_set.upsert(alice_manifest_record);
            let mut bob_set = crate::nexus::space_directory::SpaceDirectoryManifestSet::default();
            bob_set.upsert(bob_manifest_record);
            world
                .space_directory_manifests
                .insert(uaid_alice, alice_set);
            world.space_directory_manifests.insert(uaid_bob, bob_set);

            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_store);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            stx.current_dataspace_id = Some(source_dataspace);
            stx.world.current_dataspace_id = Some(source_dataspace);

            Transfer::asset_numeric(
                AssetId::new(asset_def_id.clone(), ALICE_ID.clone()),
                1_u32,
                BOB_ID.clone(),
            )
            .execute(&ALICE_ID, &mut stx)
            .expect("transfer should resolve the recipient into its bound dataspace");

            let destination_asset_id = AssetId::with_scope(
                asset_def_id.clone(),
                BOB_ID.clone(),
                iroha_data_model::asset::AssetBalanceScope::Dataspace(destination_dataspace),
            );
            assert_eq!(
                stx.world
                    .asset(&destination_asset_id)
                    .expect("destination asset created in bound dataspace")
                    .value()
                    .clone()
                    .into_inner(),
                Numeric::new(1, 0)
            );

            let wrong_scope_destination = AssetId::with_scope(
                asset_def_id.clone(),
                BOB_ID.clone(),
                iroha_data_model::asset::AssetBalanceScope::Dataspace(source_dataspace),
            );
            assert!(
                stx.world.asset(&wrong_scope_destination).is_err(),
                "destination balance must not be materialized in the source dataspace"
            );

            assert_eq!(
                stx.world
                    .asset(&source_asset_id)
                    .expect("source balance still exists")
                    .value()
                    .clone()
                    .into_inner(),
                Numeric::new(9, 0)
            );
        }

        #[test]
        fn transfer_rejects_when_issuer_policy_requires_binding_for_destination() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let alice_account = build_account_in_domain(&ALICE_ID, &domain_id);
            let bob_account = build_account_in_domain(&BOB_ID, &domain_id);
            let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let mut asset_def = {
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&ALICE_ID);
            let issuer_policy = AssetIssuerUsagePolicyV1 {
                require_subject_binding: true,
                subject_bindings: BTreeMap::from([(
                    ALICE_ID.clone(),
                    AssetSubjectBindingV1::default(),
                )]),
            };
            asset_def.metadata_mut().insert(
                ASSET_ISSUER_USAGE_POLICY_METADATA_KEY
                    .parse()
                    .expect("metadata key"),
                Json::new(issuer_policy),
            );
            let source_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
            let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(10, 0));

            let world = World::with_assets(
                [domain],
                [alice_account, bob_account],
                [asset_def],
                [source_asset],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_store);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();

            let err = Transfer::asset_numeric(source_asset_id, 1_u32, BOB_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("unbound destination must be rejected");
            assert!(
                err.to_string()
                    .contains("requires explicit subject binding"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn transfer_rejects_when_bound_domain_policy_denies_asset() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let mut domain_metadata = Metadata::default();
            let domain_policy = DomainAssetUsagePolicyV1 {
                allowed_assets: BTreeSet::new(),
                denied_assets: BTreeSet::from([asset_def_id.clone()]),
            };
            domain_metadata.insert(
                DOMAIN_ASSET_USAGE_POLICY_METADATA_KEY
                    .parse()
                    .expect("metadata key"),
                Json::new(domain_policy),
            );
            let domain = Domain::new(domain_id.clone())
                .with_metadata(domain_metadata)
                .build(&ALICE_ID);
            let alice_account = build_account_in_domain(&ALICE_ID, &domain_id);
            let bob_account = build_account_in_domain(&BOB_ID, &domain_id);

            let mut asset_def = {
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&ALICE_ID);
            let binding = AssetSubjectBindingV1 {
                allowed_domains: BTreeSet::from([domain_id.clone()]),
                allowed_dataspaces: BTreeSet::new(),
            };
            let issuer_policy = AssetIssuerUsagePolicyV1 {
                require_subject_binding: true,
                subject_bindings: BTreeMap::from([
                    (ALICE_ID.clone(), binding.clone()),
                    (BOB_ID.clone(), binding),
                ]),
            };
            asset_def.metadata_mut().insert(
                ASSET_ISSUER_USAGE_POLICY_METADATA_KEY
                    .parse()
                    .expect("metadata key"),
                Json::new(issuer_policy),
            );

            let source_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
            let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(10, 0));
            let world = World::with_assets(
                [domain],
                [alice_account, bob_account],
                [asset_def],
                [source_asset],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_store);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            let err = Transfer::asset_numeric(source_asset_id, 1_u32, BOB_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("domain deny policy must reject transfer");
            assert!(
                err.to_string().contains("domain policy"),
                "unexpected error: {err}"
            );
        }

        #[test]
        fn transfer_rejects_when_dataspace_manifest_denies_bound_asset() {
            let domain_id: DomainId = "wonderland".parse().expect("domain id");
            let dsid = DataSpaceId::new(7);
            let uaid_alice = iroha_data_model::nexus::UniversalAccountId::from_hash(
                iroha_crypto::Hash::new(b"uaid:alice"),
            );
            let uaid_bob = iroha_data_model::nexus::UniversalAccountId::from_hash(
                iroha_crypto::Hash::new(b"uaid:bob"),
            );
            let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
            let alice_account = NewAccount::new(ALICE_ID.clone())
                .with_uaid(Some(uaid_alice))
                .build(&ALICE_ID);
            let bob_account = NewAccount::new(BOB_ID.clone())
                .with_uaid(Some(uaid_bob))
                .build(&BOB_ID);

            let asset_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
            let mut asset_def = {
                let __asset_definition_id = asset_def_id.clone();
                AssetDefinition::numeric(__asset_definition_id.clone())
                    .with_name(__asset_definition_id.name().to_string())
            }
            .build(&ALICE_ID);
            let binding = AssetSubjectBindingV1 {
                allowed_domains: BTreeSet::new(),
                allowed_dataspaces: BTreeSet::from([dsid]),
            };
            let issuer_policy = AssetIssuerUsagePolicyV1 {
                require_subject_binding: true,
                subject_bindings: BTreeMap::from([
                    (ALICE_ID.clone(), binding.clone()),
                    (BOB_ID.clone(), binding),
                ]),
            };
            asset_def.metadata_mut().insert(
                ASSET_ISSUER_USAGE_POLICY_METADATA_KEY
                    .parse()
                    .expect("metadata key"),
                Json::new(issuer_policy),
            );

            let source_asset_id = AssetId::new(asset_def_id.clone(), ALICE_ID.clone());
            let source_asset = Asset::new(source_asset_id.clone(), Numeric::new(10, 0));
            let world = World::with_assets(
                [domain],
                [alice_account, bob_account],
                [asset_def],
                [source_asset],
                [],
            );
            let kura = Kura::blank_kura_for_testing();
            let query_store = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_store);

            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            stx.current_dataspace_id = Some(dsid);
            stx.world.current_dataspace_id = Some(dsid);

            let mut alice_manifest_record =
                crate::nexus::space_directory::SpaceDirectoryManifestRecord::new(
                    AssetPermissionManifest {
                        version: iroha_data_model::nexus::ManifestVersion::default(),
                        uaid: uaid_alice,
                        dataspace: dsid,
                        issued_ms: 1,
                        activation_epoch: 0,
                        expiry_epoch: None,
                        entries: Vec::new(),
                    },
                );
            alice_manifest_record.lifecycle.mark_activated(0);
            let mut bob_manifest_record =
                crate::nexus::space_directory::SpaceDirectoryManifestRecord::new(
                    AssetPermissionManifest {
                        version: iroha_data_model::nexus::ManifestVersion::default(),
                        uaid: uaid_bob,
                        dataspace: dsid,
                        issued_ms: 1,
                        activation_epoch: 0,
                        expiry_epoch: None,
                        entries: Vec::new(),
                    },
                );
            bob_manifest_record.lifecycle.mark_activated(0);
            let mut alice_set = crate::nexus::space_directory::SpaceDirectoryManifestSet::default();
            alice_set.upsert(alice_manifest_record);
            let mut bob_set = crate::nexus::space_directory::SpaceDirectoryManifestSet::default();
            bob_set.upsert(bob_manifest_record);
            stx.world
                .space_directory_manifests
                .insert(uaid_alice, alice_set);
            stx.world
                .space_directory_manifests
                .insert(uaid_bob, bob_set);

            let err = Transfer::asset_numeric(source_asset_id, 1_u32, BOB_ID.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("manifest without matching allow should deny");
            assert!(
                err.to_string().contains("dataspace policy denied"),
                "unexpected error: {err}"
            );
        }
    }
}
