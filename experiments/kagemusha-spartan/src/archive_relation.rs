//! The compiled two-slot ArchiveSent relation shared by wallet execution and the offline compiler.
use crate::{
    bootstrap_context as bc,
    certificate::{self, Cells},
    fs::SpikeRng,
    full::{self, ArchiveIncoming, VerifiedPredecessor, Wiring},
    indexed_map::{self, InsertWitness, Leaf, Path},
    monetary_state::{self, StateCells},
    native::{self, Error, Result, field},
    operation_auth::expose,
    send_arithmetic::Source,
    send_maps::PENDING_DOMAIN,
    uniform::{Array, constant, own, plus, scaled},
};
use ff::{Field, PrimeField};
use iroha_data_model::kagemusha::{
    KagemushaWalletIndexedOpeningV1, KagemushaWalletIndexedRemoveV1, KagemushaWalletOperationKindV1,
};
use iroha_pasta::{Fp, poseidon};
use std::cell::RefCell;
const SEQUENCE: usize = 11;
const PENDING: usize = 18;
const NONCE: usize = 29;
#[path = "archive_credit_opening_successor.rs"]
pub(in crate::native::engine) mod credit_opening;
#[path = "archive_receipt_successor.rs"]
pub(in crate::native::engine) mod receiver_signature;
#[path = "archive_request_successor.rs"]
pub(in crate::native::engine) mod request_original;
#[path = "archive_status_transcript.rs"]
pub(in crate::native::engine) mod transcript;
#[path = "archive_recursive_wire_successor.rs"]
mod wire;
use receiver_signature::ReceiverReceipt;

// A change at another slot changes one sibling on this path, at the highest
// differing index bit. The subtree below that fork keeps all of its old siblings.
fn subtree(mut hash: Fp, path: &Path<32>, levels: usize) -> Fp {
    for (h, sibling) in path.siblings.iter().take(levels).enumerate() {
        let pair = if (path.index >> h) & 1 == 0 {
            [hash, *sibling]
        } else {
            [*sibling, hash]
        };
        hash = poseidon::hash_with_domain(indexed_map::NODE_DOMAIN, &pair);
    }
    hash
}

fn path(original: &KagemushaWalletIndexedOpeningV1) -> Result<Path<32>> {
    let mut siblings = [Fp::ZERO; 32];
    for (dst, raw) in siblings.iter_mut().zip(&original.siblings) {
        *dst = field(raw)?;
    }
    Ok(Path {
        index: original.slot,
        siblings,
    })
}

pub(in crate::native::engine) fn reverse_insertion(
    before: &[u8; 32],
    key: &[u8; 32],
    value: &[u8; 32],
    removal: &KagemushaWalletIndexedRemoveV1,
) -> Result<InsertWitness<32>> {
    let after = removal
        .verify(before, key)
        .map_err(|_| Error::Malformed("indexed removal openings"))?;
    if removal.leaf.value != *value {
        return Err(Error::AuthorityMismatch("pending descriptor"));
    }
    let mut low_path = path(&removal.predecessor_opening)?;
    let mut slot = path(&removal.leaf_opening)?;
    let fork = (low_path.index ^ slot.index).ilog2() as usize; // verify checked distinct slots
    let low = Leaf {
        key: field(&removal.predecessor.key)?,
        value: field(&removal.predecessor.value)?,
        next: field(&removal.leaf.next_key)?,
    };
    let key = field(key)?;
    // Insertion opens the predecessor *after* deletion, and the vacant slot
    // after relinking the predecessor back to the removed key.
    let original_low = indexed_map::leaf_hash(Leaf { next: key, ..low });
    low_path.siblings[fork] = subtree(Fp::ZERO, &slot, fork);
    slot.siblings[fork] = subtree(original_low, &low_path, fork);
    Ok(InsertWitness {
        old_root: field(&after)?,
        key,
        value: field(value)?,
        low,
        low_path,
        slot,
    })
}

pub(in crate::native::engine) fn source(array: usize, cell: usize) -> Source {
    Source {
        array,
        cell,
        instance: 0,
    }
}

pub(in crate::native::engine) struct ArchiveState<'a> {
    pub(in crate::native::engine) state: &'a StateCells,
    pub(in crate::native::engine) lineage: [Source; bc::LINEAGE_FIELDS],
}

pub(in crate::native::engine) struct ArchiveInputs {
    pub(in crate::native::engine) tag: Source,
    pub(in crate::native::engine) credit: Source,
    pub(in crate::native::engine) descriptor: [Source; 7],
    pub(in crate::native::engine) results: [Source; 4],
}

/// Both map paths are hard constraints, even on the adjusted no-op branch.
/// The three evidence results and the absence of a required correction are
/// separately exposed composition ports. There is no independent branch selector.
pub(in crate::native::engine) fn append(
    w: &mut Wiring<Fp>,
    family: usize,
    before: &ArchiveState<'_>,
    after: &ArchiveState<'_>,
    inputs: ArchiveInputs,
    core_path: &InsertWitness<32>,
    adjusted_path: &InsertWitness<32>,
) {
    let ArchiveInputs {
        tag,
        credit,
        descriptor,
        results,
    } = inputs;
    let core = indexed_map::append_insert(w, family, core_path);
    let adjusted = indexed_map::append_insert(w, family, adjusted_path);
    let fields = descriptor.map(|s| (s.value(w), s.linear()));
    let hash = poseidon::hash_with_domain(PENDING_DOMAIN, &descriptor.map(|s| s.value(w)));
    let mut c = Cells::new(family);
    c.a.name = "ArchiveSent committed and adjusted pending effects";
    c.eq(
        tag.linear(),
        constant(vec![Fp::from(u64::from(
            KagemushaWalletOperationKindV1::ArchiveSent.tag(),
        ))]),
    );
    c.eq(credit.linear(), descriptor[0].linear());
    let digest = [
        c.cell(Fp::ZERO, 255),
        c.cell(hash, 255),
        c.cell(Fp::ZERO, 255),
    ];
    for map in [&core, &adjusted] {
        c.eq(source(map.local, map.key).linear(), descriptor[0].linear());
        c.eq(source(map.local, map.value).linear(), own(digest[1], 1));
    }
    // The insertion gadget runs in reverse: its new root contains the descriptor.
    c.eq(
        source(core.local, core.new_root).linear(),
        source(before.state.local, before.state.core[PENDING]).linear(),
    );
    c.eq(
        source(core.local, core.old_root).linear(),
        source(after.state.local, after.state.core[PENDING]).linear(),
    );
    c.eq(
        source(adjusted.local, adjusted.new_root).linear(),
        before.lineage[bc::PENDING_ROOT].linear(),
    );
    let bits = results.map(|s| {
        let b = c.bit(s.value(w) == Fp::ONE);
        c.eq(own(b, 1), s.linear());
        b
    });
    let accepted = bits[1..].iter().fold(bits[0], |acc, &bit| {
        let product = c.cell(c.a.values[acc][0] * c.a.values[bit][0], 1);
        c.a.rows.push([own(acc, 1), own(bit, 1), own(product, 1)]);
        product
    });
    let old = before.lineage[bc::PENDING_ROOT].linear();
    c.a.rows.push([
        own(accepted, 1),
        plus(
            &source(adjusted.local, adjusted.old_root).linear(),
            &scaled(&old, -Fp::ONE),
        ),
        plus(
            &after.lineage[bc::PENDING_ROOT].linear(),
            &scaled(&old, -Fp::ONE),
        ),
    ]);
    let b = |i| source(before.state.local, before.state.core[i]).linear();
    let a = |i| source(after.state.local, after.state.core[i]).linear();
    c.eq(a(SEQUENCE), plus(&b(SEQUENCE), &constant(vec![Fp::ONE])));
    // Both canonical openings constrain uint128 sequences and nonzero state nonces.
    for i in 0..monetary_state::CORE_FIELDS {
        if !matches!(i, SEQUENCE | NONCE | PENDING) {
            c.eq(b(i), a(i));
        }
    }
    for (&old, &new) in before.state.rest.iter().zip(&after.state.rest) {
        c.eq(
            source(before.state.local, old).linear(),
            source(after.state.local, new).linear(),
        );
    }
    for i in 0..bc::LINEAGE_FIELDS {
        if i != bc::PENDING_ROOT {
            c.eq(before.lineage[i].linear(), after.lineage[i].linear());
        }
    }
    for (i, j) in [
        (bc::LIFECYCLE, 0),
        (bc::POLICY_EPOCH, 27),
        (bc::ENABLED_CONTROLS, 22),
    ] {
        c.eq(before.lineage[i].linear(), b(j));
    }
    let local = w.push(c.a);
    certificate::hash_relation(
        w,
        family,
        local,
        PENDING_DOMAIN,
        &fields,
        digest,
        &mut SpikeRng::new(b"ArchiveSent pending descriptor"),
    );
}

#[derive(Clone)]
pub(in crate::native::engine) struct State {
    pub(in crate::native::engine) core: [Fp; 30],
    pub(in crate::native::engine) rest: [Fp; 6],
    pub(in crate::native::engine) lineage: [Fp; bc::LINEAGE_FIELDS],
}
#[derive(Clone)]
pub(in crate::native::engine) struct Case {
    pub(in crate::native::engine) before: State,
    pub(in crate::native::engine) after: State,
    pub(in crate::native::engine) tag: Fp,
    pub(in crate::native::engine) credit: Fp,
    pub(in crate::native::engine) descriptor: [Fp; 7],
    pub(in crate::native::engine) results: [Fp; 4],
    pub(in crate::native::engine) core_path: InsertWitness<32>,
    pub(in crate::native::engine) adjusted_path: InsertWitness<32>,
}

pub(in crate::native::engine) fn receiver_receipt_fields(
    evidence: &crate::native::sent_delivery::ReceiverEvidence,
    payment_statement: &[Fp; 26],
    payment_request: &crate::request::CompletedRequest,
    payment_digest: Fp,
    outcome: native::VerifiedCreditStatus,
) -> Result<receiver_signature::ReceiverReceipt> {
    use crate::{credential, deferral, full, payment_wire::limbs, receipt, statement};
    let p = &evidence.profile;
    let s = &evidence.statement;
    let tag = |kind: KagemushaWalletOperationKindV1| Fp::from(u64::from(kind.tag()));
    let (send, program) = if s[16] == tag(KagemushaWalletOperationKindV1::Receive) {
        (false, full::RECEIVE)
    } else if s[16] == tag(KagemushaWalletOperationKindV1::Send) {
        (true, full::SEND)
    } else {
        return Err(Error::AuthorityMismatch("ArchiveSent receiver head kind"));
    };
    // Native and in-circuit acceptance must agree. The decided profile names the same
    // program, and its head and statement digest are this statement's. The head receipt
    // signs the operation id of this kind and credit. A Receive names this Payment's credit.
    let (a, b) = deferral::kind_bits(program);
    let operation = poseidon::hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[
            p[bc::CONTEXT_IO + bc::WALLET],
            p[bc::CONTEXT_IO + bc::WALLET + 1],
            s[16],
            s[17],
        ],
    );
    if [p[full::KIND_IO], p[full::KIND_IO + 1]] != [Fp::from(a), Fp::from(b)]
        || p[full::HEAD_IO] != s[15]
        || p[full::STATEMENT_IO] != statement::digest(s)
        || evidence.receipt.body[114..146] != operation.to_repr()
        || (!send && s[17] != payment_statement[17])
    {
        return Err(Error::AuthorityMismatch(
            "ArchiveSent receiver head evidence",
        ));
    }
    // The opened statement's semantic predicates. The signed receipt names this statement and
    // carries its sequence, predecessor and successor. The statement names the quoted scheme
    // and asset. A Receive effect names the Request's payer wallet and the Payment amount and
    // is zero-filled.
    let body = &evidence.receipt.body;
    let sequence = s[9].to_repr();
    let quoted = &evidence.credential.body;
    let request = &payment_request.body;
    if body[98..114] != sequence[..16]
        || sequence[16..] != [0; 16]
        || body[146..178] != s[14].to_repr()
        || body[178..210] != s[15].to_repr()
        || body[210..242] != statement::digest(s).to_repr()
        || s[3..5] != limbs(&quoted[2..34])
        || s[5..7] != limbs(&quoted[34..66])
        || (!send
            && (s[18..20] != limbs(&request[66..98])
                || s[20] != payment_statement[21]
                || s[21..].iter().any(|word| !word.is_zero_vartime())))
    {
        return Err(Error::AuthorityMismatch(
            "ArchiveSent receiver head statement",
        ));
    }
    let expected = [
        credential::digest(&evidence.credential),
        p[full::RECEIPT_IO],
        p[full::HEAD_IO],
        p[full::STATEMENT_IO],
        payment_digest,
        receipt::digest(&evidence.receipt),
        p[bc::CONTEXT_IO + bc::WALLET],
        p[bc::CONTEXT_IO + bc::WALLET + 1],
        p[bc::CONTEXT_IO + bc::PAYMENT_KEY],
        p[bc::CONTEXT_IO + bc::PAYMENT_KEY + 1],
        p[bc::CONTEXT_IO + bc::PAYMENT_KEY + 2],
        p[bc::CONTEXT_IO + bc::PAYMENT_KEY + 3],
    ];
    Ok(receiver_signature::ReceiverReceipt {
        request: Some(payment_request.clone()),
        opening: Some(credit_opening::StatusOpening {
            root: p[bc::LINEAGE_IO + bc::CREDIT_ROOT],
            burned: matches!(outcome, native::VerifiedCreditStatus::Burned),
            next: evidence.next,
            index: evidence.slot,
            siblings: evidence.siblings,
        }),
        credential: evidence.credential.clone(),
        receipt: evidence.receipt.clone(),
        expected,
        send,
        subject: s[17],
        // The verifier's own reconstruction of the exact original that the circuit opens.
        statement: *s,
        statement_digest: statement::digest(s),
    })
}

/// Payer-held and receiver exact originals the native verifier supplies. The primary's
/// io already holds the profile (0..64), the folded matrix claim (from `full::LATE_IO`,
/// below the link), the link (124..128) and the inherited generator claim (from
/// `generator_claim::IO_START`, through 192).
pub(in crate::native::engine) const ORIGINALS_IO: usize = 192;
// `IO_START` is aligned to the generator claim's power-of-two width, so it ends by 192.
const _: () = assert!(ORIGINALS_IO >= crate::generator_claim::IO_START + 32);
/// Offsets inside the originals block: tag, credit, descriptor (7), transcript binding (2),
/// quoted credential digest, Payment digest, exact receipt digest, form bit, subject.
pub(in crate::native::engine) const FORM: usize = 14;
pub(in crate::native::engine) const SUBJECT: usize = 15;
/// The packed open and completed Request words.
pub(in crate::native::engine) const OPEN: usize = 16;
pub(in crate::native::engine) const COMPLETED: usize = 32;
/// The receiver statement, raw pair and primary profile digests.
pub(in crate::native::engine) const EXACT: usize = 48;
pub(in crate::native::engine) const STATEMENT_DIGEST: usize = EXACT;
pub(in crate::native::engine) const PAIR_DIGEST: usize = EXACT + 1;
pub(in crate::native::engine) const PROFILE_DIGEST: usize = EXACT + 2;
/// The remaining exact Request and Payment diagnostics still exceed the wallet profile.
pub(in crate::native::engine) const P_IO: usize = 256;
const _: () = assert!(ORIGINALS_IO + PROFILE_DIGEST < P_IO);
pub(in crate::native::engine) const S_IO: usize = 64;

#[derive(Clone, Copy)]
pub(in crate::native::engine) enum Projection<'a> {
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
    pub(in crate::native::engine) fn append_verified(
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
        if let Projection::Completed {
            credential,
            receipt,
        } = projection
        {
            let current =
                crate::operation_auth::append_current(w, family, credential, &prior, predecessor)?;
            let authenticated = crate::receipt::append_own(
                w,
                family,
                receipt,
                &current,
                &statement,
                crate::statement::Kind::Archive,
                crate::receipt::PaymentDigest::Absent,
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
pub(in crate::native::engine) struct Composed<'a> {
    pub(in crate::native::engine) case: &'a Case,
    pub(in crate::native::engine) binding: [Fp; 2],
    pub(in crate::native::engine) receiver: &'a ReceiverReceipt,
    pub(in crate::native::engine) derived: RefCell<Option<[Fp; 4]>>,
    pub(in crate::native::engine) projection: Projection<'a>,
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
