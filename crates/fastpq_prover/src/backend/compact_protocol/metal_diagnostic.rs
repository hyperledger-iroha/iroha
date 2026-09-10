//! Explicit Metal parity for the final compact ordinary and AXT context owners.
//!
//! The scoped observer sees only successful canonical CPU hashes. It records the
//! existing owned descriptor and exact encoded body; it never constructs a domain,
//! body frame or sponge state. Every nonempty device batch must complete on Metal.
//! These tests cover hash inputs and whole tapes, not a GPU proof verifier, private
//! witness generation, zero knowledge, hardware speed or production admission.

use std::{cell::RefCell, collections::BTreeSet};

use fastpq_isi::GoldilocksDigest384OwnedDomainPrefixV1;

use super::*;
use crate::{
    ProofSemantics,
    backend::{
        compact_axt_air::AxtTransferAir,
        compact_public_transfer::PublicTransferAir,
        compact_quantity_tests::{QuantityCase, QuantityFixture},
        compact_v1::{Context, Oracle, Round},
        compact_value_domain::CompactTransferValue,
    },
    digest384_batch::{Digest384LastFieldJob, hash_last_fields_cpu, try_hash_last_fields_metal},
};
use iroha_data_model::fastpq::FastpqQuantityUnits;
use sha2::{Digest as _, Sha256};

const MAX_OBSERVATIONS: usize = 2048;
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
const DEVICE_BATCH: usize = 32;

struct Observation {
    prefix: GoldilocksDigest384OwnedDomainPrefixV1,
    index: u64,
    body: Vec<u8>,
    expected: Digest,
}

#[derive(Default)]
struct Capture {
    jobs: Vec<Observation>,
    body_bytes: usize,
}

thread_local! {
    static CAPTURE: RefCell<Option<Capture>> = const { RefCell::new(None) };
}

/// Record the sole canonical CPU hash call only during this thread's active test.
pub(in crate::backend) fn observe(
    prefix: &GoldilocksDigest384OwnedDomainPrefixV1,
    index: u64,
    body: &[u8],
    expected: Digest,
) {
    CAPTURE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(capture) = slot.as_mut() else {
            return;
        };
        assert!(
            capture.jobs.len() < MAX_OBSERVATIONS,
            "bounded hash capture"
        );
        let bytes = capture.body_bytes.checked_add(body.len()).unwrap();
        assert!(bytes <= MAX_BODY_BYTES, "bounded encoded-body capture");
        capture.jobs.push(Observation {
            prefix: prefix.clone(),
            index,
            body: body.to_vec(),
            expected,
        });
        capture.body_bytes = bytes;
    });
}

struct CaptureGuard;

impl Drop for CaptureGuard {
    fn drop(&mut self) {
        CAPTURE.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

fn capture<T>(call: impl FnOnce() -> T) -> (T, Capture) {
    CAPTURE.with(|slot| {
        let mut slot = slot.borrow_mut();
        assert!(slot.is_none(), "hash capture must not be reentrant");
        *slot = Some(Capture::default());
    });
    let guard = CaptureGuard;
    let result = call();
    let captured = CAPTURE.with(|slot| slot.borrow_mut().take().unwrap());
    drop(guard);
    (result, captured)
}

fn count() -> usize {
    CAPTURE.with(|slot| slot.borrow().as_ref().unwrap().jobs.len())
}

fn one_hash(
    round: u8,
    level: usize,
    index: usize,
    call: impl FnOnce() -> crate::Result<Digest>,
) -> Digest {
    let before = count();
    let returned = call().unwrap();
    CAPTURE.with(|slot| {
        let slot = slot.borrow();
        let captured = slot.as_ref().unwrap();
        assert_eq!(captured.jobs.len(), before + 1);
        let actual = captured.jobs.last().unwrap();
        let domain = actual.prefix.domain();
        assert_eq!(domain.role, b"compact-commitment");
        assert_eq!(domain.phase, b"typed-h");
        assert_eq!(domain.counter, u64::from(round));
        assert_eq!(domain.level, level as u64);
        assert_eq!(actual.index, index as u64);
        assert_eq!(actual.expected, returned);
    });
    returned
}

fn positions(count: usize) -> Vec<usize> {
    assert!(count > 0);
    BTreeSet::from([0, count / 2, count - 1])
        .into_iter()
        .collect()
}

fn extension(seed: usize) -> GoldilocksFp4V1 {
    GoldilocksFp4V1::new([
        0,
        GOLDILOCKS_MODULUS - 1,
        seed as u64 + 1,
        0x1234_5678_9abc_def0,
    ])
    .unwrap()
}

fn child(seed: usize) -> Digest {
    Digest::new([
        0,
        1,
        GOLDILOCKS_MODULUS - 1,
        0x1234_5678_9abc_def0,
        seed as u64,
        seed as u64 + 17,
    ])
    .unwrap()
}

fn exercise(relation: &impl FixedAir) -> Capture {
    let geometry = Geometry::new(relation).unwrap();
    let binding = Binding::new(relation, &geometry).unwrap();
    assert_eq!(geometry.schema.trace_rows, 65_536);
    assert_eq!(geometry.schema.width, 342);
    assert_eq!(geometry.schema.constraints, 923);
    assert_eq!(geometry.lde_rows, 524_288);
    assert_eq!(geometry.fri_lengths.len(), 18);
    let ((), captured) = capture(|| {
        let row: Vec<_> = (0..342)
            .map(|column| match column % 4 {
                0 => 0,
                1 => GOLDILOCKS_MODULUS - 1,
                2 => 0x1234_5678_9abc_def0,
                _ => column as u64,
            })
            .collect();
        let mut leaf_count = 0;
        let mut parent_count = 0;
        let mut coverage = BTreeSet::new();
        let mut roots = Vec::new();
        for oracle in 0..21 {
            let (role, round, leaves) = match oracle {
                0 => (MerkleTreeRoleV1::AirTrace, 0, geometry.lde_rows),
                1 => (MerkleTreeRoleV1::Lde, 0, geometry.lde_rows),
                2 => (MerkleTreeRoleV1::AirComposition, 0, geometry.lde_rows),
                _ => {
                    let round = oracle - 3;
                    (
                        MerkleTreeRoleV1::Fri(round as u32),
                        round as u8,
                        if round == 17 {
                            1
                        } else {
                            geometry.lde_rows >> (round + 1)
                        },
                    )
                }
            };
            let mut terminal = None;
            for index in positions(leaves) {
                let digest = one_hash(round, 0, index, || match oracle {
                    0 => binding.row(index, &row),
                    1 => binding.mixed(index, extension(index)),
                    2 => binding.quotient(index, extension(index)),
                    _ => binding.fri(
                        round as usize,
                        index,
                        &(0..if round == 17 { 4 } else { 2 })
                            .map(|lane| extension(index + lane))
                            .collect::<Vec<_>>(),
                    ),
                });
                assert!(coverage.insert((oracle, 0, index)));
                terminal = Some(digest);
                leaf_count += 1;
            }
            let depth = leaves.ilog2().max(1) as usize;
            let mut root = None;
            for level in 1..=depth {
                for index in positions((leaves >> level).max(1)) {
                    // These bounded parent payloads exercise exact coordinates;
                    // they do not claim to reconstruct a full committed tree.
                    let left = if leaves == 1 {
                        terminal.unwrap()
                    } else {
                        child(index)
                    };
                    let right = if leaves == 1 { left } else { child(index + 1) };
                    root = Some(one_hash(round, level, index, || {
                        binding.parent(role, level, index, left, right)
                    }));
                    assert!(coverage.insert((oracle, level, index)));
                    parent_count += 1;
                }
            }
            assert!(root.is_some());
            roots.push(root.unwrap());
        }
        assert_eq!(leaf_count, 61);
        assert_eq!(parent_count, 622);
        assert_eq!(coverage.len(), 683);
        assert_eq!(count(), 683);
        assert_eq!(roots.len(), 21);

        // Invalid typed geometry must stop before the observer sees a hash.
        assert!(binding.row(geometry.lde_rows, &row).is_err());
        assert!(binding.row(0, &row[..341]).is_err());
        let mut noncanonical = row;
        noncanonical[341] = GOLDILOCKS_MODULUS;
        assert!(binding.row(0, &noncanonical).is_err());
        assert!(binding.fri(18, 0, &[extension(0); 2]).is_err());
        assert!(binding.fri(17, 1, &[extension(0); 4]).is_err());
        assert!(binding.fri(17, 0, &[extension(0); 2]).is_err());
        assert!(
            binding
                .parent(MerkleTreeRoleV1::AirTrace, 0, 0, child(0), child(1))
                .is_err()
        );
        assert!(
            binding
                .parent(MerkleTreeRoleV1::AirTrace, 20, 0, child(0), child(1))
                .is_err()
        );
        assert!(
            binding
                .parent(MerkleTreeRoleV1::AirTrace, 1, 262_144, child(0), child(1))
                .is_err()
        );
        assert!(
            binding
                .parent(MerkleTreeRoleV1::Fri(17), 1, 0, child(0), child(1))
                .is_err()
        );
        assert_eq!(count(), 683);

        // Traverse the complete production-owned schedule without substituting
        // a tape decoder, omitting unused suffixes or inventing a transcript.
        let mut transcript = binding.transcript(relation, &geometry, roots[0]).unwrap();
        assert_eq!(transcript.columns().unwrap().len(), 342);
        assert_eq!(transcript.alphas(roots[1]).unwrap().len(), 923);
        transcript.joint(roots[2]).unwrap();
        for round in 0..17 {
            transcript.beta(round, roots[round + 3]).unwrap();
        }
        let queries = transcript.queries(roots[20]).unwrap();
        assert_eq!(queries.len(), 375);
        assert!(queries.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(queries.iter().all(|index| *index < geometry.lde_rows));
        let completed_count = count();
        assert!(transcript.queries(roots[20]).is_err());
        assert!(transcript.columns().is_err());
        assert_eq!(count(), completed_count);
    });
    assert_eq!(captured.jobs.len(), 1635);
    let mut transcript_blocks = 0;
    for ordinal in 1..=22 {
        let expected = Round::new(ordinal).unwrap().tape_bytes() / 48;
        let blocks: Vec<_> = captured
            .jobs
            .iter()
            .filter(|job| {
                let domain = job.prefix.domain();
                domain.role == b"compact-transcript" && domain.counter == u64::from(ordinal)
            })
            .collect();
        assert_eq!(blocks.len(), expected);
        for (index, job) in blocks.iter().enumerate() {
            assert_eq!(job.index, index as u64);
            assert_eq!(job.prefix.domain().phase, b"whole-field-tape-block");
            assert_eq!(job.prefix.domain().level, 0);
            assert_eq!(job.body, blocks[0].body);
        }
        transcript_blocks += blocks.len();
    }
    assert_eq!(transcript_blocks, 931);
    let commits: Vec<_> = captured.jobs[683..]
        .iter()
        .filter(|job| job.prefix.domain().role == b"compact-commitment")
        .collect();
    assert_eq!(commits.len(), 21);
    for (ordinal, job) in commits.iter().enumerate() {
        assert_eq!(job.prefix.domain().counter, ordinal as u64 + 1);
        assert_eq!(job.prefix.domain().level, 0);
        assert_eq!(job.index, 0);
    }
    for job in &captured.jobs {
        assert_eq!(
            job.prefix.domain().catalog,
            fastpq_isi::FASTPQ_CATALOG_V1.as_bytes()
        );
        assert_eq!(
            job.prefix.domain().protocol,
            FASTPQ_FINAL_V1.name.as_bytes()
        );
        assert!(job.prefix.domain().profile.len() > relation.statement_bytes().len());
        assert_eq!(
            job.prefix.domain().profile,
            captured.jobs[0].prefix.domain().profile
        );
    }
    captured
}

fn execute_metal(label: &str, captured: Capture) {
    assert_eq!(captured.jobs.len(), 1635);
    let profile_sha256 = hex::encode(Sha256::digest(captured.jobs[0].prefix.domain().profile));
    let mut completed = 0usize;
    let mut output_hash = Sha256::new();
    let mut final_tape_hash = Sha256::new();
    for observations in captured.jobs.chunks(DEVICE_BATCH) {
        assert!(!observations.is_empty());
        let jobs: Vec<_> = observations
            .iter()
            .map(|observation| {
                let stream = observation
                    .prefix
                    .last_field_stream_at(observation.index, &[], observation.body.len())
                    .unwrap();
                Digest384LastFieldJob::new(stream, &observation.body).unwrap()
            })
            .collect();
        let cpu_stream = hash_last_fields_cpu(&jobs).unwrap();
        let actual = try_hash_last_fields_metal(&jobs)
            .expect("this explicit parity diagnostic requires actual completed Metal work");
        assert_eq!(actual.len(), jobs.len());
        assert_eq!(cpu_stream.len(), jobs.len());
        for ((actual, streamed), observed) in actual.iter().zip(cpu_stream).zip(observations) {
            assert_eq!(
                streamed, observed.expected,
                "canonical prefix stream differs from Context"
            );
            assert_eq!(
                *actual, observed.expected,
                "completed Metal differs from Context"
            );
            assert_eq!(actual.words().len(), 6);
            assert!(actual.words().iter().all(|word| *word < GOLDILOCKS_MODULUS));
            output_hash.update(actual.to_le_bytes());
            if observed.prefix.domain().role == b"compact-transcript" {
                final_tape_hash.update(actual.to_le_bytes());
            }
            completed += 1;
        }
    }
    assert_eq!(completed, 1635);
    eprintln!(
        "compact-context Metal {label}: payloads=deterministic-full-width profile_sha256={profile_sha256} completed_jobs={completed} batches={} lanes={} captured_body_bytes={} outputs_sha256={:x} whole_tapes_sha256={:x} release_qualified=false",
        completed.div_ceil(DEVICE_BATCH),
        completed * 6,
        captured.body_bytes,
        output_hash.finalize(),
        final_tape_hash.finalize(),
    );
}

#[test]
#[ignore = "requires real completed Metal work for the final full-domain ordinary context"]
fn final_full_domain_ordinary_context_metal_matches_every_canonical_hash() {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let (fixture, private) = QuantityFixture::new(QuantityCase::MixedScale, 1);
    drop(private);
    let expected = fixture.expected();
    let prepared = fixture.prepare(ProofSemantics::StateTransition);
    let relation = PublicTransferAir::new(&prepared, &expected).unwrap();
    assert_eq!(
        relation.schema().identity,
        FastpqQuantityUnits::TRANSFER_IDENTITY
    );
    execute_metal("ordinary", exercise(&relation));
}

#[test]
#[ignore = "requires real completed Metal work for the final full-domain AXT context"]
fn final_full_domain_axt_context_metal_matches_every_canonical_hash() {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let (fixture, private) = QuantityFixture::new(QuantityCase::Maximum, 1);
    drop(private);
    let expected = fixture.expected();
    let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
    let relation = AxtTransferAir::new(
        &prepared,
        &expected,
        &fixture.axt.binding,
        fixture.axt.metadata(),
        fixture.axt.outer,
        fixture.axt.remote.as_deref(),
    )
    .unwrap();
    assert_eq!(
        relation.schema().identity,
        FastpqQuantityUnits::AXT_IDENTITY
    );
    execute_metal("axt", exercise(&relation));
}

#[test]
fn scoped_observer_rejects_reentry_and_cleans_up_after_panic() {
    let context = Context::new(b"observer cleanup control").unwrap();
    assert!(
        std::panic::catch_unwind(|| {
            capture::<()>(|| {
                context.hash_leaf(Oracle::Row, 0, &[0; 342 * 8]).unwrap();
                panic!("intentional scope failure");
            });
        })
        .is_err()
    );
    CAPTURE.with(|slot| assert!(slot.borrow().is_none()));
    assert!(std::panic::catch_unwind(|| capture(|| capture(|| ()))).is_err());
    CAPTURE.with(|slot| assert!(slot.borrow().is_none()));
    // Inactive hashes never retain a context/body. A new active scope starts empty.
    context.hash_leaf(Oracle::Mixed, 0, &[0; 32]).unwrap();
    let ((), captured) = capture(|| {
        assert_eq!(count(), 0);
        context.hash_leaf(Oracle::Quotient, 0, &[0; 32]).unwrap();
    });
    assert_eq!(captured.jobs.len(), 1);
    drop(captured);
    CAPTURE.with(|slot| assert!(slot.borrow().is_none()));
}

#[test]
fn canonical_observed_jobs_reject_consumed_and_mismatched_descriptors() {
    let context = Context::new(b"descriptor rejection control").unwrap();
    let (_, captured) = capture(|| context.hash_leaf(Oracle::Row, 0, &[0; 342 * 8]).unwrap());
    assert_eq!(captured.jobs.len(), 1);
    let observed = &captured.jobs[0];
    let stream = observed
        .prefix
        .last_field_stream_at(observed.index, &[], observed.body.len())
        .unwrap();
    assert!(Digest384LastFieldJob::new(stream, &observed.body[..observed.body.len() - 1]).is_err());
    let mut oversized = observed.body.clone();
    oversized.push(0);
    assert!(Digest384LastFieldJob::new(stream, &oversized).is_err());
    for count in [1, 6, 7, 8, observed.body.len()] {
        let mut consumed = stream;
        consumed.update(&observed.body[..count]).unwrap();
        assert!(Digest384LastFieldJob::new(consumed, &observed.body).is_err());
    }
    let mut overrun = stream;
    assert!(overrun.update(&oversized).is_err());
    let valid = Digest384LastFieldJob::new(overrun, &observed.body).unwrap();
    assert_eq!(
        hash_last_fields_cpu(&[valid]).unwrap(),
        vec![observed.expected]
    );
    if let Some(oversized) = (u32::MAX as usize).checked_add(1) {
        assert!(
            observed
                .prefix
                .last_field_stream_at(observed.index, &[], oversized)
                .is_err()
        );
    }
}

#[test]
fn observer_is_thread_local_and_rejects_capture_limit_overruns() {
    let context = Context::new(b"observer isolation and bounds control").unwrap();
    let (_, captured) = capture(|| {
        std::thread::scope(|scope| {
            scope.spawn(|| context.hash_leaf(Oracle::Mixed, 0, &[0; 32]).unwrap());
        });
        assert_eq!(count(), 0);
        context.hash_leaf(Oracle::Mixed, 0, &[0; 32]).unwrap()
    });
    assert_eq!(captured.jobs.len(), 1);
    let original = &captured.jobs[0];
    // Exercise only observer bounds here; production callers never provide a
    // body or digest to this test-only collector except at Context::digest.
    assert!(
        std::panic::catch_unwind(|| capture(|| {
            for _ in 0..MAX_OBSERVATIONS {
                observe(&original.prefix, original.index, &[], original.expected);
            }
            assert_eq!(count(), MAX_OBSERVATIONS);
            observe(&original.prefix, original.index, &[], original.expected);
        }))
        .is_err()
    );
    CAPTURE.with(|slot| assert!(slot.borrow().is_none()));
    assert!(
        std::panic::catch_unwind(|| capture(|| {
            observe(
                &original.prefix,
                original.index,
                &vec![0; MAX_BODY_BYTES],
                original.expected,
            );
            assert_eq!(count(), 1);
            observe(&original.prefix, original.index, &[0], original.expected);
        }))
        .is_err()
    );
    CAPTURE.with(|slot| assert!(slot.borrow().is_none()));
}
