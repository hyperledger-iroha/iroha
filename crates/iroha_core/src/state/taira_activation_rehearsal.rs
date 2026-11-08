//! Finite Time and Ironwood activation on an authenticated disposable replay State.
//!
//! This needs no validator key. Four throwaway keys stand in for the committee, in the discarded
//! overlay only, so the exact compiled change set can be scheduled, attested and activated against
//! real replayed State, and the plan's pool bootstrapped. No SignedBlock, vote, QC, finality
//! authorization or publication owner is constructed. It answers whether this State can take the
//! transition; it does not exercise the certificate's real signatures or the live roster.
//!
//! A reserve account the chain has not registered yet is registered in the same overlay, and the
//! report says so. A real activation registers it by ordinary transaction before the bootstrap.

use super::*;
use crate::{
    runtime::{
        change_set::{PublishedDefinitions, activate_due, record_attestation},
        lifecycle_activation::committed_policy,
        lifecycle_preflight::{LifecyclePreflightReport, lifecycle_preflight},
        taira_activation::AUTHORIZATION_DIGEST,
    },
    smartcontracts::{Execute as _, isi::triggers::set::SetReadOnly as _},
    state::{State, StateReadOnly, WorldReadOnly, world_projection::WorldStateAccumulator},
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    account::{AccountId, NewAccount},
    block::BlockHeader,
    isi::{
        InstructionBox, Register, privacy::BootstrapPrivacyIronwoodPoolV1,
        runtime_upgrade::AttestRuntimeSupportV1, taira_activation::ScheduleTairaTimeIronwoodV1,
    },
    privacy::PrivacyIronwoodPoolBootstrapV1,
    runtime::{
        lifecycle::{ChangeSetStatusV1, RuntimeNoticeV1, SupportAttestationV1, committee_digest},
        taira_activation::{
            TAIRA_ACTIVATION_VERSION_V1, TairaActivationCertificateV1, TairaActivationPlanV1,
        },
    },
    transaction::{FeePaymentIntent, TransactionBuilder},
    trigger::Trigger,
};
use mv::storage::StorageReadOnly as _;
use norito::derive::JsonSerialize;
use std::num::NonZeroU64;

/// Evidence from one discarded activation, not a block or finality certificate.
#[derive(Debug, JsonSerialize)]
pub struct TairaActivationRehearsalReport {
    /// Fixed diagnostic scope; no consensus execution authorization is implied.
    pub scope: String,
    /// Exact authenticated native parent height.
    pub parent_height: u64,
    /// Selection the change set starts from.
    pub predecessor: String,
    /// Selection the change set produces.
    pub successor: String,
    /// Identity of the exact compiled change set at this rehearsal's height.
    pub change_set: String,
    /// Number of definitions the change set publishes.
    pub published_definitions: u64,
    /// Digest of those definitions, for comparison with what the signing tool drafted.
    pub published_digest: String,
    /// Canonical bytes of the plan, against the certificate bound.
    pub plan_bytes: u64,
    /// Whether the change set activated at its block start with every check in force.
    pub activated: bool,
    /// The recorded change-set status when it did not activate.
    pub status: String,
    /// Whether the successor selects trigger lifecycle capture.
    pub lifecycle_enabled: bool,
    /// Kernel of the selection the change set starts from.
    pub predecessor_kernel: String,
    /// Kernel selected once the change set was applied or lapsed.
    pub resulting_kernel: String,
    /// Lifecycle policy read back through the activation lineage, or why that read refused.
    pub lifecycle_lineage: String,
    /// Capture sizing over this State's complete trigger registry, rekeyed onto a throwaway account.
    pub trigger_capture: Option<LifecyclePreflightReport>,
    /// Why the capture sizing could not be produced.
    pub trigger_capture_error: Option<String>,
    /// Lane policy verdict on this State's own configuration for the signed schedule.
    pub lane_policy_schedule: String,
    /// Verdict for a validator's attestation before any certificate is retained.
    pub lane_policy_attestation_unscheduled: String,
    /// Verdict for the same attestation once the certificate is retained.
    pub lane_policy_attestation_scheduled: String,
    /// Verdict for the plan's pool bootstrap once the successor is selected.
    pub lane_policy_pool: String,
    /// Cold World commitment before the attempt.
    pub committed_world_before: String,
    /// Cold World commitment of the overlay once the successor is selected.
    pub activated_world: String,
    /// Whether the plan's reserve account was absent and had to be registered in the overlay.
    pub reserve_registered_in_overlay: bool,
    /// Result of bootstrapping the plan's pool under the retained certificate.
    pub pool_bootstrap: String,
    /// Cold World commitment of the overlay after the pool bootstrap.
    pub bootstrapped_world: String,
    /// Cold World commitment after the entire overlay was dropped.
    pub committed_world_after: String,
    /// Always false: this overlay is never published.
    pub state_published: bool,
}

fn throwaway_committee() -> [KeyPair; 4] {
    std::array::from_fn(|index| {
        KeyPair::from_seed(vec![0xA0 + index as u8; 32], Algorithm::Ed25519)
    })
}

impl State {
    /// Schedule, attest and activate the exact compiled change set in a discarded overlay on this
    /// replayed State, then bootstrap `pool` under the retained certificate.
    ///
    /// The caller must first restore and replay authenticated history on this temporary Kura. This
    /// verifies the cold committed World before and after dropping the complete overlay and never
    /// writes history.
    ///
    /// # Errors
    /// Refuses non-temporary stores, missing or mismatched native history, a selection other than
    /// the exact repaired one, or a changed committed State. A change set that lapses or a refused
    /// pool is a result in the report, not an error.
    pub fn rehearse_taira_activation(
        &self,
        pool: PrivacyIronwoodPoolBootstrapV1,
    ) -> Result<TairaActivationRehearsalReport, String> {
        if !self.kura.is_temporary_staging() {
            return Err("activation rehearsal requires the owned temporary replay store".into());
        }
        let before = self
            .verify_world_state_accumulator()
            .map_err(|error| error.to_string())?;
        let view = self.view();
        let tip = view
            .native_execution_tip()
            .ok_or("authenticated native parent is absent")?;
        let durable = self
            .kura
            .exact_durable_blocks_count()
            .map_err(|error| error.to_string())?;
        if u64::try_from(durable).ok() != Some(tip.height())
            || u64::try_from(view.height()).ok() != Some(tip.height())
            || view.latest_block_hash() != Some(tip.iroha_hash())
        {
            return Err("rehearsal State does not reproduce the exact durable native tip".into());
        }
        let height = tip.height().checked_add(1).ok_or("height overflow")?;
        let effective_height = height.checked_add(1).ok_or("height overflow")?;
        let network = view.network_id;
        let predecessor = view
            .world()
            .runtime_selection()
            .id()
            .map_err(|error| error.to_string())?;
        let predecessor_kernel = view.world().runtime_selection().kernel.to_string();
        let change_set = crate::runtime::ironwood::taira_time_ironwood_change_set(
            network,
            effective_height,
            [0x52; 32],
            view.world().runtime_selection(),
            &PublishedDefinitions(view.world().runtime_definitions()),
        )
        .map_err(|error| error.to_string())?;
        let id = change_set.id().map_err(|error| error.to_string())?;
        let keys = throwaway_committee();
        let committee: Vec<_> = keys.iter().map(|key| key.public_key().clone()).collect();
        let plan = TairaActivationPlanV1 {
            version: TAIRA_ACTIVATION_VERSION_V1,
            chain_id: view.chain_id.clone(),
            network_id: network,
            operation_id: [0x53; 32],
            change_set: change_set.clone(),
            notice: RuntimeNoticeV1 {
                blocks: 1,
                ms: 0,
                approval_cushion_ms: 0,
            },
            committee: std::array::from_fn(|index| keys[index].public_key().clone()),
            expires_at_unix_ms: u64::MAX,
            authorization_digest: AUTHORIZATION_DIGEST,
            release_digest: [0x54; 32],
            source_closure_digest: [0x55; 32],
            pool: pool.clone(),
        };
        let plan_bytes = plan.to_bytes().map_err(|error| error.to_string())?;
        let plan_id = plan.id().map_err(|error| error.to_string())?;
        let signing = plan.signing_bytes().map_err(|error| error.to_string())?;
        let signatures =
            std::array::from_fn(|index| Signature::new(keys[index].private_key(), &signing));
        let certificate = TairaActivationCertificateV1 { plan, signatures };
        let time_ms = tip.creation_time_ms().saturating_add(1_000);
        let digest = committee_digest(&committee).map_err(|error| error.to_string())?;
        let triggers: Vec<Trigger> = view.world().triggers().triggers_iter().collect();
        let capture_destination = AccountId::new(keys[1].public_key().clone());
        let (trigger_capture, trigger_capture_error) = match lifecycle_preflight(
            view.world().parameters(),
            &triggers,
            1,
            1,
            Some(&capture_destination),
        ) {
            Ok(report) => (Some(report), None),
            Err(error) => (None, Some(error)),
        };
        drop(view);

        let header = BlockHeader::new(
            NonZeroU64::new(height).ok_or("invalid successor height")?,
            Some(tip.iroha_hash()),
            None,
            time_ms,
            0,
        );
        let verdict = |result: Result<(), String>| match result {
            Ok(()) => "admitted".to_owned(),
            Err(reason) => format!("refused: {reason}"),
        };
        let signed_by_first_key = |instructions: [InstructionBox; 1]| {
            TransactionBuilder::new(
                network,
                AccountId::new(keys[0].public_key().clone()),
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions(instructions)
            .sign(keys[0].private_key())
        };
        let support = |key: &KeyPair| {
            SupportAttestationV1 {
                network,
                root: change_set.root,
                change_set: id,
                predecessor: change_set.predecessor,
                successor: change_set.successor,
                committee: digest,
                tested_checkpoint: tip.height(),
                valid_from: tip.height(),
                valid_until: effective_height.saturating_add(1),
                release: [0x56; 32],
                validator: key.public_key().clone(),
            }
            .sign(key.private_key())
            .map_err(|error| error.to_string())
        };
        let attempted = (|| -> Result<_, String> {
            let lane_policy_schedule;
            let lane_policy_attestation_unscheduled;
            let lane_policy_attestation_scheduled;
            let (mut block, ()) = self
                .block_with_owned_start_stages(
                    header,
                    |_| Ok::<_, String>(()),
                    |_, captured| Ok(captured),
                )
                .map_err(|error| format!("activation overlay acquisition failed: {error:?}"))?;
            let authority = AccountId::new(keys[0].public_key().clone());
            {
                let mut tx = block
                    .try_transaction()
                    .map_err(|error| format!("transaction admission refused: {error:?}"))?;
                lane_policy_schedule = verdict(crate::tx::lane_policy_verdict(
                    &signed_by_first_key([ScheduleTairaTimeIronwoodV1 {
                        certificate: certificate.clone(),
                    }
                    .into()]),
                    &tx,
                ));
                let first_attestation = signed_by_first_key([AttestRuntimeSupportV1 {
                    attestation: support(&keys[0])?,
                }
                .into()]);
                lane_policy_attestation_unscheduled =
                    verdict(crate::tx::lane_policy_verdict(&first_attestation, &tx));
                tx.taira_activation_entrypoint = Some(plan_id);
                (ScheduleTairaTimeIronwoodV1 { certificate })
                    .execute(&authority, &mut tx)
                    .map_err(|error| format!("schedule refused: {error}"))?;
                lane_policy_attestation_scheduled =
                    verdict(crate::tx::lane_policy_verdict(&first_attestation, &tx));
                let mut record = tx
                    .world
                    .runtime_change_sets
                    .get(&id)
                    .cloned()
                    .ok_or("the schedule recorded nothing")?;
                for key in &keys {
                    record_attestation(&mut record, support(key)?);
                }
                tx.world.runtime_change_sets.insert(id, record);
                tx.apply();
            }
            {
                let mut tx = block
                    .try_transaction()
                    .map_err(|error| format!("transaction admission refused: {error:?}"))?;
                activate_due(
                    &mut tx.world,
                    effective_height,
                    time_ms.saturating_add(1_000),
                    &committee,
                )
                .map_err(|error| format!("activation stopped the node: {error}"))?;
                tx.apply();
            }
            let status = block
                .world
                .runtime_change_sets()
                .get(&id)
                .map(|record| record.status)
                .ok_or("the scheduled record disappeared")?;
            let activated = matches!(status, ChangeSetStatusV1::Activated(_));
            let lifecycle_enabled =
                crate::runtime::trigger_lifecycle_enabled(block.world.runtime_selection())
                    .map_err(|error| error.to_string())?;
            let resulting_kernel = block.world.runtime_selection().kernel.to_string();
            let lifecycle_lineage = match committed_policy(&block.world, network, effective_height)
            {
                Ok(policy) => format!(
                    "active={} activation={:?}",
                    policy.active,
                    policy.activation.map(|(id, at)| (id.to_string(), at.get())),
                ),
                Err(error) => format!("refused: {error}"),
            };
            let activated_world = WorldStateAccumulator::capture(&block.world)?
                .root(block.world.runtime_selection())?
                .to_string();
            let reserve_registered_in_overlay =
                block.world.accounts().get(&pool.reserve_account).is_none();
            if reserve_registered_in_overlay {
                let mut tx = block
                    .try_transaction()
                    .map_err(|error| format!("transaction admission refused: {error:?}"))?;
                Register::account(NewAccount::new(pool.reserve_account.clone()))
                    .execute(&authority, &mut tx)
                    .map_err(|error| format!("reserve registration refused: {error}"))?;
                tx.apply();
            }
            let lane_policy_pool;
            let pool_bootstrap = {
                let mut tx = block
                    .try_transaction()
                    .map_err(|error| format!("transaction admission refused: {error:?}"))?;
                lane_policy_pool = verdict(crate::tx::lane_policy_verdict(
                    &signed_by_first_key(
                        [BootstrapPrivacyIronwoodPoolV1::new(pool.clone()).into()],
                    ),
                    &tx,
                ));
                let result = BootstrapPrivacyIronwoodPoolV1::new(pool).execute(&authority, &mut tx);
                match result {
                    Ok(()) => {
                        tx.apply();
                        "bootstrapped".to_owned()
                    }
                    Err(error) => format!("refused: {error:?}"),
                }
            };
            let bootstrapped_world = WorldStateAccumulator::capture(&block.world)?
                .root(block.world.runtime_selection())?
                .to_string();
            drop(block);
            Ok(AttemptOutcome {
                activated,
                status: format!("{status:?}"),
                lifecycle_enabled,
                resulting_kernel,
                lifecycle_lineage,
                activated_world,
                reserve_registered_in_overlay,
                lane_policy_schedule,
                lane_policy_attestation_unscheduled,
                lane_policy_attestation_scheduled,
                lane_policy_pool,
                pool_bootstrap,
                bootstrapped_world,
            })
        })();
        let after = self
            .verify_world_state_accumulator()
            .map_err(|error| error.to_string())?;
        if after != before
            || self.view().native_execution_tip() != Some(tip)
            || self
                .kura
                .exact_durable_blocks_count()
                .map_err(|error| error.to_string())?
                != durable
        {
            return Err("discarded activation changed committed State or durable history".into());
        }
        let outcome = attempted?;
        Ok(TairaActivationRehearsalReport {
            scope: "keyless_activation_discarded_overlay".into(),
            parent_height: tip.height(),
            predecessor: predecessor.to_string(),
            successor: change_set.successor.to_string(),
            change_set: id.to_string(),
            published_definitions: u64::try_from(change_set.publish.len()).unwrap_or(u64::MAX),
            published_digest: crate::runtime::ironwood::published_digest(&change_set)
                .map_err(|error| error.to_string())?,
            plan_bytes: u64::try_from(plan_bytes.len()).unwrap_or(u64::MAX),
            activated: outcome.activated,
            status: outcome.status,
            lifecycle_enabled: outcome.lifecycle_enabled,
            predecessor_kernel,
            resulting_kernel: outcome.resulting_kernel,
            lifecycle_lineage: outcome.lifecycle_lineage,
            trigger_capture,
            trigger_capture_error,
            lane_policy_schedule: outcome.lane_policy_schedule,
            lane_policy_attestation_unscheduled: outcome.lane_policy_attestation_unscheduled,
            lane_policy_attestation_scheduled: outcome.lane_policy_attestation_scheduled,
            lane_policy_pool: outcome.lane_policy_pool,
            committed_world_before: before.to_string(),
            activated_world: outcome.activated_world,
            reserve_registered_in_overlay: outcome.reserve_registered_in_overlay,
            pool_bootstrap: outcome.pool_bootstrap,
            bootstrapped_world: outcome.bootstrapped_world,
            committed_world_after: after.to_string(),
            state_published: false,
        })
    }
}

/// What the discarded overlay produced, before it was dropped.
struct AttemptOutcome {
    activated: bool,
    status: String,
    lifecycle_enabled: bool,
    resulting_kernel: String,
    lifecycle_lineage: String,
    activated_world: String,
    reserve_registered_in_overlay: bool,
    lane_policy_schedule: String,
    lane_policy_attestation_unscheduled: String,
    lane_policy_attestation_scheduled: String,
    lane_policy_pool: String,
    pool_bootstrap: String,
    bootstrapped_world: String,
}
