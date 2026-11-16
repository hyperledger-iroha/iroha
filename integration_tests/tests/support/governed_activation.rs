//! Governed activation of a release addition on a real test network (roadmap N14).
//!
//! The harness drives only public interfaces, in the order of `specs/runtime_upgrades.md`
//! ("Lifecycle", "Torii"):
//!
//! 1. every validator drafts the change set from its own release catalog
//!    (`POST /v1/runtime/upgrades/draft`), and all of them must draft the same one;
//! 2. a node drafts the baseline `ProposeRuntimeUpgradeProposal`
//!    (`POST /v1/runtime/upgrades/propose`), which a `CanProposeRuntimeChangeSet` holder signs
//!    and submits under the committed `runtime_upgrade_policy_v1` notice;
//! 3. a `CanManageParliament` holder opens the baseline Parliament attempt for that exact
//!    proposal (`CreateParliamentGovernanceAttemptV1`);
//! 4. Parliament's certificate enacts the proposal at its due block, which records the change
//!    set `Scheduled` ([`await_parliament_schedule`]);
//! 5. every validator's node signs its support with its own validator key
//!    (`POST /v1/runtime/upgrades/attest/{id}`), and the operator relays each attestation;
//! 6. signed carriers take the chain to the effective height (an idle chain makes no block),
//!    whose block start activates the change set; every peer must then report it `Activated`
//!    at that height, select its successor and execute that selection.
//!
//! Step 4 has no route on this release ([`ParliamentApprovalUnavailable`]), so a governed
//! activation stops there. The harness never injects a certificate, a scheduled record or a
//! selection, and uses no node-configuration toggle and no fixture-only activation.

use std::{
    num::NonZeroU64,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr as _, ensure, eyre};
use iroha::{blocking::Client, client::TransactionWaitOptions, data_model::prelude::*};
use iroha_crypto::PublicKey;
use iroha_data_model::{
    governance::types::{GovernanceAttemptId, ProposalKind, RuntimeUpgradeProposal},
    isi::{
        decode_instruction_from_pair,
        governance::{CreateParliamentGovernanceAttemptV1, ProposeRuntimeUpgradeProposal},
    },
    parameter::{Parameter, RuntimeUpgradePolicyV1},
    runtime::lifecycle::{
        ChangeSetId, ChangeSetRecordV1, ChangeSetStatusV1, ChangeSetV1, RootScopeV1,
        RuntimeNoticeV1, RuntimeUpgradeActionV1,
    },
};
use iroha_executor_data_model::permission::governance::CanProposeRuntimeChangeSet;
use iroha_test_network::{Network, NetworkBuilder, NetworkPeer};
use iroha_test_samples::ALICE_ID;
use norito::json::{self, Value};

/// The anonymous Parliament ballot family (`specs/parliament_private_ballot_design.md` §12.3).
pub const PARLIAMENT_ANONYMOUS: &str = "iroha.governance.parliament.anonymous.v1";
/// Native game partitions with their certified capture service; the only draftable route to
/// game partitions (`runtime::catalog::unqualified_native_activation`).
pub const NATIVE_GAME_CAPTURE: &str = "iroha.runtime.native_game_capture.v1";
/// Native game partitions on their own, which the draft route refuses.
pub const GAME_PARTITIONS: &str = "iroha.game_partition.v1";

/// The notice the harness genesis commits for both kinds of change: the smallest valid values,
/// so the effective height alone bounds an activation. Activation and disable notices are
/// equal, so a change set takes this notice whatever its transitions
/// (`runtime::change_set::notice_for`).
pub const NOTICE: RuntimeNoticeV1 = RuntimeNoticeV1 {
    blocks: 1,
    ms: 0,
    approval_cushion_ms: 0,
};

/// Wall-clock bound of one wait for peers to reach a height.
const HEIGHT_TIMEOUT: Duration = Duration::from_secs(240);

fn fee() -> FeePaymentIntent {
    FeePaymentIntent::authority(Vec::new(), None)
}

/// The committed notice policy of the harness genesis.
#[must_use]
pub fn notice_policy() -> RuntimeUpgradePolicyV1 {
    RuntimeUpgradePolicyV1 {
        activation: NOTICE,
        disable: NOTICE,
    }
}

/// Add what the governed path needs to a network's signed genesis: the committed
/// `runtime_upgrade_policy_v1` notice and `CanProposeRuntimeChangeSet` for Alice.
///
/// The default test-network genesis already grants Alice `CanManageParliament`, which opens
/// the attempt; a second grant would be a genuine genesis repetition rejection. Neither the
/// policy nor the grant changes a protocol rule: a root without the policy admits no change
/// set, and the proposal permission is an ordinary validated genesis grant.
///
/// # Errors
/// The notice policy does not encode as its canonical custom parameter.
pub fn with_governed_activation_genesis(builder: NetworkBuilder) -> Result<NetworkBuilder> {
    let policy = notice_policy()
        .to_custom_parameter()
        .map_err(|error| eyre!("runtime upgrade notice policy: {error}"))?;
    Ok(builder
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(policy)))
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanProposeRuntimeChangeSet),
            ALICE_ID.clone(),
        )))
}

/// One peer's `GET /v1/runtime/upgrades`: its committed selection, committee and records.
#[derive(Debug, Clone)]
pub struct RuntimeView {
    /// Identity of the committed selection, lowercase hex.
    pub selection_id_hex: String,
    /// Keys of the current committee, in the route's order.
    pub committee: Vec<PublicKey>,
    /// Every recorded change set, by identity hex.
    pub records: Vec<(String, ChangeSetRecordV1)>,
}

impl RuntimeView {
    /// Decode the route's JSON.
    ///
    /// # Errors
    /// A field is missing or does not decode.
    pub fn from_route(route: &Value) -> Result<Self> {
        let selection_id_hex = route["selection_id_hex"]
            .as_str()
            .ok_or_else(|| eyre!("the runtime route names no selection"))?
            .to_owned();
        let committee = route["committee"]
            .as_array()
            .ok_or_else(|| eyre!("the runtime route lists no committee"))?
            .iter()
            .map(|key| {
                key.as_str()
                    .ok_or_else(|| eyre!("a committee key is not a string"))?
                    .parse::<PublicKey>()
                    .map_err(|error| eyre!("committee key: {error}"))
            })
            .collect::<Result<_>>()?;
        let records = route["items"]
            .as_array()
            .ok_or_else(|| eyre!("the runtime route lists no change sets"))?
            .iter()
            .map(|item| {
                let id = item["id_hex"]
                    .as_str()
                    .ok_or_else(|| eyre!("a change-set item has no identity"))?
                    .to_owned();
                let record: ChangeSetRecordV1 = json::from_value(item["record"].clone())
                    .wrap_err("decode a change-set record")?;
                Ok((id, record))
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            selection_id_hex,
            committee,
            records,
        })
    }

    /// The record of change set `id`, if recorded.
    #[must_use]
    pub fn record(&self, id: ChangeSetId) -> Option<&ChangeSetRecordV1> {
        let id = hex::encode(id.0);
        self.records
            .iter()
            .find(|(recorded, _)| *recorded == id)
            .map(|(_, record)| record)
    }
}

/// Read one peer's runtime selection and change sets through its operator route.
///
/// # Errors
/// The operator-signed read fails or does not decode.
pub fn runtime_view(peer: &NetworkPeer) -> Result<RuntimeView> {
    let route = peer
        .client()
        .client()
        .get_runtime_upgrades_json()
        .wrap_err("read the runtime upgrades route")?;
    RuntimeView::from_route(&route)
}

/// A change set every validator drafted identically from its own release catalog.
#[derive(Debug, Clone)]
pub struct DraftedChangeSet {
    /// The release addition it activates.
    pub addition: String,
    /// Its identity, recomputed locally and equal to every node's.
    pub id: ChangeSetId,
    /// The change set.
    pub change_set: ChangeSetV1,
}

/// Have every validator draft the activation of `addition` from `effective_height`.
///
/// Each node drafts from its own release catalog and committed selection; all of them must
/// return the same change set, on this network's universal root, from the selection every
/// peer commits, with the requested height and qualification digest.
///
/// # Errors
/// A node refuses the draft, or the drafts disagree with each other or with the network.
pub fn draft_on_every_validator(
    network: &Network,
    addition: &str,
    effective_height: u64,
    qualification: [u8; 32],
) -> Result<DraftedChangeSet> {
    let mut agreed: Option<DraftedChangeSet> = None;
    for peer in network.validators() {
        let committed = runtime_view(peer)?.selection_id_hex;
        let draft = peer
            .client()
            .client()
            .post_runtime_draft_upgrade_json(
                addition,
                effective_height,
                &hex::encode(qualification),
            )
            .wrap_err_with(|| format!("a validator refused to draft {addition}"))?;
        let change_set: ChangeSetV1 = json::from_value(draft["change_set"].clone())
            .wrap_err("decode the drafted change set")?;
        let id = change_set
            .id()
            .map_err(|error| eyre!("change-set identity: {error}"))?;
        ensure!(
            draft["id_hex"].as_str() == Some(hex::encode(id.0).as_str()),
            "the node's change-set identity differs from the one its change set encodes"
        );
        ensure!(
            change_set.network == network.network_id()
                && change_set.root == RootScopeV1::Universal
                && change_set.effective_height == effective_height
                && change_set.qualification == qualification
                && change_set.predecessor.to_string() == committed,
            "the draft of {addition} does not start from this network's committed universal root"
        );
        if let Some(agreed) = &agreed {
            ensure!(
                agreed.id == id && agreed.change_set == change_set,
                "validators drafted different change sets for {addition}"
            );
        } else {
            agreed = Some(DraftedChangeSet {
                addition: addition.to_owned(),
                id,
                change_set,
            });
        }
    }
    agreed.ok_or_else(|| eyre!("the network has no validators"))
}

/// Read the error a validator returns when it refuses to draft `addition`.
///
/// # Errors
/// The node drafted the change set instead of refusing it.
pub fn draft_refusal(peer: &NetworkPeer, addition: &str, effective_height: u64) -> Result<String> {
    match peer.client().client().post_runtime_draft_upgrade_json(
        addition,
        effective_height,
        &hex::encode([0_u8; 32]),
    ) {
        Ok(draft) => Err(eyre!("the node drafted {addition}: {draft:?}")),
        Err(error) => Ok(format!("{error:?}")),
    }
}

/// A runtime-upgrade proposal admitted into governance and awaiting Parliament.
#[derive(Debug, Clone)]
pub struct ProposedChangeSet {
    /// The change set it schedules.
    pub drafted: DraftedChangeSet,
    /// The exact typed proposal the attempt names.
    pub proposal: ProposalKind,
    /// Height of the block that admitted it.
    pub height: u64,
}

impl ProposedChangeSet {
    /// The governance proposal's identity, lowercase hex.
    #[must_use]
    pub fn proposal_id_hex(&self) -> String {
        hex::encode(self.proposal.fingerprint())
    }
}

/// The typed proposal that admission records for scheduling `change_set`.
///
/// Admission freezes the committed notice and the admitting block's time into the payload,
/// and the proposal's identity is the fingerprint of all of it, so a client derives it from the
/// committed block rather than predicting it.
#[must_use]
pub fn runtime_upgrade_proposal(
    proposer: &AccountId,
    change_set: &ChangeSetV1,
    notice: RuntimeNoticeV1,
    proposed_ms: u64,
) -> ProposalKind {
    ProposalKind::RuntimeUpgrade(RuntimeUpgradeProposal {
        proposal_operator: proposer.clone(),
        action: RuntimeUpgradeActionV1::Schedule(change_set.clone()),
        notice,
        proposed_ms,
    })
}

/// Committed time of the finalized block at `height`, in milliseconds.
///
/// # Errors
/// The finality proof cannot be read, or it is for another height.
pub fn block_time_ms(network: &Network, client: &Client, height: u64) -> Result<u64> {
    let root_scope = iroha_genesis::signed_genesis_consensus_metadata(&network.genesis().0)?
        .sumeragi_context
        .root_scope;
    let proof = client.client().get_sumeragi_finality_proof(
        NonZeroU64::new(height).ok_or_else(|| eyre!("height zero has no block"))?,
        root_scope,
    )?;
    ensure!(
        proof.block_header.height().get() == height,
        "the finality proof is for another height"
    );
    Ok(u64::try_from(
        proof.block_header.creation_time().as_millis(),
    )?)
}

fn submit_applied(client: &Client, instruction: InstructionBox) -> Result<u64> {
    let hash = client.submit(instruction, fee())?;
    client
        .wait_for_transaction_applied(hash, TransactionWaitOptions::default())?
        .block_height
        .ok_or_else(|| eyre!("the applied transaction reports no block height"))
}

/// The single instruction a node drafted in a `{ ok, tx_instructions }` response.
///
/// # Errors
/// The response drafts no instruction or more than one, or it does not decode.
pub fn drafted_instruction(draft: &Value) -> Result<InstructionBox> {
    let [drafted] = draft["tx_instructions"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
    else {
        return Err(eyre!("the node did not draft exactly one instruction"));
    };
    let wire_id = drafted["wire_id"]
        .as_str()
        .ok_or_else(|| eyre!("the draft has no wire ID"))?;
    let payload = hex::decode(
        drafted["payload_hex"]
            .as_str()
            .ok_or_else(|| eyre!("the draft has no payload"))?,
    )?;
    Ok(decode_instruction_from_pair(wire_id, &payload)?)
}

/// Propose scheduling `drafted` through the baseline proposal the node drafts, signed by
/// `proposer`, and confirm on every peer that governance recorded it as `Proposed`.
///
/// # Errors
/// The node drafts another instruction, admission rejects it, or a peer does not record it.
pub fn propose(
    network: &Network,
    proposer: &Client,
    drafted: DraftedChangeSet,
) -> Result<ProposedChangeSet> {
    let change_set = json::to_value(&drafted.change_set)?;
    let draft = proposer
        .client()
        .post_runtime_propose_upgrade_json(&norito::json!({
            "change_set": change_set,
            "provenance": [],
        }))
        .wrap_err("the node drafts the proposal")?;
    let instruction = drafted_instruction(&draft)?;
    let baseline = instruction
        .as_any()
        .downcast_ref::<ProposeRuntimeUpgradeProposal>()
        .ok_or_else(|| eyre!("without provenance the node must draft the baseline proposal"))?;
    ensure!(
        baseline.action == RuntimeUpgradeActionV1::Schedule(drafted.change_set.clone()),
        "the drafted proposal schedules another change set"
    );
    let height = submit_applied(proposer, instruction)
        .wrap_err_with(|| format!("the signed proposal to activate {}", drafted.addition))?;
    let proposal = runtime_upgrade_proposal(
        proposer.client().account(),
        &drafted.change_set,
        NOTICE,
        block_time_ms(network, proposer, height)?,
    );
    let proposed = ProposedChangeSet {
        drafted,
        proposal,
        height,
    };
    await_height_on_every_peer(network, height)?;
    for peer in network.peers() {
        let response = peer
            .client()
            .client()
            .get_gov_proposal_json(&proposed.proposal_id_hex())?;
        ensure!(
            response["found"].as_bool() == Some(true),
            "a peer does not record the proposal admitted at height {height}"
        );
        let record: iroha_core::state::GovernanceProposalRecord =
            json::from_value(response["proposal"].clone()).wrap_err("decode the proposal")?;
        ensure!(
            record.kind == proposed.proposal
                && record.created_height == height
                && record.status == iroha_core::state::GovernanceProposalStatus::Proposed,
            "a peer records another proposal under the admitted identity"
        );
    }
    Ok(proposed)
}

/// A baseline Parliament attempt opened for a runtime-upgrade proposal.
#[derive(Debug, Clone)]
pub struct OpenedAttempt {
    /// The proposal it decides.
    pub proposed: ProposedChangeSet,
    /// Its identity.
    pub attempt_id: GovernanceAttemptId,
}

/// Open the first baseline Parliament attempt for `proposed`, signed by a
/// `CanManageParliament` holder, and read it back from every peer.
///
/// # Errors
/// Creation is rejected, or a peer does not project the attempt.
pub fn open_parliament_attempt(
    network: &Network,
    manager: &Client,
    proposed: ProposedChangeSet,
) -> Result<OpenedAttempt> {
    let create = CreateParliamentGovernanceAttemptV1 {
        proposal: proposed.proposal.clone(),
        attempt_sequence: 0,
    };
    let attempt_id = create.governance_attempt_id();
    let height = submit_applied(manager, create.into()).wrap_err("open the Parliament attempt")?;
    ensure!(
        height > proposed.height,
        "the attempt committed before the proposal it decides"
    );
    await_height_on_every_peer(network, height)?;
    for peer in network.peers() {
        let read = peer.client().client().get_parliament_attempt(attempt_id)?;
        ensure!(
            read.attempt.id == attempt_id,
            "a peer projects another attempt"
        );
    }
    Ok(OpenedAttempt {
        proposed,
        attempt_id,
    })
}

/// Why Parliament cannot schedule a runtime change set on a network of this release.
///
/// A `RuntimeUpgrade` attempt needs the Policy Jury's hidden binding ballot
/// (`governance::parliament::parliament_attempt_policy_v1`), and `construct_certificate`
/// refuses without its ballot binding. In the frozen baseline that ballot is timed-OVN with a
/// Parliament TLE session: the owner retired it (`specs/parliament_private_ballot_design.md`
/// §12.1 and §12.3), `specs/live_runtime_upgrades.md` ("Committed lifecycle activation
/// readiness") forbids reviving it as an activation route, consensus produces no pulse for its
/// release slot, and the daemon carries no TLE signer. Its replacement, the anonymous ballot
/// family, is itself an inactive release addition that only certifies candidate-testing
/// attempts and enacts nothing (design §12.3, slice 5). Only Parliament enactment, or the
/// exact-Taira activation certificate, records a change set `Scheduled`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParliamentApprovalUnavailable {
    /// The open attempt that cannot reach a certificate.
    pub attempt_id: GovernanceAttemptId,
}

impl core::fmt::Display for ParliamentApprovalUnavailable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Parliament cannot enact runtime change set attempt {}: the baseline Policy Jury \
             ballot is the retired timed-OVN ballot, and the anonymous ballot family is an \
             inactive addition that enacts no production certificate",
            hex::encode(self.attempt_id.as_bytes())
        )
    }
}

impl std::error::Error for ParliamentApprovalUnavailable {}

/// A change set Parliament scheduled, as every peer records it.
#[derive(Debug, Clone)]
pub struct ScheduledChangeSet {
    /// Its identity.
    pub id: ChangeSetId,
    /// The change set.
    pub change_set: ChangeSetV1,
}

/// The change set of `attempt` once Parliament's certificate enacted its proposal and every
/// peer records it `Scheduled`.
///
/// Only a genuine enactment can produce that record: the harness reads it from every peer and
/// never writes or injects one.
///
/// TODO(roadmap N14; `specs/parliament_private_ballot_design.md` §12.3 slice 5): this release
/// has no binding ballot that can certify the attempt, so an unscheduled change set fails
/// closed here. Once one exists, drive the attempt through its bodies and ballot with signed
/// transitions, carry the chain to the certificate's due block, then read the record.
///
/// # Errors
/// A peer's runtime route cannot be read, or, on this release, the change set is not
/// scheduled on every peer ([`ParliamentApprovalUnavailable`]).
pub fn await_parliament_schedule(
    network: &Network,
    attempt: &OpenedAttempt,
) -> Result<ScheduledChangeSet> {
    let drafted = &attempt.proposed.drafted;
    let mut scheduled = true;
    for peer in network.peers() {
        scheduled &= runtime_view(peer)?
            .record(drafted.id)
            .is_some_and(|record| {
                record.status == ChangeSetStatusV1::Scheduled
                    && record.change_set == drafted.change_set
            });
    }
    if scheduled {
        return Ok(ScheduledChangeSet {
            id: drafted.id,
            change_set: drafted.change_set.clone(),
        });
    }
    Err(ParliamentApprovalUnavailable {
        attempt_id: attempt.attempt_id,
    }
    .into())
}

/// Have every validator's node sign its support for `scheduled` and relay each attestation,
/// then confirm that every peer records one attestation from each committee member.
///
/// # Errors
/// A node refuses to attest, an attestation is rejected, or a peer's record lacks one.
pub fn attest_on_every_validator(
    network: &Network,
    relay: &Client,
    scheduled: &ScheduledChangeSet,
) -> Result<()> {
    let id_hex = hex::encode(scheduled.id.0);
    let mut height = 0;
    for peer in network.validators() {
        let draft = peer
            .client()
            .client()
            .post_runtime_attest_upgrade_json(&id_hex)
            .wrap_err("the node drafts its attestation")?;
        height = submit_applied(relay, drafted_instruction(&draft)?)
            .wrap_err("relay the attestation")?;
    }
    await_height_on_every_peer(network, height)?;
    for peer in network.peers() {
        let view = runtime_view(peer)?;
        let record = view
            .record(scheduled.id)
            .ok_or_else(|| eyre!("a peer lost the scheduled record"))?;
        let mut attested: Vec<_> = record
            .attestations
            .iter()
            .map(|signed| signed.attestation.validator.clone())
            .collect();
        attested.sort();
        let mut committee = view.committee.clone();
        committee.sort();
        ensure!(
            record.status == ChangeSetStatusV1::Scheduled && attested == committee,
            "a peer does not record one attestation from every committee member"
        );
    }
    Ok(())
}

/// The finalized height `client`'s peer reports.
///
/// # Errors
/// The status read fails.
pub fn tip(client: &Client) -> Result<u64> {
    Ok(client.client().get_sumeragi_status()?.committed_height)
}

/// Wait until every peer has committed `height`. Reads alone never create a block, so the
/// caller has already committed a transaction at that height.
///
/// # Errors
/// A peer stays below `height` for the harness's height timeout.
pub fn await_height_on_every_peer(network: &Network, height: u64) -> Result<()> {
    let deadline = Instant::now() + HEIGHT_TIMEOUT;
    loop {
        let mut behind = false;
        for peer in network.peers() {
            behind |= tip(&peer.client())? < height;
        }
        if !behind {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "peers did not reach height {height} within {HEIGHT_TIMEOUT:?}"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Carry the chain to `height` with signed `Log` transactions, one at a time.
///
/// # Errors
/// A carrier fails, or the chain does not reach `height` in time.
pub fn carry_to_height(network: &Network, client: &Client, height: u64, label: &str) -> Result<()> {
    let deadline = Instant::now() + HEIGHT_TIMEOUT;
    while tip(client)? < height {
        ensure!(
            Instant::now() < deadline,
            "{label}: the chain did not reach height {height}"
        );
        submit_applied(
            client,
            Log::new(Level::INFO, format!("{label}: carrier toward {height}")).into(),
        )?;
    }
    await_height_on_every_peer(network, height)
}

/// A change set every peer reports active.
#[derive(Debug, Clone)]
pub struct ActivatedChangeSet {
    /// Its identity.
    pub id: ChangeSetId,
    /// The change set.
    pub change_set: ChangeSetV1,
}

/// Carry the chain to the effective height of `scheduled` and confirm on every peer that block
/// start activated it: the record is `Activated` at that height, the committed selection is its
/// successor, and the node's release executes that selection.
///
/// # Errors
/// The chain stalls, or a peer reports another outcome.
pub fn await_activation(
    network: &Network,
    client: &Client,
    scheduled: ScheduledChangeSet,
) -> Result<ActivatedChangeSet> {
    let effective = scheduled.change_set.effective_height;
    carry_to_height(network, client, effective, "governed activation")?;
    let successor = scheduled.change_set.successor.to_string();
    for peer in network.peers() {
        let view = runtime_view(peer)?;
        let record = view
            .record(scheduled.id)
            .ok_or_else(|| eyre!("a peer lost the change set"))?;
        ensure!(
            record.status == ChangeSetStatusV1::Activated(effective),
            "a peer reports {:?} instead of activation at {effective}",
            record.status
        );
        ensure!(
            view.selection_id_hex == successor,
            "a peer does not select the successor"
        );
        let release = peer.client().client().get_runtime_release_json()?;
        ensure!(
            release["selection"]["selection_id_hex"].as_str() == Some(successor.as_str())
                && release["selection"]["supported"].as_bool() == Some(true),
            "a peer's release does not execute the successor it selects"
        );
    }
    Ok(ActivatedChangeSet {
        id: scheduled.id,
        change_set: scheduled.change_set,
    })
}

/// Activate `addition` on `network` through the governed path, effective `lead_blocks` after
/// the current tip, with Alice proposing, opening the attempt and relaying attestations.
///
/// # Errors
/// Any step fails; on this release always at Parliament approval
/// ([`ParliamentApprovalUnavailable`]).
pub fn activate_addition(
    network: &Network,
    addition: &str,
    lead_blocks: u64,
    qualification: [u8; 32],
) -> Result<ActivatedChangeSet> {
    let client = network.client();
    let effective_height = tip(&client)?
        .checked_add(lead_blocks)
        .ok_or_else(|| eyre!("effective height overflow"))?;
    let drafted = draft_on_every_validator(network, addition, effective_height, qualification)?;
    let proposed = propose(network, &client, drafted)?;
    let attempt = open_parliament_attempt(network, &client, proposed)?;
    let scheduled = await_parliament_schedule(network, &attempt)?;
    attest_on_every_validator(network, &client, &scheduled)?;
    await_activation(network, &client, scheduled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_harness_notice_is_a_valid_committed_policy() {
        let parameter = notice_policy()
            .to_custom_parameter()
            .expect("canonical notice parameter");
        assert_eq!(
            RuntimeUpgradePolicyV1::from_custom_parameter(&parameter),
            Ok(notice_policy())
        );
        assert_eq!(notice_policy().activation, notice_policy().disable);
    }

    #[test]
    fn approval_unavailability_names_both_missing_ballots() {
        let error = ParliamentApprovalUnavailable {
            attempt_id: GovernanceAttemptId::new([7; 32]),
        }
        .to_string();
        assert!(error.contains("retired timed-OVN ballot"));
        assert!(error.contains("anonymous ballot family"));
        assert!(error.contains(&hex::encode([7_u8; 32])));
    }

    #[test]
    fn the_proposal_identity_binds_the_admitting_block_time() {
        let change_set = ChangeSetV1 {
            network: iroha_data_model::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"n14")),
            ),
            root: RootScopeV1::Universal,
            predecessor: iroha_data_model::runtime::definition::SelectionId([1; 32]),
            successor: iroha_data_model::runtime::definition::SelectionId([2; 32]),
            effective_height: 9,
            publish: Vec::new(),
            transitions: Vec::new(),
            qualification: [3; 32],
        };
        let early = runtime_upgrade_proposal(&ALICE_ID, &change_set, NOTICE, 1_000);
        let late = runtime_upgrade_proposal(&ALICE_ID, &change_set, NOTICE, 1_001);
        assert_ne!(early.fingerprint(), late.fingerprint());
        assert_eq!(
            early.fingerprint(),
            runtime_upgrade_proposal(&ALICE_ID, &change_set, NOTICE, 1_000).fingerprint()
        );
    }

    #[test]
    fn the_runtime_route_decodes_its_selection_committee_and_records() {
        let key = iroha_crypto::KeyPair::from_seed(vec![5; 32], iroha_crypto::Algorithm::Ed25519)
            .public_key()
            .clone();
        let committee_key = key.to_string();
        let route = norito::json!({
            "selection_id_hex": "ab",
            "selection": {},
            "committee": [committee_key],
            "committee_digest_hex": "cd",
            "items": [],
        });
        let view = RuntimeView::from_route(&route).expect("decode the route");
        assert_eq!(view.selection_id_hex, "ab");
        assert_eq!(view.committee, vec![key]);
        assert!(view.records.is_empty());
        assert!(view.record(ChangeSetId([0; 32])).is_none());
    }

    #[test]
    fn a_draft_response_yields_exactly_one_instruction() {
        assert!(drafted_instruction(&norito::json!({"ok": true, "tx_instructions": []})).is_err());
        iroha_test_network::init_instruction_registry();
        let log: InstructionBox = Log::new(Level::INFO, "n14".to_owned()).into();
        let (wire_id, frame) = iroha_data_model::isi::framed_instruction_payload(&log)
            .expect("a registered instruction has a canonical frame");
        let payload_hex = hex::encode(frame);
        let one = norito::json!({
            "ok": true,
            "tx_instructions": [{ "wire_id": wire_id, "payload_hex": payload_hex }],
        });
        let decoded = drafted_instruction(&one).expect("decode the drafted instruction");
        assert_eq!(
            decoded.as_any().downcast_ref::<Log>(),
            log.as_any().downcast_ref::<Log>()
        );
        let two = norito::json!({
            "ok": true,
            "tx_instructions": [
                { "wire_id": wire_id, "payload_hex": payload_hex },
                { "wire_id": wire_id, "payload_hex": payload_hex },
            ],
        });
        assert!(drafted_instruction(&two).is_err());
    }
}
