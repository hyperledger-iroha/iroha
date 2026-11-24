//! Compile the five real wallet operations until the full serialized verifier
//! originals agree across successive recursive generations. A final sequential
//! wallet chain reuses that one generation and requires every native decision.
use super::*;
use crate::{
    chain_fixture::Chain,
    full::backend_chain_tests::{CompiledTape, Generation},
    native::{PRODUCER_MATRIX_DIMENSIONS, ProgramBundleV2},
    receive,
};
use compiler_receiver::{CompilerPayment, ScopedStatus};
use std::{path::Path, sync::Arc};

type Circuits = (Arc<UniformCircuit<Fp>>, Arc<UniformCircuit<Fq>>);
type Layouts = (Layout, Layout);
type History = (CompilerPayment, receive::Witness);

fn pins(pair: &full::UniformPair) -> [[u8; 32]; 2] {
    [pair.cp.program_digest(), pair.cs.program_digest()]
}

fn generators(ordinary: &Generation, archive: &Circuits) -> full::PairGens {
    let [receive, send, bootstrap, control] = &ordinary.kinds;
    full::next_gens_with_archive_programs_at(
        receive,
        send,
        bootstrap,
        control,
        &archive.0,
        &archive.1,
        PRODUCER_MATRIX_DIMENSIONS,
    )
}

fn installed_dimension_words() -> [[u32; 2]; 2] {
    let (p, s) = PRODUCER_MATRIX_DIMENSIONS;
    [[p.0 as u32, p.1 as u32], [s.0 as u32, s.1 as u32]]
}

/// Preserve compiler pins only after the independently encoded fixed point passes
/// the strict tape reader and both retail transports. The document is the existing
/// public inspection input accepted by kagemusha_wallet_artifacts.
fn inspected_profile(
    group: CompilerGroup,
    primary: [[u8; 32]; 5],
    secondary: [[u8; 32]; 5],
    bundle: &[u8],
) -> Vec<u8> {
    let inspect = match group {
        CompilerGroup::Three => ProgramBundleV2::inspect_retail_originals,
        CompilerGroup::Four => ProgramBundleV2::inspect_four_variable_retail_originals,
    };
    let dimensions = installed_dimension_words();
    let (geometry, sizes) = inspect(bundle, &primary, &secondary, dimensions)
        .expect("all ten independently pinned fixed-point originals fit the retail profile");
    eprintln!("fixed-point program geometry {geometry:?}, retail {sizes:?}");
    let hex = |pin: [u8; 32]| blake3::Hash::from(pin).to_hex().to_string();
    let primary = primary.map(hex).to_vec();
    let secondary = secondary.map(hex).to_vec();
    let [[px, py], [sx, sy]] = dimensions;
    let json = norito::json!({
        "schema": "iroha.kagemusha.spartan.public-program-inspection.v2",
        "primary": primary,
        "secondary": secondary,
        "primary_dimensions": {"x": px, "y": py},
        "secondary_dimensions": {"x": sx, "y": sy}
    });
    let mut bytes = norito::json::to_vec_pretty(&json).unwrap();
    bytes.push(b'\n');
    bytes
}

fn all_programs(ordinary: &Generation, archive: &Circuits) -> ProgramBundleV2 {
    let p = |k: usize| {
        if k == full::ARCHIVE_SENT {
            &archive.0
        } else {
            &ordinary.kinds[k].cp
        }
    };
    let s = |k: usize| {
        if k == full::ARCHIVE_SENT {
            &archive.1
        } else {
            &ordinary.kinds[k].cs
        }
    };
    ProgramBundleV2 {
        version: 2,
        primary: std::array::from_fn(|k| p(k).export_program().unwrap()),
        secondary: std::array::from_fn(|k| s(k).export_program().unwrap()),
    }
}

fn preflight(
    ordinary: &Generation,
    chain: &Chain,
    gens: &full::PairGens,
    target: Layouts,
    archive: &Circuits,
    record: &Path,
) -> ProgramBundleV2 {
    let group = ordinary.compiler_group();
    let (p, s) = ordinary.preflight_ordinary(chain, gens, target);
    let complete = |ordinary: [CompiledTape; 4], archive: CompiledTape| {
        let [receive, send, bootstrap, control] = ordinary;
        let tapes = [receive, send, bootstrap, control, archive];
        (tapes.each_ref().map(|t| t.pin), tapes.map(|t| t.bytes))
    };
    let (p, primary) = complete(p, CompiledTape::capture(&archive.0));
    let (s, secondary) = complete(s, CompiledTape::capture(&archive.1));
    let programs = ProgramBundleV2 {
        version: 2,
        primary,
        secondary,
    };
    eprintln!(
        "five-kind ten original tape bytes P {:?} S {:?}",
        programs.primary.each_ref().map(Vec::len),
        programs.secondary.each_ref().map(Vec::len)
    );
    let bytes = programs
        .encode()
        .expect("ten exact originals must fit the governed public carrier before proving");
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(record)
        .and_then(|mut f| std::io::Write::write_all(&mut f, &bytes))
        .expect("reserve an unqualified preflight original");
    let inspect = match group {
        CompilerGroup::Three => ProgramBundleV2::inspect_retail_originals,
        CompilerGroup::Four => ProgramBundleV2::inspect_four_variable_retail_originals,
    };
    let (geometry, transport) = inspect(&bytes, &p, &s, installed_dimension_words()).expect(
        "the exact ten tapes must fit normal Send and both maximum-depth CreditStatus messages",
    );
    eprintln!(
        "five-kind original bundle {} B, geometry {geometry:?}, transport {transport:?}",
        bytes.len()
    );
    programs
}

struct Round {
    pair: full::UniformPair,
    target: Layouts,
    tapes: ProgramBundleV2,
}

fn archive_round(
    payer: &Inputs<'_>,
    ordinary: &Generation,
    chain: &Chain,
    gens: &full::PairGens,
    status: &ScopedStatus,
    fixed: Option<Layouts>,
    preflight_record: &Path,
) -> Round {
    let group = ordinary.compiler_group();
    let slots = [
        full::trace_uniform(ordinary.kinds[full::SEND].clone(), gens),
        full::trace_incoming_candidate(
            ordinary.kinds[full::RECEIVE].clone(),
            gens,
            status.exact_pair().to_vec(),
        ),
    ];
    let folds =
        full::GeneratorFolds::prepare(gens, &slots, &ProofEntropy::fresh().unwrap()).unwrap();
    let (removal, noop) = cases(payer);
    let build = |case: &Case, target| {
        let receipt = compiler_receipt(payer, case, status.payment, status.status_digest);
        let lambda = primary_at_group(
            payer,
            gens,
            &slots,
            &folds,
            case,
            &status.receipt,
            Projection::Completed {
                credential: payer.current.credential(),
                receipt: &receipt,
            },
            target,
            group,
        );
        (lambda, crate::receipt::digest(&receipt))
    };
    let (natural, _) = build(&removal, None);
    let derived = natural.derived;
    let applied = derived == [Fp::ONE; 4];
    let case = if applied { &removal } else { &noop };
    if fixed.is_some() {
        assert!(
            applied,
            "a final ArchiveSent must remove the retained pending descriptor"
        );
    }
    let req_p = Requirements::of(&natural.p.0);
    let cap_p = natural.p.0.max_exported_fixed_rows();
    drop(natural);
    let natural_s = compiler_secondary_group(gens, &slots, &folds, None, group);
    let req_s = Requirements::of(&natural_s.0);
    let cap_s = natural_s.0.max_exported_fixed_rows();
    drop(natural_s);
    let target = fixed
        .unwrap_or_else(|| ordinary.archive_targets(chain, gens, [req_p, req_s], [cap_p, cap_s]));
    eprintln!(
        "five-kind true ArchiveSent branch {:?}, common layout {target:?}",
        derived.map(|f| f == Fp::ONE)
    );
    let (lambda, receipt) = build(case, Some(target.0));
    assert_eq!(lambda.derived, derived);
    let p = lambda.p;
    let s = compiler_secondary_group(gens, &slots, &folds, Some(target.1), group);
    let mut public = verifier_io(payer, case);
    public.truncate(64);
    let fields = public_payment_fields(payer, case, status.payment, status.status_digest);
    assert_eq!(fields[18], status.credited);
    public[full::STATEMENT_IO] = crate::statement::digest(&fields);
    public[full::RECEIPT_IO] = receipt;
    public[full::TABLE_IO..full::TABLE_IO + 4]
        .copy_from_slice(&payer.released.head().profile()[full::TABLE_IO..full::TABLE_IO + 4]);
    public[full::KIND_C2_IO] = Fp::ONE;
    let mut si = verifier_s_io();
    si[full::KIND_C2_IO] = Fq::ONE;
    assert_eq!(p.1.io, public);
    assert_eq!(s.1.io, si);
    let cp = Arc::new(
        group
            .import::<Fp>(&p.0.export_program().unwrap(), true)
            .unwrap(),
    );
    let cs = Arc::new(
        group
            .import::<Fq>(&s.0.export_program().unwrap(), false)
            .unwrap(),
    );
    assert_eq!(cp.program_digest(), p.0.program_digest());
    assert_eq!(cs.program_digest(), s.0.program_digest());
    let originals = (cp.clone(), cs.clone());
    let tapes = if fixed.is_some() {
        all_programs(ordinary, &originals)
    } else {
        preflight(ordinary, chain, gens, target, &originals, preflight_record)
    };
    let (pp, ps, d, rho, r) = prove_pair(gens, &p, &s);
    assert_eq!(
        verify_linked_circuit(
            &cp.shape,
            &gens.p,
            &p.1,
            &pp.to_bytes(),
            &Link::First { rho, r },
            &*cp
        )
        .unwrap(),
        d
    );
    assert_eq!(
        verify_linked_circuit(
            &cs.shape,
            &gens.s,
            &s.1,
            &ps.to_bytes(),
            &Link::Second { d, rlc: &|_| r },
            &*cs
        )
        .unwrap(),
        rho
    );
    let pair = full::UniformPair {
        cp,
        cs,
        ip: p.1,
        is: s.1,
        pp,
        ps,
        d,
        rho,
        r,
        kind: full::ARCHIVE_SENT,
    };
    if fixed.is_some() {
        native::engine::decide_accumulators(gens, &pair).unwrap();
    }
    Round {
        pair,
        target,
        tapes,
    }
}

fn final_history(
    payer: &Inputs<'_>,
    params: &native::AuthenticatedParameters,
    ordinary: Generation,
    chain: &Chain,
    gens: &full::PairGens,
    target: Layouts,
    expected: &[u8],
    output: &Path,
) {
    let group = ordinary.compiler_group();
    let ordinary = ordinary.final_wallet_chain(chain, gens);
    let send = &ordinary.kinds[full::SEND];
    let receive = &ordinary.kinds[full::RECEIVE];
    let frozen = ProgramBundleV2::decode(expected).unwrap();
    let frozen_pins = |kind: usize| {
        [
            group
                .import::<Fp>(&frozen.primary[kind], true)
                .unwrap()
                .program_digest(),
            group
                .import::<Fq>(&frozen.secondary[kind], false)
                .unwrap()
                .program_digest(),
        ]
    };

    let payment = compiler_receiver::new_payment(
        payer,
        params,
        &chain.send,
        send,
        gens,
        frozen_pins(full::SEND),
    )
    .unwrap();
    let witness = ordinary.receive_witness(chain, gens);
    let status = compiler_receiver::new_receive_status(
        payer,
        params,
        &payment,
        &witness,
        receive,
        gens,
        frozen_pins(full::RECEIVE),
    )
    .unwrap();
    assert!(
        !status.status.opening.burned,
        "the final Receive credits its incoming Payment"
    );
    assert_eq!(status.payment, payment.digest);
    assert_eq!(status.status.lineage.proof, status.exact_pair());
    eprintln!(
        "final A Payment {} B {:?}, B status {:?}, B exact pair {} B {:?}, Credited {:?}",
        payment.exact.len(),
        blake3::hash(&payment.exact),
        blake3::Hash::from(status.status_digest),
        status.exact_pair().len(),
        blake3::hash(status.exact_pair()),
        blake3::Hash::from(status.credited.to_repr())
    );
    let final_round = archive_round(payer, &ordinary, chain, gens, &status, Some(target), output);
    let encoded = final_round.tapes.encode().unwrap();
    assert!(
        encoded.as_slice() == expected,
        "all ten freshly proven programs are the recursive fixed point"
    );
    let programs: [[[u8; 32]; 2]; 5] = std::array::from_fn(|k| {
        if k == full::ARCHIVE_SENT {
            pins(&final_round.pair)
        } else {
            pins(&ordinary.kinds[k])
        }
    });
    let profile = inspected_profile(
        group,
        programs.map(|p| p[0]),
        programs.map(|p| p[1]),
        &encoded,
    );
    let staging = output.with_extension("publishing");
    std::fs::create_dir(&staging).expect("reserve final compiler output");
    assert!(!output.exists(), "preserve the previous compiler run");
    std::fs::write(staging.join("public-program-profile.json"), profile).unwrap();
    std::fs::write(staging.join("program-bundle.norito"), encoded).unwrap();
    std::fs::rename(staging, output).unwrap();
}

#[test]
#[ignore = "generates all ten real wallet program originals until complete byte closure and verifies a fresh final chain"]
fn compile_five_kind_wallet_program_bundle() {
    compile_wallet_programs(CompilerGroup::Three);
}

#[test]
#[ignore = "checks all ten four-variable monetary programs, both transports and final wallet history"]
fn compile_five_kind_wallet_program_bundle_group_four() {
    compile_wallet_programs(CompilerGroup::Four);
}

fn compile_wallet_programs(group: CompilerGroup) {
    let previous = PathBuf::from(
        std::env::var_os("KAGEMUSHA_SPARTAN_ARCHIVE_PREVIOUS").expect("previous fifth tapes"),
    );
    let output = PathBuf::from(
        std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT").expect("new compiler output"),
    );
    with_retained_inputs(|latest, send, sent, engine, params, wallet| {
        let payer = Inputs {
            current: send,
            released: sent,
            ..latest
        };
        let seeds = compiler_predecessors::build(&payer, engine, params, wallet, &previous, group);
        let chain = seeds.chain;
        let mut ordinary = seeds.ordinary;
        let mut archive = (seeds.primary[4].clone(), seeds.secondary[4].clone());
        let mut history: Option<History> = None;
        let mut previous = all_programs(&ordinary, &archive).encode().unwrap();
        for round in 1..=12 {
            let gens = generators(&ordinary, &archive);
            let send = &ordinary.kinds[full::SEND];
            let receive = &ordinary.kinds[full::RECEIVE];
            let status = match &history {
                None => {
                    compiler_receiver::reconstruct(&payer, params, receive, &gens, pins(receive))
                        .unwrap()
                }
                Some((payment, witness)) => {
                    let candidate = compiler_receiver::new_receive_status_candidate(
                        &payer,
                        params,
                        payment,
                        witness,
                        receive,
                        &gens,
                        pins(receive),
                    )
                    .unwrap();
                    eprintln!(
                        "round {round} receiver accumulator decisions {:?}",
                        candidate.decisions
                    );
                    candidate.original
                }
            };
            let payment = compiler_receiver::new_payment_candidate(
                &payer,
                params,
                &chain.send,
                send,
                &gens,
                pins(send),
            )
            .unwrap();
            let witness = ordinary.receive_witness(&chain, &gens);
            eprintln!(
                "round {round} fresh Send decisions {:?} actual Payment {:?} {:?}",
                payment.decisions,
                payment.original.size,
                blake3::hash(&payment.original.exact)
            );
            let result = archive_round(
                &payer,
                &ordinary,
                &chain,
                &gens,
                &status,
                None,
                &output.with_extension(format!("round-{round}.unqualified.norito")),
            );
            let next = ordinary.next_archive_round(&chain, &gens, round, result.target);
            archive = (result.pair.cp.clone(), result.pair.cs.clone());
            let actual = all_programs(&next, &archive).encode().unwrap();
            assert!(
                actual == result.tapes.encode().unwrap(),
                "actual proved originals equal the public preflight"
            );
            if actual == previous {
                let fixed = generators(&next, &archive);
                final_history(
                    &payer,
                    params,
                    next,
                    &chain,
                    &fixed,
                    result.target,
                    &actual,
                    &output,
                );
                return;
            }
            eprintln!(
                "five-kind round {round} genuine bundle {} B {:?}",
                actual.len(),
                blake3::hash(&actual)
            );
            previous = actual;
            history = Some((payment.original, witness));
            ordinary = next;
        }
        panic!(
            "all ten wallet originals did not reach byte closure in twelve actual compiler rounds"
        );
    });
}

/// Independent post-build extraction for an already completed compiler run.
/// One input is the earlier iteration's bounded public export and the other is
/// the final bundle written only after fresh linked proofs and all native decisions.
#[test]
#[ignore = "requires two completed four-variable fixed-point originals from separate compiler passes"]
fn rederive_four_variable_fixed_point_public_profile() {
    use std::io::{Read, Write};
    let path = |key| PathBuf::from(std::env::var_os(key).expect("fixed-point compiler original"));
    let prior = path("KAGEMUSHA_SPARTAN_ARCHIVE_PREVIOUS");
    let final_path = path("KAGEMUSHA_SPARTAN_ARCHIVE_FINAL");
    assert_ne!(
        prior.canonicalize().unwrap(),
        final_path.canonicalize().unwrap()
    );
    let read = |p| {
        let mut bytes = Vec::new();
        std::fs::File::open(p)
            .unwrap()
            .take(native::MAX_PROGRAM_BUNDLE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() <= native::MAX_PROGRAM_BUNDLE_BYTES);
        bytes
    };
    let previous = read(prior);
    let actual = read(final_path);
    assert!(
        previous == actual,
        "complete prior and final originals must agree"
    );
    let originals = ProgramBundleV2::decode(&previous).unwrap();
    let group = CompilerGroup::Four;
    fn census<F: crate::fs::Scalar>(tapes: &[Vec<u8>; 5], primary: bool) -> [[u8; 32]; 5] {
        use crate::r1cs::Circuit;
        let side = if primary { "primary" } else { "secondary" };
        std::array::from_fn(|kind| {
            let bytes = &tapes[kind];
            let tape = CompilerGroup::Four.import::<F>(bytes, primary).unwrap();
            let digest = tape.program_digest();
            assert_eq!(tape.check_declared_degrees(), Ok(()));
            assert!(
                tape.export_program().unwrap() == *bytes,
                "exact {side} tape {kind}"
            );
            eprintln!(
                "five-kind census: side={side} kind={kind} digest={} sx={} sz={} k={} constraints={} challenges={} degrees={:?} outer={:?} inner={:?}",
                blake3::Hash::from(digest),
                tape.shape.sx,
                tape.shape.sz(),
                tape.shape.k,
                tape.constraints,
                tape.phase2_challenges(),
                tape.challenge_degrees(),
                tape.shape.outer_blocks(),
                tape.shape.inner_blocks(),
            );
            digest
        })
    }
    let p = census::<Fp>(&originals.primary, true);
    let s = census::<Fq>(&originals.secondary, false);
    let profile = inspected_profile(group, p, s, &actual);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path("KAGEMUSHA_SPARTAN_TEST_OUTPUT"))
        .unwrap()
        .write_all(&profile)
        .unwrap();
}
