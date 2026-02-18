#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Negative-path integration coverage for Sumeragi evidence and reconfiguration.
use std::{thread, time::Duration};

use eyre::{Result, bail, ensure};
use integration_tests::sandbox;
use iroha::{
    client::{Client, SumeragiEvidenceListFilter},
    data_model::{
        Level,
        isi::{Log, SetParameter},
        parameter::{
            Parameter,
            system::{SumeragiConsensusMode, SumeragiNposParameters, SumeragiParameter},
        },
        prelude::TransactionBuilder,
    },
};
use iroha_core::sumeragi::{
    consensus::{NPOS_TAG, PERMISSIONED_TAG, Phase, Vote, vote_preimage},
    network_topology::Topology,
};
use iroha_crypto::{Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    ChainId,
    block::{
        BlockHeader,
        consensus::{Evidence, EvidenceKind, EvidencePayload},
    },
};
use iroha_test_network::{Network, NetworkBuilder, init_instruction_registry};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use norito::{json::Value, to_bytes};
use tokio::runtime::Runtime;

fn evidence_count(value: &norito::json::Value) -> u64 {
    value
        .get("count")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0)
}

fn evidence_hex(evidence: &Evidence) -> Result<String> {
    Ok(hex::encode(to_bytes(evidence)?))
}

fn sumeragi_mode_tag_and_prf_seed(client: &Client) -> Result<(String, [u8; 32])> {
    for _ in 0..20 {
        let status = client.get_sumeragi_status()?;
        if status.mode_tag.is_empty() {
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        if let Some(seed) = status.prf_epoch_seed {
            return Ok((status.mode_tag, seed));
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("sumeragi status did not report prf_epoch_seed")
}

fn rotated_topology_for_view(
    network: &Network,
    mode_tag: &str,
    prf_seed: [u8; 32],
    height: u64,
    view: u64,
) -> Result<Topology> {
    let mut roster: Vec<_> = network
        .topology_entries()
        .iter()
        .map(|entry| entry.peer.clone())
        .collect();
    roster.sort();
    roster.dedup();
    ensure!(
        !roster.is_empty(),
        "network should expose at least one BLS peer id"
    );

    let mut topology = Topology::new(roster);
    match mode_tag {
        PERMISSIONED_TAG => {
            topology.shuffle_prf(prf_seed, height);
            topology.nth_rotation(view);
        }
        NPOS_TAG => {
            let leader = topology.leader_index_prf(prf_seed, height, view);
            topology.rotate_preserve_view_to_front(leader);
        }
        other => bail!("unsupported consensus mode tag for evidence votes: {other}"),
    }
    Ok(topology)
}

fn make_vote(seed: u8) -> Vote {
    let hash = Hash::prehashed([seed; 32]);
    Vote {
        phase: Phase::Prepare,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(hash),
        parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
        post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
        height: 10,
        view: 3,
        epoch: 0,
        highest_qc: None,
        signer: 0,
        bls_sig: vec![seed; 96],
    }
}

fn signed_vote(
    seed: u8,
    mode_tag: &str,
    signer: u32,
    chain_id: &ChainId,
    keypair: &KeyPair,
    height: u64,
    view: u64,
    epoch: u64,
) -> Vote {
    let hash = Hash::prehashed([seed; 32]);
    let mut vote = Vote {
        phase: Phase::Prepare,
        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(hash),
        parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
        post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
        height,
        view,
        epoch,
        highest_qc: None,
        signer,
        bls_sig: Vec::new(),
    };
    let preimage = vote_preimage(chain_id, mode_tag, &vote);
    let signature = Signature::new(keypair.private_key(), &preimage);
    vote.bls_sig = signature.payload().to_vec();
    vote
}

fn valid_double_prepare_evidence(
    network: &Network,
    client: &Client,
    height: u64,
    view: u64,
) -> Result<(Evidence, Vote, Vote)> {
    let (mode_tag, prf_seed) = sumeragi_mode_tag_and_prf_seed(client)?;
    let topology = rotated_topology_for_view(network, mode_tag.as_str(), prf_seed, height, view)?;
    let Some(peer) = topology.as_ref().first() else {
        bail!("rotated topology unexpectedly empty");
    };
    let Some(signer_kp) = network.peers().iter().find_map(|peer_ref| {
        if peer_ref.bls_public_key() == Some(peer.public_key()) {
            peer_ref.bls_key_pair().cloned()
        } else {
            None
        }
    }) else {
        bail!("unable to resolve BLS keypair for rotated signer");
    };

    let signer_idx = 0u32;
    let chain_id = network.chain_id();
    let v1 = signed_vote(0x90, mode_tag.as_str(), signer_idx, &chain_id, &signer_kp, height, view, 0);
    let v2 = signed_vote(0x91, mode_tag.as_str(), signer_idx, &chain_id, &signer_kp, height, view, 0);
    let evidence = Evidence {
        kind: EvidenceKind::DoublePrepare,
        payload: EvidencePayload::DoubleVote {
            v1: v1.clone(),
            v2: v2.clone(),
        },
    };
    Ok((evidence, v1, v2))
}

fn set_evidence_horizon(client: &Client, horizon: u64) -> Result<()> {
    let params = SumeragiNposParameters {
        evidence_horizon_blocks: horizon,
        ..SumeragiNposParameters::default()
    };
    client.submit_blocking(SetParameter::new(Parameter::Custom(
        params.into_custom_parameter(),
    )))?;
    Ok(())
}

fn advance_to_height(
    runtime: &Runtime,
    network: &Network,
    client: &Client,
    target: u64,
    label: &str,
) -> Result<()> {
    let status = client.get_status()?;
    for idx in status.blocks..target {
        client.submit_blocking(Log::new(Level::INFO, format!("{label} tick {idx}")))?;
    }
    runtime.block_on(network.ensure_blocks_with(|height| height.total >= target))?;
    Ok(())
}

#[test]
fn posting_structurally_invalid_evidence_is_rejected() -> Result<()> {
    init_instruction_registry();

    let Some((network, runtime)) = start_network(stringify!(
        posting_structurally_invalid_evidence_is_rejected
    ))?
    else {
        return Ok(());
    };
    runtime.block_on(network.ensure_blocks_with(|height| height.total >= 2))?;
    let client = network.client();

    let before = evidence_count(&client.get_sumeragi_evidence_count_json()?);
    ensure!(
        before == 0,
        "expected empty evidence store on fresh network, got {before}"
    );

    let vote = make_vote(0x42);
    let forged = Evidence {
        kind: EvidenceKind::DoublePrepare,
        payload: EvidencePayload::DoubleVote {
            v1: vote.clone(),
            v2: vote,
        },
    };
    let hex_payload = evidence_hex(&forged)?;
    let err = client
        .post_sumeragi_evidence_hex(&hex_payload)
        .expect_err("invalid evidence must be rejected");
    ensure!(
        err.to_string().contains("invalid consensus evidence"),
        "expected validation error for invalid evidence payload, got {err:?}"
    );

    let after = evidence_count(&client.get_sumeragi_evidence_count_json()?);
    ensure!(
        after == 0,
        "invalid evidence must not be persisted, found {after} entries"
    );

    Ok(())
}

#[test]
fn posting_evidence_with_mismatched_signer_is_rejected() -> Result<()> {
    init_instruction_registry();

    let Some((network, runtime)) = start_network(stringify!(
        posting_evidence_with_mismatched_signer_is_rejected
    ))?
    else {
        return Ok(());
    };
    runtime.block_on(network.ensure_blocks_with(|height| height.total >= 2))?;
    let client = network.client();

    let v1 = make_vote(0x11);
    let mut v2 = v1.clone();
    v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x12; 32]));
    v2.signer = v1.signer.saturating_add(1);
    let evidence = Evidence {
        kind: EvidenceKind::DoublePrepare,
        payload: EvidencePayload::DoubleVote { v1, v2 },
    };

    let err = client
        .post_sumeragi_evidence_hex(&evidence_hex(&evidence)?)
        .expect_err("signer mismatch must be rejected");
    ensure!(
        err.to_string().contains("invalid consensus evidence"),
        "expected invalid evidence error, got {err:?}"
    );
    Ok(())
}

#[test]
fn posting_evidence_with_kind_payload_mismatch_is_rejected() -> Result<()> {
    init_instruction_registry();

    let Some((network, runtime)) = start_network(stringify!(
        posting_evidence_with_kind_payload_mismatch_is_rejected
    ))?
    else {
        return Ok(());
    };
    runtime.block_on(network.ensure_blocks_with(|height| height.total >= 2))?;
    let client = network.client();

    let v1 = make_vote(0x51);
    let mut v2 = v1.clone();
    v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x52; 32]));
    let evidence = Evidence {
        kind: EvidenceKind::InvalidQc,
        payload: EvidencePayload::DoubleVote { v1, v2 },
    };

    let err = client
        .post_sumeragi_evidence_hex(&evidence_hex(&evidence)?)
        .expect_err("kind/payload mismatch must be rejected");
    ensure!(
        err.to_string().contains("invalid consensus evidence"),
        "expected invalid evidence error, got {err:?}"
    );
    Ok(())
}

#[test]
fn posting_evidence_with_conflicting_height_is_rejected() -> Result<()> {
    init_instruction_registry();

    let Some((network, runtime)) = start_network(stringify!(
        posting_evidence_with_conflicting_height_is_rejected
    ))?
    else {
        return Ok(());
    };
    runtime.block_on(network.ensure_blocks_with(|height| height.total >= 2))?;
    let client = network.client();

    let v1 = make_vote(0x21);
    let mut v2 = v1.clone();
    v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x22; 32]));
    v2.height = v2.height.saturating_add(1);
    let evidence = Evidence {
        kind: EvidenceKind::DoublePrepare,
        payload: EvidencePayload::DoubleVote { v1, v2 },
    };

    let err = client
        .post_sumeragi_evidence_hex(&evidence_hex(&evidence)?)
        .expect_err("height mismatch must be rejected");
    ensure!(
        err.to_string().contains("invalid consensus evidence"),
        "expected invalid evidence error, got {err:?}"
    );
    Ok(())
}

#[test]
fn posting_evidence_with_conflicting_view_is_rejected() -> Result<()> {
    init_instruction_registry();

    let Some((network, runtime)) = start_network(stringify!(
        posting_evidence_with_conflicting_view_is_rejected
    ))?
    else {
        return Ok(());
    };
    runtime.block_on(network.ensure_blocks_with(|height| height.total >= 2))?;
    let client = network.client();

    let v1 = make_vote(0x31);
    let mut v2 = v1.clone();
    v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x32; 32]));
    v2.view = v2.view.saturating_add(1);
    let evidence = Evidence {
        kind: EvidenceKind::DoublePrepare,
        payload: EvidencePayload::DoubleVote { v1, v2 },
    };

    let err = client
        .post_sumeragi_evidence_hex(&evidence_hex(&evidence)?)
        .expect_err("view mismatch must be rejected");
    ensure!(
        err.to_string().contains("invalid consensus evidence"),
        "expected invalid evidence error, got {err:?}"
    );
    Ok(())
}

#[test]
fn posting_evidence_with_conflicting_epoch_is_rejected() -> Result<()> {
    init_instruction_registry();

    let Some((network, runtime)) = start_network(stringify!(
        posting_evidence_with_conflicting_epoch_is_rejected
    ))?
    else {
        return Ok(());
    };
    runtime.block_on(network.ensure_blocks_with(|height| height.total >= 2))?;
    let client = network.client();

    let mut v1 = make_vote(0x35);
    v1.epoch = 1;
    let mut v2 = v1.clone();
    v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x36; 32]));
    v2.epoch = 2;
    let evidence = Evidence {
        kind: EvidenceKind::DoublePrepare,
        payload: EvidencePayload::DoubleVote { v1, v2 },
    };

    let err = client
        .post_sumeragi_evidence_hex(&evidence_hex(&evidence)?)
        .expect_err("epoch mismatch must be rejected");
    ensure!(
        err.to_string().contains("invalid consensus evidence"),
        "expected invalid evidence error, got {err:?}"
    );
    Ok(())
}

#[test]
fn posting_evidence_with_missing_signature_is_rejected() -> Result<()> {
    init_instruction_registry();

    let Some((network, runtime)) = start_network(stringify!(
        posting_evidence_with_missing_signature_is_rejected
    ))?
    else {
        return Ok(());
    };
    runtime.block_on(network.ensure_blocks_with(|height| height.total >= 2))?;
    let client = network.client();

    let mut v1 = make_vote(0x41);
    let mut v2 = v1.clone();
    v2.block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x42; 32]));
    v1.bls_sig.clear();
    let evidence = Evidence {
        kind: EvidenceKind::DoublePrepare,
        payload: EvidencePayload::DoubleVote { v1, v2 },
    };

    let err = client
        .post_sumeragi_evidence_hex(&evidence_hex(&evidence)?)
        .expect_err("missing signature must be rejected");
    ensure!(
        err.to_string().contains("invalid consensus evidence"),
        "expected invalid evidence error, got {err:?}"
    );
    Ok(())
}

#[test]
fn mode_activation_height_requires_next_mode_and_future_height() -> Result<()> {
    init_instruction_registry();

    let Some((network, runtime)) = start_network(stringify!(
        mode_activation_height_requires_next_mode_and_future_height
    ))?
    else {
        return Ok(());
    };
    runtime.block_on(network.ensure_blocks_with(|height| height.total >= 2))?;
    let client = network.client();

    let err = client
        .submit_blocking(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::ModeActivationHeight(5),
        )))
        .expect_err("mode_activation_height without next_mode must fail");
    ensure!(
        err.to_string()
            .contains("mode_activation_height requires next_mode"),
        "expected missing-next-mode error, got {err:?}"
    );

    let current_height = client.get_status()?.blocks;
    let invalid_height_tx = TransactionBuilder::new(network.chain_id(), ALICE_ID.clone())
        .with_instructions([
            SetParameter::new(Parameter::Sumeragi(SumeragiParameter::NextMode(
                SumeragiConsensusMode::Permissioned,
            ))),
            SetParameter::new(Parameter::Sumeragi(
                SumeragiParameter::ModeActivationHeight(current_height),
            )),
        ])
        .sign(ALICE_KEYPAIR.private_key());
    let err = client
        .submit_transaction_blocking(&invalid_height_tx)
        .expect_err("mode_activation_height equal to current height must fail");
    ensure!(
        err.to_string().contains("mode_activation_height")
            && err
                .to_string()
                .contains("greater than current block height"),
        "expected height validation error, got {err:?}"
    );

    let desired_height = client.get_status()?.blocks.saturating_add(3);
    let staged_tx = TransactionBuilder::new(network.chain_id(), ALICE_ID.clone())
        .with_instructions([
            SetParameter::new(Parameter::Sumeragi(SumeragiParameter::NextMode(
                SumeragiConsensusMode::Npos,
            ))),
            SetParameter::new(Parameter::Sumeragi(
                SumeragiParameter::ModeActivationHeight(desired_height),
            )),
        ])
        .sign(ALICE_KEYPAIR.private_key());
    client.submit_transaction_blocking(&staged_tx)?;

    advance_to_height(
        &runtime,
        &network,
        &client,
        desired_height,
        "mode activation staged",
    )?;
    let params = client.get_parameters()?;
    let sp = params.sumeragi();
    ensure!(
        sp.next_mode == Some(SumeragiConsensusMode::Npos),
        "next_mode should be staged as Npos, params={sp:?}"
    );
    ensure!(
        sp.mode_activation_height == Some(desired_height),
        "mode_activation_height should equal {desired_height}, params={sp:?}"
    );

    Ok(())
}

#[test]
fn joint_consensus_switches_mode_at_activation_height() -> Result<()> {
    init_instruction_registry();

    let Some((network, runtime)) = start_network(stringify!(
        joint_consensus_switches_mode_at_activation_height
    ))?
    else {
        return Ok(());
    };
    runtime.block_on(network.ensure_blocks_with(|height| height.total >= 2))?;
    let client = network.client();

    let initial = client.get_sumeragi_collectors_json()?;
    ensure!(
        collectors_consensus_mode(&initial)
            .is_some_and(|mode| mode.eq_ignore_ascii_case("permissioned")),
        "collectors endpoint should report Permissioned before activation, payload={initial:?}"
    );

    let status = client.get_status()?;
    let activation_height = status.blocks + 3;
    let switch_tx = TransactionBuilder::new(network.chain_id(), ALICE_ID.clone())
        .with_instructions([
            SetParameter::new(Parameter::Sumeragi(SumeragiParameter::NextMode(
                SumeragiConsensusMode::Npos,
            ))),
            SetParameter::new(Parameter::Sumeragi(
                SumeragiParameter::ModeActivationHeight(activation_height),
            )),
        ])
        .sign(ALICE_KEYPAIR.private_key());
    client.submit_transaction_blocking(&switch_tx)?;

    advance_to_height(
        &runtime,
        &network,
        &client,
        activation_height.saturating_add(1),
        "joint consensus activation",
    )?;
    wait_for_collectors_mode(&client, "npos", 40, Duration::from_millis(200))?;

    let final_snapshot = client.get_sumeragi_collectors_json()?;
    ensure!(
        collectors_consensus_mode(&final_snapshot)
            .is_some_and(|mode| mode.eq_ignore_ascii_case("npos")),
        "collectors endpoint should report Npos after activation, payload={final_snapshot:?}"
    );

    Ok(())
}

#[test]
fn posting_stale_evidence_is_not_persisted() -> Result<()> {
    init_instruction_registry();

    let Some((network, runtime)) =
        start_network(stringify!(posting_stale_evidence_is_not_persisted))?
    else {
        return Ok(());
    };
    let client = network.client();
    advance_to_height(&runtime, &network, &client, 3, "stale evidence seed")?;

    set_evidence_horizon(&client, 1)?;
    advance_to_height(&runtime, &network, &client, 4, "stale evidence horizon")?;

    let before = evidence_count(&client.get_sumeragi_evidence_count_json()?);

    let status = client.get_status()?;
    let stale_height = status.blocks.saturating_sub(2);
    let (evidence, _first, _second) =
        valid_double_prepare_evidence(&network, &client, stale_height, 0)?;

    client.post_sumeragi_evidence_hex(&evidence_hex(&evidence)?)?;

    let status_before = client.get_status()?;
    advance_to_height(
        &runtime,
        &network,
        &client,
        status_before.blocks.saturating_add(2),
        "stale evidence advance",
    )?;

    let after = evidence_count(&client.get_sumeragi_evidence_count_json()?);
    ensure!(
        before == after,
        "stale evidence must not increase persisted count (before={before}, after={after})"
    );
    Ok(())
}

#[allow(clippy::too_many_lines)]
#[test]
fn posting_valid_double_vote_evidence_is_persisted_for_slashing() -> Result<()> {
    init_instruction_registry();

    let Some((network, runtime)) = start_network(stringify!(
        posting_valid_double_vote_evidence_is_persisted_for_slashing
    ))?
    else {
        return Ok(());
    };
    runtime.block_on(network.ensure_blocks_with(|height| height.total >= 2))?;
    let client = network.client();

    let before = evidence_count(&client.get_sumeragi_evidence_count_json()?);
    ensure!(
        before == 0,
        "expected empty evidence store on fresh network, got {before}"
    );

    let status = client.get_status()?;
    let (evidence, first_vote, second_vote) =
        valid_double_prepare_evidence(&network, &client, status.blocks, 0)?;
    let hex_payload = evidence_hex(&evidence)?;
    let response = client.post_sumeragi_evidence_hex(&hex_payload)?;
    ensure!(
        response.get("status").and_then(Value::as_str) == Some("accepted"),
        "expected POST response to report status=accepted, got {response:?}"
    );
    ensure!(
        response.get("kind").and_then(Value::as_str) == Some("DoublePrepare"),
        "expected response kind DoublePrepare, got {response:?}"
    );

    let after = wait_for_evidence_count_at_least(
        &client,
        before.saturating_add(1),
        40,
        Duration::from_millis(200),
    )?;
    ensure!(
        after == before + 1,
        "valid evidence should increase persisted count (before={before}, after={after})"
    );

    let filter = SumeragiEvidenceListFilter {
        limit: Some(1),
        ..SumeragiEvidenceListFilter::default()
    };
    let snapshot = client.get_sumeragi_evidence_list_json(&filter)?;
    let total = snapshot
        .get("total")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    ensure!(
        total == after,
        "evidence list total ({total}) should match persisted count ({after})"
    );
    let Some(items) = snapshot.get("items").and_then(Value::as_array) else {
        bail!("evidence list missing `items` array: {snapshot:?}");
    };
    let Some(record) = items.first() else {
        bail!("evidence list returned empty array: {snapshot:?}");
    };

    ensure!(
        record.get("kind").and_then(Value::as_str) == Some("DoublePrepare"),
        "record kind mismatch: {record:?}"
    );
    ensure!(
        record.get("phase").and_then(Value::as_str) == Some("Prepare"),
        "record phase mismatch: {record:?}"
    );
    let recorded_height = record
        .get("recorded_height")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    ensure!(
        recorded_height == first_vote.height,
        "recorded height {recorded_height} should equal subject height {}",
        first_vote.height
    );
    let recorded_view = record
        .get("recorded_view")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    ensure!(
        recorded_view == first_vote.view,
        "recorded view {recorded_view} should equal subject view {}",
        first_vote.view
    );
    let recorded_ms = record
        .get("recorded_ms")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    ensure!(recorded_ms > 0, "recorded timestamp must be populated");

    let height = record
        .get("height")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    ensure!(
        height == first_vote.height,
        "record height {height} should equal subject height {}",
        first_vote.height
    );
    let view = record
        .get("view")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    ensure!(
        view == first_vote.view,
        "record view {view} should equal subject view {}",
        first_vote.view
    );
    let epoch = record
        .get("epoch")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    ensure!(
        epoch == first_vote.epoch,
        "record epoch {epoch} should equal subject epoch {}",
        first_vote.epoch
    );
    let signer = record
        .get("signer")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    ensure!(
        signer == u64::from(first_vote.signer),
        "record signer {signer} should equal {}",
        first_vote.signer
    );
    let block_hash_1 = record
        .get("block_hash_1")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let block_hash_2 = record
        .get("block_hash_2")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let expected_hash_1 = hex::encode(first_vote.block_hash.as_ref());
    let expected_hash_2 = hex::encode(second_vote.block_hash.as_ref());
    ensure!(
        block_hash_1 == expected_hash_1,
        "first block hash mismatch: expected {expected_hash_1}, got {block_hash_1}"
    );
    ensure!(
        block_hash_2 == expected_hash_2,
        "second block hash mismatch: expected {expected_hash_2}, got {block_hash_2}"
    );

    Ok(())
}

fn start_network(context: &'static str) -> Result<Option<(sandbox::SerializedNetwork, Runtime)>> {
    // Evidence list/count endpoints are currently gated behind developer telemetry outputs.
    // Enable them for this negative-path suite so we can assert persistence behavior.
    let builder = NetworkBuilder::new().with_config_layer(|t| {
        t.write(["telemetry_profile"], "developer");
    });
    let Some((network, runtime)) = sandbox::start_network_blocking_or_skip(builder, context)?
    else {
        return Ok(None);
    };
    let client = network.client();
    advance_to_height(
        &runtime,
        &network,
        &client,
        2,
        "sumeragi negative bootstrap",
    )?;
    Ok(Some((network, runtime)))
}

fn collectors_consensus_mode(value: &Value) -> Option<&str> {
    value.get("consensus_mode").and_then(Value::as_str)
}

fn wait_for_collectors_mode(
    client: &Client,
    expected: &str,
    attempts: usize,
    delay: Duration,
) -> Result<()> {
    for attempt in 0..attempts {
        let snapshot = client.get_sumeragi_collectors_json()?;
        if collectors_consensus_mode(&snapshot)
            .is_some_and(|mode| mode.eq_ignore_ascii_case(expected))
        {
            return Ok(());
        }
        if attempt + 1 < attempts {
            thread::sleep(delay);
        }
    }
    bail!(
        "collectors mode did not switch to {expected} within {} attempts",
        attempts
    );
}

fn wait_for_evidence_count_at_least(
    client: &Client,
    expected: u64,
    attempts: usize,
    delay: Duration,
) -> Result<u64> {
    for attempt in 0..attempts {
        let count = evidence_count(&client.get_sumeragi_evidence_count_json()?);
        if count >= expected {
            return Ok(count);
        }
        if attempt + 1 < attempts {
            thread::sleep(delay);
        }
    }
    bail!(
        "evidence count did not reach {expected} within {} attempts",
        attempts
    );
}
