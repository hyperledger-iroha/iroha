//! Native asset escrow instruction handlers.

use eyre::Result;
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    IntoKeyValue,
    account::{Account, AccountId},
    asset::{AssetDefinitionId, AssetId},
    escrow::{AssetEscrowRecord, AssetEscrowResolution, AssetEscrowStatus, EscrowId},
    events::data::escrow::{AssetEscrowDisputed, AssetEscrowResolved, EscrowEvent},
    isi::escrow::{
        AcceptAssetEscrow, CancelAssetEscrow, MarkEscrowPaymentSent, OpenAssetEscrow,
        OpenEscrowDispute, ReleaseAssetEscrow, ResolveEscrowDispute,
    },
    permission::Permission,
    prelude::*,
    query::{
        dsl::{CompoundPredicate, EvaluatePredicate},
        error::{FindError, QueryExecutionFail},
        escrow::prelude::{
            FindAssetEscrowById, FindAssetEscrows, FindAssetEscrowsByBuyer,
            FindAssetEscrowsBySeller, FindAssetEscrowsByStatus,
        },
    },
};
use iroha_primitives::numeric::Numeric;
use mv::storage::StorageReadOnly;

use super::{Error, Execute, asset::isi::assert_numeric_spec_with};
use crate::{
    prelude::ValidSingularQuery,
    smartcontracts::ValidQuery,
    smartcontracts::isi::domain::isi::ensure_controller_capabilities,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};

/// Permission name required to resolve disputed native escrows.
pub const CAN_RESOLVE_ESCROW_DISPUTE: &str = "CanResolveEscrowDispute";

const ESCROW_CUSTODY_SEED_LABEL: &str = "iroha-native-asset-escrow-v1";

fn validation_err(message: impl Into<String>) -> Error {
    iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(
        message.into().into(),
    )
}

fn ensure_non_negative(value: &Numeric) -> Result<(), Error> {
    if value.mantissa().is_negative() {
        return Err(validation_err("escrow amount must not be negative"));
    }
    Ok(())
}

fn ensure_positive(value: &Numeric) -> Result<(), Error> {
    ensure_non_negative(value)?;
    if value.is_zero() {
        return Err(validation_err("escrow amount must be non-zero"));
    }
    Ok(())
}

fn ensure_resolution_split(
    total_amount: &Numeric,
    buyer_amount: &Numeric,
    seller_amount: &Numeric,
) -> Result<(), Error> {
    ensure_non_negative(buyer_amount)?;
    ensure_non_negative(seller_amount)?;
    let split_total = buyer_amount
        .clone()
        .checked_add(seller_amount.clone())
        .ok_or_else(|| validation_err("escrow resolution amount overflow"))?;
    if split_total != *total_amount {
        return Err(validation_err("court split must equal escrow amount"));
    }
    Ok(())
}

/// Derive the deterministic protocol custody account for an escrow.
#[must_use]
pub fn escrow_custody_account_id(
    chain_id: &iroha_data_model::ChainId,
    escrow_id: &EscrowId,
    asset_definition: &AssetDefinitionId,
) -> AccountId {
    let seed_material = format!(
        "{ESCROW_CUSTODY_SEED_LABEL}|{}|{}|{asset_definition}",
        chain_id.as_str(),
        hex::encode(escrow_id.as_hash().as_ref()),
    );
    let seed: [u8; Hash::LENGTH] = Hash::new(seed_material).into();
    let keypair = KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519);
    AccountId::new(keypair.public_key().clone())
}

fn ensure_custody_account(
    custody: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    ensure_controller_capabilities(
        custody.controller(),
        &state_transaction.crypto.allowed_signing,
        &state_transaction.crypto.allowed_curve_ids,
    )?;
    if state_transaction.world.account(custody).is_ok() {
        return Ok(());
    }
    let account = Account {
        id: custody.clone(),
        metadata: Metadata::default(),
        label: None,
        uaid: None,
        opaque_ids: Vec::new(),
    };
    let (id, value) = account.into_key_value();
    state_transaction.world.accounts.insert(id, value);
    Ok(())
}

fn has_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    permission_name: &str,
) -> bool {
    let has_named_permission = |permission: &Permission| permission.name() == permission_name;

    if state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|permissions| permissions.iter().any(has_named_permission))
    {
        return true;
    }

    state_transaction
        .world
        .account_roles
        .iter()
        .filter_map(|(role_key, ())| {
            if &role_key.account == authority {
                state_transaction.world.roles.get(&role_key.id)
            } else {
                None
            }
        })
        .any(|role| role.permissions().any(has_named_permission))
}

fn custody_asset(record: &AssetEscrowRecord) -> AssetId {
    AssetId::new(record.asset_definition.clone(), record.custody.clone())
}

fn party_asset(record: &AssetEscrowRecord, account: &AccountId) -> AssetId {
    AssetId::new(record.asset_definition.clone(), account.clone())
}

/// Return whether the asset id points at a protocol custody account recorded by a native escrow.
///
/// The guard intentionally covers closed records too. Escrow ISIs should leave closed custody
/// balances at zero, and keeping the source permanently blocked avoids ever making public,
/// deterministically derived custody controllers useful as generic asset debit authorities.
pub(crate) fn is_native_escrow_custody_asset(
    state_transaction: &StateTransaction<'_, '_>,
    source_id: &AssetId,
) -> Result<bool, Error> {
    let resolved_id = state_transaction
        .world
        .resolve_asset_id_for_current_scope(source_id)?;
    Ok(state_transaction
        .world
        .asset_escrows
        .iter()
        .any(|(_, record)| {
            record.asset_definition == *resolved_id.definition()
                && record.custody == *resolved_id.account()
        }))
}

impl Execute for OpenAssetEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .is_some()
        {
            return Err(validation_err("escrow already exists"));
        }

        ensure_positive(&self.amount)?;
        let spec = state_transaction
            .numeric_spec_for(&self.asset_definition)
            .map_err(Error::from)?;
        assert_numeric_spec_with(&self.amount, spec)?;
        state_transaction.world.account(authority)?;
        state_transaction
            .world
            .asset_definition(&self.asset_definition)?;

        let custody = escrow_custody_account_id(
            state_transaction.chain_id(),
            &self.escrow_id,
            &self.asset_definition,
        );
        ensure_custody_account(&custody, state_transaction)?;

        let seller_asset = AssetId::new(self.asset_definition.clone(), authority.clone());
        let custody_asset = AssetId::new(self.asset_definition.clone(), custody.clone());
        state_transaction
            .world
            .withdraw_numeric_asset(&seller_asset, &self.amount)?;
        state_transaction
            .world
            .deposit_numeric_asset(&custody_asset, &self.amount)?;

        let record = AssetEscrowRecord {
            id: self.escrow_id,
            seller: authority.clone(),
            buyer: None,
            asset_definition: self.asset_definition,
            amount: self.amount,
            custody,
            status: AssetEscrowStatus::Open,
            evidence_hashes: self.evidence_hashes,
            created_at_ms: state_transaction.block_unix_timestamp_ms(),
            accepted_at_ms: None,
            payment_sent_at_ms: None,
            disputed_at_ms: None,
            closed_at_ms: None,
            resolution: None,
        };
        state_transaction
            .world
            .asset_escrows
            .insert(record.id, record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Opened(record)));
        Ok(())
    }
}

impl Execute for AcceptAssetEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        state_transaction.world.account(authority)?;
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        if record.status != AssetEscrowStatus::Open {
            return Err(validation_err("only open escrows can be accepted"));
        }
        if &record.seller == authority {
            return Err(validation_err("seller cannot accept own escrow"));
        }
        record.buyer = Some(authority.clone());
        record.status = AssetEscrowStatus::Accepted;
        record.accepted_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        state_transaction
            .world
            .asset_escrows
            .insert(record.id, record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Accepted(record)));
        Ok(())
    }
}

impl Execute for MarkEscrowPaymentSent {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        if record.status != AssetEscrowStatus::Accepted {
            return Err(validation_err("only accepted escrows can be marked paid"));
        }
        if record.buyer.as_ref() != Some(authority) {
            return Err(validation_err("only accepted buyer may mark payment sent"));
        }
        record.status = AssetEscrowStatus::PaymentSent;
        record.payment_sent_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        state_transaction
            .world
            .asset_escrows
            .insert(record.id, record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::PaymentSent(record)));
        Ok(())
    }
}

impl Execute for ReleaseAssetEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        if record.status != AssetEscrowStatus::PaymentSent {
            return Err(validation_err("only paid escrows can be released"));
        }
        if &record.seller != authority {
            return Err(validation_err("only seller may release escrow"));
        }
        let buyer = record
            .buyer
            .clone()
            .ok_or_else(|| validation_err("escrow buyer missing"))?;
        let escrow_asset = custody_asset(&record);
        let buyer_asset = party_asset(&record, &buyer);
        state_transaction
            .world
            .withdraw_numeric_asset(&escrow_asset, &record.amount)?;
        state_transaction
            .world
            .deposit_numeric_asset(&buyer_asset, &record.amount)?;
        record.status = AssetEscrowStatus::Released;
        record.closed_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        state_transaction
            .world
            .asset_escrows
            .insert(record.id, record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Released(record)));
        Ok(())
    }
}

impl Execute for CancelAssetEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        if !matches!(
            record.status,
            AssetEscrowStatus::Open | AssetEscrowStatus::Accepted
        ) {
            return Err(validation_err(
                "escrow can only be cancelled before payment is marked",
            ));
        }
        if &record.seller != authority {
            return Err(validation_err("only seller may cancel escrow"));
        }
        let escrow_asset = custody_asset(&record);
        let seller_asset = party_asset(&record, &record.seller);
        state_transaction
            .world
            .withdraw_numeric_asset(&escrow_asset, &record.amount)?;
        state_transaction
            .world
            .deposit_numeric_asset(&seller_asset, &record.amount)?;
        record.status = AssetEscrowStatus::Cancelled;
        record.closed_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        state_transaction
            .world
            .asset_escrows
            .insert(record.id, record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Cancelled(record)));
        Ok(())
    }
}

impl Execute for OpenEscrowDispute {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        if !matches!(
            record.status,
            AssetEscrowStatus::Accepted | AssetEscrowStatus::PaymentSent
        ) {
            return Err(validation_err(
                "only accepted or paid escrows can enter dispute",
            ));
        }
        let is_seller = &record.seller == authority;
        let is_buyer = record.buyer.as_ref() == Some(authority);
        if !(is_seller || is_buyer) {
            return Err(validation_err(
                "only escrow buyer or seller may open dispute",
            ));
        }
        record.status = AssetEscrowStatus::Disputed;
        record.disputed_at_ms = Some(state_transaction.block_unix_timestamp_ms());
        record
            .evidence_hashes
            .extend(self.evidence_hashes.iter().copied());
        state_transaction
            .world
            .asset_escrows
            .insert(record.id, record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Disputed(AssetEscrowDisputed {
                escrow: record,
                opened_by: authority.clone(),
                evidence_hashes: self.evidence_hashes,
            })));
        Ok(())
    }
}

impl Execute for ResolveEscrowDispute {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if !has_permission(state_transaction, authority, CAN_RESOLVE_ESCROW_DISPUTE) {
            return Err(validation_err("not permitted: CanResolveEscrowDispute"));
        }
        let Some(mut record) = state_transaction
            .world
            .asset_escrows
            .get(&self.escrow_id)
            .cloned()
        else {
            return Err(validation_err("escrow not found"));
        };
        if record.status != AssetEscrowStatus::Disputed {
            return Err(validation_err("only disputed escrows can be resolved"));
        }
        ensure_resolution_split(&record.amount, &self.buyer_amount, &self.seller_amount)?;
        let buyer = record
            .buyer
            .clone()
            .ok_or_else(|| validation_err("escrow buyer missing"))?;
        let escrow_asset = custody_asset(&record);
        if !self.buyer_amount.is_zero() {
            let buyer_asset = party_asset(&record, &buyer);
            state_transaction
                .world
                .withdraw_numeric_asset(&escrow_asset, &self.buyer_amount)?;
            state_transaction
                .world
                .deposit_numeric_asset(&buyer_asset, &self.buyer_amount)?;
        }
        if !self.seller_amount.is_zero() {
            let seller_asset = party_asset(&record, &record.seller);
            state_transaction
                .world
                .withdraw_numeric_asset(&escrow_asset, &self.seller_amount)?;
            state_transaction
                .world
                .deposit_numeric_asset(&seller_asset, &self.seller_amount)?;
        }
        let resolved_at_ms = state_transaction.block_unix_timestamp_ms();
        record.status = AssetEscrowStatus::Resolved;
        record.closed_at_ms = Some(resolved_at_ms);
        record.resolution = Some(AssetEscrowResolution {
            resolver: authority.clone(),
            buyer_amount: self.buyer_amount.clone(),
            seller_amount: self.seller_amount.clone(),
            evidence_hashes: self.evidence_hashes.clone(),
            resolved_at_ms,
        });
        state_transaction
            .world
            .asset_escrows
            .insert(record.id, record.clone());
        state_transaction
            .world
            .emit_events(Some(EscrowEvent::Resolved(AssetEscrowResolved {
                escrow: record,
                resolver: authority.clone(),
                buyer_amount: self.buyer_amount,
                seller_amount: self.seller_amount,
            })));
        Ok(())
    }
}

impl ValidQuery for FindAssetEscrows {
    fn execute(
        self,
        filter: CompoundPredicate<AssetEscrowRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = AssetEscrowRecord>, QueryExecutionFail> {
        Ok(state_ro
            .world()
            .asset_escrows()
            .iter()
            .map(|(_, record)| record.clone())
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidSingularQuery for FindAssetEscrowById {
    fn execute(
        &self,
        state_ro: &impl StateReadOnly,
    ) -> Result<AssetEscrowRecord, QueryExecutionFail> {
        state_ro
            .world()
            .asset_escrows()
            .get(&self.escrow_id)
            .cloned()
            .ok_or_else(|| QueryExecutionFail::Find(FindError::AssetEscrow(self.escrow_id)))
    }
}

impl ValidQuery for FindAssetEscrowsBySeller {
    fn execute(
        self,
        filter: CompoundPredicate<AssetEscrowRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = AssetEscrowRecord>, QueryExecutionFail> {
        let seller = self.seller;
        Ok(state_ro
            .world()
            .asset_escrows()
            .iter()
            .filter_map(move |(_, record)| (record.seller == seller).then(|| record.clone()))
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidQuery for FindAssetEscrowsByBuyer {
    fn execute(
        self,
        filter: CompoundPredicate<AssetEscrowRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = AssetEscrowRecord>, QueryExecutionFail> {
        let buyer = self.buyer;
        Ok(state_ro
            .world()
            .asset_escrows()
            .iter()
            .filter_map(move |(_, record)| {
                record
                    .buyer
                    .as_ref()
                    .is_some_and(|record_buyer| record_buyer == &buyer)
                    .then(|| record.clone())
            })
            .filter(move |record| filter.applies(record)))
    }
}

impl ValidQuery for FindAssetEscrowsByStatus {
    fn execute(
        self,
        filter: CompoundPredicate<AssetEscrowRecord>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = AssetEscrowRecord>, QueryExecutionFail> {
        let status = self.status;
        Ok(state_ro
            .world()
            .asset_escrows()
            .iter()
            .filter_map(move |(_, record)| (record.status == status).then(|| record.clone()))
            .filter(move |record| filter.applies(record)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kura::Kura, query::store::LiveQueryStore, state::State};
    use iroha_data_model::permission::Permissions;
    use iroha_executor_data_model::permission::{Permission as _, escrow::CanResolveEscrowDispute};

    fn fixture_account(label: &str) -> AccountId {
        let seed: Vec<u8> = label.as_bytes().iter().copied().cycle().take(32).collect();
        let (public_key, _) = KeyPair::from_seed(seed, Algorithm::Ed25519).into_parts();
        AccountId::new(public_key)
    }

    fn fixture_asset_definition_id() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("aitai", "universal").expect("domain id"),
            "xor".parse().expect("asset name"),
        )
    }

    fn fixture_escrow_id(label: &str) -> EscrowId {
        EscrowId::new(Hash::new(format!("native-escrow-test:{label}")))
    }

    fn block_header(timestamp_ms: u64) -> iroha_data_model::block::BlockHeader {
        iroha_data_model::block::BlockHeader::new(
            nonzero_ext::nonzero!(1_u64),
            None,
            None,
            None,
            timestamp_ms,
            0,
        )
    }

    fn state_with_parties(
        seller: &AccountId,
        buyer: &AccountId,
        court: &AccountId,
        asset_definition: &AssetDefinitionId,
        seller_balance: Numeric,
    ) -> State {
        let domain = Domain::new(asset_definition.domain().clone()).build(seller);
        let asset_definition_entry = AssetDefinition::numeric(asset_definition.clone())
            .with_name("XOR".to_owned())
            .build(seller);
        let seller_asset_id = AssetId::of(asset_definition.clone(), seller.clone());
        let seller_asset = Asset::new(seller_asset_id, seller_balance);
        let world = crate::state::World::with_assets(
            [domain],
            [
                Account::new(seller.clone()).build(seller),
                Account::new(buyer.clone()).build(buyer),
                Account::new(court.clone()).build(court),
            ],
            [asset_definition_entry],
            [seller_asset],
            [],
        );
        State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    fn balance(
        state_transaction: &StateTransaction<'_, '_>,
        account: &AccountId,
        asset_definition: &AssetDefinitionId,
    ) -> Numeric {
        let asset_id = AssetId::of(asset_definition.clone(), account.clone());
        state_transaction
            .world
            .assets
            .get(&asset_id)
            .map(|value| value.as_ref().clone())
            .unwrap_or_else(Numeric::zero)
    }

    fn escrow_record(
        state_transaction: &StateTransaction<'_, '_>,
        escrow_id: &EscrowId,
    ) -> AssetEscrowRecord {
        state_transaction
            .world
            .asset_escrows
            .get(escrow_id)
            .cloned()
            .expect("escrow record")
    }

    fn grant_court_permission(state_transaction: &mut StateTransaction<'_, '_>, court: &AccountId) {
        let mut permissions = Permissions::default();
        permissions.insert(CanResolveEscrowDispute.into());
        state_transaction
            .world
            .account_permissions
            .insert(court.clone(), permissions);
    }

    fn state_transaction_deposit_closed_custody_dust(
        state_transaction: &mut StateTransaction<'_, '_>,
        custody_asset: &AssetId,
        amount: Numeric,
    ) {
        state_transaction
            .world
            .deposit_numeric_asset(custody_asset, &amount)
            .expect("deposit closed custody dust");
    }

    #[test]
    fn court_permission_constant_matches_typed_permission() {
        assert_eq!(
            CanResolveEscrowDispute::name().as_str(),
            CAN_RESOLVE_ESCROW_DISPUTE
        );
    }

    #[test]
    fn custody_account_derivation_is_stable() {
        let chain_id: iroha_data_model::ChainId = "00000000-0000-0000-0000-000000000001"
            .parse()
            .expect("chain id");
        let asset_definition: AssetDefinitionId =
            "61CtjvNd9T3THAR65GsMVHr82Bjc".parse().expect("asset");
        let escrow_id = EscrowId::new(Hash::new("escrow"));
        assert_eq!(
            escrow_custody_account_id(&chain_id, &escrow_id, &asset_definition),
            escrow_custody_account_id(&chain_id, &escrow_id, &asset_definition)
        );
    }

    #[test]
    fn resolution_split_must_equal_escrow_amount() {
        let total = Numeric::new(100_u32, 0);
        assert!(
            ensure_resolution_split(&total, &Numeric::new(40_u32, 0), &Numeric::new(60_u32, 0))
                .is_ok()
        );
        assert!(
            ensure_resolution_split(&total, &Numeric::new(40_u32, 0), &Numeric::new(59_u32, 0))
                .is_err()
        );
        assert!(
            ensure_resolution_split(&total, &Numeric::new(-1_i32, 0), &Numeric::new(101_u32, 0))
                .is_err()
        );
    }

    #[test]
    fn escrow_open_accept_mark_and_release_moves_custody_to_buyer() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("release");
        let amount = Numeric::new(40_u32, 0);
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(1_000));
        let mut tx = block.transaction();

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: amount.clone(),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Open);
        assert_eq!(
            balance(&tx, &seller, &asset_definition),
            Numeric::new(60_u32, 0)
        );
        assert_eq!(balance(&tx, &record.custody, &asset_definition), amount);

        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        MarkEscrowPaymentSent { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("mark payment sent");
        ReleaseAssetEscrow { escrow_id }
            .execute(&seller, &mut tx)
            .expect("release escrow");

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Released);
        assert_eq!(
            balance(&tx, &buyer, &asset_definition),
            Numeric::new(40_u32, 0)
        );
        assert_eq!(
            balance(&tx, &record.custody, &asset_definition),
            Numeric::zero()
        );
    }

    #[test]
    fn escrow_cancel_before_payment_refunds_seller() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("cancel");
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(2_000));
        let mut tx = block.transaction();

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: Numeric::new(40_u32, 0),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");
        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        CancelAssetEscrow { escrow_id }
            .execute(&seller, &mut tx)
            .expect("cancel escrow");

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Cancelled);
        assert_eq!(
            balance(&tx, &seller, &asset_definition),
            Numeric::new(100_u32, 0)
        );
        assert_eq!(balance(&tx, &buyer, &asset_definition), Numeric::zero());
    }

    #[test]
    fn escrow_dispute_requires_court_permission_and_valid_split() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("dispute");
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(3_000));
        let mut tx = block.transaction();

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: Numeric::new(40_u32, 0),
            evidence_hashes: vec![Hash::new("open-evidence")],
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");
        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        OpenEscrowDispute {
            escrow_id,
            evidence_hashes: vec![Hash::new("dispute-evidence")],
        }
        .execute(&buyer, &mut tx)
        .expect("open dispute");

        assert!(
            ResolveEscrowDispute {
                escrow_id,
                buyer_amount: Numeric::new(20_u32, 0),
                seller_amount: Numeric::new(20_u32, 0),
                evidence_hashes: Vec::new(),
            }
            .execute(&seller, &mut tx)
            .is_err(),
            "seller cannot resolve dispute without court permission"
        );

        grant_court_permission(&mut tx, &court);
        assert!(
            ResolveEscrowDispute {
                escrow_id,
                buyer_amount: Numeric::new(20_u32, 0),
                seller_amount: Numeric::new(19_u32, 0),
                evidence_hashes: Vec::new(),
            }
            .execute(&court, &mut tx)
            .is_err(),
            "court split must exactly match held amount"
        );

        ResolveEscrowDispute {
            escrow_id,
            buyer_amount: Numeric::new(25_u32, 0),
            seller_amount: Numeric::new(15_u32, 0),
            evidence_hashes: vec![Hash::new("resolution-evidence")],
        }
        .execute(&court, &mut tx)
        .expect("resolve dispute");

        let record = escrow_record(&tx, &escrow_id);
        assert_eq!(record.status, AssetEscrowStatus::Resolved);
        assert_eq!(
            balance(&tx, &buyer, &asset_definition),
            Numeric::new(25_u32, 0)
        );
        assert_eq!(
            balance(&tx, &seller, &asset_definition),
            Numeric::new(75_u32, 0)
        );
        assert_eq!(
            balance(&tx, &record.custody, &asset_definition),
            Numeric::zero()
        );
    }

    #[test]
    fn generic_debits_from_native_escrow_custody_are_rejected() {
        let seller = fixture_account("seller");
        let buyer = fixture_account("buyer");
        let court = fixture_account("court");
        let asset_definition = fixture_asset_definition_id();
        let escrow_id = fixture_escrow_id("direct-transfer");
        let state = state_with_parties(
            &seller,
            &buyer,
            &court,
            &asset_definition,
            Numeric::new(100_u32, 0),
        );
        let mut block = state.block(block_header(4_000));
        let mut tx = block.transaction();

        OpenAssetEscrow {
            escrow_id,
            asset_definition: asset_definition.clone(),
            amount: Numeric::new(40_u32, 0),
            evidence_hashes: Vec::new(),
        }
        .execute(&seller, &mut tx)
        .expect("open escrow");

        let record = escrow_record(&tx, &escrow_id);
        let custody_asset = AssetId::of(asset_definition.clone(), record.custody.clone());
        assert!(
            Transfer::asset_numeric(custody_asset.clone(), Numeric::new(1_u32, 0), buyer.clone())
                .execute(&seller, &mut tx)
                .is_err(),
            "generic asset transfer must not drain active native escrow custody"
        );
        assert!(
            Burn::asset_numeric(Numeric::new(1_u32, 0), custody_asset.clone())
                .execute(&seller, &mut tx)
                .is_err(),
            "generic asset burn must not drain active native escrow custody"
        );

        AcceptAssetEscrow { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("accept escrow");
        MarkEscrowPaymentSent { escrow_id }
            .execute(&buyer, &mut tx)
            .expect("mark payment sent");
        ReleaseAssetEscrow { escrow_id }
            .execute(&seller, &mut tx)
            .expect("release escrow");
        state_transaction_deposit_closed_custody_dust(
            &mut tx,
            &custody_asset,
            Numeric::new(1_u32, 0),
        );

        assert!(
            Transfer::asset_numeric(custody_asset.clone(), Numeric::new(1_u32, 0), buyer)
                .execute(&seller, &mut tx)
                .is_err(),
            "generic asset transfer must not drain recorded native escrow custody after close"
        );
        assert!(
            Burn::asset_numeric(Numeric::new(1_u32, 0), custody_asset.clone())
                .execute(&seller, &mut tx)
                .is_err(),
            "generic asset burn must not drain recorded native escrow custody after close"
        );
    }
}
