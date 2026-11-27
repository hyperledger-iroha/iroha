//! Successor component for the committed and lineage-adjusted ArchiveSent state effects.
//! Its public evidence inputs must come from the receiver-proof, exact evidence,
//! signature and deferred-claim verifiers when composing a complete ArchiveSent.
use super::{field, reverse_insertion};
use crate::{
    bootstrap_context as bc,
    certificate::Cells,
    full::Wiring,
    monetary_state,
    operation_auth::tests::{Built, expose, finish, holds, proof},
    send_arithmetic::Source,
    send_maps::PENDING_DOMAIN,
    uniform::{Array, constant, own},
};
use ff::{Field, PrimeField};
use iroha_data_model::kagemusha::{KagemushaWalletIndexedTreeV1, KagemushaWalletOperationKindV1};
use iroha_pasta::{Fp, poseidon};

const SEQUENCE: usize = 11;
const PENDING: usize = 18;
const NONCE: usize = 29;

use crate::native::engine::transfers::archive_sent::relation::{
    ArchiveInputs, ArchiveState, Case, State, append, credit_opening, receiver_signature,
    request_original, source,
};

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
