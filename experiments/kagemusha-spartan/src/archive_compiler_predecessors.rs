//! The first five-coordinate pass uses the retained, fully signed Send and Receive
//! and the compiler's complete empty claims. The seeds stay outside installation.
use super::*;
use crate::{
    full::backend_chain_tests::{self, CompilerGroup},
    r1cs::Circuit,
};
use std::{path::Path, sync::Arc};

pub(super) struct Predecessors {
    pub ordinary: backend_chain_tests::Generation,
    pub chain: crate::chain_fixture::Chain,
    pub generators: full::PairGens,
    pub payer: full::UniformPair,
    pub receiver: full::UniformPair,
    pub receiver_programs: [[u8; 32]; 2],
    pub primary: [Arc<UniformCircuit<Fp>>; 5],
    pub secondary: [Arc<UniformCircuit<Fq>>; 5],
}

fn seed<'a>(
    prepared: &PreparedOperation,
    completed: &CompletedOperation,
    exact: &'a native::ExactOperationWitness,
    engine: &NativeEngine,
    params: &native::AuthenticatedParameters,
) -> backend_chain_tests::ArchiveTransferSeed<'a> {
    let (operation, incoming) = match exact {
        native::ExactOperationWitness::Send(step) => (full::OperationWitness::Send(step), None),
        native::ExactOperationWitness::Receive {
            step,
            incoming_payment,
        } => {
            let payment = engine.verify_payment(incoming_payment, params).unwrap();
            let digest = crate::payment::proof_digest(payment.exact_pair()).unwrap();
            (
                full::OperationWitness::Receive(step),
                Some((*payment.profile(), digest)),
            )
        }
        _ => panic!("expected a signed retained transfer"),
    };
    // The signed source verifier checks the exact pair and public inputs first.
    // Its own IPA challenges are nonempty even when its predecessor is Bootstrap.
    let original = native::engine::decode_head(completed.head().exact_pair(), params)
        .unwrap()
        .4;
    assert_eq!(original.kind, operation.kind());
    let trace = full::trace_uniform(original, &params.inner.generators);
    let primary_opening = crate::ipa::Claim {
        g: trace.proof_p.ipa.g_final,
        us: trace.tr_p.us.clone(),
    };
    let secondary_opening = crate::ipa::Claim {
        g: trace.proof_s.ipa.g_final,
        us: trace.tr_s.us.clone(),
    };
    assert!(crate::generator_claim::decide(
        &params.inner.generators.p,
        &primary_opening
    ));
    assert!(crate::generator_claim::decide(
        &params.inner.generators.s,
        &secondary_opening
    ));
    backend_chain_tests::ArchiveTransferSeed {
        primary_opening,
        secondary_opening,
        predecessor: *prepared.predecessor.profile(),
        incoming,
        operation,
    }
}

fn transfer(
    prepared: &PreparedOperation,
    completed: &CompletedOperation,
    engine: &NativeEngine,
    params: &native::AuthenticatedParameters,
    generators: &full::PairGens,
    layout: (crate::uniform::Layout, crate::uniform::Layout),
    group: CompilerGroup,
) -> full::UniformPair {
    let exact = compiler_history::signed(prepared, completed);
    let built = backend_chain_tests::archive_seed_transfer(
        generators,
        layout,
        seed(prepared, completed, &exact, engine, params),
        group,
    );
    assert_eq!(built.ip.io.as_slice(), completed.head().profile());
    built
}

fn receiver_originals(
    wallet: &Path,
    engine: &NativeEngine,
    params: &native::AuthenticatedParameters,
) -> (PreparedOperation, CompletedOperation) {
    let scheme = native::device_fixture::scheme().scheme_id();
    let capsule = native::RecoveryCapsule::decode(
        &std::fs::read(wallet.join("counterparty-receive-capsule.norito")).unwrap(),
        &scheme,
    )
    .unwrap();
    let completion = native::CompletionRecord::decode(
        &std::fs::read(wallet.join("counterparty-receive-completion.norito")).unwrap(),
        capsule.wallet(),
    )
    .unwrap();
    engine
        .verify_released_operation(&capsule, &completion, params)
        .unwrap()
}

pub(super) fn seed_requirements(
    chain: &crate::chain_fixture::Chain,
    send: &PreparedOperation,
    sent: &CompletedOperation,
    engine: &NativeEngine,
    params: &native::AuthenticatedParameters,
    columns: [usize; 2],
    group: CompilerGroup,
) -> (
    Vec<crate::bootstrap_layout::Requirements>,
    Vec<crate::bootstrap_layout::Requirements>,
) {
    let wallet =
        std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ROOT").unwrap());
    let (receive, received) = receiver_originals(&wallet, engine, params);
    let a = compiler_history::signed(send, sent);
    let b = compiler_history::signed(&receive, &received);
    backend_chain_tests::Generation::archive_seed_requirements(
        chain,
        columns,
        group,
        seed(send, sent, &a, engine, params),
        seed(&receive, &received, &b, engine, params),
    )
}

type ArchiveTapes = (Arc<UniformCircuit<Fp>>, Arc<UniformCircuit<Fq>>);

fn tapes(previous: &Path, group: CompilerGroup) -> ArchiveTapes {
    let archive_p = Arc::new(
        group
            .import::<Fp>(
                &std::fs::read(previous.join("archive-primary.program")).unwrap(),
                true,
            )
            .unwrap(),
    );
    let archive_s = Arc::new(
        group
            .import::<Fq>(
                &std::fs::read(previous.join("archive-secondary.program")).unwrap(),
                false,
            )
            .unwrap(),
    );
    (archive_p, archive_s)
}

pub(super) fn qualify_seed_geometry(
    p: &UniformCircuit<Fp>,
    s: &UniformCircuit<Fq>,
    chain: &crate::chain_fixture::Chain,
    params: &native::AuthenticatedParameters,
) -> full::PairGens {
    let (gens, p, s) = backend_chain_tests::archive_seed_bootstrap_programs(p, s, chain);
    let (x, y) = native::PRODUCER_MATRIX_DIMENSIONS;
    for (slots, words) in [
        (p.late_slots(), 3 + 3 + x.0 + x.1 + 1),
        (s.late_slots(), 3 + 3 + y.0 + y.1 + 1),
    ] {
        assert!(
            (full::LATE_IO..full::LATE_IO + words).all(|i| slots.contains(&i)),
            "complete five-coordinate matrix claim"
        );
    }
    eprintln!(
        "authentic five-coordinate Bootstrap geometry P {:?} {:?} S {:?} {:?}",
        p.shape,
        p.layout(),
        s.shape,
        s.layout()
    );
    compiler::qualify_layout(params, &gens, &p, &s);
    gens
}

#[test]
#[ignore = "qualifies the exact proposed target using both genuine five-coordinate Bootstrap programs and stock wire codecs"]
fn qualify_five_coordinate_seed_geometry() {
    let prior =
        std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_ARCHIVE_PREVIOUS").unwrap());
    let (p, s) = tapes(&prior, CompilerGroup::Three);
    with_retained_inputs(|_, send, sent, engine, params, _| {
        let chain = compiler_history::chain(send, sent, engine, params);
        qualify_seed_geometry(&p, &s, &chain, params);
    });
}

pub(super) fn build(
    payer_inputs: &Inputs<'_>,
    engine: &NativeEngine,
    params: &native::AuthenticatedParameters,
    wallet: &Path,
    previous: &Path,
    group: CompilerGroup,
) -> Predecessors {
    let (send, sent) = (payer_inputs.current, payer_inputs.released);
    let (archive_p, archive_s) = tapes(previous, group);
    let layout = (archive_p.layout(), archive_s.layout());
    let chain = compiler_history::chain(send, sent, engine, params);
    let empty = qualify_seed_geometry(&archive_p, &archive_s, &chain, params);
    let payer = transfer(send, sent, engine, params, &empty, layout, group);
    assert_eq!(payer.kind, full::SEND);
    let payment = compiler_receiver::new_payment(
        payer_inputs,
        params,
        &chain.send,
        &payer,
        &empty,
        [payer.cp.program_digest(), payer.cs.program_digest()],
    )
    .expect("decided first Send seed must encode its real Payment on the retail transport");
    eprintln!(
        "first Send seed actual {:?}, raw proof pair {} B, Payment {:?}",
        payment.size,
        payment.pair_bytes.len(),
        blake3::Hash::from(payment.digest.to_repr())
    );
    let (boot, receiver_boot, control) =
        backend_chain_tests::archive_seed_family(&archive_p, &archive_s, &chain, &empty);
    let (prepare, release) = receiver_originals(wallet, engine, params);
    let receiver = transfer(&prepare, &release, engine, params, &empty, layout, group);
    assert_eq!(receiver.kind, full::RECEIVE);
    let primary = [
        receiver.cp.clone(),
        payer.cp.clone(),
        boot.cp.clone(),
        control.cp.clone(),
        archive_p,
    ];
    let secondary = [
        receiver.cs.clone(),
        payer.cs.clone(),
        boot.cs.clone(),
        control.cs.clone(),
        archive_s,
    ];
    let receiver_programs = [
        primary[full::RECEIVE].program_digest(),
        secondary[full::RECEIVE].program_digest(),
    ];
    let generators = full::next_gens_with_archive_programs_at(
        &receiver,
        &payer,
        &boot,
        &control,
        &primary[4],
        &secondary[4],
        native::PRODUCER_MATRIX_DIMENSIONS,
    );
    assert_eq!(
        (
            generators.defer_p.selector_bits(),
            generators.defer_s.selector_bits()
        ),
        (3, 3)
    );
    for pair in [&payer, &receiver, &boot, &control] {
        native::engine::decide_accumulators(&generators, pair).unwrap();
    }
    let ordinary = backend_chain_tests::Generation::transfer_seeds(
        [receiver.clone(), payer.clone(), boot, control],
        receiver_boot,
    );
    Predecessors {
        ordinary,
        chain,
        generators,
        payer,
        receiver,
        receiver_programs,
        primary,
        secondary,
    }
}
