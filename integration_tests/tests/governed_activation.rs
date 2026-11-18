//! Governed activation of release additions on four real validators (roadmap N14).
//!
//! The harness (`support/governed_activation.rs`) drives the public path of
//! `specs/runtime_upgrades.md`: every validator drafts the change set, a proposer submits the
//! baseline proposal the node drafts, a Parliament manager opens the baseline attempt, Parliament
//! enacts it, every validator's node attests, and block start activates it.
//!
//! `governed_activation_reaches_parliament_and_schedules_nothing_without_it` runs every step
//! this release offers on an `NPoS` network and checks that nothing schedules without Parliament:
//! each node refuses to attest and the selection never moves. The complete path,
//! `four_validators_activate_the_anonymous_ballot_family_through_parliament`, stays ignored
//! because Parliament cannot enact a runtime change set on this release (the harness's
//! `ParliamentApprovalUnavailable`).

use std::time::Duration;

use eyre::{Result, ensure, eyre};
use integration_tests::sandbox;
use iroha_data_model::governance::types::{
    GovernanceAttemptStatusV1, GovernanceStageV1, ParliamentBody,
};
use iroha_test_network::NetworkBuilder;
use iroha_torii_shared::parliament_api::ParliamentDecisionModeProjectionV1;

#[path = "support/governed_activation.rs"]
pub(crate) mod harness;

#[path = "support/anonymous_ballot_corridor.rs"]
mod anonymous_ballot;

/// Blocks between the tip at drafting and the effective height: room for the proposal and the
/// attempt, which each take one carrier block.
const LEAD_BLOCKS: u64 = 24;

/// Four `NPoS` validators with the governed-activation genesis. `NPoS` gives the native cadence a
/// Parliament attempt needs; the configuration only bounds the local cost of proving and of the
/// handshake puzzle on a shared host, as the Parliament corridors do.
fn builder(label: &str) -> Result<NetworkBuilder> {
    harness::with_governed_activation_genesis(
        NetworkBuilder::new()
            .with_base_seed(label)
            .with_peers(4)
            .with_auto_populated_trusted_peers()
            .with_npos_consensus()
            .with_block_cadence(Duration::from_secs(1))
            .with_config_layer(|layer| {
                layer
                    .write(["concurrency", "rayon_global_threads"], 2_i64)
                    .write(
                        ["nexus", "storage", "local_budget_bytes"],
                        1_073_741_824_i64,
                    )
                    .write(
                        [
                            "network",
                            "soranet_handshake",
                            "pow",
                            "puzzle",
                            "memory_kib",
                        ],
                        i64::from(iroha_crypto::soranet::puzzle::MIN_MEMORY_KIB),
                    )
                    .write(
                        ["network", "soranet_handshake", "pow", "puzzle", "time_cost"],
                        1_i64,
                    )
                    .write(
                        ["network", "soranet_handshake", "pow", "puzzle", "lanes"],
                        1_i64,
                    );
            }),
    )
}

#[test]
fn governed_activation_reaches_parliament_and_schedules_nothing_without_it() -> Result<()> {
    let context = "governed_activation_prefix";
    let Some((network, runtime)) =
        sandbox::start_network_blocking_or_skip(builder(context)?, context)?
    else {
        return Ok(());
    };
    runtime.block_on(network.ensure_blocks(1))?;
    ensure!(network.validators().len() == 4, "four validators required");
    let client = network.client();

    // Every peer commits the founded baseline selection and records no change set.
    let founded = harness::runtime_view(&network.peers()[0])?;
    for peer in network.peers() {
        let view = harness::runtime_view(peer)?;
        ensure!(
            view.selection_id_hex == founded.selection_id_hex
                && view.records.is_empty()
                && view.committee.len() == 4,
            "peers disagree on the founded selection"
        );
    }

    // Game partitions alone are refused; their capture service is what a root can draft.
    let effective_height = harness::tip(&client)? + LEAD_BLOCKS;
    let refusal = harness::draft_refusal(
        &network.peers()[0],
        harness::GAME_PARTITIONS,
        effective_height,
    )?;
    ensure!(
        refusal.contains("captured-transfer service"),
        "game partitions were refused for another reason: {refusal}"
    );
    harness::draft_on_every_validator(
        &network,
        harness::NATIVE_GAME_CAPTURE,
        effective_height,
        [0x51; 32],
    )?;

    // The anonymous ballot family: drafted identically, proposed, and given its attempt.
    let drafted = harness::draft_on_every_validator(
        &network,
        harness::PARLIAMENT_ANONYMOUS,
        effective_height,
        [0x52; 32],
    )?;
    let proposed = harness::propose(&network, &client, drafted)?;
    let attempt = harness::open_parliament_attempt(&network, &client, proposed)?;
    let read = client.client().get_parliament_attempt(attempt.attempt_id)?;
    ensure!(
        read.attempt.status == GovernanceAttemptStatusV1::Active
            && read.attempt.stage == GovernanceStageV1::Qualification,
        "the attempt did not open in qualification"
    );
    ensure!(
        read.required_bodies.iter().any(|required| {
            required.body == ParliamentBody::PolicyJury
                && matches!(
                    required.decision_mode,
                    ParliamentDecisionModeProjectionV1::HiddenBindingBallot
                )
        }),
        "a runtime-upgrade attempt must end in the Policy Jury's hidden ballot"
    );
    let blocked = harness::await_parliament_schedule(&network, &attempt)
        .err()
        .ok_or_else(|| eyre!("this release cannot schedule a change set"))?;
    ensure!(
        blocked
            .downcast_ref::<harness::ParliamentApprovalUnavailable>()
            .is_some_and(|unavailable| unavailable.attempt_id == attempt.attempt_id),
        "approval failed for another reason: {blocked:?}"
    );

    // Without Parliament nothing schedules: every node refuses to attest, and the chain passes
    // the effective height with the founded selection.
    let id_hex = hex::encode(attempt.proposed.drafted.id.0);
    for peer in network.validators() {
        let refusal = peer
            .client()
            .client()
            .post_runtime_attest_upgrade_json(&id_hex)
            .err()
            .ok_or_else(|| eyre!("a node attested an unscheduled change set"))?;
        ensure!(
            format!("{refusal:?}").contains("is not recorded"),
            "a node refused to attest for another reason: {refusal:?}"
        );
    }
    harness::carry_to_height(
        &network,
        &client,
        effective_height + 1,
        "past the effective height",
    )?;
    for peer in network.peers() {
        let view = harness::runtime_view(peer)?;
        ensure!(
            view.selection_id_hex == founded.selection_id_hex && view.records.is_empty(),
            "a selection moved without Parliament"
        );
    }
    eprintln!(
        "GOVERNED_ACTIVATION_PREFIX validators=4 drafted={} attempt={} scheduled=false selection={}",
        harness::PARLIAMENT_ANONYMOUS,
        hex::encode(attempt.attempt_id.as_bytes()),
        founded.selection_id_hex,
    );
    Ok(())
}

#[test]
#[ignore = "Parliament cannot enact a runtime change set on this release: the baseline Policy Jury ballot is the retired timed-OVN ballot and the anonymous ballot family enacts nothing (support/governed_activation.rs, ParliamentApprovalUnavailable)"]
fn four_validators_activate_the_anonymous_ballot_family_through_parliament() -> Result<()> {
    let context = "governed_activation_anonymous_family";
    let (network, runtime) = sandbox::start_network_blocking_or_skip(
        anonymous_ballot::genesis(builder(context)?)?,
        context,
    )?
    .ok_or_else(|| eyre!("governed activation requires real localhost sockets"))?;
    runtime.block_on(network.ensure_blocks(1))?;
    let activated = harness::activate_addition(
        &network,
        harness::PARLIAMENT_ANONYMOUS,
        LEAD_BLOCKS,
        [0x53; 32],
    )?;
    anonymous_ballot::exercise(&network, &runtime)?;
    eprintln!(
        "GOVERNED_ACTIVATION_COMPLETE validators=4 addition={} change_set={} height={}",
        harness::PARLIAMENT_ANONYMOUS,
        hex::encode(activated.id.0),
        activated.change_set.effective_height,
    );
    Ok(())
}
