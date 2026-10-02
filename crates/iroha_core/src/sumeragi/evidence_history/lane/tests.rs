//! Original global frontier, real signed native ancestry, and retained source refusal tests.
use super::*;
use crate::{
    state::{StateReadOnly, WorldReadOnly},
    sumeragi::{
        crypto::KeyPairSigner,
        lanes::{
            merge::{CommittedLaneBlock, LaneBlockSource},
            record::PreparedLaneWrite,
        },
        runtime_availability::history::{HistoryCapture, HistoryScan},
        test_chain::CertifiedTestChain,
    },
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_model_base::topology::LaneId;
use iroha_sumeragi::{
    availability::{PayloadAuthoring, PayloadBytes},
    crypto::{Crypto, Signer},
    message::{BlockHeader, Qc, Vote, VoteKind},
    types::{AggregateSignature, Bitmap, ControlWitness, SIGNATURE_LEN, Signature},
};
use mv::storage::StorageReadOnly;
use std::{sync::Arc, time::Duration};

#[derive(Default)]
struct OriginalFrames(parking_lot::RwLock<Vec<CommittedLaneBlock>>);
impl LaneBlockSource for OriginalFrames {
    fn tip(
        &self,
        lane: LaneId,
        _: &[u8; 32],
    ) -> Result<Option<u64>, crate::execution_attempt::ExecutionAttemptError<io::Error>> {
        assert_eq!(lane, LaneId::new(7));
        Ok(Some(self.0.read().len() as u64))
    }
    fn block(
        &self,
        lane: LaneId,
        _: &[u8; 32],
        height: u64,
    ) -> Result<
        Option<CommittedLaneBlock>,
        crate::execution_attempt::ExecutionAttemptError<io::Error>,
    > {
        assert_eq!(lane, LaneId::new(7));
        Ok(self
            .0
            .read()
            .get(height.checked_sub(1).unwrap() as usize)
            .cloned())
    }
    fn wait_for(
        &self,
        lane: LaneId,
        incarnation: &[u8; 32],
        height: u64,
        _: Duration,
    ) -> Result<bool, crate::execution_attempt::ExecutionAttemptError<io::Error>> {
        Ok(self
            .tip(lane, incarnation)?
            .is_some_and(|tip| tip >= height))
    }
}
fn capture_parts(
    chain: &CertifiedTestChain,
) -> (
    HistoryCapture,
    iroha_data_model::block::consensus::LaneEvidenceScope,
) {
    let state = chain.state();
    let generation = state.state_view_generation();
    let (capture, scope) = {
        let view = state.view();
        let record = view.world().sumeragi_lanes().lane(LaneId::new(7)).unwrap();
        let tip = view.native_execution_tip().unwrap();
        (
            HistoryCapture::from_view(state, &view, generation)
                .unwrap()
                .unwrap(),
            iroha_data_model::block::consensus::LaneEvidenceScope {
                lane: record.lane,
                incarnation: record.incarnation,
                created_at: record.created_at,
                admission_parent_height: tip.height(),
                admission_parent_hash: tip.iroha_hash(),
                admission_parent_core_hash: tip.core_hash().0,
                admission_parent_result: tip.result().0,
            },
        )
    };
    (capture, scope)
}
fn capture(chain: &CertifiedTestChain) -> LaneEvidenceContext {
    let (capture, scope) = capture_parts(chain);
    let mut history = HistoryScan::open_for_evidence(capture, scope).unwrap();
    history.complete().unwrap();
    history
        .finish_evidence()
        .unwrap_or_else(|(_, error)| panic!("{error}"))
}
fn keys() -> Vec<KeyPair> {
    let mut keys: Vec<_> = (41..45)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect();
    keys.sort_by_key(|key| crate::sumeragi::crypto::core_key(key.public_key()).unwrap());
    keys
}
fn vote_pair(reader: &LaneProofRead, height: u64) -> Evidence {
    let signer = KeyPairSigner::new(&keys()[2]).unwrap();
    let vote = |byte| {
        let mut vote = Vote {
            kind: VoteKind::Prepare,
            instance: reader.instance,
            epoch: reader.cursor.config().epoch.id,
            height,
            view: 5,
            block_hash: Hash32([byte; 32]),
            result: Hash32([73; 32]),
            attest: false,
            signer: 2,
            sig: Signature([0; SIGNATURE_LEN]),
            attestation: None,
        };
        vote.sig = signer.sign(&vote.preimage());
        vote
    };
    Evidence::VoteEquivocation(vote(71), vote(72))
}
fn anchored_chain(count: u64) -> (CertifiedTestChain, crossbeam_epoch::Guard) {
    anchored_chain_with_policy(
        count,
        iroha_data_model::parameter::system::SumeragiNposParameters::default(),
    )
}
fn anchored_chain_with_policy(
    count: u64,
    parameters: iroha_data_model::parameter::system::SumeragiNposParameters,
) -> (CertifiedTestChain, crossbeam_epoch::Guard) {
    anchored_chain_with_config(count, parameters, |_| {})
}
fn anchored_chain_with_config(
    count: u64,
    parameters: iroha_data_model::parameter::system::SumeragiNposParameters,
    configure: fn(&mut crate::sumeragi::test_chain::TestChainConfig),
) -> (CertifiedTestChain, crossbeam_epoch::Guard) {
    let source = Arc::new(OriginalFrames::default());
    let (mut chain, _, guard) =
        crate::sumeragi::runtime_availability::tests::fixed_lane_chain_with_config(
            3,
            Some(parameters),
            source.clone(),
            configure,
        );
    if count > 0 {
        let context = capture(&chain);
        let crypto = BlsCrypto::new();
        let keys = keys();
        for key in &keys {
            crypto
                .admit(
                    key.public_key(),
                    &iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
                )
                .unwrap();
        }
        let signers: Vec<_> = keys
            .iter()
            .map(|key| KeyPairSigner::new(key).unwrap())
            .collect();
        let mut parent = context.authority.genesis();
        let dir = context
            .kura
            .store_root()
            .join("lanes")
            .join(hex::encode(context.instance.0));
        std::fs::create_dir_all(&dir).unwrap();
        for height in 1..=count {
            let mut payload = PayloadBytes::from_untrusted(vec![3; 33]).unwrap();
            payload.admit(&context.budget).unwrap();
            let header = BlockHeader {
                instance: context.instance,
                epoch: context.authority.config().epoch.id,
                height,
                origin_view: 0,
                parent_hash: Hash32(parent.block_hash),
                parent_result: Hash32(parent.result),
                payload_hash: iroha_sumeragi::preimage::payload_hash(&crypto, payload.as_slice()),
                availability_digest: Hash32::ZERO,
                payload_len: 33,
                proposer: 0,
                skipped_leaders: Vec::new(),
                control_witness: ControlWitness::empty(),
                attest: false,
            };
            let authored = PayloadAuthoring::new(header, payload)
                .complete(
                    context.instance,
                    context.authority.config(),
                    &context.budget,
                    &crypto,
                    &signers[0],
                )
                .unwrap_or_else(|_| panic!("signed original lane frame"));
            let hash = authored.body.hash(&crypto);
            let mut qc = Qc {
                kind: VoteKind::Commit,
                instance: context.instance,
                epoch: context.authority.config().epoch.id,
                height,
                view: 0,
                block_hash: hash,
                result: Hash32([height as u8; 32]),
                attest: false,
                signers: Bitmap::from_indices(4, [0, 1, 2]).unwrap(),
                agg_sig: AggregateSignature([0; SIGNATURE_LEN]),
                attestations: Vec::new(),
                attestation_witness: None,
            };
            qc.agg_sig = crypto.aggregate(
                &signers[..3]
                    .iter()
                    .map(|signer| signer.sign(&qc.preimage()))
                    .collect::<Vec<_>>(),
            );
            assert!(
                iroha_sumeragi::crypto::Verifier::new(
                    &crypto,
                    &context.instance,
                    &qc.epoch,
                    &context.authority.config().committee
                )
                .verify_commit_qc(
                    &NoAttestation,
                    &qc,
                    Some(authored.body.header())
                )
            );
            parent = iroha_data_model::sumeragi_lanes::SumeragiLaneFrontier {
                height,
                block_hash: hash.0,
                result: qc.result.0,
            };
            let mut prepared = PreparedLaneWrite::new(authored.body, qc);
            std::fs::write(
                dir.join(format!("{height:020}.frame")),
                prepared.prepare(&context.budget).unwrap(),
            )
            .unwrap();
            // Only metadata from the original real-BLS certificate is exposed to global merge.
            // Its malformed lane batch is canonically merged without transaction effects.
            source.0.write().push(CommittedLaneBlock {
                block_hash: hash,
                result: Hash32(parent.result),
                batch: None,
            });
        }
        drop(context);
        chain.commit(Vec::new());
    }
    (chain, guard)
}

#[test]
fn lane_proof_genesis_parent_uses_original_creation_and_distinct_native_height() {
    let (chain, _guard) = anchored_chain(0);
    let context = capture(&chain);
    let scope = context.scope;
    let mut reader =
        LaneProofRead::new(context, 1).unwrap_or_else(|_| panic!("original active custody"));
    reader.poll().unwrap();
    assert!(reader.headers.as_slice().is_empty());
    let proof = vote_pair(&reader, 1);
    let verified = reader.verify(&proof).unwrap();
    assert_eq!(
        verified.scope(),
        iroha_data_model::block::consensus::EvidenceScope::Lane(scope)
    );
    assert_eq!(verified.height(), 1);
    assert_eq!(scope.admission_parent_height, 3);
    assert_eq!(verified.offenders()[0].signer, 2);
    assert_eq!(
        verified.offenders()[0].peer_id.public_key(),
        keys()[2].public_key()
    );
    assert!(
        verified.offenders()[0].lane_stake.is_none(),
        "fixed committee without genuine escrow is forensic-only"
    );
    assert!(
        reader.verify(&vote_pair(&reader, 2)).is_err(),
        "another native subject cannot reuse the parent"
    );
}

#[test]
fn lane_proof_replays_complete_anchored_demotion_history_and_original_pool_after_refusal() {
    let (chain, _guard) = anchored_chain(5);
    let budget = chain.state().ivm_execution_budget();
    let baseline = budget.reserved_bytes();
    let context = capture(&chain);
    assert_eq!(
        context
            .payload
            .custody_record(&context.scope.incarnation)
            .unwrap()
            .unwrap()
            .frontier()
            .height,
        5
    );
    let mut reader = LaneProofRead::new(context, 4).unwrap_or_else(|_| panic!("covered parent"));
    let pointer = std::ptr::from_ref(reader.cursor.config().epoch.as_ref());
    let held = budget.reserved_bytes();
    budget.set_limit_bytes(held);
    let error = reader.poll().unwrap_err();
    assert_eq!(error.io_kind(), io::ErrorKind::WouldBlock);
    assert!(matches!(
        error,
        crate::execution_attempt::ExecutionAttemptError::Deferred(_)
    ));
    assert_eq!(reader.cursor.next_height(), Some(5));
    assert_eq!(
        std::ptr::from_ref(reader.cursor.config().epoch.as_ref()),
        pointer
    );
    assert_eq!(budget.reserved_bytes(), held);
    budget.set_limit_bytes(1 << 30);
    reader.poll().unwrap();
    assert_eq!(
        reader
            .headers
            .as_slice()
            .iter()
            .map(|body| body.body.header().height)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(reader.parent.as_ref().unwrap().height, 3);
    assert!(reader.verify(&vote_pair(&reader, 4)).is_ok());
    assert!(
        reader
            .headers
            .as_slice()
            .iter()
            .all(|body| body.body.admitted_to(&budget))
    );
    let complete = budget.reserved_bytes();
    reader.poll().unwrap();
    assert_eq!(budget.reserved_bytes(), complete);
    drop(reader);
    assert_eq!(budget.reserved_bytes(), baseline);
}

#[test]
fn lane_proof_refuses_uncovered_parent_and_missing_original_ancestry_without_advancing() {
    let (chain, _guard) = anchored_chain(3);
    let context = capture(&chain);
    let pointer = std::ptr::from_ref(context.authority.config().epoch.as_ref());
    let (context, error) = LaneProofRead::new(context, 5)
        .err()
        .expect("uncovered parent");
    assert_eq!(error.io_kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        std::ptr::from_ref(context.authority.config().epoch.as_ref()),
        pointer
    );
    let path = context
        .kura
        .store_root()
        .join("lanes")
        .join(hex::encode(context.instance.0))
        .join("00000000000000000003.frame");
    std::fs::remove_file(path).unwrap();
    let mut reader =
        LaneProofRead::new(context, 3).unwrap_or_else(|_| panic!("covered original parent"));
    assert_eq!(
        reader.poll().unwrap_err().io_kind(),
        io::ErrorKind::NotFound
    );
    assert_eq!(reader.cursor.next_height(), Some(3));
    assert!(reader.verify(&vote_pair(&reader, 3)).is_err());
    assert!(
        reader.frame.is_some(),
        "missing original source is retained, not replaced"
    );
}

fn proposal_pair(reader: &LaneProofRead) -> Evidence {
    use iroha_sumeragi::topology::{Topology, committee_permutation, demoted_set};
    let config = reader.cursor.config();
    let height = reader.cursor.parent_height() + 1;
    let demoted = demoted_set(
        &config.committee,
        height,
        0,
        reader.cursor.configuration_owner().demotion_window(),
        reader
            .headers
            .as_slice()
            .iter()
            .map(|body| body.body.header()),
    );
    let topology = Topology::from_parts(
        committee_permutation(
            &reader.crypto,
            &reader.instance,
            &config.epoch,
            &config.committee,
        ),
        &demoted,
        height,
    )
    .unwrap();
    let proposer = topology.leader(0);
    let signer = KeyPairSigner::new(&keys()[proposer as usize]).unwrap();
    let parent = reader.parent.as_ref().unwrap();
    let proposal = |payload| {
        let header = BlockHeader {
            instance: reader.instance,
            epoch: config.epoch.id,
            height,
            origin_view: 0,
            parent_hash: parent.block_hash,
            parent_result: parent.result,
            payload_hash: Hash32([payload; 32]),
            availability_digest: Hash32([payload; 32]),
            payload_len: 1,
            proposer,
            skipped_leaders: topology.skipped_leader_keys(&config.committee, 0),
            control_witness: ControlWitness::empty(),
            attest: false,
        };
        let mut proposal = iroha_sumeragi::message::Proposal {
            instance: reader.instance,
            height,
            view: 0,
            header,
            justify: None,
            parent_qc: None,
            sig: Signature([0; SIGNATURE_LEN]),
        };
        proposal.sig = signer.sign(&proposal.signing_preimage(&reader.crypto));
        proposal
    };
    Evidence::ProposalEquivocation(Box::new(proposal(31)), Box::new(proposal(32)))
}

#[test]
fn lane_proof_requires_complete_ordered_original_demotion_custody_for_proposal_attribution() {
    let (chain, _guard) = anchored_chain(5);
    let mut reader =
        LaneProofRead::new(capture(&chain), 4).unwrap_or_else(|_| panic!("covered parent"));
    reader.poll().unwrap();
    let proof = proposal_pair(&reader);
    assert!(reader.verify(&proof).is_ok());
    reader.headers.as_mut_slice().reverse();
    assert!(matches!(
        reader.verify(&proof),
        Err(super::super::NativeEvidenceError::Proof(
            iroha_sumeragi::evidence::EvidenceError::DemotionHistory
        ))
    ));
    reader.headers.as_mut_slice().reverse();
    let last = reader.headers.pop().unwrap();
    assert!(matches!(
        reader.verify(&proof),
        Err(super::super::NativeEvidenceError::Proof(
            iroha_sumeragi::evidence::EvidenceError::DemotionHistory
        ))
    ));
    reader
        .headers
        .try_push(last)
        .unwrap_or_else(|_| panic!("original admitted descriptor slot"));
    assert!(reader.verify(&proof).is_ok());
}

#[test]
fn lane_proof_rejects_valid_same_height_certificate_on_an_unmerged_branch() {
    let (chain, _guard) = anchored_chain(3);
    let context = capture(&chain);
    let mut reader = LaneProofRead::new(context, 3).unwrap_or_else(|_| panic!("covered parent"));
    let expected = reader.cursor.next_frontier().unwrap();
    let path = reader
        .kura
        .store_root()
        .join("lanes")
        .join(hex::encode(reader.instance.0))
        .join(format!("{:020}.frame", expected.height));
    let config = reader
        .cursor
        .configuration_owner()
        .copy_config(&reader.budget)
        .unwrap();
    let source = FundedLaneSource::new(
        config,
        reader.instance,
        expected.height,
        Hash32(expected.block_hash),
        &reader.budget,
    )
    .unwrap_or_else(|_| panic!("original source"));
    let original = FundedLaneFrameRead::new(path.clone(), source, reader.budget.clone())
        .poll(&reader.budget, &reader.crypto)
        .unwrap();
    let mut payload = PayloadBytes::from_untrusted(vec![4; 33]).unwrap();
    payload.admit(&reader.budget).unwrap();
    let mut header = original.body.header().clone();
    header.payload_hash =
        iroha_sumeragi::preimage::payload_hash(&reader.crypto, payload.as_slice());
    header.availability_digest = Hash32::ZERO;
    let signers: Vec<_> = keys()
        .iter()
        .map(|key| KeyPairSigner::new(key).unwrap())
        .collect();
    let authored = PayloadAuthoring::new(header, payload)
        .complete(
            reader.instance,
            reader.cursor.config(),
            &reader.budget,
            &reader.crypto,
            &signers[0],
        )
        .unwrap_or_else(|_| panic!("other actual signed branch"));
    let mut qc = original.qc;
    qc.block_hash = authored.body.hash(&reader.crypto);
    qc.agg_sig = reader.crypto.aggregate(
        &signers[..3]
            .iter()
            .map(|signer| signer.sign(&qc.preimage()))
            .collect::<Vec<_>>(),
    );
    assert_ne!(qc.block_hash.0, expected.block_hash);
    assert!(
        iroha_sumeragi::crypto::Verifier::new(
            &reader.crypto,
            &reader.instance,
            &qc.epoch,
            &reader.cursor.config().committee
        )
        .verify_commit_qc(&NoAttestation, &qc, Some(authored.body.header()))
    );
    let mut replacement = PreparedLaneWrite::new(authored.body, qc);
    std::fs::write(path, replacement.prepare(&reader.budget).unwrap()).unwrap();
    assert_eq!(
        reader.poll().unwrap_err().io_kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(reader.cursor.next_frontier().unwrap(), expected);
    assert!(reader.verify(&vote_pair(&reader, 3)).is_err());
}

#[test]
fn retained_lane_evidence_read_preserves_original_capture_across_open_failure_and_pool_refusal() {
    use super::super::{LaneEvidenceRead, NativeEvidenceError};
    let (chain, _guard) = anchored_chain(3);
    let mut proof_context =
        LaneProofRead::new(capture(&chain), 3).unwrap_or_else(|_| panic!("original subject"));
    proof_context.poll().unwrap();
    let proof = vote_pair(&proof_context, 3);
    drop(proof_context);
    let budget = chain.state().ivm_execution_budget();
    let baseline = budget.reserved_bytes();
    let (capture, scope) = capture_parts(&chain);
    let expected_tip = chain.state().view().native_execution_tip().unwrap();
    let mut reader = LaneEvidenceRead::new(capture, scope, 3);
    let path = chain.kura().store_root().join("native-contexts");
    let held = path.with_extension("original-held");
    std::fs::rename(&path, &held).unwrap();
    let error = reader.poll(&proof).unwrap_err();
    assert!(
        matches!(error, NativeEvidenceError::Source(ref error) if error.io_kind() == io::ErrorKind::NotFound)
    );
    std::fs::rename(&held, &path).unwrap();
    budget.set_limit_bytes(baseline);
    let error = reader.poll(&proof).unwrap_err();
    assert!(
        matches!(error, NativeEvidenceError::Source(ref error) if error.io_kind() == io::ErrorKind::WouldBlock && matches!(error, crate::execution_attempt::ExecutionAttemptError::Deferred(_)))
    );
    budget.set_limit_bytes(1 << 30);
    let verified = reader.poll(&proof).unwrap();
    assert_eq!(verified.tip(), expected_tip);
    assert_eq!(
        verified.scope(),
        iroha_data_model::block::consensus::EvidenceScope::Lane(scope)
    );
    assert_eq!(verified.height(), 3);
    let attribution = verified.into_attribution();
    assert_eq!(attribution.height, 3);
    assert_eq!(
        attribution.scope,
        iroha_data_model::block::consensus::EvidenceScope::Lane(scope)
    );
    assert_eq!(attribution.offenders[0].signer, 2);
    drop(reader);
    assert_eq!(budget.reserved_bytes(), baseline);
}

#[test]
fn retained_lane_evidence_read_never_upgrades_original_cut_after_publication() {
    use super::super::LaneEvidenceRead;
    let (mut chain, _guard) = anchored_chain(3);
    let mut proof_context =
        LaneProofRead::new(capture(&chain), 3).unwrap_or_else(|_| panic!("original subject"));
    proof_context.poll().unwrap();
    let proof = vote_pair(&proof_context, 3);
    drop(proof_context);
    let (capture, scope) = capture_parts(&chain);
    let original = chain.state().view().native_execution_tip().unwrap();
    let mut reader = LaneEvidenceRead::new(capture, scope, 3);
    chain.commit(Vec::new());
    assert_ne!(
        chain.state().view().native_execution_tip().unwrap(),
        original
    );
    let verified = reader.poll(&proof).unwrap();
    assert_eq!(verified.tip(), original);
    assert_eq!(
        verified.scope(),
        iroha_data_model::block::consensus::EvidenceScope::Lane(scope)
    );
    // This verifies the old cut only. Installation is separately bound to the captured
    // State generation; a successful read cannot authorize effects in the successor.
}

#[test]
fn admission_owner_keeps_native_height_separate_and_original_history_job_on_refusal() {
    use crate::sumeragi::evidence::{EvidenceAdmissionError, admission::AdmissionRead};
    let (chain, _guard) = anchored_chain(7);
    let mut proof_context = LaneProofRead::new(capture(&chain), 7)
        .unwrap_or_else(|_| panic!("original native subject"));
    proof_context.poll().unwrap();
    let native = vote_pair(&proof_context, 7);
    drop(proof_context);
    let proof = iroha_data_model::block::consensus::Evidence::from_native(&native).unwrap();
    let state = chain.state();
    let generation = state.state_view_generation();
    let history_pool = state.ivm_execution_budget();
    let history_before = history_pool.reserved_bytes();
    let preparation_before = state.evidence_preparation_budget().reserved_bytes();
    let carrier = 5;
    let mut read = {
        let view = state.view();
        assert_eq!(view.height(), 4);
        AdmissionRead::capture(
            state,
            &view,
            generation,
            carrier,
            std::slice::from_ref(&proof),
        )
        .unwrap()
    };
    history_pool.set_limit_bytes(history_before);
    assert!(
        matches!(read.complete(), Err(EvidenceAdmissionError::Source(ref error)) if error.io_kind() == io::ErrorKind::WouldBlock && matches!(error, crate::execution_attempt::ExecutionAttemptError::Deferred(_)))
    );
    assert!(read.matches(generation, carrier, std::slice::from_ref(&proof)));
    history_pool.set_limit_bytes(1 << 30);
    read.complete().unwrap();
    let admitted = read.finish().unwrap();
    let attribution = admitted.as_slice()[0].attribution();
    assert_eq!(
        attribution.height, 7,
        "native height may exceed its root carrier height"
    );
    let iroha_data_model::block::consensus::EvidenceScope::Lane(scope) = attribution.scope else {
        panic!("original lane scope")
    };
    assert_eq!(scope.admission_parent_height + 1, carrier);
    assert!(attribution.offenders[0].lane_stake.is_none());
    drop(admitted);
    assert_eq!(history_pool.reserved_bytes(), history_before);
    assert_eq!(
        state.evidence_preparation_budget().reserved_bytes(),
        preparation_before
    );
}

#[test]
fn state_admission_cache_retains_lane_jobs_until_retry_and_refunds_original_pools() {
    use crate::sumeragi::evidence::admission::prepare_admissions;
    let _epoch = crossbeam_epoch::pin();
    let (chain, _guard) = anchored_chain(7);
    let mut reader =
        LaneProofRead::new(capture(&chain), 7).unwrap_or_else(|_| panic!("covered native subject"));
    reader.poll().unwrap();
    let proof =
        iroha_data_model::block::consensus::Evidence::from_native(&vote_pair(&reader, 7)).unwrap();
    drop(reader);
    let state = chain.state();
    let generation = state.state_view_generation();
    let carrier = state.view().native_execution_tip().unwrap().height() + 1;
    let original = state.ivm_execution_budget();
    let original_base = original.reserved_bytes();
    let admission_base = state.evidence_preparation_budget().reserved_bytes();
    original.set_limit_bytes(original_base);
    assert!(
        matches!(prepare_admissions(state, generation, carrier, std::slice::from_ref(&proof)),
        Err(crate::sumeragi::evidence::EvidenceAdmissionError::Source(ref error))
        if error.io_kind() == io::ErrorKind::WouldBlock && matches!(error, crate::execution_attempt::ExecutionAttemptError::Deferred(_)))
    );
    let waiting = state.evidence_preparation_budget().reserved_bytes();
    assert!(
        waiting > admission_base,
        "State retains the original canonical proof batch"
    );
    let empty = prepare_admissions(state, generation, carrier, &[]).unwrap();
    assert!(empty.as_slice().is_empty());
    assert_eq!(
        state.evidence_preparation_budget().reserved_bytes(),
        waiting
    );
    original.set_limit_bytes(1 << 30);
    let admitted = prepare_admissions(state, generation, carrier, &[proof]).unwrap();
    assert_eq!(admitted.as_slice()[0].attribution().height, 7);
    assert!(matches!(admitted.as_slice()[0].attribution().scope,
        iroha_data_model::block::consensus::EvidenceScope::Lane(scope)
        if scope.admission_parent_height + 1 == carrier));
    drop(admitted);
    assert_eq!(original.reserved_bytes(), original_base);
    assert_eq!(
        state.evidence_preparation_budget().reserved_bytes(),
        admission_base
    );
}

#[test]
fn native_lane_admission_and_restore_keep_original_carrier_clock_through_penalty_finality() {
    use crate::sumeragi::{evidence, test_chain::Signers};
    use iroha_data_model::{
        block::consensus::{EvidencePenaltyStatus, EvidenceScope},
        consensus::NposConsensusEffects,
    };
    let _epoch = crossbeam_epoch::pin();
    let (mut chain, _guard) = anchored_chain_with_policy(
        7,
        iroha_data_model::parameter::system::SumeragiNposParameters {
            slashing_delay_blocks: 2,
            ..Default::default()
        },
    );
    let mut reader = LaneProofRead::new(capture(&chain), 7)
        .unwrap_or_else(|_| panic!("original merged coverage"));
    reader.poll().unwrap();
    let native = vote_pair(&reader, 7);
    drop(reader);
    let proof = iroha_data_model::block::consensus::Evidence::from_native(&native).unwrap();
    let key = evidence::evidence_key(&proof);
    let parent = chain.state().view().native_execution_tip().unwrap();
    let carrier = parent.height() + 1;
    chain.commit_with_proposal(
        None,
        Vec::new(),
        Signers::Quorum,
        Default::default(),
        |proposal| {
            let parent_service_commit_qc = proposal
                .npos_consensus_effects()
                .and_then(|effects| effects.parent_service_commit_qc.clone());
            proposal.set_npos_consensus_effects(Some(NposConsensusEffects {
                parent_service_commit_qc,
                evidence_admissions: vec![proof.clone()],
                penalty_actions: Vec::new(),
            }));
        },
    );
    let original_record = chain
        .state()
        .view()
        .world()
        .consensus_evidence()
        .get(&key)
        .unwrap()
        .clone();
    assert_eq!(original_record.recorded_at_height, carrier);
    assert_eq!(original_record.attribution.height, 7);
    assert!(
        7 > carrier,
        "lane heights cannot be ordered against global carriers"
    );
    assert_eq!(
        original_record.penalty_status,
        EvidencePenaltyStatus::Pending
    );
    assert!(
        original_record
            .attribution
            .offenders
            .iter()
            .all(|offender| offender.lane_stake.is_none())
    );
    assert!(
        matches!(original_record.attribution.scope, EvidenceScope::Lane(scope)
        if scope.admission_parent_height == parent.height()
            && scope.admission_parent_hash == parent.iroha_hash())
    );
    let budget = chain.state().ivm_execution_budget();
    let base = budget.reserved_bytes();
    budget.set_limit_bytes(base);
    assert!(
        matches!(evidence::validate_persisted_records(chain.state()),
        Err(evidence::EvidenceAdmissionError::Source(ref error)) if error.io_kind() == io::ErrorKind::WouldBlock)
    );
    budget.set_limit_bytes(1 << 30);
    evidence::validate_persisted_records(chain.state()).unwrap();
    assert_eq!(
        budget.reserved_bytes(),
        base,
        "completed restore releases every original native read owner"
    );
    let delay = chain
        .state()
        .view()
        .world()
        .sumeragi_npos_parameters()
        .expect("original policy decoder completes")
        .unwrap()
        .slashing_delay_blocks();
    let due = carrier + delay;
    while (chain.state().view().height() as u64) < due {
        chain.commit(Vec::new());
    }
    let current = chain
        .state()
        .view()
        .world()
        .consensus_evidence()
        .get(&key)
        .unwrap()
        .clone();
    assert_eq!(
        current.penalty_status,
        EvidencePenaltyStatus::Applied { height: due }
    );
    assert_eq!(current.attribution, original_record.attribution);
    evidence::validate_persisted_records(chain.state()).unwrap();
    let replay = {
        let state = chain.state();
        let generation = state.state_view_generation();
        evidence::admission::prepare_admissions(state, generation, due + 1, &[proof])
    };
    assert!(
        matches!(replay, Err(evidence::EvidenceAdmissionError::Invalid(reason)) if reason == "evidence is already committed")
    );

    let mut forged = current.clone();
    let EvidenceScope::Lane(scope) = &mut forged.attribution.scope else {
        panic!("lane scope")
    };
    scope.admission_parent_core_hash[0] ^= 1;
    let mut records = chain.state().world.consensus_evidence.block();
    records.insert(key, forged);
    records.commit();
    assert!(evidence::validate_persisted_records(chain.state()).is_err());
    let mut records = chain.state().world.consensus_evidence.block();
    records.insert(key, current);
    records.commit();
    evidence::validate_persisted_records(chain.state()).unwrap();
}

#[test]
fn native_lane_observer_proposes_only_original_authenticated_reports_after_local_retry() {
    use crate::sumeragi::{evidence, lanes::runner::evidence_observer};
    use iroha_data_model::{
        block::consensus::EvidencePenaltyStatus, parameter::system::SumeragiNposParameters,
    };
    let (mut chain, _guard) = anchored_chain_with_policy(
        7,
        SumeragiNposParameters {
            slashing_delay_blocks: 2,
            ..SumeragiNposParameters::default()
        },
    );
    let mut reader = LaneProofRead::new(capture(&chain), 7)
        .unwrap_or_else(|_| panic!("original merged coverage"));
    reader.poll().unwrap();
    let native = vote_pair(&reader, 7);
    let scope = reader.scope;
    drop(reader);
    let state = Arc::clone(chain.state());
    let proof = iroha_data_model::block::consensus::Evidence::from_native(&native).unwrap();
    let key = evidence::evidence_key(&proof);
    let budget = state.evidence_preparation_budget();
    let initial = budget.reserved_bytes();
    let foreign = evidence_observer(Arc::clone(&state), scope.lane, [0xEE; 32]);
    foreign.evidence(&native);
    assert_eq!(
        budget.reserved_bytes(),
        initial,
        "foreign incarnations never enter the local source pool"
    );
    let observer = evidence_observer(Arc::clone(&state), scope.lane, scope.incarnation);
    // An observer callback is not authority: even a malformed callback stays untrusted until
    // the shared original-history verifier rejects it during optional proposal selection.
    let mut forged = native.clone();
    let Evidence::VoteEquivocation(first, _) = &mut forged else {
        panic!("vote fixture")
    };
    first.sig.0[0] ^= 1;
    observer.evidence(&forged);
    observer.evidence(&native);
    let retained = budget.reserved_bytes();
    assert!(retained > initial);
    assert!(
        state
            .view()
            .world()
            .consensus_evidence()
            .iter()
            .next()
            .is_none()
    );
    let carrier = state.view().height() as u64 + 1;
    assert!(
        7 > carrier,
        "local lane height must not exclude a global proposal"
    );
    let generation = state.state_view_generation();
    let original_pool = state.ivm_execution_budget();
    let original_base = original_pool.reserved_bytes();
    original_pool.set_limit_bytes(original_base);
    assert!(evidence::pending_evidence_admissions(&state, carrier, generation).is_empty());
    let waiting = budget.reserved_bytes();
    assert!(
        waiting > retained,
        "the refused original reader remains owned by State"
    );
    assert!(evidence::pending_evidence_admissions(&state, carrier, generation).is_empty());
    assert_eq!(
        budget.reserved_bytes(),
        waiting,
        "retry does not accumulate replacement source jobs"
    );
    original_pool.set_limit_bytes(1 << 30);
    assert_eq!(
        evidence::pending_evidence_admissions(&state, carrier, generation),
        [proof.clone()]
    );
    assert_eq!(original_pool.reserved_bytes(), original_base);
    assert_eq!(budget.reserved_bytes(), retained);
    chain.commit(Vec::new());
    let view = state.view();
    let record = view.world().consensus_evidence().get(&key).unwrap();
    assert_eq!(record.evidence, proof);
    assert_eq!(record.recorded_at_height, carrier);
    assert_eq!(record.penalty_status, EvidencePenaltyStatus::Pending);
    assert_eq!(
        view.world().consensus_evidence().iter().count(),
        1,
        "forged observations never gain World authority"
    );
    drop(view);
    evidence::validate_persisted_records(&state).unwrap();
}

fn fund_original_validator_in_signed_genesis(
    config: &mut crate::sumeragi::test_chain::TestChainConfig,
) {
    use crate::state::World;
    use iroha_data_model::{Registrable, domain::Domain};
    use iroha_data_model::{
        account::{Account, AccountId},
        asset::{Asset, AssetBalancePolicy, AssetDefinition, AssetDefinitionId, AssetId},
        isi::RegisterPublicLaneValidator,
        nexus::PublicLaneMonetaryPlanV1,
    };
    use iroha_model_base::domain::DomainId;
    use iroha_primitives::numeric::Quantity;
    let original_keys = keys();
    let peer = iroha_model_base::peer::PeerId::new(original_keys[2].public_key().clone());
    config.validator_keys = Some(original_keys);
    let owner = AccountId::new(config.genesis_key.public_key().clone());
    let validator = AccountId::new(peer.public_key().clone());
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    // Both static lanes are configured in the same physical dataspace. Its canonical
    // lowest stake-elected owner is lane 0; merely sharing a BLS key is insufficient.
    nexus.lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
        std::num::NonZeroU32::new(8).unwrap(),
        vec![
            iroha_data_model::nexus::LaneConfig::default(),
            iroha_data_model::nexus::LaneConfig {
                id: LaneId::new(7),
                alias: "original-custody-lane".to_owned(),
                ..iroha_data_model::nexus::LaneConfig::default()
            },
        ],
    )
    .unwrap();
    nexus.configured_lane_catalog = nexus.lane_catalog.clone();
    nexus.staking.max_slash_bps = 1_000;
    let staking = &nexus.staking;
    let definition: AssetDefinitionId = staking.stake_asset_id.parse().unwrap();
    let escrow = AccountId::parse_encoded(&staking.stake_escrow_account_id).unwrap();
    let sink = AccountId::parse_encoded(&staking.slash_sink_account_id).unwrap();
    let source_asset = AssetId::new(definition.clone(), validator.clone());
    let escrow_asset = AssetId::new(definition.clone(), escrow.clone());
    let amount = Quantity::from(10_000_u64);
    config.world = World::with_assets(
        [Domain::new(DomainId::try_new("nexus", "universal").unwrap()).build(&owner)],
        [
            Account::new(validator.clone()).build(&owner),
            Account::new(escrow).build(&owner),
            Account::new(sink).build(&owner),
        ],
        [
            AssetDefinition::numeric(definition, "Staked XOR", AssetBalancePolicy::Global, None)
                .build(&owner),
        ],
        [Asset::new(source_asset.clone(), amount.clone())],
        [],
    );
    config.nexus = Some(nexus);
    config.genesis_instructions.push(
        RegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator.clone(),
            peer_id: peer,
            stake_account: validator,
            initial_stake: amount.clone(),
            metadata: Default::default(),
            monetary_plan: PublicLaneMonetaryPlanV1::genesis_registration(
                source_asset,
                escrow_asset,
                amount,
            ),
        }
        .into(),
    );
}

#[test]
fn native_lane_original_genesis_escrow_is_debited_only_by_delayed_authenticated_admission() {
    use crate::sumeragi::{evidence, lanes::runner::evidence_observer};
    use iroha_data_model::{
        account::AccountId, block::consensus::EvidencePenaltyStatus,
        parameter::system::SumeragiNposParameters,
    };
    use iroha_primitives::numeric::Quantity;
    let (mut chain, _guard) = anchored_chain_with_config(
        7,
        SumeragiNposParameters {
            slashing_delay_blocks: 2,
            ..SumeragiNposParameters::default()
        },
        fund_original_validator_in_signed_genesis,
    );
    assert!(
        chain
            .genesis()
            .output_results()
            .all(|result| result.as_ref().is_ok()),
        "signed genesis executed original registration and escrow transfer"
    );
    let state = Arc::clone(chain.state());
    let validator = AccountId::new(keys()[2].public_key().clone());
    let stake_key = (LaneId::SINGLE, validator.clone());
    let initial = state
        .view()
        .world()
        .public_lane_stake_custody()
        .get(&stake_key)
        .unwrap()
        .1
        .clone();
    assert_eq!(initial, Quantity::from(10_000_u64));
    let mut reader =
        LaneProofRead::new(capture(&chain), 7).unwrap_or_else(|_| panic!("original merged branch"));
    reader.poll().unwrap();
    let native = vote_pair(&reader, 7);
    let original = reader.verify(&native).unwrap();
    assert_eq!(original.offenders().len(), 1);
    let binding = original.offenders()[0]
        .lane_stake
        .expect("original creation authenticated genuine positive escrow");
    assert!(binding.names_account(LaneId::SINGLE, &validator).unwrap());
    let scope = reader.scope;
    drop(reader);
    let proof = iroha_data_model::block::consensus::Evidence::from_native(&native).unwrap();
    let key = evidence::evidence_key(&proof);
    evidence_observer(Arc::clone(&state), scope.lane, scope.incarnation).evidence(&native);
    let carrier = state.view().height() as u64 + 1;
    chain.commit(Vec::new());
    assert_eq!(
        state
            .view()
            .world()
            .consensus_evidence()
            .get(&key)
            .unwrap()
            .penalty_status,
        EvidencePenaltyStatus::Pending
    );
    assert_eq!(
        state
            .view()
            .world()
            .public_lane_stake_custody()
            .get(&stake_key)
            .unwrap()
            .1,
        initial,
        "the admission carrier cannot debit its own report"
    );
    chain.commit(Vec::new());
    assert_eq!(
        state
            .view()
            .world()
            .public_lane_stake_custody()
            .get(&stake_key)
            .unwrap()
            .1,
        initial,
        "the complete immutable delay is preserved"
    );
    chain.commit(Vec::new());
    let view = state.view();
    let after = &view
        .world()
        .public_lane_stake_custody()
        .get(&stake_key)
        .unwrap()
        .1;
    assert_eq!(
        initial.checked_sub(after).unwrap(),
        Quantity::from(1_000_u64)
    );
    let record = view.world().consensus_evidence().get(&key).unwrap();
    assert_eq!(
        record.penalty_status,
        EvidencePenaltyStatus::Applied {
            height: carrier + 2
        }
    );
    assert_eq!(record.attribution.offenders[0].lane_stake, Some(binding));
    let expected_record = record.clone();
    let expected_custody = after.clone();
    drop(view);
    evidence::validate_persisted_records(&state).unwrap();

    // A new executor/State starts from the same original signed genesis and native
    // frontier, then independently replays the exact admitted and penalized suffix.
    let (mut replay, _replay_guard) = anchored_chain_with_config(
        7,
        SumeragiNposParameters {
            slashing_delay_blocks: 2,
            ..SumeragiNposParameters::default()
        },
        fund_original_validator_in_signed_genesis,
    );
    assert_eq!(replay.genesis().hash(), chain.genesis().hash());
    assert_eq!(replay.state().view().height() as u64, carrier - 1);
    replay.replay_from(&chain).unwrap();
    evidence::validate_persisted_records(replay.state()).unwrap();
    for _ in 0..2 {
        let view = replay.state().view();
        assert_eq!(
            view.world().consensus_evidence().get(&key),
            Some(&expected_record)
        );
        assert_eq!(
            view.world()
                .public_lane_stake_custody()
                .get(&stake_key)
                .unwrap()
                .1,
            expected_custody
        );
        assert_eq!(view.height() as u64, carrier + 2);
        drop(view);
        replay.replay_from(&chain).unwrap();
    }
}

#[test]
fn lane_admission_capture_checks_inclusive_deadline_and_missing_original_incarnation() {
    use crate::sumeragi::evidence::EvidenceAdmissionError;
    use crate::sumeragi::evidence::admission::AdmissionRead;
    use iroha_data_model::parameter::system::SumeragiNposParameters;
    let (chain, _guard) = anchored_chain_with_policy(
        7,
        SumeragiNposParameters {
            evidence_horizon_blocks: 2,
            slashing_delay_blocks: 1,
            ..SumeragiNposParameters::default()
        },
    );
    let mut native_read = LaneProofRead::new(capture(&chain), 3)
        .unwrap_or_else(|_| panic!("original native coverage"));
    native_read.poll().unwrap();
    let proof =
        iroha_data_model::block::consensus::Evidence::from_native(&vote_pair(&native_read, 3))
            .unwrap();
    let instance = native_read.instance.0;
    drop(native_read);
    let state = chain.state();
    let generation = state.state_view_generation();
    let budget = state.evidence_preparation_budget();
    let baseline = budget.reserved_bytes();
    let original = state.view().world().sumeragi_lanes().clone();
    // These explicit preflight inputs are not authenticated retirement carriers. A
    // successful capture below is not admission: the retained history read must still
    // authenticate retirement from the original carrier before it can finish.
    let replace_retirement = |retired_at| {
        let mut rows = state.world.sumeragi_lanes.block();
        rows.get_mut()
            .custody
            .iter_mut()
            .find(|row| row.instance == instance)
            .unwrap()
            .retired_at = retired_at;
        rows.commit();
    };
    replace_retirement(Some(3));
    let captured = AdmissionRead::capture(
        state,
        &state.view(),
        generation,
        5,
        std::slice::from_ref(&proof),
    )
    .unwrap();
    drop(captured);
    assert_eq!(budget.reserved_bytes(), baseline);
    replace_retirement(Some(2));
    assert!(
        matches!(AdmissionRead::capture(state, &state.view(), generation, 5,
        std::slice::from_ref(&proof)), Err(EvidenceAdmissionError::Invalid(reason))
        if reason.contains("outside original custody admission"))
    );
    assert_eq!(budget.reserved_bytes(), baseline);
    for recreate in [false, true] {
        let mut rows = state.world.sumeragi_lanes.block();
        *rows.get_mut() = original.clone();
        if recreate {
            let row = rows
                .get_mut()
                .custody
                .iter_mut()
                .find(|row| row.instance == instance)
                .unwrap();
            row.incarnation = [0x99; 32];
            row.instance = [0x98; 32];
        } else {
            rows.get_mut()
                .custody
                .retain(|row| row.instance != instance);
        }
        rows.commit();
        assert!(
            AdmissionRead::capture(
                state,
                &state.view(),
                generation,
                5,
                std::slice::from_ref(&proof)
            )
            .is_err(),
            "a reclaimed or recreated row cannot reopen old proof admission"
        );
        assert_eq!(budget.reserved_bytes(), baseline);
    }
}

#[test]
fn terminal_lane_source_failure_cannot_pin_competing_original_admission_forever() {
    use crate::sumeragi::{
        evidence::{EvidenceAdmissionError, admission::prepare_admissions},
        test_chain::Signers,
    };
    let (chain, _guard) = anchored_chain(7);
    let mut reader =
        LaneProofRead::new(capture(&chain), 3).unwrap_or_else(|_| panic!("covered native branch"));
    reader.poll().unwrap();
    let proof =
        iroha_data_model::block::consensus::Evidence::from_native(&vote_pair(&reader, 3)).unwrap();
    let path = reader
        .kura
        .store_root()
        .join("lanes")
        .join(hex::encode(reader.instance.0))
        .join("00000000000000000007.frame");
    drop(reader);
    let root = Evidence::ConflictingCertificates(
        chain.commit_qc(
            3,
            Hash32([0x31; 32]),
            Hash32([0x32; 32]),
            false,
            Signers::Quorum,
        ),
        chain.commit_qc(
            3,
            Hash32([0x33; 32]),
            Hash32([0x34; 32]),
            false,
            Signers::LastThree,
        ),
    );
    let root = iroha_data_model::block::consensus::Evidence::from_native(&root).unwrap();
    std::fs::remove_file(path).unwrap();
    let state = chain.state();
    let generation = state.state_view_generation();
    let baseline = state.ivm_execution_budget().reserved_bytes();
    let preparation = state.evidence_preparation_budget().reserved_bytes();
    assert!(matches!(prepare_admissions(state, generation, 5, &[proof]),
        Err(EvidenceAdmissionError::Source(error)) if error.io_kind() == io::ErrorKind::NotFound));
    let admitted = prepare_admissions(state, generation, 5, &[root])
        .expect("terminal lane storage failure cannot retain the same-cut admission slot");
    assert_eq!(admitted.as_slice().len(), 1);
    assert_eq!(
        admitted.as_slice()[0].attribution().scope,
        iroha_data_model::block::consensus::EvidenceScope::Root
    );
    drop(admitted);
    assert_eq!(state.ivm_execution_budget().reserved_bytes(), baseline);
    assert_eq!(
        state.evidence_preparation_budget().reserved_bytes(),
        preparation
    );
    assert_eq!(state.view().world().consensus_evidence().iter().count(), 0);
}

#[test]
fn local_proposer_skips_terminal_lane_source_without_deleting_observation() {
    use crate::sumeragi::{evidence, test_chain::Signers};
    let (chain, _guard) = anchored_chain(7);
    let mut reader = LaneProofRead::new(capture(&chain), 3)
        .unwrap_or_else(|_| panic!("original native coverage"));
    reader.poll().unwrap();
    let native = vote_pair(&reader, 3);
    let scope = reader.scope;
    let lane = iroha_data_model::block::consensus::Evidence::from_native(&native).unwrap();
    let path = reader
        .kura
        .store_root()
        .join("lanes")
        .join(hex::encode(reader.instance.0))
        .join("00000000000000000007.frame");
    drop(reader);
    let (root_native, root) = (1..=255_u8)
        .find_map(|marker| {
            let proof = Evidence::ConflictingCertificates(
                chain.commit_qc(
                    3,
                    Hash32([marker; 32]),
                    Hash32([0x32; 32]),
                    false,
                    Signers::Quorum,
                ),
                chain.commit_qc(
                    3,
                    Hash32([0; 32]),
                    Hash32([0x34; 32]),
                    false,
                    Signers::LastThree,
                ),
            );
            let wire = iroha_data_model::block::consensus::Evidence::from_native(&proof).unwrap();
            (evidence::evidence_key(&wire) > evidence::evidence_key(&lane)).then_some((proof, wire))
        })
        .expect("bounded deterministic fixture selects a later canonical root key");
    let state = chain.state();
    assert!(evidence::observe_lane(state, scope.lane, scope.incarnation, &native).unwrap());
    assert!(evidence::observe(state, &root_native).unwrap());
    std::fs::remove_file(path).unwrap();
    let preparation = state.evidence_preparation_budget().reserved_bytes();
    let original_pool = state.ivm_execution_budget().reserved_bytes();
    assert_eq!(
        evidence::pending_evidence_admissions(state, 5, state.state_view_generation()),
        vec![root]
    );
    assert!(
        !evidence::observe_lane(state, scope.lane, scope.incarnation, &native).unwrap(),
        "storage failure cannot delete or blame the original signed local observation"
    );
    assert_eq!(
        state.evidence_preparation_budget().reserved_bytes(),
        preparation
    );
    assert_eq!(state.ivm_execution_budget().reserved_bytes(), original_pool);
    assert_eq!(state.view().world().consensus_evidence().iter().count(), 0);
}

#[test]
fn original_lane_context_retains_custody_on_native_constructor_decode_refusal() {
    let (chain, _guard) = anchored_chain(3);
    let budget = chain.state().ivm_execution_budget();
    let baseline = budget.reserved_bytes();
    let context = capture(&chain);
    let retained = budget.reserved_bytes();
    let expected_scope = context.scope;
    let expected_tip = context.original_tip;
    let (context, error) = norito::core::with_decode_limits_scope(
        norito::core::DecodeLimits::new(1024, 1, 4096, 0, 32),
        || {
            LaneProofRead::new(context, 3)
                .err()
                .expect("field ceiling refuses original custody inspection")
        },
    );
    assert_eq!(error.io_kind(), io::ErrorKind::WouldBlock);
    assert!(matches!(
        error,
        crate::execution_attempt::ExecutionAttemptError::Deferred(_)
    ));
    assert_eq!(context.scope, expected_scope);
    assert_eq!(context.original_tip, expected_tip);
    assert!(context.budget.same_pool(&budget));
    assert_eq!(budget.reserved_bytes(), retained);
    let mut reader =
        LaneProofRead::new(context, 3).unwrap_or_else(|(_, error)| panic!("retry: {error}"));
    reader.poll().unwrap();
    assert!(reader.verify(&vote_pair(&reader, 3)).is_ok());
    drop(reader);
    assert_eq!(budget.reserved_bytes(), baseline);
}

#[test]
fn original_lane_verification_defers_typed_custody_decode_refusal() {
    let (chain, _guard) = anchored_chain(3);
    let budget = chain.state().ivm_execution_budget();
    let baseline = budget.reserved_bytes();
    let mut reader =
        LaneProofRead::new(capture(&chain), 3).unwrap_or_else(|_| panic!("original custody"));
    reader.poll().unwrap();
    let proof = vote_pair(&reader, 3);
    let retained = budget.reserved_bytes();
    norito::core::with_decode_limits_scope(
        norito::core::DecodeLimits::new(1024, 1, 4096, 0, 32),
        || {
            assert!(
                matches!(reader.verify(&proof), Err(super::super::NativeEvidenceError::Source(error))
            if error.io_kind() == io::ErrorKind::WouldBlock && matches!(error, crate::execution_attempt::ExecutionAttemptError::Deferred(_)))
            )
        },
    );
    assert_eq!(budget.reserved_bytes(), retained);
    assert!(reader.verify(&proof).is_ok());
    drop(reader);
    assert_eq!(budget.reserved_bytes(), baseline);
}

#[test]
fn original_lane_observer_late_policy_refusal_retains_observation_and_retries() {
    use crate::execution_attempt::ExecutionAttemptError;
    use crate::sumeragi::evidence;
    let (chain, _guard) = anchored_chain(7);
    let mut reader = LaneProofRead::new(capture(&chain), 3)
        .unwrap_or_else(|_| panic!("original native coverage"));
    reader.poll().unwrap();
    let native = vote_pair(&reader, 3);
    let scope = reader.scope;
    drop(reader);
    let state = chain.state();
    assert!(evidence::observe_lane(state, scope.lane, scope.incarnation, &native).unwrap());
    let original = iroha_data_model::block::consensus::Evidence::from_native(&native).unwrap();
    let reserved = state.evidence_preparation_budget().reserved_bytes();
    let ceiling = 1024 * 1024;
    let limits =
        |allocation| norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, allocation, 64);
    let policy_bytes = norito::with_decode_limits_scope(limits(ceiling), || {
        assert!(
            state
                .view()
                .world()
                .sumeragi_npos_parameters()
                .unwrap()
                .is_some()
        );
        let norito::Error::TotalAllocationExceeded { attempted, limit } =
            norito::core::reserve_decode_allocation(ceiling + 1).unwrap_err()
        else {
            panic!("non-charging allocation probe");
        };
        assert_eq!(limit, u64::try_from(ceiling).unwrap());
        usize::try_from(attempted).unwrap() - ceiling - 1
    });
    assert!(policy_bytes > 0);
    norito::with_decode_limits_scope(limits(policy_bytes), || {
        assert!(
            state
                .view()
                .world()
                .sumeragi_npos_parameters()
                .unwrap()
                .is_some()
        );
    });
    // Scope admission consumes exactly one original policy read. The existing
    // retained collection requires a second read before pruning any entry.
    let error = norito::with_decode_limits_scope(limits(policy_bytes), || {
        evidence::observe_lane(state, scope.lane, scope.incarnation, &native)
    })
    .unwrap_err();
    assert!(
        matches!(
            error,
            evidence::EvidenceAdmissionError::Policy(ExecutionAttemptError::Deferred(_))
        ),
        "{error:?}"
    );
    assert_eq!(
        state.evidence_preparation_budget().reserved_bytes(),
        reserved
    );
    assert!(!evidence::observe_lane(state, scope.lane, scope.incarnation, &native).unwrap());
    assert_eq!(
        state.evidence_preparation_budget().reserved_bytes(),
        reserved
    );
    assert_eq!(
        iroha_data_model::block::consensus::Evidence::from_native(&native).unwrap(),
        original
    );
    let carrier = state.view().height() as u64 + 1;
    assert_eq!(
        evidence::pending_evidence_admissions(state, carrier, state.state_view_generation()),
        [original]
    );
}
