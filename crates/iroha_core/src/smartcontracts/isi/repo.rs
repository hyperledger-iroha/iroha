//! Settlement logic for repo and reverse-repo instructions.

use iroha_data_model::{
    asset::AssetId,
    events::data::prelude::{
        AccountEvent, RepoAccountEvent, RepoAccountInitiated, RepoAccountMarginCalled,
        RepoAccountRole, RepoAccountSettled,
    },
    isi::{
        error::InstructionExecutionError,
        repo::{RepoInstructionBox, RepoIsi, RepoMarginCallIsi, ReverseRepoIsi},
    },
    prelude::*,
    repo::{RepoAgreement, RepoGovernance},
};
use iroha_primitives::numeric::NumericSpec;

use super::prelude::*;
use crate::{
    smartcontracts::isi::asset::isi::assert_numeric_spec_with,
    state::{StateTransaction, WorldReadOnly},
};

const MAX_HAIRCUT_BPS: u16 = 10_000;
const MS_PER_DAY: u64 = 86_400_000;
const ACT_360_YEAR_MS: u64 = MS_PER_DAY * 360;

fn ensure_accounts(
    stx: &StateTransaction<'_, '_>,
    initiator: &AccountId,
    counterparty: &AccountId,
    custodian: Option<&AccountId>,
) -> Result<(), Error> {
    stx.world.account(initiator).map_err(Error::from)?;
    stx.world.account(counterparty).map_err(Error::from)?;
    if let Some(custodian) = custodian {
        stx.world.account(custodian).map_err(Error::from)?;
    }
    Ok(())
}

fn normalize_governance(
    governance: RepoGovernance,
    defaults: &iroha_config::parameters::actual::Repo,
) -> RepoGovernance {
    let haircut = if governance.haircut_bps() == 0 {
        defaults.default_haircut_bps.min(MAX_HAIRCUT_BPS)
    } else {
        governance.haircut_bps().min(MAX_HAIRCUT_BPS)
    };
    let margin_frequency_secs = if governance.margin_frequency_secs() == 0 {
        defaults.margin_frequency_secs
    } else {
        governance.margin_frequency_secs()
    };
    RepoGovernance::with_defaults(haircut, margin_frequency_secs)
}

fn ensure_collateral_allowed(
    defaults: &iroha_config::parameters::actual::Repo,
    collateral_asset: &AssetDefinitionId,
) -> Result<(), Error> {
    if !defaults.eligible_collateral.is_empty()
        && !defaults
            .eligible_collateral
            .iter()
            .any(|id| id == collateral_asset)
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!("collateral asset {collateral_asset} is not eligible").into(),
        ));
    }
    Ok(())
}

fn ensure_substitution_allowed(
    defaults: &iroha_config::parameters::actual::Repo,
    recorded_asset: &AssetDefinitionId,
    proposed_asset: &AssetDefinitionId,
) -> Result<(), Error> {
    if recorded_asset == proposed_asset {
        return Ok(());
    }
    if defaults.collateral_substitution_matrix.is_empty() {
        return Ok(());
    }
    if defaults
        .collateral_substitution_matrix
        .get(recorded_asset)
        .is_some_and(|list| list.iter().any(|candidate| candidate == proposed_asset))
    {
        return Ok(());
    }
    Err(InstructionExecutionError::InvariantViolation(
        format!(
            "collateral substitution from {recorded_asset} to {proposed_asset} is not allowed by policy"
        )
        .into(),
    ))
}

fn compute_accrued_interest(
    principal: Numeric,
    rate_bps: u16,
    elapsed_ms: u64,
    cash_spec: NumericSpec,
) -> Result<Numeric, Error> {
    if rate_bps == 0 || elapsed_ms == 0 || principal.is_zero() {
        return Ok(Numeric::zero());
    }

    let rate_fraction = Numeric::try_new(u128::from(rate_bps), 4).map_err(|err| {
        InstructionExecutionError::InvariantViolation(
            format!("failed to normalise repo rate: {err}").into(),
        )
    })?;
    let elapsed_fraction = Numeric::from(elapsed_ms)
        .checked_div(Numeric::from(ACT_360_YEAR_MS), NumericSpec::fractional(18));
    let elapsed_fraction = elapsed_fraction.ok_or_else(|| {
        InstructionExecutionError::InvariantViolation(
            "failed to compute repo elapsed fraction".into(),
        )
    })?;
    let rate_time = rate_fraction
        .checked_mul(elapsed_fraction, NumericSpec::fractional(18))
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "repo interest calculation overflowed (rate*time)".into(),
            )
        })?;
    let raw_interest = principal
        .checked_mul(rate_time, NumericSpec::fractional(28))
        .ok_or_else(|| {
            InstructionExecutionError::InvariantViolation(
                "repo interest calculation overflowed (principal * factor)".into(),
            )
        })?;
    let rounded = raw_interest.round(NumericSpec::fractional(cash_spec.scale().unwrap_or(28)));
    assert_numeric_spec_with(&rounded, cash_spec)?;
    Ok(rounded)
}

fn expected_cash_settlement(
    principal: Numeric,
    rate_bps: u16,
    initiated_timestamp_ms: u64,
    settlement_timestamp_ms: u64,
    cash_spec: NumericSpec,
) -> Result<Numeric, Error> {
    if settlement_timestamp_ms < initiated_timestamp_ms {
        return Err(InstructionExecutionError::InvariantViolation(
            "reverse repo settlement predates agreement initiation".into(),
        ));
    }

    let elapsed_ms = settlement_timestamp_ms - initiated_timestamp_ms;
    let interest = compute_accrued_interest(principal.clone(), rate_bps, elapsed_ms, cash_spec)?;
    principal.checked_add(interest).ok_or_else(|| {
        InstructionExecutionError::InvariantViolation(
            "repo cash leg overflowed while adding accrued interest".into(),
        )
    })
}

#[allow(clippy::too_many_lines)]
impl Execute for RepoIsi {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let RepoIsi {
            agreement_id,
            initiator,
            counterparty,
            custodian,
            cash_leg,
            collateral_leg,
            rate_bps,
            maturity_timestamp_ms,
            governance,
        } = self;

        if &initiator != authority {
            return Err(InstructionExecutionError::InvariantViolation(
                "repo initiator must match the transaction authority".into(),
            ));
        }
        if initiator == counterparty {
            return Err(InstructionExecutionError::InvariantViolation(
                "repo counterparties must be distinct".into(),
            ));
        }
        if cash_leg.quantity().is_zero() || collateral_leg.quantity().is_zero() {
            return Err(InstructionExecutionError::InvariantViolation(
                "repo legs must specify non-zero quantities".into(),
            ));
        }

        ensure_accounts(
            state_transaction,
            &initiator,
            &counterparty,
            custodian.as_ref(),
        )?;

        if state_transaction
            .world
            .repo_agreements
            .get(&agreement_id)
            .is_some()
        {
            return Err(InstructionExecutionError::InvariantViolation(
                format!("repo agreement {agreement_id} already exists").into(),
            ));
        }

        let repo_defaults = &state_transaction.settlement.repo;
        ensure_collateral_allowed(repo_defaults, collateral_leg.asset_definition_id())?;

        let normalized_governance = normalize_governance(governance, repo_defaults);

        let cash_def_id = cash_leg.asset_definition_id().clone();
        let collateral_def_id = collateral_leg.asset_definition_id().clone();

        let initiated_timestamp_ms = u64::try_from(
            state_transaction._curr_block.creation_time().as_millis(),
        )
        .map_err(|_| {
            InstructionExecutionError::InvariantViolation(
                "block creation time exceeds u64::MAX milliseconds".into(),
            )
        })?;

        let cash_spec = state_transaction
            .numeric_spec_for(&cash_def_id)
            .map_err(Error::from)?;
        assert_numeric_spec_with(cash_leg.quantity(), cash_spec)?;

        let collateral_spec = state_transaction
            .numeric_spec_for(&collateral_def_id)
            .map_err(Error::from)?;
        assert_numeric_spec_with(collateral_leg.quantity(), collateral_spec)?;

        let cash_source = AssetId::new(cash_def_id.clone(), counterparty.clone());
        let cash_destination = AssetId::new(cash_def_id.clone(), initiator.clone());
        state_transaction
            .world
            .withdraw_numeric_asset(&cash_source, cash_leg.quantity())?;
        state_transaction
            .world
            .deposit_numeric_asset(&cash_destination, cash_leg.quantity())?;

        let collateral_holder_account = custodian.clone().unwrap_or_else(|| counterparty.clone());
        let collateral_source = AssetId::new(collateral_def_id.clone(), initiator.clone());
        let collateral_destination =
            AssetId::new(collateral_def_id.clone(), collateral_holder_account.clone());
        state_transaction
            .world
            .withdraw_numeric_asset(&collateral_source, collateral_leg.quantity())?;
        state_transaction
            .world
            .deposit_numeric_asset(&collateral_destination, collateral_leg.quantity())?;

        let agreement = RepoAgreement::new(
            agreement_id.clone(),
            initiator.clone(),
            counterparty.clone(),
            cash_leg.clone(),
            collateral_leg.clone(),
            rate_bps,
            maturity_timestamp_ms,
            initiated_timestamp_ms,
            normalized_governance,
            custodian.clone(),
        );
        state_transaction
            .world
            .repo_agreements
            .insert(agreement_id.clone(), agreement.clone());

        iroha_logger::info!(
            %agreement_id,
            initiator=%initiator,
            counterparty=%counterparty,
            custodian=?custodian,
            "repo agreement initiated"
        );

        let mut repo_events =
            Vec::with_capacity(2_usize.saturating_add(usize::from(custodian.is_some())));
        repo_events.push(AccountEvent::Repo(RepoAccountEvent::Initiated(
            RepoAccountInitiated::new(
                initiator.clone(),
                counterparty.clone(),
                agreement.clone(),
                RepoAccountRole::Initiator,
            ),
        )));
        repo_events.push(AccountEvent::Repo(RepoAccountEvent::Initiated(
            RepoAccountInitiated::new(
                counterparty.clone(),
                initiator.clone(),
                agreement.clone(),
                RepoAccountRole::Counterparty,
            ),
        )));
        if let Some(custodian_account) = &custodian {
            repo_events.push(AccountEvent::Repo(RepoAccountEvent::Initiated(
                RepoAccountInitiated::new(
                    custodian_account.clone(),
                    initiator.clone(),
                    agreement,
                    RepoAccountRole::Custodian,
                ),
            )));
        }
        state_transaction.world.emit_events(repo_events);

        Ok(())
    }
}

impl Execute for RepoInstructionBox {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match self {
            RepoInstructionBox::Initiate(isi) => isi.execute(authority, state_transaction),
            RepoInstructionBox::Reverse(isi) => isi.execute(authority, state_transaction),
            RepoInstructionBox::MarginCall(isi) => isi.execute(authority, state_transaction),
        }
    }
}

#[allow(clippy::too_many_lines)]
impl Execute for ReverseRepoIsi {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let ReverseRepoIsi {
            agreement_id,
            initiator,
            counterparty,
            cash_leg,
            collateral_leg,
            settlement_timestamp_ms,
        } = self;

        if &initiator != authority {
            return Err(InstructionExecutionError::InvariantViolation(
                "reverse repo initiator must match the transaction authority".into(),
            ));
        }
        if initiator == counterparty {
            return Err(InstructionExecutionError::InvariantViolation(
                "reverse repo counterparties must be distinct".into(),
            ));
        }
        if cash_leg.quantity().is_zero() || collateral_leg.quantity().is_zero() {
            return Err(InstructionExecutionError::InvariantViolation(
                "reverse repo legs must specify non-zero quantities".into(),
            ));
        }

        let stored_agreement = state_transaction
            .world
            .repo_agreements
            .get(&agreement_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("repo agreement {agreement_id} is not active").into(),
                )
            })?;

        if stored_agreement.initiator() != &initiator {
            return Err(InstructionExecutionError::InvariantViolation(
                "reverse repo initiator does not match original agreement".into(),
            ));
        }
        if stored_agreement.counterparty() != &counterparty {
            return Err(InstructionExecutionError::InvariantViolation(
                "reverse repo counterparty does not match original agreement".into(),
            ));
        }
        if stored_agreement.cash_leg().asset_definition_id() != cash_leg.asset_definition_id() {
            return Err(InstructionExecutionError::InvariantViolation(
                "reverse repo cash leg asset definition differs from agreement".into(),
            ));
        }

        let collateral_holder_account = stored_agreement
            .custodian()
            .clone()
            .unwrap_or_else(|| counterparty.clone());

        ensure_accounts(
            state_transaction,
            &initiator,
            &counterparty,
            stored_agreement.custodian().as_ref(),
        )?;

        let block_timestamp_ms = u64::try_from(
            state_transaction._curr_block.creation_time().as_millis(),
        )
        .map_err(|_| {
            InstructionExecutionError::InvariantViolation(
                "block creation time exceeds u64::MAX milliseconds".into(),
            )
        })?;
        if settlement_timestamp_ms > block_timestamp_ms {
            return Err(InstructionExecutionError::InvariantViolation(
                "reverse repo settlement timestamp cannot be in the future".into(),
            ));
        }

        let cash_def_id = cash_leg.asset_definition_id().clone();
        let collateral_def_id = collateral_leg.asset_definition_id().clone();
        let cash_spec = state_transaction
            .numeric_spec_for(&cash_def_id)
            .map_err(Error::from)?;
        let expected_cash_quantity = expected_cash_settlement(
            stored_agreement.cash_leg().quantity().clone(),
            *stored_agreement.rate_bps(),
            *stored_agreement.initiated_timestamp_ms(),
            settlement_timestamp_ms,
            cash_spec,
        )?;
        if cash_leg.quantity() != &expected_cash_quantity {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "reverse repo cash leg must repay principal plus accrued interest: expected {expected_cash_quantity}, received {}",
                    cash_leg.quantity()
                )
                .into(),
            ));
        }
        assert_numeric_spec_with(cash_leg.quantity(), cash_spec)?;

        let collateral_spec = state_transaction
            .numeric_spec_for(&collateral_def_id)
            .map_err(Error::from)?;
        assert_numeric_spec_with(collateral_leg.quantity(), collateral_spec)?;

        let stored_collateral = stored_agreement.collateral_leg();
        let collateral_differs = stored_collateral.asset_definition_id()
            != collateral_leg.asset_definition_id()
            || stored_collateral.quantity() != collateral_leg.quantity();
        if collateral_differs {
            ensure_collateral_allowed(&state_transaction.settlement.repo, &collateral_def_id)?;
            ensure_substitution_allowed(
                &state_transaction.settlement.repo,
                stored_collateral.asset_definition_id(),
                collateral_leg.asset_definition_id(),
            )?;
            if collateral_leg.quantity() < stored_collateral.quantity() {
                return Err(InstructionExecutionError::InvariantViolation(
                    "collateral substitution must not deliver less than the recorded pledge".into(),
                ));
            }
        }

        let cash_source = AssetId::new(cash_def_id.clone(), initiator.clone());
        let cash_destination = AssetId::new(cash_def_id.clone(), counterparty.clone());
        state_transaction
            .world
            .withdraw_numeric_asset(&cash_source, cash_leg.quantity())?;
        state_transaction
            .world
            .deposit_numeric_asset(&cash_destination, cash_leg.quantity())?;

        let collateral_source =
            AssetId::new(collateral_def_id.clone(), collateral_holder_account.clone());
        let collateral_destination = AssetId::new(collateral_def_id.clone(), initiator.clone());
        state_transaction
            .world
            .withdraw_numeric_asset(&collateral_source, collateral_leg.quantity())?;
        state_transaction
            .world
            .deposit_numeric_asset(&collateral_destination, collateral_leg.quantity())?;

        state_transaction
            .world
            .repo_agreements
            .remove(agreement_id.clone());

        iroha_logger::info!(
            %agreement_id,
            initiator=%initiator,
            counterparty=%counterparty,
            custodian=?stored_agreement.custodian(),
            substituted = collateral_differs,
            "reverse repo settled"
        );

        let mut repo_events = Vec::with_capacity(
            2_usize.saturating_add(usize::from(stored_agreement.custodian().is_some())),
        );
        repo_events.push(AccountEvent::Repo(RepoAccountEvent::Settled(
            RepoAccountSettled::new(
                initiator.clone(),
                counterparty.clone(),
                agreement_id.clone(),
                cash_leg.clone(),
                collateral_leg.clone(),
                settlement_timestamp_ms,
                RepoAccountRole::Initiator,
            ),
        )));
        repo_events.push(AccountEvent::Repo(RepoAccountEvent::Settled(
            RepoAccountSettled::new(
                counterparty.clone(),
                initiator.clone(),
                agreement_id.clone(),
                cash_leg.clone(),
                collateral_leg.clone(),
                settlement_timestamp_ms,
                RepoAccountRole::Counterparty,
            ),
        )));
        if let Some(custodian_account) = stored_agreement.custodian() {
            repo_events.push(AccountEvent::Repo(RepoAccountEvent::Settled(
                RepoAccountSettled::new(
                    custodian_account.clone(),
                    initiator.clone(),
                    agreement_id.clone(),
                    cash_leg.clone(),
                    collateral_leg,
                    settlement_timestamp_ms,
                    RepoAccountRole::Custodian,
                ),
            )));
        }
        state_transaction.world.emit_events(repo_events);

        Ok(())
    }
}

impl Execute for RepoMarginCallIsi {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let RepoMarginCallIsi { agreement_id } = self;

        let mut agreement = state_transaction
            .world
            .repo_agreements
            .get(&agreement_id)
            .cloned()
            .ok_or_else(|| {
                InstructionExecutionError::InvariantViolation(
                    format!("repo agreement {agreement_id} is not active").into(),
                )
            })?;

        let is_authorised = authority == agreement.initiator()
            || authority == agreement.counterparty()
            || agreement
                .custodian()
                .as_ref()
                .is_some_and(|custodian| custodian == authority);
        if !is_authorised {
            return Err(InstructionExecutionError::InvariantViolation(
                "margin call must be initiated by a repo participant".into(),
            ));
        }

        if agreement.governance().margin_frequency_secs() == 0 {
            return Err(InstructionExecutionError::InvariantViolation(
                "margin checks are disabled for this agreement".into(),
            ));
        }

        let current_timestamp_ms = u64::try_from(
            state_transaction._curr_block.creation_time().as_millis(),
        )
        .map_err(|_| {
            InstructionExecutionError::InvariantViolation(
                "block creation time exceeds u64::MAX milliseconds".into(),
            )
        })?;

        if !agreement.is_margin_check_due(current_timestamp_ms) {
            return Err(InstructionExecutionError::InvariantViolation(
                "margin check is not yet due for this agreement".into(),
            ));
        }

        agreement.record_margin_check(current_timestamp_ms);
        state_transaction
            .world
            .repo_agreements
            .insert(agreement_id.clone(), agreement.clone());

        iroha_logger::info!(
            %agreement_id,
            initiator=%agreement.initiator(),
            counterparty=%agreement.counterparty(),
            custodian=?agreement.custodian(),
            margin_timestamp=current_timestamp_ms,
            "repo margin call recorded"
        );

        let mut repo_events = Vec::with_capacity(
            2_usize.saturating_add(usize::from(agreement.custodian().is_some())),
        );
        repo_events.push(AccountEvent::Repo(RepoAccountEvent::MarginCalled(
            RepoAccountMarginCalled::new(
                agreement.initiator().clone(),
                agreement.counterparty().clone(),
                agreement_id.clone(),
                current_timestamp_ms,
                RepoAccountRole::Initiator,
            ),
        )));
        repo_events.push(AccountEvent::Repo(RepoAccountEvent::MarginCalled(
            RepoAccountMarginCalled::new(
                agreement.counterparty().clone(),
                agreement.initiator().clone(),
                agreement_id.clone(),
                current_timestamp_ms,
                RepoAccountRole::Counterparty,
            ),
        )));
        if let Some(custodian_account) = agreement.custodian().as_ref() {
            repo_events.push(AccountEvent::Repo(RepoAccountEvent::MarginCalled(
                RepoAccountMarginCalled::new(
                    custodian_account.clone(),
                    agreement.initiator().clone(),
                    agreement_id.clone(),
                    current_timestamp_ms,
                    RepoAccountRole::Custodian,
                ),
            )));
        }
        state_transaction.world.emit_events(repo_events);

        Ok(())
    }
}

/// Repo-related query implementations.
pub mod query {
    use eyre::Result;
    use iroha_data_model::{
        query::{
            dsl::{CompoundPredicate, EvaluatePredicate},
            error::QueryExecutionFail as Error,
            repo::prelude::FindRepoAgreements,
        },
        repo::RepoAgreement,
    };
    use iroha_telemetry::metrics;

    use super::*;
    use crate::{smartcontracts::ValidQuery, state::StateReadOnly};

    impl ValidQuery for FindRepoAgreements {
        #[metrics(+"find_repo_agreements")]
        fn execute(
            self,
            filter: CompoundPredicate<RepoAgreement>,
            state_ro: &impl StateReadOnly,
        ) -> Result<impl Iterator<Item = RepoAgreement>, Error> {
            Ok(state_ro
                .world()
                .repo_agreements()
                .iter()
                .filter(move |(_, agreement)| filter.applies(agreement))
                .map(|(_, agreement)| agreement.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use hex::encode_upper;
    use iroha_crypto::{Hash, KeyPair};
    use iroha_data_model::{
        account::{Account, AccountId},
        asset::{
            Asset, AssetDefinition,
            prelude::{AssetDefinitionId, AssetId},
        },
        block::BlockHeader,
        domain::{Domain, DomainId},
        events::data::prelude::{
            AccountEvent, DataEvent, DomainEvent, RepoAccountEvent, RepoAccountRole,
        },
        isi::{InstructionBox, repo::RepoInstructionBox},
        repo::{RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance},
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::{ALICE_ID, BOB_ID};
    use nonzero_ext::nonzero;
    use norito::json::{Map, Number, Value};

    use super::*;
    use crate::{kura::Kura, prelude::World, query::store::LiveQueryStore, state::State};

    fn setup_state() -> (State, RepoAgreementId, AssetDefinitionId, AssetDefinitionId) {
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);

        let alice_account =
            Account::new_in_domain(ALICE_ID.clone(), domain_id.clone()).build(&ALICE_ID);
        let bob_account =
            Account::new_in_domain(BOB_ID.clone(), domain_id.clone()).build(&ALICE_ID);

        let cash_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "usd".parse().unwrap(),
        );
        let collateral_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "bond".parse().unwrap(),
        );

        let cash_def = {
            let __asset_definition_id = cash_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&ALICE_ID);
        let collateral_def = {
            let __asset_definition_id = collateral_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&ALICE_ID);

        let bob_cash = Asset::new(
            AssetId::new(cash_def_id.clone(), BOB_ID.clone()),
            Numeric::from(2_000u32),
        );
        let alice_collateral = Asset::new(
            AssetId::new(collateral_def_id.clone(), ALICE_ID.clone()),
            Numeric::from(1_500u32),
        );

        let world = World::with_assets(
            [domain],
            [alice_account, bob_account],
            [cash_def, collateral_def],
            [bob_cash, alice_collateral],
            [],
        );

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new(world, kura, query);
        let agreement_id: RepoAgreementId = "daily_repo".parse().unwrap();

        (state, agreement_id, cash_def_id, collateral_def_id)
    }

    fn setup_state_with_dual_collateral() -> (
        State,
        RepoAgreementId,
        AssetDefinitionId,
        AssetDefinitionId,
        AssetDefinitionId,
    ) {
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);

        let alice_account =
            Account::new_in_domain(ALICE_ID.clone(), domain_id.clone()).build(&ALICE_ID);
        let bob_account =
            Account::new_in_domain(BOB_ID.clone(), domain_id.clone()).build(&ALICE_ID);

        let cash_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "usd".parse().unwrap(),
        );
        let collateral_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "bond".parse().unwrap(),
        );
        let alt_collateral_def_id: AssetDefinitionId =
            iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "note".parse().unwrap(),
            );

        let cash_def = {
            let __asset_definition_id = cash_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&ALICE_ID);
        let collateral_def = {
            let __asset_definition_id = collateral_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&ALICE_ID);
        let alt_collateral_def = {
            let __asset_definition_id = alt_collateral_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&ALICE_ID);

        let bob_cash = Asset::new(
            AssetId::new(cash_def_id.clone(), BOB_ID.clone()),
            Numeric::from(2_000u32),
        );
        let alice_collateral = Asset::new(
            AssetId::new(collateral_def_id.clone(), ALICE_ID.clone()),
            Numeric::from(1_500u32),
        );
        let bob_alt_collateral = Asset::new(
            AssetId::new(alt_collateral_def_id.clone(), BOB_ID.clone()),
            Numeric::from(2_000u32),
        );

        let world = World::with_assets(
            [domain],
            [alice_account, bob_account],
            [cash_def, collateral_def, alt_collateral_def],
            [bob_cash, alice_collateral, bob_alt_collateral],
            [],
        );

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new(world, kura, query);
        let agreement_id: RepoAgreementId = "daily_repo".parse().unwrap();

        (
            state,
            agreement_id,
            cash_def_id,
            collateral_def_id,
            alt_collateral_def_id,
        )
    }

    fn setup_state_with_custodian() -> (
        State,
        RepoAgreementId,
        AssetDefinitionId,
        AssetDefinitionId,
        AccountId,
    ) {
        crate::test_alias::ensure();
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);

        let alice_account =
            Account::new_in_domain(ALICE_ID.clone(), domain_id.clone()).build(&ALICE_ID);
        let bob_account =
            Account::new_in_domain(BOB_ID.clone(), domain_id.clone()).build(&ALICE_ID);
        let custodian_id = AccountId::new(KeyPair::random().public_key().clone());
        let custodian_account =
            Account::new_in_domain(custodian_id.clone(), domain_id.clone()).build(&ALICE_ID);

        let cash_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "usd".parse().unwrap(),
        );
        let collateral_def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "bond".parse().unwrap(),
        );

        let cash_def = {
            let __asset_definition_id = cash_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&ALICE_ID);
        let collateral_def = {
            let __asset_definition_id = collateral_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&ALICE_ID);

        let bob_cash = Asset::new(
            AssetId::new(cash_def_id.clone(), BOB_ID.clone()),
            Numeric::from(2_000u32),
        );
        let alice_collateral = Asset::new(
            AssetId::new(collateral_def_id.clone(), ALICE_ID.clone()),
            Numeric::from(1_500u32),
        );

        let world = World::with_assets(
            [domain],
            [alice_account, bob_account, custodian_account],
            [cash_def, collateral_def],
            [bob_cash, alice_collateral],
            [],
        );

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new(world, kura, query);
        let agreement_id: RepoAgreementId = "daily_repo".parse().unwrap();

        (
            state,
            agreement_id,
            cash_def_id,
            collateral_def_id,
            custodian_id,
        )
    }

    fn repo_setup_instruction(
        agreement_id: &RepoAgreementId,
        cash_def_id: &AssetDefinitionId,
        collateral_def_id: &AssetDefinitionId,
    ) -> RepoIsi {
        RepoIsi::new(
            agreement_id.clone(),
            ALICE_ID.clone(),
            BOB_ID.clone(),
            None,
            RepoCashLeg {
                asset_definition_id: cash_def_id.clone(),
                quantity: Numeric::from(1_000u32),
            },
            RepoCollateralLeg::new(collateral_def_id.clone(), Numeric::from(1_100u32)),
            250,
            1_704_000_000_000,
            RepoGovernance::with_defaults(1_500, 86_400),
        )
    }

    #[test]
    fn repo_initiation_transfers_assets_and_records_agreement() {
        let (state, agreement_id, cash_def_id, collateral_def_id) = setup_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let repo_instruction =
            repo_setup_instruction(&agreement_id, &cash_def_id, &collateral_def_id);
        repo_instruction
            .execute(&ALICE_ID, &mut stx)
            .expect("repo execution");

        let mut initiator_event = None;
        let mut counterparty_event = None;
        let mut custodian_event = None;
        for event in &stx.world.internal_event_buf {
            if let DataEvent::Domain(DomainEvent::Account(AccountEvent::Repo(
                RepoAccountEvent::Initiated(payload),
            ))) = event.as_ref()
            {
                match payload.role {
                    RepoAccountRole::Initiator => initiator_event = Some(payload.clone()),
                    RepoAccountRole::Counterparty => counterparty_event = Some(payload.clone()),
                    RepoAccountRole::Custodian => custodian_event = Some(payload.clone()),
                }
            }
        }
        let initiator_event =
            initiator_event.expect("initiator should receive a repo initiation event");
        let counterparty_event =
            counterparty_event.expect("counterparty should receive a repo initiation event");
        assert_eq!(initiator_event.account.clone(), ALICE_ID.clone());
        assert_eq!(initiator_event.counterparty.clone(), BOB_ID.clone());
        assert_eq!(initiator_event.agreement.id(), &agreement_id);
        assert_eq!(initiator_event.role, RepoAccountRole::Initiator);
        assert_eq!(counterparty_event.account.clone(), BOB_ID.clone());
        assert_eq!(counterparty_event.counterparty.clone(), ALICE_ID.clone());
        assert_eq!(counterparty_event.agreement.id(), &agreement_id);
        assert_eq!(counterparty_event.role, RepoAccountRole::Counterparty);
        assert!(
            custodian_event.is_none(),
            "custodian should be absent in two-party repo"
        );

        assert!(stx.world.repo_agreements.get(&agreement_id).is_some());
        let recorded = stx
            .world
            .repo_agreements
            .get(&agreement_id)
            .expect("agreement");
        assert_eq!(recorded.initiated_timestamp_ms, 0);

        let alice_cash_id = AssetId::new(cash_def_id.clone(), ALICE_ID.clone());
        let bob_cash_id = AssetId::new(cash_def_id.clone(), BOB_ID.clone());
        assert_eq!(
            **stx.world.assets.get(&alice_cash_id).expect("alice cash"),
            Numeric::from(1_000u32)
        );
        assert_eq!(
            **stx.world.assets.get(&bob_cash_id).expect("bob cash"),
            Numeric::from(1_000u32)
        );

        let alice_collateral_id = AssetId::new(collateral_def_id.clone(), ALICE_ID.clone());
        let bob_collateral_id = AssetId::new(collateral_def_id.clone(), BOB_ID.clone());
        assert_eq!(
            **stx
                .world
                .assets
                .get(&alice_collateral_id)
                .expect("alice collateral"),
            Numeric::from(400u32)
        );
        assert_eq!(
            **stx
                .world
                .assets
                .get(&bob_collateral_id)
                .expect("bob collateral"),
            Numeric::from(1_100u32)
        );

        stx.apply();
        block.commit().expect("commit succeeds");

        let view = state.view();
        assert!(view.world.repo_agreements().get(&agreement_id).is_some());
    }

    #[test]
    fn repo_instruction_box_executes_via_instruction_dispatch() {
        let (state, agreement_id, cash_def_id, collateral_def_id) = setup_state();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let repo_instruction =
            repo_setup_instruction(&agreement_id, &cash_def_id, &collateral_def_id);
        let boxed: InstructionBox = RepoInstructionBox::from(repo_instruction).into();
        boxed
            .execute(&ALICE_ID, &mut stx)
            .expect("repo instruction box execution");

        assert!(
            stx.world.repo_agreements.get(&agreement_id).is_some(),
            "repo instruction box should record the agreement"
        );
    }

    #[test]
    fn repo_initiation_with_custodian_routes_collateral() {
        let (state, agreement_id, cash_def_id, collateral_def_id, custodian_id) =
            setup_state_with_custodian();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let repo_instruction = RepoIsi::new(
            agreement_id.clone(),
            ALICE_ID.clone(),
            BOB_ID.clone(),
            Some(custodian_id.clone()),
            RepoCashLeg {
                asset_definition_id: cash_def_id.clone(),
                quantity: Numeric::from(1_000u32),
            },
            RepoCollateralLeg::new(collateral_def_id.clone(), Numeric::from(1_100u32)),
            250,
            1_704_000_000_000,
            RepoGovernance::with_defaults(1_500, 86_400),
        );
        repo_instruction
            .execute(&ALICE_ID, &mut stx)
            .expect("repo execution");

        let mut roles = Vec::new();
        for event in &stx.world.internal_event_buf {
            if let DataEvent::Domain(DomainEvent::Account(AccountEvent::Repo(
                RepoAccountEvent::Initiated(payload),
            ))) = event.as_ref()
            {
                roles.push((payload.account.clone(), payload.role));
                if payload.role == RepoAccountRole::Custodian {
                    assert_eq!(&payload.account, &custodian_id);
                    assert_eq!(&payload.counterparty, &*ALICE_ID);
                }
            }
        }
        assert!(roles.contains(&(ALICE_ID.clone(), RepoAccountRole::Initiator)));
        assert!(roles.contains(&(BOB_ID.clone(), RepoAccountRole::Counterparty)));
        assert!(roles.contains(&(custodian_id.clone(), RepoAccountRole::Custodian)));

        let custodian_collateral_id = AssetId::new(collateral_def_id.clone(), custodian_id.clone());
        assert_eq!(
            **stx
                .world
                .assets
                .get(&custodian_collateral_id)
                .expect("custodian collateral"),
            Numeric::from(1_100u32)
        );

        stx.apply();
        block.commit().expect("commit succeeds");

        let view = state.view();
        let stored = view
            .world
            .repo_agreements()
            .get(&agreement_id)
            .expect("agreement stored");
        assert_eq!(stored.custodian(), &Some(custodian_id));
    }

    #[allow(clippy::too_many_lines)]
    #[test]
    fn reverse_repo_restores_assets_and_clears_agreement() {
        let (state, agreement_id, cash_def_id, collateral_def_id) = setup_state();

        {
            let initiation_ms: u64 = 1_704_000_000_000;
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, initiation_ms, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            let repo_instruction =
                repo_setup_instruction(&agreement_id, &cash_def_id, &collateral_def_id);
            repo_instruction
                .execute(&ALICE_ID, &mut stx)
                .expect("repo execute");
            stx.apply();
            block.commit().expect("commit");
        }

        let settlement_ms: u64 = 1_704_000_000_000 + super::MS_PER_DAY;
        let stored_agreement = {
            let view = state.view();
            view.world
                .repo_agreements()
                .get(&agreement_id)
                .cloned()
                .expect("agreement snapshot")
        };
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, settlement_ms, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let cash_spec = stx
            .numeric_spec_for(&cash_def_id)
            .expect("cash spec for settlement");
        let expected_cash = super::expected_cash_settlement(
            stored_agreement.cash_leg().quantity().clone(),
            *stored_agreement.rate_bps(),
            *stored_agreement.initiated_timestamp_ms(),
            settlement_ms,
            cash_spec,
        )
        .expect("interest calculation");
        let interest_due = expected_cash
            .clone()
            .clone()
            .checked_sub(stored_agreement.cash_leg().quantity().clone())
            .expect("interest non-negative");

        let alice_cash_id = AssetId::new(cash_def_id.clone(), ALICE_ID.clone());
        if !interest_due.is_zero() {
            stx.world
                .deposit_numeric_asset(&alice_cash_id, &interest_due)
                .expect("seed interest funds");
        }

        let reverse_instruction = ReverseRepoIsi::new(
            agreement_id.clone(),
            ALICE_ID.clone(),
            BOB_ID.clone(),
            RepoCashLeg {
                asset_definition_id: cash_def_id.clone(),
                quantity: expected_cash.clone(),
            },
            RepoCollateralLeg::new(collateral_def_id.clone(), Numeric::from(1_100u32)),
            settlement_ms,
        );

        reverse_instruction
            .execute(&ALICE_ID, &mut stx)
            .expect("reverse repo execute");

        let mut initiator_event = None;
        let mut counterparty_event = None;
        let mut custodian_event = None;
        for event in &stx.world.internal_event_buf {
            if let DataEvent::Domain(DomainEvent::Account(AccountEvent::Repo(
                RepoAccountEvent::Settled(payload),
            ))) = event.as_ref()
            {
                match payload.role() {
                    RepoAccountRole::Initiator => initiator_event = Some(payload.clone()),
                    RepoAccountRole::Counterparty => counterparty_event = Some(payload.clone()),
                    RepoAccountRole::Custodian => custodian_event = Some(payload.clone()),
                }
            }
        }
        let initiator_event =
            initiator_event.expect("initiator should receive a repo settlement event");
        let counterparty_event =
            counterparty_event.expect("counterparty should receive a repo settlement event");
        assert_eq!(initiator_event.agreement_id(), &agreement_id);
        assert_eq!(initiator_event.account().clone(), ALICE_ID.clone());
        assert_eq!(initiator_event.counterparty().clone(), BOB_ID.clone());
        assert_eq!(initiator_event.cash_leg().quantity(), &expected_cash);
        assert_eq!(
            initiator_event.collateral_leg().quantity(),
            stored_agreement.collateral_leg().quantity()
        );
        assert_eq!(initiator_event.settled_timestamp_ms(), &settlement_ms);
        assert_eq!(initiator_event.role(), &RepoAccountRole::Initiator);
        assert_eq!(counterparty_event.agreement_id(), &agreement_id);
        assert_eq!(counterparty_event.account().clone(), BOB_ID.clone());
        assert_eq!(counterparty_event.counterparty().clone(), ALICE_ID.clone());
        assert_eq!(counterparty_event.cash_leg().quantity(), &expected_cash);
        assert_eq!(
            counterparty_event.collateral_leg().quantity(),
            stored_agreement.collateral_leg().quantity()
        );
        assert_eq!(counterparty_event.settled_timestamp_ms(), &settlement_ms);
        assert_eq!(counterparty_event.role(), &RepoAccountRole::Counterparty);
        assert!(
            custodian_event.is_none(),
            "custodian event unexpected for repo without custodian"
        );

        assert!(
            stx.world.repo_agreements.get(&agreement_id).is_none(),
            "agreement should be cleared during unwind"
        );

        let alice_cash_id = AssetId::new(cash_def_id.clone(), ALICE_ID.clone());
        let bob_cash_id = AssetId::new(cash_def_id.clone(), BOB_ID.clone());
        assert!(
            stx.world.assets.get(&alice_cash_id).is_none(),
            "alice cash entry should be pruned when balance returns to zero"
        );
        assert_eq!(
            **stx.world.assets.get(&bob_cash_id).expect("bob cash"),
            Numeric::from(2_000u32)
                .checked_add(interest_due)
                .expect("principal + interest")
        );

        let alice_collateral_id = AssetId::new(collateral_def_id.clone(), ALICE_ID.clone());
        let bob_collateral_id = AssetId::new(collateral_def_id.clone(), BOB_ID.clone());
        assert_eq!(
            **stx
                .world
                .assets
                .get(&alice_collateral_id)
                .expect("alice collateral"),
            Numeric::from(1_500u32)
        );
        assert!(
            stx.world.assets.get(&bob_collateral_id).is_none(),
            "counterparty collateral should be fully returned"
        );

        stx.apply();
        block.commit().expect("commit");

        let view = state.view();
        assert!(view.world.repo_agreements().get(&agreement_id).is_none());
    }

    #[allow(clippy::too_many_lines)]
    #[test]
    fn reverse_repo_allows_collateral_substitution_quantity() {
        let (state, agreement_id, cash_def_id, collateral_def_id) = setup_state();

        let initiation_ms: u64 = 1_704_500_000_000;
        {
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, initiation_ms, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            repo_setup_instruction(&agreement_id, &cash_def_id, &collateral_def_id)
                .execute(&ALICE_ID, &mut stx)
                .expect("repo execute");
            stx.apply();
            block.commit().expect("commit");
        }

        let settlement_ms = initiation_ms + super::MS_PER_DAY;
        let stored_agreement = {
            let view = state.view();
            view.world
                .repo_agreements()
                .get(&agreement_id)
                .cloned()
                .expect("agreement snapshot")
        };
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, settlement_ms, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let cash_spec = stx
            .numeric_spec_for(&cash_def_id)
            .expect("cash spec for settlement");
        let expected_cash = super::expected_cash_settlement(
            stored_agreement.cash_leg().quantity().clone(),
            *stored_agreement.rate_bps(),
            *stored_agreement.initiated_timestamp_ms(),
            settlement_ms,
            cash_spec,
        )
        .expect("interest calculation");
        let interest_due = expected_cash
            .clone()
            .checked_sub(stored_agreement.cash_leg().quantity().clone())
            .expect("interest non-negative");

        let alice_cash_id = AssetId::new(cash_def_id.clone(), ALICE_ID.clone());
        if !interest_due.is_zero() {
            stx.world
                .deposit_numeric_asset(&alice_cash_id, &interest_due)
                .expect("seed interest funds");
        }

        // Provide additional collateral to Bob so he can settle above the recorded pledge.
        let extra_collateral = Numeric::from(50u32);
        let bob_collateral_id = AssetId::new(collateral_def_id.clone(), BOB_ID.clone());
        stx.world
            .deposit_numeric_asset(&bob_collateral_id, &extra_collateral)
            .expect("seed substitution collateral");

        let adjusted_collateral = stored_agreement
            .collateral_leg()
            .quantity()
            .clone()
            .checked_add(extra_collateral.clone())
            .expect("adjusted collateral fits");
        let reverse_instruction = ReverseRepoIsi::new(
            agreement_id.clone(),
            ALICE_ID.clone(),
            BOB_ID.clone(),
            RepoCashLeg {
                asset_definition_id: cash_def_id.clone(),
                quantity: expected_cash.clone(),
            },
            RepoCollateralLeg::new(collateral_def_id.clone(), adjusted_collateral.clone()),
            settlement_ms,
        );

        reverse_instruction
            .execute(&ALICE_ID, &mut stx)
            .expect("reverse repo execute");

        let mut initiator_event = None;
        let mut counterparty_event = None;
        let mut custodian_event = None;
        for event in &stx.world.internal_event_buf {
            if let DataEvent::Domain(DomainEvent::Account(AccountEvent::Repo(
                RepoAccountEvent::Settled(payload),
            ))) = event.as_ref()
            {
                match payload.role() {
                    RepoAccountRole::Initiator => initiator_event = Some(payload.clone()),
                    RepoAccountRole::Counterparty => counterparty_event = Some(payload.clone()),
                    RepoAccountRole::Custodian => custodian_event = Some(payload.clone()),
                }
            }
        }
        let initiator_event =
            initiator_event.expect("initiator should receive a repo settlement event");
        let counterparty_event =
            counterparty_event.expect("counterparty should receive a repo settlement event");
        assert_eq!(initiator_event.agreement_id(), &agreement_id);
        assert_eq!(initiator_event.account().clone(), ALICE_ID.clone());
        assert_eq!(initiator_event.counterparty().clone(), BOB_ID.clone());
        assert_eq!(initiator_event.cash_leg().quantity(), &expected_cash);
        assert_eq!(
            initiator_event.collateral_leg().quantity(),
            &adjusted_collateral
        );
        assert_eq!(initiator_event.settled_timestamp_ms(), &settlement_ms);
        assert_eq!(initiator_event.role(), &RepoAccountRole::Initiator);
        assert_eq!(counterparty_event.agreement_id(), &agreement_id);
        assert_eq!(counterparty_event.account().clone(), BOB_ID.clone());
        assert_eq!(counterparty_event.counterparty().clone(), ALICE_ID.clone());
        assert_eq!(counterparty_event.cash_leg().quantity(), &expected_cash);
        assert_eq!(
            counterparty_event.collateral_leg().quantity(),
            &adjusted_collateral
        );
        assert_eq!(counterparty_event.settled_timestamp_ms(), &settlement_ms);
        assert_eq!(counterparty_event.role(), &RepoAccountRole::Counterparty);
        assert!(
            custodian_event.is_none(),
            "custodian event unexpected for repo without custodian"
        );

        assert!(
            stx.world.repo_agreements.get(&agreement_id).is_none(),
            "agreement should be cleared during unwind"
        );

        let bob_cash_id = AssetId::new(cash_def_id.clone(), BOB_ID.clone());
        assert_eq!(
            **stx.world.assets.get(&bob_cash_id).expect("bob cash"),
            Numeric::from(2_000u32)
                .checked_add(interest_due)
                .expect("principal + interest")
        );

        let alice_collateral_id = AssetId::new(collateral_def_id.clone(), ALICE_ID.clone());
        assert_eq!(
            **stx
                .world
                .assets
                .get(&alice_collateral_id)
                .expect("alice collateral"),
            Numeric::from(1_500u32)
                .checked_add(extra_collateral.clone())
                .expect("collateral returned with substitution increment")
        );
        assert!(
            stx.world.assets.get(&bob_collateral_id).is_none(),
            "counterparty collateral should be fully returned even after substitution"
        );

        stx.apply();
        block.commit().expect("commit");

        let view = state.view();
        assert!(view.world.repo_agreements().get(&agreement_id).is_none());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn reverse_repo_substitution_rejected_without_matrix_entry() {
        let (mut state, agreement_id, cash_def_id, collateral_def_id, alt_collateral_id) =
            setup_state_with_dual_collateral();

        let mut repo_config = state.settlement.repo.clone();
        repo_config.eligible_collateral =
            vec![collateral_def_id.clone(), alt_collateral_id.clone()];
        repo_config
            .collateral_substitution_matrix
            .insert(collateral_def_id.clone(), vec![collateral_def_id.clone()]);
        state.settlement.repo = repo_config;

        let initiation_ms: u64 = 1_704_500_000_000;
        {
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, initiation_ms, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            repo_setup_instruction(&agreement_id, &cash_def_id, &collateral_def_id)
                .execute(&ALICE_ID, &mut stx)
                .expect("repo execute");
            stx.apply();
            block.commit().expect("commit");
        }

        let unwind_ms = initiation_ms + super::MS_PER_DAY;
        let stored_agreement = {
            let view = state.view();
            view.world
                .repo_agreements()
                .get(&agreement_id)
                .cloned()
                .expect("agreement snapshot")
        };
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, unwind_ms, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let cash_spec = stx
            .numeric_spec_for(&cash_def_id)
            .expect("cash spec snapshot");
        let expected_cash = super::expected_cash_settlement(
            stored_agreement.cash_leg().quantity().clone(),
            *stored_agreement.rate_bps(),
            *stored_agreement.initiated_timestamp_ms(),
            unwind_ms,
            cash_spec,
        )
        .expect("interest calculation");
        let interest_due = expected_cash
            .clone()
            .checked_sub(stored_agreement.cash_leg().quantity().clone())
            .expect("interest non-negative");
        if !interest_due.is_zero() {
            let alice_cash = AssetId::new(cash_def_id.clone(), ALICE_ID.clone());
            stx.world
                .deposit_numeric_asset(&alice_cash, &interest_due)
                .expect("seed interest");
        }

        let reverse = ReverseRepoIsi::new(
            agreement_id.clone(),
            ALICE_ID.clone(),
            BOB_ID.clone(),
            RepoCashLeg {
                asset_definition_id: cash_def_id.clone(),
                quantity: expected_cash.clone(),
            },
            RepoCollateralLeg::new(
                alt_collateral_id.clone(),
                stored_agreement.collateral_leg().quantity().clone(),
            ),
            unwind_ms,
        );

        let err = reverse
            .execute(&ALICE_ID, &mut stx)
            .expect_err("matrix should reject substitution");
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("collateral substitution from")
                && err_msg.contains("is not allowed by policy"),
            "unexpected error message: {err:?}"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn reverse_repo_substitution_allowed_with_matrix_entry() {
        let (mut state, agreement_id, cash_def_id, collateral_def_id, alt_collateral_id) =
            setup_state_with_dual_collateral();

        let mut repo_config = state.settlement.repo.clone();
        repo_config.eligible_collateral =
            vec![collateral_def_id.clone(), alt_collateral_id.clone()];
        repo_config
            .collateral_substitution_matrix
            .insert(collateral_def_id.clone(), vec![alt_collateral_id.clone()]);
        state.settlement.repo = repo_config;

        let initiation_ms: u64 = 1_704_600_000_000;
        {
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, initiation_ms, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            repo_setup_instruction(&agreement_id, &cash_def_id, &collateral_def_id)
                .execute(&ALICE_ID, &mut stx)
                .expect("repo execute");
            stx.apply();
            block.commit().expect("commit");
        }

        let unwind_ms = initiation_ms + super::MS_PER_DAY;
        let stored_agreement = {
            let view = state.view();
            view.world
                .repo_agreements()
                .get(&agreement_id)
                .cloned()
                .expect("agreement snapshot")
        };
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, unwind_ms, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        let cash_spec = stx
            .numeric_spec_for(&cash_def_id)
            .expect("cash spec snapshot");
        let expected_cash = super::expected_cash_settlement(
            stored_agreement.cash_leg().quantity().clone(),
            *stored_agreement.rate_bps(),
            *stored_agreement.initiated_timestamp_ms(),
            unwind_ms,
            cash_spec,
        )
        .expect("interest calculation");
        let interest_due = expected_cash
            .clone()
            .checked_sub(stored_agreement.cash_leg().quantity().clone())
            .expect("interest non-negative");
        if !interest_due.is_zero() {
            let alice_cash = AssetId::new(cash_def_id.clone(), ALICE_ID.clone());
            stx.world
                .deposit_numeric_asset(&alice_cash, &interest_due)
                .expect("seed interest");
        }

        let reverse = ReverseRepoIsi::new(
            agreement_id.clone(),
            ALICE_ID.clone(),
            BOB_ID.clone(),
            RepoCashLeg {
                asset_definition_id: cash_def_id.clone(),
                quantity: expected_cash.clone(),
            },
            RepoCollateralLeg::new(
                alt_collateral_id.clone(),
                stored_agreement.collateral_leg().quantity().clone(),
            ),
            unwind_ms,
        );

        reverse
            .execute(&ALICE_ID, &mut stx)
            .expect("matrix should allow substitution");
        stx.apply();
        block.commit().expect("commit");

        let view = state.view();
        let alice_alt_asset = AssetId::new(alt_collateral_id.clone(), ALICE_ID.clone());
        let quantity = view
            .world
            .assets()
            .get(&alice_alt_asset)
            .expect("alice alt collateral after substitution");
        assert_eq!(
            **quantity,
            stored_agreement.collateral_leg().quantity().clone(),
            "alice should receive the pledged quantity in the alternate asset"
        );
    }

    #[allow(clippy::too_many_lines)]
    #[test]
    fn reverse_repo_with_custodian_emits_events_for_all_parties() {
        let (state, agreement_id, cash_def_id, collateral_def_id, custodian_id) =
            setup_state_with_custodian();

        let initiation_ms: u64 = 1_705_000_000_000;
        {
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, initiation_ms, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            RepoIsi::new(
                agreement_id.clone(),
                ALICE_ID.clone(),
                BOB_ID.clone(),
                Some(custodian_id.clone()),
                RepoCashLeg {
                    asset_definition_id: cash_def_id.clone(),
                    quantity: Numeric::from(1_000u32),
                },
                RepoCollateralLeg::new(collateral_def_id.clone(), Numeric::from(1_100u32)),
                250,
                initiation_ms,
                RepoGovernance::with_defaults(1_500, 86_400),
            )
            .execute(&ALICE_ID, &mut stx)
            .expect("repo execute");
            stx.apply();
            block.commit().expect("commit");
        }

        let settlement_ms = initiation_ms + super::MS_PER_DAY;
        let stored_agreement = {
            let view = state.view();
            view.world
                .repo_agreements()
                .get(&agreement_id)
                .cloned()
                .expect("agreement snapshot")
        };
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, settlement_ms, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let cash_spec = stx
            .numeric_spec_for(&cash_def_id)
            .expect("cash spec for settlement");
        let expected_cash = super::expected_cash_settlement(
            stored_agreement.cash_leg().quantity().clone(),
            *stored_agreement.rate_bps(),
            *stored_agreement.initiated_timestamp_ms(),
            settlement_ms,
            cash_spec,
        )
        .expect("interest calculation");
        let interest_due = expected_cash
            .clone()
            .checked_sub(stored_agreement.cash_leg().quantity().clone())
            .expect("interest non-negative");

        let alice_cash_id = AssetId::new(cash_def_id.clone(), ALICE_ID.clone());
        if !interest_due.is_zero() {
            stx.world
                .deposit_numeric_asset(&alice_cash_id, &interest_due)
                .expect("seed interest funds");
        }

        ReverseRepoIsi::new(
            agreement_id.clone(),
            ALICE_ID.clone(),
            BOB_ID.clone(),
            RepoCashLeg {
                asset_definition_id: cash_def_id.clone(),
                quantity: expected_cash.clone(),
            },
            stored_agreement.collateral_leg().clone(),
            settlement_ms,
        )
        .execute(&ALICE_ID, &mut stx)
        .expect("reverse repo execute");

        let mut initiator_event = None;
        let mut counterparty_event = None;
        let mut custodian_event = None;
        for event in &stx.world.internal_event_buf {
            if let DataEvent::Domain(DomainEvent::Account(AccountEvent::Repo(
                RepoAccountEvent::Settled(payload),
            ))) = event.as_ref()
            {
                match payload.role() {
                    RepoAccountRole::Initiator => initiator_event = Some(payload.clone()),
                    RepoAccountRole::Counterparty => counterparty_event = Some(payload.clone()),
                    RepoAccountRole::Custodian => custodian_event = Some(payload.clone()),
                }
            }
        }

        let initiator_event =
            initiator_event.expect("initiator should receive a repo settlement event");
        let counterparty_event =
            counterparty_event.expect("counterparty should receive a repo settlement event");
        let custodian_event =
            custodian_event.expect("custodian should receive a repo settlement event");

        assert_eq!(initiator_event.account().clone(), ALICE_ID.clone());
        assert_eq!(initiator_event.role(), &RepoAccountRole::Initiator);
        assert_eq!(initiator_event.cash_leg().quantity(), &expected_cash);
        assert_eq!(
            initiator_event.collateral_leg().quantity(),
            stored_agreement.collateral_leg().quantity()
        );
        assert_eq!(initiator_event.settled_timestamp_ms(), &settlement_ms);

        assert_eq!(counterparty_event.account().clone(), BOB_ID.clone());
        assert_eq!(counterparty_event.role(), &RepoAccountRole::Counterparty);
        assert_eq!(counterparty_event.cash_leg().quantity(), &expected_cash);
        assert_eq!(
            counterparty_event.collateral_leg().quantity(),
            stored_agreement.collateral_leg().quantity()
        );
        assert_eq!(counterparty_event.settled_timestamp_ms(), &settlement_ms);

        assert_eq!(custodian_event.account().clone(), custodian_id.clone());
        assert_eq!(custodian_event.role(), &RepoAccountRole::Custodian);
        assert_eq!(custodian_event.counterparty().clone(), ALICE_ID.clone());
        assert_eq!(custodian_event.cash_leg().quantity(), &expected_cash);
        assert_eq!(
            custodian_event.collateral_leg().quantity(),
            stored_agreement.collateral_leg().quantity()
        );
        assert_eq!(custodian_event.settled_timestamp_ms(), &settlement_ms);

        assert!(
            stx.world.repo_agreements.get(&agreement_id).is_none(),
            "agreement should be cleared during unwind"
        );

        let custodian_collateral_id = AssetId::new(collateral_def_id.clone(), custodian_id.clone());
        assert!(
            stx.world.assets.get(&custodian_collateral_id).is_none(),
            "custodian collateral should return to the initiator"
        );

        stx.apply();
        block.commit().expect("commit");

        let view = state.view();
        assert!(view.world.repo_agreements().get(&agreement_id).is_none());
    }

    #[test]
    fn repo_margin_call_updates_schedule_and_emits_events() {
        let (state, agreement_id, cash_def_id, collateral_def_id) = setup_state();

        {
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            repo_setup_instruction(&agreement_id, &cash_def_id, &collateral_def_id)
                .execute(&ALICE_ID, &mut stx)
                .expect("repo execute");
            stx.apply();
            block.commit().expect("commit");
        }

        let margin_timestamp_ms = super::MS_PER_DAY;
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, margin_timestamp_ms, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        RepoMarginCallIsi::new(agreement_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("margin call execute");

        let mut roles = Vec::new();
        for event in &stx.world.internal_event_buf {
            if let DataEvent::Domain(DomainEvent::Account(AccountEvent::Repo(
                RepoAccountEvent::MarginCalled(payload),
            ))) = event.as_ref()
            {
                roles.push((payload.account().clone(), *payload.role()));
                assert_eq!(payload.agreement_id(), &agreement_id);
                assert_eq!(payload.margin_timestamp_ms(), &margin_timestamp_ms);
            }
        }
        assert!(roles.contains(&(ALICE_ID.clone(), RepoAccountRole::Initiator)));
        assert!(roles.contains(&(BOB_ID.clone(), RepoAccountRole::Counterparty)));

        stx.apply();
        block.commit().expect("commit");

        let view = state.view();
        let recorded = view
            .world
            .repo_agreements()
            .get(&agreement_id)
            .expect("repo agreement persists");
        assert_eq!(
            recorded.last_margin_check_timestamp_ms(),
            &margin_timestamp_ms
        );
    }

    #[test]
    fn repo_margin_call_rejected_when_not_due() {
        let (state, agreement_id, cash_def_id, collateral_def_id) = setup_state();

        {
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            repo_setup_instruction(&agreement_id, &cash_def_id, &collateral_def_id)
                .execute(&ALICE_ID, &mut stx)
                .expect("repo execute");
            stx.apply();
            block.commit().expect("commit");
        }

        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, super::MS_PER_DAY / 2, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let err = RepoMarginCallIsi::new(agreement_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect_err("margin call should be rejected when not due");
        assert!(err.to_string().contains("margin check is not yet due"));
    }

    #[allow(clippy::too_many_lines)]
    fn capture_proof_stage(
        state: &State,
        stage_label: &'static str,
        tracked_assets: &[AssetId],
        agreement_id: &RepoAgreementId,
        base_timestamp_ms: u64,
    ) -> Value {
        fn string_value(value: &impl ToString) -> Value {
            Value::String(value.to_string())
        }
        fn number_value(value: u64) -> Value {
            Value::Number(Number::from(value))
        }

        let view = state.view();
        let agreement_snapshot = view.world.repo_agreements().get(agreement_id).cloned();
        let agreement_value = agreement_snapshot.map_or_else(
            || {
                let mut cleared = Map::new();
                cleared.insert("id".into(), string_value(agreement_id));
                cleared.insert("status".into(), Value::String("cleared".into()));
                Value::Object(cleared)
            },
            |agreement| {
                let mut obj = Map::new();
                obj.insert("id".into(), string_value(agreement.id()));
                obj.insert("initiator".into(), string_value(agreement.initiator()));
                obj.insert(
                    "counterparty".into(),
                    string_value(agreement.counterparty()),
                );
                obj.insert(
                    "custodian".into(),
                    agreement
                        .custodian()
                        .as_ref()
                        .map_or(Value::Null, string_value),
                );
                obj.insert(
                    "rate_bps".into(),
                    number_value(u64::from(*agreement.rate_bps())),
                );
                obj.insert(
                    "haircut_bps".into(),
                    number_value(u64::from(agreement.governance().haircut_bps())),
                );
                obj.insert(
                    "margin_frequency_secs".into(),
                    number_value(agreement.governance().margin_frequency_secs()),
                );
                obj.insert(
                    "initiated_offset_ms".into(),
                    number_value(
                        agreement
                            .initiated_timestamp_ms()
                            .saturating_sub(base_timestamp_ms),
                    ),
                );
                obj.insert(
                    "last_margin_offset_ms".into(),
                    number_value(
                        agreement
                            .last_margin_check_timestamp_ms()
                            .saturating_sub(base_timestamp_ms),
                    ),
                );
                obj.insert(
                    "maturity_offset_ms".into(),
                    number_value(
                        agreement
                            .maturity_timestamp_ms()
                            .saturating_sub(base_timestamp_ms),
                    ),
                );

                let mut cash = Map::new();
                cash.insert(
                    "asset".into(),
                    string_value(agreement.cash_leg().asset_definition_id()),
                );
                cash.insert(
                    "quantity".into(),
                    string_value(agreement.cash_leg().quantity()),
                );
                obj.insert("cash".into(), Value::Object(cash));

                let mut collateral = Map::new();
                collateral.insert(
                    "asset".into(),
                    string_value(agreement.collateral_leg().asset_definition_id()),
                );
                collateral.insert(
                    "quantity".into(),
                    string_value(agreement.collateral_leg().quantity()),
                );
                obj.insert("collateral".into(), Value::Object(collateral));

                Value::Object(obj)
            },
        );

        let mut sorted_assets = tracked_assets.to_vec();
        sorted_assets.sort();
        let assets = sorted_assets
            .into_iter()
            .map(|asset_id| {
                let value = view.world.assets().get(&asset_id);
                let present = value.is_some();
                let quantity = value.map_or_else(|| "0".to_string(), |asset| (**asset).to_string());
                let mut entry = Map::new();
                entry.insert("id".into(), string_value(&asset_id));
                entry.insert("present".into(), Value::Bool(present));
                entry.insert("quantity".into(), Value::String(quantity));
                Value::Object(entry)
            })
            .collect::<Vec<_>>();

        let mut frame = Map::new();
        frame.insert("stage".into(), Value::String(stage_label.to_string()));
        frame.insert("agreement".into(), agreement_value);
        frame.insert("assets".into(), Value::Array(assets));
        Value::Object(frame)
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn repo_deterministic_lifecycle_proof_matches_fixture() {
        const PROOF_FIXTURE_JSON: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/repo_lifecycle_proof.json"
        ));
        const PROOF_FIXTURE_DIGEST_HEX: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/repo_lifecycle_proof.digest"
        ));
        const BASE_TIMESTAMP_MS: u64 = 1_704_111_000_000;
        // Refresh the fixtures with `scripts/regen_repo_proof_fixture.sh` under
        // the pinned toolchain whenever repo semantics change so governance
        // evidence stays reproducible.

        let (state, agreement_id, cash_def_id, collateral_def_id) = setup_state();
        let tracked_assets = vec![
            AssetId::new(cash_def_id.clone(), ALICE_ID.clone()),
            AssetId::new(cash_def_id.clone(), BOB_ID.clone()),
            AssetId::new(collateral_def_id.clone(), ALICE_ID.clone()),
            AssetId::new(collateral_def_id.clone(), BOB_ID.clone()),
        ];
        let mut frames = Vec::new();

        {
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, BASE_TIMESTAMP_MS, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            repo_setup_instruction(&agreement_id, &cash_def_id, &collateral_def_id)
                .execute(&ALICE_ID, &mut stx)
                .expect("repo execute");
            stx.apply();
            block.commit().expect("commit");
        }
        frames.push(capture_proof_stage(
            &state,
            "repo_initiated",
            &tracked_assets,
            &agreement_id,
            BASE_TIMESTAMP_MS,
        ));

        {
            let margin_timestamp = BASE_TIMESTAMP_MS + super::MS_PER_DAY;
            let header = BlockHeader::new(nonzero!(2_u64), None, None, None, margin_timestamp, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            RepoMarginCallIsi::new(agreement_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect("margin call execute");
            stx.apply();
            block.commit().expect("commit");
        }
        frames.push(capture_proof_stage(
            &state,
            "margin_called",
            &tracked_assets,
            &agreement_id,
            BASE_TIMESTAMP_MS,
        ));

        {
            let unwind_timestamp = BASE_TIMESTAMP_MS + (2 * super::MS_PER_DAY);
            let stored_agreement = state
                .view()
                .world
                .repo_agreements()
                .get(&agreement_id)
                .cloned()
                .expect("agreement snapshot");
            let header = BlockHeader::new(nonzero!(3_u64), None, None, None, unwind_timestamp, 0);
            let mut block = state.block(header);
            let mut stx = block.transaction();
            let cash_spec = stx
                .numeric_spec_for(&cash_def_id)
                .expect("cash spec snapshot");
            let expected_cash = super::expected_cash_settlement(
                stored_agreement.cash_leg().quantity().clone(),
                *stored_agreement.rate_bps(),
                *stored_agreement.initiated_timestamp_ms(),
                unwind_timestamp,
                cash_spec,
            )
            .expect("interest calculation");
            let interest_due = expected_cash
                .clone()
                .checked_sub(stored_agreement.cash_leg().quantity().clone())
                .expect("interest non-negative");
            if !interest_due.is_zero() {
                let alice_cash = AssetId::new(cash_def_id.clone(), ALICE_ID.clone());
                stx.world
                    .deposit_numeric_asset(&alice_cash, &interest_due)
                    .expect("seed interest funds");
            }

            let extra_collateral = Numeric::from(75u32);
            let bob_collateral_id = AssetId::new(collateral_def_id.clone(), BOB_ID.clone());
            stx.world
                .deposit_numeric_asset(&bob_collateral_id, &extra_collateral)
                .expect("seed substitution collateral");
            let adjusted_collateral = stored_agreement
                .collateral_leg()
                .quantity()
                .clone()
                .checked_add(extra_collateral.clone())
                .expect("adjusted collateral fits");

            ReverseRepoIsi::new(
                agreement_id.clone(),
                ALICE_ID.clone(),
                BOB_ID.clone(),
                RepoCashLeg {
                    asset_definition_id: cash_def_id.clone(),
                    quantity: expected_cash.clone(),
                },
                RepoCollateralLeg::new(collateral_def_id.clone(), adjusted_collateral.clone()),
                unwind_timestamp,
            )
            .execute(&ALICE_ID, &mut stx)
            .expect("reverse repo execute");

            stx.apply();
            block.commit().expect("commit");
        }
        frames.push(capture_proof_stage(
            &state,
            "unwound_with_substitution",
            &tracked_assets,
            &agreement_id,
            BASE_TIMESTAMP_MS,
        ));

        let proof = norito::json!({
            "scenario": "repo_lifecycle_deterministic",
            "frames": frames,
        });

        let proof_bytes = norito::json::to_vec_pretty(&proof).expect("serialize proof");
        if let Ok(path) = std::env::var("REPO_PROOF_SNAPSHOT_OUT") {
            let snapshot_path = std::path::PathBuf::from(&path);
            if let Some(parent) = snapshot_path.parent() {
                std::fs::create_dir_all(parent).unwrap_or_else(|err| {
                    panic!("failed to create repo proof snapshot parent dirs: {err}");
                });
            }
            std::fs::write(&snapshot_path, &proof_bytes).unwrap_or_else(|err| {
                panic!("failed to write repo proof snapshot to {path}: {err}");
            });
        }
        let digest_hex = encode_upper(Hash::new(&proof_bytes).as_ref());
        if let Ok(path) = std::env::var("REPO_PROOF_DIGEST_OUT") {
            let digest_path = std::path::PathBuf::from(&path);
            if let Some(parent) = digest_path.parent() {
                std::fs::create_dir_all(parent).unwrap_or_else(|err| {
                    panic!("failed to create repo proof digest parent dirs: {err}");
                });
            }
            std::fs::write(&digest_path, format!("{digest_hex}\n")).unwrap_or_else(|err| {
                panic!("failed to write repo proof digest to {path}: {err}");
            });
        }
        let expected_proof: Value =
            norito::json::from_str(PROOF_FIXTURE_JSON).expect("repo proof fixture parses");
        let normalized_fixture_bytes =
            norito::json::to_vec_pretty(&expected_proof).expect("serialize fixture");
        let normalized_fixture_pretty =
            String::from_utf8(normalized_fixture_bytes.clone()).expect("fixture pretty JSON utf8");
        assert_eq!(
            PROOF_FIXTURE_JSON.trim_end_matches('\n'),
            normalized_fixture_pretty,
            "repo proof fixture formatting drifted; run scripts/regen_repo_proof_fixture.sh"
        );
        assert_eq!(
            proof, expected_proof,
            "repo lifecycle proof fixture mismatch; run scripts/regen_repo_proof_fixture.sh"
        );
        assert_eq!(
            proof_bytes, normalized_fixture_bytes,
            "repo lifecycle proof snapshot drifted; run scripts/regen_repo_proof_fixture.sh"
        );
        let expected_digest_hex = PROOF_FIXTURE_DIGEST_HEX.trim();
        let fixture_digest_hex = encode_upper(Hash::new(&normalized_fixture_bytes).as_ref());
        assert_eq!(
            fixture_digest_hex, expected_digest_hex,
            "repo proof digest fixture is out of sync; run scripts/regen_repo_proof_fixture.sh"
        );
        assert_eq!(
            digest_hex, expected_digest_hex,
            "repo lifecycle proof digest snapshot mismatch: {digest_hex}"
        );
    }
}
