//! Bounded scoped observations of actual q77 canonical H and atomic G calls.
//!
//! Deterministic fixtures only. Each copied body/tape is immediately guarded.
//! Device parity covers exactly the tree continuation path used in production;
//! SHAKE tapes and non-tree transcript H stay scalar and are checked one-shot.
use super::*;
use crate::field::GoldilocksFp4V1 as F;
use crate::{
    ProofSemantics,
    backend::{
        compact_axt_batch::AxtTransferBatch,
        compact_protocol::FixedAir,
        compact_public_batch::{BatchContextLimits, PublicTransferBatch},
        compact_quantity_tests::{QuantityCase, QuantityFixture},
        compact_value_domain::CompactTransferValue,
        deep_binding::{Context as Binding, Message, Oracle, Transcript},
        deep_relation::DeepRelation,
    },
};
use iroha_data_model::fastpq::FastpqQuantityUnits;
use std::{cell::RefCell, collections::BTreeSet};
const MAX_OBSERVATIONS: usize = 2048;
const MAX_CAPTURE_BYTES: usize = 4 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Hash {
        kind: u8,
        oracle: u8,
        round: u8,
        level: u32,
        index: u32,
        first_field_address: usize,
    },
    Tape {
        round: u8,
        raw_address: usize,
    },
}
struct Observation {
    prefix: Arc<AbsorbedPrefix>,
    body: SecretPolynomial<u8>,
    raw: SecretPolynomial<u8>,
    kind: Kind,
    digest: Option<Digest>,
}
#[derive(Default)]
struct Capture {
    jobs: Vec<Observation>,
    bytes: usize,
}
thread_local! {static CAPTURE:RefCell<Option<Capture>>=const{RefCell::new(None)};}
fn record(
    prefix: &Arc<AbsorbedPrefix>,
    body: &[u8],
    raw: &[u8],
    kind: Kind,
    digest: Option<Digest>,
) {
    CAPTURE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(capture) = slot.as_mut() else {
            return;
        };
        assert_eq!(
            matches!(kind, Kind::Hash { .. }),
            digest.is_some(),
            "hash observation requires exactly one digest"
        );
        assert!(
            capture.jobs.len() < MAX_OBSERVATIONS,
            "bounded observation count"
        );
        let bytes = capture
            .bytes
            .checked_add(body.len())
            .and_then(|v| v.checked_add(raw.len()))
            .unwrap();
        assert!(bytes <= MAX_CAPTURE_BYTES, "bounded observation payload");
        capture.jobs.push(Observation {
            prefix: prefix.clone(),
            body: SecretPolynomial::from_slice(body).unwrap(),
            raw: SecretPolynomial::from_slice(raw).unwrap(),
            kind,
            digest,
        });
        capture.bytes = bytes;
    });
}
pub(super) fn hash(prefix: &Arc<AbsorbedPrefix>, frame: &Frame<'_>, body: &[u8], digest: Digest) {
    let first_field_address = match frame.fields {
        BodyFields::One(first) | BodyFields::Two(first, _) => first.as_ptr() as usize,
    };
    record(
        prefix,
        body,
        &[],
        Kind::Hash {
            kind: frame.kind,
            oracle: frame.oracle,
            round: frame.round,
            level: frame.level,
            index: frame.position,
            first_field_address,
        },
        Some(digest),
    );
}
pub(super) fn tape(prefix: &Arc<AbsorbedPrefix>, round: RawTapeRoundV1, body: &[u8], raw: &[u8]) {
    record(
        prefix,
        body,
        raw,
        Kind::Tape {
            round: round.ordinal(),
            raw_address: raw.as_ptr() as usize,
        },
        None,
    );
}
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        CAPTURE.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}
fn capture<T>(call: impl FnOnce() -> T) -> (T, Capture) {
    CAPTURE.with(|slot| {
        let mut slot = slot.borrow_mut();
        assert!(slot.is_none(), "nonreentrant capture");
        *slot = Some(Capture::default());
    });
    let guard = Guard;
    let result = call();
    let capture = CAPTURE.with(|slot| slot.borrow_mut().take().unwrap());
    drop(guard);
    (result, capture)
}
fn count() -> usize {
    CAPTURE.with(|slot| slot.borrow().as_ref().unwrap().jobs.len())
}
fn assert_last_hash(kind: u8, oracle: u8, round: u8, level: u32, index: u32, expected: Digest) {
    CAPTURE.with(|slot| {
        let slot = slot.borrow();
        let actual = slot.as_ref().unwrap().jobs.last().unwrap();
        let Kind::Hash {
            kind: actual_kind,
            oracle: actual_oracle,
            round: actual_round,
            level: actual_level,
            index: actual_index,
            ..
        } = actual.kind
        else {
            panic!("hash observation")
        };
        assert_eq!(
            (
                actual_kind,
                actual_oracle,
                actual_round,
                actual_level,
                actual_index
            ),
            (kind, oracle, round, level, index)
        );
        let digest = actual.digest.expect("hash digest");
        assert_eq!(digest, expected);
    });
}
fn positions(count: usize) -> Vec<usize> {
    assert!(count > 0);
    BTreeSet::from([0, count / 2, count - 1])
        .into_iter()
        .collect()
}
fn child(seed: usize) -> Digest {
    Digest::from_bytes(core::array::from_fn(|i| {
        (seed * 17 + i * 73).to_le_bytes()[0]
    }))
}
const ORACLES: [Oracle; 8] = [
    Oracle::Row,
    Oracle::QuotientAndMask,
    Oracle::Fri(0),
    Oracle::Fri(1),
    Oracle::Fri(2),
    Oracle::Fri(3),
    Oracle::Fri(4),
    Oracle::Terminal,
];
fn exercise(relation: &impl DeepRelation) -> Capture {
    let binding = Binding::for_relation(relation).unwrap();
    assert_eq!(relation.schema().trace_rows, 65536);
    assert_eq!(relation.schema().width, 342);
    assert_eq!(relation.schema().constraints, 923);
    let ((), captured) = capture(|| {
        let mut roots = Vec::new();
        let mut coverage = BTreeSet::new();
        let mut leaf_count = 0;
        let mut parent_count = 0;
        for (ordinal, oracle) in ORACLES.into_iter().enumerate() {
            let (tag, round, leaves, width) = oracle.shape().unwrap();
            let expected_shapes = [
                (1, 0, 8_388_608, 2408),
                (3, 0, 8_388_608, 96),
                (4, 0, 524_288, 512),
                (4, 1, 32_768, 512),
                (4, 2, 4096, 256),
                (4, 3, 512, 256),
                (4, 4, 128, 128),
                (4, 5, 1, 4096),
            ];
            assert_eq!((tag, round, leaves, width), expected_shapes[ordinal]);
            let bytes = (0..width / 8)
                .flat_map(|i| {
                    if i % 2 == 0 {
                        0u64.to_le_bytes()
                    } else {
                        (fastpq_isi::poseidon::FIELD_MODULUS - 1).to_le_bytes()
                    }
                })
                .collect::<Vec<_>>();
            let mut terminal = None;
            for index in positions(leaves) {
                let before = count();
                terminal = Some(
                    binding
                        .hash_leaf(
                            oracle,
                            u32::try_from(index).expect("bounded oracle index"),
                            &bytes,
                        )
                        .unwrap(),
                );
                assert_eq!(count(), before + 1);
                assert_last_hash(
                    1,
                    tag,
                    round,
                    0,
                    u32::try_from(index).expect("bounded oracle index"),
                    terminal.unwrap(),
                );
                assert!(coverage.insert((ordinal, 0, index)));
                leaf_count += 1;
            }
            let mut root = None;
            for level in 1..=leaves.ilog2().max(1) {
                for index in positions((leaves >> level).max(1)) {
                    let left = if leaves == 1 {
                        terminal.unwrap()
                    } else {
                        child(index)
                    };
                    let right = if leaves == 1 { left } else { child(index + 1) };
                    let before = count();
                    root = Some(
                        binding
                            .hash_parent(
                                oracle,
                                level,
                                u32::try_from(index).expect("bounded oracle index"),
                                left,
                                right,
                            )
                            .unwrap(),
                    );
                    assert_eq!(count(), before + 1);
                    assert_last_hash(
                        2,
                        tag,
                        round,
                        level,
                        u32::try_from(index).expect("bounded oracle index"),
                        root.unwrap(),
                    );
                    assert!(coverage.insert((ordinal, level, index)));
                    parent_count += 1;
                }
            }
            roots.push(root.unwrap());
            let before = count();
            assert!(
                binding
                    .hash_leaf(
                        oracle,
                        u32::try_from(leaves).expect("bounded oracle leaves"),
                        &bytes
                    )
                    .is_err()
            );
            assert!(binding.hash_leaf(oracle, 0, &bytes[..width - 1]).is_err());
            let mut invalid = bytes;
            invalid[..8].copy_from_slice(&u64::MAX.to_le_bytes());
            assert!(binding.hash_leaf(oracle, 0, &invalid).is_err());
            assert!(
                binding
                    .hash_parent(oracle, 0, 0, child(0), child(1))
                    .is_err()
            );
            assert!(
                binding
                    .hash_parent(oracle, leaves.ilog2().max(1) + 1, 0, child(0), child(1))
                    .is_err()
            );
            assert!(
                binding
                    .hash_parent(
                        oracle,
                        1,
                        u32::try_from((leaves / 2).max(1)).expect("bounded parent index"),
                        child(0),
                        child(1)
                    )
                    .is_err()
            );
            if leaves == 1 {
                assert!(
                    binding
                        .hash_parent(oracle, 1, 0, child(0), child(1))
                        .is_err()
                );
            }
            assert_eq!(count(), before, "invalid typed geometry must not hash");
        }
        assert_eq!((leaf_count, parent_count, coverage.len()), (22, 304, 326));
        assert_eq!(roots.len(), 8);
        let mut transcript = Transcript::new(binding.clone());
        let zero = vec![F::ZERO; 301];
        for ordinal in 1_u8..=10 {
            let message = transcript.challenge().unwrap();
            match ordinal {
                1 => assert_eq!(message, Message::Dummy),
                2 => assert!(matches!(message,Message::Fields(v)if v.len()==923)),
                3..=9 => assert!(matches!(message,Message::Fields(v)if v.len()==1)),
                10 => {
                    let Message::Queries(q) = message else {
                        panic!("queries")
                    };
                    assert_eq!(q.len(), 77);
                    assert!(q.windows(2).all(|w| w[0] < w[1]));
                    assert!(q.iter().all(|&i| i < 8_388_608));
                }
                _ => unreachable!(),
            }
            if ordinal == 10 {
                break;
            }
            if ordinal == 3 {
                transcript.commit_ood(&zero, &zero, &[F::ZERO; 2]).unwrap();
            } else {
                let index = match ordinal {
                    1 => 0,
                    2 => 1,
                    4..=8 => usize::from(ordinal - 2),
                    9 => 7,
                    _ => unreachable!(),
                };
                transcript
                    .commit_root(ORACLES[index], roots[index])
                    .unwrap();
            }
        }
        let completed = count();
        assert!(transcript.challenge().is_err());
        assert!(transcript.commit_root(Oracle::Row, roots[0]).is_err());
        assert_eq!(count(), completed);
    });
    check_schedule(&captured, relation.statement_bytes().len());
    captured
}
fn check_schedule(capture: &Capture, statement_len: usize) {
    assert_eq!(capture.jobs.len(), 346);
    let mut hash_count = 0;
    let mut tape_count = 0;
    let mut chain_rounds = BTreeSet::new();
    let mut tape_rounds = BTreeSet::new();
    let mut chain_order = Vec::new();
    let mut tape_order = Vec::new();
    for job in &capture.jobs {
        assert!(Arc::ptr_eq(&job.prefix, &capture.jobs[0].prefix));
        assert!(job.prefix.encoded.len() > statement_len);
        match job.kind {
            Kind::Hash { kind, round, .. } => {
                let digest = job.digest.expect("hash digest");
                let mut reference = Sha3_256V1::new();
                reference.update(&job.prefix.encoded);
                reference.update(&job.body);
                assert_eq!(reference.finalize(), digest);
                hash_count += 1;
                if kind == 3 {
                    assert!(chain_rounds.insert(round));
                    chain_order.push(round);
                    let Kind::Hash {
                        oracle,
                        level,
                        index,
                        first_field_address,
                        ..
                    } = job.kind
                    else {
                        unreachable!()
                    };
                    assert_eq!((oracle, level, index), (0, 0, 0));
                    let tape=capture.jobs.iter().find(|entry|matches!(entry.kind,Kind::Tape{round:actual,..} if actual==round)).unwrap();
                    let Kind::Tape { raw_address, .. } = tape.kind else {
                        unreachable!()
                    };
                    assert_eq!(
                        first_field_address, raw_address,
                        "chain borrows the actual complete pending tape owner"
                    );
                }
            }
            Kind::Tape { round, .. } => {
                assert!(job.digest.is_none());
                let descriptor = RawTapeRoundV1::new(round).unwrap();
                assert_eq!(job.raw.len(), descriptor.tape_bytes());
                let mut reference = Shake256V1::new();
                reference.update(&job.prefix.encoded);
                reference.update(&job.body);
                let mut expected = SecretPolynomial::<u8>::zeroed(job.raw.len()).unwrap();
                reference.finalize().read(&mut expected);
                assert_eq!(&*expected, &*job.raw);
                assert!(tape_rounds.insert(round));
                tape_order.push(round);
                tape_count += 1;
            }
        }
    }
    assert_eq!((hash_count, tape_count), (336, 10));
    assert_eq!(chain_order, (1..=9).collect::<Vec<_>>());
    assert_eq!(tape_order, (1..=10).collect::<Vec<_>>());
    assert_eq!(chain_rounds, (1..=9).collect());
    assert_eq!(tape_rounds, (1..=10).collect());
}
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
fn execute_metal(label: &str, capture: &Capture) {
    use sha2::{Digest as _, Sha256};
    crate::keccak_gpu::preflight(crate::Digest384GpuBackendV1::Metal).unwrap();
    let tree = capture
        .jobs
        .iter()
        .filter(|job| {
            matches!(
                job.kind,
                Kind::Hash {
                    kind: 1,
                    oracle: 1..=4,
                    ..
                } | Kind::Hash { kind: 2, .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(tree.len(), 326);
    let mut done = 0;
    let mut digest = Sha256::new();
    for chunk in tree.chunks(32) {
        let jobs = chunk
            .iter()
            .map(|job| crate::keccak_batch::Job::new(&job.prefix.hash, &job.body))
            .collect::<Vec<_>>();
        let mut output = SecretPolynomial::<[u8; 32]>::zeroed(jobs.len()).unwrap();
        crate::keccak_gpu::hash(crate::Digest384GpuBackendV1::Metal, &jobs, &mut output)
            .expect("actual completed Metal SHA3; no substitute");
        for ((actual, job), input) in output.iter().zip(chunk).zip(&jobs) {
            let Kind::Hash { .. } = job.kind else {
                unreachable!()
            };
            let expected = job.digest.expect("hash digest");
            assert_eq!(*actual, expected.into_bytes());
            assert_eq!(input.scalar(), expected);
            digest.update(actual);
            done += 1;
        }
    }
    assert_eq!(done, 326);
    eprintln!(
        "q77 canonical Metal {label}: completed_tree_hashes={done}; scalar_atomic_tapes=10; observed_bytes={}; tree_outputs_sha256={:x}; release_qualified=false",
        capture.bytes,
        digest.finalize()
    );
}
#[test]
fn full_domain_ordinary_context_captures_every_canonical_hash_and_whole_tape() {
    let (fixture, private) = QuantityFixture::new(QuantityCase::MixedScale, 1);
    drop(private);
    let expected = fixture.expected();
    let prepared = fixture.prepare(ProofSemantics::StateTransition);
    let batch =
        PublicTransferBatch::new(&prepared, &expected, &[], BatchContextLimits::default()).unwrap();
    let relation = batch.segment(0).unwrap();
    assert_eq!(
        relation.schema().identity,
        FastpqQuantityUnits::BATCH_IDENTITY
    );
    exercise(&relation);
}
#[test]
fn full_domain_axt_context_captures_every_canonical_hash_and_whole_tape() {
    let (fixture, private) = QuantityFixture::new(QuantityCase::Maximum, 1);
    drop(private);
    let expected = fixture.expected();
    let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
    let batch = AxtTransferBatch::new(
        &prepared,
        &expected,
        &[],
        fixture.context(),
        BatchContextLimits::default(),
    )
    .unwrap();
    let relation = batch.segment(0).unwrap();
    assert_eq!(
        relation.schema().identity,
        FastpqQuantityUnits::AXT_BATCH_IDENTITY
    );
    exercise(&relation);
}
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
#[test]
#[ignore = "requires actual Metal execution of full ordinary q77 context trees"]
fn full_domain_ordinary_context_metal_matches_every_canonical_tree_hash() {
    let _lane = crate::backend::acquire_gpu_lane();
    let (fixture, private) = QuantityFixture::new(QuantityCase::MixedScale, 1);
    drop(private);
    let expected = fixture.expected();
    let prepared = fixture.prepare(ProofSemantics::StateTransition);
    let batch =
        PublicTransferBatch::new(&prepared, &expected, &[], BatchContextLimits::default()).unwrap();
    let relation = batch.segment(0).unwrap();
    execute_metal("ordinary", &exercise(&relation));
}
#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
#[test]
#[ignore = "requires actual Metal execution of full AXT q77 context trees"]
fn full_domain_axt_context_metal_matches_every_canonical_tree_hash() {
    let _lane = crate::backend::acquire_gpu_lane();
    let (fixture, private) = QuantityFixture::new(QuantityCase::Maximum, 1);
    drop(private);
    let expected = fixture.expected();
    let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
    let batch = AxtTransferBatch::new(
        &prepared,
        &expected,
        &[],
        fixture.context(),
        BatchContextLimits::default(),
    )
    .unwrap();
    let relation = batch.segment(0).unwrap();
    execute_metal("axt", &exercise(&relation));
}
#[test]
fn scoped_capture_rejects_reentry_cleans_unwind_and_never_records_inactive_hashes() {
    let context = Context::new(b"observer custody control").unwrap();
    let frame = context.frame(1, 1, 0, 0, 0, 32, BodyFields::One(b"body"));
    assert!(
        std::panic::catch_unwind(|| capture(|| {
            context.hash_frame(&frame).unwrap();
            panic!("injected capture unwind")
        }))
        .is_err()
    );
    CAPTURE.with(|c| assert!(c.borrow().is_none()));
    assert!(std::panic::catch_unwind(|| capture(|| capture(|| ()))).is_err());
    CAPTURE.with(|c| assert!(c.borrow().is_none()));
    context.hash_frame(&frame).unwrap();
    let (_, captured) = capture(|| {
        assert_eq!(count(), 0);
        context.hash_frame(&frame).unwrap()
    });
    assert_eq!(captured.jobs.len(), 1);
}
#[test]
fn capture_is_thread_local_and_rejects_both_storage_limits() {
    let context = Context::new(b"bounded observer").unwrap();
    let frame = context.frame(1, 1, 0, 0, 0, 32, BodyFields::One(b"body"));
    let (_, captured) = capture(|| {
        std::thread::scope(|scope| {
            scope.spawn(|| context.hash_frame(&frame).unwrap());
        });
        assert_eq!(count(), 0);
        context.hash_frame(&frame).unwrap()
    });
    assert_eq!(captured.jobs.len(), 1);
    let kind = Kind::Tape {
        round: 1,
        raw_address: 0,
    };
    assert!(
        std::panic::catch_unwind(|| capture(|| {
            for _ in 0..MAX_OBSERVATIONS {
                record(&context.prefix, &[], &[], kind, None);
            }
            assert_eq!(count(), MAX_OBSERVATIONS);
            record(&context.prefix, &[], &[], kind, None)
        }))
        .is_err()
    );
    CAPTURE.with(|c| assert!(c.borrow().is_none()));
    assert!(
        std::panic::catch_unwind(|| capture(|| {
            record(
                &context.prefix,
                &vec![0; MAX_CAPTURE_BYTES],
                &[],
                kind,
                None,
            );
            assert_eq!(count(), 1);
            record(&context.prefix, &[], &[0], kind, None)
        }))
        .is_err()
    );
    CAPTURE.with(|c| assert!(c.borrow().is_none()));
}

#[test]
fn canonical_prepared_jobs_bind_unconsumed_prefix_exact_bodies_and_bounds() {
    let (fixture, private) = QuantityFixture::new(QuantityCase::MixedScale, 1);
    drop(private);
    let expected = fixture.expected();
    let prepared = fixture.prepare(ProofSemantics::StateTransition);
    let batch =
        PublicTransferBatch::new(&prepared, &expected, &[], BatchContextLimits::default()).unwrap();
    let relation = batch.segment(0).unwrap();
    let binding = Binding::for_relation(&relation).unwrap();
    let body = vec![0; 301 * 8];
    let sealed = binding.prepare_leaf(Oracle::Row, 0, &body).unwrap();
    let canonical = binding.hash_leaf(Oracle::Row, 0, &body).unwrap();
    let job = sealed.job();
    assert_eq!(job.scalar(), canonical);
    // The production owner lends immutable exact prefix/body references. A
    // caller-supplied consumed descriptor no longer exists. General SHA3 jobs
    // deliberately admit arbitrary continuations; none of these altered jobs
    // authenticate as this canonical typed leaf.
    for count in [1, 6, 7, 8, job.body().len()] {
        let mut consumed = job.prefix().clone();
        consumed.update(&job.body()[..count]);
        assert_ne!(
            crate::keccak_batch::Job::new(&consumed, job.body()).scalar(),
            canonical
        );
    }
    let mut long = job.body().to_vec();
    long.push(0);
    assert_ne!(
        crate::keccak_batch::Job::new(job.prefix(), &long).scalar(),
        canonical
    );
    assert_ne!(
        crate::keccak_batch::Job::new(job.prefix(), &job.body()[..job.body().len() - 1]).scalar(),
        canonical
    );
    assert!(
        binding
            .prepare_leaf(Oracle::Row, 0, &body[..body.len() - 1])
            .is_err()
    );
    let mut long = body.clone();
    long.push(0);
    assert!(binding.prepare_leaf(Oracle::Row, 0, &long).is_err());
    assert!(binding.prepare_leaf(Oracle::Row, u32::MAX, &body).is_err());
    let mut noncanonical = body;
    noncanonical[..8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(binding.prepare_leaf(Oracle::Row, 0, &noncanonical).is_err());
    let over_limit = vec![0; crate::keccak_batch::MAX_BODY_BYTES + 1];
    let invalid = crate::keccak_batch::Job::new(job.prefix(), &over_limit);
    assert!(crate::keccak_batch::validate(&[invalid], 1).is_err());
    // Invalid alternatives cannot consume or mutate the original owner.
    assert_eq!(sealed.job().scalar(), canonical);
}

#[test]
fn observation_digest_presence_is_exact_and_mismatch_never_mutates_capture() {
    let context = Context::new(b"observer metadata presence").unwrap();
    let hash = Kind::Hash {
        kind: 1,
        oracle: 1,
        round: 0,
        level: 0,
        index: 3,
        first_field_address: 123,
    };
    let tape = Kind::Tape {
        round: 1,
        raw_address: 456,
    };
    let digest = Digest::from_bytes([29; 32]);
    let ((), captured) = capture(|| {
        for (kind, wrong) in [(hash, None), (tape, Some(digest))] {
            assert!(
                std::panic::catch_unwind(|| record(&context.prefix, b"body", b"raw", kind, wrong))
                    .is_err()
            );
            assert_eq!(count(), 0);
            CAPTURE.with(|slot| assert_eq!(slot.borrow().as_ref().unwrap().bytes, 0));
        }
        record(&context.prefix, b"hash body", &[], hash, Some(digest));
        record(&context.prefix, b"tape body", b"raw tape", tape, None);
    });
    assert_eq!(captured.jobs.len(), 2);
    assert_eq!(captured.jobs[0].kind, hash);
    assert_eq!(captured.jobs[0].digest, Some(digest));
    assert_eq!(&*captured.jobs[0].body, b"hash body");
    assert!(captured.jobs[0].raw.is_empty());
    assert_eq!(captured.jobs[1].kind, tape);
    assert_eq!(captured.jobs[1].digest, None);
    assert_eq!(&*captured.jobs[1].body, b"tape body");
    assert_eq!(&*captured.jobs[1].raw, b"raw tape");
    assert_eq!(captured.bytes, 26);
}
