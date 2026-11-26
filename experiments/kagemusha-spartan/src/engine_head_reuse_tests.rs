//! Verified-head reuse: a head an owner verified under one installed parameter
//! instance is reused only for byte-identical bytes under that same instance and
//! credential, and the reuse ends with the instance. Everything else (witness
//! reconstruction, the capsule comparison, live controls, hardware consent)
//! runs as before. Decode counts are per instance, so these tests may run in
//! parallel with any other test.
use super::*;
use std::path::{Path, PathBuf};
use std::sync::{Weak, atomic::Ordering::SeqCst};

pub(super) fn install(bundle: &Path) -> AuthenticatedParameters {
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

/// Full head decodes under this instance.
pub(super) fn decodes(parameters: &AuthenticatedParameters) -> usize {
    parameters.verified_head.decodes.load(SeqCst)
}

pub(super) fn same_head(a: &VerifiedWalletHead, b: &VerifiedWalletHead) -> bool {
    a.parameters_id == b.parameters_id
        && a.manifest_digest == b.manifest_digest
        && a.profile == b.profile
        && a.pair[..] == b.pair[..]
        && exact_credential(&a.credential, &b.credential)
        && certificate_bytes(&a.certificate) == certificate_bytes(&b.certificate)
        && a.custody_digest == b.custody_digest
        && a.enrollment_marker == b.enrollment_marker
}

/// Every word of a decoded pair, and its programs by identity.
pub(super) fn same_pair(a: &full::UniformPair, b: &full::UniformPair) -> bool {
    Arc::ptr_eq(&a.cp, &b.cp)
        && Arc::ptr_eq(&a.cs, &b.cs)
        && a.ip.io == b.ip.io
        && a.is.io == b.is.io
        && a.pp.to_bytes() == b.pp.to_bytes()
        && a.ps.to_bytes() == b.ps.to_bytes()
        && (a.d, a.rho, a.r, a.kind) == (b.d, b.rho, b.r, b.kind)
}

/// The payer's released head after its Send, rebuilt exactly from the Payment:
/// the same pair, credential and certificate finalization encodes.
pub(super) fn payer_head(
    payment: &[u8],
    parameters: &AuthenticatedParameters,
) -> (Vec<u8>, Credential) {
    let (trusted, d) = NativeEngine::payment_candidate(payment, parameters).unwrap();
    let decoded = carrier::decode_projected_pair(&trusted, &d.pair_bytes, &d.projection).unwrap();
    let u = assemble_pair(&parameters.inner, full::SEND, decoded).unwrap();
    let head = encode_head(parameters, &u, &d.originals.payer, &d.originals.certificate).unwrap();
    let (frame, ..) = decode_head(&head, parameters).unwrap();
    assert_eq!(
        frame.pair, d.pair_bytes,
        "the head carries the Payment's exact pair"
    );
    assert_eq!(usize::from(frame.kind), full::SEND);
    (head, d.originals.payer.clone())
}

pub(super) fn selected_payments() -> (PathBuf, Vec<Vec<u8>>) {
    let bundle = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let paid = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT").unwrap());
    let payments = ["two-wallet-payment.ipm1", "two-wallet-return-payment.ipm1"]
        .iter()
        .map(|f| std::fs::read(paid.join(f)).unwrap())
        .collect();
    (bundle, payments)
}

/// A reused head equals a full verification; a hit needs the same owner,
/// instance, credential and bytes; a refusal is unchanged and never replaces
/// the entry; a fresh engine (a restart) verifies in full.
#[test]
#[ignore = "requires the selected program bundle and its two Payments"]
fn predecessor_pair_reuse_is_exact_and_bound() {
    let (bundle, payments) = selected_payments();
    let parameters = install(&bundle);
    let heads: Vec<(Vec<u8>, Credential)> = payments
        .iter()
        .map(|p| payer_head(p, &parameters))
        .collect();
    assert!(!exact_credential(&heads[0].1, &heads[1].1), "two wallets");
    for (i, (head, credential)) in heads.iter().enumerate() {
        let case = format!("payer head {i}");
        let engine = NativeEngine::default();
        let start = decodes(&parameters);
        let (head1, pair1) = engine
            .verified_head_and_pair(head, credential, &parameters)
            .unwrap();
        assert_eq!(
            decodes(&parameters) - start,
            1,
            "{case}: first verification is full"
        );
        let (head2, pair2) = engine
            .verified_head_and_pair(head, credential, &parameters)
            .unwrap();
        assert_eq!(
            decodes(&parameters) - start,
            1,
            "{case}: second verification is reused"
        );
        assert!(
            same_head(&head1, &head2) && same_pair(&pair1, &pair2),
            "{case}: reuse"
        );
        let (_, profile, c, cert, independent) = decode_head(head, &parameters).unwrap();
        assert!(same_pair(&pair2, &independent), "{case}: independent pair");
        assert!(head2.profile == profile && exact_credential(&head2.credential, &c));
        assert_eq!(
            certificate_bytes(&head2.certificate),
            certificate_bytes(&cert)
        );
        let head3 = engine.verify_head(head, credential, &parameters).unwrap();
        assert!(
            same_head(&head1, &head3) && decodes(&parameters) - start == 1,
            "{case}: verify_head"
        );
        // Credential binding: refused as a fresh engine refuses it, after a full verification.
        let other = &heads[1 - i].1;
        let before = decodes(&parameters);
        for e in [&engine, &NativeEngine::default()] {
            assert!(
                matches!(
                    e.verified_head_and_pair(head, other, &parameters),
                    Err(Error::AuthorityMismatch("requested head credential"))
                ),
                "{case}: credential"
            );
        }
        assert_eq!(
            decodes(&parameters) - before,
            2,
            "{case}: a different credential never hits"
        );
        // Byte binding: a changed head gets the fresh engine's refusal.
        let mut changed = head.clone();
        *changed.last_mut().unwrap() ^= 1;
        let warm = format!(
            "{:?}",
            engine
                .verified_head_and_pair(&changed, credential, &parameters)
                .err()
        );
        let cold = format!(
            "{:?}",
            NativeEngine::default()
                .verified_head_and_pair(&changed, credential, &parameters)
                .err()
        );
        assert!(
            warm != "None" && warm == cold,
            "{case}: changed bytes {warm} {cold}"
        );
        // A refusal never replaced the accepted head.
        let before = decodes(&parameters);
        engine
            .verified_head_and_pair(head, credential, &parameters)
            .unwrap();
        assert_eq!(
            decodes(&parameters),
            before,
            "{case}: refusals keep the entry"
        );
        // Instance binding: another installation of the same bundle has its own cache.
        let reinstalled = install(&bundle);
        engine
            .verified_head_and_pair(head, credential, &reinstalled)
            .unwrap();
        assert_eq!(
            decodes(&reinstalled),
            1,
            "{case}: another instance verifies in full"
        );
        engine
            .verified_head_and_pair(head, credential, &reinstalled)
            .unwrap();
        assert_eq!(
            decodes(&reinstalled),
            1,
            "{case}: and then reuses under itself"
        );
        assert_eq!(
            decodes(&parameters),
            before,
            "{case}: without touching the first instance"
        );
        // Owner binding: an epoch-selected owner verifies in full.
        let owner = NativeEngine {
            selection_owner: Arc::new(()),
            ..engine.clone()
        };
        owner
            .verified_head_and_pair(head, credential, &parameters)
            .unwrap();
        assert_eq!(
            decodes(&parameters) - before,
            1,
            "{case}: another owner verifies in full"
        );
        // A restart is a fresh engine: full verification, same result.
        let (fresh, fresh_pair) = NativeEngine::default()
            .verified_head_and_pair(head, credential, &parameters)
            .unwrap();
        assert_eq!(
            decodes(&parameters) - before,
            2,
            "{case}: a restart verifies in full"
        );
        assert!(
            same_head(&head1, &fresh) && same_pair(&pair1, &fresh_pair),
            "{case}: restart"
        );
    }
}

/// Reinstallation: once the old installed instance is gone, its programs and
/// parameter set are freed although the engine that verified under it lives
/// on and keeps reusing heads under the new instance.
#[test]
#[ignore = "requires the selected program bundle and its two Payments"]
fn reinstallation_drops_the_old_circuits() {
    let (bundle, payments) = selected_payments();
    let engine = NativeEngine::default();
    let old = install(&bundle);
    let (head, credential) = payer_head(&payments[0], &old);
    for _ in 0..2 {
        engine
            .verified_head_and_pair(&head, &credential, &old)
            .map(drop)
            .unwrap();
    }
    engine
        .verify_head(&head, &credential, &old)
        .map(drop)
        .unwrap();
    assert_eq!(
        decodes(&old),
        1,
        "the old instance verified once and then reused"
    );
    let set: Weak<ParameterSet> = Arc::downgrade(&old.inner);
    let primary: Vec<Weak<UniformCircuit<Fp>>> = old
        .inner
        .programs
        .iter()
        .map(|x| Arc::downgrade(&x.primary))
        .collect();
    let secondary: Vec<Weak<UniformCircuit<Fq>>> = old
        .inner
        .programs
        .iter()
        .map(|x| Arc::downgrade(&x.secondary))
        .collect();
    // Reinstall and drop the old instance while the engine lives on, before it
    // verifies anything under the new one.
    let fresh = install(&bundle);
    drop(old);
    assert!(set.upgrade().is_none(), "the old parameter set is freed");
    assert!(
        primary.iter().all(|w| w.upgrade().is_none()),
        "every old primary circuit is freed"
    );
    assert!(
        secondary.iter().all(|w| w.upgrade().is_none()),
        "every old secondary circuit is freed"
    );
    // The same engine verifies once under the new instance and then reuses.
    for _ in 0..2 {
        engine
            .verified_head_and_pair(&head, &credential, &fresh)
            .map(drop)
            .unwrap();
    }
    assert_eq!(
        decodes(&fresh),
        1,
        "the new instance verifies once and then reuses"
    );
}

/// On a retained signed Receive capsule: a resume that reuses the predecessor
/// rebuilds the same preparation as a full resume and as a restarted engine,
/// with full witness reconstruction and the capsule comparison.
#[test]
#[ignore = "requires a five-kind bundle and a retained Receive capsule under it"]
fn resume_with_reused_predecessor_rebuilds_the_same_preparation() {
    let bundle = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let root = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ROOT").unwrap());
    let parameters = install(&bundle);
    let capsule = RecoveryCapsule::decode(
        &std::fs::read(root.join("counterparty-receive-capsule.norito")).unwrap(),
        &parameters.inner.scheme,
    )
    .unwrap();
    let engine = NativeEngine::default();
    let cold = engine.resume(&capsule, &parameters).unwrap();
    assert_eq!(decodes(&parameters), 1, "cold resume verifies the head");
    // Stock execute: the owner already verified the Payment, not the head.
    *parameters.verified_head.entry.lock().unwrap() = None;
    let stock = engine.resume(&capsule, &parameters).unwrap();
    assert_eq!(
        decodes(&parameters),
        2,
        "without an accepted head the resume verifies it"
    );
    let reused = engine.resume(&capsule, &parameters).unwrap();
    assert_eq!(decodes(&parameters), 2, "the accepted head is reused");
    let restart = NativeEngine::default();
    let fresh = restart.resume(&capsule, &parameters).unwrap();
    assert_eq!(decodes(&parameters), 3, "a restart verifies the head");
    let expected = capsule.encode().unwrap();
    for (label, p, owner) in [
        ("stock", &stock, &engine),
        ("reused", &reused, &engine),
        ("restart", &fresh, &restart),
    ] {
        assert_eq!(p.capsule.encode().unwrap(), expected, "{label}: capsule");
        assert_eq!(
            p.witness.encode().unwrap(),
            cold.witness.encode().unwrap(),
            "{label}: witness"
        );
        assert!(
            same_head(&p.predecessor, &cold.predecessor),
            "{label}: predecessor head"
        );
        let (a, b) = (
            p.resumed_under.as_ref().unwrap(),
            cold.resumed_under.as_ref().unwrap(),
        );
        assert!(
            same_pair(
                a.predecessor.as_ref().unwrap(),
                b.predecessor.as_ref().unwrap()
            ),
            "{label}: pair"
        );
        assert!(
            Arc::ptr_eq(&a.original, &p.predecessor.pair),
            "{label}: finalize takes this pair"
        );
        assert!(
            owner.resumed_here(p, &parameters),
            "{label}: resumed under its owner"
        );
        let receive = |q: &PreparedOperation| {
            q.receive
                .as_ref()
                .map(|r| (r.can_credit, r.pair_bytes.clone(), r.verified.is_some()))
        };
        assert_eq!(receive(p), receive(&cold), "{label}: Receive candidate");
        assert_eq!(
            p.receipt_body, cold.receipt_body,
            "{label}: receipt template"
        );
    }
}

/// Exercise direct finalization with the head and incoming proof of a freshly
/// prepared signed Receive. Its enclosing test also replays the exact capsule.
pub(in crate::native) fn check_signed_receive_inputs(
    owner: &NativeEngine,
    direct: &mut PreparedOperation,
    parameters: &AuthenticatedParameters,
) -> Result<()> {
    assert!(direct.resumed_under.is_none());
    let (_, expected_pair) = owner.verified_head_and_pair(
        &direct.predecessor.pair,
        &direct.predecessor.credential,
        parameters,
    )?;
    let before = decodes(parameters);
    let hit = owner.predecessor_for_proof(direct, parameters)?;
    assert!(same_pair(&expected_pair, &hit));
    assert_eq!(decodes(parameters), before);
    let other = NativeEngine::default();
    let miss = other.predecessor_for_proof(direct, parameters)?;
    assert!(same_pair(&expected_pair, &miss));
    assert_eq!(decodes(parameters), before + 1);
    direct.predecessor.credential.signature[0] ^= 1;
    assert!(matches!(
        other.predecessor_for_proof(direct, parameters),
        Err(Error::AuthorityMismatch("requested head credential"))
    ));
    direct.predecessor.credential.signature[0] ^= 1;

    let incoming = direct.receive.as_ref().expect("prepared Receive");
    let pair = &incoming.pair;
    let gens = &parameters.inner.generators;
    let traced = full::trace_incoming_candidate(pair.clone(), gens, incoming.pair_bytes.clone());
    fn check<C: iroha_pasta::PastaCurve>(
        gens: &Gens<C>,
        key: &[crate::ipa::Aff<C>],
        circuit: &UniformCircuit<crate::ipa::F<C>>,
        instance: &Instance<crate::ipa::F<C>>,
        proof: &spartan::Proof<C>,
        link: &Link,
        trace: &crate::verifier::Trace<C>,
        word: [u8; 32],
    ) where
        crate::ipa::F<C>: Scalar,
    {
        let verified = spartan::verify_linked_circuit_with_io(
            &circuit.shape,
            gens,
            instance,
            &proof.to_bytes(),
            link,
            circuit,
            Some(key),
        )
        .unwrap();
        assert_eq!(verified.word, word);
        assert_eq!(verified.io, trace.circuit.as_ref().unwrap().io_full);
        let expected = full::inherited_generator_claim(trace, gens).unwrap();
        let inherited = full::inherited_generator_claim_from_io(&verified.io, gens).unwrap();
        assert_eq!(inherited.g, expected.g);
        assert_eq!(inherited.us, expected.us);
        assert!(crate::generator_claim::decide(gens, &inherited));
    }
    check(
        &gens.p,
        &gens.keys_p.as_ref().expect("mounted primary keys")[full::SEND],
        &pair.cp,
        &pair.ip,
        &pair.pp,
        &Link::First {
            rho: pair.rho,
            r: pair.r,
        },
        &traced.tr_p,
        pair.d,
    );
    check(
        &gens.s,
        &gens.keys_s.as_ref().expect("mounted secondary keys")[full::SEND],
        &pair.cs,
        &pair.is,
        &pair.ps,
        &Link::Second {
            d: pair.d,
            rlc: &|_| pair.r,
        },
        &traced.tr_s,
        pair.rho,
    );
    Ok(())
}

fn retained_receive(parameters: &AuthenticatedParameters) -> (RecoveryCapsule, CompletionRecord) {
    let root = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ROOT").unwrap());
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
    (capsule, record)
}

/// Hardware consent on the reused path: the receipt the hardware signed for
/// this capsule verifies against the stock, reused and restarted preparations
/// alike, a tampered signature is refused alike, and finalize refuses it before
/// any proof work.
#[test]
#[ignore = "requires a five-kind bundle and a retained Receive capsule with its completion"]
fn reused_resume_keeps_hardware_consent() {
    let bundle = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let parameters = install(&bundle);
    let (capsule, record) = retained_receive(&parameters);
    let signed = record.exact_receipt();
    let (body, signature): (&[u8], [u8; 64]) = (&signed[..338], signed[338..].try_into().unwrap());
    let mut forged = signature;
    forged[63] ^= 1;
    let engine = NativeEngine::default();
    engine.resume(&capsule, &parameters).map(drop).unwrap();
    *parameters.verified_head.entry.lock().unwrap() = None;
    let stock = engine.resume(&capsule, &parameters).unwrap();
    let reused = engine.resume(&capsule, &parameters).unwrap();
    let restart = NativeEngine::default();
    let fresh = restart.resume(&capsule, &parameters).unwrap();
    assert_eq!(
        decodes(&parameters),
        3,
        "cold, stock and restart verify; the reused resume does not"
    );
    let consent = |p: &PreparedOperation| {
        let accepted = HardwareAdvanceReceipt::verify_selected(p, body, signature).unwrap();
        (
            accepted.operation_id,
            accepted.capsule_digest,
            accepted.receipt.body,
            accepted.receipt.signature,
        )
    };
    let refusal = |p: &PreparedOperation| {
        format!(
            "{:?}",
            HardwareAdvanceReceipt::verify_selected(p, body, forged).err()
        )
    };
    assert_eq!(
        consent(&reused),
        consent(&stock),
        "the signed receipt binds the reused preparation"
    );
    assert_eq!(consent(&fresh), consent(&stock), "and the restarted one");
    assert!(
        refusal(&stock) != "None"
            && refusal(&reused) == refusal(&stock)
            && refusal(&fresh) == refusal(&stock)
    );
    let accepted = HardwareAdvanceReceipt::verify_selected(&stock, body, signature).unwrap();
    let tampered = HardwareAdvanceReceipt {
        receipt: bootstrap::Receipt {
            body: accepted.receipt.body,
            signature: forged,
        },
        operation_id: accepted.operation_id,
        capsule_digest: accepted.capsule_digest,
    };
    let start = std::time::Instant::now();
    let refused_reused = format!(
        "{:?}",
        engine.finalize(reused, &tampered, &parameters).err()
    );
    let refused_stock = format!("{:?}", engine.finalize(stock, &tampered, &parameters).err());
    assert!(
        refused_reused != "None" && refused_reused == refused_stock,
        "{refused_reused} {refused_stock}"
    );
    assert!(
        start.elapsed() < std::time::Duration::from_secs(30),
        "refused before proving"
    );
}

/// The public wallet path end to end on the reused preparation: finalize with
/// the hardware-signed receipt proves, and its output passes the stock head
/// verifier inside finalize.
#[test]
#[ignore = "requires a five-kind bundle and a retained Receive capsule with its completion; proves a Receive"]
fn reused_resume_finalizes_with_the_signed_receipt() {
    let bundle = PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
    let parameters = install(&bundle);
    let (capsule, record) = retained_receive(&parameters);
    let signed = record.exact_receipt();
    let engine = NativeEngine::default();
    engine.resume(&capsule, &parameters).map(drop).unwrap();
    let reused = engine.resume(&capsule, &parameters).unwrap();
    assert_eq!(decodes(&parameters), 1, "the second resume reused the head");
    let receipt = HardwareAdvanceReceipt::verify_selected(
        &reused,
        &signed[..338],
        signed[338..].try_into().unwrap(),
    )
    .unwrap();
    let completed = engine.finalize(reused, &receipt, &parameters).unwrap();
    assert_eq!(
        completed.completion().exact_receipt(),
        signed,
        "the completion carries the signed receipt"
    );
}
