//! Metal continuation of canonically framed six-lane Digest384 byte fields.

use fastpq_isi::{
    GOLDILOCKS_DIGEST384_LANES_V1, GOLDILOCKS_DIGEST384_ROUNDS_V1, GoldilocksDigest384V1,
    goldilocks_digest384_lane_round_constants_v1,
    poseidon::{MDS, STATE_WIDTH},
};

use super::*;
use crate::digest384_batch::Digest384LastFieldJob;

pub(super) const KERNEL: &str = "fastpq_digest384_last_fields";
static CONTEXT: OnceLock<MetalResult<Digest384Context>> = OnceLock::new();

struct Digest384Context {
    device: Device,
    queues: QueuePool,
    pipeline: ComputePipelineState,
    round_constants: Buffer,
    mds: Buffer,
}

#[derive(Debug)]
struct BatchLayout {
    jobs: u32,
    threads: u32,
    prefix_words: usize,
    slice_words: usize,
    output_words: usize,
    payload_bytes: usize,
}

fn checked_layout(job_count: usize, payload_bytes: usize) -> MetalResult<BatchLayout> {
    let jobs = u32::try_from(job_count)
        .map_err(|_| GpuError::InvalidInput("Digest384 Metal job count exceeds u32"))?;
    let threads = jobs
        .checked_mul(GOLDILOCKS_DIGEST384_LANES_V1 as u32)
        .ok_or(GpuError::InvalidInput(
            "Digest384 Metal lane count exceeds u32",
        ))?;
    let output_words = usize::try_from(threads)
        .map_err(|_| GpuError::InvalidInput("Digest384 Metal output exceeds platform limits"))?;
    let prefix_words = output_words.checked_mul(4).ok_or(GpuError::InvalidInput(
        "Digest384 Metal prefixes exceed platform limits",
    ))?;
    let slice_words = job_count.checked_mul(2).ok_or(GpuError::InvalidInput(
        "Digest384 Metal slices exceed platform limits",
    ))?;
    for words in [prefix_words, slice_words, output_words] {
        words
            .checked_mul(mem::size_of::<u64>())
            .ok_or(GpuError::InvalidInput(
                "Digest384 Metal buffer bytes exceed platform limits",
            ))?;
    }
    u64::try_from(payload_bytes)
        .map_err(|_| GpuError::InvalidInput("Digest384 Metal payload exceeds u64"))?;
    Ok(BatchLayout {
        jobs,
        threads,
        prefix_words,
        slice_words,
        output_words,
        payload_bytes,
    })
}

fn build_context() -> MetalResult<Digest384Context> {
    let device = select_metal_device().ok_or(GpuError::Unsupported(GpuBackend::Metal))?;
    register_metal_device_hints(&device);
    let library = load_metal_library(&device)?;
    let pipeline = load_pipeline(&device, &library, KERNEL)?;
    let queues = QueuePool::new(&device, resolve_queue_policy(&device))?;
    let mut constants =
        [0_u64; GOLDILOCKS_DIGEST384_LANES_V1 * GOLDILOCKS_DIGEST384_ROUNDS_V1 * STATE_WIDTH];
    for lane in 0..GOLDILOCKS_DIGEST384_LANES_V1 {
        for round in 0..GOLDILOCKS_DIGEST384_ROUNDS_V1 {
            let offset = (lane * GOLDILOCKS_DIGEST384_ROUNDS_V1 + round) * STATE_WIDTH;
            constants[offset..offset + STATE_WIDTH].copy_from_slice(
                &goldilocks_digest384_lane_round_constants_v1(lane, round)
                    .expect("fixed canonical lane and round"),
            );
        }
    }
    let round_constants = copied_buffer(&device, &constants)?;
    let mds = copied_buffer(&device, &MDS)?;
    Ok(Digest384Context {
        device,
        queues,
        pipeline,
        round_constants,
        mds,
    })
}

/// Resume every canonical job on Metal and return only completed digest words.
pub(crate) fn hash_last_fields(
    jobs: &[Digest384LastFieldJob<'_>],
) -> MetalResult<Vec<GoldilocksDigest384V1>> {
    if jobs.is_empty() {
        return Ok(Vec::new());
    }
    let payload_bytes = jobs.iter().try_fold(0usize, |total, job| {
        total
            .checked_add(job.final_field().len())
            .ok_or(GpuError::InvalidInput(
                "Digest384 Metal concatenated payload exceeds platform limits",
            ))
    })?;
    let layout = checked_layout(jobs.len(), payload_bytes)?;
    let context = match CONTEXT.get_or_init(build_context) {
        Ok(context) => context,
        Err(error) => return Err(error.clone()),
    };
    for byte_len in [
        layout.prefix_words * mem::size_of::<u64>(),
        layout.slice_words * mem::size_of::<u64>(),
        layout.payload_bytes.max(1),
    ] {
        validate_metal_buffer_byte_len(&context.device, byte_len as u64)?;
    }
    validate_metal_pooled_word_len(&context.device, layout.output_words)?;
    let mut prefixes = Vec::new();
    prefixes
        .try_reserve_exact(layout.prefix_words)
        .map_err(|_| {
            GpuError::InvalidInput("Digest384 Metal prefixes exceed available host memory")
        })?;
    let mut slices = Vec::new();
    slices.try_reserve_exact(layout.slice_words).map_err(|_| {
        GpuError::InvalidInput("Digest384 Metal slices exceed available host memory")
    })?;
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(layout.payload_bytes.max(1))
        .map_err(|_| {
            GpuError::InvalidInput("Digest384 Metal payload exceeds available host memory")
        })?;
    for job in jobs {
        for lane in 0..GOLDILOCKS_DIGEST384_LANES_V1 {
            let prefix = job
                .prefix()
                .lane_prefix_v1(lane)
                .expect("fixed canonical lane");
            prefixes.extend_from_slice(&prefix.state());
            prefixes.push(prefix.next_rate_position() as u64);
        }
        slices.extend_from_slice(&[payload.len() as u64, job.final_field().len() as u64]);
        payload.extend_from_slice(job.final_field());
    }
    if payload.is_empty() {
        payload.push(0); // Metal requires a nonempty allocation, even for zero-byte fields.
    }
    let prefixes = copied_buffer(&context.device, &prefixes)?;
    let slices = copied_buffer(&context.device, &slices)?;
    let payload = copied_buffer(&context.device, &payload)?;
    let mut output = PooledBuffer::zeroed(layout.output_words)?;
    let output_buffer = shared_pooled_buffer(&context.device, &mut output)?;
    let (queue, queue_index) = context.queues.select(layout.jobs, 0);
    let ticket = submit_compute(
        queue,
        queue_index,
        &context.pipeline,
        u64::from(layout.threads),
        None,
        false,
        |encoder| {
            encoder.set_buffer(0, Some(&prefixes), 0);
            encoder.set_buffer(1, Some(&payload), 0);
            encoder.set_buffer(2, Some(&slices), 0);
            encoder.set_buffer(3, Some(&context.round_constants), 0);
            encoder.set_buffer(4, Some(&context.mds), 0);
            encoder.set_buffer(5, Some(&output_buffer), 0);
            encoder.set_bytes(
                6,
                mem::size_of::<u32>() as u64,
                ptr::from_ref(&layout.jobs).cast(),
            );
        },
    )?;
    // Do not read shared memory or return apparent success on a failed/timed-out
    // command. Metal retains every buffer (including the pooled backing) until
    // its command releases them, preserving fallback safety after an error.
    collect_completed_output(wait_for_ticket(ticket), &output, jobs.len())
}

fn collect_completed_output(
    completion: MetalResult<()>,
    output: &PooledBuffer,
    job_count: usize,
) -> MetalResult<Vec<GoldilocksDigest384V1>> {
    completion?;
    let expected_words =
        job_count
            .checked_mul(GOLDILOCKS_DIGEST384_LANES_V1)
            .ok_or(GpuError::InvalidInput(
                "Digest384 Metal result length exceeds platform limits",
            ))?;
    if output.len() != expected_words {
        return Err(GpuError::InvalidInput(
            "Digest384 Metal result length differs from job count",
        ));
    }
    let mut digests = Vec::new();
    digests.try_reserve_exact(job_count).map_err(|_| {
        GpuError::InvalidInput("Digest384 Metal digests exceed available host memory")
    })?;
    for index in 0..job_count {
        let mut words = [0_u64; GOLDILOCKS_DIGEST384_LANES_V1];
        output.copy_range_to_slice(index * GOLDILOCKS_DIGEST384_LANES_V1, &mut words);
        digests.push(
            GoldilocksDigest384V1::new(words).ok_or(GpuError::Execution {
                backend: GpuBackend::Metal,
                message: "Digest384 Metal returned a noncanonical field element".into(),
            })?,
        );
    }
    Ok(digests)
}

#[cfg(test)]
mod tests {
    use crate::digest384_batch::{hash_last_fields_cpu, try_hash_last_fields_metal};
    use fastpq_isi::{GoldilocksDigest384LastFieldStreamV1, GoldilocksDigestDomainV1};

    use super::*;

    #[test]
    fn layout_checks_lane_and_buffer_arithmetic() {
        let layout = checked_layout(2, 15).unwrap();
        assert_eq!(layout.jobs, 2);
        assert_eq!(layout.threads, 12);
        assert_eq!(layout.prefix_words, 48);
        assert_eq!(layout.slice_words, 4);
        assert_eq!(layout.output_words, 12);
        assert_eq!(layout.payload_bytes, 15);
        assert!(checked_layout(usize::MAX, 0).is_err());
        assert!(checked_layout(u32::MAX as usize / 6 + 1, 0).is_err());
        assert_eq!(checked_layout(0, 0).unwrap().threads, 0);
    }

    #[test]
    fn completion_failure_is_propagated_before_reading_shared_output() {
        let empty = PooledBuffer::zeroed(0).unwrap();
        let failed = Err(GpuError::Execution {
            backend: GpuBackend::Metal,
            message: "injected command timeout".into(),
        });
        assert!(matches!(
            collect_completed_output(failed, &empty, usize::MAX),
            Err(GpuError::Execution { message, .. }) if message == "injected command timeout"
        ));
    }

    #[test]
    fn completed_output_requires_exact_shape_and_canonical_lane_words() {
        let words = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, FIELD_MODULUS - 1];
        let output = PooledBuffer::from_slice(&words).unwrap();
        let digests = collect_completed_output(Ok(()), &output, 2).unwrap();
        assert_eq!(digests[0].words(), [0, 1, 2, 3, 4, 5]);
        assert_eq!(digests[1].words(), [6, 7, 8, 9, 10, FIELD_MODULUS - 1]);
        assert!(collect_completed_output(Ok(()), &output, 1).is_err());
        assert!(collect_completed_output(Ok(()), &output, usize::MAX).is_err());
        let invalid = PooledBuffer::from_slice(&[0, 1, 2, 3, 4, FIELD_MODULUS]).unwrap();
        assert!(matches!(
            collect_completed_output(Ok(()), &invalid, 1),
            Err(GpuError::Execution { .. })
        ));
    }

    #[test]
    #[ignore = "requires real Metal execution; never substitutes CPU results"]
    fn digest384_heterogeneous_frames_match_cpu_on_metal() {
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        let lengths = [0, 1, 6, 7, 8, 13, 14, 15, 27, 28, 29, 135, 136, 137, 512];
        let payloads: Vec<Vec<u8>> = lengths
            .iter()
            .map(|&len| {
                (0..len)
                    .map(|index| ((index * 73 + len) & 255) as u8)
                    .collect()
            })
            .collect();
        let mut positions = [false; 2];
        let jobs: Vec<_> = payloads
            .iter()
            .enumerate()
            .map(|(index, payload)| {
                let domain = GoldilocksDigestDomainV1 {
                    catalog: b"iroha-privacy-exact12-v1",
                    protocol: b"fastpq-state-transition-stark-v1",
                    profile: b"fastpq-state-transition-stark-v1",
                    role: b"fastpq:v1:air-trace",
                    phase: b"leaf",
                    level: u64::MAX,
                    index: index as u64,
                    counter: u64::MAX,
                };
                let fields: &[&[u8]] = if index % 2 == 0 { &[] } else { &[b""] };
                let stream =
                    GoldilocksDigest384LastFieldStreamV1::new(domain, fields, payload.len())
                        .unwrap();
                positions[stream.lane_prefix_v1(0).unwrap().next_rate_position()] = true;
                Digest384LastFieldJob::new(stream, payload).unwrap()
            })
            .collect();
        assert_eq!(
            positions,
            [true, true],
            "exercise both prefix rate positions"
        );
        let expected = hash_last_fields_cpu(&jobs).unwrap();
        for _ in 0..3 {
            assert_eq!(
                try_hash_last_fields_metal(&jobs).expect("actual Metal completion"),
                expected
            );
        }
        assert!(hash_last_fields(&[]).unwrap().is_empty());
    }

    #[test]
    #[ignore = "requires real Metal execution of independent Python reference frames"]
    fn digest384_matches_independent_reference_frames_on_metal() {
        struct Frame {
            domain: [Vec<u8>; 5],
            level: u64,
            index: u64,
            counter: u64,
            fields: Vec<Vec<u8>>,
            expected: GoldilocksDigest384V1,
        }
        let _gpu_lane = crate::backend::acquire_gpu_lane();
        let frames: Vec<_> = include_str!("../../fastpq_isi/src/assets/digest384_reference_v1.tsv")
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .filter_map(|line| {
                let parts: Vec<_> = line.split('\t').collect();
                assert_eq!(parts.len(), 11);
                if parts[9] == "-" {
                    return None; // A no-fields digest has no final-field handoff.
                }
                Some(Frame {
                    domain: core::array::from_fn(|index| hex::decode(parts[index + 1]).unwrap()),
                    level: parts[6].parse().unwrap(),
                    index: parts[7].parse().unwrap(),
                    counter: parts[8].parse().unwrap(),
                    fields: parts[9]
                        .split(',')
                        .map(|field| hex::decode(field).unwrap())
                        .collect(),
                    expected: GoldilocksDigest384V1::from_le_bytes(
                        hex::decode(parts[10]).unwrap().try_into().unwrap(),
                    )
                    .unwrap(),
                })
            })
            .collect();
        let jobs: Vec<_> = frames
            .iter()
            .map(|frame| {
                let domain = GoldilocksDigestDomainV1 {
                    catalog: &frame.domain[0],
                    protocol: &frame.domain[1],
                    profile: &frame.domain[2],
                    role: &frame.domain[3],
                    phase: &frame.domain[4],
                    level: frame.level,
                    index: frame.index,
                    counter: frame.counter,
                };
                let (final_field, preceding) = frame.fields.split_last().unwrap();
                let preceding: Vec<&[u8]> = preceding.iter().map(Vec::as_slice).collect();
                let stream = GoldilocksDigest384LastFieldStreamV1::new(
                    domain,
                    &preceding,
                    final_field.len(),
                )
                .unwrap();
                Digest384LastFieldJob::new(stream, final_field).unwrap()
            })
            .collect();
        assert!(!jobs.is_empty());
        let expected: Vec<_> = frames.iter().map(|frame| frame.expected).collect();
        assert_eq!(hash_last_fields_cpu(&jobs).unwrap(), expected);
        for _ in 0..3 {
            assert_eq!(
                try_hash_last_fields_metal(&jobs).expect("actual Metal completion"),
                expected
            );
        }
    }

    #[test]
    #[ignore = "measures real Metal and CPU continuation; local diagnostic, not release qualification"]
    fn digest384_air_row_batch_resource_diagnostic() {
        use std::{hint::black_box, time::Instant};

        let _gpu_lane = crate::backend::acquire_gpu_lane();
        // These are complete 256-column AIR-row payloads, with canonical field
        // elements and distinct typed row indices. Prefix preparation remains
        // CPU work and is measured separately for both execution routes.
        const COLUMNS: usize = 256;
        for job_count in [16, 64, 256] {
            let payloads: Vec<Vec<u8>> = (0..job_count)
                .map(|row| {
                    (0..COLUMNS)
                        .flat_map(|column| ((row * COLUMNS + column) as u64).to_le_bytes())
                        .collect()
                })
                .collect();
            let started = Instant::now();
            let jobs: Vec<_> = payloads
                .iter()
                .enumerate()
                .map(|(row, payload)| {
                    let domain = GoldilocksDigestDomainV1 {
                        catalog: fastpq_isi::FASTPQ_CATALOG_V1.as_bytes(),
                        protocol: fastpq_isi::FASTPQ_FINAL_V1_ID.as_bytes(),
                        profile: fastpq_isi::FASTPQ_FINAL_V1_ID.as_bytes(),
                        role: b"air-trace-commitment",
                        phase: b"leaf",
                        level: 0,
                        index: row as u64,
                        counter: 0,
                    };
                    let prefix =
                        GoldilocksDigest384LastFieldStreamV1::new(domain, &[], payload.len())
                            .unwrap();
                    Digest384LastFieldJob::new(prefix, payload).unwrap()
                })
                .collect();
            let prefix_ms = started.elapsed().as_secs_f64() * 1_000.0;
            let expected = hash_last_fields_cpu(&jobs).unwrap();
            // The first dispatch initializes the actual Metal context outside
            // the steady-state timer; every subsequent sample still includes
            // buffer allocation, upload, launch, completion and output reads.
            let started = Instant::now();
            assert_eq!(
                try_hash_last_fields_metal(&jobs).expect("actual Metal completion"),
                expected
            );
            let first_dispatch_ms = started.elapsed().as_secs_f64() * 1_000.0;
            for repetition in 0..3 {
                let started = Instant::now();
                let cpu = black_box(hash_last_fields_cpu(black_box(&jobs)).unwrap());
                let cpu_ms = started.elapsed().as_secs_f64() * 1_000.0;
                let started = Instant::now();
                let metal = black_box(
                    try_hash_last_fields_metal(black_box(&jobs)).expect("actual Metal completion"),
                );
                let metal_ms = started.elapsed().as_secs_f64() * 1_000.0;
                assert_eq!(cpu, expected);
                assert_eq!(metal, expected);
                let record = norito::json!({
                    "kind": "fastpq_digest384_air_row_batch_diagnostic_v1",
                    "qualification": "local diagnostic only; compiled test profile",
                    "jobs": job_count,
                    "columns": COLUMNS,
                    "payload_bytes_per_job": (COLUMNS * 8),
                    "repetition": repetition,
                    "prefix_prepare_ms": prefix_ms,
                    "first_metal_dispatch_ms": first_dispatch_ms,
                    "cpu_continuation_ms": cpu_ms,
                    "metal_continuation_ms": metal_ms,
                    "cpu_prefix_and_continuation_ms": (prefix_ms + cpu_ms),
                    "metal_prefix_and_continuation_ms": (prefix_ms + metal_ms),
                    "actual_metal_dispatch": true,
                    "all_six_lanes_equal": true,
                });
                println!("{}", norito::json::to_json(&record).unwrap());
            }
        }
    }
}
