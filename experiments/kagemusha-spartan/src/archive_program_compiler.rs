//! Compile the completed ArchiveSent relation over the installed predecessor programs.
//! These two tapes are one compiler iteration. A five-kind generation additionally
//! needs the three-selector-coordinate predecessors and the ten-tape fixed point.
use super::*;
use crate::full::backend_chain_tests::CompilerGroup;
use crate::{
    bootstrap_layout::{self, Requirements},
    r1cs::Circuit,
    spartan::{Link, verify_linked_circuit},
    uniform::Layout,
};
use std::path::PathBuf;

/// The stock software host fixture creates its payer credential with the public
/// `FakePlatformV1` seed 0x5a. This local compiler fixture can sign only for that
/// exact public key. Nothing here enters Native preparation, custody or installation.
pub(super) fn compiler_receipt(
    payer: &Inputs<'_>,
    case: &Case,
    payment: Fp,
    status: [u8; 32],
) -> crate::receipt::Receipt {
    use num_bigint::BigUint;
    let credential = payer.current.credential();
    let scalar = BigUint::from_bytes_be(&[0x5a; 32]);
    let point = crate::p256::mul(&crate::p256::generator(), &scalar).unwrap();
    assert_eq!(credential.body[130], 4);
    assert_eq!(BigUint::from_bytes_be(&credential.body[131..163]), point.x);
    assert_eq!(BigUint::from_bytes_be(&credential.body[163..195]), point.y);
    let fields = public_payment_fields(payer, case, payment, status);
    let operation = iroha_data_model::kagemusha::kagemusha_wallet_operation_id_v1(
        &credential.body[66..98].try_into().unwrap(),
        iroha_data_model::kagemusha::KagemushaWalletOperationKindV1::ArchiveSent,
        &fields[18].to_repr(),
    )
    .unwrap();
    let mut receipt =
        native::funding::unsigned_receipt(credential, &fields, field(&operation).unwrap());
    // Compiler witnesses use nonzero public sample values for the two frozen
    // custody operands. The real signed four-kind capsule deliberately refuses tag 5.
    receipt.body[242..274].copy_from_slice(&payment.to_repr());
    receipt.body[274..306].copy_from_slice(payer.current.capsule_digest());
    receipt.signature = crate::bootstrap::sign(
        crate::receipt::message(&receipt),
        &scalar,
        "ArchiveSent compiler payer",
    );
    receipt
}

/// Combine only the naturally compiled circuits. Installed tapes contain
/// optional VK padding from their original common layout. Carrying that padding
/// into a later row requirement can double the recursive witness height.
fn compiler_layout<F: crate::fs::Scalar>(
    installed: impl Iterator<Item = Requirements>,
    archive: &UniformCircuit<F>,
) -> Layout {
    let requirements: Vec<_> = installed.chain([Requirements::of(archive)]).collect();
    assert_eq!(requirements.len(), 5);
    for (kind, r) in requirements.iter().enumerate() {
        eprintln!("natural monetary requirements {kind}: {r:?}");
    }
    let cap = archive.max_exported_fixed_rows();
    let padded = bootstrap_layout::common_requirements(&requirements);
    let target = bootstrap_layout::common_export_layout(&requirements, cap).unwrap();
    eprintln!(
        "ArchiveSent fixed rows: archive natural={} max ordinary plus archive={} optional target={} exporter maximum ordinary={} selected={} wallet={}",
        archive.layout().fixed,
        requirements.iter().map(|r| r.layout.fixed).max().unwrap(),
        padded.fixed,
        cap,
        target.fixed,
        requirements[0].wallet_rows
    );
    target
}

/// Use the stock retail serializers on the exact proposed target, before
/// spending a new proof. The output tapes may be retained by the offline
/// generator for diagnosis even when their target does not fit the wire.
pub(super) fn qualify_layout(
    params: &native::AuthenticatedParameters,
    gens: &full::PairGens,
    p: &UniformCircuit<Fp>,
    s: &UniformCircuit<Fq>,
) {
    let transport = crate::carrier::TrustedProfile::from_parameters(
        params.inner.relation,
        p.shape,
        p,
        s.shape,
        s,
        true,
    )
    .unwrap();
    let matrix = |words, dimensions, shape: crate::r1cs::Shape| native::engine::MatrixQueryLayout {
        words,
        dimensions,
        occupied: [shape.sx, shape.sz()],
    };
    let matrices = [
        matrix(gens.defer_p.n(), [gens.defer_p.x, gens.defer_p.y], p.shape),
        matrix(gens.defer_s.n(), [gens.defer_s.x, gens.defer_s.y], s.shape),
    ];
    let payment = crate::payment_wire::admit_send_profile(&transport);
    let credit = native::engine::admit_common_program_geometry(&transport, matrices);
    eprintln!(
        "proposed compiler layout Send {payment:?}; worst-depth Receive and later-Send CreditStatus {credit:?}"
    );
    payment.expect("proposed Send must fit the normal Payment transport");
    credit.expect("both proposed worst-depth CreditStatus messages must fit the normal transport");
}

#[test]
#[ignore = "compiles and proves the signed payer relation on both curves from retained software-fixture originals"]
fn compile_completed_archive_programs() {
    completed_archive(CompilerGroup::Three);
}

#[test]
#[ignore = "four-variable engineering compiler over genuine wallet originals"]
fn compile_completed_archive_programs_group_four() {
    completed_archive(CompilerGroup::Four);
}

/// Persist a source-specific compiler result before any size or proof verdict.
fn write_unqualified_tapes(output: &std::path::Path, p: &[u8], s: &[u8]) {
    let dir = output.with_extension("unqualified");
    std::fs::create_dir(&dir).expect("reserve unqualified compiler originals");
    std::fs::write(dir.join("archive-primary.program"), p).unwrap();
    std::fs::write(dir.join("archive-secondary.program"), s).unwrap();
    eprintln!(
        "unqualified compiler originals P {} B, S {} B",
        p.len(),
        s.len()
    );
}

fn completed_archive(group: CompilerGroup) {
    let output =
        PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT").expect("compiler output"));
    let mut programs = None;
    with_both_forms(|forms, send, sent, params| {
        let gens = &params.inner.generators;
        let engine = NativeEngine::default();
        let chain = compiler_history::chain(send, sent, &engine, params);
        let columns = [
            gens.p.g.len().ilog2() as usize,
            gens.s.g.len().ilog2() as usize,
        ];
        let (p_requirements, s_requirements) = compiler_predecessors::seed_requirements(
            &chain, send, sent, &engine, params, columns, group,
        );
        for (form, payer) in FORMS.iter().zip(forms) {
            let (case, _) = cases(payer);
            let receipt = compiler_receipt(
                payer,
                &case,
                payer.delivery.payment().digest(),
                payer.delivery.status_digest().unwrap(),
            );
            let receiver = delivered_receipt(payer.delivery).unwrap();
            let projection = Projection::Completed {
                credential: payer.current.credential(),
                receipt: &receipt,
            };
            let slots = verified_slots(payer, params);
            let folds =
                full::GeneratorFolds::prepare(gens, &slots, &ProofEntropy::fresh().unwrap())
                    .unwrap();
            let build = |layout| {
                primary_at_group(
                    payer, gens, &slots, &folds, &case, &receiver, projection, layout, group,
                )
            };
            let first = build(None);
            assert_eq!(first.derived, [Fp::ONE; 4]);
            assert_eq!(first.p.0.wallet_io(), Some(full::TABLE_IO));
            let primary_layout = compiler_layout(p_requirements.iter().copied(), &first.p.0);
            drop(first);
            let second = compiler_secondary_group(gens, &slots, &folds, None, group);
            assert_eq!(second.0.wallet_io(), None);
            let secondary_layout = compiler_layout(s_requirements.iter().copied(), &second.0);
            drop(second);
            eprintln!("{form} ArchiveSent iteration P {primary_layout:?} S {secondary_layout:?}");
            let primary = build(Some(primary_layout)).p;
            let secondary =
                compiler_secondary_group(gens, &slots, &folds, Some(secondary_layout), group);
            assert_eq!(
                (primary.0.layout(), secondary.0.layout()),
                (primary_layout, secondary_layout)
            );

            let mut public = verifier_io(payer, &case);
            public.truncate(64);
            public[full::RECEIPT_IO] = crate::receipt::digest(&receipt);
            public[full::TABLE_IO..full::TABLE_IO + 4].copy_from_slice(
                &payer.released.head().profile()[full::TABLE_IO..full::TABLE_IO + 4],
            );
            assert_eq!(primary.1.io, public);
            assert_eq!(secondary.1.io, verifier_s_io());
            let p = primary.0.export_program().unwrap();
            let s = secondary.0.export_program().unwrap();
            let cp = group.import::<Fp>(&p, true).unwrap();
            let cs = group.import::<Fq>(&s, false).unwrap();
            assert_eq!(cp.program_digest(), primary.0.program_digest());
            assert_eq!(cs.program_digest(), secondary.0.program_digest());
            if programs.is_none() {
                write_unqualified_tapes(&output, &p, &s);
            }
            qualify_layout(params, gens, &cp, &cs);
            if group == CompilerGroup::Four && programs.is_none() {
                assert!(CompilerGroup::Three.import::<Fp>(&p, true).is_err());
                assert!(CompilerGroup::Three.import::<Fq>(&s, false).is_err());
                compiler_predecessors::qualify_seed_geometry(&cp, &cs, &chain, params);
            }
            let (pp, ps, d, rho, r) = prove_pair(gens, &primary, &secondary);
            let check_p = |io: Vec<Fp>| {
                verify_linked_circuit(
                    &cp.shape,
                    &gens.p,
                    &Instance { io },
                    &pp.to_bytes(),
                    &Link::First { rho, r },
                    &cp,
                )
            };
            assert_eq!(check_p(public.clone()).unwrap(), d);
            assert_eq!(
                verify_linked_circuit(
                    &cs.shape,
                    &gens.s,
                    &secondary.1,
                    &ps.to_bytes(),
                    &Link::Second { d, rlc: &|_| r },
                    &cs,
                )
                .unwrap(),
                rho
            );
            for at in [full::RECEIPT_IO, full::TABLE_IO, full::STATEMENT_IO] {
                let mut changed = public.clone();
                changed[at] += Fp::ONE;
                assert!(
                    check_p(changed).is_err(),
                    "substituted payer public word {at}"
                );
            }
            let pair = full::UniformPair {
                cp: std::sync::Arc::new(cp),
                ip: primary.1,
                pp,
                cs: std::sync::Arc::new(cs),
                is: secondary.1,
                ps,
                d,
                rho,
                r,
                kind: full::RECEIVE,
            };
            native::engine::decide_accumulators(gens, &pair).unwrap();
            eprintln!(
                "{form} compiled P {} bytes {} S {} bytes {}",
                p.len(),
                blake3::Hash::from(pair.cp.program_digest()),
                s.len(),
                blake3::Hash::from(pair.cs.program_digest())
            );
            if programs.is_none() {
                let staging = output.with_extension("receiving");
                std::fs::create_dir(&staging).expect("reserve fresh compiler output");
                std::fs::write(staging.join("archive-primary.program"), &p).unwrap();
                std::fs::write(staging.join("archive-secondary.program"), &s).unwrap();
                assert!(!output.exists(), "preserve previous compiler output");
                std::fs::rename(&staging, &output).unwrap();
            }
            assert_eq!(
                programs.get_or_insert_with(|| (p.clone(), s.clone())),
                &(p, s),
                "both signed receiver forms compile identical canonical program bytes"
            );
        }
    });
    let (p, s) = programs.unwrap();
    assert_eq!(
        std::fs::read(output.join("archive-primary.program")).unwrap(),
        p
    );
    assert_eq!(
        std::fs::read(output.join("archive-secondary.program")).unwrap(),
        s
    );
}

/// Compile the actual fifth selector from a genuine Send and receiver Receive in a
/// five-kind compiler generation. No generation or test authority is installed.
#[test]
#[ignore = "builds and decides four real compiler seed pairs and the first five-kind ArchiveSent pair"]
fn compile_first_five_coordinate_archive_programs() {
    first_five_archive(CompilerGroup::Three);
}

#[test]
#[ignore = "four-variable engineering compiler over genuine wallet originals"]
fn compile_first_five_coordinate_archive_programs_group_four() {
    first_five_archive(CompilerGroup::Four);
}

fn first_five_archive(group: CompilerGroup) {
    let previous = PathBuf::from(
        std::env::var_os("KAGEMUSHA_SPARTAN_ARCHIVE_PREVIOUS")
            .expect("previous monetary compiler tapes"),
    );
    let output =
        PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT").expect("compiler output"));
    with_retained_inputs(|latest, send, sent, engine, params, wallet| {
        let payer = Inputs {
            current: send,
            released: sent,
            ..latest
        };
        let seeds = compiler_predecessors::build(&payer, engine, params, wallet, &previous, group);
        let gens = &seeds.generators;
        let status = compiler_receiver::reconstruct(
            &payer,
            params,
            &seeds.receiver,
            gens,
            seeds.receiver_programs,
        )
        .unwrap();
        assert_ne!(
            status.status_digest,
            payer.delivery.status_digest().unwrap()
        );
        let original_status = payer.delivery.wire_status().unwrap();
        assert_ne!(
            status.exact_pair(),
            original_status.lineage.proof.as_slice()
        );
        assert_eq!(status.status.proof_digest, original_status.proof_digest);
        assert_eq!(status.payment, payer.delivery.payment().digest());
        assert_eq!(status.status.lineage.proof, status.exact_pair());
        eprintln!(
            "five-coordinate receiver pair {} bytes BLAKE3 {} status {} credited {} payment {}",
            status.exact_pair().len(),
            blake3::hash(status.exact_pair()),
            blake3::Hash::from(status.status_digest),
            blake3::Hash::from(status.credited.to_repr()),
            blake3::Hash::from(status.payment.to_repr())
        );
        let slots = [
            full::trace_uniform(seeds.payer.clone(), gens),
            full::trace_incoming_candidate(
                seeds.receiver.clone(),
                gens,
                status.exact_pair().to_vec(),
            ),
        ];
        let folds =
            full::GeneratorFolds::prepare(gens, &slots, &ProofEntropy::fresh().unwrap()).unwrap();
        let (case, _) = cases(&payer);
        let receipt = compiler_receipt(&payer, &case, status.payment, status.status_digest);
        let projection = Projection::Completed {
            credential: send.credential(),
            receipt: &receipt,
        };
        let build = |layout| {
            primary_at_group(
                &payer,
                gens,
                &slots,
                &folds,
                &case,
                &status.receipt,
                projection,
                layout,
                group,
            )
        };
        let (p_requirements, s_requirements) = seeds
            .ordinary
            .archive_ordinary_requirements(&seeds.chain, gens);
        let first = build(None);
        assert_eq!(first.derived, [Fp::ONE; 4]);
        let primary_layout = compiler_layout(p_requirements.into_iter(), &first.p.0);
        drop(first);
        let s = compiler_secondary_group(gens, &slots, &folds, None, group);
        let secondary_layout = compiler_layout(s_requirements.into_iter(), &s.0);
        drop(s);
        eprintln!(
            "five-coordinate ArchiveSent common layout P {primary_layout:?} S {secondary_layout:?}"
        );
        let p = build(Some(primary_layout)).p;
        let s = compiler_secondary_group(gens, &slots, &folds, Some(secondary_layout), group);

        let mut public = verifier_io(&payer, &case);
        public.truncate(64);
        public[full::STATEMENT_IO] = public_statement(&payer, &case, status.status_digest);
        public[full::RECEIPT_IO] = crate::receipt::digest(&receipt);
        public[full::TABLE_IO..full::TABLE_IO + 4]
            .copy_from_slice(&sent.head().profile()[full::TABLE_IO..full::TABLE_IO + 4]);
        public[full::KIND_C2_IO] = Fp::ONE;
        let mut secondary_io = verifier_s_io();
        secondary_io[full::KIND_C2_IO] = Fq::ONE;
        assert_eq!(p.1.io, public);
        assert_eq!(s.1.io, secondary_io);
        let primary = p.0.export_program().unwrap();
        let secondary = s.0.export_program().unwrap();
        let cp = std::sync::Arc::new(group.import::<Fp>(&primary, true).unwrap());
        let cs = std::sync::Arc::new(group.import::<Fq>(&secondary, false).unwrap());
        assert_eq!(cp.program_digest(), p.0.program_digest());
        assert_eq!(cs.program_digest(), s.0.program_digest());
        write_unqualified_tapes(&output, &primary, &secondary);
        qualify_layout(params, gens, &cp, &cs);
        for slots in [cp.late_slots(), cs.late_slots()] {
            assert!(slots.contains(&crate::generator_claim::IO_START));
        }
        assert_eq!(
            (cp.wallet_io(), cs.wallet_io()),
            (Some(full::TABLE_IO), None)
        );
        let (pp, ps, d, rho, r) = prove_pair(gens, &p, &s);
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
        let check = |ip: &Instance<Fp>| {
            verify_linked_circuit(
                &pair.cp.shape,
                &gens.p,
                ip,
                &pair.pp.to_bytes(),
                &Link::First { rho, r },
                pair.cp.as_ref(),
            )
        };
        assert_eq!(check(&pair.ip).unwrap(), d);
        assert_eq!(
            verify_linked_circuit(
                &pair.cs.shape,
                &gens.s,
                &pair.is,
                &pair.ps.to_bytes(),
                &Link::Second { d, rlc: &|_| r },
                pair.cs.as_ref()
            )
            .unwrap(),
            rho
        );
        for at in [
            full::KIND_C2_IO,
            full::RECEIPT_IO,
            full::STATEMENT_IO,
            full::TABLE_IO,
        ] {
            let mut io = public.clone();
            io[at] += Fp::ONE;
            assert!(check(&Instance { io }).is_err());
        }
        native::engine::decide_accumulators(gens, &pair).unwrap();
        std::fs::create_dir(&output).expect("fresh five-coordinate compiler output");
        std::fs::write(output.join("archive-primary.program"), &primary).unwrap();
        std::fs::write(output.join("archive-secondary.program"), &secondary).unwrap();
        eprintln!(
            "five-coordinate completed ArchiveSent proofs P {} S {} tapes P {} {} S {} {}",
            pair.pp.to_bytes().len(),
            pair.ps.to_bytes().len(),
            primary.len(),
            blake3::Hash::from(pair.cp.program_digest()),
            secondary.len(),
            blake3::Hash::from(pair.cs.program_digest())
        );
    });
}

#[path = "archive_fixed_point_compiler.rs"]
mod fixed_point;
