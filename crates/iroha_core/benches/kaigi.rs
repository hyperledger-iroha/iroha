//! Cost of Kaigi call-record work and proofs as a roster grows.
//!
//! Each instruction decodes, validates and re-encodes the whole retained record, so
//! these groups show how that scales toward the protocol's 4,096-participant limit.
//! `roster_root_incremental` keeps the replaced leaf-by-leaf tree build for comparison.
//!
//! ```sh
//! scripts/cargo_fast.sh --target-slot perf --stable-local-metadata -- \
//!   bench -p iroha_core --features bench --bench kaigi
//! ```
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use ff::PrimeField;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World},
};
use iroha_core_zk::kaigi_client_v1::{
    KaigiAuthorizationActionV1, KaigiAuthorizationWitnessV1, kaigi_authorization_context_v1,
    prove_kaigi_authorization_artifacts_v1,
};
use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_data_model::{
    isi::kaigi::{CreateKaigi, JoinKaigi, LeaveKaigi},
    kaigi::{
        KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode,
        KaigiRecord, NewKaigi, scalar::KaigiAuthorizationScalarV1,
    },
    prelude::*,
    proof::ProofBox,
};
use iroha_model_base::domain::DomainId;
use iroha_pasta::{Eq, msm::MemoryBudget};
use iroha_plonk::{
    cs::InstanceType,
    keys::{KeygenConfigV2, keygen_vk_with_binding_v2},
    pcs::ipa::PinnedParams,
    verifier::verify_full,
};
use iroha_primitives::json::Json;
use iroha_test_samples::gen_account_in;
use kaigi_zk::{
    Scalar,
    authorization_v1::{KaigiAuthorizationCircuitV1, KaigiAuthorizationContextV1},
    native::NativeRelationV1,
};
use nonzero_ext::nonzero;
use std::{hint::black_box, time::Duration, time::Instant};

const ROSTER_SIZES: [usize; 3] = [64, 1_024, 4_096];
/// The 1-MiB ceiling stops a private roster long before 4,096 participants.
const PRIVATE_ROSTER_SIZES: [usize; 3] = [64, 512, 1_024];
const FIRST_SEEN_PROOFS: u8 = 16;

fn call_id() -> KaigiId {
    KaigiId::new(
        DomainId::try_new("kaigi", "universal").expect("domain id"),
        "bench".parse().expect("call name"),
    )
}

fn commitments(count: usize) -> Vec<KaigiParticipantCommitment> {
    (1..=u64::try_from(count).expect("count fits u64"))
        .map(|value| {
            let mut bytes = [0_u8; 32];
            bytes[..8].copy_from_slice(&value.to_le_bytes());
            KaigiParticipantCommitment {
                commitment: KaigiAuthorizationScalarV1::from_le_bytes(bytes)
                    .expect("small scalar is canonical"),
            }
        })
        .collect()
}

/// A transparent record retaining `count` participants, with no state behind it.
fn record(count: usize) -> KaigiRecord {
    let (host, _) = gen_account_in("kaigi");
    let mut record = KaigiRecord::from_new(&NewKaigi::with_defaults(call_id(), host), 0);
    for _ in 0..count {
        let (participant, _) = gen_account_in("kaigi");
        record.push_participant(participant);
    }
    record
}

/// A private record whose `count` participants each joined once, as a real call stores.
fn private_record(count: usize) -> KaigiRecord {
    let (host, _) = gen_account_in("kaigi");
    let mut template = NewKaigi::with_defaults(call_id(), host);
    template.privacy_mode = KaigiPrivacyMode::ZkRosterV1;
    let mut record = KaigiRecord::from_new(&template, 0);
    for commitment in commitments(count) {
        let (participant, _) = gen_account_in("kaigi");
        record
            .private_participation
            .commit_join(&participant, 1, commitment.commitment)
            .expect("join private record");
        record.push_commitment(commitment);
        record.push_nullifier(KaigiParticipantNullifier {
            digest: commitment.commitment,
        });
    }
    record
}

fn bench_record_codec(c: &mut Criterion) {
    let mut group = c.benchmark_group("kaigi_record");
    for (label, records) in [
        ("transparent", ROSTER_SIZES.map(record)),
        ("private", PRIVATE_ROSTER_SIZES.map(private_record)),
    ] {
        for record in records {
            let count = record.participants.len() + record.roster_commitments.len();
            group.throughput(Throughput::Elements(count as u64));
            let encoded = Json::try_new(&record).expect("encode record");
            println!(
                "{label} record of {count}: {} JSON bytes",
                encoded.as_ref().len()
            );
            group.bench_with_input(
                BenchmarkId::new(format!("{label}_encode_json"), count),
                &record,
                |b, record| b.iter(|| Json::try_new(black_box(record)).expect("encode record")),
            );
            group.bench_with_input(
                BenchmarkId::new(format!("{label}_decode_json"), count),
                &encoded,
                |b, encoded| {
                    b.iter(|| {
                        black_box(encoded)
                            .try_into_any_norito::<KaigiRecord>()
                            .expect("decode record")
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_roster_root(c: &mut Criterion) {
    let mut group = c.benchmark_group("kaigi_record");
    for count in ROSTER_SIZES {
        group.throughput(Throughput::Elements(count as u64));
        let commitments = commitments(count);
        group.bench_with_input(
            BenchmarkId::new("roster_root", count),
            &commitments,
            |b, commitments| b.iter(|| KaigiRecord::compute_roster_root(black_box(commitments))),
        );
        group.bench_with_input(
            BenchmarkId::new("roster_root_incremental", count),
            &commitments,
            |b, commitments| {
                b.iter(|| {
                    let mut tree = MerkleTree::<[u8; 32]>::default();
                    for entry in black_box(commitments) {
                        tree.add(HashOf::from_untyped_unchecked(Hash::new(
                            entry.commitment.to_le_bytes(),
                        )));
                    }
                    tree.root()
                });
            },
        );
    }
    group.finish();
}

/// A live state whose one call already retains `count` transparent participants.
struct LiveCall {
    state: State,
    call: KaigiId,
    newcomer: AccountId,
}

fn live_call(count: usize) -> LiveCall {
    let domain_id = call_id().domain_id;
    let (host, _) = gen_account_in("kaigi");
    let (newcomer, _) = gen_account_in("kaigi");
    let members = (0..count)
        .map(|_| gen_account_in("kaigi").0)
        .collect::<Vec<_>>();
    let accounts = members
        .iter()
        .chain([&host, &newcomer])
        .map(|account| Account::new(account.clone()).build(&host))
        .collect::<Vec<_>>();
    let domain = Domain::new(domain_id).build(&host);
    let world = World::with([domain], accounts, []);
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let call = call_id();
    let mut block = state.block(BlockHeader::new(nonzero!(2_u64), None, None, 0, 0));
    let mut tx = block.transaction();
    let mut template = NewKaigi::with_defaults(call.clone(), host.clone());
    template.privacy_mode = KaigiPrivacyMode::Transparent;
    CreateKaigi {
        call: template,
        commitment: None,
        nullifier: None,
        roster_root: None,
        proof: None,
    }
    .execute(&host, &mut tx)
    .expect("create call");
    for member in &members {
        JoinKaigi {
            call_id: call.clone(),
            participant: member.clone(),
            commitment: None,
            nullifier: None,
            roster_root: None,
            proof: None,
        }
        .execute(member, &mut tx)
        .expect("join call");
    }
    tx.apply();
    block
        .commit_world_overlay_for_testing()
        .expect("commit fixture block");
    LiveCall {
        state,
        call,
        newcomer,
    }
}

fn bench_roster_change(c: &mut Criterion) {
    let mut group = c.benchmark_group("kaigi_execute");
    group.sample_size(20);
    for count in ROSTER_SIZES {
        // The newcomer brings the roster to `count`, which may be the protocol's limit.
        let live = live_call(count - 1);
        group.bench_function(BenchmarkId::new("join_and_leave", count), |b| {
            b.iter_batched_ref(
                || {
                    live.state
                        .block(BlockHeader::new(nonzero!(3_u64), None, None, 0, 0))
                },
                |block| {
                    let mut tx = block.transaction();
                    JoinKaigi {
                        call_id: live.call.clone(),
                        participant: live.newcomer.clone(),
                        commitment: None,
                        nullifier: None,
                        roster_root: None,
                        proof: None,
                    }
                    .execute(&live.newcomer, &mut tx)
                    .expect("join call");
                    LeaveKaigi {
                        call_id: live.call.clone(),
                        participant: live.newcomer.clone(),
                        commitment: None,
                        nullifier: None,
                        roster_root: None,
                        proof: None,
                    }
                    .execute(&live.newcomer, &mut tx)
                    .expect("leave call");
                },
                // A State admits one live block, so a larger batch would wait on itself.
                BatchSize::PerIteration,
            );
        });
    }
    group.finish();
}

/// Ended calls stay in domain metadata forever, so a domain's copy-on-write cost
/// grows with every call it has ever hosted, even when each call is small.
fn bench_retained_calls(c: &mut Criterion) {
    let mut group = c.benchmark_group("kaigi_execute");
    group.sample_size(20);
    for retained in [0_usize, 10_000, 100_000] {
        let live = live_call(0);
        let (host, _) = gen_account_in("kaigi");
        let domain_id = live.call.domain_id.clone();
        let filler = KaigiRecord::from_new(&NewKaigi::with_defaults(call_id(), host), 0);
        let filler = Json::try_new(&filler).expect("encode filler record");
        {
            let mut block = live
                .state
                .block(BlockHeader::new(nonzero!(3_u64), None, None, 0, 0));
            let mut tx = block.transaction();
            let domain = tx.world.domain_mut(&domain_id).expect("fixture domain");
            for index in 0..retained {
                let key = format!("retained_{index}").parse().expect("metadata key");
                domain.metadata_mut().insert(key, filler.clone());
            }
            tx.apply();
            block
                .commit_world_overlay_for_testing()
                .expect("commit fixture block");
        }
        group.bench_function(
            BenchmarkId::new("join_with_retained_calls", retained),
            |b| {
                b.iter_batched_ref(
                    || {
                        live.state
                            .block(BlockHeader::new(nonzero!(4_u64), None, None, 0, 0))
                    },
                    |block| {
                        let mut tx = block.transaction();
                        JoinKaigi {
                            call_id: live.call.clone(),
                            participant: live.newcomer.clone(),
                            commitment: None,
                            nullifier: None,
                            roster_root: None,
                            proof: None,
                        }
                        .execute(&live.newcomer, &mut tx)
                        .expect("join call");
                    },
                    // A State admits one live block, so a larger batch would wait on itself.
                    BatchSize::PerIteration,
                );
            },
        );
    }
    group.finish();
}

fn witness_with(blinding: u8) -> KaigiAuthorizationWitnessV1 {
    let mut bytes = [blinding; 32];
    KaigiAuthorizationWitnessV1::take_blinding(&mut bytes).expect("canonical blinding")
}
fn witness() -> KaigiAuthorizationWitnessV1 {
    witness_with(0x11)
}

/// Mean time to verify a proof that the verifying owner has never seen.
///
/// Proving verifies each proof on the shared verifier, whose memo then answers every
/// repeat. Only an independent owner, with empty memos, shows what a peer pays the first
/// time a proof reaches it.
fn first_seen_verification(context: KaigiAuthorizationContextV1) -> Duration {
    let relation = NativeRelationV1::Authorization;
    let params = PinnedParams::<Eq>::derive(relation.k()).expect("public parameters");
    let (binding, key) = keygen_vk_with_binding_v2(
        &params,
        &KaigiAuthorizationCircuitV1::default(),
        &KeygenConfigV2::pipa_r(vec![InstanceType::Field]),
    )
    .expect("verifying key");
    let proofs = (1..=FIRST_SEEN_PROOFS)
        .map(|blinding| {
            let (inputs, proof) = relation
                .prover()
                .expect("prover")
                .prove_authorization(context, witness_with(blinding))
                .expect("proof")
                .into_parts();
            let inputs = inputs
                .iter()
                .map(|bytes| {
                    Option::<Scalar>::from(Scalar::from_repr(*bytes)).expect("canonical scalar")
                })
                .collect::<Vec<_>>();
            (inputs, proof)
        })
        .collect::<Vec<_>>();
    let started = Instant::now();
    for (inputs, proof) in proofs {
        verify_full(
            &params,
            &binding,
            &key,
            &[inputs],
            &proof,
            MemoryBudget::DEFAULT,
        )
        .expect("valid proof");
    }
    started.elapsed() / u32::from(FIRST_SEEN_PROOFS)
}

fn bench_proof(c: &mut Criterion) {
    let mut group = c.benchmark_group("kaigi_proof");
    group.sample_size(10);
    let (host, _) = gen_account_in("kaigi");
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new([7; 32])));
    let context = kaigi_authorization_context_v1(
        network,
        &call_id(),
        &host,
        &host,
        0,
        KaigiAuthorizationActionV1::HostCreate,
        &KaigiRecord::compute_roster_root(&[]),
    )
    .expect("host-create context");
    let artifacts = prove_kaigi_authorization_artifacts_v1(context, witness())
        .expect("first authorization proof");
    println!("kaigi authorization proof bytes: {}", artifacts.proof.len());
    println!(
        "kaigi authorization proof seen first: {:?} to verify",
        first_seen_verification(context)
    );
    group.bench_function("prove_authorization", |b| {
        b.iter(|| {
            prove_kaigi_authorization_artifacts_v1(black_box(context), witness())
                .expect("authorization proof")
        });
    });
    let key = iroha_core_zk::native_pipa_r::kaigi_verifying_key(
        kaigi_zk::native::NativeRelationV1::Authorization,
    )
    .expect("compiled key");
    let proof = ProofBox::new(
        iroha_core_zk::ZK_BACKEND_NATIVE_PIPA_R.to_owned(),
        artifacts.proof,
    );
    group.bench_function("verify_authorization_repeat", |b| {
        b.iter(|| {
            assert!(
                iroha_core_zk::verify_backend(
                    iroha_core_zk::ZK_BACKEND_NATIVE_PIPA_R,
                    black_box(&proof),
                    Some(&key),
                )
                .expect("verification completes")
            );
        });
    });
    group.finish();
}

fn main() {
    let mut criterion = Criterion::default().configure_from_args();
    bench_record_codec(&mut criterion);
    bench_roster_root(&mut criterion);
    bench_roster_change(&mut criterion);
    bench_retained_calls(&mut criterion);
    bench_proof(&mut criterion);
    criterion.final_summary();
}
