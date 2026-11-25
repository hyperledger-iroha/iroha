//! Native incoming payer Omega for an ordinary Core30 Payment.
use super::*;
use crate::{
    admin_sigma::core30_initial::initial_prior_root,
    bootstrap_owner_fixture::{BootstrapIdentity, SourceProfile},
    omega::{OmegaPlan, native},
    omega_boundary_tests::RootedBootstrapOmega,
    operation_relation::objects::ObjectKind,
};
use ff::{Field, PrimeField};
use iroha_pasta::{Ep, msm::MemoryBudget};
use iroha_plonk::{
    keys::{CosetCachePolicy, KeygenConfigV2, keygen_pk_v2, pk::artifact::ReadConfig},
    pcs::ipa::PinnedParams,
    verify_full,
};
use iroha_plonk_gadgets::{bytes::p_bytes_native, statement::STATEMENT_DOMAIN};
use iroha_plonk_recursion::AccumulatorT;

include!("../tests/common/receive_payment_originals.rs");

#[path = "core30_unfolded_credit_test.rs"]
mod unfolded_credit;

/// Fix both actual terminal keys before rebuilding any wallet's signed objects.
fn common_owner(
    root: &RootedBootstrapOmega,
    load: &load_chain::AuthenticatedLoad,
) -> native::Prover {
    assert_eq!(root.source.binding, load.binding);
    assert_ne!(root.source.key.to_bytes(), load.key.to_bytes());
    let pallas = PinnedParams::<Ep>::derive(16).unwrap();
    let program = native::Program::for_compiled_catalog(
        root.source.binding.encoded(),
        &[
            root.source.key.to_bytes().to_vec(),
            load.key.to_bytes().to_vec(),
        ],
        PinnedParams::<Eq>::derive(16).unwrap(),
        pallas.clone(),
    )
    .unwrap();
    let mut config = KeygenConfigV2::pipa_r(OmegaPlan::instance_types().to_vec());
    config.compress_selectors = false;
    let key = keygen_pk_v2(&pallas, &program.source_circuit().unwrap(), &config).unwrap();
    assert_eq!(key.binding(), &root.binding);
    assert_ne!(key.vk().to_bytes(), root.key.to_bytes());
    let original = key.artifact_bytes_v2().unwrap();
    native::Prover::from_original_artifact(
        program,
        key.binding().encoded(),
        key.vk().to_bytes(),
        &original,
        ReadConfig {
            maximum_bytes: original.len(),
            maximum_rows: 1 << 16,
            coset_cache: CosetCachePolicy::OnDemand,
            msm_budget: MemoryBudget::DEFAULT,
        },
    )
    .unwrap()
}

fn prove_outer(owner: &native::Prover, input: native::Input) -> native::Output {
    let source = input.public;
    let original_pallas = input.pallas.clone();
    let output = owner
        .prepare(
            input,
            crate::common::public_fixture_tape(141),
            MemoryBudget::DEFAULT,
        )
        .unwrap()
        .prove(ProverRandomness::hedged(), ProverConfig::default())
        .unwrap();
    assert_eq!(output.pallas, original_pallas);
    let digest = crate::bootstrap_owner_fixture::digest(&source, &original_pallas.as_input());
    assert_eq!(
        output.instances,
        crate::fold_wrap::native::omega_instances(digest, &output.vesta)
    );
    verify_full(
        &PinnedParams::<Ep>::derive(16).unwrap(),
        owner.binding(),
        owner.verifying_key(),
        &output.instances,
        &output.proof,
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    output
}

fn rebind_enrollment(
    owner: &native::Prover,
    seed: &RootedBootstrapOmega,
    identity: BootstrapIdentity,
) -> RootedBootstrapOmega {
    let digest = owner
        .verifying_key()
        .kagemusha_digest(owner.binding())
        .unwrap();
    let source = funded_predecessor::core30_bootstrap_source(
        digest,
        SourceProfile::Tagged { buses: 3 },
        identity,
    );
    assert_eq!(source.key.to_bytes(), seed.source.key.to_bytes());
    assert_eq!(source.binding, seed.source.binding);
    assert_eq!(source.state.lineage[17], digest);
    let output = prove_outer(
        owner,
        native::Input {
            key: source.key.clone(),
            proof: source.proof.clone(),
            frame: source.instances.as_slice().try_into().unwrap(),
            public: source.state.lineage,
            pallas: source.pallas.clone(),
        },
    );
    RootedBootstrapOmega {
        source,
        key: owner.verifying_key().clone(),
        binding: owner.binding().clone(),
        proof: output.proof,
        instances: output.instances,
        opening: output.opening,
        vesta: output.vesta,
    }
}

/// A Payment carries the sender's *pre-Send* Omega beside the genuine Send sigma.
#[test]
#[ignore = "rebinds signed Core30 Bootstrap and finalized first Load under their common fixture catalog and authenticates the payer Omega carried by the Payment"]
fn funded_payer_omega_is_the_signed_incoming_payment_predecessor() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap()
        .install(prove_incoming_payment);
}

fn prove_incoming_payment() {
    let raw = ordinary_load_receipt::verified_receipt(&ordinary_load_receipt::first_capture());
    let fixture = funded_predecessor::core30_first_load_source(raw);
    let seed = funded_predecessor::rooted_initial(BootstrapIdentity::Payer);
    let initial = load_chain::authenticated_load(&seed, &fixture);
    let owner = common_owner(&seed, &initial);
    let payer = rebind_enrollment(&owner, &seed, BootstrapIdentity::Payer);
    let funded = load_chain::authenticated_load(&payer, &fixture);
    assert_eq!(funded.key.to_bytes(), initial.key.to_bytes());
    assert_eq!(funded.binding, initial.binding);
    assert_eq!(funded.receipt, raw);
    assert_eq!(funded.state.core[8], Fp::from(100));
    assert_eq!(funded.state.lineage[17], payer.source.state.lineage[17]);
    let incoming = prove_outer(
        &owner,
        native::Input {
            key: funded.key,
            proof: funded.proof,
            frame: funded.instances.try_into().unwrap(),
            public: funded.state.lineage,
            pallas: funded.pallas,
        },
    );
    let receiver = rebind_enrollment(&owner, &seed, BootstrapIdentity::Receiver);
    let signed = send_objects::from_load(&funded.state);
    let before = &funded.state.lineage;
    let root = initial_prior_root();
    assert_eq!(before[5], candidate_head(&signed.step.predecessor, root));
    let mut step = signed.step.clone();
    step.core30_prior_root = Some(root);
    let debit = step.evaluate(SigmaRelation::SEND);
    assert!(debit.is_honest());
    assert_eq!(debit.statement[14], before[5]);
    assert_eq!(debit.statement[18..20], receiver.source.state.lineage[6..8]);
    let (_, _, receiver_credential) = receive_objects::receiver();
    let request = paired_request::verify_signed_original(
        ObjectKind::Request,
        &signed.objects[1],
        &receiver_credential.bytes[130..195],
    )
    .unwrap();
    assert_eq!(request.digest(), debit.statement[23]);
    let (verifier, sigma, _) = prove_candidate_step(SigmaRelation::SEND, signed.step.clone(), root);
    assert_eq!(sigma.public, debit.public());
    verifier.verify(&sigma.public, &sigma.bytes).unwrap();

    let transport = incoming.transport();
    assert_eq!(transport.len(), 5120);
    let (proof, claims) = transport.split_at(incoming.proof.len());
    let (p, v) = claims.split_at(iroha_plonk_recursion::ACCUMULATOR_BYTES);
    let pallas = AccumulatorT::<Ep>::from_bytes(p).unwrap();
    let vesta = AccumulatorT::<Eq>::from_bytes(v).unwrap();
    let pp = PinnedParams::<Ep>::derive(16).unwrap();
    pallas.decide(&pp, MemoryBudget::DEFAULT).unwrap();
    vesta
        .decide(
            &PinnedParams::<Eq>::derive(16).unwrap(),
            MemoryBudget::DEFAULT,
        )
        .unwrap();
    let instances = crate::fold_wrap::native::omega_instances(
        crate::bootstrap_owner_fixture::digest(before, &pallas.as_input()),
        &vesta,
    );
    assert_eq!(instances, incoming.instances);
    let accepts = |bytes: &[u8]| {
        verify_full(
            &pp,
            owner.binding(),
            owner.verifying_key(),
            &instances,
            bytes,
            MemoryBudget::DEFAULT,
        )
        .is_ok()
    };
    assert!(accepts(proof));
    assert!(!accepts(&receiver.proof));
    assert!(!accepts(&payer.proof));

    let mut omega = public_bytes(before);
    omega.extend(&transport);
    crate::a_relation::native::receive::check_payment_original_sizes(
        omega.len(),
        sigma.bytes.len(),
    )
    .unwrap();
    let proof_digest = |omega: &[u8]| {
        p_bytes_native::<Fp>(
            u64::from_le_bytes(*b"kgwprf_1"),
            &[frame(omega), frame(&sigma.bytes)].concat(),
        )
    };
    let digest = proof_digest(&omega);
    let receipt = receipt(&debit.statement, &before[6..8], digest, Fp::ZERO, 29, 71);
    let payer_key = &signed.objects[0][130..195];
    let checked =
        paired_request::verify_signed_original(ObjectKind::Receipt, &receipt.bytes, payer_key)
            .unwrap();
    assert_eq!(&checked.bytes[146..178], &before[5].to_repr());
    assert_eq!(&checked.bytes[178..210], &debit.statement[15].to_repr());
    assert_eq!(
        &checked.bytes[210..242],
        &hash_with_domain(STATEMENT_DOMAIN, &debit.statement).to_repr()
    );
    assert_eq!(&checked.bytes[242..274], &digest.to_repr());
    let payment = payment_transcript(
        &signed.objects[1],
        &signed.objects[0],
        &debit.statement,
        digest,
        checked.digest(),
    );
    if let Some(dir) = std::env::var_os("KAGEMUSHA_CORE30_UNFOLDED_OUTPUT") {
        let dir = std::path::PathBuf::from(dir);
        let statement: Vec<u8> = debit
            .statement
            .iter()
            .flat_map(PrimeField::to_repr)
            .collect();
        for (name, original) in [
            ("first-load-receipt-transcript.bin", raw.as_slice()),
            ("pre-send-omega.bin", &omega),
            ("omega.vk", owner.verifying_key().to_bytes()),
            ("omega.descriptor", owner.binding().encoded()),
            ("send-statement-fields.bin", &statement),
            ("send.sigma", &sigma.bytes),
            ("send.vk", verifier.vk_bytes()),
            ("send.descriptor", verifier.descriptor_bytes()),
            ("payer-credential.bin", &signed.objects[0]),
            ("payer-receipt.bin", &checked.bytes),
        ] {
            std::fs::write(dir.join(name), original).unwrap();
        }
    }
    unfolded_credit::verify_receiver(
        &signed,
        &receiver.source.state,
        &debit.statement,
        &request,
        &receiver_credential,
        &payment,
        root,
        receiver.source.state.lineage[5],
    );
    let mut substituted = public_bytes(&receiver.source.state.lineage);
    substituted.extend(&receiver.proof);
    substituted.extend(receiver.source.pallas.to_bytes());
    substituted.extend(receiver.vesta.to_bytes());
    let wrong = proof_digest(&substituted);
    assert_ne!(wrong, digest);
    assert_ne!(
        payment_transcript(
            &signed.objects[1],
            &signed.objects[0],
            &debit.statement,
            wrong,
            checked.digest()
        ),
        payment
    );
    println!(
        "CORE30_INCOMING_PAYER_OMEGA catalog_terminals=2 payer_balance=100 amount=12 credit={:?} payer_head={:?} omega_proof_bytes={} omega_transport_bytes={} sigma_bytes={} payment_transcript_bytes={} recipient_omega_refused=true unfunded_omega_refused=true substituted_package_refused=true",
        debit.statement[17],
        before[5],
        proof.len(),
        transport.len(),
        sigma.bytes.len(),
        payment.len()
    );
}
