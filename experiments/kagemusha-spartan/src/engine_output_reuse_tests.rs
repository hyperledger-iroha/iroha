//! A completion's output is verified once per owner: the exact reuse and its
//! limits, on two retained signed operations. No proof is produced here.
use super::*;
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

fn install(bundle: &Path) -> AuthenticatedParameters {
    let read = |name: &str| std::fs::read(bundle.join(name)).unwrap();
    let world: [u8; 32] = read("world-manifest-digest.bin").try_into().unwrap();
    crate::native::device_fixture::install_selected(
        &read("program-bundle.norito"),
        &read("parameter-manifest.norito"),
        read("parameter-signature.bin").try_into().unwrap(),
        Some(&world),
    )
    .unwrap()
}

/// One retained operation: its selected capsule and the completion staged for it.
struct Retained {
    capsule: RecoveryCapsule,
    record: CompletionRecord,
}

fn retained(variable: &str, parameters: &AuthenticatedParameters) -> Retained {
    let root = PathBuf::from(std::env::var_os(variable).unwrap());
    let capsule = RecoveryCapsule::decode(
        &std::fs::read(root.join("counterparty-receive-capsule.norito")).unwrap(),
        &parameters.inner.scheme,
    )
    .unwrap();
    let record = CompletionRecord::decode(
        &std::fs::read(root.join("counterparty-receive-completion.norito")).unwrap(),
        capsule.wallet(),
    )
    .unwrap();
    Retained { capsule, record }
}

/// A verification allocates its head's pair bytes; only the stored result shares them.
fn stored(a: &VerifiedWalletHead, b: &VerifiedWalletHead) -> bool {
    Arc::ptr_eq(&a.pair, &b.pair)
}

/// A staged check's outcome as text: the released record's hash, or the refusal.
fn verdict(result: Result<CompletedOperation>) -> String {
    match result {
        Ok(done) => format!(
            "released {}",
            blake3::hash(&done.completion().encode().unwrap()).to_hex()
        ),
        Err(error) => format!("{error:?}"),
    }
}

fn same_head(a: &VerifiedWalletHead, b: &VerifiedWalletHead) -> bool {
    a.parameters_id == b.parameters_id
        && a.manifest_digest == b.manifest_digest
        && a.profile == b.profile
        && a.pair[..] == b.pair[..]
        && exact_credential(&a.credential, &b.credential)
        && certificate_bytes(&a.certificate) == certificate_bytes(&b.certificate)
        && a.custody_digest == b.custody_digest
        && a.enrollment_marker == b.enrollment_marker
}

#[test]
#[ignore = "requires the selected program bundle and its two Payments"]
fn outgoing_payment_reuses_only_the_exact_verified_send() {
    use crate::native::engine::head_reuse_tests::{payer_head, selected_payments};
    let (bundle, payments) = selected_payments();
    let parameters = install(&bundle);
    let engine = NativeEngine::default();
    let (head, credential) = payer_head(&payments[0], &parameters);
    let (_, pair) = engine
        .verified_head_and_pair(&head, &credential, &parameters)
        .unwrap();
    let pair = pair.pair;
    let (trusted, candidate) = NativeEngine::payment_candidate(&payments[0], &parameters).unwrap();
    let send = &parameters.inner.programs[full::SEND];
    let raw = carrier::frame_projected_pair(&trusted, &candidate.pair_bytes).unwrap();
    let matches =
        |head: &full::UniformPair| same_send_proof(head, send, &candidate.projection, &raw);
    assert!(matches(&pair));
    let other_program = &parameters.inner.programs[full::RECEIVE];
    for change in 0..8 {
        let mut different = pair.clone();
        match change {
            0 => different.kind = full::RECEIVE,
            1 => different.cp = other_program.primary.clone(),
            2 => different.cs = other_program.secondary.clone(),
            3 => different.ip.io[0] += Fp::ONE,
            4 => different.is.io[0] += Fq::ONE,
            5 => different.r[0] ^= 1,
            6 => different.pp.ipa.z2 += Fp::ONE,
            _ => different.ps.ipa.z2 += Fq::ONE,
        }
        assert!(!matches(&different), "changed Send component {change}");
    }
    let output = |payment| {
        crate::native::encode(&OutputFrame {
            version: 1,
            head: head.clone(),
            payment,
        })
        .unwrap()
    };
    let accepted = engine
        .output_head(&output(payments[0].clone()), &parameters, &credential)
        .unwrap();
    assert_eq!(accepted.0.profile.as_slice(), pair.ip.io.as_slice());
    assert!(
        engine
            .output_head(&output(payments[1].clone()), &parameters, &credential)
            .is_err()
    );

    // Leave the correctly signed monetary originals and ordinary IO intact.
    // A different, canonically encoded final response must go through verification.
    let mut damaged = candidate.pair_bytes.clone();
    let end = raw.ranges.primary.end;
    damaged[end - 32..end].copy_from_slice(&(pair.pp.ipa.z2 + Fp::ONE).to_repr());
    let profile = payment_wire::Profile {
        pair: &trusted,
        scheme: parameters.inner.scheme,
        relation: parameters.inner.relation,
    };
    let altered = payment_wire::test_replace_pair(&profile, &payments[0], damaged).unwrap();
    assert!(
        engine
            .output_head(&output(altered), &parameters, &credential)
            .is_err()
    );
}

#[test]
#[ignore = "requires the signed device bundle and two retained Receive capsules with their completions"]
fn completion_output_reuse_is_exact_and_local_to_its_owner() {
    let bundle = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let parameters = install(&bundle);
    let current = retained("KAGEMUSHA_SPARTAN_DEVICE_ROOT", &parameters);
    let foreign = retained("KAGEMUSHA_SPARTAN_FOREIGN_ROOT", &parameters);
    let output = current.record.exact_output().to_vec();
    assert_ne!(
        output,
        foreign.record.exact_output(),
        "two distinct outputs"
    );

    let engine = NativeEngine::default();
    let started = Instant::now();
    let prepared = engine.resume(&current.capsule, &parameters).unwrap();
    let reconstruction = started.elapsed();
    let c = terms(&prepared.witness).0.clone();
    let staged = |owner: &NativeEngine, record: &CompletionRecord| {
        owner.verify_staged_completion(&prepared, record, &parameters)
    };
    let head = |owner: &NativeEngine, parameters: &AuthenticatedParameters| {
        owner.output_head(&output, parameters, &c).unwrap().0
    };
    // What an owner that holds nothing decides: it reconstructs the capsule and
    // verifies every part of the record. Each refusal below must be this one.
    let reference = |capsule: &RecoveryCapsule, record: &CompletionRecord| {
        verdict(NativeEngine::default().verify_completion(capsule, record, &parameters))
    };
    let mut references = 0;

    // The first presentation is verified in full; the same owner then gets that result.
    let started = Instant::now();
    let first = staged(&engine, &current.record).unwrap();
    let cold = started.elapsed();
    let started = Instant::now();
    let again = staged(&engine, &current.record).unwrap();
    let warm = started.elapsed();
    assert!(stored(&first.head, &again.head), "same owner, same bytes");
    assert!(same_head(&first.head, &again.head));
    assert_eq!(first.head.custody_digest, *current.record.capsule_digest());
    assert_eq!(
        again.completion().encode().unwrap(),
        current.record.encode().unwrap()
    );
    // What is stored is the verifier's head, before any caller set its custody digest.
    let started = Instant::now();
    let (direct, frame) = engine.output_head(&output, &parameters, &c).unwrap();
    let warm_output = started.elapsed();
    assert!(stored(&first.head, &direct));
    assert_eq!(direct.custody_digest, [0; 32]);
    assert!(
        frame.version == 1 && frame.payment.is_empty(),
        "a Receive output"
    );
    assert!(
        stored(&first.head, &head(&engine.clone(), &parameters)),
        "a clone is the same owner"
    );

    // Any other owner verifies for itself: a fresh engine, and one that selected a
    // protected epoch. The latter shares the single slot, which then holds its result.
    let fresh = NativeEngine::default();
    let started = Instant::now();
    let independent = head(&fresh, &parameters);
    let cold_output = started.elapsed();
    assert!(!stored(&first.head, &independent));
    assert!(same_head(&direct, &independent));
    assert!(stored(&independent, &head(&fresh, &parameters)));
    let genesis = parameters.genesis_commit_verifier();
    let checkpoint = genesis
        .export_epoch_checkpoint(genesis.initial_commit_epoch())
        .unwrap();
    let reselected = engine
        .with_protected_epoch(&parameters, &checkpoint)
        .unwrap();
    let other_owner = head(&reselected, &parameters);
    assert!(
        !stored(&first.head, &other_owner),
        "another epoch owner verifies"
    );
    assert!(same_head(&direct, &other_owner));
    assert!(stored(&other_owner, &head(&reselected, &parameters)));
    let back = head(&engine, &parameters);
    assert!(!stored(&first.head, &back) && !stored(&other_owner, &back));
    assert!(stored(&back, &head(&engine, &parameters)));
    // That owner's staged check reconstructs the preparation and verifies the output itself.
    let reselected_staged = staged(&reselected, &current.record).unwrap();
    assert!(!stored(&back, &reselected_staged.head));
    assert!(same_head(&first.head, &reselected_staged.head));
    let held = head(&engine, &parameters);
    assert!(!stored(&reselected_staged.head, &held));

    // Another credential is not served from the slot. It gets the verifier's refusal,
    // and a refusal stores nothing.
    for byte in [0, 97] {
        let mut wrong = c.clone();
        wrong.body[byte] ^= 1;
        assert!(matches!(
            engine.output_head(&output, &parameters, &wrong),
            Err(Error::AuthorityMismatch("requested head credential"))
        ));
    }
    let mut wrong = c.clone();
    wrong.signature[63] ^= 1;
    assert!(matches!(
        engine.output_head(&output, &parameters, &wrong),
        Err(Error::AuthorityMismatch("requested head credential"))
    ));
    assert!(stored(&held, &head(&engine, &parameters)));

    // Altered and malformed outputs are refused exactly as before, with the original held.
    for at in [
        0,
        1,
        output.len() / 7,
        output.len() / 2,
        output.len() - 2,
        output.len() - 1,
    ] {
        for bit in [1u8, 0x80] {
            let mut altered = output.clone();
            altered[at] ^= bit;
            assert!(
                engine.output_head(&altered, &parameters, &c).is_err(),
                "byte {at}"
            );
            let mut record = current.record.clone();
            record.frame.output = altered;
            let refusal = staged(&engine, &record);
            assert!(refusal.is_err(), "staged byte {at}");
            if bit == 1 && (at == output.len() / 2 || at == output.len() - 1) {
                assert_eq!(
                    verdict(refusal),
                    reference(&current.capsule, &record),
                    "byte {at}"
                );
                references += 1;
            }
        }
    }
    let mut longer = output.clone();
    longer.push(0);
    let decoded: OutputFrame = crate::native::decode(&output).unwrap();
    let relabel = |version: u16| {
        crate::native::encode(&OutputFrame {
            version,
            head: decoded.head.clone(),
            payment: decoded.payment.clone(),
        })
        .unwrap()
    };
    for malformed in [
        &[][..],
        &output[..output.len() - 1],
        &longer[..],
        &relabel(0)[..],
        &relabel(3)[..],
    ] {
        assert!(engine.output_head(malformed, &parameters, &c).is_err());
    }
    assert_eq!(relabel(1), output);
    // The same head under the Unload version is a valid head, and not this operation's output.
    let mut unload_version = current.record.clone();
    unload_version.frame.output = relabel(2);
    assert!(matches!(
        staged(&engine, &unload_version),
        Err(Error::AuthorityMismatch(
            "completion exact selected operation"
        ))
    ));
    assert_eq!(
        verdict(staged(&engine, &unload_version)),
        reference(&current.capsule, &unload_version)
    );
    references += 1;
    let held = head(&engine, &parameters);
    assert!(stored(&held, &head(&engine, &parameters)));

    // With the output held, the staged check still requires the wallet, the selected
    // receipt, the operation, the capsule digest and the parameter identity.
    let mut wrong_wallet = current.record.clone();
    wrong_wallet.frame.wallet[0] ^= 1;
    let mut wrong_operation = current.record.clone();
    wrong_operation.frame.operation_id[0] ^= 1;
    let mut wrong_signature = current.record.clone();
    *wrong_signature.frame.receipt.last_mut().unwrap() ^= 1;
    let mut wrong_body = current.record.clone();
    wrong_body.frame.receipt[200] ^= 1;
    let mut wrong_parameters = current.record.clone();
    wrong_parameters.frame.parameters_id[0] ^= 1;
    let mut wrong_custody = current.record.clone();
    wrong_custody.frame.capsule_digest[0] ^= 1;
    for record in [
        &wrong_wallet,
        &wrong_operation,
        &wrong_signature,
        &wrong_body,
        &wrong_parameters,
        &wrong_custody,
    ] {
        let refusal = staged(&engine, record);
        assert!(refusal.is_err());
        assert_eq!(verdict(refusal), reference(&current.capsule, record));
        references += 1;
    }
    assert!(matches!(
        staged(&engine, &wrong_custody),
        Err(Error::AuthorityMismatch("retained completion"))
    ));
    assert!(stored(
        &held,
        &staged(&engine, &current.record).unwrap().head
    ));

    // A second operation's output replaces the first in the slot. Neither record
    // passes with the other's preparation, and a held output spliced into the
    // other operation's record is refused by that operation's own checks.
    let other = engine.resume(&foreign.capsule, &parameters).unwrap();
    let foreign_first = engine
        .verify_staged_completion(&other, &foreign.record, &parameters)
        .unwrap();
    assert!(!stored(&held, &foreign_first.head));
    let foreign_again = engine
        .verify_staged_completion(&other, &foreign.record, &parameters)
        .unwrap();
    assert!(stored(&foreign_first.head, &foreign_again.head));
    assert!(staged(&engine, &foreign.record).is_err());
    assert!(
        engine
            .verify_staged_completion(&other, &current.record, &parameters)
            .is_err()
    );
    // Whatever a fresh owner decides for a spliced record, the owner that holds
    // the spliced output decides the same.
    let mut spliced = current.record.clone();
    spliced.frame.output = foreign.record.exact_output().to_vec();
    let held_foreign = verdict(staged(&engine, &spliced));
    assert_eq!(
        held_foreign,
        reference(&current.capsule, &spliced),
        "held foreign output in this record"
    );
    references += 1;
    let replaced = staged(&engine, &current.record).unwrap();
    assert!(
        !stored(&held, &replaced.head),
        "one slot: the first output was replaced"
    );
    assert!(stored(
        &replaced.head,
        &staged(&engine, &current.record).unwrap().head
    ));
    let mut spliced = foreign.record.clone();
    spliced.frame.output = output.clone();
    let held_current = verdict(engine.verify_staged_completion(&other, &spliced, &parameters));
    assert_eq!(
        held_current,
        reference(&foreign.capsule, &spliced),
        "held output in the foreign record"
    );
    references += 1;
    eprintln!("output reuse spliced records: {held_foreign}; {held_current}");

    // A fresh installation of the same signed bundle is another installation.
    let next = install(&bundle);
    let held = head(&engine, &parameters);
    let reinstalled = head(&engine, &next);
    assert!(!stored(&held, &reinstalled));
    assert!(stored(&reinstalled, &head(&engine, &next)));
    let returned = head(&engine, &parameters);
    assert!(!stored(&held, &returned) && !stored(&reinstalled, &returned));
    let under_next = engine
        .verify_staged_completion(&prepared, &current.record, &next)
        .unwrap();
    assert!(!stored(&returned, &under_next.head));
    assert!(same_head(&first.head, &under_next.head));

    // Cold recovery: a fresh owner reconstructs the capsule and verifies the output.
    // Its second recovery reconstructs and checks the receipt again, and reuses only
    // the output it verified itself.
    let recovering = NativeEngine::default();
    let started = Instant::now();
    let recovered = recovering
        .verify_completion(&current.capsule, &current.record, &parameters)
        .unwrap();
    let recovery = started.elapsed();
    assert!(!stored(&returned, &recovered.head) && !stored(&first.head, &recovered.head));
    assert!(same_head(&first.head, &recovered.head));
    let started = Instant::now();
    let recovered_again = recovering
        .verify_completion(&current.capsule, &current.record, &parameters)
        .unwrap();
    let recovery_again = started.elapsed();
    assert!(stored(&recovered.head, &recovered_again.head));
    assert!(
        recovering
            .verify_completion(&current.capsule, &wrong_signature, &parameters)
            .is_err()
    );
    assert!(
        recovering
            .verify_completion(&current.capsule, &wrong_custody, &parameters)
            .is_err()
    );
    assert!(
        recovering
            .verify_completion(&foreign.capsule, &current.record, &parameters)
            .is_err()
    );

    eprintln!(
        "output reuse: reconstruction {reconstruction:?}; staged check first {cold:?}, same owner again {warm:?}; \
         output alone fresh owner {cold_output:?}, held {warm_output:?}; cold recovery {recovery:?}, again {recovery_again:?}"
    );
    eprintln!("output reuse: {references} refusals equal a fresh owner's");
    eprintln!(
        "output reuse held bytes: output {} + head pair {} (shared with the returned head) + slot {}",
        output.len(),
        first.head.pair.len(),
        std::mem::size_of::<AcceptedOutput>()
    );
}
