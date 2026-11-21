//! Native authentication of the captured first Load plus Core30 state proofs.
//! Ordinary receipts for Bootstrap and Load sign the exact independent sigma.
use super::*;
use crate::{
    admin_sigma::{
        BootstrapCircuit, StateWitness,
        core30_initial::{InitialAdmin, initial_prior_root},
    },
    operation_relation::objects::ObjectKind,
};
use ff::{Field, PrimeField};
use iroha_pasta::msm::MemoryBudget;
use iroha_plonk::{
    keys::{KeygenConfigV2, keygen_pk_v2},
    pcs::ipa::PinnedParams,
    verify_full,
};
use iroha_plonk_gadgets::{bytes::p_bytes_native, statement::STATEMENT_DOMAIN};

fn signed(kind: ObjectKind, bytes: &[u8], key: &[u8]) -> bootstrap_objects::Signed {
    paired_request::verify_signed_original(kind, bytes, key).expect("original signed object")
}

fn prove_initial<const LOAD: bool>(
    step: &InitialAdmin<LOAD>,
    ordinary: Fp,
) -> (iroha_plonk::ProvingKey<Eq>, Vec<u8>) {
    let params = crate::common::vesta_params(12);
    let start = Instant::now();
    let key = keygen_pk_v2(
        &params,
        step,
        &KeygenConfigV2::pipa_r(BootstrapCircuit::instance_types().to_vec()),
    )
    .unwrap();
    let keygen = start.elapsed();
    let public = step.public();
    assert_ne!(ordinary, public[0][0]);
    let start = Instant::now();
    let proof = prove_circuit(
        &params,
        &key,
        step,
        &public,
        ProverRandomness::hedged(),
        ProverConfig::default(),
    )
    .unwrap();
    let proving = start.elapsed();
    let independent = PinnedParams::<Eq>::derive(12).unwrap();
    let start = Instant::now();
    verify_full(
        &independent,
        key.binding(),
        key.vk(),
        &public,
        &proof,
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    let verifying = start.elapsed();
    assert!(
        verify_full(
            &independent,
            key.binding(),
            key.vk(),
            &[vec![ordinary]],
            &proof,
            MemoryBudget::DEFAULT
        )
        .is_err()
    );
    println!(
        "CORE30_FIRST_{} k=12 proof_bytes={} keygen_ms={:.3} prove_ms={:.3} fresh_verify_ms={:.3}",
        if LOAD { "LOAD" } else { "BOOTSTRAP" },
        proof.len(),
        keygen.as_secs_f64() * 1e3,
        proving.as_secs_f64() * 1e3,
        verifying.as_secs_f64() * 1e3
    );
    (key, proof)
}

fn verify_advance(
    statement: &[Fp; 26],
    proof: &[u8],
    original: &bootstrap_objects::Signed,
    scheme: &[u8],
    wallet: &[u8],
    credential: &[u8],
) {
    let payment_key = &credential[130..195];
    let bytes = &original.bytes;
    signed(ObjectKind::Receipt, bytes, payment_key);
    assert_eq!(&bytes[..2], &1u16.to_le_bytes());
    assert_eq!(&bytes[2..34], scheme);
    assert_eq!(&bytes[34..66], wallet);
    assert_eq!(&bytes[66..98], &credential[195..227]);
    assert_eq!(&bytes[98..114], &statement[9].to_repr()[..16]);
    let mut operation = vec![
        Fp::from_u128(u128::from_le_bytes(wallet[..16].try_into().unwrap())),
        Fp::from_u128(u128::from_le_bytes(wallet[16..].try_into().unwrap())),
        statement[16],
        statement[17],
    ];
    if statement[16] == Fp::ONE {
        operation.push(statement[18]);
    }
    assert_eq!(
        &bytes[114..146],
        &hash_with_domain(u64::from_le_bytes(*b"kgwopid1"), &operation).to_repr()
    );
    assert!(bytes[274..306].iter().any(|byte| *byte != 0));
    assert_eq!(&bytes[146..178], &statement[14].to_repr());
    assert_eq!(&bytes[178..210], &statement[15].to_repr());
    assert_eq!(
        &bytes[210..242],
        &hash_with_domain(STATEMENT_DOMAIN, statement).to_repr()
    );
    let mut tape = u32::try_from(proof.len()).unwrap().to_le_bytes().to_vec();
    tape.extend(proof);
    assert_eq!(
        &bytes[242..274],
        &p_bytes_native::<Fp>(u64::from_le_bytes(*b"kgwstep1"), &tape).to_repr()
    );
    assert_eq!(&bytes[306..338], &[0u8; 32]);
}

#[test]
#[ignore = "authenticates captured first Load, proves two candidate administrative steps and consumes that funded head in a matched Send and Receive"]
fn committed_first_load_discharges_the_matching_send_predecessor() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    pool.install(|| prove_funded_pair(None));
}

struct RootedParents<'a> {
    payer: &'a crate::omega_boundary_tests::RootedBootstrapOmega,
    funded: &'a StateWitness,
    receiver: &'a crate::omega_boundary_tests::RootedBootstrapOmega,
}

fn prove_funded_pair(parents: Option<RootedParents<'_>>) {
    let rooted_enrollment = parents.as_ref().map(|p| &p.payer.source.state);
    let start = Instant::now();
    let capture = ordinary_load_receipt::first_capture();
    // Anchor the admission to the intended fixture genesis and exact event.
    use sha2::{Digest, Sha256};
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(capture.as_bytes())),
        [
            126, 149, 247, 176, 198, 79, 33, 249, 106, 37, 235, 185, 126, 56, 69, 172, 157, 55, 42,
            36, 244, 199, 244, 24, 178, 145, 108, 9, 236, 191, 6, 68
        ]
    );
    let raw = ordinary_load_receipt::verified_receipt(&capture);
    let finality = start.elapsed();
    let (mut boot, certificate, credential) = bootstrap_objects::enrollment();
    if let Some(authenticated) = rooted_enrollment {
        assert_eq!(boot.core, authenticated.core);
        assert_eq!(boot.rest, authenticated.rest);
        boot.lineage[17] = authenticated.lineage[17];
    }
    let certificate_body = ObjectKind::Certificate.body_len();
    let credential_body = ObjectKind::Credential.body_len();
    let issuer_root = bootstrap_objects::sec1(bootstrap_objects::key(23));
    let certificate = signed(ObjectKind::Certificate, &certificate.bytes, &issuer_root);
    assert_eq!(&certificate.bytes[..2], &1u16.to_le_bytes());
    assert_eq!(&certificate.bytes[2..34], &raw[2..34]);
    assert_eq!(certificate.bytes[34], 1); // Wallet eligibility issuer.
    assert_eq!(certificate_body, 108);
    let credential = signed(
        ObjectKind::Credential,
        &credential.bytes,
        &certificate.bytes[35..100],
    );
    assert_eq!(&credential.bytes[..2], &1u16.to_le_bytes());
    assert_eq!(&credential.bytes[2..98], &raw[2..98]);
    assert_eq!(
        &credential.bytes[credential_body - 32..credential_body],
        &certificate.digest().to_repr()
    );
    let payer_key = &credential.bytes[130..195];
    assert_eq!(
        &payer_key[..],
        &bootstrap_objects::sec1(bootstrap_objects::key(
            bootstrap_objects::Identity::Payer.secret()
        ))
    );
    assert_eq!(credential.digest(), boot.core[7]);

    let (mut funded, insertion) = load_objects::from_finalized_receipt(&boot, &raw);
    let root = initial_prior_root();
    let core29_boot = hash_with_domain(STATEMENT_DOMAIN, &boot.statement);
    let core29_load = hash_with_domain(STATEMENT_DOMAIN, &funded.statement);
    let origin = candidate_head(&send_objects::state(&StateWitness::from(&boot)), root);
    let funded_head = candidate_head(&send_objects::state(&funded.successor), root);
    boot.statement[15] = origin;
    boot.lineage[5] = origin;
    funded.predecessor.lineage[5] = origin;
    funded.successor.lineage[5] = funded_head;
    funded.statement[14] = origin;
    funded.statement[15] = funded_head;
    if let Some(authenticated) = rooted_enrollment {
        assert_eq!(boot.statement, authenticated.statement);
        assert_eq!(boot.lineage, authenticated.lineage);
    }
    assert_eq!(boot.statement[14], Fp::ZERO);
    assert_eq!(funded.predecessor.core, boot.core);
    assert_eq!(funded.predecessor.rest, boot.rest);
    assert_eq!(funded.predecessor.lineage, boot.lineage);
    assert_eq!(
        funded.statement[17],
        p_bytes_native(u64::from_le_bytes(*b"kgwolod1"), &raw)
    );
    assert_eq!(
        funded.statement[18..21],
        [Fp::ZERO, Fp::from(100), Fp::ZERO]
    );
    assert_eq!(funded.successor.core[8], Fp::from(100));
    assert_eq!(funded.successor.core[12], Fp::ONE);
    assert_eq!(insertion.slot, 1);
    // Authenticate the exact recovery record to both state roots. The
    // administrative proof binds the roots; the folded A owns the opening.
    use crate::operation_relation::map_effects::LOAD_DOMAIN;
    use crate::tree::{INDEXED_NODE_DOMAIN, IndexedLeaf, path_root};
    let key = Fp::from(2).pow_vartime([128]) + funded.statement[18];
    let value = hash_with_domain(
        LOAD_DOMAIN,
        &[
            funded.statement[18],
            funded.statement[17],
            funded.statement[19],
        ],
    );
    assert!(insertion.leaf.brackets(&key));
    let low_root = |leaf: IndexedLeaf<Fp>| {
        path_root(
            INDEXED_NODE_DOMAIN,
            leaf.hash(),
            u64::from(insertion.leaf_slot),
            &insertion.leaf_siblings,
        )
    };
    let slot_root = |word| {
        path_root(
            INDEXED_NODE_DOMAIN,
            word,
            u64::from(insertion.slot),
            &insertion.slot_siblings,
        )
    };
    assert_eq!(low_root(insertion.leaf), funded.predecessor.core[18]);
    assert_eq!(
        low_root(IndexedLeaf {
            next_key: key,
            ..insertion.leaf
        }),
        slot_root(Fp::ZERO)
    );
    assert_eq!(
        slot_root(
            IndexedLeaf {
                key,
                value,
                next_key: insertion.leaf.next_key
            }
            .hash()
        ),
        funded.successor.core[18]
    );
    let (_, bootstrap_sigma) = prove_initial(&InitialAdmin::bootstrap(&boot), core29_boot);
    let bootstrap_advance = bootstrap_objects::receipt(&boot, &bootstrap_sigma);
    verify_advance(
        &boot.statement,
        &bootstrap_sigma,
        &bootstrap_advance,
        &raw[2..34],
        &raw[66..98],
        &credential.bytes,
    );
    let (_, load_sigma) = prove_initial(&InitialAdmin::load(&funded), core29_load);
    let load_advance = load_objects::receipt(&funded, &load_sigma);
    verify_advance(
        &funded.statement,
        &load_sigma,
        &load_advance,
        &raw[2..34],
        &raw[66..98],
        &credential.bytes,
    );
    assert!(
        paired_request::verify_signed_original(
            ObjectKind::Receipt,
            &load_advance.bytes,
            &issuer_root
        )
        .is_none()
    );
    println!(
        "CORE30_FUNDED_PREDECESSOR amount=100 ordinal=0 charge=0 genesis_finality_event_ms={:.3} bootstrap_head={origin:?} load_head={funded_head:?} sigma_receipts_verified=2 candidate_recursive_omega=false",
        finality.as_secs_f64() * 1e3
    );
    let sender_before = parents.as_ref().map_or(&funded.successor, |p| p.funded);
    assert_eq!(sender_before.core, funded.successor.core);
    assert_eq!(sender_before.rest, funded.successor.rest);
    assert_eq!(sender_before.lineage, funded.successor.lineage);
    paired_request::check_matching_steps(
        &boot,
        sender_before,
        &credential,
        Some(funded_head),
        parents.as_ref().map(|p| (p.payer, p.receiver)),
    );
}

#[test]
#[ignore = "build and decide the real compact Core30 Bootstrap Omega, then fund its exact first Load and prove the connected steps"]
fn authenticated_bootstrap_omega_precedes_the_finalized_first_load() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    pool.install(prove_from_rooted_bootstrap);
}

pub(super) fn core30_bootstrap_source(
    digest: Fp,
    profile: crate::bootstrap_owner_fixture::SourceProfile,
    identity: crate::bootstrap_owner_fixture::BootstrapIdentity,
) -> crate::bootstrap_owner_fixture::AuthenticatedBootstrap {
    crate::bootstrap_owner_fixture::bootstrap_from_source(
        false,
        digest,
        profile,
        identity,
        [1, 2],
        [1, 2],
        crate::admin_sigma::core30_initial::open_initial_state,
        |initial| {
            let ordinary = hash_with_domain(STATEMENT_DOMAIN, &initial.statement);
            let head = candidate_head(
                &send_objects::state(&StateWitness::from(&*initial)),
                initial_prior_root(),
            );
            initial.lineage[5] = head;
            initial.statement[15] = head;
            prove_initial(&InitialAdmin::bootstrap(initial), ordinary)
        },
    )
    .expect("authenticated Core30 Bootstrap terminal")
}

pub(super) fn rooted_initial(
    identity: crate::bootstrap_owner_fixture::BootstrapIdentity,
) -> crate::omega_boundary_tests::RootedBootstrapOmega {
    crate::omega_boundary_tests::rooted_compact_bootstrap_from_source(|digest, profile| {
        core30_bootstrap_source(digest, profile, identity)
    })
}

pub(super) fn core30_first_load_source(raw: [u8; 282]) -> load_chain::LoadFixture {
    let originals = std::env::var_os("KAGEMUSHA_CORE30_LOAD_ORIGINALS")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "kagemusha-core30-first-load-{}",
                std::process::id()
            ))
        });
    std::sync::Arc::new(move |rooted| {
        ordinary_load_fixture::build_with_source(
            rooted,
            &raw,
            &originals,
            |mut p| {
                p.initial_core30 = true;
                p
            },
            |state| {
                let ordinary = hash_with_domain(STATEMENT_DOMAIN, &state.statement);
                let head =
                    candidate_head(&send_objects::state(&state.successor), initial_prior_root());
                state.successor.lineage[5] = head;
                state.statement[15] = head;
                prove_initial(&InitialAdmin::load(state), ordinary).1
            },
            |initial| {
                let head = candidate_head(
                    &send_objects::state(&StateWitness::from(&*initial)),
                    initial_prior_root(),
                );
                initial.lineage[5] = head;
                initial.statement[15] = head;
            },
        )
    })
}

fn prove_from_rooted_bootstrap() {
    use crate::bootstrap_owner_fixture::BootstrapIdentity;
    let start = Instant::now();
    let rooted = rooted_initial(BootstrapIdentity::Payer);
    let building = start.elapsed();
    let independent = PinnedParams::<iroha_pasta::Ep>::derive(16).unwrap();
    let start = Instant::now();
    verify_full(
        &independent,
        &rooted.binding,
        &rooted.key,
        &rooted.instances,
        &rooted.proof,
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    let verifying = start.elapsed();
    assert_eq!(
        rooted.source.state.lineage[17],
        rooted.key.kagemusha_digest(&rooted.binding).unwrap()
    );
    let mut changed = rooted.instances.clone();
    changed[0][0] += iroha_pasta::Fq::ONE;
    assert!(
        verify_full(
            &independent,
            &rooted.binding,
            &rooted.key,
            &changed,
            &rooted.proof,
            MemoryBudget::DEFAULT
        )
        .is_err()
    );
    println!(
        "CORE30_ROOTED_BOOTSTRAP outer_bytes={} build_source_and_outer_ms={:.3} fresh_outer_verify_ms={:.3} load_outer=false",
        rooted.proof.len(),
        building.as_secs_f64() * 1e3,
        verifying.as_secs_f64() * 1e3
    );
    let raw = ordinary_load_receipt::verified_receipt(&ordinary_load_receipt::first_capture());
    let source = core30_first_load_source(raw);
    let start = Instant::now();
    let funded = load_chain::authenticated_load(&rooted, &source);
    let built = start.elapsed();
    let independent = PinnedParams::<Eq>::derive(16).unwrap();
    let start = Instant::now();
    verify_full(
        &independent,
        &funded.binding,
        &funded.key,
        &[funded.instances.clone()],
        &funded.proof,
        MemoryBudget::DEFAULT,
    )
    .unwrap();
    let verified = start.elapsed();
    assert_eq!(funded.receipt, raw);
    assert_eq!(funded.state.core[8], Fp::from(100));
    assert_eq!(
        funded.state.lineage[5],
        candidate_head(&send_objects::state(&funded.state), initial_prior_root())
    );
    let (mut expected, _) = load_objects::from_finalized_receipt(&rooted.source.state, &raw);
    expected.successor.lineage[5] = candidate_head(
        &send_objects::state(&expected.successor),
        initial_prior_root(),
    );
    assert_eq!(expected.successor.core, funded.state.core);
    assert_eq!(expected.successor.rest, funded.state.rest);
    assert_eq!(expected.successor.lineage, funded.state.lineage);
    println!(
        "CORE30_AUTHENTICATED_FIRST_LOAD terminal_bytes={} terminal_build_replay_mutation_tests_ms={:.3} fresh_terminal_verify_ms={:.3} funded_head={:?} predecessor_omega=true load_outer=false",
        funded.proof.len(),
        built.as_secs_f64() * 1e3,
        verified.as_secs_f64() * 1e3,
        funded.state.lineage[5]
    );
    let receiver = rooted_initial(BootstrapIdentity::Receiver);
    assert_eq!(receiver.binding, rooted.binding);
    assert_eq!(receiver.key.to_bytes(), rooted.key.to_bytes());
    prove_funded_pair(Some(RootedParents {
        payer: &rooted,
        funded: &funded.state,
        receiver: &receiver,
    }));
}
