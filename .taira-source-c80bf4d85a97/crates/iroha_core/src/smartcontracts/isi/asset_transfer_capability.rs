//! Exact bounded asset-transfer capability handlers and queries.

use std::collections::BTreeSet;

use iroha_data_model::{
    asset_transfer_capability::{AssetTransferCapabilityStatusV1, AssetTransferCapabilityV1},
    events::data::asset_transfer_capability::AssetTransferCapabilityEventV1,
    isi::asset_transfer_capability::{
        ExecuteAssetTransferCapabilityV1, RegisterAssetTransferCapabilityV1,
        RevokeAssetTransferCapabilityV1,
    },
    prelude::*,
    query::{
        asset_transfer_capability::prelude::{
            FindAssetTransferCapabilitiesByDelegateV1, FindAssetTransferCapabilitiesByGrantorV1,
            FindAssetTransferCapabilitiesBySourceV1, FindAssetTransferCapabilitiesByStatusV1,
            FindAssetTransferCapabilitiesV1, FindAssetTransferCapabilityByIdV1,
        },
        dsl::{CompoundPredicate, EvaluatePredicate},
        error::{FindError, QueryExecutionFail},
    },
};
use mv::storage::StorageReadOnly;

use super::{Error, Execute, asset::isi::execute_user_numeric_asset_transfer};
use crate::{
    prelude::ValidSingularQuery,
    smartcontracts::ValidQuery,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};

fn validation_err(message: impl Into<String>) -> Error {
    iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(
        message.into().into(),
    )
}

fn ensure_account_exists(
    world: &impl WorldReadOnly,
    account: &AccountId,
    role: &str,
) -> Result<(), Error> {
    world.account(account).map(|_| ()).map_err(|_| {
        validation_err(format!(
            "asset transfer capability {role} account not found"
        ))
    })
}

fn ensure_contract_scope_is_live(
    state_transaction: &StateTransaction<'_, '_>,
    record: &AssetTransferCapabilityV1,
) -> Result<(), Error> {
    let Some(scope) = record.contract_scope.as_ref() else {
        if state_transaction
            .world
            .contract_subject_addresses()
            .get(&record.delegate)
            .is_some()
        {
            return Err(validation_err(
                "historical contract subjects require an exact contract-scoped capability",
            ));
        }
        return Ok(());
    };

    if scope.contract_address.subject_id() != record.delegate {
        return Err(validation_err(
            "capability contract address does not derive its fixed delegate",
        ));
    }
    if state_transaction
        .world
        .contract_subject_addresses()
        .get(&record.delegate)
        != Some(&scope.contract_address)
    {
        return Err(validation_err(
            "capability contract delegate reverse binding is missing or inconsistent",
        ));
    }
    if state_transaction
        .world
        .contract_instances()
        .get(&scope.contract_address)
        != Some(&scope.code_hash)
    {
        return Err(validation_err(
            "capability contract is inactive or its code hash changed",
        ));
    }
    let manifest = state_transaction
        .world
        .contract_manifests()
        .get(&scope.code_hash)
        .ok_or_else(|| validation_err("capability contract manifest is missing"))?;
    if !manifest.entrypoints.as_ref().is_some_and(|entrypoints| {
        entrypoints
            .iter()
            .any(|entrypoint| entrypoint.name == scope.entrypoint)
    }) {
        return Err(validation_err(
            "capability contract entrypoint is not declared by the bound code",
        ));
    }
    Ok(())
}

impl Execute for RegisterAssetTransferCapabilityV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        self.intent
            .validate()
            .map_err(|error| validation_err(error.to_string()))?;
        if &self.intent.grantor != authority {
            return Err(validation_err(
                "only the exact source owner may register an asset transfer capability",
            ));
        }
        if self.capability_id != self.intent.id() {
            return Err(validation_err(
                "asset transfer capability id does not match its immutable intent",
            ));
        }
        if state_transaction
            .world
            .asset_transfer_capabilities()
            .get(&self.capability_id)
            .is_some()
        {
            return Err(validation_err(
                "asset transfer capability id is already registered",
            ));
        }

        ensure_account_exists(&state_transaction.world, &self.intent.grantor, "grantor")?;
        ensure_account_exists(&state_transaction.world, &self.intent.delegate, "delegate")?;
        ensure_account_exists(
            &state_transaction.world,
            &self.intent.destination,
            "destination",
        )?;
        state_transaction
            .numeric_spec_for(self.intent.source.definition())
            .map_err(Error::from)?;

        let now_ms = state_transaction.block_unix_timestamp_ms();
        if now_ms >= self.intent.expires_at_ms {
            return Err(validation_err(
                "cannot register an already expired asset transfer capability",
            ));
        }
        let record = AssetTransferCapabilityV1::from_intent(self.intent, now_ms);
        ensure_contract_scope_is_live(state_transaction, &record)?;
        if state_transaction
            .world
            .insert_asset_transfer_capability_entry(record.clone())
            .is_some()
        {
            return Err(validation_err(
                "asset transfer capability duplicate insertion",
            ));
        }
        state_transaction
            .world
            .emit_events(Some(AssetTransferCapabilityEventV1::Registered(record)));
        Ok(())
    }
}

impl Execute for RevokeAssetTransferCapabilityV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut record = state_transaction
            .world
            .asset_transfer_capabilities()
            .get(&self.capability_id)
            .cloned()
            .ok_or_else(|| validation_err("asset transfer capability not found"))?;
        if &record.grantor != authority {
            return Err(validation_err(
                "only the capability grantor may revoke remaining uses",
            ));
        }
        if record.status != AssetTransferCapabilityStatusV1::Active {
            return Err(validation_err(
                "only an active asset transfer capability may be revoked",
            ));
        }
        if record.remaining_uses != self.expected_remaining_uses {
            return Err(validation_err(
                "asset transfer capability revocation compare-and-set mismatch",
            ));
        }
        record.status = AssetTransferCapabilityStatusV1::Revoked;
        record.updated_at_ms = state_transaction.block_unix_timestamp_ms();
        state_transaction
            .world
            .insert_asset_transfer_capability_entry(record.clone());
        state_transaction
            .world
            .emit_events(Some(AssetTransferCapabilityEventV1::Revoked(record)));
        Ok(())
    }
}

impl Execute for ExecuteAssetTransferCapabilityV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut record = state_transaction
            .world
            .asset_transfer_capabilities()
            .get(&self.capability_id)
            .cloned()
            .ok_or_else(|| validation_err("asset transfer capability not found"))?;
        if &record.delegate != authority {
            return Err(validation_err(
                "only the exact non-delegable capability delegate may execute it",
            ));
        }
        if self.source != record.source
            || self.destination != record.destination
            || self.amount != record.amount_per_use
            || self.evidence_digest != record.evidence_digest
        {
            return Err(validation_err(
                "asset transfer capability execution does not match its exact committed terms",
            ));
        }
        if record.status != AssetTransferCapabilityStatusV1::Active {
            return Err(validation_err("asset transfer capability is not active"));
        }
        if record.remaining_uses == 0 {
            return Err(validation_err(
                "asset transfer capability execution budget is exhausted",
            ));
        }
        if record.remaining_uses != self.expected_remaining_uses {
            return Err(validation_err(
                "asset transfer capability execution compare-and-set mismatch",
            ));
        }
        let now_ms = state_transaction.block_unix_timestamp_ms();
        if now_ms < record.valid_from_ms {
            return Err(validation_err("asset transfer capability is not valid yet"));
        }
        if now_ms >= record.expires_at_ms {
            return Err(validation_err("asset transfer capability has expired"));
        }
        ensure_contract_scope_is_live(state_transaction, &record)?;

        // Core has already established exact capability authorization. The ordinary transfer
        // pipeline still enforces account admission, numeric policy, freeze/blacklist/limit
        // controls, balance checks, transcript identity, and canonical asset events.
        execute_user_numeric_asset_transfer(
            state_transaction,
            authority,
            record.source.clone(),
            record.destination.clone(),
            record.amount_per_use.clone(),
        )?;

        record.remaining_uses = record
            .remaining_uses
            .checked_sub(1)
            .ok_or_else(|| validation_err("capability execution budget underflow"))?;
        if record.remaining_uses == 0 {
            record.status = AssetTransferCapabilityStatusV1::Consumed;
        }
        record.updated_at_ms = now_ms;
        state_transaction
            .world
            .insert_asset_transfer_capability_entry(record.clone());
        state_transaction
            .world
            .emit_events(Some(AssetTransferCapabilityEventV1::Executed(record)));
        Ok(())
    }
}

impl ValidQuery for FindAssetTransferCapabilitiesV1 {
    fn execute(
        self,
        filter: CompoundPredicate<AssetTransferCapabilityV1>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = AssetTransferCapabilityV1>, QueryExecutionFail> {
        Ok(state_ro
            .world()
            .asset_transfer_capabilities()
            .iter()
            .filter_map(move |(_, record)| filter.applies(record).then(|| record.clone())))
    }
}

impl ValidSingularQuery for FindAssetTransferCapabilityByIdV1 {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<AssetTransferCapabilityV1, QueryExecutionFail> {
        state_ro
            .world()
            .asset_transfer_capabilities()
            .get(&self.capability_id)
            .cloned()
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::AssetTransferCapability(self.capability_id))
            })
    }
}

macro_rules! indexed_capability_query {
    ($query:ty, $field:ident, $index:ident) => {
        impl ValidQuery for $query {
            fn execute(
                self,
                filter: CompoundPredicate<AssetTransferCapabilityV1>,
                state_ro: &impl StateReadOnly,
            ) -> Result<impl Iterator<Item = AssetTransferCapabilityV1>, QueryExecutionFail> {
                let key = self.$field;
                let world = state_ro.world();
                Ok(world
                    .$index()
                    .get(&key)
                    .into_iter()
                    .flat_map(BTreeSet::iter)
                    .filter_map(move |capability_id| {
                        world
                            .asset_transfer_capabilities()
                            .get(capability_id)
                            .cloned()
                    })
                    .filter(move |record| filter.applies(record)))
            }
        }
    };
}

indexed_capability_query!(
    FindAssetTransferCapabilitiesByGrantorV1,
    grantor,
    asset_transfer_capabilities_by_grantor
);
indexed_capability_query!(
    FindAssetTransferCapabilitiesByDelegateV1,
    delegate,
    asset_transfer_capabilities_by_delegate
);
indexed_capability_query!(
    FindAssetTransferCapabilitiesBySourceV1,
    source,
    asset_transfer_capabilities_by_source
);
indexed_capability_query!(
    FindAssetTransferCapabilitiesByStatusV1,
    status,
    asset_transfer_capabilities_by_status
);

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        asset_transfer_capability::{AssetTransferCapabilityIdV1, AssetTransferCapabilityIntentV1},
        block::BlockHeader,
        isi::SetAssetTransferFreeze,
    };
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    #[derive(Clone)]
    struct TestIds {
        grantor: AccountId,
        delegate: AccountId,
        destination: AccountId,
        attacker: AccountId,
        definition: AssetDefinitionId,
        source: AssetId,
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("fixture key")
                .public_key()
                .clone(),
        )
    }

    fn fixture() -> (State, TestIds) {
        let grantor = account(1);
        let delegate = account(2);
        let destination = account(3);
        let attacker = account(4);
        let domain_id = DomainId::try_new("cbdc", "universal").expect("domain");
        let definition =
            AssetDefinitionId::new(domain_id.clone(), "ils".parse().expect("asset name"));
        let source = AssetId::new(definition.clone(), grantor.clone());
        let world = World::with_assets(
            [Domain::new(domain_id).build(&grantor)],
            [
                Account::new(grantor.clone()).build(&grantor),
                Account::new(delegate.clone()).build(&delegate),
                Account::new(destination.clone()).build(&destination),
                Account::new(attacker.clone()).build(&attacker),
            ],
            [AssetDefinition::numeric(definition.clone())
                .with_name("Digital ILS".to_owned())
                .build(&grantor)],
            [Asset::new(source.clone(), Quantity::from(100_u32))],
            [],
        );
        (
            State::new_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            ),
            TestIds {
                grantor,
                delegate,
                destination,
                attacker,
                definition,
                source,
            },
        )
    }

    fn intent(ids: &TestIds, amount: u32, uses: u32) -> AssetTransferCapabilityIntentV1 {
        AssetTransferCapabilityIntentV1 {
            grantor: ids.grantor.clone(),
            delegate: ids.delegate.clone(),
            source: ids.source.clone(),
            destination: ids.destination.clone(),
            amount_per_use: Quantity::from(amount),
            evidence_digest: Hash::new(b"court-order:sha256:fixture"),
            valid_from_ms: 50,
            expires_at_ms: 1_000,
            initial_uses: uses,
            contract_scope: None,
            nonce: 17,
        }
    }

    fn transaction_header(timestamp_ms: u64) -> BlockHeader {
        BlockHeader::new(nonzero!(1_u64), None, None, None, timestamp_ms, 0)
    }

    fn seed_call_hash(transaction: &mut StateTransaction<'_, '_>) {
        transaction.tx_call_hash = Some(Hash::new(b"asset-transfer-capability-test-call"));
    }

    fn balance(transaction: &StateTransaction<'_, '_>, asset_id: &AssetId) -> Quantity {
        transaction
            .world
            .assets
            .get(asset_id)
            .map(|entry| entry.as_ref().clone())
            .unwrap_or_else(Quantity::zero)
    }

    fn record(
        transaction: &StateTransaction<'_, '_>,
        id: AssetTransferCapabilityIdV1,
    ) -> AssetTransferCapabilityV1 {
        transaction
            .world
            .asset_transfer_capabilities()
            .get(&id)
            .cloned()
            .expect("capability record")
    }

    #[test]
    fn registration_requires_grantor_and_canonical_id_without_partial_state() {
        let (state, ids) = fixture();
        let mut block = state.block(transaction_header(100));
        let mut transaction = block.transaction();
        let canonical = RegisterAssetTransferCapabilityV1::new(intent(&ids, 7, 2));
        let id = canonical.capability_id;

        let error = canonical
            .clone()
            .execute(&ids.attacker, &mut transaction)
            .expect_err("non-grantor registration must fail");
        assert!(error.to_string().contains("source owner"));
        assert!(
            transaction
                .world
                .asset_transfer_capabilities()
                .get(&id)
                .is_none()
        );

        let mut forged = canonical.clone();
        forged.capability_id = AssetTransferCapabilityIdV1::new(Hash::new(b"forged-id"));
        let error = forged
            .execute(&ids.grantor, &mut transaction)
            .expect_err("forged id must fail");
        assert!(error.to_string().contains("does not match"));
        assert!(
            transaction
                .world
                .asset_transfer_capabilities()
                .get(&id)
                .is_none()
        );

        canonical
            .execute(&ids.grantor, &mut transaction)
            .expect("canonical grantor registration succeeds");
        let stored = record(&transaction, id);
        assert_eq!(stored.remaining_uses, 2);
        assert_eq!(stored.status, AssetTransferCapabilityStatusV1::Active);
    }

    #[test]
    fn exact_terms_delegate_window_and_cas_are_enforced_atomically() {
        let (state, ids) = fixture();
        let mut block = state.block(transaction_header(100));
        let mut transaction = block.transaction();
        seed_call_hash(&mut transaction);
        let registration = RegisterAssetTransferCapabilityV1::new(intent(&ids, 7, 2));
        let id = registration.capability_id;
        registration
            .execute(&ids.grantor, &mut transaction)
            .expect("register");
        let stored = record(&transaction, id);
        let canonical = ExecuteAssetTransferCapabilityV1::from_record(&stored);

        canonical
            .clone()
            .execute(&ids.attacker, &mut transaction)
            .expect_err("wrong delegate must fail");

        let mut adversarial = Vec::new();
        let mut wrong_source = canonical.clone();
        wrong_source.source = AssetId::new(ids.definition.clone(), ids.attacker.clone());
        adversarial.push(wrong_source);
        let mut wrong_destination = canonical.clone();
        wrong_destination.destination = ids.attacker.clone();
        adversarial.push(wrong_destination);
        let mut wrong_amount = canonical.clone();
        wrong_amount.amount = Quantity::from(8_u32);
        adversarial.push(wrong_amount);
        let mut wrong_evidence = canonical.clone();
        wrong_evidence.evidence_digest = Hash::new(b"substituted-evidence");
        adversarial.push(wrong_evidence);
        let mut wrong_cas = canonical.clone();
        wrong_cas.expected_remaining_uses = 1;
        adversarial.push(wrong_cas);

        for attempt in adversarial {
            attempt
                .execute(&ids.delegate, &mut transaction)
                .expect_err("rebound terms or stale CAS must fail");
            assert_eq!(record(&transaction, id).remaining_uses, 2);
            assert_eq!(balance(&transaction, &ids.source), Quantity::from(100_u32));
        }

        canonical
            .clone()
            .execute(&ids.delegate, &mut transaction)
            .expect("first exact execution");
        assert_eq!(record(&transaction, id).remaining_uses, 1);
        assert_eq!(balance(&transaction, &ids.source), Quantity::from(93_u32));
        let destination_asset = AssetId::new(ids.definition.clone(), ids.destination.clone());
        assert_eq!(
            balance(&transaction, &destination_asset),
            Quantity::from(7_u32)
        );

        canonical
            .execute(&ids.delegate, &mut transaction)
            .expect_err("stale replay must fail");
        assert_eq!(record(&transaction, id).remaining_uses, 1);
        assert_eq!(balance(&transaction, &ids.source), Quantity::from(93_u32));

        ExecuteAssetTransferCapabilityV1::from_record(&record(&transaction, id))
            .execute(&ids.delegate, &mut transaction)
            .expect("second exact execution");
        let exhausted = record(&transaction, id);
        assert_eq!(exhausted.remaining_uses, 0);
        assert_eq!(exhausted.status, AssetTransferCapabilityStatusV1::Consumed);
        assert_eq!(balance(&transaction, &ids.source), Quantity::from(86_u32));
        assert_eq!(
            balance(&transaction, &destination_asset),
            Quantity::from(14_u32)
        );
        ExecuteAssetTransferCapabilityV1::from_record(&exhausted)
            .execute(&ids.delegate, &mut transaction)
            .expect_err("consumed capability must never replay");
    }

    #[test]
    fn ordinary_freeze_failure_does_not_consume_capability() {
        let (state, ids) = fixture();
        let mut block = state.block(transaction_header(100));
        let mut transaction = block.transaction();
        seed_call_hash(&mut transaction);
        let registration = RegisterAssetTransferCapabilityV1::new(intent(&ids, 7, 2));
        let id = registration.capability_id;
        registration
            .execute(&ids.grantor, &mut transaction)
            .expect("register");
        SetAssetTransferFreeze::new(
            ids.grantor.clone(),
            ids.definition.clone(),
            true,
            Some("court review".to_owned()),
        )
        .execute(&ids.grantor, &mut transaction)
        .expect("source owner freezes outbound transfer");

        let error = ExecuteAssetTransferCapabilityV1::from_record(&record(&transaction, id))
            .execute(&ids.delegate, &mut transaction)
            .expect_err("ordinary transfer freeze must block capability");
        assert!(error.to_string().contains("frozen"));
        assert_eq!(record(&transaction, id).remaining_uses, 2);
        assert_eq!(balance(&transaction, &ids.source), Quantity::from(100_u32));
    }

    #[test]
    fn revocation_is_grantor_only_cas_guarded_and_reindexed() {
        let (state, ids) = fixture();
        let mut block = state.block(transaction_header(100));
        let mut transaction = block.transaction();
        let registration = RegisterAssetTransferCapabilityV1::new(intent(&ids, 7, 2));
        let id = registration.capability_id;
        registration
            .execute(&ids.grantor, &mut transaction)
            .expect("register");

        RevokeAssetTransferCapabilityV1::new(id, 2)
            .execute(&ids.delegate, &mut transaction)
            .expect_err("delegate cannot revoke");
        RevokeAssetTransferCapabilityV1::new(id, 1)
            .execute(&ids.grantor, &mut transaction)
            .expect_err("stale revocation CAS must fail");
        RevokeAssetTransferCapabilityV1::new(id, 2)
            .execute(&ids.grantor, &mut transaction)
            .expect("grantor exact-CAS revocation");

        let revoked = record(&transaction, id);
        assert_eq!(revoked.status, AssetTransferCapabilityStatusV1::Revoked);
        assert_eq!(revoked.remaining_uses, 2);
        let revoked_ids: Vec<_> =
            FindAssetTransferCapabilitiesByStatusV1::new(AssetTransferCapabilityStatusV1::Revoked)
                .execute(
                    CompoundPredicate::<AssetTransferCapabilityV1>::PASS,
                    &transaction,
                )
                .expect("status query")
                .map(|record| record.id)
                .collect();
        assert_eq!(revoked_ids, vec![id]);
        assert!(
            transaction
                .world
                .asset_transfer_capabilities_by_status()
                .get(&AssetTransferCapabilityStatusV1::Active)
                .is_none_or(BTreeSet::is_empty)
        );
    }
}
