//! On-chain oracle instruction handlers.

#[cfg(feature = "telemetry")]
use std::time::Instant;
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
};

use blake3::Hasher;
use iroha_data_model::{
    events::data::oracle::{
        OracleChangeProposed, OracleChangeStageUpdated, OracleEvent, TwitterBindingRecorded,
        TwitterBindingRevoked,
    },
    isi::{
        error::{InstructionExecutionError as Error, InvalidParameterError},
        oracle::{
            AggregateOracleFeed, OpenOracleDispute, ProposeOracleChange, RecordTwitterBinding,
            RegisterOracleFeed, ResolveOracleDispute, RevokeTwitterBinding, RollbackOracleChange,
            SubmitOracleObservation, VoteOracleChangeStage,
        },
    },
    oracle::{
        FeedConfigVersion, FeedEventOutcome, FeedId, FeedSlot, Observation, ObservationOutcome,
        OracleChangeEvidence, OracleChangeFailure, OracleChangeProposal, OracleChangeStage,
        OracleChangeStageFailure, OracleChangeStageRecord, OracleChangeStatus, OracleDispute,
        OracleDisputeId, OracleDisputeOutcome, OracleDisputeStatus, OraclePenalty,
        OraclePenaltyKind, OracleProviderKey, OracleProviderStats, OracleReward,
        TwitterBindingAttestation, TwitterBindingRecord,
    },
    prelude::*,
};
use iroha_primitives::numeric::Numeric;

use super::prelude::*;
use crate::{
    oracle::{FeedEventRecord, ObservationWindow, ObservationWindowKey},
    state::{StateTransaction, WorldTransaction},
};

fn aggregation_err(err: &iroha_data_model::oracle::OracleAggregationError) -> Error {
    Error::InvalidParameter(InvalidParameterError::SmartContract(err.to_string()))
}

fn signature_err(message: impl Into<String>) -> Error {
    Error::InvalidParameter(InvalidParameterError::SmartContract(message.into()))
}

fn ensure_provider_allowed(
    config: &iroha_data_model::oracle::FeedConfig,
    provider: &AccountId,
) -> Result<(), Error> {
    if config.providers.contains(provider) {
        Ok(())
    } else {
        Err(signature_err(format!(
            "oracle provider `{provider}` is not part of feed `{}`",
            config.feed_id.as_str()
        )))
    }
}

fn expected_twitter_feed_id(
    cfg: &iroha_config::parameters::actual::OracleTwitterBinding,
) -> FeedId {
    FeedId(cfg.feed_id.clone())
}

fn validate_binding_ttl(
    attestation: &TwitterBindingAttestation,
    cfg: &iroha_config::parameters::actual::OracleTwitterBinding,
) -> Result<(), Error> {
    if attestation.expires_at_ms <= attestation.observed_at_ms {
        return Err(signature_err(
            "twitter binding expiry must be greater than observed_at_ms",
        ));
    }
    let ttl = attestation
        .expires_at_ms
        .saturating_sub(attestation.observed_at_ms);
    if ttl > cfg.max_ttl_ms {
        return Err(signature_err(format!(
            "twitter binding TTL {}ms exceeds max {}ms",
            ttl, cfg.max_ttl_ms
        )));
    }
    if ttl < cfg.min_ttl_ms {
        return Err(signature_err(format!(
            "twitter binding TTL {}ms is below min {}ms",
            ttl, cfg.min_ttl_ms
        )));
    }
    Ok(())
}

fn ensure_binding_matches_history(
    state_transaction: &StateTransaction<'_, '_>,
    feed_id: &FeedId,
    attestation: &TwitterBindingAttestation,
) -> Result<(), Error> {
    let history = state_transaction
        .world
        .oracle_history
        .get(feed_id)
        .cloned()
        .unwrap_or_default();
    let expected_value = attestation.observation_value();
    let found = history.into_iter().rev().any(|record| {
        record.event.feed_config_version == attestation.feed_config_version
            && record.event.slot == attestation.slot
            && matches!(
                record.event.outcome,
                FeedEventOutcome::Success(ref success) if success.value == expected_value
            )
    });

    if found {
        Ok(())
    } else {
        Err(signature_err(format!(
            "no oracle feed event found for slot {} and binding hash in feed `{}`",
            attestation.slot,
            feed_id.as_str()
        )))
    }
}

fn validate_feed_registration(
    feed: &iroha_data_model::oracle::FeedConfig,
    world: &WorldTransaction<'_, '_>,
) -> Result<(), Error> {
    // Enforce provider uniqueness and reasonable caps.
    let mut providers = BTreeSet::new();
    for provider in &feed.providers {
        if !providers.insert(provider.clone()) {
            return Err(signature_err(format!(
                "duplicate oracle provider `{provider}` in feed `{}`",
                feed.feed_id.as_str()
            )));
        }
    }
    if providers.is_empty() {
        return Err(signature_err(format!(
            "feed `{}` must list at least one provider",
            feed.feed_id.as_str()
        )));
    }
    if providers.len() > usize::from(feed.max_observers) {
        return Err(signature_err(format!(
            "feed `{}` allows {}/{} observers",
            feed.feed_id.as_str(),
            providers.len(),
            feed.max_observers
        )));
    }
    if usize::from(feed.min_signers) > providers.len() {
        return Err(signature_err(format!(
            "feed `{}` min_signers {} exceeds providers {}",
            feed.feed_id.as_str(),
            feed.min_signers,
            providers.len()
        )));
    }

    if let Some(existing) = world.oracle_feeds.get(&feed.feed_id) {
        if existing.feed_config_version >= feed.feed_config_version {
            return Err(signature_err(format!(
                "feed `{}` already registered with version {} (incoming {})",
                feed.feed_id.as_str(),
                existing.feed_config_version.0,
                feed.feed_config_version.0
            )));
        }
    }

    Ok(())
}

fn stage_deadline(
    stage: iroha_data_model::oracle::OracleChangeStage,
    cfg: &iroha_config::parameters::actual::OracleGovernance,
    started_at: u64,
) -> Option<u64> {
    let sla = match stage {
        iroha_data_model::oracle::OracleChangeStage::Intake => cfg.intake_sla_blocks,
        iroha_data_model::oracle::OracleChangeStage::RulesCommittee => cfg.rules_sla_blocks,
        iroha_data_model::oracle::OracleChangeStage::CopReview => cfg.cop_sla_blocks,
        iroha_data_model::oracle::OracleChangeStage::TechnicalAudit => cfg.technical_sla_blocks,
        iroha_data_model::oracle::OracleChangeStage::PolicyJury => cfg.policy_jury_sla_blocks,
        iroha_data_model::oracle::OracleChangeStage::Enactment => cfg.enact_sla_blocks,
    };
    if sla == 0 {
        None
    } else {
        Some(started_at.saturating_add(sla))
    }
}

fn seed_change_stages(
    created_at: u64,
    cfg: &iroha_config::parameters::actual::OracleGovernance,
) -> Vec<OracleChangeStageRecord> {
    use iroha_data_model::oracle::OracleChangeStage as Stage;
    let mut stages = Vec::with_capacity(6);
    stages.push(OracleChangeStageRecord {
        stage: Stage::Intake,
        approvals: BTreeSet::new(),
        rejections: BTreeSet::new(),
        evidence: Vec::new(),
        started_at: Some(created_at),
        deadline: stage_deadline(Stage::Intake, cfg, created_at),
        completed_at: None,
        failure: None,
    });
    for stage in [
        Stage::RulesCommittee,
        Stage::CopReview,
        Stage::TechnicalAudit,
        Stage::PolicyJury,
        Stage::Enactment,
    ] {
        stages.push(OracleChangeStageRecord {
            stage,
            approvals: BTreeSet::new(),
            rejections: BTreeSet::new(),
            evidence: Vec::new(),
            started_at: None,
            deadline: None,
            completed_at: None,
            failure: None,
        });
    }
    stages
}

fn required_votes(
    stage: iroha_data_model::oracle::OracleChangeStage,
    class: iroha_data_model::oracle::OracleChangeClass,
    cfg: &iroha_config::parameters::actual::OracleGovernance,
) -> NonZeroUsize {
    use iroha_data_model::oracle::OracleChangeStage as Stage;
    match stage {
        Stage::Intake => cfg.intake_min_votes,
        Stage::RulesCommittee => cfg.rules_min_votes,
        Stage::CopReview => class.required_votes(
            cfg.cop_min_votes.low,
            cfg.cop_min_votes.medium,
            cfg.cop_min_votes.high,
        ),
        Stage::TechnicalAudit => cfg.technical_min_votes,
        Stage::PolicyJury => class.required_votes(
            cfg.policy_jury_min_votes.low,
            cfg.policy_jury_min_votes.medium,
            cfg.policy_jury_min_votes.high,
        ),
        Stage::Enactment => NonZeroUsize::new(1).expect("non-zero"),
    }
}

fn append_stage_evidence(
    proposal: &mut iroha_data_model::oracle::OracleChangeProposal,
    stage: iroha_data_model::oracle::OracleChangeStage,
    evidence_hashes: &[Hash],
) {
    if evidence_hashes.is_empty() {
        return;
    }
    if let Some(record) = proposal
        .stages
        .iter_mut()
        .find(|record| record.stage == stage)
    {
        for hash in evidence_hashes {
            if !record.evidence.contains(hash) {
                record.evidence.push(*hash);
            }
            proposal.evidence.push(OracleChangeEvidence {
                stage,
                evidence_hash: *hash,
                note: None,
            });
        }
    }
}

#[derive(Clone, Copy)]
enum ProviderOutcome {
    Inlier,
    Outlier,
    Error,
    NoShow,
}

fn classify_providers(
    config: &iroha_data_model::oracle::FeedConfig,
    observations: &[Observation],
    outcome: &iroha_data_model::oracle::FeedEventOutcome,
) -> BTreeMap<AccountId, ProviderOutcome> {
    let mut map = BTreeMap::new();
    for obs in observations {
        match obs.body.outcome {
            ObservationOutcome::Value(_) => {
                map.insert(obs.body.provider_id.clone(), ProviderOutcome::Inlier);
            }
            ObservationOutcome::Error(_) => {
                map.insert(obs.body.provider_id.clone(), ProviderOutcome::Error);
            }
        }
    }

    if let iroha_data_model::oracle::FeedEventOutcome::Success(success) = outcome {
        for entry in &success.entries {
            map.insert(
                entry.oracle_id.clone(),
                if entry.outlier {
                    ProviderOutcome::Outlier
                } else {
                    ProviderOutcome::Inlier
                },
            );
        }
    }

    for provider in &config.providers {
        map.entry(provider.clone())
            .or_insert(ProviderOutcome::NoShow);
    }

    map
}

fn with_provider_stats_mut<F>(
    world: &mut WorldTransaction<'_, '_>,
    feed_id: &FeedId,
    provider: &AccountId,
    mut f: F,
) where
    F: FnMut(&mut OracleProviderStats),
{
    let key = OracleProviderKey::new(feed_id.clone(), provider.clone());
    let mut stats = world
        .oracle_provider_stats
        .get(&key)
        .copied()
        .unwrap_or_default();
    f(&mut stats);
    world.oracle_provider_stats.insert(key, stats);
}

fn reward_provider(
    state_transaction: &mut StateTransaction<'_, '_>,
    feed_id: &FeedId,
    feed_config_version: FeedConfigVersion,
    slot: FeedSlot,
    request_hash: Hash,
    provider: &AccountId,
) -> Result<(), Error> {
    let economics = &state_transaction.oracle.economics;
    let amount = economics.reward_amount.clone();
    let pool_asset = AssetId::new(
        economics.reward_asset.clone(),
        economics.reward_pool.clone(),
    );
    let provider_asset = AssetId::new(economics.reward_asset.clone(), provider.clone());

    if amount.is_zero() {
        return Ok(());
    }

    state_transaction
        .world
        .withdraw_numeric_asset(&pool_asset, &amount)?;
    state_transaction
        .world
        .deposit_numeric_asset(&provider_asset, &amount)?;

    with_provider_stats_mut(&mut state_transaction.world, feed_id, provider, |stats| {
        stats.inliers = stats.inliers.saturating_add(1);
        stats.record_reward();
    });

    state_transaction
        .world
        .emit_events(Some(OracleEvent::RewardApplied(OracleReward {
            feed_id: feed_id.clone(),
            feed_config_version,
            slot,
            request_hash,
            oracle_id: provider.clone(),
            amount: amount.clone(),
        })));

    #[cfg(feature = "telemetry")]
    {
        state_transaction
            .telemetry
            .observe_oracle_reward(feed_id, &amount);
    }

    Ok(())
}

fn slash_provider(
    state_transaction: &mut StateTransaction<'_, '_>,
    feed_id: &FeedId,
    ctx: (FeedConfigVersion, FeedSlot, Hash),
    provider: &AccountId,
    kind: OraclePenaltyKind,
    amount: &Numeric,
) -> Result<(), Error> {
    let (feed_config_version, slot, request_hash) = ctx;
    let economics = &state_transaction.oracle.economics;
    let provider_asset = AssetId::new(economics.slash_asset.clone(), provider.clone());
    let receiver_asset = AssetId::new(
        economics.slash_asset.clone(),
        economics.slash_receiver.clone(),
    );

    if amount.is_zero() {
        return Ok(());
    }

    state_transaction
        .world
        .withdraw_numeric_asset(&provider_asset, amount)?;
    state_transaction
        .world
        .deposit_numeric_asset(&receiver_asset, amount)?;

    with_provider_stats_mut(&mut state_transaction.world, feed_id, provider, |stats| {
        match kind {
            OraclePenaltyKind::Outlier => {
                stats.outliers = stats.outliers.saturating_add(1);
            }
            OraclePenaltyKind::Error => {
                stats.errors = stats.errors.saturating_add(1);
            }
            OraclePenaltyKind::NoShow => {
                stats.no_shows = stats.no_shows.saturating_add(1);
            }
            OraclePenaltyKind::BadSignature | OraclePenaltyKind::Dispute => {}
        }
        stats.record_slash();
    });

    state_transaction
        .world
        .emit_events(Some(OracleEvent::PenaltyApplied(OraclePenalty {
            feed_id: feed_id.clone(),
            feed_config_version,
            slot,
            request_hash,
            oracle_id: provider.clone(),
            kind,
            amount: amount.clone(),
        })));

    #[cfg(feature = "telemetry")]
    {
        state_transaction
            .telemetry
            .observe_oracle_penalty(feed_id, amount, kind);
    }

    Ok(())
}

fn apply_economics_for_slot(
    state_transaction: &mut StateTransaction<'_, '_>,
    config: &iroha_data_model::oracle::FeedConfig,
    observations: &[Observation],
    outcome: &iroha_data_model::oracle::FeedEventOutcome,
    slot: FeedSlot,
    request_hash: Hash,
) -> Result<(), Error> {
    let feed_version = config.feed_config_version;
    let classifications = classify_providers(config, observations, outcome);
    let economics = state_transaction.oracle.economics.clone();
    for (provider, class) in classifications {
        match class {
            ProviderOutcome::Inlier => reward_provider(
                state_transaction,
                &config.feed_id,
                feed_version,
                slot,
                request_hash,
                &provider,
            )?,
            ProviderOutcome::Outlier => slash_provider(
                state_transaction,
                &config.feed_id,
                (feed_version, slot, request_hash),
                &provider,
                OraclePenaltyKind::Outlier,
                &economics.slash_outlier_amount,
            )?,
            ProviderOutcome::Error => slash_provider(
                state_transaction,
                &config.feed_id,
                (feed_version, slot, request_hash),
                &provider,
                OraclePenaltyKind::Error,
                &economics.slash_error_amount,
            )?,
            ProviderOutcome::NoShow => slash_provider(
                state_transaction,
                &config.feed_id,
                (feed_version, slot, request_hash),
                &provider,
                OraclePenaltyKind::NoShow,
                &economics.slash_no_show_amount,
            )?,
        }
    }

    Ok(())
}

fn derive_dispute_id(
    feed_id: &FeedId,
    slot: FeedSlot,
    request_hash: Hash,
    challenger: &AccountId,
    target: &AccountId,
) -> OracleDisputeId {
    let mut hasher = Hasher::new();
    hasher.update(feed_id.as_str().as_bytes());
    hasher.update(&slot.to_le_bytes());
    hasher.update(request_hash.as_ref());
    hasher.update(challenger.to_string().as_bytes());
    hasher.update(target.to_string().as_bytes());
    let digest = hasher.finalize();
    let mut buf = [0_u8; 8];
    buf.copy_from_slice(&digest.as_bytes()[..8]);
    OracleDisputeId(u64::from_le_bytes(buf))
}

impl Execute for RegisterOracleFeed {
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let feed = self.feed;

        validate_feed_registration(&feed, &state_transaction.world)?;

        // Persist the new configuration.
        state_transaction
            .world
            .oracle_feeds
            .insert(feed.feed_id.clone(), feed);

        Ok(())
    }
}

impl Execute for SubmitOracleObservation {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let observation = self.observation;
        let provider = &observation.body.provider_id;

        if provider != authority {
            return Err(signature_err(format!(
                "authority `{authority}` is not the observation provider `{provider}`"
            )));
        }

        let config = state_transaction
            .world
            .oracle_feeds
            .get(&observation.body.feed_id)
            .cloned()
            .ok_or_else(|| {
                signature_err(format!(
                    "oracle feed `{}` is not registered",
                    observation.body.feed_id.as_str()
                ))
            })?;

        ensure_provider_allowed(&config, provider)?;

        // Verify the provider signature over the observation payload.
        let signatory = provider.controller().single_signatory().ok_or_else(|| {
            signature_err("oracle providers must use single-signature controllers")
        })?;
        if let Err(err) = observation.signature.verify(signatory, &observation.body) {
            let slash_amount = state_transaction
                .oracle
                .economics
                .slash_error_amount
                .clone();
            let _ = slash_provider(
                state_transaction,
                &observation.body.feed_id,
                (
                    config.feed_config_version,
                    observation.body.slot,
                    observation.body.request_hash,
                ),
                provider,
                OraclePenaltyKind::BadSignature,
                &slash_amount,
            );
            return Err(signature_err(format!(
                "invalid observation signature: {err}"
            )));
        }

        // Buffer the observation for its `(feed, slot, request_hash)` window.
        let key = ObservationWindowKey::new(
            observation.body.feed_id.clone(),
            observation.body.feed_config_version,
            observation.body.slot,
            observation.body.request_hash,
        );
        let mut window = state_transaction
            .world
            .oracle_observations
            .get(&key)
            .cloned()
            .unwrap_or_else(|| ObservationWindow::new(&key));

        window
            .push(&config, observation)
            .map_err(|err| aggregation_err(&err))?;

        state_transaction
            .world
            .oracle_observations
            .insert(key, window);
        Ok(())
    }
}

impl Execute for AggregateOracleFeed {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let config = state_transaction
            .world
            .oracle_feeds
            .get(&self.feed_id)
            .cloned()
            .ok_or_else(|| {
                signature_err(format!(
                    "oracle feed `{}` is not registered",
                    self.feed_id.as_str()
                ))
            })?;

        ensure_provider_allowed(&config, authority)?;

        let key = ObservationWindowKey::new(
            self.feed_id.clone(),
            config.feed_config_version,
            self.slot,
            self.request_hash,
        );
        let window = state_transaction
            .world
            .oracle_observations
            .remove(key.clone())
            .unwrap_or_else(|| ObservationWindow::new(&key));

        let observations = window.into_sorted();
        #[cfg(feature = "telemetry")]
        let started = Instant::now();
        let output = crate::oracle::aggregate(
            &config,
            self.slot,
            self.request_hash,
            authority.clone(),
            &observations,
        )
        .map_err(|err| aggregation_err(&err))?;

        let record = FeedEventRecord {
            event: output.into_feed_event(),
            evidence_hashes: self.evidence_hashes,
        };

        apply_economics_for_slot(
            state_transaction,
            &config,
            &observations,
            &record.event.outcome,
            self.slot,
            self.request_hash,
        )?;

        // Persist bounded history (per feed).
        let mut history = state_transaction
            .world
            .oracle_history
            .get(&self.feed_id)
            .cloned()
            .unwrap_or_default();
        history.push(record.clone());
        let max_history = state_transaction.oracle.history_depth.get();
        if history.len() > max_history {
            let excess = history.len() - max_history;
            history.drain(0..excess);
        }
        state_transaction
            .world
            .oracle_history
            .insert(self.feed_id.clone(), history);

        // Emit data event for subscribers.
        state_transaction
            .world
            .emit_events(Some(OracleEvent::FeedProcessed(record.clone())));

        #[cfg(feature = "telemetry")]
        {
            state_transaction.telemetry.observe_oracle_aggregation(
                &self.feed_id,
                observations.len() as u64,
                &record.evidence_hashes,
                started.elapsed(),
            );
        }

        Ok(())
    }
}

impl Execute for OpenOracleDispute {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let config = state_transaction
            .world
            .oracle_feeds
            .get(&self.feed_id)
            .ok_or_else(|| {
                signature_err(format!(
                    "oracle feed `{}` is not registered",
                    self.feed_id.as_str()
                ))
            })?;

        ensure_provider_allowed(config, &self.target)?;

        let economics = state_transaction.oracle.economics.clone();
        let bond_amount = self
            .bond
            .unwrap_or_else(|| economics.dispute_bond_amount.clone());

        let challenger_asset =
            AssetId::new(economics.dispute_bond_asset.clone(), authority.clone());
        let escrow_asset = AssetId::new(
            economics.dispute_bond_asset.clone(),
            economics.slash_receiver.clone(),
        );

        state_transaction
            .world
            .withdraw_numeric_asset(&challenger_asset, &bond_amount)?;
        state_transaction
            .world
            .deposit_numeric_asset(&escrow_asset, &bond_amount)?;

        let dispute_id = derive_dispute_id(
            &self.feed_id,
            self.slot,
            self.request_hash,
            authority,
            &self.target,
        );
        if state_transaction
            .world
            .oracle_disputes
            .get(&dispute_id)
            .is_some()
        {
            return Err(signature_err(format!(
                "dispute `{}` already exists",
                dispute_id.0
            )));
        }

        let dispute = OracleDispute {
            id: dispute_id,
            feed_id: self.feed_id,
            slot: self.slot,
            request_hash: self.request_hash,
            challenger: authority.clone(),
            target: self.target,
            bond: bond_amount,
            evidence: self.evidence_hashes,
            reason: self.reason,
            status: OracleDisputeStatus::Open,
        };

        state_transaction
            .world
            .oracle_disputes
            .insert(dispute_id, dispute.clone());
        state_transaction
            .world
            .emit_events(Some(OracleEvent::DisputeOpened(dispute)));
        Ok(())
    }
}

struct DisputeAssets {
    escrow: AssetId,
    challenger: AssetId,
    target: AssetId,
}

impl DisputeAssets {
    fn new(
        economics: &iroha_config::parameters::actual::OracleEconomics,
        challenger: &AccountId,
        target: &AccountId,
    ) -> Self {
        let escrow = AssetId::new(
            economics.dispute_bond_asset.clone(),
            economics.slash_receiver.clone(),
        );
        let challenger = AssetId::new(economics.dispute_bond_asset.clone(), challenger.clone());
        let target = AssetId::new(economics.dispute_bond_asset.clone(), target.clone());
        Self {
            escrow,
            challenger,
            target,
        }
    }
}

struct DisputeEconomics {
    outlier_slash: Numeric,
    error_slash: Numeric,
    reward: Numeric,
    frivolous_slash: Numeric,
}

impl DisputeEconomics {
    fn from_config(economics: &iroha_config::parameters::actual::OracleEconomics) -> Self {
        Self {
            outlier_slash: economics.slash_outlier_amount.clone(),
            error_slash: economics.slash_error_amount.clone(),
            reward: economics.dispute_reward_amount.clone(),
            frivolous_slash: economics.frivolous_slash_amount.clone(),
        }
    }
}

fn resolve_upheld_dispute(
    state_transaction: &mut StateTransaction<'_, '_>,
    dispute: &OracleDispute,
    ctx: (FeedConfigVersion, FeedSlot, Hash),
    economics: &DisputeEconomics,
    assets: &DisputeAssets,
) -> Result<(), Error> {
    slash_provider(
        state_transaction,
        &dispute.feed_id,
        ctx,
        &dispute.target,
        OraclePenaltyKind::Dispute,
        &economics.outlier_slash,
    )?;
    move_from_escrow(state_transaction, assets, &dispute.bond, &assets.challenger)?;
    move_from_escrow(
        state_transaction,
        assets,
        &economics.reward,
        &assets.challenger,
    )
}

fn resolve_reduced_dispute(
    state_transaction: &mut StateTransaction<'_, '_>,
    dispute: &OracleDispute,
    ctx: (FeedConfigVersion, FeedSlot, Hash),
    economics: &DisputeEconomics,
    assets: &DisputeAssets,
) -> Result<(), Error> {
    slash_provider(
        state_transaction,
        &dispute.feed_id,
        ctx,
        &dispute.target,
        OraclePenaltyKind::Dispute,
        &economics.error_slash,
    )?;
    move_from_escrow(state_transaction, assets, &dispute.bond, &assets.challenger)
}

fn resolve_frivolous_dispute(
    state_transaction: &mut StateTransaction<'_, '_>,
    economics: &DisputeEconomics,
    assets: &DisputeAssets,
) -> Result<(), Error> {
    move_from_escrow(
        state_transaction,
        assets,
        &economics.frivolous_slash,
        &assets.target,
    )
}

fn move_from_escrow(
    state_transaction: &mut StateTransaction<'_, '_>,
    assets: &DisputeAssets,
    amount: &Numeric,
    destination: &AssetId,
) -> Result<(), Error> {
    state_transaction
        .world
        .withdraw_numeric_asset(&assets.escrow, amount)?;
    state_transaction
        .world
        .deposit_numeric_asset(destination, amount)
}

impl Execute for ResolveOracleDispute {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut dispute) = state_transaction
            .world
            .oracle_disputes
            .get(&self.dispute_id)
            .cloned()
        else {
            return Err(signature_err(format!(
                "dispute `{}` does not exist",
                self.dispute_id.0
            )));
        };

        if !matches!(dispute.status, OracleDisputeStatus::Open) {
            return Err(signature_err(format!(
                "dispute `{}` already resolved",
                dispute.id.0
            )));
        }

        let economics_cfg = state_transaction.oracle.economics.clone();
        let config = state_transaction
            .world
            .oracle_feeds
            .get(&dispute.feed_id)
            .cloned()
            .ok_or_else(|| {
                signature_err(format!(
                    "oracle feed `{}` is not registered",
                    dispute.feed_id.as_str()
                ))
            })?;
        let assets = DisputeAssets::new(&economics_cfg, &dispute.challenger, &dispute.target);
        let economics = DisputeEconomics::from_config(&economics_cfg);
        let ctx = (
            config.feed_config_version,
            dispute.slot,
            dispute.request_hash,
        );

        match self.outcome {
            OracleDisputeOutcome::Upheld => {
                resolve_upheld_dispute(state_transaction, &dispute, ctx, &economics, &assets)?
            }
            OracleDisputeOutcome::Reduced => {
                resolve_reduced_dispute(state_transaction, &dispute, ctx, &economics, &assets)?
            }
            OracleDisputeOutcome::Frivolous => {
                resolve_frivolous_dispute(state_transaction, &economics, &assets)?
            }
        }

        dispute.status = OracleDisputeStatus::Resolved(self.outcome);
        state_transaction
            .world
            .oracle_disputes
            .insert(dispute.id, dispute.clone());
        state_transaction
            .world
            .emit_events(Some(OracleEvent::DisputeResolved(dispute)));
        Ok(())
    }
}

impl Execute for ProposeOracleChange {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if state_transaction
            .world
            .oracle_changes
            .get(&self.change_id)
            .is_some()
        {
            return Err(signature_err(format!(
                "oracle change `{}` already exists",
                self.change_id.as_hash()
            )));
        }

        validate_feed_registration(&self.feed, &state_transaction.world)?;
        let created_at = state_transaction.block_height();
        let mut proposal = OracleChangeProposal {
            id: self.change_id,
            feed: self.feed,
            class: self.class,
            payload_hash: self.payload_hash,
            proposer: authority.clone(),
            created_at,
            evidence: Vec::new(),
            stages: seed_change_stages(created_at, &state_transaction.oracle.governance),
            status: OracleChangeStatus::Pending,
        };
        append_stage_evidence(
            &mut proposal,
            OracleChangeStage::Intake,
            &self.evidence_hashes,
        );

        state_transaction
            .world
            .oracle_changes
            .insert(self.change_id, proposal.clone());
        state_transaction
            .world
            .emit_events(Some(OracleEvent::ChangeProposed(OracleChangeProposed {
                change_id: self.change_id,
                feed_id: proposal.feed.feed_id.clone(),
                class: proposal.class,
                payload_hash: proposal.payload_hash,
                proposer: authority.clone(),
            })));

        if let Some(intake) = proposal
            .stages
            .iter()
            .find(|stage| stage.stage == OracleChangeStage::Intake)
        {
            state_transaction
                .world
                .emit_events(Some(OracleEvent::ChangeStageUpdated(
                    OracleChangeStageUpdated {
                        change_id: proposal.id,
                        stage: OracleChangeStage::Intake,
                        status: proposal.status.clone(),
                        approvals: u32::try_from(intake.approvals.len()).unwrap_or(u32::MAX),
                        rejections: u32::try_from(intake.rejections.len()).unwrap_or(u32::MAX),
                        evidence_hashes: intake.evidence.clone(),
                    },
                )));
        }

        Ok(())
    }
}

fn start_stage_if_needed(
    proposal: &mut OracleChangeProposal,
    stage: OracleChangeStage,
    now: u64,
    cfg: &iroha_config::parameters::actual::OracleGovernance,
) {
    if let Some(record) = proposal
        .stages
        .iter_mut()
        .find(|record| record.stage == stage)
    {
        if record.started_at.is_none() {
            record.started_at = Some(now);
            record.deadline = stage_deadline(stage, cfg, now);
        }
    }
}

fn enact_change(
    state_transaction: &mut StateTransaction<'_, '_>,
    proposal: &OracleChangeProposal,
) -> Result<(), Error> {
    validate_feed_registration(&proposal.feed, &state_transaction.world)?;
    state_transaction
        .world
        .oracle_feeds
        .insert(proposal.feed.feed_id.clone(), proposal.feed.clone());
    Ok(())
}

impl Execute for VoteOracleChangeStage {
    #[allow(clippy::too_many_lines)]
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut proposal) = state_transaction
            .world
            .oracle_changes
            .get(&self.change_id)
            .cloned()
        else {
            return Err(signature_err(format!(
                "oracle change `{}` is not registered",
                self.change_id.as_hash()
            )));
        };

        if proposal.status.is_terminal() {
            return Err(signature_err(format!(
                "oracle change `{}` is already finalized",
                proposal.id.as_hash()
            )));
        }

        let now = state_transaction.block_height();
        if proposal.stages.is_empty() {
            proposal.stages = seed_change_stages(now, &state_transaction.oracle.governance);
        }
        start_stage_if_needed(
            &mut proposal,
            self.stage,
            now,
            &state_transaction.oracle.governance,
        );
        append_stage_evidence(&mut proposal, self.stage, &self.evidence_hashes);

        let stage_idx = proposal
            .stages
            .iter()
            .position(|record| record.stage == self.stage)
            .ok_or_else(|| {
                signature_err(format!(
                    "stage {:?} is not part of the oracle change pipeline",
                    self.stage
                ))
            })?;
        let mut next_stage = None;
        let mut should_enact = false;
        let mut status_update: Option<OracleChangeStatus> = None;
        let approvals;
        let rejections;
        let evidence_hashes;

        {
            let stage_rec = proposal
                .stages
                .get_mut(stage_idx)
                .expect("stage exists by index");

            if !stage_rec.is_pending() {
                return Err(signature_err("stage already completed or failed"));
            }

            if self.approve {
                stage_rec.approvals.insert(authority.clone());
                let needed = required_votes(
                    stage_rec.stage,
                    proposal.class,
                    &state_transaction.oracle.governance,
                );
                if stage_rec.approvals.len() >= needed.get() {
                    stage_rec.completed_at = Some(now);
                    if let Some(next) = stage_rec.stage.next() {
                        next_stage = Some(next);
                        if matches!(next, OracleChangeStage::Enactment) {
                            should_enact = true;
                        }
                    } else {
                        should_enact = true;
                    }
                }
            } else {
                stage_rec.rejections.insert(authority.clone());
                stage_rec.failure = Some(OracleChangeStageFailure::Rejected);
                stage_rec.completed_at.get_or_insert(now);
                status_update = Some(OracleChangeStatus::Failed(OracleChangeFailure {
                    stage: stage_rec.stage,
                    reason: OracleChangeStageFailure::Rejected,
                    at: now,
                }));
            }

            approvals = u32::try_from(stage_rec.approvals.len()).unwrap_or(u32::MAX);
            rejections = u32::try_from(stage_rec.rejections.len()).unwrap_or(u32::MAX);
            evidence_hashes = stage_rec.evidence.clone();
        }

        if let Some(next) = next_stage {
            start_stage_if_needed(
                &mut proposal,
                next,
                now,
                &state_transaction.oracle.governance,
            );
        }

        if self.approve && should_enact {
            enact_change(state_transaction, &proposal)?;
            if let Some(enact) = proposal
                .stages
                .iter_mut()
                .find(|record| record.stage == OracleChangeStage::Enactment)
            {
                enact.started_at.get_or_insert(now);
                if let Some(deadline) = stage_deadline(
                    OracleChangeStage::Enactment,
                    &state_transaction.oracle.governance,
                    now,
                ) {
                    enact.deadline.get_or_insert(deadline);
                }
                enact.completed_at.get_or_insert(now);
            }
            status_update = Some(OracleChangeStatus::Enacted(now));
        }

        if let Some(status) = status_update {
            proposal.status = status;
        }

        state_transaction
            .world
            .oracle_changes
            .insert(self.change_id, proposal.clone());
        state_transaction
            .world
            .emit_events(Some(OracleEvent::ChangeStageUpdated(
                OracleChangeStageUpdated {
                    change_id: self.change_id,
                    stage: self.stage,
                    status: proposal.status.clone(),
                    approvals,
                    rejections,
                    evidence_hashes,
                },
            )));

        Ok(())
    }
}

/// Oracle-specific query handlers.
pub mod query {
    use eyre::Result;
    use iroha_data_model::{
        oracle::TwitterBindingRecord,
        query::{
            error::{FindError, QueryExecutionFail as Error},
            oracle::prelude::FindTwitterBindingByHash,
        },
    };
    use iroha_telemetry::metrics;
    use mv::storage::StorageReadOnly;

    use crate::{
        smartcontracts::ValidSingularQuery,
        state::{StateReadOnly, WorldReadOnly},
    };

    impl ValidSingularQuery for FindTwitterBindingByHash {
        #[metrics(+"find_twitter_binding_by_hash")]
        fn execute(&self, state_ro: &impl StateReadOnly) -> Result<TwitterBindingRecord, Error> {
            state_ro
                .world()
                .twitter_bindings()
                .get(&self.binding_hash.digest)
                .cloned()
                .ok_or_else(|| Error::Find(FindError::TwitterBinding(self.binding_hash.clone())))
        }
    }
}

impl Execute for RecordTwitterBinding {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let RecordTwitterBinding {
            attestation,
            feed_id,
        } = self;
        let cfg = &state_transaction.oracle.twitter_binding;
        let expected_feed_id = expected_twitter_feed_id(cfg);
        if feed_id != expected_feed_id {
            return Err(signature_err(format!(
                "twitter binding feed `{}` does not match configured feed `{}`",
                feed_id.as_str(),
                expected_feed_id.as_str()
            )));
        }

        let feed_config = state_transaction
            .world
            .oracle_feeds
            .get(&feed_id)
            .cloned()
            .ok_or_else(|| {
                signature_err(format!(
                    "oracle feed `{}` is not registered",
                    feed_id.as_str()
                ))
            })?;

        ensure_provider_allowed(&feed_config, authority)?;

        if attestation.feed_config_version != feed_config.feed_config_version {
            return Err(signature_err(format!(
                "attestation feed_config_version {} does not match registered version {}",
                attestation.feed_config_version.0, feed_config.feed_config_version.0
            )));
        }

        if attestation.binding_hash.pepper_id != cfg.pepper_id {
            return Err(signature_err(format!(
                "twitter binding pepper `{}` does not match expected `{}`",
                attestation.binding_hash.pepper_id, cfg.pepper_id
            )));
        }

        validate_binding_ttl(&attestation, cfg)?;

        let now_ms = state_transaction.block_unix_timestamp_ms();
        if attestation.is_expired(now_ms) {
            return Err(signature_err(
                "twitter binding attestation is already expired",
            ));
        }

        ensure_binding_matches_history(state_transaction, &feed_id, &attestation)?;

        let digest = attestation.binding_hash.digest;
        if let Some(existing) = state_transaction.world.twitter_bindings.get(&digest) {
            let spacing = attestation
                .observed_at_ms
                .saturating_sub(existing.attestation.observed_at_ms);
            if spacing < cfg.min_update_spacing_ms && !existing.attestation.is_expired(now_ms) {
                return Err(signature_err(format!(
                    "twitter binding update too frequent: {}ms < {}ms",
                    spacing, cfg.min_update_spacing_ms
                )));
            }
        }

        let record = TwitterBindingRecord {
            feed_id,
            provider: authority.clone(),
            attestation,
            recorded_at_ms: now_ms,
        };
        let binding_digest = record.binding_digest();

        state_transaction
            .world
            .twitter_bindings
            .insert(binding_digest, record.clone());

        let mut uaid_index = state_transaction
            .world
            .twitter_bindings_by_uaid
            .get(&record.attestation.uaid)
            .cloned()
            .unwrap_or_default();
        if !uaid_index.contains(&binding_digest) {
            uaid_index.push(binding_digest);
        }
        state_transaction
            .world
            .twitter_bindings_by_uaid
            .insert(record.attestation.uaid, uaid_index);

        state_transaction
            .world
            .emit_events(Some(OracleEvent::TwitterBindingRecorded(
                TwitterBindingRecorded {
                    record: record.clone(),
                },
            )));

        #[cfg(feature = "telemetry")]
        {
            state_transaction.telemetry.observe_twitter_binding(
                record.feed_id.as_str(),
                record.attestation.status,
                record.attestation.is_expired(now_ms),
            );
        }

        Ok(())
    }
}

impl Execute for RevokeTwitterBinding {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let cfg = &state_transaction.oracle.twitter_binding;
        let feed_id = expected_twitter_feed_id(cfg);
        let feed_config = state_transaction
            .world
            .oracle_feeds
            .get(&feed_id)
            .cloned()
            .ok_or_else(|| {
                signature_err(format!(
                    "oracle feed `{}` is not registered",
                    feed_id.as_str()
                ))
            })?;
        ensure_provider_allowed(&feed_config, authority)?;

        let digest = self.binding_hash.digest;
        let Some(record) = state_transaction.world.twitter_bindings.remove(digest) else {
            return Err(signature_err("twitter binding not found"));
        };

        if let Some(mut uaids) = state_transaction
            .world
            .twitter_bindings_by_uaid
            .get(&record.attestation.uaid)
            .cloned()
        {
            uaids.retain(|existing| existing != &digest);
            if uaids.is_empty() {
                state_transaction
                    .world
                    .twitter_bindings_by_uaid
                    .remove(record.attestation.uaid);
            } else {
                state_transaction
                    .world
                    .twitter_bindings_by_uaid
                    .insert(record.attestation.uaid, uaids);
            }
        }

        let now_ms = state_transaction.block_unix_timestamp_ms();
        state_transaction
            .world
            .emit_events(Some(OracleEvent::TwitterBindingRevoked(
                TwitterBindingRevoked {
                    binding_hash: self.binding_hash,
                    revoked_by: authority.clone(),
                    reason: self.reason,
                    recorded_at_ms: now_ms,
                },
            )));

        Ok(())
    }
}

impl Execute for RollbackOracleChange {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut proposal) = state_transaction
            .world
            .oracle_changes
            .get(&self.change_id)
            .cloned()
        else {
            return Err(signature_err(format!(
                "oracle change `{}` is not registered",
                self.change_id.as_hash()
            )));
        };

        if proposal.status.is_terminal() {
            return Err(signature_err(format!(
                "oracle change `{}` is already finalized",
                self.change_id.as_hash()
            )));
        }

        let now = state_transaction.block_height();
        let target_stage = self
            .stage
            .or_else(|| proposal.current_stage().map(|stage| stage.stage))
            .unwrap_or(OracleChangeStage::Enactment);

        if let Some(record) = proposal
            .stages
            .iter_mut()
            .find(|record| record.stage == target_stage)
        {
            record.failure = Some(OracleChangeStageFailure::Rollback(self.reason.clone()));
            record.completed_at.get_or_insert(now);
        }

        proposal.status = OracleChangeStatus::Failed(OracleChangeFailure {
            stage: target_stage,
            reason: OracleChangeStageFailure::Rollback(self.reason.clone()),
            at: now,
        });

        state_transaction
            .world
            .oracle_changes
            .insert(self.change_id, proposal.clone());
        state_transaction
            .world
            .emit_events(Some(OracleEvent::ChangeStageUpdated(
                OracleChangeStageUpdated {
                    change_id: self.change_id,
                    stage: target_stage,
                    status: proposal.status.clone(),
                    approvals: proposal.current_stage().map_or(0, |stage| {
                        u32::try_from(stage.approvals.len()).unwrap_or(u32::MAX)
                    }),
                    rejections: proposal.current_stage().map_or(0, |stage| {
                        u32::try_from(stage.rejections.len()).unwrap_or(u32::MAX)
                    }),
                    evidence_hashes: proposal
                        .current_stage()
                        .map(|stage| stage.evidence.clone())
                        .unwrap_or_default(),
                },
            )));
        Ok(())
    }
}
