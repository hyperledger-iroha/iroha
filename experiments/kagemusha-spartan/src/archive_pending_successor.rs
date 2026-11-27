//! Successor component for the committed and lineage-adjusted ArchiveSent state effects.
//! Its public evidence inputs must come from the receiver-proof, exact evidence,
//! signature and deferred-claim verifiers when composing a complete ArchiveSent.
use super::{field, reverse_insertion};
use crate::{
    bootstrap_context as bc,
    certificate::{self, Cells},
    fs::SpikeRng,
    full::Wiring,
    indexed_map::{self, InsertWitness},
    monetary_state::{self, StateCells},
    operation_auth::tests::{Built, expose, finish, holds, proof},
    send_arithmetic::Source,
    send_maps::PENDING_DOMAIN,
    uniform::{Array, constant, own, plus, scaled},
};
use ff::{Field, PrimeField};
use iroha_data_model::kagemusha::{KagemushaWalletIndexedTreeV1, KagemushaWalletOperationKindV1};
use iroha_pasta::{Fp, poseidon};

const SEQUENCE: usize = 11;
const PENDING: usize = 18;
const NONCE: usize = 29;

fn source(array: usize, cell: usize) -> Source {
    Source {
        array,
        cell,
        instance: 0,
    }
}

struct ArchiveState<'a> {
    state: &'a StateCells,
    lineage: [Source; bc::LINEAGE_FIELDS],
}

struct ArchiveInputs {
    tag: Source,
    credit: Source,
    descriptor: [Source; 7],
    results: [Source; 4],
}

/// Both map paths are hard constraints, even on the adjusted no-op branch.
/// The three evidence results and the absence of a required correction are
/// separately exposed composition ports. There is no independent branch selector.
fn append(
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
struct State {
    core: [Fp; 30],
    rest: [Fp; 6],
    lineage: [Fp; bc::LINEAGE_FIELDS],
}
#[derive(Clone)]
struct Case {
    before: State,
    after: State,
    tag: Fp,
    credit: Fp,
    descriptor: [Fp; 7],
    results: [Fp; 4],
    core_path: InsertWitness<32>,
    adjusted_path: InsertWitness<32>,
}

impl Case {
    fn credited() -> Self {
        let send = crate::send::tests::fixture(0, false).input;
        let descriptor: [Fp; 7] = send.statement[17..24].try_into().unwrap();
        let key = descriptor[0].to_repr();
        let hash = poseidon::hash_with_domain(PENDING_DOMAIN, &descriptor).to_repr();
        let remove = |mut tree: KagemushaWalletIndexedTreeV1| {
            tree.insert(key, hash).unwrap();
            let start = tree.root();
            let removal = tree.remove(&key).unwrap();
            let inverse = reverse_insertion(&start, &key, &hash, &removal).unwrap();
            (
                field(&start).unwrap(),
                field(&tree.root()).unwrap(),
                inverse,
            )
        };
        let (core_old, core_new, core_path) = remove(KagemushaWalletIndexedTreeV1::new());
        let mut retained = KagemushaWalletIndexedTreeV1::new();
        retained
            .insert(Fp::from(7).to_repr(), Fp::from(121).to_repr())
            .unwrap();
        let (line_old, line_new, adjusted_path) = remove(retained);
        assert_ne!(core_old, line_old);
        assert_eq!(send.after_core[PENDING], core_old);
        let before = State {
            core: send.after_core,
            rest: send.after_rest,
            lineage: [
                send.after_core[0],
                send.after_core[27],
                send.after_core[22],
                send.after_core[10] + Fp::from(2),
                line_old,
                Fp::from(51),
            ],
        };
        let mut after = before.clone();
        after.core[SEQUENCE] += Fp::ONE;
        after.core[NONCE] += Fp::ONE;
        after.core[PENDING] = core_new;
        after.lineage[bc::PENDING_ROOT] = line_new;
        Self {
            before,
            after,
            tag: Fp::from(u64::from(KagemushaWalletOperationKindV1::ArchiveSent.tag())),
            credit: descriptor[0],
            descriptor,
            results: [Fp::ONE; 4],
            core_path,
            adjusted_path,
        }
    }
    fn noop(&self, failed: usize) -> Self {
        let mut result = self.clone();
        result.results[failed] = Fp::ZERO;
        result.after.lineage[bc::PENDING_ROOT] = self.before.lineage[bc::PENDING_ROOT];
        result
    }
    fn build(&self) -> Built {
        self.build_with_binding(None)
    }
    fn build_with_binding(&self, binding: Option<[Fp; 2]>) -> Built {
        self.build_for_receipt(binding, None)
    }
    fn build_for_receipt(
        &self,
        binding: Option<[Fp; 2]>,
        receipt: Option<&receiver_signature::ReceiverReceipt>,
    ) -> Built {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let prior =
            monetary_state::append_state(&mut w, family, &self.before.core, &self.before.rest);
        let next = monetary_state::append_state(&mut w, family, &self.after.core, &self.after.rest);
        let mut c = Cells::new(family);
        c.a.name = "ArchiveSent component public inputs";
        let tag = c.cell(self.tag, 255);
        let credit = c.cell(self.credit, 255);
        let descriptor = self.descriptor.map(|v| c.cell(v, 255));
        let results = self.results.map(|v| c.cell(v, 255));
        let before = self.before.lineage.map(|v| c.cell(v, 255));
        let after = self.after.lineage.map(|v| c.cell(v, 255));
        let binding = binding.map(|words| words.map(|word| c.cell(word, 255)));
        let local = w.push(c.a);
        let s = |k| source(local, k);
        let prior = ArchiveState {
            state: &prior,
            lineage: before.map(s),
        };
        let next = ArchiveState {
            state: &next,
            lineage: after.map(s),
        };
        let descriptor = descriptor.map(s);
        let mut results = results.map(s);
        let signature =
            receipt.map(|receipt| receipt.append(&mut w, family, descriptor, results[1]));
        if let Some(ports) = &signature {
            results[1] = ports.evidence;
            results[2] = ports.signature;
        }
        append(
            &mut w,
            family,
            &prior,
            &next,
            ArchiveInputs {
                tag: s(tag),
                credit: s(credit),
                descriptor,
                results,
            },
            &self.core_path,
            &self.adjusted_path,
        );
        let mut public = vec![
            source(prior.state.local, prior.state.commitment),
            source(next.state.local, next.state.commitment),
        ];
        public.extend(prior.lineage);
        public.extend(next.lineage);
        public.extend([s(tag), s(credit)]);
        public.extend(descriptor);
        public.extend(results);
        if let Some(binding) = binding {
            public.extend(binding.map(s));
        }
        expose(&mut w, family, 32, &public);
        if let Some(ports) = signature {
            expose(&mut w, family, 64, &ports.public);
            if let Some(q) = ports.request {
                expose(&mut w, family, 80, &q.open);
                expose(&mut w, family, 96, &q.completed);
            }
            if !ports.opening.is_empty() {
                expose(&mut w, family, 128, &ports.opening);
            }
            return finish(w, 16);
        }
        let range = w.push(Array {
            name: "ArchiveSent ranges",
            family,
            rho: 11,
            r_count: 2048,
            fixed: true,
            lookups: vec![],
            widths: vec![11],
            rows: vec![],
            values: vec![(0..2048u64).map(Fp::from).collect()],
        });
        w.tables.push((range, crate::nonnative::RANGE_TABLE));
        finish(w, 12)
    }
}

#[test]
fn successor_archive_pending_effect_proves_removal_and_required_noop() {
    let accepted = Case::credited();
    let positive = accepted.build();
    assert!(holds(&positive), "accepted ArchiveSent state constraints");
    let fingerprint = positive.0.program_digest();
    eprintln!(
        "ArchiveSent pending effect program={} sx={} sz={} cols={}",
        blake3::Hash::from(fingerprint).to_hex(),
        positive.0.shape.sx,
        positive.0.shape.sz(),
        positive.0.shape.cols()
    );
    assert!(proof(&positive), "accepted ArchiveSent state proof");
    drop(positive);
    for failed in 0..4 {
        let branch = accepted.noop(failed).build();
        assert_eq!(branch.0.program_digest(), fingerprint);
        assert!(holds(&branch));
        if failed == 3 {
            assert!(proof(&branch));
        }
    }
}

#[test]
fn successor_archive_pending_effect_refuses_unrelated_changes_and_discretionary_branches() {
    let accepted = Case::credited();
    let changes: &[(&str, fn(&mut Case))] = &[
        ("operation tag", |x| x.tag = Fp::from(4)),
        ("operation credit", |x| x.credit += Fp::ONE),
        ("payer balance", |x| x.after.core[9] += Fp::ONE),
        ("quota usage", |x| x.after.core[21] += Fp::ONE),
        ("fee claim", |x| x.after.core[20] += Fp::ONE),
        ("consumed root", |x| x.after.core[17] += Fp::ONE),
        ("policy", |x| x.after.rest[2] += Fp::ONE),
        ("sequence", |x| x.after.core[SEQUENCE] += Fp::ONE),
        ("burned total", |x| {
            x.after.lineage[bc::BURNED_TOTAL] += Fp::ONE
        }),
        ("credit root", |x| {
            x.after.lineage[bc::CREDIT_ROOT] += Fp::ONE
        }),
        ("wrong descriptor field", |x| x.descriptor[6] += Fp::ONE),
        ("nonboolean evidence", |x| x.results[1] = Fp::from(2)),
        ("accepted evidence retains descriptor", |x| {
            x.after.lineage[bc::PENDING_ROOT] = x.before.lineage[bc::PENDING_ROOT]
        }),
        ("uncorrected removal after failed evidence", |x| {
            x.results[1] = Fp::ZERO
        }),
    ];
    for (label, change) in changes {
        let mut case = accepted.clone();
        change(&mut case);
        assert!(!holds(&case.build()), "{label}");
    }
    let mut noop = accepted.noop(2);
    noop.core_path = accepted.adjusted_path.clone();
    assert!(
        !holds(&noop.build()),
        "no-op must authenticate and update the committed core"
    );
    let mut noop = accepted.noop(2);
    noop.adjusted_path = accepted.core_path.clone();
    assert!(
        !holds(&noop.build()),
        "no-op must still prove the adjusted opening"
    );
}

#[path = "archive_authenticated_successor.rs"]
mod authenticated;

#[path = "archive_receipt_successor.rs"]
mod receiver_signature;

#[path = "archive_credit_opening_successor.rs"]
mod credit_opening;

#[path = "archive_request_successor.rs"]
mod request_original;
