//! Canonical six-lane primitive measurements with explicit device dispatch and full parity.
//!
//! These developer measurements do not qualify a complete proof or admit a profile.
use std::{hint::black_box, time::Instant};

use fastpq_isi::{
    FASTPQ_CATALOG_V1, FASTPQ_FINAL_V1_ID, GoldilocksDigest384FrameV1, GoldilocksDigest384V1,
    StarkParameterSet,
};
use norito::json::{self, Value};

use crate::{
    Error, Result, TraceColumn, digest,
    digest_executor::{self, DigestExecutionV1},
};

/// Explicit device requested for a primitive measurement; errors never select the CPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Digest384BenchmarkDeviceV1 {
    /// Complete six-lane Metal dispatcher.
    Metal,
    /// Complete six-lane CUDA dispatcher.
    Cuda,
}
impl Digest384BenchmarkDeviceV1 {
    fn label(self) -> &'static str {
        match self {
            Self::Metal => "metal",
            Self::Cuda => "cuda",
        }
    }
}

/// Public synthetic input to the actual preprocessing commitment framing owner.
#[derive(Clone, Copy)]
pub enum Digest384BenchmarkInputV1<'a> {
    /// Named columns containing canonical Goldilocks values of equal positive length.
    TraceColumns(&'a [TraceColumn]),
    /// Complete six-lane child digests, with exactly two children per parent.
    MerklePairs(&'a [GoldilocksDigest384V1]),
}

/// Timing samples and exact evidence for one complete primitive input batch.
pub struct Digest384BenchmarkReportV1 {
    /// Canonical operation ID; no scalar operation aliases are accepted.
    pub operation: &'static str,
    /// Number of complete digest frames per invocation.
    pub columns: usize,
    /// Values per column, or twelve words per digest pair.
    pub input_len: usize,
    /// Aggregate bytes of actual input fields, including UTF-8 column names.
    pub input_bytes: usize,
    /// Aggregate six-lane output bytes.
    pub output_bytes: usize,
    /// Logical frame-word, descriptor and output buffer bytes per invocation.
    pub gpu_payload_buffer_bytes: usize,
    /// CPU wall-clock samples; canonical encoding and hashing are included.
    pub cpu_samples_ms: Vec<f64>,
    /// Device wall-clock samples, present only after every requested invocation passed parity.
    pub gpu_samples_ms: Option<Vec<f64>>,
    /// Exact schema object consumed by both benchmark report adapters.
    pub evidence: Value,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
struct FrameWork {
    frames: usize,
    words: usize,
}
impl FrameWork {
    fn observe(&mut self, frames: &[GoldilocksDigest384FrameV1<'_>]) -> Result<()> {
        self.frames = add(self.frames, frames.len())?;
        for frame in frames {
            self.words = add(self.words, frame.word_count())?;
        }
        Ok(())
    }
}
#[derive(Default)]
struct DeviceWork {
    dispatches: usize,
    work: FrameWork,
    max_batch_frames: usize,
    max_batch_words: usize,
}
impl DeviceWork {
    #[cfg(any(test, feature = "fastpq-gpu"))]
    fn observe_completed(&mut self, frames: &[GoldilocksDigest384FrameV1<'_>]) -> Result<()> {
        let mut batch = FrameWork::default();
        batch.observe(frames)?;
        self.dispatches = add(self.dispatches, 1)?;
        self.work.frames = add(self.work.frames, batch.frames)?;
        self.work.words = add(self.work.words, batch.words)?;
        self.max_batch_frames = self.max_batch_frames.max(batch.frames);
        self.max_batch_words = self.max_batch_words.max(batch.words);
        Ok(())
    }
}
fn invalid(details: &str) -> Error {
    Error::NativeDigestExecution {
        details: details.into(),
    }
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| invalid("benchmark count overflow"))
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| invalid("benchmark byte/work count overflow"))
}

/// Measure complete canonical CPU/device frames and check every output lane in order.
///
/// Timing includes canonical input preparation and bounded dispatch. Successful device
/// counters exclude the separate readiness KAT and all CPU work. Logical buffer byte
/// counts exclude parameter buffers, dispatch arguments, page rounding and erasure traffic.
///
/// # Errors
/// Rejects empty or malformed geometry, noncanonical input, count overflow, unavailable
/// device execution, unsuccessful dispatch or any CPU/device output mismatch.
pub fn benchmark_digest384_v1(
    params: &StarkParameterSet,
    input: Digest384BenchmarkInputV1<'_>,
    device: Option<Digest384BenchmarkDeviceV1>,
    warmups: usize,
    iterations: usize,
) -> Result<Digest384BenchmarkReportV1> {
    benchmark_with_dispatch(
        params,
        input,
        device,
        warmups,
        iterations,
        &mut |device, frames, work| {
            #[cfg(feature = "fastpq-gpu")]
            {
                use crate::{
                    digest_executor::{
                        MAX_DIGEST384_BATCH_FRAMES_V1, MAX_DIGEST384_BATCH_WORDS_V1,
                    },
                    digest384_gpu::{Digest384GpuBackendV1, try_hash_digest384_frames_v1},
                };
                let backend = match device {
                    Digest384BenchmarkDeviceV1::Metal => Digest384GpuBackendV1::Metal,
                    Digest384BenchmarkDeviceV1::Cuda => Digest384GpuBackendV1::Cuda,
                };
                digest_executor::execute_bounded_digest384_frames_v1(
                    frames,
                    MAX_DIGEST384_BATCH_FRAMES_V1,
                    MAX_DIGEST384_BATCH_WORDS_V1,
                    &mut |chunk| {
                        let output = try_hash_digest384_frames_v1(backend, chunk)
                            .map_err(|error| invalid(&error.to_string()))?;
                        work.observe_completed(chunk)?;
                        Ok(output)
                    },
                )
            }
            #[cfg(not(feature = "fastpq-gpu"))]
            {
                let _ = (device, frames, work);
                Err(invalid(
                    "six-lane device execution requires the fastpq-gpu feature",
                ))
            }
        },
    )
}

fn execute_input(
    params: &StarkParameterSet,
    input: Digest384BenchmarkInputV1<'_>,
    execute: &mut impl FnMut(&[GoldilocksDigest384FrameV1<'_>]) -> Result<Vec<GoldilocksDigest384V1>>,
) -> Result<Vec<GoldilocksDigest384V1>> {
    match input {
        Digest384BenchmarkInputV1::TraceColumns(columns) => digest::hash_trace_columns_v1(
            params,
            columns,
            columns.first().map_or(0, |column| column.values.len()),
            execute,
        ),
        Digest384BenchmarkInputV1::MerklePairs(children) => {
            digest::hash_trace_pairs_v1(params, children, 1, execute)
        }
    }
}

fn benchmark_with_dispatch(
    params: &StarkParameterSet,
    input: Digest384BenchmarkInputV1<'_>,
    device: Option<Digest384BenchmarkDeviceV1>,
    warmups: usize,
    iterations: usize,
    dispatch: &mut impl FnMut(
        Digest384BenchmarkDeviceV1,
        &[GoldilocksDigest384FrameV1<'_>],
        &mut DeviceWork,
    ) -> Result<Vec<GoldilocksDigest384V1>>,
) -> Result<Digest384BenchmarkReportV1> {
    if fastpq_isi::find_by_name(FASTPQ_FINAL_V1_ID) != Some(params) {
        return Err(invalid(
            "benchmark requires the exact canonical FASTPQ V1 parameter set",
        ));
    }
    if iterations == 0 {
        return Err(invalid("benchmark iterations must be positive"));
    }
    let invocations = add(warmups, iterations)?;
    let (operation, phase, level, columns, input_len, input_bytes) = match input {
        Digest384BenchmarkInputV1::TraceColumns(columns) => {
            let rows = columns.first().map_or(0, |column| column.values.len());
            if columns.is_empty() || rows == 0 {
                return Err(invalid("trace column benchmark input is empty"));
            }
            let mut bytes = 0;
            for column in columns {
                bytes = add(bytes, add(column.name.len(), mul(column.values.len(), 8)?)?)?;
            }
            (
                "digest384_trace_columns",
                "column-leaf",
                0,
                columns.len(),
                rows,
                bytes,
            )
        }
        Digest384BenchmarkInputV1::MerklePairs(children) => {
            if children.is_empty() || !children.len().is_multiple_of(2) {
                return Err(invalid(
                    "Merkle benchmark requires complete nonempty digest pairs",
                ));
            }
            (
                "digest384_merkle_pairs",
                "binary-node",
                1,
                children.len() / 2,
                12,
                mul(children.len(), 48)?,
            )
        }
    };
    let mut reference_work = FrameWork::default();
    let expected = execute_input(params, input, &mut |frames| {
        reference_work.observe(frames)?;
        digest_executor::execute_digest384_frames_v1(frames, DigestExecutionV1::Cpu)
    })?;
    if reference_work.frames != columns || expected.len() != columns {
        return Err(invalid(
            "canonical reference returned an incorrect frame count",
        ));
    }
    let output_bytes = mul(columns, 48)?;
    let gpu_payload_buffer_bytes = add(
        add(mul(reference_work.words, 8)?, mul(columns, 24)?)?,
        output_bytes,
    )?;
    let expected_device_frames = mul(columns, invocations)?;
    let expected_device_words = mul(reference_work.words, invocations)?;
    let parity_checked_lanes = mul(expected_device_frames, 6)?;
    let mut cpu_samples_ms = Vec::with_capacity(iterations);
    for invocation in 0..invocations {
        let started = Instant::now();
        let output = execute_input(params, input, &mut |frames| {
            digest_executor::execute_digest384_frames_v1(frames, DigestExecutionV1::Cpu)
        })?;
        let elapsed = started.elapsed().as_secs_f64() * 1_000.0;
        if output != expected {
            return Err(invalid("canonical CPU reference changed"));
        }
        drop(black_box(output));
        if invocation >= warmups {
            cpu_samples_ms.push(elapsed);
        }
    }
    let mut fields = json::Map::new();
    for (key, value) in [
        ("schema", "fastpq-digest384-primitive-benchmark-v1"),
        ("catalog", FASTPQ_CATALOG_V1),
        ("protocol", FASTPQ_FINAL_V1_ID),
        ("profile", params.name),
        ("role", "fastpq:v1:preprocessing-trace"),
        ("phase", phase),
        ("output_encoding", "six-canonical-u64-le-words"),
    ] {
        fields.insert(key.into(), Value::from(value));
    }
    for (key, value) in [
        ("level", level),
        ("digest_lanes", 6),
        ("frame_count", columns),
        ("input_field_bytes", input_bytes),
        ("canonical_words", reference_work.words),
        ("sponge_permutations", mul(reference_work.words, 3)?),
        ("output_bytes", output_bytes),
    ] {
        fields.insert(
            key.into(),
            json::to_value(&value).expect("serialize checked work count"),
        );
    }
    fields.insert("cpu_reference_verified".into(), Value::Bool(true));
    let mut evidence = Value::Object(fields);
    let gpu_samples_ms = if let Some(device) = device {
        let mut samples = Vec::with_capacity(iterations);
        let mut work = DeviceWork::default();
        for invocation in 0..invocations {
            let started = Instant::now();
            let output = execute_input(params, input, &mut |frames| {
                dispatch(device, frames, &mut work)
            })?;
            let elapsed = started.elapsed().as_secs_f64() * 1_000.0;
            if output != expected {
                return Err(invalid(
                    "six-lane device output differs from canonical CPU reference",
                ));
            }
            drop(black_box(output));
            if invocation >= warmups {
                samples.push(elapsed);
            }
        }
        if work.work.frames != expected_device_frames
            || work.work.words != expected_device_words
            || work.dispatches == 0
        {
            return Err(invalid(
                "device payload counters do not cover every complete invocation",
            ));
        }
        let mut gpu = json::Map::new();
        gpu.insert("backend".into(), Value::from(device.label()));
        for (key, value) in [
            ("warmup_invocations", warmups),
            ("timed_invocations", iterations),
            ("invocations", invocations),
            ("dispatches", work.dispatches),
            ("frames", work.work.frames),
            ("canonical_words", work.work.words),
            ("descriptor_words", mul(work.work.frames, 3)?),
            ("output_words", mul(work.work.frames, 6)?),
            ("max_batch_frames", work.max_batch_frames),
            ("max_batch_words", work.max_batch_words),
            ("parity_checked_digests", expected_device_frames),
            ("parity_checked_lanes", parity_checked_lanes),
        ] {
            gpu.insert(
                key.into(),
                json::to_value(&value).expect("serialize checked device count"),
            );
        }
        evidence
            .as_object_mut()
            .expect("evidence object")
            .insert("gpu".into(), Value::Object(gpu));
        Some(samples)
    } else {
        None
    };
    Ok(Digest384BenchmarkReportV1 {
        operation,
        columns,
        input_len,
        input_bytes,
        output_bytes,
        gpu_payload_buffer_bytes,
        cpu_samples_ms,
        gpu_samples_ms,
        evidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn params() -> &'static StarkParameterSet {
        fastpq_isi::find_by_name("fastpq-state-transition-stark-v1").unwrap()
    }
    fn columns() -> Vec<TraceColumn> {
        vec![
            TraceColumn {
                name: "bench_00".into(),
                values: vec![1, 2, 3, 4],
            },
            TraceColumn {
                name: "bench_01".into(),
                values: vec![5, 6, 7, 8],
            },
        ]
    }
    #[test]
    fn cpu_benchmark_records_complete_frame_work_without_device_claim() {
        let input = columns();
        let report = benchmark_digest384_v1(
            params(),
            Digest384BenchmarkInputV1::TraceColumns(&input),
            None,
            1,
            2,
        )
        .unwrap();
        assert_eq!(report.columns, 2);
        assert_eq!(report.output_bytes, 96);
        assert_eq!(report.input_bytes, 80);
        assert_eq!(report.cpu_samples_ms.len(), 2);
        assert!(report.gpu_samples_ms.is_none());
        assert!(report.evidence.get("gpu").is_none());
        let words = report
            .evidence
            .get("canonical_words")
            .unwrap()
            .as_u64()
            .unwrap();
        assert_eq!(
            report.evidence.get("sponge_permutations").unwrap().as_u64(),
            Some(words * 3)
        );
        assert_eq!(report.gpu_payload_buffer_bytes as u64, words * 8 + 48 + 96);
    }
    #[test]
    fn observed_dispatch_counts_exclude_cpu_and_cover_warmups_and_every_lane() {
        let input = columns();
        let mut calls = 0;
        let report = benchmark_with_dispatch(
            params(),
            Digest384BenchmarkInputV1::TraceColumns(&input),
            Some(Digest384BenchmarkDeviceV1::Metal),
            1,
            2,
            &mut |_, frames, work| {
                digest_executor::execute_bounded_digest384_frames_v1(
                    frames,
                    1,
                    usize::MAX,
                    &mut |chunk| {
                        calls += 1;
                        let out = chunk.iter().map(GoldilocksDigest384FrameV1::hash).collect();
                        work.observe_completed(chunk)?;
                        Ok(out)
                    },
                )
            },
        )
        .unwrap();
        assert_eq!(calls, 6);
        let gpu = report.evidence.get("gpu").unwrap();
        for (field, expected) in [
            ("dispatches", 6),
            ("frames", 6),
            ("parity_checked_digests", 6),
            ("parity_checked_lanes", 36),
            ("max_batch_frames", 1),
            ("timed_invocations", 2),
        ] {
            assert_eq!(gpu.get(field).unwrap().as_u64(), Some(expected));
        }
        assert_eq!(report.gpu_samples_ms.unwrap().len(), 2);
    }
    #[test]
    fn every_device_lane_count_and_dispatch_failure_abort_capture() {
        let input = columns();
        for fault in 0..8 {
            let mut calls = 0;
            let result = benchmark_with_dispatch(
                params(),
                Digest384BenchmarkInputV1::TraceColumns(&input),
                Some(Digest384BenchmarkDeviceV1::Cuda),
                0,
                2,
                &mut |_, frames, work| {
                    calls += 1;
                    if fault == 7 {
                        return Err(invalid("injected device failure"));
                    }
                    let mut out: Vec<_> = frames
                        .iter()
                        .map(GoldilocksDigest384FrameV1::hash)
                        .collect();
                    work.observe_completed(frames)?;
                    if fault == 6 {
                        out.pop();
                    } else {
                        let bytes = out[1].to_le_bytes();
                        let mut words = std::array::from_fn(|lane| {
                            u64::from_le_bytes(bytes[lane * 8..lane * 8 + 8].try_into().unwrap())
                        });
                        words[fault] = (words[fault] + 1) % crate::GOLDILOCKS_MODULUS_V1;
                        out[1] = GoldilocksDigest384V1::new(words).unwrap();
                    }
                    Ok(out)
                },
            );
            assert!(result.is_err(), "fault {fault} must fail closed");
            assert_eq!(
                calls, 1,
                "failed capture must never retry or substitute CPU"
            );
        }
    }
    #[test]
    fn malformed_inputs_are_rejected_before_any_device_callback() {
        let mut cases = vec![vec![], columns(), columns()];
        cases[1][1].values.pop();
        cases[2][1].values[0] = crate::GOLDILOCKS_MODULUS_V1;
        for input in cases {
            assert!(
                benchmark_with_dispatch(
                    params(),
                    Digest384BenchmarkInputV1::TraceColumns(&input),
                    Some(Digest384BenchmarkDeviceV1::Cuda),
                    0,
                    1,
                    &mut |_, _, _| panic!("malformed input must never dispatch")
                )
                .is_err()
            );
        }
        let digest = GoldilocksDigest384V1::new([1; 6]).unwrap();
        assert!(
            benchmark_digest384_v1(
                params(),
                Digest384BenchmarkInputV1::MerklePairs(&[digest]),
                None,
                0,
                1
            )
            .is_err()
        );
        assert!(
            benchmark_digest384_v1(
                params(),
                Digest384BenchmarkInputV1::MerklePairs(&[digest, digest]),
                None,
                0,
                0
            )
            .is_err()
        );
        let mut wrong_params = *params();
        wrong_params.trace_root ^= 1;
        assert!(
            benchmark_digest384_v1(
                &wrong_params,
                Digest384BenchmarkInputV1::MerklePairs(&[digest, digest]),
                None,
                0,
                1
            )
            .is_err()
        );
        assert!(add(usize::MAX, 1).is_err());
        assert!(mul(usize::MAX, 6).is_err());
    }
}
