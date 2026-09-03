//! Real-proof payment-corridor fixtures, separate from transition-model qualification.
//!
//! The mint fixture creates a genuine four-validator finality certificate and recursively
//! proves Bootstrap -> FinalizedMint with one reusable carrier key pair. Certificate preflight
//! tests are not proof evidence; the explicitly ignored real-proof gate is expensive.
//!
//! TODO: connect these funded inputs to the revised terminal/payment-proof corridor once its
//! post-commit proof shape is fixed. No rotation or fabricated positive bootstrap substitutes
//! for the pending SendSplit -> transported authorization -> ReceiveFold qualification.

use super::*;
use crate::zk::kagemusha_v1_recursion::{
    KagemushaGeneratedMintAuthorityArtifactsV1, KagemushaGeneratedMintAuthorityProofV1,
    KagemushaLoadedEpMintAuthorityArtifactsV1, KagemushaLoadedEqMintAuthorityArtifactsV1,
    KagemushaMintAuthorityGenerationWitnessV1, KagemushaMintAuthorityStepV1,
    KagemushaMintCertificateWitnessV1, KagemushaMintFinalitySignerV1, KagemushaMintFinalityTreeV1,
    accumulation::{
        fold_kagemusha_ep_accumulators_with_rng_v1, fold_kagemusha_eq_accumulators_with_rng_v1,
    },
    generate_kagemusha_mint_authority_artifacts_v1, kagemusha_mint_finality_empty_root_v1,
    mint_authority::{
        KagemushaMintAuthorityEpCircuitV1, KagemushaMintAuthorityEqCircuitV1,
        public_instance as mint_instance,
    },
    mint_transport_decider::{
        KagemushaMintAuthorityTransportEpCircuitV1, KagemushaMintAuthorityTransportEqCircuitV1,
    },
    prove_kagemusha_mint_authority_v1,
};
use halo2_proofs::{
    halo2curves::{ff::Field as _, group::Group as _},
    poly::commitment::Params as _,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::AccountId,
    block::consensus_v2::{HeightContextId, ValidatorPower},
    isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1,
        KagemushaMintFinalityEpochRosterV1, KagemushaMintFinalitySealBundleV1,
        KagemushaMintFinalitySealMessageV1, KagemushaPastaSchnorrSignatureV1, KagemushaTopUpLeafV1,
        KagemushaTopUpMembershipWitnessV1, kagemusha_mint_finality_root_v1,
    },
    kagemusha::{
        KagemushaLifecycleBindingV1, KagemushaMintCreditStatementV1, KagemushaOperationKindV1,
    },
    peer::PeerId,
};
use zeroize::Zeroizing;

/// Reusable genuinely signed funding material, not an invented nonzero state.
struct FundingCertificate {
    bootstrap: KagemushaMintCertificateWitnessV1,
    finalized: KagemushaMintCertificateWitnessV1,
    genesis_roster_id: DigestV1,
}

impl FundingCertificate {
    fn new(state: &KagemushaStateV1, recipient: AccountId, amount: u128) -> Self {
        assert!(
            amount > 0,
            "a payment corridor must begin with positive funding"
        );
        let mut validators = (0_u8..4)
            .map(|index| {
                let key = KeyPair::from_seed(vec![index + 1; 32], Algorithm::Ed25519);
                ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                }
            })
            .collect::<Vec<_>>();
        validators.sort_by(|left, right| left.validator.cmp(&right.validator));
        let roster = crate::kagemusha_v1_test_fixtures::mint_finality_roster(
            state.lane.network_id,
            0,
            &validators,
        );
        let genesis_roster_id = roster.finality_epoch_id().expect("finality roster ID");
        let statement = KagemushaMintCreditStatementV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            lifecycle: KagemushaLifecycleBindingV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                network_id: state.lane.network_id,
                protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
                suite_id: state.suite_id,
                vk_digest: state.vk_digest,
                release_id: state.release_id,
                asset: state.lane.asset.clone(),
                asset_incarnation: state.asset_incarnation,
                scale: state.lane.scale,
                liability_pool_id: state.liability_pool_id,
                hardware_profile_id: state.hardware_profile_id,
                policy_epoch: state.policy_epoch,
                operation_kind: KagemushaOperationKindV1::MintFold,
                request_id: [0; 32],
                receiver_lane_commitment: [0; 32],
                credit_id: [0; 32],
                ciphertext_digest: digest(b"funding-ciphertext", 0),
            },
            recipient_credential_commitment: digest(b"funding-recipient-credential", 0),
            authorization_context_digest: digest(b"funding-authorization-context", 0),
            mint_authorization_digest: digest(b"funding-authorization", 0),
            amount,
            issuance_commitment: digest(b"funding-issuance", 0),
            recipient,
            credit_commitment: digest(b"funding-credit", 0),
            minted_at_ms: 100,
        }
        .seal_credit_id()
        .expect("seal positive-value funding statement");
        let leaf = KagemushaTopUpLeafV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            operation_id: digest(b"funding-operation", 0),
            reserve_receipt_digest: digest(b"funding-reserve-receipt", 0),
            statement_digest: statement
                .canonical_digest()
                .expect("funding statement digest"),
            amount,
        };
        let tree = KagemushaMintFinalityTreeV1::new(vec![leaf.clone()])
            .expect("genuine sparse top-up membership tree");
        let message = Self::message(
            &roster,
            genesis_roster_id,
            tree.execution_root(),
            tree.leaf_count(),
            None,
        );
        let seals = (0_u32..3)
            .map(|index| {
                let signer = KagemushaMintFinalitySignerV1::from_seed(
                    Zeroizing::new([0xA0 + u8::try_from(index).expect("small signer index"); 32]),
                    index,
                    &roster,
                )
                .expect("real signer admitted by its paired-Pasta roster keys");
                signer
                    .sign(&message)
                    .expect("real paired-Pasta finality seal")
            })
            .collect();
        let finalized = KagemushaMintCertificateWitnessV1 {
            statement: statement.clone(),
            membership: tree
                .witness(leaf.operation_id)
                .expect("funding membership path"),
            seal_bundle: KagemushaMintFinalitySealBundleV1 { message, seals },
            epoch_roster: roster.clone(),
        };
        finalized
            .validate_shape()
            .expect("valid positive-value funding certificate");
        let empty_root = kagemusha_mint_finality_empty_root_v1().expect("empty finality root");
        let bootstrap = KagemushaMintCertificateWitnessV1 {
            statement,
            membership: KagemushaTopUpMembershipWitnessV1 {
                leaf,
                leaf_index: 0,
                root: empty_root,
                siblings: vec![
                    KagemushaPastaStateCommitmentV1::ZERO;
                    KAGEMUSHA_MINT_FINALITY_TREE_DEPTH_V1
                ],
            },
            seal_bundle: KagemushaMintFinalitySealBundleV1 {
                message: Self::message(
                    &roster,
                    genesis_roster_id,
                    kagemusha_mint_finality_root_v1(empty_root),
                    0,
                    Some(genesis_roster_id),
                ),
                seals: Vec::new(),
            },
            epoch_roster: roster,
        };
        bootstrap
            .validate_for_step(KagemushaMintAuthorityStepV1::Bootstrap)
            .expect("zero-authority bootstrap certificate shape");
        Self {
            bootstrap,
            finalized,
            genesis_roster_id,
        }
    }

    fn message(
        roster: &KagemushaMintFinalityEpochRosterV1,
        epoch_id: DigestV1,
        root: Hash,
        count: u32,
        next: Option<DigestV1>,
    ) -> KagemushaMintFinalitySealMessageV1 {
        KagemushaMintFinalitySealMessageV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            finality_epoch_id: epoch_id,
            validator_count: u32::try_from(roster.validators.len()).expect("four validators"),
            network_id: roster.network_id,
            block_height: if count == 0 { 1 } else { 2 },
            height_context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(digest(
                b"funding-height",
                u64::from(count),
            )))),
            subject_digest: digest(b"funding-subject", u64::from(count)),
            execution_commitment_digest: digest(b"funding-execution", u64::from(count)),
            kagemusha_top_up_root: root,
            kagemusha_top_up_count: count,
            next_finality_epoch_id: next,
        }
    }
}

/// Loaded keys retained across the bootstrap and every funded mint proof.
struct MintKeys {
    eq: KagemushaLoadedEqMintAuthorityArtifactsV1,
    ep: KagemushaLoadedEpMintAuthorityArtifactsV1,
    eq_protocol: PlonkProtocol<EqAffine>,
    ep_protocol: PlonkProtocol<EpAffine>,
    eq_digest: DigestV1,
    ep_digest: DigestV1,
}

impl MintKeys {
    fn decode(generated: KagemushaGeneratedMintAuthorityArtifactsV1) -> Self {
        macro_rules! decode {
            ($bytes:expr, $key:ident, $circuit:ty, $params:expr) => {{
                let mut cursor = Cursor::new($bytes.as_ref());
                let key = $key::read_checked::<_, $circuit>(
                    &mut cursor,
                    SerdeFormat::Processed,
                    u32::try_from($params.k).expect("generated mint circuit degree"),
                    $params.clone(),
                )
                .expect("decode generated mint-authority key");
                assert_eq!(
                    usize::try_from(cursor.position()).expect("key cursor"),
                    $bytes.len()
                );
                key
            }};
        }
        // This direct-decoding fixture has no authenticated release profile or manifest.
        // These shared labels are test-only metadata, never release-authentication evidence;
        // the production loader obtains both identities from its authenticated artifact set.
        let test_only_profile_digest = digest(b"funding-test-profile-identity", 0);
        let test_only_manifest_digest = digest(b"funding-test-manifest-identity", 0);
        let eq = KagemushaLoadedEqMintAuthorityArtifactsV1 {
            parameters: ParamsIPA::read(&mut Cursor::new(generated.eq_parameters.as_ref()))
                .expect("Eq mint parameters"),
            proving_key: decode!(
                generated.eq_proving_key,
                ProvingKey,
                KagemushaMintAuthorityTransportEqCircuitV1,
                generated.eq_circuit_params
            ),
            verifying_key: decode!(
                generated.eq_verifying_key,
                VerifyingKey,
                KagemushaMintAuthorityTransportEqCircuitV1,
                generated.eq_circuit_params
            ),
            circuit_params: generated.eq_circuit_params,
            inner_proving_key: decode!(
                generated.inner_eq_proving_key,
                ProvingKey,
                KagemushaMintAuthorityEqCircuitV1,
                generated.inner_eq_circuit_params
            ),
            inner_verifying_key: decode!(
                generated.inner_eq_verifying_key,
                VerifyingKey,
                KagemushaMintAuthorityEqCircuitV1,
                generated.inner_eq_circuit_params
            ),
            inner_circuit_params: generated.inner_eq_circuit_params,
            protocol_digest: generated.eq_protocol_digest,
            release_id: generated.release_id,
            profile_digest: test_only_profile_digest,
            artifact_manifest_digest: test_only_manifest_digest,
            genesis_roster_id: generated.genesis_roster_id,
        };
        let ep = KagemushaLoadedEpMintAuthorityArtifactsV1 {
            parameters: ParamsIPA::read(&mut Cursor::new(generated.ep_parameters.as_ref()))
                .expect("Ep mint parameters"),
            proving_key: decode!(
                generated.ep_proving_key,
                ProvingKey,
                KagemushaMintAuthorityTransportEpCircuitV1,
                generated.ep_circuit_params
            ),
            verifying_key: decode!(
                generated.ep_verifying_key,
                VerifyingKey,
                KagemushaMintAuthorityTransportEpCircuitV1,
                generated.ep_circuit_params
            ),
            circuit_params: generated.ep_circuit_params,
            inner_proving_key: decode!(
                generated.inner_ep_proving_key,
                ProvingKey,
                KagemushaMintAuthorityEpCircuitV1,
                generated.inner_ep_circuit_params
            ),
            inner_verifying_key: decode!(
                generated.inner_ep_verifying_key,
                VerifyingKey,
                KagemushaMintAuthorityEpCircuitV1,
                generated.inner_ep_circuit_params
            ),
            inner_circuit_params: generated.inner_ep_circuit_params,
            protocol_digest: generated.ep_protocol_digest,
            release_id: generated.release_id,
            profile_digest: test_only_profile_digest,
            artifact_manifest_digest: test_only_manifest_digest,
            genesis_roster_id: generated.genesis_roster_id,
        };
        let eq_protocol = compile(
            &eq.parameters,
            &eq.verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_protocol = compile(
            &ep.parameters,
            &ep.verifying_key,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
        );
        assert_eq!(
            native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
                .expect("Eq mint identity"),
            generated.eq_protocol_digest
        );
        assert_eq!(
            native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
                .expect("Ep mint identity"),
            generated.ep_protocol_digest
        );
        Self {
            eq,
            ep,
            eq_protocol,
            ep_protocol,
            eq_digest: generated.eq_protocol_digest,
            ep_digest: generated.ep_protocol_digest,
        }
    }

    fn generate(
        eq_params: &ParamsIPA<EqAffine>,
        ep_params: &ParamsIPA<EpAffine>,
        eq_seed: PlonkProtocol<EqAffine>,
        ep_seed: PlonkProtocol<EpAffine>,
        funding: &FundingCertificate,
    ) -> Self {
        let eq_history =
            initial_kagemusha_eq_accumulator_v1(eq_params).expect("Eq mint seed history");
        let ep_history =
            initial_kagemusha_ep_accumulator_v1(ep_params).expect("Ep mint seed history");
        // The generator owns inner/outer key-shape convergence and returns stable outer keys.
        let padding = MintPadding::new(&eq_seed, &ep_seed, &eq_history, &ep_history);
        let started = std::time::Instant::now();
        let keys = Self::decode(
            generate_kagemusha_mint_authority_artifacts_v1(padding.witness(
                funding,
                &eq_seed,
                &ep_seed,
                &eq_history,
                &ep_history,
            ))
            .expect("generate real mint-authority keys"),
        );
        eprintln!(
            "KAGEMUSHA converged mint-authority key generation: {:?}",
            started.elapsed()
        );
        keys
    }

    fn prove_funding(&self, funding: &FundingCertificate) -> ProvenFunding {
        let eq_initial = initial_kagemusha_eq_accumulator_v1(&self.eq.parameters)
            .expect("Eq mint initial history");
        let ep_initial = initial_kagemusha_ep_accumulator_v1(&self.ep.parameters)
            .expect("Ep mint initial history");
        let padding = MintPadding::new(
            &self.eq_protocol,
            &self.ep_protocol,
            &eq_initial,
            &ep_initial,
        );
        let started = std::time::Instant::now();
        let bootstrap = prove_kagemusha_mint_authority_v1(
            &self.eq,
            &self.ep,
            padding.witness(
                funding,
                &self.eq_protocol,
                &self.ep_protocol,
                &eq_initial,
                &ep_initial,
            ),
        )
        .expect("real bootstrap mint-authority proof");
        // The compact proof already includes its private carrier in its history.
        // The next authority step must extend that transported history, not the empty seed.
        let eq_bootstrap_history =
            KagemushaEqAccumulatorV1::try_from_bytes(&bootstrap.proof.eq_history)
                .expect("decode actual Eq bootstrap history");
        let ep_bootstrap_history =
            KagemushaEpAccumulatorV1::try_from_bytes(&bootstrap.proof.ep_history)
                .expect("decode actual Ep bootstrap history");
        self.decide(&bootstrap, &eq_bootstrap_history, &ep_bootstrap_history);
        eprintln!(
            "KAGEMUSHA genuine mint-authority bootstrap proof: {:?}",
            started.elapsed()
        );
        let eq_fold = fold_kagemusha_eq_accumulators_with_rng_v1(
            &self.eq.parameters,
            &bootstrap.eq_current_accumulator,
            &eq_bootstrap_history,
            OsRng,
        )
        .expect("fold actual Eq bootstrap proof");
        let ep_fold = fold_kagemusha_ep_accumulators_with_rng_v1(
            &self.ep.parameters,
            &bootstrap.ep_current_accumulator,
            &ep_bootstrap_history,
            OsRng,
        )
        .expect("fold actual Ep bootstrap proof");
        let eq_instances = vec![bootstrap.eq_public_instances.clone()];
        let ep_instances = vec![bootstrap.ep_public_instances.clone()];
        let started = std::time::Instant::now();
        let finalized = prove_kagemusha_mint_authority_v1(
            &self.eq,
            &self.ep,
            KagemushaMintAuthorityGenerationWitnessV1 {
                step: KagemushaMintAuthorityStepV1::FinalizedMint,
                release_id: funding.finalized.statement.lifecycle.release_id,
                genesis_roster_id: funding.genesis_roster_id,
                eq_protocol_digest: self.eq_digest,
                ep_protocol_digest: self.ep_digest,
                eq_deferred_audit: [1; 32],
                ep_deferred_audit: [2; 32],
                certificate: funding.finalized.clone(),
                eq_parent_protocol: &self.eq_protocol,
                ep_parent_protocol: &self.ep_protocol,
                eq_parent_instances: &eq_instances,
                ep_parent_instances: &ep_instances,
                eq_parent_proof: &bootstrap.proof.eq_proof,
                ep_parent_proof: &bootstrap.proof.ep_proof,
                eq_parent_history: &eq_bootstrap_history,
                ep_parent_history: &ep_bootstrap_history,
                eq_parent_fold_proof: eq_fold.proof(),
                ep_parent_fold_proof: ep_fold.proof(),
                eq_successor_history: eq_fold.successor(),
                ep_successor_history: ep_fold.successor(),
            },
        )
        .expect("real quorum-backed finalized-mint proof");
        let eq_finalized_history =
            KagemushaEqAccumulatorV1::try_from_bytes(&finalized.proof.eq_history)
                .expect("decode actual Eq finalized-mint history");
        let ep_finalized_history =
            KagemushaEpAccumulatorV1::try_from_bytes(&finalized.proof.ep_history)
                .expect("decode actual Ep finalized-mint history");
        self.decide(&finalized, &eq_finalized_history, &ep_finalized_history);
        eprintln!(
            "KAGEMUSHA genuine finalized-mint proof: {:?}; Eq={} Ep={} bytes",
            started.elapsed(),
            finalized.proof.eq_proof.len(),
            finalized.proof.ep_proof.len()
        );
        ProvenFunding {
            proof: finalized,
            eq_history: eq_finalized_history,
            ep_history: ep_finalized_history,
        }
    }

    fn decide(
        &self,
        proof: &KagemushaGeneratedMintAuthorityProofV1,
        eq_history: &KagemushaEqAccumulatorV1,
        ep_history: &KagemushaEpAccumulatorV1,
    ) {
        assert_eq!(
            eq_history.as_bytes().as_slice(),
            proof.proof.eq_history.as_slice(),
            "the decided Eq ancestry must be the transported proof history"
        );
        assert_eq!(
            ep_history.as_bytes().as_slice(),
            proof.proof.ep_history.as_slice(),
            "the decided Ep ancestry must be the transported proof history"
        );
        let eq_current = KagemushaEqAccumulatorV1::from_native(
            &verify_eq_succinct_protocol(
                &self.eq.parameters,
                &self.eq_protocol,
                &proof.proof.eq_proof,
                &proof.eq_public_instances,
            )
            .expect("verify actual Eq mint proof"),
        )
        .expect("Eq mint accumulator");
        let ep_current = KagemushaEpAccumulatorV1::from_native(
            &verify_ep_succinct_protocol(
                &self.ep.parameters,
                &self.ep_protocol,
                &proof.proof.ep_proof,
                &proof.ep_public_instances,
            )
            .expect("verify actual Ep mint proof"),
        )
        .expect("Ep mint accumulator");
        assert_eq!(eq_current, proof.eq_current_accumulator);
        assert_eq!(ep_current, proof.ep_current_accumulator);
        decide_kagemusha_eq_accumulator_v1(&self.eq.parameters, &eq_current)
            .expect("decide actual Eq mint equation");
        decide_kagemusha_ep_accumulator_v1(&self.ep.parameters, &ep_current)
            .expect("decide actual Ep mint equation");
        decide_kagemusha_eq_accumulator_v1(&self.eq.parameters, eq_history)
            .expect("decide actual Eq mint ancestry");
        decide_kagemusha_ep_accumulator_v1(&self.ep.parameters, ep_history)
            .expect("decide actual Ep mint ancestry");
        assert!(
            proof.proof.eq_proof.len() + proof.proof.ep_proof.len()
                <= KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
        );
    }
}

struct ProvenFunding {
    proof: KagemushaGeneratedMintAuthorityProofV1,
    eq_history: KagemushaEqAccumulatorV1,
    ep_history: KagemushaEpAccumulatorV1,
}

/// Inactive bootstrap parsing material; explicitly not an accepted predecessor proof.
struct MintPadding {
    eq_instances: Vec<Vec<Fp>>,
    ep_instances: Vec<Vec<Fq>>,
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_fold: KagemushaEqFoldProofV1,
    ep_fold: KagemushaEpFoldProofV1,
}

impl MintPadding {
    fn new(
        eq: &PlonkProtocol<EqAffine>,
        ep: &PlonkProtocol<EpAffine>,
        eq_history: &KagemushaEqAccumulatorV1,
        ep_history: &KagemushaEpAccumulatorV1,
    ) -> Self {
        assert_eq!(
            eq.num_instance,
            vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]
        );
        assert_eq!(
            ep.num_instance,
            vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]
        );
        Self {
            eq_instances: history_instances(mint_instance::HISTORY_START, eq_history.as_bytes()),
            ep_instances: history_instances(mint_instance::HISTORY_START, ep_history.as_bytes()),
            eq_proof: dummy_ordinary_proof(eq, EqAffine::generator()),
            ep_proof: dummy_ordinary_proof(ep, EpAffine::generator()),
            eq_fold: dummy_eq_fold(),
            ep_fold: dummy_ep_fold(),
        }
    }

    fn witness<'a>(
        &'a self,
        funding: &FundingCertificate,
        eq: &'a PlonkProtocol<EqAffine>,
        ep: &'a PlonkProtocol<EpAffine>,
        eq_history: &'a KagemushaEqAccumulatorV1,
        ep_history: &'a KagemushaEpAccumulatorV1,
    ) -> KagemushaMintAuthorityGenerationWitnessV1<'a> {
        KagemushaMintAuthorityGenerationWitnessV1 {
            step: KagemushaMintAuthorityStepV1::Bootstrap,
            release_id: funding.bootstrap.statement.lifecycle.release_id,
            genesis_roster_id: funding.genesis_roster_id,
            eq_protocol_digest: native_parent_protocol_digest_v1(eq, KagemushaPastaParityV1::Eq)
                .expect("Eq mint parent identity"),
            ep_protocol_digest: native_parent_protocol_digest_v1(ep, KagemushaPastaParityV1::Ep)
                .expect("Ep mint parent identity"),
            eq_deferred_audit: [1; 32],
            ep_deferred_audit: [2; 32],
            certificate: funding.bootstrap.clone(),
            eq_parent_protocol: eq,
            ep_parent_protocol: ep,
            eq_parent_instances: &self.eq_instances,
            ep_parent_instances: &self.ep_instances,
            eq_parent_proof: &self.eq_proof,
            ep_parent_proof: &self.ep_proof,
            eq_parent_history: eq_history,
            ep_parent_history: ep_history,
            eq_parent_fold_proof: &self.eq_fold,
            ep_parent_fold_proof: &self.ep_fold,
            eq_successor_history: eq_history,
            ep_successor_history: ep_history,
        }
    }
}

fn funding_fixture() -> (
    KagemushaPlatformCredentialRelationWitnessV1,
    KagemushaStateV1,
    FundingCertificate,
) {
    let release = digest(b"payment-corridor-release", 0);
    let (credential, _) = credential_witness(0, release, digest(b"empty-durable-effect", 0));
    let state = aggregate_state(
        release,
        &credential.statement,
        digest(b"payment-state-nonce", 0),
    );
    let recipient = AccountId::new(
        KeyPair::from_seed(vec![91; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let funding = FundingCertificate::new(&state, recipient, 1_000);
    (credential, state, funding)
}

// Independently check the actual Schnorr equations for fixture preflight. This test-only
// check does not replace the production mint circuit or authenticate a consensus epoch.
fn signature_equation<C: CurveAffine>(
    parity: u8,
    validator_index: u32,
    signing_digest: DigestV1,
    public_key: DigestV1,
    signature: &KagemushaPastaSchnorrSignatureV1,
) -> bool
where
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let point = |bytes: &DigestV1| {
        let mut repr = C::Repr::default();
        repr.as_mut().copy_from_slice(bytes);
        Option::<C>::from(C::from_bytes(&repr)).filter(|value| !bool::from(value.is_identity()))
    };
    let Some(public) = point(&public_key) else {
        return false;
    };
    let Some(nonce) = point(&signature.nonce_commitment) else {
        return false;
    };
    let Some(response) = decode_pasta::<C::ScalarExt>(signature.response) else {
        return false;
    };
    if bool::from(response.is_zero()) {
        return false;
    }
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:mint-finality:challenge");
    hash.update([0, parity]);
    hash.update(validator_index.to_le_bytes());
    hash.update(signing_digest);
    hash.update(signature.nonce_commitment);
    hash.update(public_key);
    let digest: DigestV1 = hash.finalize().into();
    let challenge = from_u128::<C::ScalarExt>(u128::from_le_bytes(
        digest[..16].try_into().expect("challenge half"),
    ));
    C::CurveExt::generator() * response
        == C::CurveExt::from(nonce) + C::CurveExt::from(public) * challenge
}

fn certificate_signature_equations(certificate: &KagemushaMintCertificateWitnessV1) -> bool {
    let digest = certificate
        .seal_bundle
        .message
        .signing_digest()
        .expect("fixture signing digest");
    certificate.seal_bundle.seals.iter().all(|seal| {
        let keys = &certificate.epoch_roster.validators
            [usize::try_from(seal.validator_index).expect("fixture signer")];
        signature_equation::<EpAffine>(
            0,
            seal.validator_index,
            digest,
            keys.eq_proof_public_key,
            &seal.eq_proof_signature,
        ) && signature_equation::<EqAffine>(
            1,
            seal.validator_index,
            digest,
            keys.ep_proof_public_key,
            &seal.ep_proof_signature,
        )
    })
}

#[test]
fn funding_certificate_preflight_has_exact_real_quorum_and_positive_membership() {
    let (_, state, funding) = funding_fixture();
    assert_eq!(
        state.balance, 0,
        "funding must not invent a positive bootstrap balance"
    );
    assert_eq!(funding.finalized.statement.amount, 1_000);
    assert_eq!(funding.finalized.epoch_roster.validators.len(), 4);
    assert_eq!(funding.finalized.seal_bundle.seals.len(), 3);
    assert!(
        certificate_signature_equations(&funding.finalized),
        "genuine paired-Pasta quorum signatures"
    );
    let mut substituted = funding.finalized.clone();
    substituted.membership.leaf.amount += 1;
    assert!(substituted.validate_shape().is_err());
    let mut substituted = funding.finalized.clone();
    substituted.seal_bundle.message.subject_digest = digest(b"substituted-finality-subject", 0);
    assert!(!certificate_signature_equations(&substituted));
}

#[test]
#[ignore = "expensive real mint-authority prerequisite; not payment-corridor qualification"]
fn real_mint_authority_bootstrap_and_positive_finalized_mint_use_reusable_keys() {
    std::thread::Builder::new()
        .name("kagemusha-real-mint".to_owned())
        .stack_size(512 * 1024 * 1024)
        .spawn(|| {
            let started = std::time::Instant::now();
            let (credential, _, funding) = funding_fixture();
            let eq = ParamsIPA::<EqAffine>::new(KAGEMUSHA_HALO2_K_V1);
            let ep = ParamsIPA::<EpAffine>::new(KAGEMUSHA_HALO2_K_V1);
            let seed = CredentialKeys::generate(&eq, &ep, &credential);
            let eq_seed = compile(
                &eq,
                seed.eq_proving_key.get_vk(),
                snark_verifier::system::halo2::Config::ipa()
                    .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
            );
            let ep_seed = compile(
                &ep,
                seed.ep_proving_key.get_vk(),
                snark_verifier::system::halo2::Config::ipa()
                    .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
            );
            let keys = MintKeys::generate(&eq, &ep, eq_seed, ep_seed, &funding);
            let funded = keys.prove_funding(&funding);
            keys.decide(&funded.proof, &funded.eq_history, &funded.ep_history);
            assert_eq!(funded.proof.authority_head, funding.genesis_roster_id);
            eprintln!(
                "KAGEMUSHA real positive funding prerequisite wall time: {:?}",
                started.elapsed()
            );
        })
        .expect("start explicitly sized real-proof stack")
        .join()
        .expect("real funding proof thread");
}
