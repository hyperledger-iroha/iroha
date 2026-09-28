//! Current-format cryptographic proof and wire regressions, independent of retired V2 fixtures.
use super::*;
use crate::{
    account::AccountId,
    block::{CommitCertificate, builder::BlockBuilder, output_test_support},
    isi::Log,
    level::Level,
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use iroha_crypto::{KeyPair, bls_normal_pop_prove};
use iroha_sumeragi::types::{Bitmap, ChainParams, HeightConfig};
use std::{collections::BTreeSet, num::NonZeroU64};

pub(super) struct Fixture {
    pub(super) genesis: SignedBlock,
    pub(super) first: SumeragiFinalityProof,
    pub(super) second: SumeragiFinalityProof,
    pub(super) keys: Vec<KeyPair>,
    pub(super) validators: Vec<FinalityValidator>,
    pub(super) network: NetworkId,
}

pub(super) fn result(block: &SignedBlock, committee: Committee) -> ExecutionResultCommitment {
    let (len, hash) = block.executed_block_wire_identity().unwrap();
    ExecutionResultCommitment::new(
        ExecutionCommitment {
            parent_state_root: Hash::new(b"parent"),
            post_state_root: Hash::new(b"post"),
            ordinary_writes_root: Hash::new(b"ordinary"),
            kagemusha_top_up_root: None,
            kagemusha_top_up_count: 0,
            executed_block_wire_len: len,
            executed_block_wire_hash: hash,
            transaction_input_commitment: block.network_input_merkle_commitment(),
            transaction_output_commitment: block.output_merkle_commitment(),
        },
        &HeightConfig {
            committee,
            params: ChainParams::default(),
        },
    )
}

pub(super) fn sign_qc(qc: &mut Qc, keys: &[KeyPair], chosen: &[u32]) {
    qc.signers = Bitmap::from_indices(keys.len(), chosen.iter().copied()).unwrap();
    let signatures: Vec<_> = chosen
        .iter()
        .map(|index| {
            iroha_crypto::Signature::try_new(keys[*index as usize].private_key(), &qc.preimage())
                .unwrap()
        })
        .collect();
    let bytes: Vec<_> = signatures
        .iter()
        .map(|signature| signature.payload())
        .collect();
    qc.agg_sig = AggregateSignature(
        bls_normal_aggregate_signatures(&bytes)
            .unwrap()
            .try_into()
            .unwrap(),
    );
}

impl Fixture {
    pub(super) fn new() -> Self {
        let mut keys: Vec<_> = (1..=4)
            .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
            .collect();
        keys.sort_by_key(|key| key.public_key().try_to_bytes().unwrap().1.to_vec());
        let validators: Vec<_> = keys
            .iter()
            .map(|key| FinalityValidator {
                public_key: key.public_key().clone(),
                proof_of_possession: bls_normal_pop_prove(key.private_key()).unwrap(),
            })
            .collect();
        let (crypto, committee) = ProofCrypto::new(&validators).unwrap();
        let authority = KeyPair::from_seed(vec![41; 32], Algorithm::Ed25519);
        let account = AccountId::new(authority.public_key().clone());
        let tx = TransactionBuilder::new_genesis(
            account.clone(),
            FeePaymentIntent::authority(vec![], None),
        )
        .with_instructions([Log::new(Level::INFO, "signed genesis".into())])
        .sign(authority.private_key());
        let genesis =
            SignedBlock::try_genesis(vec![tx], authority.private_key(), None, None).unwrap();
        let network = NetworkId::from_genesis_hash(genesis.hash());
        let instance = instance_id(
            &crypto,
            &Hash32(Hash::from(genesis.hash()).into()),
            b"portable-finality-test",
            InstanceKind::Global,
            0,
        );
        let mut first_block = genesis.clone();
        output_test_support::install_network(&mut first_block, vec![Ok(Default::default())])
            .unwrap();
        let first_result = result(&first_block, committee.clone());
        first_block.set_commit_certificate(Some(CommitCertificate::new(
            vec![],
            vec![],
            first_result.preimage().unwrap(),
        )));
        let first = SumeragiFinalityProof {
            block_header: first_block.header(),
            block_wire: first_block.encode_wire().unwrap(),
            committee: validators.clone(),
        };
        let tx =
            TransactionBuilder::new(network, account, FeePaymentIntent::authority(vec![], None))
                .with_instructions([Log::new(Level::INFO, "real submitted work".into())])
                .sign(authority.private_key());
        let mut builder = BlockBuilder::new(BlockHeader::new(
            NonZeroU64::new(2).unwrap(),
            Some(genesis.hash()),
            None,
            genesis.header().creation_time_ms.saturating_add(1),
            0,
        ));
        builder.push_transaction(tx);
        let mut block = builder.build(BTreeSet::new());
        output_test_support::install_network(&mut block, vec![Ok(Default::default())]).unwrap();
        let result = result(&block, committee);
        let payload = block.canonical_resultless_proposal().encode_wire().unwrap();
        let header = CoreHeader {
            instance,
            height: 2,
            origin_view: 0,
            parent_hash: Hash32(Hash::from(genesis.hash()).into()),
            parent_result: first_result.result().unwrap(),
            payload_hash: payload_hash(&crypto, &payload),
            payload_len: payload.len().try_into().unwrap(),
            proposer: 0,
            skipped_leaders: vec![],
            attest: false,
        };
        let mut qc = Qc {
            kind: VoteKind::Commit,
            instance,
            height: 2,
            view: 0,
            block_hash: header.hash(&crypto),
            result: result.result().unwrap(),
            attest: false,
            signers: Bitmap::new(4),
            agg_sig: AggregateSignature([0; 96]),
            attestations: vec![],
        };
        sign_qc(&mut qc, &keys, &[0, 1, 2]);
        block.set_commit_certificate(Some(CommitCertificate::new(
            norito::encode_canonical(&header).unwrap(),
            norito::encode_canonical(&qc).unwrap(),
            result.preimage().unwrap(),
        )));
        let second = SumeragiFinalityProof {
            block_header: block.header(),
            block_wire: block.encode_wire().unwrap(),
            committee: validators.clone(),
        };
        Self {
            genesis,
            first,
            second,
            keys,
            validators,
            network,
        }
    }
    pub(super) fn verifier(&self) -> SumeragiFinalityVerifier {
        SumeragiFinalityVerifier::new(
            &self.genesis,
            "portable-finality-test",
            self.validators.clone(),
        )
        .unwrap()
    }
    pub(super) fn alternate(&self) -> SumeragiFinalityProof {
        let mut proof = self.second.clone();
        let mut block = decode_versioned_signed_block(&proof.block_wire).unwrap();
        let mut certificate = block.commit_certificate().unwrap().clone();
        let mut qc: Qc = norito::decode_canonical(&certificate.commit_qc).unwrap();
        qc.view = 1;
        sign_qc(&mut qc, &self.keys, &[1, 2, 3]);
        certificate.commit_qc = norito::encode_canonical(&qc).unwrap();
        block.set_commit_certificate(Some(certificate));
        proof.block_wire = block.encode_wire().unwrap();
        proof
    }
}

#[test]
fn current_proofs_roundtrip_and_verify_successful_exact_execution() {
    let fixture = Fixture::new();
    for proof in [&fixture.first, &fixture.second] {
        let encoded = norito::encode_canonical(proof).unwrap();
        assert_eq!(
            norito::decode_canonical::<SumeragiFinalityProof>(&encoded).unwrap(),
            *proof
        );
        let json = norito::json::to_vec(proof).unwrap();
        assert_eq!(
            norito::json::from_slice::<SumeragiFinalityProof>(&json).unwrap(),
            *proof
        );
    }
    let mut verifier = fixture.verifier();
    assert!(verifier.verify(&fixture.second).is_err(), "no gaps");
    verifier.verify(&fixture.first).unwrap();
    let verified = verifier.verify(&fixture.second).unwrap();
    let committed = output_test_support::committed(verified.block(), 0);
    verified
        .verify_committed_transaction(&fixture.network, &committed)
        .unwrap();
    let foreign =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"foreign")));
    assert!(
        verified
            .verify_committed_transaction(&foreign, &committed)
            .is_err()
    );
    assert!(
        decode_versioned_signed_block(&verified.canonical_executed_wire().unwrap())
            .unwrap()
            .commit_certificate()
            .is_none()
    );
}

#[test]
fn alternate_current_quorum_witnesses_have_one_authenticated_execution() {
    let fixture = Fixture::new();
    let alternate = fixture.alternate();
    let mut verifier = fixture.verifier();
    assert!(
        verifier
            .verify_same_decision(&fixture.second, &fixture.second)
            .is_err()
    );
    verifier.verify(&fixture.first).unwrap();
    let first = verifier.verify(&fixture.second).unwrap();
    let second = verifier
        .verify_same_decision(&fixture.second, &alternate)
        .unwrap();
    assert_ne!(fixture.second.block_wire, alternate.block_wire);
    assert_eq!(
        first.canonical_executed_wire().unwrap(),
        second.canonical_executed_wire().unwrap()
    );
    assert_eq!(first.result(), second.result());
}

#[test]
fn current_proof_rejects_tampered_qc_result_committee_parent_and_wire() {
    let fixture = Fixture::new();
    for mutation in 0..6 {
        let mut bad = fixture.second.clone();
        let mut block = decode_versioned_signed_block(&bad.block_wire).unwrap();
        let mut certificate = block.commit_certificate().unwrap().clone();
        let mut qc: Qc = norito::decode_canonical(&certificate.commit_qc).unwrap();
        match mutation {
            0 => qc.agg_sig.0[0] ^= 1,
            1 => qc.result = Hash32([9; 32]),
            2 => bad.committee[0].proof_of_possession[0] ^= 1,
            3 => {
                let mut header: CoreHeader =
                    norito::decode_canonical(&certificate.consensus_header).unwrap();
                header.parent_result = Hash32([9; 32]);
                let (crypto, _) = ProofCrypto::new(&fixture.validators).unwrap();
                qc.block_hash = header.hash(&crypto);
                sign_qc(&mut qc, &fixture.keys, &[0, 1, 2]);
                certificate.consensus_header = norito::encode_canonical(&header).unwrap();
            }
            4 => sign_qc(&mut qc, &fixture.keys, &[0, 1]),
            _ => certificate.result_preimage.push(0),
        }
        certificate.commit_qc = norito::encode_canonical(&qc).unwrap();
        block.set_commit_certificate(Some(certificate));
        bad.block_wire = block.encode_wire().unwrap();
        let mut verifier = fixture.verifier();
        verifier.verify(&fixture.first).unwrap();
        assert!(verifier.verify(&bad).is_err(), "mutation {mutation}");
    }
    let mut trailing = fixture.second.clone();
    trailing.block_wire.push(0);
    assert!(trailing.decode_checked().is_err());
}

#[test]
fn current_attestation_roundtrip_binds_challenge_node_status_and_runtime_identity() {
    let fixture = Fixture::new();
    let node_id = PeerId::new(fixture.keys[0].public_key().clone());
    let body = SumeragiFinalityAttestationBody {
        challenge: [7; 32],
        network_id: fixture.network,
        node_fingerprint: Hash::new(node_id.encode()),
        node_id,
        build_fingerprint: Hash::new(b"compiled build"),
        config_fingerprint: Hash::new(b"effective config"),
        genesis_block_hash: fixture.genesis.hash(),
        genesis_finality_proof: fixture.first.clone(),
        status: SumeragiStatus {
            instance: fixture.verifier().instance().0,
            height: 3,
            view: 0,
            stage: 0,
            leader: None,
            proxy_tail: None,
            high_qc_view: None,
            level: 0,
            start_level: 0,
            t_retx_ms: 100,
            committed_height: 2,
            applied_height: 2,
            awaiting: false,
            signer: Some(fixture.keys[0].public_key().clone()),
            unanchored: false,
            abstaining: false,
            halted: None,
            footprint: Default::default(),
        },
        finality_proof: fixture.second,
    };
    let attestation = SumeragiFinalityAttestation {
        signature: SignatureOf::try_from_hash(fixture.keys[0].private_key(), body.signing_hash())
            .unwrap(),
        body,
    };
    attestation.verify().unwrap();
    let wire = norito::encode_canonical(&attestation).unwrap();
    assert_eq!(
        norito::decode_canonical::<SumeragiFinalityAttestation>(&wire).unwrap(),
        attestation
    );
    let json = norito::json::to_vec(&attestation).unwrap();
    assert_eq!(
        norito::json::from_slice::<SumeragiFinalityAttestation>(&json).unwrap(),
        attestation
    );
    for mutation in 0..5 {
        let mut bad = attestation.clone();
        match mutation {
            0 => bad.body.challenge = [0; 32],
            1 => bad.body.status.committed_height = 3,
            2 => bad.body.node_id = PeerId::new(fixture.keys[1].public_key().clone()),
            3 => bad.body.build_fingerprint = Hash::new(b"different binary"),
            _ => bad.body.config_fingerprint = Hash::new(b"different config"),
        }
        assert!(bad.verify().is_err(), "mutation {mutation}");
    }
}
