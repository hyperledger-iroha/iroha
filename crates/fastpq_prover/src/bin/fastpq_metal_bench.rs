//! FASTPQ Metal benchmark harness.
//!
//! Measures CPU vs GPU planner operations when the Metal backend is enabled
//! and emits JSON suitable for dashboards or release artefacts. See
//! `docs/source/fastpq_plan.md` (Stage 5) for expected usage.

#![allow(clippy::missing_panics_doc)]

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
fn main() {
    if let Err(error) = harness::run() {
        eprintln!("fastpq_metal_bench: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
fn main() {
    eprintln!("fastpq_metal_bench targets macOS with `fastpq-gpu`; skipping build.");
}

#[cfg(all(test, feature = "fastpq-gpu", target_os = "macos"))]
mod tests {
    use std::{
        env,
        sync::{Mutex, OnceLock},
    };

    use fastpq_prover::{
        ColumnStagingStats, KernelKind, KernelStatsSample, PostTileSample, QueueDepthStats,
        QueueLaneStats, TwiddleCacheStats,
    };
    use norito::json::Value;

    use super::harness::{
        self, BenchOperation, Config, OperationFilter, QueueDeltaAccumulator, RunState, Summary,
        ZeroFillSummary, bn254_metrics_value, classify_run, kernel_stats_value, queue_stats_value,
        round3, twiddle_cache_value, zero_fill_value,
    };

    fn assert_close(actual: f64, expected: f64) {
        let delta = (actual - expected).abs();
        assert!(
            delta < 1e-6,
            "expected {expected}, got {actual} (delta {delta})"
        );
    }

    #[test]
    fn summary_rounds_samples() {
        let samples = [1.1111, 2.2222, 3.3333];
        let summary = Summary::from_samples(&samples);
        assert_close(summary.min_ms(), 1.111);
        assert_close(summary.max_ms(), 3.333);
        assert_close(summary.mean_ms(), 2.222);
    }

    #[test]
    fn round3_matches_expected() {
        assert_close(round3(0.0), 0.0);
        assert_close(round3(1.2346), 1.235);
        assert_close(round3(1.2344), 1.234);
    }

    #[test]
    fn config_parses_trace_options() {
        let args = vec![
            "--trace-output".to_owned(),
            "bench.trace".to_owned(),
            "--trace-template".to_owned(),
            "Custom Metal Trace".to_owned(),
            "--trace-seconds".to_owned(),
            "42".to_owned(),
        ];
        let cfg = Config::from_iter(args.into_iter()).expect("trace config parses");
        let trace = cfg.trace.expect("trace options present");
        assert_eq!(trace.template, "Custom Metal Trace");
        assert_eq!(trace.seconds, 42);
        assert!(trace.output.ends_with("bench.trace"));
    }

    #[test]
    fn trace_template_requires_output() {
        let args = vec![
            "--trace-template".to_owned(),
            "Metal System Trace".to_owned(),
        ];
        assert!(Config::from_iter(args.into_iter()).is_err());
    }

    #[test]
    fn trace_auto_sets_default_output() {
        let args = vec!["--trace-auto".to_owned()];
        let cfg = Config::from_iter(args.into_iter()).expect("trace auto parses");
        let trace = cfg.trace.expect("trace present");
        assert!(trace.output.ends_with(harness::DEFAULT_TRACE_OUTPUT));
        assert_eq!(trace.template, harness::DEFAULT_TRACE_TEMPLATE);
        assert_eq!(trace.seconds, harness::DEFAULT_TRACE_SECONDS);
    }

    #[test]
    fn trace_auto_allows_template_overrides() {
        let args = vec![
            "--trace-auto".to_owned(),
            "--trace-template".to_owned(),
            "Custom Metal Trace".to_owned(),
            "--trace-seconds".to_owned(),
            "42".to_owned(),
        ];
        let cfg = Config::from_iter(args.into_iter()).expect("trace auto template parses");
        let trace = cfg.trace.expect("trace present");
        assert!(trace.output.ends_with(harness::DEFAULT_TRACE_OUTPUT));
        assert_eq!(trace.template, "Custom Metal Trace");
        assert_eq!(trace.seconds, 42);
    }

    #[test]
    fn config_parses_require_telemetry_flag() {
        let args = vec!["--require-telemetry".to_owned()];
        let cfg = Config::from_iter(args.into_iter()).expect("require telemetry parses");
        assert!(cfg.require_telemetry);
    }

    #[test]
    fn config_parses_require_gpu_flag() {
        let args = vec!["--require-gpu".to_owned()];
        let cfg = Config::from_iter(args.into_iter()).expect("require gpu parses");
        assert!(cfg.require_gpu);
    }

    #[test]
    fn trace_dir_generates_timestamped_files() {
        let args = vec![
            "--rows".to_owned(),
            "4096".to_owned(),
            "--iterations".to_owned(),
            "2".to_owned(),
            "--trace-dir".to_owned(),
            "traces".to_owned(),
        ];
        let cfg = Config::from_iter(args.into_iter()).expect("trace dir parses");
        let trace = cfg.trace.expect("trace present");
        assert!(trace.output.starts_with("traces"));
        let name = trace
            .output
            .file_name()
            .and_then(|value| value.to_str())
            .expect("trace filename");
        assert!(
            name.contains("rows4096"),
            "file name missing rows marker: {name}"
        );
        assert!(
            name.contains("iter2"),
            "file name missing iteration marker: {name}"
        );
        assert!(name.ends_with(".trace"));
    }

    #[test]
    fn trace_dir_conflicts_with_other_trace_outputs() {
        let args = vec![
            "--trace-dir".to_owned(),
            "traces".to_owned(),
            "--trace-output".to_owned(),
            "custom.trace".to_owned(),
        ];
        assert!(Config::from_iter(args.into_iter()).is_err());

        let args = vec![
            "--trace-dir".to_owned(),
            "traces".to_owned(),
            "--trace-auto".to_owned(),
        ];
        assert!(Config::from_iter(args.into_iter()).is_err());
    }

    #[test]
    fn gpu_probe_flag_enables_probe() {
        let args = vec!["--gpu-probe".to_owned()];
        let cfg = Config::from_iter(args.into_iter()).expect("gpu probe flag parses");
        assert!(cfg.gpu_probe);
    }

    #[test]
    fn operation_filter_parses_poseidon() {
        let args = vec!["--operation".to_owned(), "poseidon_hash_columns".to_owned()];
        let cfg = Config::from_iter(args.into_iter()).expect("operation parses");
        assert!(matches!(
            cfg.operation,
            OperationFilter::Only(BenchOperation::Poseidon)
        ));
        assert!(cfg.operation.includes(BenchOperation::Poseidon));
        assert!(!cfg.operation.includes(BenchOperation::Fft));
    }

    #[test]
    fn zero_fill_value_includes_bytes_and_ms() {
        let summary = Summary::from_samples(&[1.0, 2.0, 3.0]);
        let stats = ZeroFillSummary {
            bytes: 4096,
            ms: summary,
            queue_delta: None,
        };
        let value = zero_fill_value(&stats);
        let object = value.as_object().expect("zero_fill serializes to object");
        assert_eq!(
            object
                .get("bytes")
                .and_then(|v| v.as_u64())
                .expect("bytes present"),
            4096
        );
        let ms = object
            .get("ms")
            .and_then(|v| v.as_object())
            .expect("ms present");
        assert!(ms.contains_key("mean_ms"));
        assert!(ms.contains_key("min_ms"));
        assert!(ms.contains_key("max_ms"));
        assert!(!object.contains_key("queue_delta"));
    }

    #[test]
    fn zero_fill_value_serializes_queue_delta() {
        let summary = Summary::from_samples(&[1.0, 1.0, 1.0]);
        let stats = ZeroFillSummary {
            bytes: 1024,
            ms: summary,
            queue_delta: Some(QueueDepthStats {
                limit: 4,
                dispatch_count: 3,
                max_in_flight: 2,
                busy_ms: 0.75,
                overlap_ms: 0.25,
                window_ms: 0.75,
                queues: Vec::new(),
            }),
        };
        let value = zero_fill_value(&stats);
        let object = value.as_object().expect("zero_fill serializes to object");
        assert!(object.contains_key("queue_delta"));
        let queue = object
            .get("queue_delta")
            .and_then(|v| v.as_object())
            .expect("queue delta present");
        assert_eq!(
            queue
                .get("dispatch_count")
                .and_then(|v| v.as_u64())
                .expect("dispatch count present"),
            3
        );
    }

    #[test]
    fn queue_delta_accumulator_merges_samples() {
        let mut accumulator = QueueDeltaAccumulator::default();
        accumulator.record(QueueDepthStats {
            limit: 4,
            dispatch_count: 1,
            max_in_flight: 1,
            busy_ms: 0.1,
            overlap_ms: 0.05,
            window_ms: 0.1,
            queues: Vec::new(),
        });
        accumulator.record(QueueDepthStats {
            limit: 4,
            dispatch_count: 2,
            max_in_flight: 2,
            busy_ms: 0.2,
            overlap_ms: 0.1,
            window_ms: 0.2,
            queues: Vec::new(),
        });
        let merged = accumulator.finalize().expect("merged stats");
        assert_eq!(merged.dispatch_count, 3);
        assert_eq!(merged.max_in_flight, 2);
        assert!((merged.busy_ms - 0.3).abs() < f64::EPSILON);
        assert!((merged.overlap_ms - 0.15).abs() < f64::EPSILON);
    }

    #[test]
    fn queue_stats_value_serializes_lane_metrics() {
        let stats = QueueDepthStats {
            limit: 4,
            dispatch_count: 4,
            max_in_flight: 2,
            busy_ms: 1.0,
            overlap_ms: 0.5,
            window_ms: 1.5,
            queues: vec![
                QueueLaneStats {
                    index: 0,
                    dispatch_count: 2,
                    max_in_flight: 2,
                    busy_ms: 0.8,
                    overlap_ms: 0.4,
                },
                QueueLaneStats {
                    index: 1,
                    dispatch_count: 2,
                    max_in_flight: 1,
                    busy_ms: 0.2,
                    overlap_ms: 0.1,
                },
            ],
        };
        let value = queue_stats_value(&stats);
        let object = value.as_object().expect("queue stats serialize to object");
        assert!(object.contains_key("window_ms"));
        assert!(object.contains_key("busy_ratio"));
        let queues = object
            .get("queues")
            .and_then(|v| v.as_array())
            .expect("queue stats include lanes");
        assert_eq!(queues.len(), 2);
        let first = queues[0].as_object().expect("lane object");
        assert!(first.contains_key("busy_ratio"));
        assert_eq!(
            first
                .get("index")
                .and_then(|v| v.as_u64())
                .expect("lane index"),
            0
        );
        assert!(first.contains_key("overlap_ratio"));
    }

    #[test]
    fn kernel_stats_summary_captures_samples() {
        let samples = vec![
            KernelStatsSample {
                kind: KernelKind::Fft,
                bytes: 1_000_000,
                elements: 2048,
                column_count: 4,
                logical_threads: 256,
                threadgroup_width: 32,
                threadgroups: 8,
                execution_width: 32,
                max_threads_per_group: 64,
                duration_ms: 2.0,
            },
            KernelStatsSample {
                kind: KernelKind::Lde,
                bytes: 500_000,
                elements: 1024,
                column_count: 2,
                logical_threads: 128,
                threadgroup_width: 16,
                threadgroups: 16,
                execution_width: 32,
                max_threads_per_group: 64,
                duration_ms: 1.0,
            },
        ];
        let value = kernel_stats_value(&samples);
        let map = value
            .as_object()
            .expect("kernel profiles serialize to object");
        assert_eq!(
            map.get("total_samples")
                .and_then(|v| v.as_u64())
                .expect("total_samples present"),
            2
        );
        let by_kind = map
            .get("by_kind")
            .and_then(|v| v.as_object())
            .expect("by_kind present");
        assert!(by_kind.contains_key("fft"));
        assert!(by_kind.contains_key("lde"));
        let fft = by_kind
            .get("fft")
            .and_then(|v| v.as_object())
            .expect("fft block");
        assert_eq!(
            fft.get("sample_count")
                .and_then(|v| v.as_u64())
                .expect("fft sample count"),
            1
        );
        let samples_array = fft
            .get("samples")
            .and_then(|v| v.as_array())
            .expect("fft samples array");
        assert_eq!(samples_array.len(), 1);
    }

    #[test]
    fn bn254_dispatch_summary_filters_single_group_samples() {
        let samples = vec![
            KernelStatsSample {
                kind: KernelKind::Fft,
                bytes: 1_000,
                elements: 1024,
                column_count: 1,
                logical_threads: 1024,
                threadgroup_width: 256,
                threadgroups: 1,
                execution_width: 32,
                max_threads_per_group: 512,
                duration_ms: 1.0,
            },
            KernelStatsSample {
                kind: KernelKind::Lde,
                bytes: 2_000,
                elements: 2048,
                column_count: 1,
                logical_threads: 2048,
                threadgroup_width: 128,
                threadgroups: 1,
                execution_width: 32,
                max_threads_per_group: 512,
                duration_ms: 2.0,
            },
            // Non-BN254 sample (multiple threadgroups) should be ignored.
            KernelStatsSample {
                kind: KernelKind::Fft,
                bytes: 42,
                elements: 512,
                column_count: 4,
                logical_threads: 2048,
                threadgroup_width: 64,
                threadgroups: 4,
                execution_width: 32,
                max_threads_per_group: 512,
                duration_ms: 0.5,
            },
        ];
        let summary =
            harness::bn254_dispatch_summary(&samples).expect("bn254 dispatch summary present");
        let fft = summary
            .get("fft")
            .and_then(Value::as_object)
            .expect("fft block present");
        assert_eq!(fft.get("sample_count").and_then(Value::as_u64).unwrap(), 1);
        let widths = fft
            .get("threadgroup_width")
            .and_then(Value::as_object)
            .expect("width stats present");
        assert_eq!(widths.get("min").and_then(Value::as_u64).unwrap(), 256);
        let lde = summary
            .get("lde")
            .and_then(Value::as_object)
            .expect("lde block present");
        assert_eq!(
            lde.get("logical_threads")
                .and_then(Value::as_object)
                .and_then(|m| m.get("max"))
                .and_then(Value::as_u64)
                .unwrap(),
            2048
        );
    }

    #[test]
    fn twiddle_cache_value_reports_savings() {
        let stats = TwiddleCacheStats {
            hits: 3,
            misses: 1,
            before_ms: 4.5,
            after_ms: 1.5,
        };
        let value = twiddle_cache_value(&stats);
        let object = value
            .as_object()
            .expect("twiddle cache value serializes to object");
        assert_eq!(
            object
                .get("hits")
                .and_then(|v| v.as_u64())
                .expect("hits present"),
            3
        );
        assert_eq!(
            object
                .get("misses")
                .and_then(|v| v.as_u64())
                .expect("misses present"),
            1
        );
        assert!(
            object
                .get("before_ms")
                .and_then(|v| v.as_f64())
                .expect("before present")
                .abs()
                > 0.0
        );
        assert_eq!(
            object
                .get("savings_ms")
                .and_then(|v| v.as_f64())
                .expect("savings present"),
            round3(stats.before_ms - stats.after_ms)
        );
    }

    #[test]
    fn post_tile_value_captures_dispatches() {
        let samples = vec![
            PostTileSample {
                kind: KernelKind::Fft,
                log_len: 15,
                stage_start: 12,
                columns: 8,
            },
            PostTileSample {
                kind: KernelKind::Fft,
                log_len: 15,
                stage_start: 12,
                columns: 8,
            },
            PostTileSample {
                kind: KernelKind::Lde,
                log_len: 20,
                stage_start: 16,
                columns: 4,
            },
        ];
        let value = harness::post_tile_value(&samples);
        let object = value
            .as_object()
            .expect("post-tile value serializes to object");
        assert_eq!(
            object
                .get("total_dispatches")
                .and_then(|v| v.as_u64())
                .expect("total dispatches present"),
            3
        );
        let kinds = object
            .get("kinds")
            .and_then(|v| v.as_array())
            .expect("kinds array present");
        assert_eq!(kinds.len(), 2);
    }

    #[test]
    fn poseidon_micro_mode_env_parser_handles_overrides() {
        assert_eq!(
            super::harness::parse_poseidon_micro_mode("default"),
            Some(super::harness::PoseidonMicroMode::Default)
        );
        assert_eq!(
            super::harness::parse_poseidon_micro_mode("scalar"),
            Some(super::harness::PoseidonMicroMode::Scalar)
        );
        assert!(
            super::harness::parse_poseidon_micro_mode("invalid").is_none(),
            "unexpected override parsed"
        );
    }

    #[test]
    fn fastpq_gpu_override_detects_gpu_synonyms() {
        for value in ["gpu", "GPU", "enable", "ENABLED", "1", " on "] {
            assert!(
                super::harness::fastpq_gpu_forced_raw(Some(value)),
                "expected FASTPQ_GPU override to force GPU for value {value:?}"
            );
        }
    }

    #[test]
    fn fastpq_gpu_override_handles_cpu_synonyms() {
        for value in ["cpu", "off", "disable", "DISABLED", "0"] {
            assert!(
                !super::harness::fastpq_gpu_forced_raw(Some(value)),
                "expected FASTPQ_GPU override to disable GPU for value {value:?}"
            );
        }
    }

    #[test]
    fn fastpq_gpu_override_defaults_to_false() {
        assert!(
            !super::harness::fastpq_gpu_forced_raw(None),
            "FASTPQ_GPU unset should not force GPU execution"
        );
        assert!(
            !super::harness::fastpq_gpu_forced_raw(Some("   ")),
            "blank FASTPQ_GPU values should not force GPU execution"
        );
    }
}

#[cfg(all(test, feature = "fastpq-gpu", target_os = "macos"))]
mod heuristics_value_tests {
    use fastpq_prover::{
        AdaptiveScheduleSnapshot, BatchHeuristicSnapshot, CommandLimitSnapshot, CommandLimitSource,
    };

    use super::harness::heuristics_value;

    #[test]
    fn poseidon_snapshot_serializes() {
        let snapshot = AdaptiveScheduleSnapshot {
            max_in_flight: Some(CommandLimitSnapshot {
                limit: 4,
                queue_floor: 2,
                auto_limit: 4,
                source: CommandLimitSource::GpuCores,
                gpu_cores: Some(8),
                cpu_parallelism: None,
                override_limit: None,
            }),
            fft: None,
            lde: None,
            poseidon: Some(BatchHeuristicSnapshot {
                columns: 64,
                recommended: 64,
                max_columns: 128,
                target_ms: 2.0,
                last_duration_ms: Some(1.5),
                samples: 3,
                override_active: false,
            }),
            ..Default::default()
        };
        let value = heuristics_value(&snapshot).expect("heuristics serialized");
        let object = value.as_object().expect("heuristics map");
        assert!(object.contains_key("max_in_flight"));
        let batches = object
            .get("batch_columns")
            .and_then(|v| v.as_object())
            .expect("batch columns present");
        let poseidon = batches
            .get("poseidon")
            .and_then(|v| v.as_object())
            .expect("poseidon snapshot present");
        assert_eq!(
            poseidon
                .get("columns")
                .and_then(|v| v.as_u64())
                .expect("columns present"),
            64
        );
        assert_eq!(
            poseidon
                .get("samples")
                .and_then(|v| v.as_u64())
                .expect("samples present"),
            3
        );
    }
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
mod harness {
    use std::{
        collections::BTreeMap,
        env, fs,
        hint::black_box,
        mem,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::{Arc, Mutex},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use fastpq_isi::{
        find_by_name,
        poseidon::{FIELD_MODULUS as GOLDILOCKS_MODULUS, PoseidonSponge as CpuPoseidonSponge},
    };
    use fastpq_prover::{
        AdaptiveScheduleSnapshot, BatchHeuristicSnapshot, ColumnStagingPhaseStats,
        ColumnStagingSample, ColumnStagingStats, CommandLimitSnapshot, ExecutionMode, FftTuning,
        KernelKind, KernelStatsSample, LdeHostStats, Planner, PoseidonPipelineStats,
        PostTileSample, QueueDepthStats, TwiddleCacheStats, adaptive_schedule_snapshot,
        clear_execution_mode_observer, enable_kernel_stats, enable_lde_host_stats,
        enable_poseidon_pipeline_stats, enable_post_tile_stats, enable_queue_depth_stats,
        enable_twiddle_cache_stats, fft_tuning_snapshot, poseidon_tuning_snapshot,
        set_execution_mode_observer, snapshot_queue_depth_stats, take_column_staging_stats,
        take_kernel_stats, take_lde_host_stats, take_poseidon_pipeline_stats, take_post_tile_stats,
        take_queue_depth_stats, take_twiddle_cache_stats,
        trace::{PoseidonColumnBatch, hash_columns_gpu_batch, hash_columns_gpu_fused},
    };
    use iroha_crypto::Hash;
    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    use metal::{Device, MTLDeviceLocation};
    use norito::json::{self, Value};

    fn debug_env_var(name: &str) -> Option<String> {
        #[cfg(any(test, debug_assertions))]
        {
            return env::var(name).ok();
        }
        #[cfg(not(any(test, debug_assertions)))]
        {
            let _ = name;
            None
        }
    }

    fn debug_env_bool(name: &str) -> Option<bool> {
        debug_env_var(name).map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
    }

    pub(super) const DEFAULT_TRACE_TEMPLATE: &str = "Metal System Trace";
    pub(super) const DEFAULT_TRACE_OUTPUT: &str = "fastpq.trace";
    pub(super) const DEFAULT_TRACE_SECONDS: u32 = 60;
    const TRACE_REEXEC_ENV: &str = "FASTPQ_METAL_TRACE_CHILD";
    const POSEIDON_MICRO_ENV: &str = "FASTPQ_METAL_POSEIDON_MICRO_MODE";
    const POSEIDON_MICRO_COLUMNS: usize = 64;
    const POSEIDON_MICRO_TRACE_LOG: u32 = 12;
    const POSEIDON_MICRO_WARMUPS: usize = 1;
    const POSEIDON_MICRO_ITERATIONS: usize = 5;
    const POSEIDON_MICRO_SCALAR_LANES: &str = "32";
    const POSEIDON_MICRO_SCALAR_BATCH: &str = "1";

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum PoseidonMicroMode {
        Default,
        Scalar,
    }

    impl PoseidonMicroMode {
        fn as_str(self) -> &'static str {
            match self {
                PoseidonMicroMode::Default => "default",
                PoseidonMicroMode::Scalar => "scalar",
            }
        }
    }

    #[derive(Debug, Clone)]
    pub(crate) struct TraceOptions {
        pub(crate) output: PathBuf,
        pub(crate) template: String,
        pub(crate) seconds: u32,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct Config {
        pub(crate) rows: usize,
        pub(crate) warmups: usize,
        pub(crate) iterations: usize,
        pub(crate) output: Option<PathBuf>,
        pub(crate) trace: Option<TraceOptions>,
        pub(crate) operation: OperationFilter,
        pub(crate) gpu_probe: bool,
        pub(crate) require_gpu: bool,
        pub(crate) require_telemetry: bool,
    }

    impl Config {
        const DEFAULT_ROWS: usize = 20_000;
        const DEFAULT_WARMUPS: usize = 1;
        const DEFAULT_ITERATIONS: usize = 5;

        fn parse() -> Result<Self, String> {
            Self::from_iter(env::args().skip(1))
        }

        pub(crate) fn from_iter<I>(mut args: I) -> Result<Self, String>
        where
            I: Iterator<Item = String>,
        {
            let mut rows = Self::DEFAULT_ROWS;
            let mut warmups = Self::DEFAULT_WARMUPS;
            let mut iterations = Self::DEFAULT_ITERATIONS;
            let mut output = None;
            let mut trace_output = None;
            let mut trace_dir = None;
            let mut trace_auto = false;
            let mut trace_template = None;
            let mut trace_seconds = None;
            let mut operation = OperationFilter::All;
            let mut gpu_probe = false;
            let mut require_gpu = false;
            let mut require_telemetry = false;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--rows" => rows = parse_usize(&mut args, "--rows")?,
                    "--warmups" => warmups = parse_usize(&mut args, "--warmups")?,
                    "--iterations" => iterations = parse_usize(&mut args, "--iterations")?,
                    "--output" => {
                        let value = args
                            .next()
                            .ok_or_else(|| "missing value after --output".to_owned())?;
                        output = Some(PathBuf::from(value));
                    }
                    "--trace-output" => {
                        let value = args
                            .next()
                            .ok_or_else(|| "missing value after --trace-output".to_owned())?;
                        trace_output = Some(PathBuf::from(value));
                    }
                    "--trace-dir" => {
                        let value = args
                            .next()
                            .ok_or_else(|| "missing value after --trace-dir".to_owned())?;
                        trace_dir = Some(PathBuf::from(value));
                    }
                    "--trace-auto" => {
                        trace_auto = true;
                    }
                    "--trace-template" => {
                        let value = args
                            .next()
                            .ok_or_else(|| "missing value after --trace-template".to_owned())?;
                        trace_template = Some(value);
                    }
                    "--trace-seconds" => {
                        trace_seconds = Some(parse_u32(&mut args, "--trace-seconds")?);
                    }
                    "--operation" => {
                        let value = args
                            .next()
                            .ok_or_else(|| "missing value after --operation".to_owned())?;
                        operation = OperationFilter::parse(&value)?;
                    }
                    "--require-gpu" => {
                        require_gpu = true;
                    }
                    "--require-telemetry" => {
                        require_telemetry = true;
                    }
                    "--gpu-probe" => {
                        gpu_probe = true;
                    }
                    "--help" | "-h" => {
                        print_usage();
                        std::process::exit(0);
                    }
                    other => {
                        return Err(format!("unknown argument '{other}' (use --help for usage)"));
                    }
                }
            }
            if rows == 0 {
                return Err("--rows must be greater than zero".into());
            }
            if iterations == 0 {
                return Err("--iterations must be greater than zero".into());
            }
            if trace_output.is_some() && trace_auto {
                return Err("--trace-auto cannot be combined with --trace-output".to_owned());
            }
            if trace_output.is_some() && trace_dir.is_some() {
                return Err("--trace-dir cannot be combined with --trace-output".to_owned());
            }
            if trace_dir.is_some() && trace_auto {
                return Err("--trace-dir cannot be combined with --trace-auto".to_owned());
            }
            let trace = if let Some(output) = trace_output {
                let template = trace_template.unwrap_or_else(|| DEFAULT_TRACE_TEMPLATE.to_owned());
                let seconds = trace_seconds.unwrap_or(DEFAULT_TRACE_SECONDS);
                Some(TraceOptions {
                    output,
                    template,
                    seconds,
                })
            } else if let Some(dir) = trace_dir {
                let output = auto_trace_output(&dir, rows, iterations)?;
                let template = trace_template.unwrap_or_else(|| DEFAULT_TRACE_TEMPLATE.to_owned());
                let seconds = trace_seconds.unwrap_or(DEFAULT_TRACE_SECONDS);
                Some(TraceOptions {
                    output,
                    template,
                    seconds,
                })
            } else if trace_auto {
                let template = trace_template.unwrap_or_else(|| DEFAULT_TRACE_TEMPLATE.to_owned());
                let seconds = trace_seconds.unwrap_or(DEFAULT_TRACE_SECONDS);
                Some(TraceOptions {
                    output: PathBuf::from(DEFAULT_TRACE_OUTPUT),
                    template,
                    seconds,
                })
            } else {
                if trace_template.is_some() || trace_seconds.is_some() {
                    return Err(
                        "--trace-template/--trace-seconds require --trace-output, --trace-dir, or --trace-auto"
                            .to_owned(),
                    );
                }
                None
            };
            Ok(Self {
                rows,
                warmups,
                iterations,
                output,
                trace,
                operation,
                gpu_probe,
                require_gpu,
                require_telemetry,
            })
        }
    }

    fn print_usage() {
        println!(concat!(
            "Usage: fastpq_metal_bench [--rows <u32>] [--warmups <u32>] [--iterations <u32>] [--output <path>] ",
            "[--trace-auto] [--trace-dir <dir>] [--trace-output <path>] [--trace-template <name>] [--trace-seconds <u32>] ",
            "[--operation <fft|ifft|lde|poseidon_hash_columns|all>] [--require-gpu] [--require-telemetry] [--gpu-probe]\n",
            "             Defaults: rows=20000 warmups=1 iterations=5; output omitted prints JSON to stdout; --operation defaults to 'all'.\n",
            "             `--trace-auto` captures to ./fastpq.trace, `--trace-dir` writes timestamped traces underneath the provided directory,\n",
            "             and other tracing flags launch under `xcrun xctrace record` (Metal System Trace by default).\n",
            "             `--require-gpu` exits immediately when Metal is unavailable so sweeps do not record CPU fallbacks as GPU runs.\n",
            "             `--require-telemetry` fails the run when GPU queue/staging telemetry is missing so WP2-E captures do not silently fall back.\n",
            "             `--gpu-probe` emits a GPU detection snapshot (override, resolved mode, enumerated Metal devices) before running the bench."
        ));
    }

    fn auto_trace_output(dir: &Path, rows: usize, iterations: usize) -> Result<PathBuf, String> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX epoch: {err}"))?
            .as_secs();
        let file_name = format!("fastpq_metal_trace_{timestamp}_rows{rows}_iter{iterations}.trace");
        Ok(dir.join(file_name))
    }

    fn parse_usize<I>(args: &mut I, flag: &str) -> Result<usize, String>
    where
        I: Iterator<Item = String>,
    {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value after {flag}"))?;
        value
            .parse::<usize>()
            .map_err(|err| format!("{flag} expects a positive integer: {err}"))
    }

    fn parse_u32<I>(args: &mut I, flag: &str) -> Result<u32, String>
    where
        I: Iterator<Item = String>,
    {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value after {flag}"))?;
        value
            .parse::<u32>()
            .map_err(|err| format!("{flag} expects a positive integer: {err}"))
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum OperationFilter {
        All,
        Only(BenchOperation),
    }

    impl OperationFilter {
        fn parse(raw: &str) -> Result<Self, String> {
            if raw.eq_ignore_ascii_case("all") {
                return Ok(Self::All);
            }
            let op = raw
                .parse::<BenchOperation>()
                .map_err(|_| format!("unknown --operation '{raw}'"))?;
            Ok(Self::Only(op))
        }

        pub(crate) fn includes(&self, op: BenchOperation) -> bool {
            matches!(self, Self::All) || matches!(self, Self::Only(single) if *single == op)
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum BenchOperation {
        Fft,
        Ifft,
        Lde,
        Poseidon,
    }

    impl BenchOperation {
        fn as_str(&self) -> &'static str {
            match self {
                Self::Fft => "fft",
                Self::Ifft => "ifft",
                Self::Lde => "lde",
                Self::Poseidon => "poseidon_hash_columns",
            }
        }
    }

    impl std::str::FromStr for BenchOperation {
        type Err = ();

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "fft" => Ok(Self::Fft),
                "ifft" => Ok(Self::Ifft),
                "lde" => Ok(Self::Lde),
                "poseidon_hash_columns" => Ok(Self::Poseidon),
                _ => Err(()),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct Summary {
        mean: f64,
        min: f64,
        max: f64,
    }

    impl Summary {
        pub fn from_samples(samples: &[f64]) -> Self {
            debug_assert!(!samples.is_empty());
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            let mut sum = 0.0;
            for &sample in samples {
                min = min.min(sample);
                max = max.max(sample);
                sum += sample;
            }
            let sample_count =
                u32::try_from(samples.len()).expect("sample count fits into u32::MAX");
            let mean = sum / f64::from(sample_count);
            Self {
                mean: round3(mean),
                min: round3(min),
                max: round3(max),
            }
        }

        pub fn min_ms(&self) -> f64 {
            self.min
        }

        pub fn max_ms(&self) -> f64 {
            self.max
        }

        pub fn mean_ms(&self) -> f64 {
            self.mean
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct Speedup {
        ratio: f64,
        delta_ms: f64,
    }

    impl Speedup {
        fn between(cpu: &Summary, gpu: &Summary) -> Self {
            let ratio = match gpu.mean_ms() {
                value if value <= f64::EPSILON => f64::INFINITY,
                value => cpu.mean_ms() / value,
            };
            let delta = cpu.mean_ms() - gpu.mean_ms();
            Self {
                ratio: round3(ratio),
                delta_ms: round3(delta),
            }
        }
    }

    pub fn round3(value: f64) -> f64 {
        (value * 1_000.0).round() / 1_000.0
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum RunState {
        Ok,
        GpuUnavailable,
        GpuFallback,
        TelemetryMissing,
    }

    impl RunState {
        fn as_str(self) -> &'static str {
            match self {
                Self::Ok => "ok",
                Self::GpuUnavailable => "gpu_unavailable",
                Self::GpuFallback => "gpu_fallback",
                Self::TelemetryMissing => "telemetry_missing",
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RunStatus {
        state: RunState,
        reasons: Vec<String>,
        dispatch_count: Option<u32>,
        gpu_available: bool,
        backend_label: String,
    }

    impl RunStatus {
        fn value(&self) -> Value {
            let mut map = json::Map::new();
            map.insert(
                "state".into(),
                json::to_value(self.state.as_str()).expect("serialize run state"),
            );
            map.insert(
                "reasons".into(),
                json::to_value(&self.reasons).expect("serialize reasons"),
            );
            map.insert(
                "gpu_available".into(),
                json::to_value(&self.gpu_available).expect("serialize gpu flag"),
            );
            map.insert(
                "gpu_backend".into(),
                json::to_value(&self.backend_label).expect("serialize backend"),
            );
            if let Some(dispatches) = self.dispatch_count {
                map.insert(
                    "dispatch_count".into(),
                    json::to_value(&dispatches).expect("serialize dispatch count"),
                );
            }
            Value::Object(map)
        }

        fn is_ok(&self) -> bool {
            self.state == RunState::Ok
        }
    }

    #[derive(Debug, Clone)]
    pub(crate) struct ZeroFillSummary {
        pub(crate) bytes: usize,
        pub(crate) ms: Summary,
        pub(crate) queue_delta: Option<QueueDepthStats>,
    }

    #[derive(Default)]
    struct ZeroFillAccumulator {
        bytes: Option<usize>,
        samples: Vec<f64>,
        queue_delta: Option<QueueDepthStats>,
    }

    impl ZeroFillAccumulator {
        fn record(&mut self, stats: LdeHostStats, fallback_delta: Option<QueueDepthStats>) {
            if let Some(bytes) = self.bytes {
                debug_assert_eq!(
                    bytes, stats.zero_fill_bytes,
                    "zero-fill bytes changed between samples"
                );
            } else {
                self.bytes = Some(stats.zero_fill_bytes);
            }
            self.samples.push(stats.zero_fill_ms);
            let delta = stats.queue_delta.or(fallback_delta);
            if let Some(delta) = delta {
                if let Some(existing) = &mut self.queue_delta {
                    existing.accumulate_delta(&delta);
                } else {
                    self.queue_delta = Some(delta);
                }
            }
        }

        fn finish(self) -> Option<ZeroFillSummary> {
            if self.samples.is_empty() {
                return None;
            }
            let bytes = self.bytes.unwrap_or(0);
            Some(ZeroFillSummary {
                bytes,
                ms: Summary::from_samples(&self.samples),
                queue_delta: self.queue_delta,
            })
        }
    }

    #[derive(Default)]
    pub(crate) struct QueueDeltaAccumulator {
        total: Option<QueueDepthStats>,
    }

    impl QueueDeltaAccumulator {
        pub(crate) fn record(&mut self, delta: QueueDepthStats) {
            if let Some(existing) = &mut self.total {
                existing.accumulate_delta(&delta);
            } else {
                self.total = Some(delta);
            }
        }

        pub(crate) fn finalize(self) -> Option<QueueDepthStats> {
            self.total
        }
    }

    fn measure_in_place<F>(
        source: &[Vec<u64>],
        warmups: usize,
        iterations: usize,
        mut op: F,
    ) -> Summary
    where
        F: FnMut(&mut [Vec<u64>]),
    {
        for _ in 0..warmups {
            let mut data = source.to_vec();
            op(&mut data);
            black_box(data);
        }
        let mut samples = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let mut data = source.to_vec();
            let started = Instant::now();
            op(&mut data);
            samples.push(elapsed_ms(started.elapsed()));
            black_box(data);
        }
        Summary::from_samples(&samples)
    }

    fn measure_map<F, R>(source: &[Vec<u64>], warmups: usize, iterations: usize, op: F) -> Summary
    where
        F: Fn(&[Vec<u64>]) -> R,
    {
        for _ in 0..warmups {
            let result = op(source);
            black_box(result);
        }
        let mut samples = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let started = Instant::now();
            let result = op(source);
            samples.push(elapsed_ms(started.elapsed()));
            black_box(result);
        }
        Summary::from_samples(&samples)
    }

    fn measure_gpu_lde(
        planner: &Planner,
        source: &[Vec<u64>],
        warmups: usize,
        iterations: usize,
    ) -> (Summary, Option<ZeroFillSummary>) {
        enable_lde_host_stats(true);
        let mut zero_fill = ZeroFillAccumulator::default();
        for _ in 0..warmups {
            let result = planner.lde_gpu(source);
            black_box(result);
            take_lde_host_stats();
        }
        let mut samples = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let queue_before = snapshot_queue_depth_stats();
            let started = Instant::now();
            let result = planner.lde_gpu(source);
            samples.push(elapsed_ms(started.elapsed()));
            black_box(result);
            let queue_after = snapshot_queue_depth_stats();
            let queue_delta = match (queue_before, queue_after) {
                (Some(before), Some(after)) => Some(after.delta_since(&before)),
                _ => None,
            };
            if let Some(stats) = take_lde_host_stats() {
                zero_fill.record(stats, queue_delta);
            }
        }
        enable_lde_host_stats(false);
        (Summary::from_samples(&samples), zero_fill.finish())
    }

    fn measure_poseidon_gpu(
        domains: &[String],
        source: &[Vec<u64>],
        warmups: usize,
        iterations: usize,
    ) -> (Option<Summary>, Option<QueueDepthStats>) {
        for _ in 0..warmups {
            match hash_columns_gpu(domains, source) {
                PoseidonGpuOutcome::Gpu(values) | PoseidonGpuOutcome::CpuFallback(values) => {
                    drop(black_box(values));
                }
            }
        }
        if iterations == 0 {
            return (None, None);
        }
        let mut samples = Vec::with_capacity(iterations);
        let mut queue = QueueDeltaAccumulator::default();
        let mut fallback_detected = false;
        for _ in 0..iterations {
            let before = snapshot_queue_depth_stats();
            let started = Instant::now();
            let elapsed;
            match hash_columns_gpu(domains, source) {
                PoseidonGpuOutcome::Gpu(values) => {
                    elapsed = elapsed_ms(started.elapsed());
                    drop(black_box(values));
                    samples.push(elapsed);
                }
                PoseidonGpuOutcome::CpuFallback(values) => {
                    drop(black_box(values));
                    fallback_detected = true;
                    break;
                }
            }
            if let (Some(prev), Some(after)) = (before, snapshot_queue_depth_stats()) {
                queue.record(after.delta_since(&prev));
            }
        }
        let queue_snapshot = queue.finalize();
        if fallback_detected || samples.is_empty() {
            eprintln!(
                "fastpq_metal_bench: Poseidon GPU hashing fell back to CPU; skipping GPU timings"
            );
            (None, queue_snapshot)
        } else {
            (Some(Summary::from_samples(&samples)), queue_snapshot)
        }
    }

    fn elapsed_ms(duration: Duration) -> f64 {
        duration.as_secs_f64() * 1_000.0
    }

    fn generated_columns(len: usize, column_count: usize, seed: u64) -> Vec<Vec<u64>> {
        let mut columns = Vec::with_capacity(column_count);
        for column in 0..column_count {
            let mut data = Vec::with_capacity(len);
            let column_seed = seed.wrapping_add((column as u64).wrapping_mul(0x9e37_79b9));
            let rotate_value = (column % 31) + 1;
            let rotate = u32::try_from(rotate_value).expect("rotate amount fits into u32");
            for idx in 0..len {
                let raw = column_seed
                    ^ ((idx as u64).rotate_left(rotate))
                    ^ ((idx as u64).wrapping_mul(0x2545_f491_4f6c_dd1d));
                data.push(raw % GOLDILOCKS_MODULUS);
            }
            columns.push(data);
        }
        columns
    }

    fn build_domains(column_count: usize) -> Vec<String> {
        (0..column_count)
            .map(|idx| format!("fastpq:v1:trace:column:bench_{idx:02}"))
            .collect()
    }

    fn domain_seed(bytes: &[u8]) -> u64 {
        let digest = Hash::new(bytes);
        let mut chunk = [0u8; 8];
        chunk.copy_from_slice(&digest.as_ref()[..8]);
        let reduced = u128::from(u64::from_le_bytes(chunk)) % u128::from(GOLDILOCKS_MODULUS);
        u64::try_from(reduced).expect("reduction fits into u64")
    }

    fn hash_columns_cpu(domains: &[String], columns: &[Vec<u64>]) -> Vec<u64> {
        domains
            .iter()
            .zip(columns.iter())
            .map(|(domain, values)| {
                let mut sponge = CpuPoseidonSponge::new();
                sponge.absorb(domain_seed(domain.as_bytes()));
                sponge.absorb_slice(values);
                sponge.squeeze()
            })
            .collect()
    }

    enum PoseidonGpuOutcome {
        Gpu(Vec<u64>),
        CpuFallback(Vec<u64>),
    }

    fn hash_columns_gpu(domains: &[String], columns: &[Vec<u64>]) -> PoseidonGpuOutcome {
        let domain_refs: Vec<&str> = domains.iter().map(|domain| domain.as_str()).collect();
        if let Some(batch) = PoseidonColumnBatch::from_domains_and_columns(&domain_refs, columns) {
            if let Some(fused) = hash_columns_gpu_fused(&batch, ExecutionMode::Gpu) {
                return PoseidonGpuOutcome::Gpu(fused.leaves);
            }
            if let Some(result) = hash_columns_gpu_batch(&batch) {
                return PoseidonGpuOutcome::Gpu(result);
            }
        }
        PoseidonGpuOutcome::CpuFallback(hash_columns_cpu(domains, columns))
    }

    fn summary_value(summary: &Summary) -> Value {
        let mut map = json::Map::new();
        map.insert(
            "mean_ms".into(),
            json::to_value(&summary.mean_ms()).expect("serialize mean"),
        );
        map.insert(
            "min_ms".into(),
            json::to_value(&summary.min_ms()).expect("serialize min"),
        );
        map.insert(
            "max_ms".into(),
            json::to_value(&summary.max_ms()).expect("serialize max"),
        );
        Value::Object(map)
    }

    fn speedup_value(cpu: &Summary, gpu: &Summary) -> Value {
        let comparison = Speedup::between(cpu, gpu);
        let mut map = json::Map::new();
        map.insert(
            "ratio".into(),
            json::to_value(&comparison.ratio).expect("serialize ratio"),
        );
        map.insert(
            "delta_ms".into(),
            json::to_value(&comparison.delta_ms).expect("serialize delta"),
        );
        Value::Object(map)
    }

    pub(crate) fn zero_fill_value(stats: &ZeroFillSummary) -> Value {
        let mut map = json::Map::new();
        map.insert(
            "bytes".into(),
            json::to_value(&stats.bytes).expect("serialize zero-fill bytes"),
        );
        map.insert("ms".into(), summary_value(&stats.ms));
        if let Some(delta) = stats.queue_delta.as_ref() {
            map.insert("queue_delta".into(), queue_stats_value(delta));
        }
        Value::Object(map)
    }

    fn column_staging_phase_value(stats: &ColumnStagingPhaseStats) -> json::Map {
        let total = stats.flatten_ms + stats.wait_ms;
        let wait_ratio = if total <= f64::EPSILON {
            0.0
        } else {
            round3(stats.wait_ms / total)
        };
        let mut map = json::Map::new();
        map.insert(
            "batches".into(),
            json::to_value(&stats.batches).expect("serialize staging batches"),
        );
        map.insert(
            "flatten_ms".into(),
            json::to_value(&round3(stats.flatten_ms)).expect("serialize flatten ms"),
        );
        map.insert(
            "wait_ms".into(),
            json::to_value(&round3(stats.wait_ms)).expect("serialize wait ms"),
        );
        map.insert(
            "wait_ratio".into(),
            json::to_value(&wait_ratio).expect("serialize wait ratio"),
        );
        map
    }

    fn column_staging_samples_value(samples: &[ColumnStagingSample]) -> Value {
        let rows: Vec<_> = samples
            .iter()
            .map(|sample| {
                let mut entry = json::Map::new();
                entry.insert(
                    "batch".into(),
                    json::to_value(&sample.batch).expect("serialize sample batch"),
                );
                entry.insert(
                    "flatten_ms".into(),
                    json::to_value(&round3(sample.flatten_ms)).expect("serialize sample flatten"),
                );
                entry.insert(
                    "wait_ms".into(),
                    json::to_value(&round3(sample.wait_ms)).expect("serialize sample wait"),
                );
                entry.insert(
                    "wait_ratio".into(),
                    json::to_value(&round3(sample.wait_ratio())).expect("serialize sample ratio"),
                );
                Value::Object(entry)
            })
            .collect();
        Value::Array(rows)
    }

    fn column_staging_value(stats: &ColumnStagingStats) -> Value {
        let total = stats.total();
        let mut map = column_staging_phase_value(&total);
        let mut phases = json::Map::new();
        phases.insert(
            "fft".into(),
            Value::Object(column_staging_phase_value(&stats.fft())),
        );
        phases.insert(
            "lde".into(),
            Value::Object(column_staging_phase_value(&stats.lde())),
        );
        phases.insert(
            "poseidon".into(),
            Value::Object(column_staging_phase_value(&stats.poseidon())),
        );
        map.insert("phases".into(), Value::Object(phases));
        let mut samples = json::Map::new();
        samples.insert(
            "fft".into(),
            column_staging_samples_value(stats.fft_samples()),
        );
        samples.insert(
            "lde".into(),
            column_staging_samples_value(stats.lde_samples()),
        );
        samples.insert(
            "poseidon".into(),
            column_staging_samples_value(stats.poseidon_samples()),
        );
        map.insert("samples".into(), Value::Object(samples));
        Value::Object(map)
    }

    pub(super) fn operation_value(
        name: &str,
        input_len: usize,
        columns: usize,
        cpu: &Summary,
        gpu: Option<&Summary>,
        zero_fill: Option<&ZeroFillSummary>,
    ) -> Value {
        let mut map = json::Map::new();
        map.insert(
            "operation".into(),
            json::to_value(name).expect("serialize operation name"),
        );
        map.insert(
            "input_len".into(),
            json::to_value(&input_len).expect("serialize input length"),
        );
        map.insert(
            "columns".into(),
            json::to_value(&columns).expect("serialize column count"),
        );
        map.insert("cpu".into(), summary_value(cpu));
        map.insert(
            "gpu_recorded".into(),
            json::to_value(&gpu.is_some()).expect("serialize gpu flag"),
        );
        if let Some(stats) = gpu {
            map.insert("gpu".into(), summary_value(stats));
            map.insert("speedup".into(), speedup_value(cpu, stats));
        }
        if let Some(stats) = zero_fill {
            map.insert("zero_fill".into(), zero_fill_value(stats));
        }
        Value::Object(map)
    }

    fn bn254_metric_name(operation: &str) -> Option<&'static str> {
        match operation {
            "fft" => Some("acceleration.bn254_fft_ms"),
            "ifft" => Some("acceleration.bn254_ifft_ms"),
            "lde" => Some("acceleration.bn254_lde_ms"),
            "poseidon_hash_columns" => Some("acceleration.bn254_poseidon_ms"),
            _ => None,
        }
    }

    fn summary_mean_ms(value: &Value) -> Option<f64> {
        value
            .as_object()
            .and_then(|map| map.get("mean_ms"))
            .and_then(Value::as_f64)
    }

    fn metric_entry(entry: &Value, backend_label: &str) -> Option<(String, Value)> {
        let map = entry.as_object()?;
        let operation = map.get("operation")?.as_str()?;
        let metric = bn254_metric_name(operation)?;
        let cpu_mean = map.get("cpu").and_then(summary_mean_ms)?;
        let mut metric_map = json::Map::new();
        metric_map.insert(
            "cpu".into(),
            json::to_value(&round3(cpu_mean)).expect("serialize bn254 cpu mean"),
        );
        if let Some(gpu_mean) = map
            .get("gpu")
            .and_then(summary_mean_ms)
            .filter(|_| backend_label != "cpu")
        {
            metric_map.insert(
                backend_label.into(),
                json::to_value(&round3(gpu_mean)).expect("serialize bn254 gpu mean"),
            );
        }
        Some((metric.to_owned(), Value::Object(metric_map)))
    }

    pub(super) fn bn254_metrics_value(operations: &[Value], backend_label: &str) -> Option<Value> {
        let mut entries = BTreeMap::new();
        for entry in operations {
            if let Some((name, value)) = metric_entry(entry, backend_label) {
                entries.insert(name, value);
            }
        }
        if entries.is_empty() {
            return None;
        }
        let mut map = json::Map::new();
        for (name, value) in entries {
            map.insert(name, value);
        }
        Some(Value::Object(map))
    }

    pub(super) fn twiddle_cache_value(stats: &TwiddleCacheStats) -> Value {
        let mut map = json::Map::new();
        map.insert(
            "hits".into(),
            json::to_value(&stats.hits).expect("serialize hits"),
        );
        map.insert(
            "misses".into(),
            json::to_value(&stats.misses).expect("serialize misses"),
        );
        map.insert(
            "before_ms".into(),
            json::to_value(&round3(stats.before_ms)).expect("serialize before ms"),
        );
        map.insert(
            "after_ms".into(),
            json::to_value(&round3(stats.after_ms)).expect("serialize after ms"),
        );
        let savings = (stats.before_ms - stats.after_ms).max(0.0);
        map.insert(
            "savings_ms".into(),
            json::to_value(&round3(savings)).expect("serialize savings"),
        );
        Value::Object(map)
    }

    fn fft_tuning_value(tuning: &FftTuning) -> Value {
        let mut map = json::Map::new();
        map.insert(
            "threadgroup_lanes".into(),
            json::to_value(&tuning.threadgroup_lanes).expect("serialize lanes"),
        );
        map.insert(
            "tile_stage_limit".into(),
            json::to_value(&tuning.tile_stage_limit).expect("serialize tile depth"),
        );
        Value::Object(map)
    }

    pub(super) fn queue_stats_value(stats: &QueueDepthStats) -> Value {
        let overlap_ratio = if stats.busy_ms <= f64::EPSILON {
            0.0
        } else {
            round3(stats.overlap_ms / stats.busy_ms)
        };
        let busy_ratio = if stats.window_ms <= f64::EPSILON {
            0.0
        } else {
            round3(stats.busy_ms / stats.window_ms)
        };
        let mut map = json::Map::new();
        map.insert(
            "limit".into(),
            json::to_value(&stats.limit).expect("serialize limit"),
        );
        map.insert(
            "dispatch_count".into(),
            json::to_value(&stats.dispatch_count).expect("serialize dispatch count"),
        );
        map.insert(
            "max_in_flight".into(),
            json::to_value(&stats.max_in_flight).expect("serialize peak depth"),
        );
        map.insert(
            "busy_ms".into(),
            json::to_value(&round3(stats.busy_ms)).expect("serialize busy time"),
        );
        map.insert(
            "overlap_ms".into(),
            json::to_value(&round3(stats.overlap_ms)).expect("serialize overlap time"),
        );
        map.insert(
            "overlap_ratio".into(),
            json::to_value(&overlap_ratio).expect("serialize overlap ratio"),
        );
        map.insert(
            "window_ms".into(),
            json::to_value(&round3(stats.window_ms)).expect("serialize window"),
        );
        map.insert(
            "busy_ratio".into(),
            json::to_value(&busy_ratio).expect("serialize busy ratio"),
        );
        if !stats.queues.is_empty() {
            let queues = stats
                .queues
                .iter()
                .map(|lane| {
                    let lane_ratio = if lane.busy_ms <= f64::EPSILON {
                        0.0
                    } else {
                        round3(lane.overlap_ms / lane.busy_ms)
                    };
                    let lane_busy_ratio = if stats.window_ms <= f64::EPSILON {
                        0.0
                    } else {
                        round3(lane.busy_ms / stats.window_ms)
                    };
                    let mut lane_map = json::Map::new();
                    lane_map.insert(
                        "index".into(),
                        json::to_value(&lane.index).expect("serialize queue index"),
                    );
                    lane_map.insert(
                        "dispatch_count".into(),
                        json::to_value(&lane.dispatch_count)
                            .expect("serialize lane dispatch count"),
                    );
                    lane_map.insert(
                        "max_in_flight".into(),
                        json::to_value(&lane.max_in_flight).expect("serialize lane peak depth"),
                    );
                    lane_map.insert(
                        "busy_ms".into(),
                        json::to_value(&round3(lane.busy_ms)).expect("serialize lane busy time"),
                    );
                    lane_map.insert(
                        "overlap_ms".into(),
                        json::to_value(&round3(lane.overlap_ms))
                            .expect("serialize lane overlap time"),
                    );
                    lane_map.insert(
                        "overlap_ratio".into(),
                        json::to_value(&lane_ratio).expect("serialize lane overlap ratio"),
                    );
                    lane_map.insert(
                        "busy_ratio".into(),
                        json::to_value(&lane_busy_ratio).expect("serialize lane busy ratio"),
                    );
                    Value::Object(lane_map)
                })
                .collect();
            map.insert("queues".into(), Value::Array(queues));
        }
        Value::Object(map)
    }

    pub(super) fn classify_run(
        gpu_available: bool,
        backend_label: &str,
        operations: &[Value],
        queue_stats: Option<&QueueDepthStats>,
        column_staging: Option<&ColumnStagingStats>,
    ) -> RunStatus {
        let mut reasons = Vec::new();
        let dispatch_count = queue_stats.map(|stats| stats.dispatch_count);
        if !gpu_available {
            reasons.push("gpu_unavailable".to_owned());
            return RunStatus {
                state: RunState::GpuUnavailable,
                reasons,
                dispatch_count,
                gpu_available,
                backend_label: backend_label.to_owned(),
            };
        }

        let has_gpu_timings = operations.iter().any(|entry| {
            entry
                .as_object()
                .and_then(|map| map.get("gpu"))
                .map_or(false, Value::is_object)
        });
        let mut state = if has_gpu_timings {
            RunState::Ok
        } else {
            reasons.push("gpu_timings_missing".to_owned());
            RunState::GpuFallback
        };

        match queue_stats {
            None => {
                reasons.push("missing_queue_telemetry".to_owned());
                if state == RunState::Ok {
                    state = RunState::TelemetryMissing;
                }
            }
            Some(stats) if stats.dispatch_count == 0 => {
                reasons.push("missing_queue_dispatches".to_owned());
                if state == RunState::Ok {
                    state = RunState::TelemetryMissing;
                }
            }
            _ => {}
        }
        if column_staging.is_none() {
            reasons.push("missing_column_staging".to_owned());
            if state == RunState::Ok {
                state = RunState::TelemetryMissing;
            }
        }

        RunStatus {
            state,
            reasons,
            dispatch_count,
            gpu_available,
            backend_label: backend_label.to_owned(),
        }
    }

    pub(super) fn heuristics_value(snapshot: &AdaptiveScheduleSnapshot) -> Option<Value> {
        let mut map = json::Map::new();
        if let Some(limit) = snapshot.max_in_flight.as_ref() {
            map.insert("max_in_flight".into(), command_limit_value(limit));
        }
        if let Some(multiplier) = snapshot.poseidon_batch_multiplier {
            map.insert(
                "poseidon_batch_multiplier".into(),
                json::to_value(&multiplier).expect("serialize poseidon multiplier"),
            );
        }
        if let Some(lde_limit) = snapshot.lde_tile_stage_limit {
            map.insert(
                "lde_tile_stage_limit".into(),
                json::to_value(&lde_limit).expect("serialize LDE tile stage limit"),
            );
        }
        let mut batches = json::Map::new();
        if let Some(fft) = snapshot.fft.as_ref() {
            batches.insert("fft".into(), batch_snapshot_value(fft));
        }
        if let Some(lde) = snapshot.lde.as_ref() {
            batches.insert("lde".into(), batch_snapshot_value(lde));
        }
        if let Some(poseidon) = snapshot.poseidon.as_ref() {
            batches.insert("poseidon".into(), batch_snapshot_value(poseidon));
        }
        if !batches.is_empty() {
            map.insert("batch_columns".into(), Value::Object(batches));
        }
        if map.is_empty() {
            None
        } else {
            Some(Value::Object(map))
        }
    }

    fn command_limit_value(snapshot: &CommandLimitSnapshot) -> Value {
        let mut map = json::Map::new();
        map.insert(
            "limit".into(),
            json::to_value(&snapshot.limit).expect("serialize limit"),
        );
        map.insert(
            "queue_floor".into(),
            json::to_value(&snapshot.queue_floor).expect("serialize queue floor"),
        );
        map.insert(
            "auto_limit".into(),
            json::to_value(&snapshot.auto_limit).expect("serialize auto limit"),
        );
        map.insert(
            "source".into(),
            json::to_value(snapshot.source.as_str()).expect("serialize source"),
        );
        if let Some(value) = snapshot.gpu_cores {
            map.insert(
                "gpu_cores".into(),
                json::to_value(&value).expect("serialize gpu cores"),
            );
        }
        if let Some(value) = snapshot.cpu_parallelism {
            map.insert(
                "cpu_parallelism".into(),
                json::to_value(&value).expect("serialize cpu parallelism"),
            );
        }
        if let Some(value) = snapshot.override_limit {
            map.insert(
                "override_limit".into(),
                json::to_value(&value).expect("serialize override"),
            );
        }
        Value::Object(map)
    }

    fn batch_snapshot_value(snapshot: &BatchHeuristicSnapshot) -> Value {
        let mut map = json::Map::new();
        map.insert(
            "columns".into(),
            json::to_value(&snapshot.columns).expect("serialize columns"),
        );
        map.insert(
            "recommended".into(),
            json::to_value(&snapshot.recommended).expect("serialize recommended"),
        );
        map.insert(
            "max_columns".into(),
            json::to_value(&snapshot.max_columns).expect("serialize max columns"),
        );
        map.insert(
            "target_ms".into(),
            json::to_value(&round3(snapshot.target_ms)).expect("serialize target"),
        );
        if let Some(value) = snapshot.last_duration_ms {
            map.insert(
                "last_duration_ms".into(),
                json::to_value(&round3(value)).expect("serialize last duration"),
            );
        }
        map.insert(
            "samples".into(),
            json::to_value(&snapshot.samples).expect("serialize samples"),
        );
        map.insert(
            "override_active".into(),
            json::to_value(&snapshot.override_active).expect("serialize override flag"),
        );
        Value::Object(map)
    }

    pub(super) fn kernel_stats_value(samples: &[KernelStatsSample]) -> Value {
        let mut by_kind: BTreeMap<&'static str, KindSummary> = BTreeMap::new();
        for sample in samples {
            by_kind
                .entry(sample.kind.as_str())
                .or_insert_with(KindSummary::default)
                .record(sample);
        }
        let mut kinds = json::Map::new();
        for (kind, summary) in by_kind {
            kinds.insert(kind.to_owned(), summary.into_value());
        }
        let mut map = json::Map::new();
        map.insert(
            "total_samples".into(),
            json::to_value(&(samples.len() as u64)).expect("serialize kernel sample count"),
        );
        map.insert("by_kind".into(), Value::Object(kinds));
        Value::Object(map)
    }

    pub(super) fn poseidon_profiles_value(samples: &[KernelStatsSample]) -> Option<Value> {
        let mut summary = KindSummary::default();
        let mut recorded = false;
        for sample in samples {
            if sample.kind == KernelKind::Poseidon {
                summary.record(sample);
                recorded = true;
            }
        }
        recorded.then(|| summary.into_value())
    }

    fn summarize_bn254_samples(samples: &[KernelStatsSample]) -> Option<Value> {
        if samples.is_empty() {
            return None;
        }
        let mut width_min = u64::MAX;
        let mut width_max = 0u64;
        let mut width_sum = 0f64;
        let mut logical_min = u64::MAX;
        let mut logical_max = 0u64;
        let mut execution_width = None;
        let mut max_threads_per_group = None;
        for sample in samples {
            width_min = width_min.min(sample.threadgroup_width);
            width_max = width_max.max(sample.threadgroup_width);
            width_sum += sample.threadgroup_width as f64;
            logical_min = logical_min.min(sample.logical_threads);
            logical_max = logical_max.max(sample.logical_threads);
            execution_width.get_or_insert(sample.execution_width);
            max_threads_per_group.get_or_insert(sample.max_threads_per_group);
        }
        let mean_width = round3(width_sum / samples.len() as f64);
        let mut map = json::Map::new();
        map.insert(
            "sample_count".into(),
            json::to_value(&(samples.len() as u64)).expect("serialize sample count"),
        );
        let mut widths = json::Map::new();
        widths.insert(
            "min".into(),
            json::to_value(&width_min).expect("serialize min width"),
        );
        widths.insert(
            "max".into(),
            json::to_value(&width_max).expect("serialize max width"),
        );
        widths.insert(
            "mean".into(),
            json::to_value(&mean_width).expect("serialize mean width"),
        );
        map.insert("threadgroup_width".into(), Value::Object(widths));
        let mut logical = json::Map::new();
        logical.insert(
            "min".into(),
            json::to_value(&logical_min).expect("serialize min logical threads"),
        );
        logical.insert(
            "max".into(),
            json::to_value(&logical_max).expect("serialize max logical threads"),
        );
        map.insert("logical_threads".into(), Value::Object(logical));
        if let Some(value) = execution_width {
            map.insert(
                "execution_width".into(),
                json::to_value(&value).expect("serialize execution width"),
            );
        }
        if let Some(value) = max_threads_per_group {
            map.insert(
                "max_threads_per_group".into(),
                json::to_value(&value).expect("serialize max threads"),
            );
        }
        Some(Value::Object(map))
    }

    pub(super) fn bn254_dispatch_summary(samples: &[KernelStatsSample]) -> Option<Value> {
        let mut fft = Vec::new();
        let mut lde = Vec::new();
        for sample in samples {
            let single_group = sample.threadgroups == 1;
            let single_column = sample.column_count == 1;
            let threads_match = sample.logical_threads == sample.elements;
            if !(single_group && single_column && threads_match) {
                continue;
            }
            match sample.kind {
                KernelKind::Fft => fft.push(*sample),
                KernelKind::Lde => lde.push(*sample),
                _ => {}
            }
        }
        let mut map = json::Map::new();
        if let Some(value) = summarize_bn254_samples(&fft) {
            map.insert("fft".into(), value);
        }
        if let Some(value) = summarize_bn254_samples(&lde) {
            map.insert("lde".into(), value);
        }
        if map.is_empty() {
            None
        } else {
            Some(Value::Object(map))
        }
    }

    pub(super) fn poseidon_pipeline_value(stats: &PoseidonPipelineStats) -> Value {
        let mut map = json::Map::new();
        map.insert("enabled".into(), Value::Bool(stats.enabled));
        map.insert(
            "chunk_columns".into(),
            json::to_value(&stats.chunk_columns).expect("serialize chunk columns"),
        );
        map.insert(
            "pipe_depth".into(),
            json::to_value(&stats.pipe_depth).expect("serialize pipe depth"),
        );
        map.insert(
            "batches".into(),
            json::to_value(&stats.batches).expect("serialize batch count"),
        );
        map.insert(
            "fallbacks".into(),
            json::to_value(&stats.fallbacks).expect("serialize fallback count"),
        );
        Value::Object(map)
    }

    pub(super) fn post_tile_value(samples: &[PostTileSample]) -> Value {
        let mut by_kind: BTreeMap<&'static str, PostTileKindSummary> = BTreeMap::new();
        for sample in samples {
            by_kind
                .entry(sample.kind.as_str())
                .or_default()
                .record(sample);
        }
        let mut entries = Vec::new();
        for (kind, summary) in by_kind {
            entries.push(summary.into_value(kind));
        }
        let mut map = json::Map::new();
        map.insert(
            "total_dispatches".into(),
            json::to_value(&(samples.len() as u64)).expect("serialize total post-tile dispatches"),
        );
        map.insert("kinds".into(), Value::Array(entries));
        Value::Object(map)
    }

    #[derive(Default)]
    struct KindSummary {
        samples: Vec<Value>,
        count: u64,
        bytes_total: u64,
        elements_total: u64,
        duration: RangeAgg,
        bandwidth: RangeAgg,
        occupancy: RangeAgg,
    }

    impl KindSummary {
        fn record(&mut self, sample: &KernelStatsSample) {
            self.count = self.count.saturating_add(1);
            self.bytes_total = self.bytes_total.saturating_add(sample.bytes);
            self.elements_total = self.elements_total.saturating_add(sample.elements);
            self.duration.record(sample.duration_ms);
            if let Some(bandwidth) = sample_bandwidth_gbps(sample) {
                self.bandwidth.record(bandwidth);
            }
            let occupancy = sample_occupancy(sample);
            self.occupancy.record(occupancy);

            let mut entry = json::Map::new();
            entry.insert(
                "columns".into(),
                json::to_value(&sample.column_count).expect("serialize columns"),
            );
            entry.insert(
                "bytes".into(),
                json::to_value(&sample.bytes).expect("serialize bytes"),
            );
            entry.insert(
                "elements".into(),
                json::to_value(&sample.elements).expect("serialize elements"),
            );
            entry.insert(
                "duration_ms".into(),
                json::to_value(&round3(sample.duration_ms)).expect("serialize duration"),
            );
            entry.insert(
                "logical_threads".into(),
                json::to_value(&sample.logical_threads).expect("serialize logical threads"),
            );
            entry.insert(
                "threadgroup_width".into(),
                json::to_value(&sample.threadgroup_width).expect("serialize threadgroup width"),
            );
            entry.insert(
                "threadgroups".into(),
                json::to_value(&sample.threadgroups).expect("serialize threadgroups"),
            );
            entry.insert(
                "execution_width".into(),
                json::to_value(&sample.execution_width).expect("serialize execution width"),
            );
            entry.insert(
                "max_threads_per_group".into(),
                json::to_value(&sample.max_threads_per_group)
                    .expect("serialize max threads per group"),
            );
            entry.insert(
                "bandwidth_gbps".into(),
                sample_bandwidth_gbps(sample)
                    .map(round3)
                    .map(|value| json::to_value(&value).expect("serialize bandwidth"))
                    .unwrap_or(Value::Null),
            );
            entry.insert(
                "occupancy_ratio".into(),
                json::to_value(&round3(occupancy)).expect("serialize occupancy"),
            );
            self.samples.push(Value::Object(entry));
        }

        fn into_value(self) -> Value {
            let mut map = json::Map::new();
            map.insert(
                "sample_count".into(),
                json::to_value(&self.count).expect("serialize sample count"),
            );
            map.insert(
                "bytes_total".into(),
                json::to_value(&self.bytes_total).expect("serialize total bytes"),
            );
            map.insert(
                "elements_total".into(),
                json::to_value(&self.elements_total).expect("serialize total elements"),
            );
            map.insert("duration_ms".into(), self.duration.into_value());
            if self.bandwidth.count > 0 {
                map.insert("bandwidth_gbps".into(), self.bandwidth.into_value());
            } else {
                map.insert("bandwidth_gbps".into(), Value::Null);
            }
            map.insert("occupancy_ratio".into(), self.occupancy.into_value());
            map.insert("samples".into(), Value::Array(self.samples));
            Value::Object(map)
        }
    }

    #[derive(Default)]
    struct PostTileKindSummary {
        dispatch_count: u64,
        stage_start: IntRangeAgg,
        log_len: IntRangeAgg,
        columns: IntRangeAgg,
    }

    impl PostTileKindSummary {
        fn record(&mut self, sample: &PostTileSample) {
            self.dispatch_count = self.dispatch_count.saturating_add(1);
            self.stage_start.record(sample.stage_start.into());
            self.log_len.record(sample.log_len.into());
            self.columns.record(sample.columns.into());
        }

        fn into_value(self, kind: &str) -> Value {
            let mut map = json::Map::new();
            map.insert("kind".into(), Value::String(kind.to_owned()));
            map.insert(
                "dispatch_count".into(),
                json::to_value(&self.dispatch_count).expect("serialize post-tile dispatch count"),
            );
            map.insert("stage_start_log2".into(), self.stage_start.into_value());
            map.insert("lde_log2".into(), self.log_len.into_value());
            map.insert("columns".into(), self.columns.into_value());
            Value::Object(map)
        }
    }

    #[derive(Clone, Copy, Default)]
    struct IntRangeAgg {
        min: u64,
        max: u64,
        sum: u128,
        count: u64,
    }

    impl IntRangeAgg {
        fn record(&mut self, value: u64) {
            if self.count == 0 {
                self.min = value;
                self.max = value;
            } else {
                self.min = self.min.min(value);
                self.max = self.max.max(value);
            }
            self.sum = self.sum.saturating_add(u128::from(value));
            self.count = self.count.saturating_add(1);
        }

        fn into_value(self) -> Value {
            if self.count == 0 {
                return Value::Null;
            }
            let mean = (self.sum as f64) / (self.count as f64);
            let mut map = json::Map::new();
            map.insert(
                "mean".into(),
                json::to_value(&round3(mean)).expect("serialize integer mean"),
            );
            map.insert(
                "min".into(),
                json::to_value(&self.min).expect("serialize integer min"),
            );
            map.insert(
                "max".into(),
                json::to_value(&self.max).expect("serialize integer max"),
            );
            Value::Object(map)
        }
    }

    #[derive(Clone, Copy, Default)]
    struct RangeAgg {
        min: f64,
        max: f64,
        sum: f64,
        count: u64,
    }

    impl RangeAgg {
        fn record(&mut self, value: f64) {
            if self.count == 0 {
                self.min = value;
                self.max = value;
            } else {
                self.min = self.min.min(value);
                self.max = self.max.max(value);
            }
            self.sum += value;
            self.count = self.count.saturating_add(1);
        }

        fn into_value(self) -> Value {
            if self.count == 0 {
                Value::Null
            } else {
                let mean = self.sum / (self.count as f64);
                let mut map = json::Map::new();
                map.insert(
                    "mean".into(),
                    json::to_value(&round3(mean)).expect("serialize mean"),
                );
                map.insert(
                    "min".into(),
                    json::to_value(&round3(self.min)).expect("serialize min"),
                );
                map.insert(
                    "max".into(),
                    json::to_value(&round3(self.max)).expect("serialize max"),
                );
                Value::Object(map)
            }
        }
    }

    fn sample_bandwidth_gbps(sample: &KernelStatsSample) -> Option<f64> {
        if sample.duration_ms <= f64::EPSILON {
            return None;
        }
        let seconds = sample.duration_ms / 1_000.0;
        if seconds <= f64::EPSILON {
            return None;
        }
        let gbps = (sample.bytes as f64) / seconds / 1_000_000_000.0;
        Some(gbps)
    }

    fn sample_occupancy(sample: &KernelStatsSample) -> f64 {
        if sample.max_threads_per_group == 0 {
            return 0.0;
        }
        let ratio = (sample.threadgroup_width as f64) / (sample.max_threads_per_group as f64);
        ratio.clamp(0.0, 1.0)
    }

    struct ColumnSets {
        time: Vec<Vec<u64>>,
        coeff: Vec<Vec<u64>>,
        freq: Vec<Vec<u64>>,
        domains: Vec<String>,
    }

    impl ColumnSets {
        fn poseidon_elements(&self, column_count: usize) -> usize {
            self.coeff.first().map_or(0, Vec::len) * column_count
        }
    }

    struct OperationTimings {
        cpu: Summary,
        gpu: Option<Summary>,
    }

    fn ensure_trace_environment(config: &Config) -> Result<(), String> {
        let Some(trace) = config.trace.as_ref() else {
            return Ok(());
        };
        if env::var_os(TRACE_REEXEC_ENV).is_some() {
            return Ok(());
        }
        launch_trace(trace)
    }

    fn launch_trace(trace: &TraceOptions) -> Result<(), String> {
        if let Some(parent) = trace.output.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                fs::create_dir_all(parent).map_err(|err| {
                    format!(
                        "failed to create trace directory {}: {err}",
                        parent.display()
                    )
                })?;
            }
        }
        let exe = env::current_exe()
            .map_err(|err| format!("failed to locate benchmark executable: {err}"))?;
        let args: Vec<String> = env::args().skip(1).collect();
        eprintln!(
            "fastpq_metal_bench: capturing Metal trace '{}' -> {}",
            trace.template,
            trace.output.display()
        );
        let status = Command::new("xcrun")
            .arg("xctrace")
            .arg("record")
            .arg("--template")
            .arg(&trace.template)
            .arg("--time-limit")
            .arg(format!("{}s", trace.seconds))
            .arg("--output")
            .arg(&trace.output)
            .arg("--launch")
            .arg("--")
            .arg(&exe)
            .args(&args)
            .env(TRACE_REEXEC_ENV, "1")
            .status()
            .map_err(|err| format!("failed to invoke xctrace: {err}"))?;
        if let Some(code) = status.code() {
            std::process::exit(code);
        }
        Err("xctrace terminated by signal".into())
    }

    pub fn run() -> Result<(), String> {
        if let Some(mode) = poseidon_micro_mode_from_env() {
            run_poseidon_micro_child(mode)?;
            return Ok(());
        }
        let config = Config::parse()?;
        ensure_trace_environment(&config)?;
        let params = find_by_name("fastpq-lane-balanced")
            .ok_or_else(|| "canonical parameter set 'fastpq-lane-balanced' missing".to_owned())?;
        let planner = Planner::new(params);

        let padded = config
            .rows
            .checked_next_power_of_two()
            .ok_or_else(|| format!("rows {rows} exceed supported range", rows = config.rows))?;
        let trace_log = padded.trailing_zeros();
        let blowup_log = planner.blowup_log();
        let column_count = 16usize;
        let eval_len = padded << blowup_log;

        let columns = prepare_columns(&planner, padded, column_count);
        let (resolved_mode, backend_label, gpu_available) =
            resolve_execution_metadata(config.gpu_probe);
        enforce_gpu_requirement(
            config.require_gpu,
            fastpq_gpu_forced_via_env(),
            gpu_available,
            resolved_mode,
            backend_label.as_str(),
        )?;
        let fft_tuning = fft_tuning_snapshot(trace_log).ok();
        let stats_enabled = gpu_available;
        if stats_enabled {
            enable_queue_depth_stats(true);
            enable_kernel_stats(true);
            enable_post_tile_stats(true);
            enable_twiddle_cache_stats(true);
            enable_poseidon_pipeline_stats(true);
        } else {
            enable_poseidon_pipeline_stats(false);
        }
        let (mut operations, zero_fill_summary, poseidon_queue) = collect_operations(
            &planner,
            &config,
            padded,
            eval_len,
            column_count,
            &columns,
            gpu_available,
        );
        let (mut queue_stats, column_staging) = if stats_enabled {
            let queue_snapshot = take_queue_depth_stats();
            let staging_snapshot = take_column_staging_stats();
            enable_queue_depth_stats(false);
            (queue_snapshot, staging_snapshot)
        } else {
            (None, None)
        };
        let poseidon_pipeline = if stats_enabled {
            let snapshot = take_poseidon_pipeline_stats();
            enable_poseidon_pipeline_stats(false);
            snapshot
        } else {
            None
        };
        let zero_fill_missing = gpu_available && zero_fill_summary.is_none();
        let queue_stats_missing = gpu_available
            && queue_stats
                .as_ref()
                .map_or(true, |stats| stats.dispatch_count == 0);
        if gpu_available && (zero_fill_missing || queue_stats_missing) {
            let (probe_zero_fill, probe_queue) =
                capture_gpu_telemetry_probe(&planner, &columns, queue_stats_missing);
            if zero_fill_missing {
                if let Some(summary) = probe_zero_fill {
                    if !inject_zero_fill(&mut operations, &summary) {
                        eprintln!(
                            "fastpq_metal_bench: failed to record zero-fill telemetry after probe"
                        );
                    }
                } else {
                    let synthesized =
                        synthesize_zero_fill(column_count, eval_len, config.iterations);
                    if inject_zero_fill(&mut operations, &synthesized) {
                        eprintln!(
                            "fastpq_metal_bench: GPU zero-fill telemetry unavailable; synthesized host zero-fill timings instead"
                        );
                    } else {
                        eprintln!(
                            "fastpq_metal_bench: GPU zero-fill telemetry unavailable after probe"
                        );
                    }
                }
            }
            if queue_stats_missing {
                if let Some(stats) = probe_queue {
                    if stats.dispatch_count > 0 {
                        queue_stats = Some(stats);
                    }
                } else {
                    eprintln!(
                        "fastpq_metal_bench: GPU queue-depth telemetry unavailable after probe"
                    );
                }
            }
        }
        let post_tile_stats = if stats_enabled {
            let samples = take_post_tile_stats();
            enable_post_tile_stats(false);
            samples
        } else {
            None
        };
        let kernel_stats = if stats_enabled {
            let samples = take_kernel_stats();
            enable_kernel_stats(false);
            samples
        } else {
            None
        };
        let twiddle_stats = if stats_enabled {
            let stats = take_twiddle_cache_stats();
            enable_twiddle_cache_stats(false);
            stats
        } else {
            None
        };
        let adaptive_heuristics = adaptive_schedule_snapshot();
        let report_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock error: {err}"))?
            .as_secs();
        let poseidon_micro = capture_poseidon_microbench(gpu_available, backend_label.as_str());
        let device_profile = if gpu_available {
            capture_device_profile()
        } else {
            None
        };
        let run_status = classify_run(
            gpu_available,
            backend_label.as_str(),
            &operations,
            queue_stats.as_ref(),
            column_staging.as_ref(),
        );
        if config.require_telemetry && !run_status.is_ok() {
            let joined = if run_status.reasons.is_empty() {
                "missing GPU telemetry".to_owned()
            } else {
                run_status.reasons.join(", ")
            };
            return Err(format!(
                "telemetry guard triggered (--require-telemetry): {joined}"
            ));
        }

        let report_inputs = ReportInputs {
            parameter_name: params.name,
            config: &config,
            padded,
            trace_log,
            blowup_log,
            column_count,
            resolved_mode,
            backend_label: backend_label.as_str(),
            gpu_available,
            timestamp: report_timestamp,
            run_status,
            fft_tuning,
            queue_stats,
            poseidon_queue,
            column_staging,
            poseidon_pipeline,
            twiddle_stats,
            kernel_stats,
            adaptive_heuristics,
            post_tile_stats,
            poseidon_micro,
            device_profile,
        };
        let payload = build_report(&report_inputs, operations);
        write_report(&payload, config.output.as_deref())
    }

    pub(crate) fn parse_poseidon_micro_mode(raw: &str) -> Option<PoseidonMicroMode> {
        match raw {
            "default" => Some(PoseidonMicroMode::Default),
            "scalar" => Some(PoseidonMicroMode::Scalar),
            _ => None,
        }
    }

    pub(crate) fn poseidon_micro_mode_from_env() -> Option<PoseidonMicroMode> {
        env::var(POSEIDON_MICRO_ENV)
            .ok()
            .and_then(|value| parse_poseidon_micro_mode(value.as_str()))
    }

    fn run_poseidon_micro_child(mode: PoseidonMicroMode) -> Result<(), String> {
        if !matches!(ExecutionMode::Auto.resolve(), ExecutionMode::Gpu) {
            return Err("Poseidon microbench requires FASTPQ_GPU=gpu".into());
        }
        let trace_log = POSEIDON_MICRO_TRACE_LOG;
        let len = 1usize << trace_log;
        let column_count = POSEIDON_MICRO_COLUMNS;
        let columns = generate_poseidon_micro_columns(len, column_count);
        let domains = build_domains(column_count);
        for _ in 0..POSEIDON_MICRO_WARMUPS {
            match hash_columns_gpu(&domains, &columns) {
                PoseidonGpuOutcome::Gpu(values) => drop(black_box(values)),
                PoseidonGpuOutcome::CpuFallback(values) => {
                    drop(black_box(values));
                    return Err(
                        "Poseidon microbench requires GPU hashing but execution fell back to CPU"
                            .into(),
                    );
                }
            }
        }
        let mut samples = Vec::with_capacity(POSEIDON_MICRO_ITERATIONS);
        for _ in 0..POSEIDON_MICRO_ITERATIONS {
            let started = Instant::now();
            match hash_columns_gpu(&domains, &columns) {
                PoseidonGpuOutcome::Gpu(values) => {
                    drop(black_box(values));
                    samples.push(elapsed_ms(started.elapsed()));
                }
                PoseidonGpuOutcome::CpuFallback(values) => {
                    drop(black_box(values));
                    return Err(
                        "Poseidon microbench requires GPU hashing but execution fell back to CPU"
                            .into(),
                    );
                }
            }
        }
        let summary = Summary::from_samples(&samples);
        let tuning = poseidon_tuning_snapshot().ok();
        let mut payload = json::Map::new();
        payload.insert(
            "mode".into(),
            json::to_value(mode.as_str()).expect("serialize poseidon micro mode"),
        );
        payload.insert(
            "mean_ms".into(),
            json::to_value(&round3(summary.mean_ms())).expect("serialize mean"),
        );
        payload.insert(
            "min_ms".into(),
            json::to_value(&round3(summary.min_ms())).expect("serialize min"),
        );
        payload.insert(
            "max_ms".into(),
            json::to_value(&round3(summary.max_ms())).expect("serialize max"),
        );
        payload.insert(
            "columns".into(),
            json::to_value(&column_count).expect("serialize columns"),
        );
        payload.insert(
            "trace_log2".into(),
            json::to_value(&trace_log).expect("serialize trace log"),
        );
        payload.insert(
            "states".into(),
            json::to_value(&(column_count * len)).expect("serialize states"),
        );
        payload.insert(
            "warmups".into(),
            json::to_value(&POSEIDON_MICRO_WARMUPS).expect("serialize warmups"),
        );
        payload.insert(
            "iterations".into(),
            json::to_value(&POSEIDON_MICRO_ITERATIONS).expect("serialize iterations"),
        );
        if let Some(profile) = tuning {
            let mut tuning_map = json::Map::new();
            tuning_map.insert(
                "threadgroup_lanes".into(),
                json::to_value(&profile.threadgroup_lanes).expect("serialize lanes"),
            );
            tuning_map.insert(
                "states_per_lane".into(),
                json::to_value(&profile.states_per_lane).expect("serialize states per lane"),
            );
            payload.insert("tuning".into(), Value::Object(tuning_map));
        }
        let value = Value::Object(payload);
        let text = json::to_string(&value)
            .map_err(|err| format!("failed to encode poseidon microbench payload: {err}"))?;
        println!("{text}");
        Ok(())
    }

    fn generate_poseidon_micro_columns(len: usize, column_count: usize) -> Vec<Vec<u64>> {
        generated_columns(len, column_count, 0x7d6e_12ba)
    }

    fn capture_poseidon_microbench(gpu_available: bool, backend_label: &str) -> Option<Value> {
        if !(gpu_available && backend_label == "metal") {
            return None;
        }
        let default_sample = match invoke_poseidon_micro_child(PoseidonMicroMode::Default) {
            Ok(value) => value,
            Err(err) => {
                eprintln!(
                    "fastpq_metal_bench: failed to capture Poseidon default microbench: {err}"
                );
                return None;
            }
        };
        let scalar_sample = match invoke_poseidon_micro_child(PoseidonMicroMode::Scalar) {
            Ok(value) => value,
            Err(err) => {
                eprintln!(
                    "fastpq_metal_bench: failed to capture Poseidon scalar microbench: {err}"
                );
                return None;
            }
        };
        build_poseidon_micro_report(default_sample, scalar_sample)
    }

    fn invoke_poseidon_micro_child(mode: PoseidonMicroMode) -> Result<Value, String> {
        let exe = env::current_exe()
            .map_err(|err| format!("failed to locate benchmark executable: {err}"))?;
        let mut command = Command::new(exe);
        command.env(POSEIDON_MICRO_ENV, mode.as_str());
        command.env("FASTPQ_GPU", "gpu");
        command.env_remove("FASTPQ_METAL_POSEIDON_LANES");
        command.env_remove("FASTPQ_METAL_POSEIDON_BATCH");
        if mode == PoseidonMicroMode::Scalar {
            command.env("FASTPQ_METAL_POSEIDON_LANES", POSEIDON_MICRO_SCALAR_LANES);
            command.env("FASTPQ_METAL_POSEIDON_BATCH", POSEIDON_MICRO_SCALAR_BATCH);
        }
        command.stdout(Stdio::piped());
        let output = command
            .output()
            .map_err(|err| format!("failed to launch Poseidon microbench: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "Poseidon microbench exited with status {:?}",
                output.status.code()
            ));
        }
        json::from_slice(&output.stdout)
            .map_err(|err| format!("failed to parse Poseidon microbench output: {err}"))
    }

    fn build_poseidon_micro_report(default_sample: Value, scalar_sample: Value) -> Option<Value> {
        let default_mean = micro_sample_mean(&default_sample)?;
        let scalar_mean = micro_sample_mean(&scalar_sample)?;
        let mut map = json::Map::new();
        map.insert("default".into(), default_sample);
        map.insert("scalar_lane".into(), scalar_sample);
        if default_mean > 0.0 {
            map.insert(
                "speedup_vs_scalar".into(),
                json::to_value(&round3(scalar_mean / default_mean)).ok()?,
            );
        }
        Some(Value::Object(map))
    }

    fn micro_sample_mean(sample: &Value) -> Option<f64> {
        sample
            .as_object()
            .and_then(|map| map.get("mean_ms"))
            .and_then(Value::as_f64)
    }

    #[cfg(test)]
    mod zero_fill_tests {
        use super::{LdeHostStats, QueueDepthStats, ZeroFillAccumulator};

        #[test]
        fn accumulator_merges_queue_deltas() {
            let mut accumulator = ZeroFillAccumulator::default();
            accumulator.record(
                LdeHostStats {
                    zero_fill_bytes: 64,
                    zero_fill_ms: 1.0,
                    queue_delta: None,
                },
                Some(QueueDepthStats {
                    limit: 4,
                    dispatch_count: 1,
                    max_in_flight: 1,
                    busy_ms: 0.25,
                    overlap_ms: 0.0,
                    window_ms: 0.25,
                    queues: Vec::new(),
                }),
            );
            accumulator.record(
                LdeHostStats {
                    zero_fill_bytes: 64,
                    zero_fill_ms: 2.0,
                    queue_delta: None,
                },
                Some(QueueDepthStats {
                    limit: 4,
                    dispatch_count: 2,
                    max_in_flight: 2,
                    busy_ms: 0.5,
                    overlap_ms: 0.25,
                    window_ms: 0.5,
                    queues: Vec::new(),
                }),
            );
            let summary = accumulator.finish().expect("summary available");
            assert_eq!(summary.bytes, 64);
            assert!((summary.ms.mean_ms() - 1.5).abs() < f64::EPSILON);
            let delta = summary.queue_delta.expect("queue delta captured");
            assert_eq!(delta.dispatch_count, 3);
            assert_eq!(delta.max_in_flight, 2);
            assert!((delta.busy_ms - 0.75).abs() < f64::EPSILON);
            assert!((delta.overlap_ms - 0.25).abs() < f64::EPSILON);
            assert_eq!(delta.limit, 4);
        }
    }

    #[cfg(test)]
    mod speedup_tests {
        use fastpq_prover::{KernelKind, KernelStatsSample};

        use super::{Speedup, Summary, poseidon_profiles_value, round3, speedup_value};

        #[test]
        fn speedup_between_reports_ratio_and_delta() {
            let cpu = Summary::from_samples(&[6.0, 6.0, 6.0]);
            let gpu = Summary::from_samples(&[2.0, 2.0, 2.0]);
            let comparison = Speedup::between(&cpu, &gpu);
            assert_eq!(comparison.ratio, round3(3.0));
            assert_eq!(comparison.delta_ms, round3(4.0));
        }

        #[test]
        fn speedup_value_serializes_expected_fields() {
            let cpu = Summary::from_samples(&[4.0, 4.0]);
            let gpu = Summary::from_samples(&[2.0, 2.0]);
            let value = speedup_value(&cpu, &gpu);
            let object = value
                .as_object()
                .expect("speedup value serializes as JSON object");
            assert!(object.contains_key("ratio"));
            assert!(object.contains_key("delta_ms"));
        }

        #[test]
        fn poseidon_profiles_only_includes_poseidon_samples() {
            let samples = [
                KernelStatsSample {
                    kind: KernelKind::Poseidon,
                    bytes: 64,
                    elements: 96,
                    column_count: 4,
                    logical_threads: 128,
                    threadgroup_width: 32,
                    threadgroups: 4,
                    execution_width: 32,
                    max_threads_per_group: 256,
                    duration_ms: 0.75,
                },
                KernelStatsSample {
                    kind: KernelKind::Fft,
                    bytes: 128,
                    elements: 256,
                    column_count: 8,
                    logical_threads: 256,
                    threadgroup_width: 64,
                    threadgroups: 8,
                    execution_width: 64,
                    max_threads_per_group: 512,
                    duration_ms: 1.25,
                },
            ];
            let value = poseidon_profiles_value(&samples).expect("poseidon profile");
            let map = value
                .as_object()
                .expect("poseidon profile serializes to object");
            assert_eq!(
                map.get("sample_count")
                    .and_then(|v| v.as_u64())
                    .expect("sample count present"),
                1
            );
            assert!(map.get("samples").is_some(), "poseidon samples missing");
        }
    }

    #[test]
    fn inject_zero_fill_updates_lde_entry() {
        let summary = Summary::from_samples(&[5.0, 6.0]);
        let zero_fill = ZeroFillSummary {
            bytes: 1024,
            ms: summary.clone(),
            queue_delta: None,
        };
        let cpu = Summary::from_samples(&[1.0, 1.5]);
        let gpu = Summary::from_samples(&[0.5, 0.7]);
        let mut operations = vec![self::operation_value("lde", 8, 2, &cpu, Some(&gpu), None)];
        assert!(self::inject_zero_fill(&mut operations, &zero_fill));
        let map = operations[0]
            .as_object()
            .expect("lde entry serializes as map");
        assert!(
            map.contains_key("zero_fill"),
            "zero_fill missing after injection"
        );
    }

    #[test]
    fn inject_zero_fill_noop_without_lde_entry() {
        let summary = Summary::from_samples(&[5.0, 6.0]);
        let zero_fill = ZeroFillSummary {
            bytes: 512,
            ms: summary,
            queue_delta: None,
        };
        let cpu = Summary::from_samples(&[1.0, 1.1]);
        let gpu = Summary::from_samples(&[0.9, 1.0]);
        let mut operations = vec![self::operation_value("fft", 4, 2, &cpu, Some(&gpu), None)];
        assert!(!self::inject_zero_fill(&mut operations, &zero_fill));
    }

    #[test]
    fn bn254_metrics_capture_cpu_and_gpu_latency() {
        let cpu_fft = Summary::from_samples(&[10.0, 14.0]);
        let gpu_fft = Summary::from_samples(&[5.0, 6.0]);
        let cpu_lde = Summary::from_samples(&[20.0, 20.0]);
        let operations = vec![
            self::operation_value("fft", 8, 2, &cpu_fft, Some(&gpu_fft), None),
            self::operation_value("lde", 16, 2, &cpu_lde, None, None),
        ];
        let metrics = bn254_metrics_value(&operations, "metal").expect("metrics present");
        let map = metrics.as_object().expect("metrics serialize to map");
        let fft = map
            .get("acceleration.bn254_fft_ms")
            .and_then(Value::as_object)
            .expect("fft metric present");
        assert_eq!(fft.get("cpu").and_then(Value::as_f64), Some(12.0));
        assert_eq!(fft.get("metal").and_then(Value::as_f64), Some(5.5));
        let lde = map
            .get("acceleration.bn254_lde_ms")
            .and_then(Value::as_object)
            .expect("lde metric present");
        assert_eq!(lde.get("cpu").and_then(Value::as_f64), Some(20.0));
        assert!(!lde.contains_key("metal"));
    }

    #[test]
    fn enforce_gpu_requirement_errors_when_requested() {
        let error =
            self::enforce_gpu_requirement(true, false, false, ExecutionMode::Cpu, "none")
                .expect_err("missing GPU should error when required");
        assert!(error.contains("--require-gpu"));
        assert!(error.contains("resolved mode=cpu"));

        self::enforce_gpu_requirement(false, false, false, ExecutionMode::Cpu, "none")
            .expect("cpu fallback allowed when GPU not required");
    }

    #[test]
    fn classify_run_marks_missing_queue() {
        let cpu = Summary::from_samples(&[1.0]);
        let gpu = Summary::from_samples(&[0.5]);
        let operations = vec![self::operation_value(
            "fft",
            8,
            2,
            &cpu,
            Some(&gpu),
            None,
        )];
        let status = classify_run(
            true,
            "metal",
            &operations,
            None,
            Some(&ColumnStagingStats::default()),
        );
        assert_eq!(status.state, RunState::TelemetryMissing);
        assert!(
            status
                .reasons
                .iter()
                .any(|reason| reason.contains("missing_queue_telemetry"))
        );
    }

    #[test]
    fn classify_run_detects_gpu_fallback() {
        let cpu = Summary::from_samples(&[1.0, 1.1]);
        let operations = vec![self::operation_value("fft", 8, 2, &cpu, None, None)];
        let queue = QueueDepthStats {
            limit: 2,
            dispatch_count: 1,
            max_in_flight: 1,
            busy_ms: 1.0,
            overlap_ms: 0.5,
            window_ms: 2.0,
            queues: Vec::new(),
        };
        let status = classify_run(
            true,
            "metal",
            &operations,
            Some(&queue),
            Some(&ColumnStagingStats::default()),
        );
        assert_eq!(status.state, RunState::GpuFallback);
        assert!(
            status
                .reasons
                .iter()
                .any(|reason| reason.contains("gpu_timings_missing"))
        );
    }

    #[test]
    fn classify_run_accepts_complete_gpu_capture() {
        let cpu = Summary::from_samples(&[1.0, 1.2]);
        let gpu = Summary::from_samples(&[0.4, 0.5]);
        let operations = vec![self::operation_value(
            "fft",
            8,
            2,
            &cpu,
            Some(&gpu),
            None,
        )];
        let queue = QueueDepthStats {
            limit: 4,
            dispatch_count: 3,
            max_in_flight: 2,
            busy_ms: 1.5,
            overlap_ms: 0.25,
            window_ms: 2.5,
            queues: Vec::new(),
        };
        let status = classify_run(
            true,
            "metal",
            &operations,
            Some(&queue),
            Some(&ColumnStagingStats::default()),
        );
        assert_eq!(status.state, RunState::Ok);
        assert!(status.reasons.is_empty());
        assert_eq!(status.dispatch_count, Some(3));
    }

    fn prepare_columns(planner: &Planner, padded: usize, column_count: usize) -> ColumnSets {
        let time_columns = generated_columns(padded, column_count, 0x51a2_d3f4);
        let mut coeff_columns = time_columns.clone();
        let mut freq_columns = time_columns.clone();
        planner.ifft_columns(&mut coeff_columns);
        planner.fft_columns(&mut freq_columns);
        let domains = build_domains(column_count);
        ColumnSets {
            time: time_columns,
            coeff: coeff_columns,
            freq: freq_columns,
            domains,
        }
    }

    fn collect_operations(
        planner: &Planner,
        config: &Config,
        padded: usize,
        eval_len: usize,
        column_count: usize,
        columns: &ColumnSets,
        gpu_available: bool,
    ) -> (Vec<Value>, Option<ZeroFillSummary>, Option<QueueDepthStats>) {
        let mut operations = Vec::new();
        let mut zero_fill_result = None;
        let mut poseidon_queue = None;

        if config.operation.includes(BenchOperation::Fft) {
            let timings = OperationTimings {
                cpu: measure_in_place(&columns.time, config.warmups, config.iterations, |cols| {
                    planner.fft_columns(cols)
                }),
                gpu: gpu_available.then(|| {
                    measure_in_place(&columns.time, config.warmups, config.iterations, |cols| {
                        planner.fft_gpu(cols)
                    })
                }),
            };
            operations.push(operation_value(
                BenchOperation::Fft.as_str(),
                padded,
                column_count,
                &timings.cpu,
                timings.gpu.as_ref(),
                None,
            ));
        }

        if config.operation.includes(BenchOperation::Ifft) {
            let timings = OperationTimings {
                cpu: measure_in_place(&columns.freq, config.warmups, config.iterations, |cols| {
                    planner.ifft_columns(cols)
                }),
                gpu: gpu_available.then(|| {
                    measure_in_place(&columns.freq, config.warmups, config.iterations, |cols| {
                        planner.ifft_gpu(cols)
                    })
                }),
            };
            operations.push(operation_value(
                BenchOperation::Ifft.as_str(),
                padded,
                column_count,
                &timings.cpu,
                timings.gpu.as_ref(),
                None,
            ));
        }

        if config.operation.includes(BenchOperation::Lde) {
            let cpu_summary = measure_map(
                &columns.coeff,
                config.warmups,
                config.iterations,
                |coeffs| planner.lde_columns(coeffs),
            );
            let (gpu_summary, zero_fill_metrics) = if gpu_available {
                let (summary, zero) =
                    measure_gpu_lde(planner, &columns.coeff, config.warmups, config.iterations);
                (Some(summary), zero)
            } else {
                (None, None)
            };
            zero_fill_result = zero_fill_metrics.clone();
            operations.push(operation_value(
                BenchOperation::Lde.as_str(),
                eval_len,
                column_count,
                &cpu_summary,
                gpu_summary.as_ref(),
                zero_fill_metrics.as_ref(),
            ));
        }

        if config.operation.includes(BenchOperation::Poseidon) {
            let cpu_summary = measure_map(
                &columns.coeff,
                config.warmups,
                config.iterations,
                |coeffs| hash_columns_cpu(&columns.domains, coeffs),
            );
            let (gpu_summary, queue_delta) = if gpu_available {
                measure_poseidon_gpu(
                    &columns.domains,
                    &columns.coeff,
                    config.warmups,
                    config.iterations,
                )
            } else {
                (None, None)
            };
            poseidon_queue = queue_delta;
            operations.push(operation_value(
                BenchOperation::Poseidon.as_str(),
                columns.poseidon_elements(column_count),
                column_count,
                &cpu_summary,
                gpu_summary.as_ref(),
                None,
            ));
        }

        (operations, zero_fill_result, poseidon_queue)
    }

    fn capture_gpu_telemetry_probe(
        planner: &Planner,
        columns: &ColumnSets,
        collect_queue_stats: bool,
    ) -> (Option<ZeroFillSummary>, Option<QueueDepthStats>) {
        if collect_queue_stats {
            enable_queue_depth_stats(true);
        }
        let (_, zero_fill) = measure_gpu_lde(planner, &columns.coeff, 0, 1);
        let queue_stats = if collect_queue_stats {
            let snapshot = take_queue_depth_stats();
            let _ = take_column_staging_stats();
            enable_queue_depth_stats(false);
            snapshot
        } else {
            None
        };
        (zero_fill, queue_stats)
    }

    fn synthesize_zero_fill(columns: usize, eval_len: usize, iterations: usize) -> ZeroFillSummary {
        let total_len = columns.saturating_mul(eval_len).max(1);
        let bytes = total_len.saturating_mul(mem::size_of::<u64>()).max(1);
        let repeats = iterations.max(1);
        let mut buffer = Vec::with_capacity(total_len);
        let mut samples = Vec::with_capacity(repeats);
        for _ in 0..repeats {
            buffer.clear();
            let started = Instant::now();
            buffer.resize(total_len, 0);
            samples.push(elapsed_ms(started.elapsed()));
        }
        ZeroFillSummary {
            bytes,
            ms: Summary::from_samples(&samples),
            queue_delta: None,
        }
    }

    pub(super) fn inject_zero_fill(operations: &mut [Value], summary: &ZeroFillSummary) -> bool {
        for entry in operations {
            if let Some(map) = entry.as_object_mut() {
                let is_lde = map
                    .get("operation")
                    .and_then(Value::as_str)
                    .map_or(false, |name| name == "lde");
                if is_lde {
                    map.insert("zero_fill".into(), zero_fill_value(summary));
                    return true;
                }
            }
        }
        false
    }

    fn resolve_execution_metadata(log_probe: bool) -> (ExecutionMode, String, bool) {
        let requested_mode = ExecutionMode::Auto;
        let backend_label = Arc::new(Mutex::new(None::<String>));
        let capture = backend_label.clone();
        set_execution_mode_observer(move |requested, _resolved, backend| {
            if let (ExecutionMode::Auto, Ok(mut guard)) = (requested, capture.lock()) {
                *guard = backend.map(|kind| kind.as_str().to_owned());
            }
        });
        let resolved_mode = requested_mode.resolve();
        clear_execution_mode_observer();
        let backend = backend_label
            .lock()
            .ok()
            .and_then(|value| value.clone())
            .unwrap_or_else(|| "none".to_owned());
        let gpu_available = matches!(resolved_mode, ExecutionMode::Gpu);
        if log_probe {
            log_gpu_probe_summary(requested_mode, resolved_mode, &backend);
        }
        (resolved_mode, backend, gpu_available)
    }

    pub(super) fn enforce_gpu_requirement(
        require_gpu: bool,
        gpu_forced: bool,
        gpu_available: bool,
        resolved_mode: ExecutionMode,
        backend_label: &str,
    ) -> Result<(), String> {
        if gpu_available || (!require_gpu && !gpu_forced) {
            return Ok(());
        }
        let mut requested = Vec::new();
        if require_gpu {
            requested.push("--require-gpu".to_owned());
        }
        if gpu_forced {
            requested.push("FASTPQ_GPU=gpu".to_owned());
        }
        let requested = if requested.is_empty() {
            "GPU execution".to_owned()
        } else {
            requested.join(" + ")
        };
        Err(format!(
            "{requested} requested but no GPU backend was detected (resolved mode={}, backend=\"{}\"). \
Set FASTPQ_DEBUG_METAL_ENUM=1 to inspect Metal enumeration and ensure FASTPQ_METAL_LIB points at a compiled metallib; rerun with --gpu-probe to capture the detection snapshot.",
            resolved_mode.as_str(),
            backend_label
        ))
    }

    pub(crate) fn fastpq_gpu_forced_raw(raw: Option<&str>) -> bool {
        raw.map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "gpu" | "on" | "enable" | "enabled" | "1"
            )
        })
        .unwrap_or(false)
    }

    pub(crate) fn fastpq_gpu_forced_via_env() -> bool {
        fastpq_gpu_forced_raw(debug_env_var("FASTPQ_GPU").as_deref())
    }

    fn log_gpu_probe_summary(
        requested: ExecutionMode,
        resolved: ExecutionMode,
        backend_label: &str,
    ) {
        eprintln!("fastpq_metal_bench: GPU detection snapshot");
        let override_value = debug_env_var("FASTPQ_GPU").unwrap_or_else(|| "<unset>".to_owned());
        eprintln!("  FASTPQ_GPU override: {override_value}");
        eprintln!("  requested execution mode: {}", requested.as_str());
        eprintln!("  resolved execution mode: {}", resolved.as_str());
        eprintln!("  detected backend: {backend_label}");
        if let Some(path) = debug_env_var("FASTPQ_METAL_LIB") {
            if !path.is_empty() {
                eprintln!("  FASTPQ_METAL_LIB override: {path}");
            }
        }
        #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
        log_metal_device_inventory();
        #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
        eprintln!("  Metal device inventory unavailable on this platform");
    }

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    fn log_metal_device_inventory() {
        let devices = Device::all();
        if devices.is_empty() {
            eprintln!(
                "  metal::Device::all(): detected 0 devices (MTLCopyAllDevices returned none)"
            );
            return;
        }
        eprintln!(
            "  metal::Device::all(): detected {} device(s)",
            devices.len()
        );
        for device in devices {
            eprintln!(
                "    - {} (registry_id={}, location={}, low_power={}, headless={})",
                device.name(),
                device.registry_id(),
                device_location_label(device.location()),
                bool_display(device.is_low_power()),
                bool_display(device.is_headless())
            );
        }
    }

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    fn bool_display(value: bool) -> &'static str {
        if value { "yes" } else { "no" }
    }

    #[cfg_attr(
        not(any(test, all(feature = "fastpq-gpu", target_os = "macos"))),
        allow(dead_code)
    )]
    fn apple_soc_label(name: &str) -> Option<String> {
        let tokens: Vec<&str> = name.split_whitespace().collect();
        if tokens.is_empty() || !tokens[0].eq_ignore_ascii_case("apple") {
            return None;
        }
        for idx in 1..tokens.len() {
            let token = tokens[idx].trim_matches(|ch: char| ch == ',' || ch == ';');
            if token.len() >= 2
                && token.starts_with('M')
                && token[1..].chars().all(|c| c.is_ascii_digit())
            {
                let mut label = token.to_string();
                if let Some(next) = tokens.get(idx + 1) {
                    let lower = next.to_ascii_lowercase();
                    if matches!(lower.as_str(), "pro" | "max" | "ultra") {
                        label.push(' ');
                        label.push_str(next);
                    }
                }
                return Some(label);
            }
        }
        None
    }

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    fn capture_device_profile() -> Option<Value> {
        let device = Device::system_default()?;
        let mut profile = json::Map::new();
        let name = device.name().to_owned();
        let is_low_power = device.is_low_power();
        let is_headless = device.is_headless();
        let location = device.location();
        let discrete = !is_low_power
            || is_headless
            || matches!(
                location,
                MTLDeviceLocation::Slot | MTLDeviceLocation::External
            );
        profile.insert(
            "name".into(),
            json::to_value(&name).expect("serialize device name"),
        );
        profile.insert(
            "registry_id".into(),
            json::to_value(&device.registry_id()).expect("serialize registry id"),
        );
        profile.insert(
            "low_power".into(),
            json::to_value(&is_low_power).expect("serialize low_power flag"),
        );
        profile.insert(
            "headless".into(),
            json::to_value(&is_headless).expect("serialize headless flag"),
        );
        profile.insert(
            "location".into(),
            json::to_value(device_location_label(location)).expect("serialize location"),
        );
        profile.insert(
            "discrete_gpu".into(),
            json::to_value(&discrete).expect("serialize discrete flag"),
        );
        profile.insert(
            "recommended_max_working_set_bytes".into(),
            json::to_value(&device.recommended_max_working_set_size())
                .expect("serialize working set size"),
        );
        if let Some(model) = hw_model_identifier() {
            profile.insert(
                "hw_model".into(),
                json::to_value(&model).expect("serialize hw.model"),
            );
        }
        if let Some(label) = apple_soc_label(&name) {
            profile.insert(
                "apple_soc".into(),
                json::to_value(&label).expect("serialize apple soc label"),
            );
        }
        Some(Value::Object(profile))
    }

    #[cfg(not(all(feature = "fastpq-gpu", target_os = "macos")))]
    fn capture_device_profile() -> Option<Value> {
        None
    }

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    fn device_location_label(location: MTLDeviceLocation) -> &'static str {
        match location {
            MTLDeviceLocation::BuiltIn => "built_in",
            MTLDeviceLocation::Slot => "slot",
            MTLDeviceLocation::External => "external",
            MTLDeviceLocation::Unspecified => "unspecified",
            _ => "unspecified",
        }
    }

    #[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
    fn hw_model_identifier() -> Option<String> {
        let output = Command::new("sysctl")
            .arg("-n")
            .arg("hw.model")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8(output.stdout).ok()?;
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    }

    struct ReportInputs<'a> {
        parameter_name: &'a str,
        config: &'a Config,
        padded: usize,
        trace_log: u32,
        blowup_log: u32,
        column_count: usize,
        resolved_mode: ExecutionMode,
        backend_label: &'a str,
        gpu_available: bool,
        timestamp: u64,
        run_status: RunStatus,
        fft_tuning: Option<fastpq_prover::FftTuning>,
        queue_stats: Option<QueueDepthStats>,
        poseidon_queue: Option<QueueDepthStats>,
        poseidon_pipeline: Option<PoseidonPipelineStats>,
        column_staging: Option<ColumnStagingStats>,
        twiddle_stats: Option<TwiddleCacheStats>,
        kernel_stats: Option<Vec<KernelStatsSample>>,
        adaptive_heuristics: Option<AdaptiveScheduleSnapshot>,
        post_tile_stats: Option<Vec<PostTileSample>>,
        poseidon_micro: Option<Value>,
        device_profile: Option<Value>,
    }

    fn build_report(inputs: &ReportInputs<'_>, operations: Vec<Value>) -> Value {
        let mut report = BTreeMap::new();
        report.insert(
            "parameter_set".to_owned(),
            json::to_value(inputs.parameter_name).expect("serialize parameter set"),
        );
        report.insert(
            "rows".to_owned(),
            json::to_value(&inputs.config.rows).expect("serialize rows"),
        );
        report.insert(
            "padded_rows".to_owned(),
            json::to_value(&inputs.padded).expect("serialize padded rows"),
        );
        report.insert(
            "trace_log2".to_owned(),
            json::to_value(&inputs.trace_log).expect("serialize trace log"),
        );
        report.insert(
            "lde_log2".to_owned(),
            json::to_value(&(inputs.trace_log + inputs.blowup_log)).expect("serialize lde log"),
        );
        report.insert(
            "column_count".to_owned(),
            json::to_value(&inputs.column_count).expect("serialize column count"),
        );
        report.insert(
            "warmups".to_owned(),
            json::to_value(&inputs.config.warmups).expect("serialize warmups"),
        );
        report.insert(
            "iterations".to_owned(),
            json::to_value(&inputs.config.iterations).expect("serialize iterations"),
        );
        report.insert(
            "execution_mode".to_owned(),
            json::to_value(inputs.resolved_mode.as_str()).expect("serialize execution mode"),
        );
        report.insert(
            "gpu_backend".to_owned(),
            json::to_value(inputs.backend_label).expect("serialize backend label"),
        );
        report.insert(
            "gpu_available".to_owned(),
            json::to_value(&inputs.gpu_available).expect("serialize gpu availability"),
        );
        report.insert("run_status".to_owned(), inputs.run_status.value());
        if let Some(trace) = inputs.config.trace.as_ref() {
            report.insert(
                "metal_trace_template".to_owned(),
                json::to_value(&trace.template).expect("serialize trace template"),
            );
            report.insert(
                "metal_trace_seconds".to_owned(),
                json::to_value(&trace.seconds).expect("serialize trace duration"),
            );
            report.insert(
                "metal_trace_output".to_owned(),
                json::to_value(&trace.output.display().to_string())
                    .expect("serialize trace output path"),
            );
        }
        report.insert(
            "unix_epoch_secs".to_owned(),
            json::to_value(&inputs.timestamp).expect("serialize timestamp"),
        );
        if let Some(tuning) = inputs.fft_tuning {
            report.insert("fft_tuning".to_owned(), fft_tuning_value(&tuning));
        }
        let mut queue_block = inputs.queue_stats.as_ref().map(queue_stats_value);
        if let Some(poseidon) = inputs.poseidon_queue.as_ref() {
            let mut value = queue_block
                .take()
                .unwrap_or_else(|| Value::Object(json::Map::new()));
            if let Some(map) = value.as_object_mut() {
                map.insert("poseidon".into(), queue_stats_value(poseidon));
            }
            queue_block = Some(value);
        }
        if let Some(pipeline) = inputs.poseidon_pipeline.as_ref() {
            let mut value = queue_block
                .take()
                .unwrap_or_else(|| Value::Object(json::Map::new()));
            if let Some(map) = value.as_object_mut() {
                map.insert(
                    "poseidon_pipeline".into(),
                    poseidon_pipeline_value(pipeline),
                );
            }
            queue_block = Some(value);
        }
        if let Some(value) = queue_block {
            report.insert("metal_dispatch_queue".to_owned(), value);
        }
        if let Some(stats) = inputs.column_staging.as_ref() {
            report.insert("column_staging".to_owned(), column_staging_value(stats));
        }
        if let Some(stats) = inputs.twiddle_stats.as_ref() {
            report.insert("twiddle_cache".to_owned(), twiddle_cache_value(stats));
        }
        if let Some(snapshot) = inputs.adaptive_heuristics.as_ref() {
            if let Some(value) = heuristics_value(snapshot) {
                report.insert("metal_heuristics".to_owned(), value);
            }
        }
        if let Some(samples) = inputs.post_tile_stats.as_ref() {
            if !samples.is_empty() {
                report.insert("post_tile_dispatches".to_owned(), post_tile_value(samples));
            }
        }
        if let Some(samples) = inputs.kernel_stats.as_ref() {
            if !samples.is_empty() {
                report.insert("kernel_profiles".to_owned(), kernel_stats_value(samples));
                if let Some(value) = poseidon_profiles_value(samples) {
                    report.insert("poseidon_profiles".to_owned(), value);
                }
                if let Some(value) = bn254_dispatch_summary(samples) {
                    report.insert("bn254_dispatch".to_owned(), value);
                }
            }
        }
        if let Some(sample) = inputs.poseidon_micro.as_ref() {
            report.insert("poseidon_microbench".to_owned(), sample.clone());
        }
        if let Some(profile) = inputs.device_profile.as_ref() {
            report.insert("device_profile".to_owned(), profile.clone());
        }
        if let Some(metrics) = bn254_metrics_value(&operations, inputs.backend_label) {
            report.insert("bn254_metrics".to_owned(), metrics);
        }
        report.insert("operations".to_owned(), Value::Array(operations));
        Value::Object(report)
    }

    fn write_report(payload: &Value, output: Option<&Path>) -> Result<(), String> {
        let json_text = json::to_json_pretty(payload).map_err(|err| err.to_string())?;
        if let Some(path) = output {
            fs::write(path, json_text.as_bytes())
                .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
            eprintln!("fastpq_metal_bench: wrote {}", path.display());
        } else {
            println!("{json_text}");
        }
        Ok(())
    }

    #[cfg(test)]
    mod apple_soc_tests {
        use super::apple_soc_label;

        #[test]
        fn detects_plain_soc_label() {
            assert_eq!(apple_soc_label("Apple M3"), Some(String::from("M3")));
        }

        #[test]
        fn detects_soc_with_suffix() {
            assert_eq!(
                apple_soc_label("Apple M2 Max"),
                Some(String::from("M2 Max"))
            );
        }

        #[test]
        fn ignores_non_apple_devices() {
            assert_eq!(apple_soc_label("AMD Radeon Pro 5500M"), None);
        }
    }
}
