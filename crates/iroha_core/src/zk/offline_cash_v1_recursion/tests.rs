//! Structural and native-primitive tests for the Offline Cash V1 recursion seam.

use std::cell::RefCell;

use ff::Field as _;
use halo2_proofs::{
    dev::MockProver,
    halo2curves::{
        group::{Curve as _, Group as _, GroupEncoding, prime::PrimeCurveAffine as _},
        pasta::{Ep, EpAffine, Eq, EqAffine, Fp, Fq},
    },
    poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    domain::DomainId,
    nexus::AxtAssetIncarnationV1,
    offline::{
        OFFLINE_CASH_CURRENT_PROOFS_MAX_BYTES_V1, OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1,
        OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1, OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
        OFFLINE_CASH_WIRE_VERSION_V1, OfflineCashArtifactBindingV1, OfflineCashArtifactRoleV1,
        OfflineCashCommitWrapperProofV1, OfflineCashLifecycleBindingV1, OfflineCashOperationKindV1,
        OfflineCashPastaStateCommitmentV1, offline_cash_liability_pool_id_v1,
    },
};
use snark_verifier::{loader::native::NativeLoader, pcs::ipa::IpaAccumulator};

use super::*;
use crate::zk::offline_cash_v1_state::{ConsumedCreditInsertWitnessV1, CreditIdV1, DigestV1};

fn digest(tag: u8) -> DigestV1 {
    [tag; 32]
}

fn eq_digest(tag: u64) -> DigestV1 {
    crate::zk::offline_cash_v1_poseidon::encode(Fp::from(tag))
}

fn ep_digest(tag: u64) -> DigestV1 {
    crate::zk::offline_cash_v1_poseidon::encode(Fq::from(tag))
}

fn pasta_pair(tag: u64) -> OfflineCashPastaStateCommitmentV1 {
    OfflineCashPastaStateCommitmentV1 {
        eq: eq_digest(tag),
        ep: ep_digest(tag + 1),
    }
}

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"offline-cash-v1-recursion-tests",
    )))
}

fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "xor".parse().expect("asset name"),
    )
}

fn incarnation() -> AxtAssetIncarnationV1 {
    let network = network();
    let asset = asset();
    let registration =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"offline-cash-registration"));
    AxtAssetIncarnationV1::derive(
        &network,
        &asset,
        &registration,
        &Hash::new(b"offline-cash-registration-execution"),
        1,
    )
}

fn eq_history(tag: u64) -> OfflineCashEqAccumulatorV1 {
    let challenges = (0..OFFLINE_CASH_RECURSION_IPA_K_V1)
        .map(|round| Fp::from(tag + u64::from(round) + 1))
        .collect();
    let point = (Eq::generator() * Fp::from(tag + 97)).to_affine();
    OfflineCashEqAccumulatorV1::from_native(&IpaAccumulator::<EqAffine, NativeLoader>::new(
        challenges, point,
    ))
    .expect("canonical Eq fixture")
}

fn ep_history(tag: u64) -> OfflineCashEpAccumulatorV1 {
    let challenges = (0..OFFLINE_CASH_RECURSION_IPA_K_V1)
        .map(|round| Fq::from(tag + u64::from(round) + 1))
        .collect();
    let point = (Ep::generator() * Fq::from(tag + 193)).to_affine();
    OfflineCashEpAccumulatorV1::from_native(&IpaAccumulator::<EpAffine, NativeLoader>::new(
        challenges, point,
    ))
    .expect("canonical Ep fixture")
}

fn artifact_binding(role: OfflineCashArtifactRoleV1, tag: u8) -> OfflineCashArtifactBindingV1 {
    OfflineCashArtifactBindingV1 {
        role,
        sha256: digest(tag),
        byte_len: 1_024,
    }
}

fn artifacts() -> OfflineCashRecursionArtifactsV1 {
    OfflineCashRecursionArtifactsV1 {
        release_id: digest(0x43),
        profile_digest: digest(0x50),
        eq_protocol_digest: eq_digest(0x15),
        ep_protocol_digest: ep_digest(0x16),
        commit_wrapper_eq_protocol_digest: eq_digest(0x17),
        commit_wrapper_ep_protocol_digest: ep_digest(0x18),
        mint_authorization_eq_protocol_digest: eq_digest(0x19),
        mint_authorization_ep_protocol_digest: ep_digest(0x1A),
        guard_bundle_eq_protocol_digest: eq_digest(0x1B),
        guard_bundle_ep_protocol_digest: ep_digest(0x1C),
        guard_bundle_verifying_key_eq: artifact_binding(
            OfflineCashArtifactRoleV1::GuardBundleVkEq,
            0x53,
        ),
        guard_bundle_verifying_key_ep: artifact_binding(
            OfflineCashArtifactRoleV1::GuardBundleVkEp,
            0x54,
        ),
        commit_wrapper_verifying_key_eq: artifact_binding(
            OfflineCashArtifactRoleV1::CommitWrapperVkEq,
            0x55,
        ),
        commit_wrapper_verifying_key_ep: artifact_binding(
            OfflineCashArtifactRoleV1::CommitWrapperVkEp,
            0x56,
        ),
        mint_finality: OfflineCashMintFinalityArtifactsV1 {
            proving_key_eq: artifact_binding(OfflineCashArtifactRoleV1::MintCreditPkEq, 0x57),
            verifying_key_eq: artifact_binding(OfflineCashArtifactRoleV1::MintCreditVkEq, 0x58),
            proving_key_ep: artifact_binding(OfflineCashArtifactRoleV1::MintCreditPkEp, 0x59),
            verifying_key_ep: artifact_binding(OfflineCashArtifactRoleV1::MintCreditVkEp, 0x5A),
        },
        artifact_manifest_digest: digest(0x5B),
        canonical_empty_effect_digest: digest(0x40),
    }
}

fn send_output() -> OfflineCashRecursivePublicOutputV1 {
    let network_id = network();
    let asset = asset();
    let asset_incarnation = incarnation();
    let lifecycle = OfflineCashLifecycleBindingV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        network_id,
        protocol_version: OFFLINE_CASH_WIRE_VERSION_V1,
        suite_id: digest(0x60),
        vk_digest: digest(0x61),
        release_id: artifacts().release_id,
        asset: asset.clone(),
        asset_incarnation,
        scale: 4,
        liability_pool_id: offline_cash_liability_pool_id_v1(
            &network_id,
            &asset,
            asset_incarnation,
        )
        .expect("liability pool"),
        hardware_profile_id: digest(0x62),
        policy_epoch: 1,
        operation_kind: OfflineCashOperationKindV1::SendSplit,
        request_id: digest(0x63),
        acceptance_ticket_id: digest(0x64),
        credit_id: digest(0x65),
        ciphertext_digest: digest(0x66),
    };
    OfflineCashRecursivePublicOutputV1::new(
        lifecycle,
        digest(0x67),
        digest(0x68),
        digest(0x69),
        digest(0x6A),
        digest(0x6B),
        digest(0x6C),
        digest(0x6D),
        75,
        digest(0x6E),
    )
    .expect("valid unlinkable send output")
}

fn wrapper_proof(
    output: &OfflineCashRecursivePublicOutputV1,
    eq_body_len: usize,
    ep_body_len: usize,
) -> OfflineCashCommitWrapperProofV1 {
    OfflineCashCommitWrapperProofV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        eq_protocol_digest: artifacts().commit_wrapper_eq_protocol_digest,
        ep_protocol_digest: artifacts().commit_wrapper_ep_protocol_digest,
        semantic_digest: output.semantic_digest,
        candidate_envelope_digest: output.candidate_envelope_digest,
        commit_certificate_digest: output.commit_certificate_digest,
        eq_deferred_audit: eq_digest(0x71),
        ep_deferred_audit: ep_digest(0x72),
        eq_proof: vec![0xA1; eq_body_len],
        ep_proof: vec![0xB2; ep_body_len],
        eq_history: eq_history(7).as_bytes().to_vec(),
        ep_history: ep_history(11).as_bytes().to_vec(),
    }
}

/// Exact test-only checker; production has no accepting non-cryptographic backend.
struct ExactFixtureVerifier {
    expected_output: OfflineCashRecursivePublicOutputV1,
    expected_proof: OfflineCashCommitWrapperProofV1,
    calls: RefCell<Vec<(OfflineCashPastaParityV1, usize, usize)>>,
}

impl ExactFixtureVerifier {
    fn new(
        expected_output: OfflineCashRecursivePublicOutputV1,
        expected_proof: OfflineCashCommitWrapperProofV1,
    ) -> Self {
        Self {
            expected_output,
            expected_proof,
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl OfflineCashRecursiveVerifierV1 for ExactFixtureVerifier {
    fn verify_state_proof_and_decide(
        &self,
        _request: &OfflineCashStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("fixture has no state proof".to_owned())
    }

    fn verify_mint_finality_helper(
        &self,
        _request: &OfflineCashMintFinalityHelperVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("fixture has no mint proof".to_owned())
    }

    fn verify_commit_wrapper_and_decide(
        &self,
        request: &OfflineCashParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        let (protocol, proof, history) = match request.parity {
            OfflineCashPastaParityV1::Eq => (
                self.expected_proof.eq_protocol_digest,
                self.expected_proof.eq_proof.as_slice(),
                self.expected_proof.eq_history.as_slice(),
            ),
            OfflineCashPastaParityV1::Ep => (
                self.expected_proof.ep_protocol_digest,
                self.expected_proof.ep_proof.as_slice(),
                self.expected_proof.ep_history.as_slice(),
            ),
        };
        if request.protocol_digest != protocol
            || request.public_output != &self.expected_output
            || request.eq_deferred_audit != self.expected_proof.eq_deferred_audit
            || request.ep_deferred_audit != self.expected_proof.ep_deferred_audit
            || request.current_proof != proof
            || request.history_accumulator.as_slice() != history
        {
            return Err("fixture substitution".to_owned());
        }
        self.calls.borrow_mut().push((
            request.parity,
            request.current_proof.len(),
            request.history_accumulator.len(),
        ));
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AggregateState {
    balance: u128,
    sequence: u128,
    epoch: u128,
    replay_root: OfflineCashPastaStateCommitmentV1,
}

impl AggregateState {
    fn empty(root_seed: u64) -> Self {
        Self {
            balance: 0,
            sequence: 0,
            epoch: 1,
            replay_root: pasta_pair(root_seed),
        }
    }

    fn receive(&mut self, amount: u128, next_root: u64) -> OfflineCashOperationRelationWitnessV1 {
        let before = *self;
        self.balance = self.balance.checked_add(amount).expect("u128 balance");
        self.sequence = self.sequence.checked_add(1).expect("u128 sequence");
        self.replay_root = pasta_pair(next_root);
        operation_witness(before, *self, OfflineCashOperationV1::ReceiveFold, amount)
    }

    fn spend(
        &mut self,
        operation: OfflineCashOperationV1,
        amount: u128,
    ) -> OfflineCashOperationRelationWitnessV1 {
        assert!(matches!(
            operation,
            OfflineCashOperationV1::SendSplit | OfflineCashOperationV1::RedeemSplit
        ));
        let before = *self;
        self.balance = self
            .balance
            .checked_sub(amount)
            .expect("sufficient balance");
        self.sequence = self.sequence.checked_add(1).expect("u128 sequence");
        operation_witness(before, *self, operation, amount)
    }
}

fn operation_witness(
    before: AggregateState,
    after: AggregateState,
    operation: OfflineCashOperationV1,
    amount: u128,
) -> OfflineCashOperationRelationWitnessV1 {
    OfflineCashOperationRelationWitnessV1 {
        operation,
        balance_before: before.balance,
        balance_after: after.balance,
        amount,
        logical_sequence_before: before.sequence,
        logical_sequence_after: after.sequence,
        hardware_epoch_before: before.epoch,
        hardware_epoch_after: after.epoch,
        replay_root_before: before.replay_root,
        replay_root_after: after.replay_root,
    }
}

fn operation_tag(operation: OfflineCashOperationV1) -> u64 {
    match operation {
        OfflineCashOperationV1::Bootstrap => 0,
        OfflineCashOperationV1::MintFold => 1,
        OfflineCashOperationV1::SendSplit => 2,
        OfflineCashOperationV1::ReceiveFold => 3,
        OfflineCashOperationV1::RedeemSplit => 4,
        OfflineCashOperationV1::SuiteUpgrade => 5,
        OfflineCashOperationV1::Rotate => 6,
    }
}

fn assert_paired_operation_relation(witness: OfflineCashOperationRelationWitnessV1) {
    let tag = operation_tag(witness.operation);
    MockProver::run(
        12,
        &OfflineCashOperationRelationCircuitV1::<Fp>::new(witness),
        vec![vec![Fp::from(tag)]],
    )
    .expect("Eq relation synthesizes")
    .assert_satisfied();
    MockProver::run(
        12,
        &OfflineCashOperationRelationCircuitV1::<Fq>::new(witness),
        vec![vec![Fq::from(tag)]],
    )
    .expect("Ep relation synthesizes")
    .assert_satisfied();
}

#[test]
fn accumulators_are_exactly_544_bytes_and_strictly_canonical() {
    let eq = eq_history(1);
    let ep = ep_history(2);
    assert_eq!(
        eq.as_bytes().len(),
        OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1
    );
    assert_eq!(
        ep.as_bytes().len(),
        OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1
    );
    assert_eq!(
        OfflineCashEqAccumulatorV1::try_from_bytes(eq.as_bytes()).unwrap(),
        eq
    );
    assert_eq!(
        OfflineCashEpAccumulatorV1::try_from_bytes(ep.as_bytes()).unwrap(),
        ep
    );

    assert!(matches!(
        OfflineCashEqAccumulatorV1::try_from_bytes(&eq.as_bytes()[..543]),
        Err(OfflineCashRecursionErrorV1::InvalidAccumulatorLength { actual: 543, .. })
    ));
    let mut noncanonical = *eq.as_bytes();
    noncanonical[..32].fill(0xFF);
    assert!(matches!(
        OfflineCashEqAccumulatorV1::try_from_bytes(&noncanonical),
        Err(OfflineCashRecursionErrorV1::NonCanonicalAccumulatorScalar {
            parity: OfflineCashPastaParityV1::Eq,
            round: 0,
        })
    ));
    let mut identity = *ep.as_bytes();
    identity[512..].copy_from_slice(EpAffine::identity().to_bytes().as_ref());
    assert!(matches!(
        OfflineCashEpAccumulatorV1::try_from_bytes(&identity),
        Err(OfflineCashRecursionErrorV1::InvalidAccumulatorPoint(
            OfflineCashPastaParityV1::Ep
        ))
    ));
}

#[test]
fn fold_transcripts_have_one_fixed_shape() {
    assert_eq!(OFFLINE_CASH_IPA_FOLD_PROOF_BYTES_V1, 1_280);
    assert!(OfflineCashEqFoldProofV1::try_from_bytes(&vec![0; 1_280]).is_ok());
    assert!(OfflineCashEpFoldProofV1::try_from_bytes(&vec![0; 1_280]).is_ok());
    assert!(OfflineCashEqFoldProofV1::try_from_bytes(&vec![0; 1_279]).is_err());
    assert!(OfflineCashEpFoldProofV1::try_from_bytes(&vec![0; 1_281]).is_err());
}

#[test]
fn one_thousand_receipts_collapse_into_one_unrestricted_balance() {
    let mut merchant = AggregateState::empty(10_000);
    let mut first = None;
    let mut last = None;
    for receipt in 0_u64..1_000 {
        let witness = merchant.receive(1, 10_001 + receipt);
        first.get_or_insert(witness);
        last = Some(witness);
    }
    assert_eq!((merchant.balance, merchant.sequence), (1_000, 1_000));
    assert_paired_operation_relation(first.expect("first receipt"));
    assert_paired_operation_relation(last.expect("last receipt"));

    assert_paired_operation_relation(merchant.spend(OfflineCashOperationV1::SendSplit, 1_000));
    assert_eq!(merchant.balance, 0);

    let mut recipient = AggregateState::empty(20_000);
    assert_paired_operation_relation(recipient.receive(1_000, 20_001));
    assert_paired_operation_relation(recipient.spend(OfflineCashOperationV1::RedeemSplit, 400));
    assert_paired_operation_relation(recipient.spend(OfflineCashOperationV1::SendSplit, 600));
    assert_eq!(recipient.balance, 0);
}

#[test]
fn one_thousand_twenty_four_handoffs_keep_fixed_public_and_wire_shapes() {
    const STATE_PUBLIC_INSTANCE_COUNT: usize =
        offline_cash_state_public_instance_v1::ASSET_SCALE + 1;
    const RECURSIVE_PUBLIC_INSTANCE_COUNT: usize =
        STATE_PUBLIC_INSTANCE_COUNT + OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1 / 16;
    assert_eq!(STATE_PUBLIC_INSTANCE_COUNT, 81);
    assert_eq!(RECURSIVE_PUBLIC_INSTANCE_COUNT, 115);

    let output = send_output();
    let reference = wrapper_proof(&output, 256, 256);
    let reference_len = norito::encode_canonical(&reference)
        .expect("proof encoding")
        .len();
    let mut holder = AggregateState::empty(30_000);
    holder.receive(1, 30_001);

    for depth in 1_u64..=1_024 {
        let send = holder.spend(OfflineCashOperationV1::SendSplit, 1);
        let mut receiver = AggregateState::empty(40_000 + depth * 2);
        let receive = receiver.receive(1, 40_001 + depth * 2);
        if matches!(depth, 8 | 64 | 1_024) {
            assert_paired_operation_relation(send);
            assert_paired_operation_relation(receive);
            let mut depth_proof = reference.clone();
            depth_proof.eq_history = eq_history(depth).as_bytes().to_vec();
            depth_proof.ep_history = ep_history(depth).as_bytes().to_vec();
            assert_eq!(
                norito::encode_canonical(&depth_proof)
                    .expect("depth proof encoding")
                    .len(),
                reference_len,
                "proof wire size changed at depth {depth}",
            );
        }
        holder = receiver;
    }
    assert_eq!(holder.balance, 1);
}

#[test]
fn commit_wrapper_verification_is_constant_work_and_rejects_substitution() {
    let output = send_output();
    let proof = wrapper_proof(&output, 1_280, 1_280);
    let verifier = ExactFixtureVerifier::new(output.clone(), proof.clone());
    let verified =
        verify_offline_cash_recursive_proof_v1(&verifier, artifacts(), output.clone(), &proof)
            .expect("exact fixture verifies");
    assert_eq!(verified.public_output(), output);
    assert_eq!(verifier.calls.borrow().len(), 2);

    let mut protocol_substitution = proof.clone();
    protocol_substitution.eq_protocol_digest = eq_digest(0x99);
    assert!(matches!(
        verify_offline_cash_recursive_proof_v1(
            &ExactFixtureVerifier::new(output.clone(), protocol_substitution.clone()),
            artifacts(),
            output.clone(),
            &protocol_substitution,
        ),
        Err(OfflineCashRecursionErrorV1::ArtifactSubstitution)
    ));

    let mut body_substitution = proof.clone();
    body_substitution.eq_proof[0] ^= 1;
    assert!(matches!(
        verify_offline_cash_recursive_proof_v1(
            &verifier,
            artifacts(),
            output.clone(),
            &body_substitution,
        ),
        Err(OfflineCashRecursionErrorV1::TransitionProofRejected {
            parity: OfflineCashPastaParityV1::Eq,
            ..
        })
    ));

    let mut noncanonical = proof;
    noncanonical.eq_history[..32].fill(0xFF);
    assert!(matches!(
        verify_offline_cash_recursive_proof_v1(
            &ExactFixtureVerifier::new(output.clone(), noncanonical.clone()),
            artifacts(),
            output,
            &noncanonical,
        ),
        Err(OfflineCashRecursionErrorV1::NonCanonicalAccumulatorScalar {
            parity: OfflineCashPastaParityV1::Eq,
            round: 0,
        })
    ));
}

#[test]
fn wrapper_caps_and_incoming_binding_are_history_independent() {
    assert_eq!(OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1, 2_495);
    assert_eq!(OFFLINE_CASH_CURRENT_PROOFS_MAX_BYTES_V1, 4_990);
    assert_eq!(OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1, 6_528);
    let output = send_output();
    let proof = wrapper_proof(
        &output,
        OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
        OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
    );
    assert!(
        norito::encode_canonical(&proof)
            .expect("proof encoding")
            .len()
            <= OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1
    );
    let binding =
        offline_cash_incoming_proof_binding_digest_v1(&output, &proof).expect("incoming binding");
    let mut different_terminal_output = output.clone();
    different_terminal_output.terminal_output_binding = digest(0xEF);
    assert_ne!(
        offline_cash_incoming_proof_binding_digest_v1(&different_terminal_output, &proof)
            .expect("different terminal output binding"),
        binding,
    );
    let mut substituted = proof.clone();
    substituted.semantic_digest = digest(0xEE);
    assert_eq!(
        offline_cash_incoming_proof_binding_digest_v1(&output, &substituted),
        Err(OfflineCashRecursionErrorV1::PublicBindingMismatch)
    );
    let mut different_history = proof;
    different_history.eq_history = eq_history(900).as_bytes().to_vec();
    assert_eq!(
        offline_cash_incoming_proof_binding_digest_v1(&output, &different_history)
            .expect("different valid history"),
        binding,
    );
}

#[test]
fn reject_all_backend_never_grants_monetary_authority() {
    let output = send_output();
    let proof = wrapper_proof(&output, 128, 128);
    assert!(matches!(
        verify_offline_cash_recursive_proof_v1(
            &RejectAllOfflineCashRecursiveVerifierV1,
            artifacts(),
            output,
            &proof,
        ),
        Err(OfflineCashRecursionErrorV1::TransitionProofRejected {
            parity: OfflineCashPastaParityV1::Eq,
            ..
        })
    ));
}

#[test]
fn replay_insert_witness_requires_a_real_root_change() {
    let state_witness = ConsumedCreditInsertWitnessV1 {
        credit_id: CreditIdV1(digest(0xA1)),
        envelope_digest: digest(0xA2),
        predecessor_root: pasta_pair(0xA3),
        successor_root: pasta_pair(0xA4),
        siblings_root_to_leaf: [pasta_pair(0xA5); OFFLINE_CASH_REPLAY_PATH_DEPTH_V1],
    };
    let witness = OfflineCashReplayInsertWitnessV1::from(&state_witness);
    witness.validate_shape().expect("valid replay insertion");
    assert_eq!(witness.siblings_root_to_leaf.len(), 256);
    let mut no_op = witness;
    no_op.successor_root = no_op.predecessor_root;
    assert_eq!(
        no_op.validate_shape(),
        Err(OfflineCashRecursionErrorV1::InvalidReplayWitness)
    );
}

fn ipa_h_coefficients<F: ff::Field>(challenges: &[F], scalar: F) -> Vec<F> {
    let mut coefficients = vec![F::ZERO; 1 << challenges.len()];
    coefficients[0] = scalar;
    for (len, challenge) in challenges
        .iter()
        .rev()
        .enumerate()
        .map(|(index, challenge)| (1 << index, challenge))
    {
        let (left, right) = coefficients.split_at_mut(len);
        let right = &mut right[..len];
        right.copy_from_slice(left);
        for coefficient in right {
            *coefficient *= challenge;
        }
    }
    coefficients
}

fn valid_eq_accumulator(params: &ParamsIPA<EqAffine>, seed: u64) -> OfflineCashEqAccumulatorV1 {
    let challenges = (0..OFFLINE_CASH_RECURSION_IPA_K_V1)
        .map(|round| Fp::from(seed + u64::from(round) + 1))
        .collect::<Vec<_>>();
    let coefficients = ipa_h_coefficients(&challenges, Fp::ONE);
    let point = params
        .get_g()
        .iter()
        .zip(coefficients)
        .fold(Eq::identity(), |sum, (base, coefficient)| {
            sum + *base * coefficient
        })
        .to_affine();
    OfflineCashEqAccumulatorV1::from_native(&IpaAccumulator::new(challenges, point)).unwrap()
}

fn valid_ep_accumulator(params: &ParamsIPA<EpAffine>, seed: u64) -> OfflineCashEpAccumulatorV1 {
    let challenges = (0..OFFLINE_CASH_RECURSION_IPA_K_V1)
        .map(|round| Fq::from(seed + u64::from(round) + 1))
        .collect::<Vec<_>>();
    let coefficients = ipa_h_coefficients(&challenges, Fq::ONE);
    let point = params
        .get_g()
        .iter()
        .zip(coefficients)
        .fold(Ep::identity(), |sum, (base, coefficient)| {
            sum + *base * coefficient
        })
        .to_affine();
    OfflineCashEpAccumulatorV1::from_native(&IpaAccumulator::new(challenges, point)).unwrap()
}

#[test]
#[ignore = "expensive fixed-k native primitive qualification; run in the Offline Cash release lane"]
fn real_k16_native_folds_decide_and_reject_substitution() {
    let eq_params = ParamsIPA::<EqAffine>::new(OFFLINE_CASH_RECURSION_IPA_K_V1);
    let eq_current = valid_eq_accumulator(&eq_params, 3);
    let eq_predecessor = valid_eq_accumulator(&eq_params, 29);
    let eq_fold =
        fold_offline_cash_eq_accumulators_v1(&eq_params, &eq_current, &eq_predecessor).unwrap();
    verify_and_decide_offline_cash_eq_fold_v1(&eq_params, &eq_current, &eq_predecessor, &eq_fold)
        .unwrap();
    let mut tampered = eq_fold.proof().as_bytes().to_vec();
    tampered[0] ^= 1;
    let tampered = OfflineCashEqFoldOutputV1::from_parts(
        eq_fold.successor().clone(),
        OfflineCashEqFoldProofV1::try_from_bytes(&tampered).unwrap(),
    );
    assert!(
        verify_and_decide_offline_cash_eq_fold_v1(
            &eq_params,
            &eq_current,
            &eq_predecessor,
            &tampered,
        )
        .is_err()
    );

    let ep_params = ParamsIPA::<EpAffine>::new(OFFLINE_CASH_RECURSION_IPA_K_V1);
    let ep_current = valid_ep_accumulator(&ep_params, 5);
    let ep_predecessor = valid_ep_accumulator(&ep_params, 31);
    let ep_fold =
        fold_offline_cash_ep_accumulators_v1(&ep_params, &ep_current, &ep_predecessor).unwrap();
    verify_and_decide_offline_cash_ep_fold_v1(&ep_params, &ep_current, &ep_predecessor, &ep_fold)
        .unwrap();
}
