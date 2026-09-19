//! Historical Native execution under the fixture's independently anchored global finality.
//!
//! The complete execution wire authenticates the retained Decision/source bytes. We also
//! check exact fixture committee signatures and regenerate RS16. This is not the stronger
//! opening-context write-witness proof used by NativeExecutionEvidenceVerifier: Torii does
//! not expose that witness here, and this reader creates no live lane authority.

use super::*;
use iroha_core::{
    kura::{BlockIndex, BlockStore, Kura},
    torii_proxy::decode_and_validate_lane_admitted_input_v1,
};
use iroha_crypto::{Signature, bls_normal_pop_verify};
use iroha_data_model::block::{
    SignedBlock,
    consensus_v2::{encode_payload_chunks, payload_chunk_root},
    lane_consensus::{LaneDecisionV1, LaneManifestV1, LaneMessageV1, LanePhaseV1, LaneQcV1},
    proofs::AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1,
};

/// Require a complete typed Network proof and the exact Native execution source.
pub(super) fn authenticated_native_execution(
    finality: &FixtureFinality,
    peer: &PeerId,
    client: &iroha::client::Client,
    store: &Path,
    transaction: &SignedTransaction,
    height: u64,
    route: RoutingDecision,
    expected_committee: Option<&[PeerId]>,
) -> Result<Vec<u8>> {
    let details = client.get_successful_transaction_details(transaction.hash_as_entrypoint())?;
    let committed = &details.transaction;
    ensure!(
        committed.entrypoint() == &TransactionEntrypoint::External(transaction.clone())
            && committed.result().0.is_ok(),
        "committed transaction bytes or result changed"
    );
    let commitment =
        finality.execution_commitment(peer, client, height, *committed.block_hash())?;
    let bytes = client.get_canonical_executed_block_wire(
        NonZeroU64::new(height).ok_or_else(|| eyre!("zero Applied height"))?,
        committed,
        &commitment,
    )?;
    let block = decode_framed_signed_block(&bytes)?;
    ensure!(
        committed.verify_inclusion_in_authenticated_execution(&block, &commitment),
        "exact typed Network input/output proofs differ from authenticated execution"
    );
    client.get_lane_lifecycle_status()?.validate()?;
    let context = block
        .execution_context()
        .ok_or_else(|| eyre!("missing execution context"))?;
    ensure!(
        context.external.is_empty() && context.merge_entry.is_none(),
        "Native carrier contains ordinary or retired merged execution sources"
    );
    let batch = context
        .native_lane_decisions
        .as_ref()
        .ok_or_else(|| eyre!("QueuePlanSynced carrier omitted Native Decisions"))?;
    batch.validate_structure().map_err(|error| eyre!(error))?;
    let matches: Vec<_> = batch
        .groups
        .iter()
        .enumerate()
        .filter(|(_, group)| {
            group.payload.input.entrypoint.hash() == transaction.hash_as_entrypoint()
        })
        .collect();
    ensure!(
        matches.len() == 1,
        "exact transaction must have one Native group, got {}",
        matches.len()
    );
    let (index, group) = matches[0];
    ensure!(
        committed.entrypoint_proof.leaf_index() as usize == index
            && group.payload.input.entrypoint == *committed.entrypoint()
            && group
                .payload
                .input
                .routing_plan()
                .map_err(|error| eyre!(error))?
                == RoutingPlan::single(route)
            && group.payload.descriptor.slots.len() == 1
            && group.payload.descriptor.slots[0].route == route
            && group.decisions.len() == 1,
        "successful signed transaction executed on a different Native route or source position"
    );
    let admitted = norito::encode_canonical(&group.payload.input)?;
    let verified = decode_and_validate_lane_admitted_input_v1(&finality.network_id, &admitted)
        .map_err(|error| eyre!(error))?;
    ensure!(
        verified.input() == &group.payload.input,
        "complete input authentication changed the source"
    );
    let descriptor = &group.payload.descriptor;
    ensure!(
        descriptor.admission_priority.carrier_height < height,
        "Native execution predates its first admission"
    );
    let proofs = {
        let cache = finality
            .peers
            .get(peer)
            .ok_or_else(|| eyre!("unknown fixture peer"))?
            .lock()
            .map_err(|_| eyre!("poisoned finality cache"))?;
        cache.proofs.clone()
    };
    let mut blocks = BlockStore::open_read_only(Kura::canonical_storage_paths(store).0)?;
    // The cache contains a contiguous, independently anchored prefix. Read only that
    // prefix; hashes, complete wire commitments and first occurrence all rejoin.
    let mut first = None;
    for at in 1..=descriptor.admission_priority.carrier_height {
        let proof = proofs
            .get(&at)
            .ok_or_else(|| eyre!("authenticated first-admission history has a gap"))?;
        let source = authenticated_local_carrier(&mut blocks, proof)?;
        if let Some(context) = source.execution_context() {
            for (position, encoded) in context.queue_plan_admissions.iter().enumerate() {
                let input =
                    decode_and_validate_lane_admitted_input_v1(&finality.network_id, encoded)
                        .map_err(|error| eyre!(error))?;
                if input.input().certificate.binding.registry_key()
                    == group.payload.input.certificate.binding.registry_key()
                {
                    ensure!(
                        encoded == &admitted,
                        "earlier admission rebinds the immutable source"
                    );
                    first.get_or_insert((at, position, source.hash()));
                }
            }
        }
    }
    ensure!(
        first
            == Some((
                descriptor.admission_priority.carrier_height,
                descriptor.admission_priority.admission_index as usize,
                descriptor.admission_carrier_hash
            )),
        "Native source is not the exact earliest finalized carrier input"
    );
    let committee: Vec<_> = finality
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect();
    if let Some(expected) = expected_committee {
        ensure!(
            expected == committee,
            "new lane committee differs from the independently pinned four validators"
        );
    }
    let payload = norito::encode_canonical(&group.payload)?;
    verify_fixture_native_decision(
        &payload,
        &group.decisions[0],
        &committee,
        &finality.validator_pops,
    )?;
    Ok(bytes)
}

fn authenticated_local_carrier(
    store: &mut BlockStore,
    proof: &BridgeFinalityProof,
) -> Result<SignedBlock> {
    let height = proof.block_header.height().get();
    let mut index = BlockIndex::default();
    store.read_block_indices(
        height
            .checked_sub(1)
            .ok_or_else(|| eyre!("zero source height"))?,
        std::slice::from_mut(&mut index),
    )?;
    let len = usize::try_from(index.length)?;
    ensure!(
        (1..=AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1).contains(&len),
        "source carrier is absent or exceeds bounded wire length"
    );
    let mut bytes = vec![0; len];
    store.read_block_data(index.start, &mut bytes)?;
    let execution = proof.finality_artifact.commit_qc.execution_commitment;
    execution.validate()?;
    ensure!(
        execution.executed_block_wire_len == index.length
            && execution.executed_block_wire_hash == Hash::new(&bytes),
        "first-admission carrier differs from its independently authenticated execution wire"
    );
    let block =
        norito::with_decode_limits_scope(norito::canonical_decode_limits(bytes.len()), || {
            decode_framed_signed_block(&bytes)
        })?;
    ensure!(
        block.header() == proof.block_header
            && block.hash() == proof.finality_artifact.block_hash
            && block.encode_wire()? == bytes,
        "first-admission block/header/wire binding changed"
    );
    block
        .validate_proposal_commitments()
        .map_err(|error| eyre!(error))?;
    block.validate_output_merkle_cache()?;
    Ok(block)
}

fn verify_fixture_native_manifest(payload: &[u8], manifest: &LaneManifestV1) -> Result<()> {
    manifest.validate_availability()?;
    let chunks = encode_payload_chunks(manifest.layout, payload)?;
    let hashes = chunks.iter().map(Hash::new).collect::<Vec<_>>();
    ensure!(
        manifest.value.payload_hash == Hash::new(payload)
            && Some(manifest.chunk_root) == payload_chunk_root(&hashes)
            && manifest.chunk_count as usize == chunks.len()
            && manifest.byte_len == payload.len() as u64,
        "Native manifest differs from the exact regenerated signed RS16 codeword"
    );
    Ok(())
}

/// Compose the exact signed value and regenerated codeword. The containing
/// batch separately binds this body to its complete descriptor and route slots.
fn verify_fixture_native_decision(
    payload: &[u8],
    decision: &LaneDecisionV1,
    committee: &[PeerId],
    pops: &[Vec<u8>],
) -> Result<()> {
    ensure!(
        decision.commit_qc.statement.value == decision.manifest.value,
        "Native Commit statement differs from its exact manifest value"
    );
    verify_fixture_native_commit(&decision.commit_qc, committee, pops)?;
    verify_fixture_native_manifest(payload, &decision.manifest)
}

/// The fixture independently owns the full ordered committee and all its PoPs.
/// Native Commit shares sign the exact value/instance/height/view statement; a
/// Prepare proof or self-described alternate signer roster cannot substitute.
fn verify_fixture_native_commit(
    qc: &LaneQcV1,
    committee: &[PeerId],
    pops: &[Vec<u8>],
) -> Result<()> {
    ensure!(
        committee.len() == 4
            && pops.len() == 4
            && committee.windows(2).all(|pair| pair[0] < pair[1]),
        "fixture Native authority is not exact ordered 3f+1"
    );
    // Cryptographically valid shares do not excuse an invalid immutable origin
    // or zero source identities. Reuse the native codec's complete shape rules.
    LaneMessageV1::QuorumCertificate(qc.clone()).validate_shape(committee.len())?;
    for (peer, pop) in committee.iter().zip(pops) {
        bls_normal_pop_verify(peer.public_key(), pop)?;
    }
    ensure!(
        qc.statement.phase == LanePhaseV1::Commit
            && qc.shares.len() == 3
            && qc
                .shares
                .windows(2)
                .all(|pair| pair[0].signer < pair[1].signer),
        "Native Decision lacks exact ordered 2f+1 Commit shares"
    );
    let signed = qc.statement.signature_preimage()?;
    for share in &qc.shares {
        let signer = committee
            .get(usize::try_from(share.signer)?)
            .ok_or_else(|| eyre!("Native signer outside exact fixture committee"))?;
        ensure!(
            share.signature.len() == 96,
            "Native share has noncanonical BLS length"
        );
        Signature::try_from_bytes(&share.signature)?.verify(signer.public_key(), &signed)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::block::lane_consensus::{
        LaneRoundV1, LaneSignatureShareV1, LaneValueKindV1, LaneValueRefV1, LaneVoteStatementV1,
        lane_availability_hash,
    };

    fn fixture_native_keys() -> Vec<KeyPair> {
        let mut keys = (1..=4)
            .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| PeerId::new(key.public_key().clone()));
        keys
    }

    fn resign_fixture_commit(qc: &mut LaneQcV1) {
        let keys = fixture_native_keys();
        let bytes = qc.statement.signature_preimage().unwrap();
        for share in &mut qc.shares {
            share.signature = Signature::new(keys[share.signer as usize].private_key(), &bytes)
                .payload()
                .to_vec();
        }
    }

    fn signed_commit() -> (LaneQcV1, Vec<PeerId>, Vec<Vec<u8>>) {
        let keys = fixture_native_keys();
        let instance = Hash::new(b"catalog proof test instance");
        let statement = LaneVoteStatementV1 {
            round: LaneRoundV1 {
                instance_id: instance,
                lane_height: 3,
                voting_view: 2,
            },
            phase: LanePhaseV1::Commit,
            value: LaneValueRefV1 {
                instance_id: instance,
                admitted_binding_hash: Hash::new(b"exact admission binding"),
                kind: LaneValueKindV1::Execution,
                origin_view: 1,
                origin_producer: 2,
                descriptor_hash: Hash::new(b"complete input descriptor"),
                payload_hash: Hash::new(b"complete input payload"),
                availability_hash: Hash::new(b"signed RS16 commitment"),
            },
        };
        let bytes = statement.signature_preimage().unwrap();
        let shares = keys
            .iter()
            .take(3)
            .enumerate()
            .map(|(signer, key)| LaneSignatureShareV1 {
                signer: u32::try_from(signer).unwrap(),
                signature: Signature::new(key.private_key(), &bytes).payload().to_vec(),
            })
            .collect();
        let committee = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect();
        let pops = keys
            .iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
            .collect();
        (LaneQcV1 { statement, shares }, committee, pops)
    }

    #[test]
    fn native_catalog_proof_requires_exact_committee_quorum_and_statement() {
        let (qc, committee, pops) = signed_commit();
        verify_fixture_native_commit(&qc, &committee, &pops).unwrap();
        for mutation in 0..6 {
            let mut wrong = qc.clone();
            match mutation {
                0 => {
                    wrong.shares.pop();
                }
                1 => {
                    wrong.shares[1] = wrong.shares[0].clone();
                }
                2 => {
                    wrong.statement.phase = LanePhaseV1::Prepare;
                }
                3 => {
                    wrong.statement.value.payload_hash = Hash::new(b"substitution");
                }
                4 => {
                    wrong.statement.round.instance_id = Hash::new(b"foreign instance");
                }
                _ => {
                    wrong.shares[0].signature[0] ^= 1;
                }
            }
            assert!(
                verify_fixture_native_commit(&wrong, &committee, &pops).is_err(),
                "mutation {mutation}"
            );
        }
        let mut outside_origin = qc.clone();
        outside_origin.statement.value.origin_producer = 4;
        resign_fixture_commit(&mut outside_origin);
        // Prove the rejection is structural, not a consequence of stale shares.
        let bytes = outside_origin.statement.signature_preimage().unwrap();
        for share in &outside_origin.shares {
            Signature::try_from_bytes(&share.signature)
                .unwrap()
                .verify(committee[share.signer as usize].public_key(), &bytes)
                .unwrap();
        }
        assert!(verify_fixture_native_commit(&outside_origin, &committee, &pops).is_err());
        let mut reordered = committee.clone();
        reordered.swap(0, 1);
        assert!(verify_fixture_native_commit(&qc, &reordered, &pops).is_err());
        let mut foreign_pops = pops.clone();
        foreign_pops.swap(0, 1);
        assert!(verify_fixture_native_commit(&qc, &committee, &foreign_pops).is_err());
    }

    #[test]
    fn native_catalog_proof_reconstructs_exact_manifest_codeword() {
        let payload = b"the exact complete admitted input and every native route slot";
        let layout = iroha_data_model::block::consensus_v2::recommended_data_availability_layout();
        let chunks = encode_payload_chunks(layout, payload).unwrap();
        let chunk_root =
            payload_chunk_root(&chunks.iter().map(Hash::new).collect::<Vec<_>>()).unwrap();
        let (qc, _, _) = signed_commit();
        let mut value = qc.statement.value;
        value.payload_hash = Hash::new(payload);
        value.availability_hash = lane_availability_hash(
            layout,
            chunk_root,
            payload.len() as u64,
            chunks.len() as u32,
        )
        .unwrap();
        let manifest = LaneManifestV1 {
            value,
            layout,
            chunk_root,
            byte_len: payload.len() as u64,
            chunk_count: chunks.len() as u32,
        };
        verify_fixture_native_manifest(payload, &manifest).unwrap();
        assert!(verify_fixture_native_manifest(b"different input", &manifest).is_err());
        let mut changed = manifest;
        changed.chunk_root = Hash::new(b"different RS16 root");
        changed.value.availability_hash = lane_availability_hash(
            changed.layout,
            changed.chunk_root,
            changed.byte_len,
            changed.chunk_count,
        )
        .unwrap();
        assert!(
            verify_fixture_native_manifest(payload, &changed).is_err(),
            "a self-consistent substituted manifest is not the encoded original body"
        );
    }

    #[test]
    fn native_catalog_proof_joins_signed_value_to_exact_manifest_and_body() {
        let payload = b"the original complete admitted source";
        let substituted_payload = b"a foreign but internally valid complete admitted source";
        let (mut qc, committee, pops) = signed_commit();
        let manifest = |payload: &[u8]| {
            let layout =
                iroha_data_model::block::consensus_v2::recommended_data_availability_layout();
            let chunks = encode_payload_chunks(layout, payload).unwrap();
            let chunk_root =
                payload_chunk_root(&chunks.iter().map(Hash::new).collect::<Vec<_>>()).unwrap();
            let mut value = qc.statement.value;
            value.payload_hash = Hash::new(payload);
            value.availability_hash = lane_availability_hash(
                layout,
                chunk_root,
                payload.len() as u64,
                chunks.len() as u32,
            )
            .unwrap();
            LaneManifestV1 {
                value,
                layout,
                chunk_root,
                byte_len: payload.len() as u64,
                chunk_count: chunks.len() as u32,
            }
        };
        let original = manifest(payload);
        let foreign = manifest(substituted_payload);
        qc.statement.value = original.value;
        resign_fixture_commit(&mut qc);
        let decision = LaneDecisionV1 {
            manifest: original,
            commit_qc: qc,
        };
        verify_fixture_native_decision(payload, &decision, &committee, &pops).unwrap();
        verify_fixture_native_commit(&decision.commit_qc, &committee, &pops).unwrap();
        verify_fixture_native_manifest(substituted_payload, &foreign).unwrap();
        let mut changed = decision.clone();
        changed.manifest = foreign;
        assert!(
            verify_fixture_native_decision(substituted_payload, &changed, &committee, &pops)
                .is_err(),
            "separately valid QC and codeword cannot be joined across source values"
        );
        assert!(
            verify_fixture_native_decision(substituted_payload, &decision, &committee, &pops)
                .is_err(),
            "signed manifest cannot authenticate different input bytes"
        );
    }
}
