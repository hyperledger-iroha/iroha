//! Compile the completed ArchiveSent relation over the installed predecessor programs.
//! These two tapes are one compiler iteration. A five-kind generation additionally
//! needs the three-selector-coordinate predecessors and the ten-tape fixed point.
use super::*;
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
fn compiler_receipt(payer: &Inputs<'_>, case: &Case) -> crate::receipt::Receipt {
    use num_bigint::BigUint;
    let credential = payer.current.credential();
    let scalar = BigUint::from_bytes_be(&[0x5a; 32]);
    let point = crate::p256::mul(&crate::p256::generator(), &scalar).unwrap();
    assert_eq!(credential.body[130], 4);
    assert_eq!(BigUint::from_bytes_be(&credential.body[131..163]), point.x);
    assert_eq!(BigUint::from_bytes_be(&credential.body[163..195]), point.y);
    let fields = public_fields(payer, case, payer.delivery.status_digest().unwrap());
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
    receipt.body[242..274].copy_from_slice(&payer.delivery.payment().digest().to_repr());
    receipt.body[274..306].copy_from_slice(payer.current.capsule_digest());
    receipt.signature = crate::bootstrap::sign(
        crate::receipt::message(&receipt),
        &scalar,
        "ArchiveSent compiler payer",
    );
    receipt
}

/// Start from the four installed, already padded program layouts and the natural
/// fifth. The five-kind compiler must recompute these targets from fresh witnesses
/// after it builds five-selector predecessors.
fn compiler_layout<F: crate::fs::Scalar>(
    installed: impl Iterator<Item = Requirements>,
    archive: &UniformCircuit<F>,
) -> Layout {
    let requirements: Vec<_> = installed.chain([Requirements::of(archive)]).collect();
    assert_eq!(requirements.len(), 5);
    bootstrap_layout::common_requirements(&requirements)
}

#[test]
#[ignore = "compiles and proves the signed payer relation on both curves from retained software-fixture originals"]
fn compile_completed_archive_programs() {
    let output =
        PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT").expect("compiler output"));
    let mut programs = None;
    with_both_forms(|forms, _, _, params| {
        let gens = &params.inner.generators;
        for (form, payer) in FORMS.iter().zip(forms) {
            let (case, _) = cases(payer);
            let receipt = compiler_receipt(payer, &case);
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
                primary_at(
                    payer, gens, &slots, &folds, &case, &receiver, projection, layout,
                )
            };
            let first = build(None);
            assert_eq!(first.derived, [Fp::ONE; 4]);
            assert_eq!(first.p.0.wallet_io(), Some(full::TABLE_IO));
            let primary_layout = compiler_layout(
                params
                    .inner
                    .programs
                    .iter()
                    .map(|p| Requirements::of(&p.primary)),
                &first.p.0,
            );
            drop(first);
            let second = compiler_secondary(gens, &slots, &folds, None);
            assert_eq!(second.0.wallet_io(), None);
            let secondary_layout = compiler_layout(
                params
                    .inner
                    .programs
                    .iter()
                    .map(|p| Requirements::of(&p.secondary)),
                &second.0,
            );
            drop(second);
            eprintln!("{form} ArchiveSent iteration P {primary_layout:?} S {secondary_layout:?}");
            let primary = build(Some(primary_layout)).p;
            let secondary = compiler_secondary(gens, &slots, &folds, Some(secondary_layout));
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
            let cp = UniformCircuit::<Fp>::import_program(&p, true).unwrap();
            let cs = UniformCircuit::<Fq>::import_program(&s, false).unwrap();
            assert_eq!(cp.program_digest(), primary.0.program_digest());
            assert_eq!(cs.program_digest(), secondary.0.program_digest());
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
            assert_eq!(
                programs.get_or_insert_with(|| (p.clone(), s.clone())),
                &(p, s),
                "both signed receiver forms compile identical canonical program bytes"
            );
        }
    });
    let (p, s) = programs.unwrap();
    std::fs::create_dir(&output).expect("reserve fresh compiler output");
    std::fs::write(output.join("archive-primary.program"), p).unwrap();
    std::fs::write(output.join("archive-secondary.program"), s).unwrap();
}
