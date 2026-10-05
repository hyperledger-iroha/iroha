//! Shared fixtures for this crate's unit tests.

use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use crate::{
    platform,
    schema::{
        AddressSpaceObservation, AllocationObservation, AllocationSource, AllocationSourceKind,
        ByteCounter, ByteCounters, ByteKind, CachePolicy, Classification, DeclaredQuantity,
        FailureLog, FlowKind, MeasurementRecord, PeakRssSource, PhaseNode, PhaseTree, PhaseWorkers,
        ProcessObservation, ProvenanceKind, RECORD_SCHEMA_V1, RecorderHealth, RunContext,
        RunIdentity, RunOutcome, Scheduling, ThermalState, Unit, WorkCounter, WorkCounters,
    },
};

/// A complete identity context as the harness would write it.
pub fn bound_context() -> RunContext {
    RunContext {
        source_commit: "7e93d3e0497e93d3e0497e93d3e0497e93d3e049".to_owned(),
        source_dirty: true,
        source_dirty_digest: Some(
            "4f0d6c1c8f3b2a1e9d7c5b3a19e8f7d6c5b4a3928170f6e5d4c3b2a1908f7e6d".to_owned(),
        ),
        artifact: "sha256:9a3f".to_owned(),
        profile: "release+fastpq-final-v1".to_owned(),
        config: "defaults/privacy@taira".to_owned(),
        hardware: "Mac13,2/apple-m1-ultra/20c/128GiB".to_owned(),
        cache_policy: CachePolicy::Cold,
    }
}

fn phase(
    parent: Option<u32>,
    label: &str,
    calls: u64,
    wall: (u64, u64),
    thread: (u64, u64),
    process: (u64, u64),
    (peak_concurrent_children, active_threads_max): (u32, u32),
) -> PhaseNode {
    PhaseNode {
        parent,
        label: label.to_owned(),
        calls,
        completed: calls,
        interrupted: 0,
        unwound: 0,
        truncated: 0,
        wall_inclusive_ns: wall.0,
        wall_exclusive_ns: wall.1,
        thread_cpu_inclusive_ns: thread.0,
        thread_cpu_exclusive_ns: thread.1,
        process_cpu_window_inclusive_ns: process.0,
        process_cpu_window_exclusive_ns: process.1,
        peak_concurrent_children,
        active_threads_max,
        load_milli_first_enter: 2500,
        load_milli_last_exit: 3000,
        load_milli_max: 3200,
        allocations: 0,
        allocated_bytes: 0,
        live_bytes_high_water: 8192,
    }
}

/// A hand-written record that satisfies every report invariant.
///
/// Phase 1 has the sequential child 2. Phase 3 has four overlapping calls of
/// child 4: 4 x 100 ms inside a 120 ms parent whose children cover 110 ms.
pub fn sample_record() -> MeasurementRecord {
    let mut transform = phase(
        Some(1),
        "transform",
        3,
        (600_000_000, 600_000_000),
        (590_000_000, 590_000_000),
        (595_000_000, 595_000_000),
        (0, 1),
    );
    transform.allocations = 2;
    transform.allocated_bytes = 8192;
    MeasurementRecord {
        schema: RECORD_SCHEMA_V1.to_owned(),
        identity: RunIdentity::new(
            bound_context(),
            "sample_prove",
            FlowKind::Proof,
            "rust.iroha_measurement",
        ),
        outcome: RunOutcome::Succeeded,
        failures: FailureLog {
            classification: Classification::Measured,
            entries: Vec::new(),
            dropped: 0,
        },
        phase_tree: PhaseTree {
            classification: Classification::Measured,
            nodes: vec![
                phase(
                    None,
                    "prove",
                    1,
                    (1_000_000_000, 1_000_000),
                    (885_000_000, 3_000_000),
                    (1_330_000_000, 5_000_000),
                    (1, 1),
                ),
                phase(
                    Some(0),
                    "commit",
                    1,
                    (879_000_000, 279_000_000),
                    (870_000_000, 280_000_000),
                    (875_000_000, 280_000_000),
                    (1, 1),
                ),
                transform,
                phase(
                    Some(0),
                    "fold",
                    1,
                    (120_000_000, 10_000_000),
                    (12_000_000, 12_000_000),
                    (450_000_000, 10_000_000),
                    (4, 5),
                ),
                phase(
                    Some(3),
                    "worker",
                    4,
                    (400_000_000, 400_000_000),
                    (390_000_000, 390_000_000),
                    (1_560_000_000, 1_560_000_000),
                    (0, 5),
                ),
            ],
        },
        byte_counters: ByteCounters {
            classification: Classification::Measured,
            entries: vec![
                ByteCounter {
                    phase: 2,
                    kind: ByteKind::Proof,
                    label: "proof_frame".to_owned(),
                    count: 1,
                    total_bytes: 973_089,
                    min_bytes: 973_089,
                    max_bytes: 973_089,
                },
                ByteCounter {
                    phase: 0,
                    kind: ByteKind::Transaction,
                    label: "transfer".to_owned(),
                    count: 3,
                    total_bytes: 2100,
                    min_bytes: 600,
                    max_bytes: 800,
                },
            ],
        },
        work_counters: WorkCounters {
            classification: Classification::Measured,
            entries: vec![
                WorkCounter {
                    phase: 2,
                    label: "transform_columns".to_owned(),
                    count: 3,
                    total_units: 20,
                    min_units: 4,
                    max_units: 8,
                },
                WorkCounter {
                    phase: 4,
                    label: "fold_rows".to_owned(),
                    count: 4,
                    total_units: 4096,
                    min_units: 1024,
                    max_units: 1024,
                },
            ],
        },
        allocations: AllocationObservation {
            classification: Classification::Measured,
            sources: vec![
                AllocationSource {
                    label: "lde_columns".to_owned(),
                    kind: AllocationSourceKind::ScopedCounter,
                    allocations: 2,
                    allocated_bytes: 8192,
                    frees: 1,
                    freed_bytes: 4096,
                    live_bytes: 4096,
                    live_bytes_high_water: 8192,
                    live_buffers: 1,
                    live_buffers_high_water: 2,
                },
                AllocationSource {
                    label: "prover_pool".to_owned(),
                    kind: AllocationSourceKind::AllocationBudget,
                    allocations: 0,
                    allocated_bytes: 0,
                    frees: 0,
                    freed_bytes: 0,
                    live_bytes: 1024,
                    live_bytes_high_water: 65_536,
                    live_buffers: 0,
                    live_buffers_high_water: 0,
                },
            ],
        },
        process: ProcessObservation {
            classification: Classification::Measured,
            pid: 4242,
            logical_cpus: 20,
            cpu_user_ns: 9_000_000_000,
            cpu_system_ns: 500_000_000,
            peak_rss_bytes_at_begin: 50 << 20,
            peak_rss_bytes: 2 << 30,
            peak_rss_source: PeakRssSource::KernelLifetimeHighWater,
            load_milli_begin: 2500,
            load_milli_finish: 9800,
            thermal_begin: ThermalState::Nominal,
            thermal_finish: ThermalState::Fair,
            thermal_source: "darwin.notify.thermalpressurelevel".to_owned(),
        },
        scheduling: Scheduling {
            classification: Classification::LocalScheduling,
            workers: 20,
            workers_provenance: "rayon.current_num_threads".to_owned(),
            phase_workers: vec![PhaseWorkers {
                phase: 3,
                workers: 4,
            }],
        },
        address_space: AddressSpaceObservation {
            classification: Classification::LocalScheduling,
            source: "getrlimit.RLIMIT_AS".to_owned(),
            soft_limit_bytes: Some(32 << 30),
            hard_limit_bytes: Some(32 << 30),
            enforced: true,
        },
        declared: vec![
            DeclaredQuantity {
                classification: Classification::EngineeringTarget,
                label: "prover_seconds_target".to_owned(),
                unit: Unit::Seconds,
                value: 300,
                provenance_kind: ProvenanceKind::PlanTarget,
                provenance: "specs/zk_delivery_plan.md".to_owned(),
            },
            DeclaredQuantity {
                classification: Classification::DeterministicConsensusBound,
                label: "max_proof_bytes".to_owned(),
                unit: Unit::Bytes,
                value: 9_437_184,
                provenance_kind: ProvenanceKind::ProtocolConstant,
                provenance: "zk_x509/profile.rs:ZK_X509_MAX_PROOF_BYTES_V1".to_owned(),
            },
            DeclaredQuantity {
                classification: Classification::LocalScheduling,
                label: "prover_pool".to_owned(),
                unit: Unit::Bytes,
                value: 65_536,
                provenance_kind: ProvenanceKind::LocalConfiguration,
                provenance: "iroha_allocation.AllocationBudget.limit_bytes".to_owned(),
            },
            DeclaredQuantity {
                classification: Classification::Projection,
                label: "maximum_shape_seconds".to_owned(),
                unit: Unit::Seconds,
                value: 1200,
                provenance_kind: ProvenanceKind::ModelProjection,
                provenance: "model:linear_rows".to_owned(),
            },
        ],
        recorder: RecorderHealth::default(),
    }
}

/// A fresh empty directory under the system temporary directory.
pub fn scratch_directory(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = std::env::temp_dir().join(format!(
        "iroha-measurement-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("scratch directory");
    directory
}

/// Busy-loop until both the wall clock and this thread's CPU clock advanced
/// by at least `duration`, so timing assertions hold on a loaded host.
pub fn spin_for(duration: Duration) {
    let nanoseconds = u64::try_from(duration.as_nanos()).expect("short spin");
    let wall = Instant::now();
    let cpu = platform::thread_cpu_ns();
    let mut accumulator = 0_u64;
    loop {
        for value in 0..2_000_u64 {
            accumulator = accumulator.wrapping_mul(31).wrapping_add(value);
        }
        std::hint::black_box(accumulator);
        let cpu_done = match (cpu, platform::thread_cpu_ns()) {
            (Some(start), Some(now)) => now.saturating_sub(start) >= nanoseconds,
            _ => true,
        };
        if cpu_done && wall.elapsed() >= duration {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_directories_are_fresh_and_distinct() {
        let first = scratch_directory("support");
        let second = scratch_directory("support");
        assert_ne!(first, second);
        assert_eq!(std::fs::read_dir(&first).unwrap().count(), 0);
        std::fs::remove_dir_all(first).unwrap();
        std::fs::remove_dir_all(second).unwrap();
    }

    #[test]
    fn spin_consumes_wall_and_thread_cpu_time() {
        let wall = Instant::now();
        let cpu = platform::thread_cpu_ns();
        spin_for(Duration::from_millis(2));
        assert!(wall.elapsed() >= Duration::from_millis(2));
        if let (Some(start), Some(end)) = (cpu, platform::thread_cpu_ns()) {
            assert!(end - start >= 2_000_000);
        }
    }

    #[test]
    fn bound_context_and_sample_identity_are_complete() {
        let context = bound_context();
        assert!(crate::text::is_source_commit(&context.source_commit));
        assert!(crate::text::is_sha256_hex(
            context.source_dirty_digest.as_deref().unwrap()
        ));
        let record = sample_record();
        assert_eq!(record.identity.context, context);
        assert_eq!(record.phase_tree.nodes.len(), 5);
    }
}
