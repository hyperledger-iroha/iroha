//! Λ_archive's recursive receiver binding for both authenticated receiver forms
//! (successor candidate, unsigned).
//!
//! The joined ArchiveSent component pins its receiver Ω verdict (`results[0]`) and the
//! absence of a required correction (`results[3]`) to 1 after native `verify_pair`. Here
//! the same successor relation, with the 26-word statement opening and every semantic
//! check of the combined statement component, runs inside Λ_archive's primary circuit
//! (`full::p_archive_operation`, with `full::s_archive_operation` as its secondary), in the
//! ordinary deferred-claim form:
//!
//! - the payer's own folded head, whatever its kind, is the hard predecessor slot; the
//!   successor's before-state commitment and its whole before-lineage are that verified
//!   head's io. The historical Send that created the pending descriptor is not this slot;
//! - the receiver's folded head from the CreditStatus is the total incoming slot. It is
//!   either the signed Receive of this Payment or a later signed receiver Send. Its kind
//!   bits are its verified io values, held boolean with c1 = 0 in both deferred folds;
//!   they select the signed Receive or Send program's S key (in the primary) and P key
//!   (in the secondary) and that program's matrices in both deferred claims. Nothing about
//!   the receiver's kind is a program constant, so one program verifies both forms;
//! - `results[0]` is that slot's derived verdict; `results[3]` is the derived conjunction
//!   of its four deferred accumulator decisions;
//! - the receiver words the receipt and opening relations compare (folded receipt digest,
//!   head, statement, wallet, payment key and credit root) are that slot's io leaves;
//! - the evidence factor also requires the verified head to be a signed Receive or Send
//!   (c1 = 0) whose kind bit c0 equals the public form bit, a total predicate: a
//!   disagreement derives a false evidence and only the no-op;
//! - both slots' matrix claims fold into Λ_archive's own accumulator (its late io).
//!
//! The public ArchiveSent statement binds the wallet's canonical Credited transcript,
//! including all opening fields and the exact receiver Ω. The circuit reads every Ω
//! byte from the same input as the recursive verifier. Extra diagnostic words expose
//! retained Request, Payment and profile inputs during engineering qualification.
use super::*;
#[path = "archive_recursive_wire_successor.rs"]
mod wire;
use crate::{
    full::{self, ArchiveIncoming, TracedPair, VerifiedPredecessor},
    r1cs::Witness,
    verifier::Fq,
};
use iroha_data_model::kagemusha::{
    KagemushaWalletIndexedInsertV1, KagemushaWalletIndexedLeafV1, KagemushaWalletIndexedOpeningV1,
};
use std::cell::RefCell;

type ReceiverReceipt = super::super::receiver_signature::ReceiverReceipt;

/// Payer-held and receiver exact originals the native verifier supplies. The primary's
/// io already holds the profile (0..64), the folded matrix claim (from `full::LATE_IO`,
/// below the link), the link (124..128) and the inherited generator claim (from
/// `generator_claim::IO_START`, through 192).
const ORIGINALS_IO: usize = 192;
// `IO_START` is aligned to the generator claim's power-of-two width, so it ends by 192.
const _: () = assert!(ORIGINALS_IO >= crate::generator_claim::IO_START + 32);
/// Offsets inside the originals block: tag, credit, descriptor (7), transcript binding (2),
/// quoted credential digest, Payment digest, exact receipt digest, form bit, subject.
const FORM: usize = 14;
const SUBJECT: usize = 15;
/// The packed open and completed Request words.
const OPEN: usize = 16;
const COMPLETED: usize = 32;
/// The receiver statement, raw pair and primary profile digests.
const EXACT: usize = 48;
const STATEMENT_DIGEST: usize = EXACT;
const PAIR_DIGEST: usize = EXACT + 1;
const PROFILE_DIGEST: usize = EXACT + 2;
/// The remaining exact Request and Payment diagnostics still exceed the wallet profile.
const P_IO: usize = 256;
const _: () = assert!(ORIGINALS_IO + PROFILE_DIGEST < P_IO);
const S_IO: usize = 64;

#[derive(Clone, Copy)]
enum Projection<'a> {
    Diagnostics,
    Wallet,
    Completed {
        credential: &'a crate::credential::Credential,
        receipt: &'a crate::receipt::Receipt,
    },
}

impl Case {
    /// The successor relation over verifier-owned sources. Returns the derived values of
    /// the four branch inputs `[receiver verdict, evidence, signature, no correction]`.
    #[allow(clippy::too_many_arguments)]
    fn append_verified(
        &self,
        w: &mut Wiring<Fp>,
        family: usize,
        binding: [Fp; 2],
        receiver: &ReceiverReceipt,
        predecessor: &VerifiedPredecessor,
        incoming: &ArchiveIncoming<'_>,
        projection: Projection<'_>,
    ) -> std::result::Result<[Fp; 4], &'static str> {
        use crate::credential_policy::{all, zero_predicate};
        let facts = incoming.facts;
        let prior = monetary_state::append_state(w, family, &self.before.core, &self.before.rest);
        let next = monetary_state::append_state(w, family, &self.after.core, &self.after.rest);
        let [k0, k1] = facts.kind();
        let mut c = Cells::new(family);
        c.a.name = "Lambda_archive successor inputs and verified predecessor joins";
        let tag = c.cell(self.tag, 255);
        let credit = c.cell(self.credit, 255);
        let descriptor = self.descriptor.map(|v| c.cell(v, 255));
        let before = self.before.lineage.map(|v| c.cell(v, 255));
        let after = self.after.lineage.map(|v| c.cell(v, 255));
        let binding = binding.map(|v| c.cell(v, 255));
        // The before state and every before-lineage field are the verified payer head's.
        c.eq(
            source(prior.local, prior.commitment).linear(),
            predecessor.head().linear(),
        );
        for (&cell, verified) in before.iter().zip(predecessor.lineage()) {
            c.eq(own(cell, 1), verified.linear());
        }
        // The verified receiver head is a signed Receive or Send head: c1 = 0. The slot's
        // two admitted keys and both deferred folds already hold this; this total
        // predicate reads the same verified io leaf, never a free prover bit.
        let signed_kind = zero_predicate(&mut c, k1.linear(), k1.value(w));
        let local = w.push(c.a);
        let s = |k| source(local, k);
        let descriptor = descriptor.map(s);
        let ports = receiver.append(w, family, descriptor, s(signed_kind));
        // The words the receipt and opening relations compare are the verified head's.
        let context = facts.context();
        let mut joins = Cells::new(family);
        joins.a.name = "Lambda_archive receiver words from the total verifier slot";
        for (i, verified) in [
            (1, facts.receipt_digest()),
            (2, facts.head()),
            (3, facts.statement_digest()),
            (6, context[bc::WALLET]),
            (7, context[bc::WALLET + 1]),
            (8, context[bc::PAYMENT_KEY]),
            (9, context[bc::PAYMENT_KEY + 1]),
            (10, context[bc::PAYMENT_KEY + 2]),
            (11, context[bc::PAYMENT_KEY + 3]),
        ] {
            joins.eq(ports.public[i].linear(), verified.linear());
        }
        let opening = &ports.opening;
        if opening.len() != 4 + 32 {
            return Err("ArchiveSent credit opening");
        }
        joins.eq(
            opening[0].linear(),
            facts.lineage()[bc::CREDIT_ROOT].linear(),
        );
        w.push(joins.a);
        // The authenticated receiver kind against the public form: its verified kind bit
        // c0 (Send 1, Receive 0) must be the form bit, which the statement's own kind
        // word and the receipt's signed operation id already match. A total predicate in
        // the evidence: a disagreement derives a false result and only the no-op.
        let form = ports.public[12];
        let mut k = Cells::new(family);
        k.a.name = "Lambda_archive authenticated receiver kind against the form";
        let agrees = zero_predicate(
            &mut k,
            plus(&k0.linear(), &scaled(&form.linear(), -Fp::ONE)),
            k0.value(w) - form.value(w),
        );
        let receipt_evidence = k.bit(ports.evidence.value(w) == Fp::ONE);
        k.eq(own(receipt_evidence, 1), ports.evidence.linear());
        let evidence = all(&mut k, &[receipt_evidence, agrees]);
        let kind_local = w.push(k.a);
        let results = [
            facts.validity(),
            source(kind_local, evidence),
            ports.signature,
            incoming.corrections,
        ];
        let derived = results.map(|r| r.value(w));
        append(
            w,
            family,
            &ArchiveState {
                state: &prior,
                lineage: before.map(s),
            },
            &ArchiveState {
                state: &next,
                lineage: after.map(s),
            },
            ArchiveInputs {
                tag: s(tag),
                credit: s(credit),
                descriptor,
                results,
            },
            &self.core_path,
            &self.adjusted_path,
        );
        let statement = wire::append(
            w,
            family,
            predecessor,
            incoming,
            &prior,
            &next,
            s(credit),
            &ports,
        );
        if let Projection::Completed { credential, receipt } = projection {
            let current = crate::operation_auth::append_current(
                w, family, credential, &prior, predecessor,
            )?;
            let authenticated = crate::receipt::append_own(
                w, family, receipt, &current, &statement,
                crate::statement::Kind::Archive, crate::receipt::PaymentDigest::Absent,
            )?;
            expose(w, family, full::RECEIPT_IO, &[authenticated.digest()]);
        }

        expose(
            w,
            family,
            full::HEAD_IO,
            &[source(next.local, next.commitment)],
        );
        expose(w, family, bc::CONTEXT_IO, &predecessor.context());
        expose(w, family, bc::LINEAGE_IO, &after.map(s));
        if !matches!(projection, Projection::Diagnostics) {
            crate::full::archive_receiver_projection::bind(
                w,
                family,
                incoming.facts,
                crate::full::archive_receiver_projection::Originals {
                    statement: &ports.statement_fields,
                    statement_digest: ports.public[14],
                    receipt_digest: ports.public[5],
                    wallet: ports.quoted_wallet,
                    key: ports.quoted_key,
                    table: ports.quoted_table,
                    send_form: ports.public[12],
                    subject: ports.public[13],
                },
            );
            return Ok(derived);
        }
        let mut originals = vec![s(tag), s(credit)];
        originals.extend(descriptor);
        originals.extend(binding.map(s));
        originals.extend([
            ports.public[0],
            ports.public[4],
            ports.public[5],
            ports.public[12],
            ports.public[13],
        ]);
        debug_assert_eq!(originals.len(), OPEN);
        expose(w, family, ORIGINALS_IO, &originals);
        let request = ports.request.as_ref().ok_or("ArchiveSent signed Request")?;
        expose(w, family, ORIGINALS_IO + OPEN, &request.open);
        expose(w, family, ORIGINALS_IO + COMPLETED, &request.completed);
        expose(
            w,
            family,
            ORIGINALS_IO + EXACT,
            &[ports.public[14], facts.proof_digest(), incoming.io_digest],
        );
        Ok(derived)
    }
}

/// [`full::ArchiveRelation`] for one successor case and its receiver originals.
struct Composed<'a> {
    case: &'a Case,
    binding: [Fp; 2],
    receiver: &'a ReceiverReceipt,
    derived: RefCell<Option<[Fp; 4]>>,
    projection: Projection<'a>,
}

impl full::ArchiveRelation for Composed<'_> {
    fn append(
        &self,
        w: &mut Wiring<Fp>,
        family: usize,
        predecessor: &VerifiedPredecessor,
        incoming: &ArchiveIncoming<'_>,
    ) -> std::result::Result<(), &'static str> {
        let derived = self.case.append_verified(
            w,
            family,
            self.binding,
            self.receiver,
            predecessor,
            incoming,
            self.projection,
        )?;
        *self.derived.borrow_mut() = Some(derived);
        Ok(())
    }
}

type BuiltS = (UniformCircuit<Fq>, Instance<Fq>, Witness<Fq>);

/// Every constraint of one circuit of a linked pair: its link phase filled at `rho`,
/// the link io carrying `r` (the secondary's binding R when `None`), its second phase at
/// fixed challenges. `io` replaces the instance's ordinary io (a verifier's own public
/// input) when given. Returns the verdict, the R used and the late io values.
fn linked_holds<F: crate::fs::Scalar>(
    c: &UniformCircuit<F>,
    instance: &Instance<F>,
    witness: &Witness<F>,
    rho: &[u8; 32],
    r: Option<[u8; 32]>,
    io: Option<&[F]>,
) -> (bool, [u8; 32], Vec<F>) {
    use crate::r1cs::Circuit;
    let mut w = witness.w.clone();
    c.fill_link(rho, &mut w);
    let r = r
        .or_else(|| c.binding_r(&w))
        .expect("the secondary computes the binding R");
    let link = c.link_io(rho, &r);
    let ch: Vec<F> = (0..c.phase2_challenges())
        .map(|i| F::from(0x9e37_79b9_7f4a_7c15 ^ (0x1234_5678 * (i as u64 + 1))))
        .collect();
    let rel = c.instantiate(&c.shape, &ch, Some(w.as_mut_slice()));
    let late = c.late_values(&w);
    let ordinary = io.map_or_else(|| instance.io.clone(), <[F]>::to_vec);
    assert_eq!(
        ordinary.len(),
        instance.io.len(),
        "{}: ordinary io length",
        c.label
    );
    let io = crate::spartan::io_with_late(
        &crate::r1cs::io_with_link(&ordinary, &link),
        &c.late_slots(),
        &late,
    );
    let z = crate::r1cs::z_vector(&c.shape, &Instance { io }, &Witness { w });
    let (a, b, cc) = rel.abc(&c.shape, &z);
    let failing: Vec<usize> = (0..a.len()).filter(|&i| a[i] * b[i] != cc[i]).collect();
    if !failing.is_empty() {
        eprintln!(
            "  {}: {} unsatisfied rows, first {:?}",
            c.label,
            failing.len(),
            &failing[..failing.len().min(6)]
        );
        let mut owners = std::collections::BTreeMap::new();
        for &x in &failing {
            let key = c
                .row_owner(x)
                .map_or(("<unowned>".to_string(), 0), |(a, j, _)| {
                    (c.arrays[a].name.to_string(), j)
                });
            *owners.entry(key).or_insert(0usize) += 1;
        }
        for ((name, row), count) in owners.iter().take(8) {
            eprintln!("    {name} template row {row}: {count} instances");
        }
    }
    (failing.is_empty(), r, late)
}

/// The hard slot: the payer's own folded head, from its natively decided original.
fn payer_slot(payer: &Inputs<'_>, params: &native::AuthenticatedParameters) -> TracedPair {
    let (_, profile, _, _, own) =
        crate::native::engine::decode_head(payer.released.head().exact_pair(), params).unwrap();
    assert_eq!(&profile, payer.released.head().profile());
    full::trace_uniform(own, &params.inner.generators)
}

/// The signed program of a receiver head, from its authenticated statement's own kind
/// word: Receive (4) or Send (3), nothing else.
fn receiver_kind(evidence: &crate::native::sent_delivery::ReceiverEvidence) -> usize {
    let tag = |kind: KagemushaWalletOperationKindV1| Fp::from(u64::from(kind.tag()));
    match evidence.statement[16] {
        s if s == tag(KagemushaWalletOperationKindV1::Receive) => full::RECEIVE,
        s if s == tag(KagemushaWalletOperationKindV1::Send) => full::SEND,
        _ => panic!("the receiver head is a signed Receive or Send"),
    }
}

/// An independent decision of the receiver pair: decoded from the CreditStatus's exact
/// raw original under its authenticated projection for the statement's own kind and
/// decided by the native owners (`verify_pair`, including all four deferred
/// accumulators). It must be the native constructor's `evidence.pair`.
fn decided_receiver_pair(
    evidence: &crate::native::sent_delivery::ReceiverEvidence,
    params: &native::AuthenticatedParameters,
) -> full::UniformPair {
    let kind = receiver_kind(evidence);
    let trusted = params.inner.profile(kind).unwrap();
    let projection = crate::native::engine::projection(&evidence.profile, kind).unwrap();
    let decoded =
        crate::carrier::decode_projected_pair(&trusted, &evidence.exact_pair, &projection).unwrap();
    let pair = crate::native::engine::verify_pair(&params.inner, kind, decoded).unwrap();
    let given = &evidence.pair;
    assert_eq!(pair.kind, given.kind, "the decided receiver kind");
    assert_eq!(pair.ip.io, given.ip.io);
    assert_eq!(pair.ip.io.as_slice(), evidence.profile.as_slice());
    assert_eq!(pair.is.io, given.is.io);
    assert_eq!(pair.pp.to_bytes(), given.pp.to_bytes());
    assert_eq!(pair.ps.to_bytes(), given.ps.to_bytes());
    assert_eq!((pair.d, pair.rho, pair.r), (given.d, given.rho, given.r));
    let selected = &params.inner.programs[kind];
    for (decided, signed) in [
        (pair.cp.program_digest(), selected.primary.program_digest()),
        (given.cp.program_digest(), selected.primary.program_digest()),
        (
            pair.cs.program_digest(),
            selected.secondary.program_digest(),
        ),
        (
            given.cs.program_digest(),
            selected.secondary.program_digest(),
        ),
    ] {
        assert_eq!(decided, signed, "the signed program of the receiver's kind");
    }
    pair
}

/// The two slots: the payer's own folded head (hard) and a receiver pair with its exact
/// raw original as the total incoming candidate.
fn slots_with(
    payer: &Inputs<'_>,
    params: &native::AuthenticatedParameters,
    receiver: full::UniformPair,
    original: Vec<u8>,
) -> [TracedPair; 2] {
    [
        payer_slot(payer, params),
        full::trace_incoming_candidate(receiver, &params.inner.generators, original),
    ]
}

/// The two verified slots: the payer's own folded head and the receiver's folded head
/// carried by the CreditStatus, from the native constructor's decided pair and its exact
/// raw original.
fn verified_slots(payer: &Inputs<'_>, params: &native::AuthenticatedParameters) -> [TracedPair; 2] {
    let evidence = payer.delivery.receiver_evidence();
    slots_with(
        payer,
        params,
        evidence.pair.clone(),
        evidence.exact_pair.clone(),
    )
}

struct Lambda {
    p: Built,
    derived: [Fp; 4],
    incoming_validity: Fp,
}

/// Λ_archive's primary circuit for one successor case and its receiver originals.
fn primary(
    payer: &Inputs<'_>,
    params: &native::AuthenticatedParameters,
    slots: &[TracedPair; 2],
    folds: &full::GeneratorFolds,
    case: &Case,
    receiver: &ReceiverReceipt,
) -> Lambda {
    primary_with_projection(payer, params, slots, folds, case, receiver, false)
}

fn primary_with_projection(
    payer: &Inputs<'_>,
    params: &native::AuthenticatedParameters,
    slots: &[TracedPair; 2],
    folds: &full::GeneratorFolds,
    case: &Case,
    receiver: &ReceiverReceipt,
    wallet_projection: bool,
) -> Lambda {
    let projection = if wallet_projection { Projection::Wallet } else { Projection::Diagnostics };
    primary_at(payer, &params.inner.generators, slots, folds, case, receiver, projection, None)
}

#[allow(clippy::too_many_arguments)]
fn primary_at(
    payer: &Inputs<'_>,
    gens: &full::PairGens,
    slots: &[TracedPair; 2],
    folds: &full::GeneratorFolds,
    case: &Case,
    receiver: &ReceiverReceipt,
    projection: Projection<'_>,
    layout: Option<crate::uniform::Layout>,
) -> Lambda {
    let relation = Composed {
        case,
        binding: payer.binding(),
        receiver,
        derived: RefCell::new(None),
        projection,
    };
    let (w, idx) = full::p_archive_operation(&relation, gens, slots, folds).unwrap();
    let incoming_validity = {
        let cell = idx.defer.incoming_validity.expect("corrected P claim");
        w.arrays[cell.array].values[cell.cell][cell.instance]
    };
    let columns = gens.p.g.len().ilog2() as usize;
    let p = if matches!(projection, Projection::Completed { .. }) {
        full::backend_chain_tests::build_monetary_program(w, columns, true, layout)
    } else {
        let (a, p, n, t, c) = w.finish();
        UniformCircuit::build_with_copies_at(
            "Lambda_archive P", a, p, n, t, c, columns,
            if matches!(projection, Projection::Diagnostics) { P_IO } else { 64 },
            b"Lambda_archive P", layout,
        )
    };
    Lambda {
        p,
        derived: relation.derived.into_inner().unwrap(),
        incoming_validity,
    }
}

fn secondary(
    params: &native::AuthenticatedParameters,
    slots: &[TracedPair; 2],
    folds: &full::GeneratorFolds,
) -> BuiltS {
    let gens = &params.inner.generators;
    let (w, _) = full::s_archive_operation(gens, slots, folds);
    let cols = params.inner.programs[full::RECEIVE]
        .secondary
        .shape
        .cols()
        .trailing_zeros() as usize;
    let (a, p, n, t, c) = w.finish();
    UniformCircuit::build_with_copies_at(
        "Lambda_archive S",
        a,
        p,
        n,
        t,
        c,
        cols,
        S_IO,
        b"Lambda_archive S",
        None,
    )
}

fn compiler_secondary(
    gens: &full::PairGens,
    slots: &[TracedPair; 2],
    folds: &full::GeneratorFolds,
    layout: Option<crate::uniform::Layout>,
) -> BuiltS {
    let (w, _) = full::s_archive_operation(gens, slots, folds);
    full::backend_chain_tests::build_monetary_program(w, gens.s.g.len().ilog2() as usize, false, layout)
}

/// One Λ_archive witness against one secondary: both circuits' constraints with one
/// link R (the secondary's), and the primary's own R equal to it. `io` is a verifier's
/// own primary public input, replacing the one the witness implies.
fn pair_holds(lambda: &Lambda, s: &BuiltS, rho: &[u8; 32], io: Option<&[Fp]>) -> (bool, Vec<Fp>) {
    let s_io = verifier_s_io();
    let (s_ok, r, _) = linked_holds(&s.0, &s.1, &s.2, rho, None, Some(s_io.as_slice()));
    let (p_ok, _, late) = linked_holds(&lambda.p.0, &lambda.p.1, &lambda.p.2, rho, Some(r), io);
    (s_ok && p_ok, late)
}

/// The secondary's ordinary public input as a verifier fixes it: one, then zeros (no own
/// kind, no mode; its folded claim and link are late and link io).
fn verifier_s_io() -> Vec<Fq> {
    let mut io = vec![Fq::ZERO; S_IO];
    io[0] = Fq::ONE;
    io
}

/// The verifier's own reconstruction of Λ_archive's whole ordinary primary public input,
/// from natively verified originals only: the payer's released head and the successor
/// `case` its custody derived, and the verified delivery's Payment, open Request and
/// receiver evidence (statement, exact raw pair, profile, receipt and opening). No value of
/// the circuit or its witness is read.
fn verifier_io(payer: &Inputs<'_>, case: &Case) -> Vec<Fp> {
    use super::super::request_original::expected_words;
    let delivery = payer.delivery;
    let evidence = delivery.receiver_evidence();
    let status = delivery.status_digest().unwrap();
    let open = native::decode_open_request_peer(delivery.open_request()).unwrap();
    let completed = delivery.payment().request();
    let completed = [completed.body.as_slice(), &completed.signature].concat();
    let mut io = vec![Fp::ZERO; P_IO];
    io[0] = Fp::ONE;
    io[full::HEAD_IO] = monetary_state::commitment(&case.after.core, &case.after.rest);
    let profile = payer.released.head().profile();
    io[bc::CONTEXT_IO..bc::CONTEXT_IO + bc::FIELDS]
        .copy_from_slice(&profile[bc::CONTEXT_IO..bc::CONTEXT_IO + bc::FIELDS]);
    io[bc::LINEAGE_IO..bc::LINEAGE_IO + bc::LINEAGE_FIELDS].copy_from_slice(&case.after.lineage);
    io[full::STATEMENT_IO] = public_statement(payer, case, status);
    let form = receiver_kind(evidence) == full::SEND;
    let mut words = vec![case.tag, case.credit];
    words.extend(case.descriptor);
    words.extend(payer.binding());
    words.extend([
        crate::credential::digest(&evidence.credential),
        delivery.payment().digest(),
        crate::receipt::digest(&evidence.receipt),
        Fp::from(u64::from(form)),
        evidence.statement[17],
    ]);
    assert_eq!(words.len(), OPEN);
    words.extend(expected_words(open.request));
    words.extend(expected_words(&completed));
    assert_eq!(words.len(), EXACT);
    words.extend([
        crate::statement::digest(&evidence.statement),
        crate::payment::proof_digest(&evidence.exact_pair).unwrap(),
        crate::fs::io_digest(&evidence.profile),
    ]);
    io[ORIGINALS_IO..ORIGINALS_IO + words.len()].copy_from_slice(&words);
    io
}

/// Independently rebuild the shipped payer statement from native wallet inputs.
fn public_statement(payer: &Inputs<'_>, case: &Case, status: [u8; 32]) -> Fp {
    crate::statement::digest(&public_fields(payer, case, status))
}

fn public_fields(payer: &Inputs<'_>, case: &Case, status: [u8; 32]) -> [Fp; 26] {
    use crate::native::engine::transfers::archive_sent::{archive_statement, credited_digest};
    let payment = payer.delivery.payment();
    let credit = payment.statement()[17];
    let credited = credited_digest(credit, payment.digest(), status).unwrap();
    let profile = payer.released.head().profile();
    let at = bc::CONTEXT_IO + bc::RELATION;
    let relation = [profile[at], profile[at + 1]];
    archive_statement(
        relation,
        &case.before.core,
        &case.after.core,
        &case.before.rest,
        credit,
        credited,
    )
}

/// The invalid-evidence branch still names its exact submitted status. This is
/// only a transcript encoder for that test; the installed native validation rejects it.
fn changed_opening_statement(payer: &Inputs<'_>, case: &Case) -> Fp {
    use iroha_data_model::kagemusha::{
        KAGEMUSHA_WALLET_CREDIT_STATUS_DOMAIN_V1, kagemusha_wallet_poseidon_bytes_v1,
    };
    let mut status = payer.delivery.wire_status().unwrap();
    let sibling = &mut status.opening.siblings[31 * 32..];
    let changed = field((&*sibling).try_into().unwrap()).unwrap() + Fp::ONE;
    sibling.copy_from_slice(&changed.to_repr());
    assert!(status.credit_status_digest().is_err());
    let mut exact = Vec::from(1u16.to_le_bytes());
    exact.extend(status.statement.statement_digest().unwrap());
    exact.extend(status.proof_digest);
    exact.extend(crate::receipt::digest(&payer.delivery.receiver_evidence().receipt).to_repr());
    exact.extend(status.lineage.lineage_digest());
    exact.extend(status.opening.opening_digest().unwrap());
    public_statement(
        payer,
        case,
        kagemusha_wallet_poseidon_bytes_v1(KAGEMUSHA_WALLET_CREDIT_STATUS_DOMAIN_V1, &exact),
    )
}

/// A fixed nonzero link challenge for constraint checks.
fn rho() -> [u8; 32] {
    let mut r = [0u8; 32];
    r[..8].copy_from_slice(&0x5a17_c0de_u64.to_le_bytes());
    r
}

/// The successor removal and the retained-descriptor no-op at one fresh nonce.
fn cases(payer: &Inputs<'_>) -> (Case, Case) {
    let nonce = funding::fresh_state_nonce(Some(
        payer.released.released_core(payer.current).unwrap()[NONCE],
    ))
    .unwrap();
    let removal = payer.case(nonce).unwrap();
    let mut noop = removal.clone();
    noop.after.lineage[bc::PENDING_ROOT] = noop.before.lineage[bc::PENDING_ROOT];
    assert_ne!(
        removal.after.lineage[bc::PENDING_ROOT],
        noop.after.lineage[bc::PENDING_ROOT]
    );
    (removal, noop)
}

/// The receiver's credit route at its later Send head, read-only.
struct CreditRoute {
    root: [u8; 32],
    leaf: KagemushaWalletIndexedLeafV1,
    path: KagemushaWalletIndexedOpeningV1,
}

impl native::SpartanWalletMaps for CreditRoute {
    fn member_or_low(
        &mut self,
        map: native::SpartanWalletMap,
        root: [u8; 32],
        key: [u8; 32],
    ) -> Result<(
        KagemushaWalletIndexedLeafV1,
        KagemushaWalletIndexedOpeningV1,
    )> {
        if map != native::SpartanWalletMap::Credit || root != self.root || key != self.leaf.key {
            return Err(Error::AuthorityMismatch("fixture credit route"));
        }
        Ok((self.leaf, self.path))
    }
    fn insert(
        &mut self,
        _: native::SpartanWalletMap,
        _: [u8; 32],
        _: [u8; 32],
        _: [u8; 32],
    ) -> Result<KagemushaWalletIndexedInsertV1> {
        panic!("read-only delivery must not mutate the indexed tree")
    }
}

/// The CreditStatus of the receiver's genuine later signed Send (`incoming-payment.bin`,
/// released from the same signed device inputs), encoded by the native producer for this
/// Payment from the receiver's credit route under the Receive head's credit root.
fn later_send_status(
    payer: &Inputs<'_>,
    engine: &NativeEngine,
    params: &native::AuthenticatedParameters,
    wallet: &std::path::Path,
) -> (native::VerifiedIncomingPayment, Vec<u8>) {
    use super::super::credit_opening::StatusOpening;
    let later = engine
        .verify_payment(
            &std::fs::read(wallet.join("incoming-payment.bin")).unwrap(),
            params,
        )
        .unwrap();
    let original = payer.delivery.payment();
    let root = payer.delivery.receiver_evidence().profile[bc::LINEAGE_IO + bc::CREDIT_ROOT];
    assert_eq!(later.profile()[bc::LINEAGE_IO + bc::CREDIT_ROOT], root);
    let opening = StatusOpening::from_verified(payer.delivery, root).unwrap();
    let credit = original.statement()[17];
    let mut route = CreditRoute {
        root: root.to_repr(),
        leaf: KagemushaWalletIndexedLeafV1 {
            key: credit.to_repr(),
            value: poseidon::hash_with_domain(
                crate::receive_lineage::CREDIT_DOMAIN,
                &[credit, original.digest(), Fp::ZERO],
            )
            .to_repr(),
            next_key: opening.next.to_repr(),
        },
        path: KagemushaWalletIndexedOpeningV1 {
            slot: opening.index,
            siblings: opening.siblings.map(|v| v.to_repr()),
        },
    };
    let wire = engine
        .encode_credit_status_at_send_head(
            &later,
            original,
            payer.delivery.open_request(),
            &mut route,
            params,
        )
        .unwrap();
    (later, wire)
}

/// Both authenticated receiver forms of one delivered Send, each through the ordinary
/// native delivery verification: the retained Receive CreditStatus and the genuine
/// later Send CreditStatus.
fn with_both_forms(
    action: impl FnOnce(
        [&Inputs<'_>; 2],
        &PreparedOperation,
        &CompletedOperation,
        &native::AuthenticatedParameters,
    ),
) {
    with_retained_inputs(|payer, send, sent, engine, params, wallet| {
        let (later, wire) = later_send_status(&payer, engine, params, wallet);
        let delivery = engine
            .verify_sent_delivery(send, sent, payer.delivery.open_request(), &wire, params)
            .unwrap();
        let at_later_send = Inputs {
            delivery: &delivery,
            ..payer
        };
        let (received, headed) = (
            payer.delivery.receiver_evidence(),
            delivery.receiver_evidence(),
        );
        assert_eq!(receiver_kind(received), full::RECEIVE);
        assert_eq!(receiver_kind(headed), full::SEND);
        assert_eq!(headed.statement, *later.statement());
        assert_eq!(headed.exact_pair.as_slice(), later.exact_pair());
        assert_eq!(&headed.profile, later.profile());
        // The pending removal of both forms is the same descriptor at the same payer head.
        assert_eq!(
            delivery
                .verify_pending_removal(payer.released, payer.adjusted_removal)
                .unwrap(),
            payer
                .delivery
                .verify_pending_removal(payer.released, payer.adjusted_removal)
                .unwrap()
        );
        action([&payer, &at_later_send], send, sent, params);
    });
}

const FORMS: [&str; 2] = ["Receive", "later Send"];

/// The receipt the relation reads for a receiver slot: `receipt`, with the receiver words
/// that Λ_archive takes from the verified slot (folded receipt, head, statement digest,
/// wallet, payment key and credit root) set to that slot's profile, as the hard joins
/// require. Only a crossed test needs it; authentic evidence already agrees.
fn with_slot_words(receipt: &ReceiverReceipt, profile: &[Fp; 64]) -> ReceiverReceipt {
    let mut r = receipt.clone();
    for (i, at) in [
        (1, full::RECEIPT_IO),
        (2, full::HEAD_IO),
        (3, full::STATEMENT_IO),
        (6, bc::CONTEXT_IO + bc::WALLET),
        (7, bc::CONTEXT_IO + bc::WALLET + 1),
        (8, bc::CONTEXT_IO + bc::PAYMENT_KEY),
        (9, bc::CONTEXT_IO + bc::PAYMENT_KEY + 1),
        (10, bc::CONTEXT_IO + bc::PAYMENT_KEY + 2),
        (11, bc::CONTEXT_IO + bc::PAYMENT_KEY + 3),
    ] {
        r.expected[i] = profile[at];
    }
    r.opening.as_mut().unwrap().root = profile[bc::LINEAGE_IO + bc::CREDIT_ROOT];
    r
}

/// Positive and negative cases of both forms at one payer head: genuine removal, the
/// no-op that authentic evidence cannot elect, and the opening sibling substituted both as
/// public evidence (malformed: only the no-op) and as a private witness under the
/// verifier's authentic public input (unsatisfiable on both branches). One program
/// verifies every case of both forms.
#[test]
#[ignore = "requires the retained signed payer, receiver and device fixture originals"]
fn successor_archive_recursive_receiver_slot_derives_verdict_corrections_and_folded_root() {
    with_both_forms(|forms, _, _, params| {
        let gens = &params.inner.generators;
        let n = gens.defer_p.n();
        let rho = rho();
        let mut programs = (None, None);
        for (form, payer) in FORMS.iter().zip(forms) {
            let clock = std::time::Instant::now();
            let evidence = payer.delivery.receiver_evidence();
            // The native owners decide the receiver pair (its trace and all four deferred
            // accumulators) exactly as the in-circuit verdict bits below are derived.
            decided_receiver_pair(evidence, params);
            crate::native::engine::decide_accumulators(gens, &evidence.pair).unwrap();
            let slots = verified_slots(payer, params);
            assert!(slots[1].tr_p.accepts && slots[1].tr_s.accepts);
            let folds =
                full::GeneratorFolds::prepare(gens, &slots, &ProofEntropy::fresh().unwrap())
                    .unwrap();
            let (removal, noop) = cases(payer);
            let genuine = delivered_receipt(payer.delivery).unwrap();
            assert_eq!(
                genuine.opening.as_ref().unwrap().root,
                evidence.profile[bc::LINEAGE_IO + bc::CREDIT_ROOT]
            );
            // An opening whose route computes another root under the same verified folded
            // root: the root cell stays the verified head's io, so only the evidence fails.
            let mut mismatched = genuine.clone();
            mismatched.opening.as_mut().unwrap().siblings[31] += Fp::ONE;
            let s = secondary(params, &slots, &folds);
            assert_eq!(s.1.io, verifier_s_io(), "{form}: secondary public input");
            let s_digest = s.0.program_digest();
            assert_eq!(
                *programs.1.get_or_insert(s_digest),
                s_digest,
                "{form}: one S program"
            );
            eprintln!(
                "Lambda_archive S {form}: program={} sx={} sz={} cols={} built in {:.1}s",
                blake3::Hash::from(s_digest).to_hex(),
                s.0.shape.sx,
                s.0.shape.sz(),
                s.0.shape.cols(),
                clock.elapsed().as_secs_f64()
            );
            let accepted = [Fp::ONE; 4];
            let mismatch = [Fp::ONE, Fp::ZERO, Fp::ONE, Fp::ONE];
            for (label, case, receipt, derived, expected, witness_only) in [
                ("genuine, removal", &removal, &genuine, accepted, true, None),
                (
                    "genuine, retained descriptor",
                    &noop,
                    &genuine,
                    accepted,
                    false,
                    None,
                ),
                (
                    "mismatched sibling, removal",
                    &removal,
                    &mismatched,
                    mismatch,
                    false,
                    Some(false),
                ),
                (
                    "mismatched sibling, retained descriptor",
                    &noop,
                    &mismatched,
                    mismatch,
                    true,
                    Some(false),
                ),
            ] {
                let clock = std::time::Instant::now();
                let lambda = primary(payer, params, &slots, &folds, case, receipt);
                assert_eq!(
                    lambda.derived, derived,
                    "{form} {label}: derived branch inputs"
                );
                assert_eq!(
                    lambda.incoming_validity,
                    Fp::ONE,
                    "{form} {label}: corrected P claim"
                );
                let digest = lambda.p.0.program_digest();
                assert_eq!(
                    *programs.0.get_or_insert(digest),
                    digest,
                    "{form} {label}: one program"
                );
                let io = &lambda.p.1.io;
                let authentic = verifier_io(payer, case);
                if witness_only.is_none() {
                    assert_eq!(
                        io, &authentic,
                        "{form} {label}: the verifier's public input"
                    );
                } else {
                    let differing: Vec<_> = (0..P_IO).filter(|&i| io[i] != authentic[i]).collect();
                    assert_eq!(
                        differing,
                        [full::STATEMENT_IO],
                        "{form} {label}: exact opening changes the public ArchiveSent statement"
                    );
                    assert_eq!(
                        io[full::STATEMENT_IO],
                        changed_opening_statement(payer, case)
                    );
                }
                let (holds, late) = pair_holds(&lambda, &s, &rho, None);
                assert_eq!(holds, expected, "{form} {label}: linked constraints");
                if holds {
                    // Λ_archive's own accumulator: both slots' claims folded. It decides
                    // natively on the installed polynomial.
                    let claim = crate::deferral::claim_of(&late[..=n]);
                    assert_eq!(
                        gens.defer_p.eval(&claim.point),
                        claim.value,
                        "{form} {label}: folded claim"
                    );
                }
                if let Some(want) = witness_only {
                    let (private, _) = pair_holds(&lambda, &s, &rho, Some(authentic.as_slice()));
                    assert_eq!(
                        private, want,
                        "{form} {label}: private sibling under authentic input"
                    );
                }
                eprintln!(
                    "Lambda_archive P {form} {label}: holds={holds} program={} sx={} sz={} cols={} io={} built+checked in {:.1}s",
                    blake3::Hash::from(digest).to_hex(),
                    lambda.p.0.shape.sx,
                    lambda.p.0.shape.sz(),
                    lambda.p.0.shape.cols(),
                    io.len(),
                    clock.elapsed().as_secs_f64()
                );
            }
        }
    });
}

/// Malformed public evidence on both orientations derives a false evidence and admits
/// only the no-op: a form bit that disagrees with the verified receiver head's kind (and
/// with its statement's kind word), and crossed receipts and statements against the other
/// form's verified head. Neither can remove the pending descriptor.
#[test]
#[ignore = "requires the retained signed payer, receiver and device fixture originals"]
fn successor_archive_recursive_receiver_form_disagreements_and_crossed_heads_force_the_noop() {
    with_both_forms(|forms, _, _, params| {
        let gens = &params.inner.generators;
        let rho = rho();
        let receipts = forms.map(|payer| delivered_receipt(payer.delivery).unwrap());
        let credit = forms[0].delivery.payment().statement()[17];
        let mut program = None;
        for (index, payer) in forms.into_iter().enumerate() {
            let clock = std::time::Instant::now();
            let slots = verified_slots(payer, params);
            let folds =
                full::GeneratorFolds::prepare(gens, &slots, &ProofEntropy::fresh().unwrap())
                    .unwrap();
            let s = secondary(params, &slots, &folds);
            eprintln!(
                "Lambda_archive S {}: built in {:.1}s",
                FORMS[index],
                clock.elapsed().as_secs_f64()
            );
            let (removal, noop) = cases(payer);
            let profile = &payer.delivery.receiver_evidence().profile;
            // The form bit flipped against this verified head. A Receive-form bit also names
            // only this Payment's credit, as the hard subject row requires.
            let mut flipped = receipts[index].clone();
            flipped.send = !flipped.send;
            if !flipped.send {
                flipped.subject = credit;
            }
            // The other form's genuine receipt and statement against this verified head.
            let crossed = with_slot_words(&receipts[1 - index], profile);
            for (label, receipt) in [
                ("form bit disagreeing with the verified kind", &flipped),
                ("the other form's receipt and statement", &crossed),
            ] {
                for (branch, case, expected) in [
                    ("removal", &removal, false),
                    ("retained descriptor", &noop, true),
                ] {
                    let clock = std::time::Instant::now();
                    let lambda = primary(payer, params, &slots, &folds, case, receipt);
                    assert_eq!(
                        lambda.derived[0],
                        Fp::ONE,
                        "{label}: the verified head stays valid"
                    );
                    assert_eq!(lambda.derived[1], Fp::ZERO, "{label}: derived evidence");
                    assert_eq!(lambda.derived[3], Fp::ONE, "{label}: no correction");
                    let digest = lambda.p.0.program_digest();
                    assert_eq!(
                        *program.get_or_insert(digest),
                        digest,
                        "{label}: one program"
                    );
                    let io = &lambda.p.1.io;
                    assert_eq!(io[ORIGINALS_IO + FORM], Fp::from(u64::from(receipt.send)));
                    assert_eq!(io[ORIGINALS_IO + SUBJECT], receipt.subject);
                    assert_eq!(
                        io[ORIGINALS_IO + STATEMENT_DIGEST],
                        receipt.statement_digest
                    );
                    let (holds, _) = pair_holds(&lambda, &s, &rho, None);
                    assert_eq!(holds, expected, "{} head, {label}, {branch}", FORMS[index]);
                    eprintln!(
                        "Lambda_archive P {} head, {label}, {branch}: derived={:?} holds={holds} in {:.1}s",
                        FORMS[index],
                        lambda.derived.map(|v| v == Fp::ONE),
                        clock.elapsed().as_secs_f64()
                    );
                }
            }
        }
    });
}

/// Witness-only substitutions under the verifier's authentic public input are
/// unsatisfiable on both branches: a private statement preimage (both forms), a private
/// receiver profile word (its public profile digest stays authentic) and a private
/// receiver secondary io word (held to its kind's projection on any public input). As
/// malformed public evidence, the substituted profile derives a false verdict and admits
/// only the no-op.
#[test]
#[ignore = "requires the retained signed payer, receiver and device fixture originals"]
fn successor_archive_recursive_rejects_witness_only_statement_profile_and_secondary_substitutions()
{
    with_both_forms(|forms, _, _, params| {
        let gens = &params.inner.generators;
        let rho = rho();
        for (form, payer) in FORMS.iter().zip(forms) {
            let clock = std::time::Instant::now();
            let slots = verified_slots(payer, params);
            let folds =
                full::GeneratorFolds::prepare(gens, &slots, &ProofEntropy::fresh().unwrap())
                    .unwrap();
            let s = secondary(params, &slots, &folds);
            eprintln!(
                "Lambda_archive S {form}: built in {:.1}s",
                clock.elapsed().as_secs_f64()
            );
            let (removal, noop) = cases(payer);
            let genuine = delivered_receipt(payer.delivery).unwrap();
            // A different private preimage under the authentic public statement digest.
            let mut statement = genuine.clone();
            statement.statement[9] += Fp::ONE;
            for (branch, case) in [("removal", &removal), ("retained descriptor", &noop)] {
                let lambda = primary(payer, params, &slots, &folds, case, &statement);
                assert_eq!(
                    lambda.p.1.io,
                    verifier_io(payer, case),
                    "{form}: authentic public input"
                );
                let (holds, _) = pair_holds(&lambda, &s, &rho, None);
                assert!(!holds, "{form} private statement, {branch}");
                eprintln!("Lambda_archive P {form} private statement, {branch}: holds={holds}");
            }
        }
        // The Receive form's receiver pair with one private profile word changed: the
        // receiver's own pending root, which no receipt or opening word reads.
        let payer = forms[0];
        let evidence = payer.delivery.receiver_evidence();
        let mut profiled = evidence.pair.clone();
        profiled.ip.io[bc::LINEAGE_IO + bc::PENDING_ROOT] += Fp::ONE;
        let slots = slots_with(payer, params, profiled, evidence.exact_pair.clone());
        assert!(
            !slots[1].tr_p.accepts,
            "the changed profile is not the proven one"
        );
        let folds =
            full::GeneratorFolds::prepare(gens, &slots, &ProofEntropy::fresh().unwrap()).unwrap();
        let s = secondary(params, &slots, &folds);
        let (removal, noop) = cases(payer);
        let genuine = delivered_receipt(payer.delivery).unwrap();
        for (branch, case, expected) in [
            ("removal", &removal, false),
            ("retained descriptor", &noop, true),
        ] {
            let lambda = primary(payer, params, &slots, &folds, case, &genuine);
            assert_eq!(
                lambda.derived[0],
                Fp::ZERO,
                "changed profile: derived verdict"
            );
            let authentic = verifier_io(payer, case);
            let differing: Vec<_> = (0..P_IO)
                .filter(|&i| lambda.p.1.io[i] != authentic[i])
                .collect();
            assert_eq!(
                differing,
                [full::STATEMENT_IO, ORIGINALS_IO + PROFILE_DIGEST],
                "the exact lineage and profile change"
            );
            let (malformed, _) = pair_holds(&lambda, &s, &rho, None);
            assert_eq!(malformed, expected, "public changed profile, {branch}");
            let (private, _) = pair_holds(&lambda, &s, &rho, Some(authentic.as_slice()));
            assert!(
                !private,
                "private changed profile under authentic input, {branch}"
            );
            eprintln!(
                "Lambda_archive P private profile word, {branch}: derived={:?} as public={malformed} under authentic={private}",
                lambda.derived.map(|v| v == Fp::ONE)
            );
        }
        // A private secondary io word: the projection pin refuses it on any public input.
        let mut secondary_io = evidence.pair.clone();
        secondary_io.is.io[5] = Fq::ONE;
        let slots = slots_with(payer, params, secondary_io, evidence.exact_pair.clone());
        let folds =
            full::GeneratorFolds::prepare(gens, &slots, &ProofEntropy::fresh().unwrap()).unwrap();
        let s = secondary(params, &slots, &folds);
        let (holds, _, _) = linked_holds(&s.0, &s.1, &s.2, &rho, None, None);
        assert!(
            !holds,
            "a private secondary io word outside the kind's projection"
        );
        eprintln!("Lambda_archive S private secondary io word: holds={holds}");
    });
}

/// The receiver P proof's carried matrix accumulator value changed inside its exact raw
/// original, for both forms. Native owners refuse it at their first failing check and at
/// the first deferred decision. In-circuit, the corrected claim derives the verdict and the
/// correction conjunction as 0: as public evidence (its own pair digest) only the no-op
/// holds; as a private pair under the authentic pair digest neither branch holds.
#[test]
#[ignore = "requires the retained signed originals; builds a second Lambda_archive receiver slot"]
fn successor_archive_recursive_receiver_slot_corrects_a_false_accumulator_to_the_noop() {
    with_both_forms(|forms, _, _, params| {
        let gens = &params.inner.generators;
        let n = gens.defer_p.n();
        let rho = rho();
        for (form, payer) in FORMS.iter().zip(forms) {
            let evidence = payer.delivery.receiver_evidence();
            let kind = receiver_kind(evidence);
            let decided = decided_receiver_pair(evidence, params);
            // Change only the receiver P proof's carried matrix accumulator value inside its
            // exact raw original. The candidate stays canonical, so the total parser keeps it.
            let value = decided.pp.late[n];
            let needle = value.to_repr();
            let at: Vec<usize> = evidence
                .exact_pair
                .windows(32)
                .enumerate()
                .filter(|(_, w)| *w == needle.as_ref())
                .map(|(i, _)| i)
                .collect();
            assert_eq!(
                at.len(),
                1,
                "{form}: one exact accumulator value in the original"
            );
            let mut raw = evidence.exact_pair.clone();
            raw[at[0]..at[0] + 32].copy_from_slice((value + Fp::ONE).to_repr().as_ref());
            let trusted = params.inner.profile(kind).unwrap();
            let projection = crate::native::engine::projection(&evidence.profile, kind).unwrap();
            let decode = || crate::carrier::decode_projected_candidate(&trusted, &raw, &projection);
            let first = crate::native::engine::verify_pair(&params.inner, kind, decode().unwrap())
                .err()
                .expect("native verification refuses the changed accumulator");
            let d = decode().unwrap();
            let changed = full::UniformPair {
                cp: decided.cp.clone(),
                ip: d.ip,
                pp: d.pp,
                cs: decided.cs.clone(),
                is: d.is,
                ps: d.ps,
                d: d.d,
                rho: d.rho,
                r: d.r,
                kind,
            };
            assert_eq!(changed.pp.late[n], value + Fp::ONE);
            let decision = crate::native::engine::decide_accumulators(gens, &changed)
                .err()
                .expect("the first deferred decision refuses");
            assert!(crate::native::engine::is_accumulator_refusal(&decision));
            let slots = slots_with(payer, params, changed, raw.clone());
            let folds =
                full::GeneratorFolds::prepare(gens, &slots, &ProofEntropy::fresh().unwrap())
                    .unwrap();
            let s = secondary(params, &slots, &folds);
            let (removal, noop) = cases(payer);
            let genuine = delivered_receipt(payer.delivery).unwrap();
            for (label, case, expected) in [
                ("false receiver accumulator, removal", &removal, false),
                (
                    "false receiver accumulator, retained descriptor",
                    &noop,
                    true,
                ),
            ] {
                let lambda = primary(payer, params, &slots, &folds, case, &genuine);
                assert_eq!(
                    lambda.incoming_validity,
                    Fp::ZERO,
                    "{form} {label}: corrected P claim iff"
                );
                assert_eq!(
                    lambda.derived[0],
                    Fp::ZERO,
                    "{form} {label}: derived receiver verdict"
                );
                assert_eq!(
                    lambda.derived[3],
                    Fp::ZERO,
                    "{form} {label}: derived correction conjunction"
                );
                let authentic = verifier_io(payer, case);
                let differing: Vec<_> = (0..P_IO)
                    .filter(|&i| lambda.p.1.io[i] != authentic[i])
                    .collect();
                assert_eq!(
                    differing,
                    [full::STATEMENT_IO, ORIGINALS_IO + PAIR_DIGEST],
                    "{form} {label}: the exact lineage and pair digest"
                );
                assert_eq!(
                    lambda.p.1.io[ORIGINALS_IO + PAIR_DIGEST],
                    crate::payment::proof_digest(&raw).unwrap()
                );
                let (holds, late) = pair_holds(&lambda, &s, &rho, None);
                assert_eq!(holds, expected, "{form} {label}: linked constraints");
                if holds {
                    // The self-computed corrected claim keeps Λ_archive's own accumulator true.
                    let claim = crate::deferral::claim_of(&late[..=n]);
                    assert_eq!(
                        gens.defer_p.eval(&claim.point),
                        claim.value,
                        "{form} {label}: folded claim"
                    );
                }
                let (private, _) = pair_holds(&lambda, &s, &rho, Some(authentic.as_slice()));
                assert!(
                    !private,
                    "{form} {label}: the changed pair under the authentic pair digest"
                );
                eprintln!(
                    "Lambda_archive P {form} {label}: derived={:?} holds={holds} under authentic={private}",
                    lambda.derived.map(|v| v == Fp::ONE)
                );
            }
            eprintln!(
                "{form}: native first failure: {first:?}; first deferred decision: {decision:?}"
            );
        }
    });
}

/// One linked Λ_archive proof under the installed generators: the primary's phase-1
/// digest, the secondary with it (giving rho and its binding R), the primary with both.
#[allow(clippy::type_complexity)]
fn prove_pair(
    gens: &full::PairGens,
    p: &Built,
    s: &BuiltS,
) -> (
    crate::spartan::Proof<iroha_pasta::Eq>,
    crate::spartan::Proof<iroha_pasta::Ep>,
    [u8; 32],
    [u8; 32],
    [u8; 32],
) {
    use crate::spartan::{Link, prepare_linked_phase1, prove_linked_circuit};
    let (cp, ip, wp) = p;
    let (cs, is, ws) = s;
    let entropy = ProofEntropy::fresh().unwrap();
    let rng_p = entropy.stream(b"Lambda_archive primary");
    let mut rng_s = entropy.stream(b"Lambda_archive secondary");
    let rlc = |_: &[u8; 32]| [0u8; 32];
    let d = prepare_linked_phase1(&cp.shape, &gens.p, ip, wp, &mut rng_p.clone(), cp);
    let (ps, rho, r) = prove_linked_circuit(
        &cs.shape,
        &gens.s,
        is,
        ws,
        &mut rng_s,
        &Link::Second { d, rlc: &rlc },
        cs,
    );
    let (pp, d2, _) = prove_linked_circuit(
        &cp.shape,
        &gens.p,
        ip,
        wp,
        &mut rng_p.clone(),
        &Link::First { rho, r },
        cp,
    );
    assert_eq!(d, d2, "same primary rows");
    (pp, ps, d, rho, r)
}

/// The linked Λ_archive pair for the genuine later receiver Send, proved at the payer's
/// current head and verified under programs built from another payer head (the earlier
/// Send) over the other receiver form (the Receive): the programs depend on no payer or
/// receiver value and on neither form. The verifier's public input is its own
/// reconstruction. Changed public words, the Receive form's public input and the earlier
/// head are refused. Λ_archive's own deferred values decide natively.
#[test]
#[ignore = "heavy: proves and verifies the linked Lambda_archive pair from the retained originals"]
fn successor_archive_recursive_pair_proves_verifies_and_decides_its_accumulators() {
    use crate::spartan::{Link, verify_linked_circuit};
    use std::sync::Arc;
    with_both_forms(|[received, headed], send, sent, params| {
        let gens = &params.inner.generators;
        let n = gens.defer_p.n();
        let clock = std::time::Instant::now();
        let earlier = Inputs {
            current: send,
            released: sent,
            ..*received
        };
        let nonce = funding::fresh_state_nonce(Some(
            headed.released.released_core(headed.current).unwrap()[NONCE],
        ))
        .unwrap();
        let (vp_program, vs_program) = {
            let slots = verified_slots(&earlier, params);
            let folds =
                full::GeneratorFolds::prepare(gens, &slots, &ProofEntropy::fresh().unwrap())
                    .unwrap();
            let receipt = delivered_receipt(earlier.delivery).unwrap();
            let lambda = primary(
                &earlier,
                params,
                &slots,
                &folds,
                &earlier.case(nonce).unwrap(),
                &receipt,
            );
            let s = secondary(params, &slots, &folds);
            (
                Arc::new(lambda.p.0.into_verifier_program()),
                Arc::new(s.0.into_verifier_program()),
            )
        };
        let verifier_secs = clock.elapsed().as_secs_f64();
        let slots = verified_slots(headed, params);
        let folds =
            full::GeneratorFolds::prepare(gens, &slots, &ProofEntropy::fresh().unwrap()).unwrap();
        let removal = headed.case(nonce).unwrap();
        let receipt = delivered_receipt(headed.delivery).unwrap();
        let lambda = primary(headed, params, &slots, &folds, &removal, &receipt);
        assert_eq!(lambda.derived, [Fp::ONE; 4]);
        let s = secondary(params, &slots, &folds);
        drop((folds, slots));
        assert_eq!(lambda.p.0.program_digest(), vp_program.program_digest());
        assert_eq!(s.0.program_digest(), vs_program.program_digest());
        // The verifier's own public inputs, never the producer's.
        let ip = Instance {
            io: verifier_io(headed, &removal),
        };
        let is = Instance {
            io: verifier_s_io(),
        };
        assert_eq!(ip.io, lambda.p.1.io);
        assert_eq!(is.io, s.1.io);
        let clock = std::time::Instant::now();
        let (pp, ps, d, rho, r) = prove_pair(gens, &lambda.p, &s);
        let prove_secs = clock.elapsed().as_secs_f64();
        drop((lambda, s));
        let fixed_r = move |_: &[u8; 32]| r;
        let verify = |ip: &Instance<Fp>, pp: &[u8]| {
            let rho_v = verify_linked_circuit(
                &vs_program.shape,
                &gens.s,
                &is,
                &ps.to_bytes(),
                &Link::Second { d, rlc: &fixed_r },
                &*vs_program,
            );
            let d_v = verify_linked_circuit(
                &vp_program.shape,
                &gens.p,
                ip,
                pp,
                &Link::First { rho, r },
                &*vp_program,
            );
            rho_v.is_ok_and(|x| x == rho) && d_v.is_ok_and(|x| x == d)
        };
        let clock = std::time::Instant::now();
        assert!(
            verify(&ip, &pp.to_bytes()),
            "the linked Lambda_archive pair verifies"
        );
        let verify_secs = clock.elapsed().as_secs_f64();
        assert_eq!(
            ip.io[bc::LINEAGE_IO + bc::PENDING_ROOT],
            removal.after.lineage[bc::PENDING_ROOT],
            "the verified successor carries the lineage-adjusted removal"
        );
        for (label, at) in [
            ("successor head", full::HEAD_IO),
            ("adjusted pending root", bc::LINEAGE_IO + bc::PENDING_ROOT),
            ("payer wallet context", bc::CONTEXT_IO + bc::WALLET),
            ("ArchiveSent statement", full::STATEMENT_IO),
            ("descriptor credit", ORIGINALS_IO + 2),
            ("transcript binding", ORIGINALS_IO + 9),
            ("Payment digest", ORIGINALS_IO + 12),
            ("exact receipt digest", ORIGINALS_IO + 13),
            ("form bit", ORIGINALS_IO + FORM),
            ("subject", ORIGINALS_IO + SUBJECT),
            ("completed Request word", ORIGINALS_IO + COMPLETED),
            ("exact statement digest", ORIGINALS_IO + STATEMENT_DIGEST),
            ("exact pair digest", ORIGINALS_IO + PAIR_DIGEST),
            ("receiver profile digest", ORIGINALS_IO + PROFILE_DIGEST),
        ] {
            let mut changed = ip.clone();
            changed.io[at] += Fp::ONE;
            assert!(!verify(&changed, &pp.to_bytes()), "{label}");
        }
        // The Receive form's public input at the same payer head and successor is not
        // this proof's, nor is the earlier payer head's.
        let as_receive = Instance {
            io: verifier_io(received, &removal),
        };
        assert_ne!(as_receive.io, ip.io);
        assert!(
            !verify(&as_receive, &pp.to_bytes()),
            "Receive-form public input"
        );
        let crossed = Inputs {
            current: send,
            released: sent,
            ..*headed
        };
        let at_earlier = Instance {
            io: verifier_io(&crossed, &crossed.case(nonce).unwrap()),
        };
        assert!(!verify(&at_earlier, &pp.to_bytes()), "earlier payer head");
        // Λ_archive's own deferred values decide natively: both matrix accumulators and
        // both inherited generator accumulators. (The pair has no installed selector;
        // `kind` is not read by these decisions.)
        let pair = full::UniformPair {
            cp: vp_program.clone(),
            ip: ip.clone(),
            pp,
            cs: vs_program.clone(),
            is: is.clone(),
            ps,
            d,
            rho,
            r,
            kind: full::RECEIVE,
        };
        let claim = crate::deferral::claim_of(&pair.pp.late[..=n]);
        assert_eq!(gens.defer_p.eval(&claim.point), claim.value);
        crate::native::engine::decide_accumulators(gens, &pair).unwrap();
        eprintln!(
            "Lambda_archive linked pair (later receiver Send): P {} B, S {} B, verifier programs {:.1}s, prove {:.1}s, verify {:.2}s, P program {}, S program {}",
            pair.pp.to_bytes().len(),
            pair.ps.to_bytes().len(),
            verifier_secs,
            prove_secs,
            verify_secs,
            blake3::Hash::from(vp_program.program_digest()).to_hex(),
            blake3::Hash::from(vs_program.program_digest()).to_hex(),
        );
    });
}

/// Qualify the 64-word receiver projection without admitting a fifth payer program.
#[test]
#[ignore = "requires retained wallet originals and full two-curve proving"]
fn wallet_projection_proves_both_heads_and_their_exact_noops() {
    use crate::spartan::{Link, verify_linked_circuit};
    use std::sync::Arc;
    with_both_forms(|forms, _, _, params| {
        let gens = &params.inner.generators;
        let mut programs = None;
        for (form, payer) in FORMS.iter().zip(forms) {
            let slots = verified_slots(payer, params);
            let folds =
                full::GeneratorFolds::prepare(gens, &slots, &ProofEntropy::fresh().unwrap())
                    .unwrap();
            let (removal, noop) = cases(payer);
            let genuine = delivered_receipt(payer.delivery).unwrap();
            let mut invalid = genuine.clone();
            invalid.opening.as_mut().unwrap().siblings[31] += Fp::ONE;
            for (branch, case, receipt, member) in [
                ("removal", &removal, &genuine, Fp::ONE),
                ("no-op", &noop, &invalid, Fp::ZERO),
            ] {
                let start = std::time::Instant::now();
                let lambda =
                    primary_with_projection(payer, params, &slots, &folds, case, receipt, true);
                let second = secondary(params, &slots, &folds);
                assert_eq!(lambda.derived, [Fp::ONE, member, Fp::ONE, Fp::ONE]);
                let mut public = verifier_io(payer, case);
                public.truncate(64);
                let mut opposite = public.clone();
                if branch == "no-op" {
                    public[full::STATEMENT_IO] = changed_opening_statement(payer, case);
                } else {
                    opposite[full::STATEMENT_IO] = changed_opening_statement(payer, case);
                }
                assert_ne!(public[full::STATEMENT_IO], opposite[full::STATEMENT_IO]);
                assert_eq!(
                    lambda.p.1.io, public,
                    "{form} {branch}: independently rebuilt payer statement"
                );
                assert_eq!(second.1.io, verifier_s_io());
                let (pp, ps, d, rho, r) = prove_pair(gens, &lambda.p, &second);
                let cp = Arc::new(lambda.p.0.into_verifier_program());
                let cs = Arc::new(second.0.into_verifier_program());
                let ids = [cp.program_digest(), cs.program_digest()];
                assert_eq!(
                    *programs.get_or_insert(ids),
                    ids,
                    "one verifier for both forms and branches"
                );
                let pair = full::UniformPair {
                    cp,
                    ip: Instance { io: public },
                    pp,
                    cs,
                    is: Instance {
                        io: verifier_s_io(),
                    },
                    ps,
                    d,
                    rho,
                    r,
                    kind: full::RECEIVE,
                };
                let fixed_r = |_: &[u8; 32]| r;
                assert_eq!(
                    verify_linked_circuit(
                        &pair.cs.shape,
                        &gens.s,
                        &pair.is,
                        &pair.ps.to_bytes(),
                        &Link::Second { d, rlc: &fixed_r },
                        &*pair.cs
                    )
                    .unwrap(),
                    rho
                );
                let verify = |i: &Instance<Fp>| {
                    verify_linked_circuit(
                        &pair.cp.shape,
                        &gens.p,
                        i,
                        &pair.pp.to_bytes(),
                        &Link::First { rho, r },
                        &*pair.cp,
                    )
                };
                assert_eq!(verify(&pair.ip).unwrap(), d);
                assert!(
                    verify(&Instance { io: opposite }).is_err(),
                    "same proof cannot rename its opening"
                );
                crate::native::engine::decide_accumulators(gens, &pair).unwrap();
                eprintln!(
                    "ArchiveSent wallet projection {form} {branch}: P/S ordinary=64/64 proofs={}/{} B programs={} {} elapsed={:.2}s",
                    pair.pp.to_bytes().len(),
                    pair.ps.to_bytes().len(),
                    blake3::Hash::from(ids[0]),
                    blake3::Hash::from(ids[1]),
                    start.elapsed().as_secs_f64()
                );
            }
        }
    });
}

#[path = "archive_program_compiler.rs"]
mod compiler;
