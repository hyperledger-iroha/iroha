use std::{
    error::Error,
    io::{BufRead, Read, Write},
    time::Instant,
};

use norito::{
    Archived, NoritoSerialize,
    codec::encode_with_header_flags,
    core::Header,
    crc64_fallback, decode_from_bytes,
    json::{self, Value},
    streaming::{
        BUNDLED_RANS_BUILD_AVAILABLE, BundleAcceleration, EntropyMode,
        chunk::BaselineDecoder,
        codec::{
            BaselineEncoder, BaselineEncoderConfig, Chroma420Frame, EncodedSegment,
            FrameDimensions, RawFrame, SegmentBundle,
        },
    },
};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;

/// Parameters controlling the streaming entropy benchmark.
#[derive(Clone, Debug)]
pub struct EntropyBenchOptions {
    pub frames_per_segment: u16,
    pub segments: u16,
    pub bundle_width: u8,
    pub y4m_out: Option<std::path::PathBuf>,
    pub y4m_in: Option<std::path::PathBuf>,
    pub chunk_out: Option<std::path::PathBuf>,
    pub psnr_mode: PsnrMode,
    pub quantizers: Vec<u8>,
    pub target_bitrates_mbps: Vec<f64>,
    pub tiny_clip_preset: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PsnrMode {
    /// Luma-only PSNR (default).
    Luma,
    /// Full-frame PSNR over YUV420.
    Yuv,
}

impl PsnrMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Luma => "y",
            Self::Yuv => "yuv",
        }
    }
}

impl Default for EntropyBenchOptions {
    fn default() -> Self {
        Self {
            frames_per_segment: 3,
            segments: 2,
            bundle_width: 3,
            y4m_out: None,
            y4m_in: None,
            chunk_out: None,
            psnr_mode: PsnrMode::Luma,
            quantizers: Vec::new(),
            target_bitrates_mbps: Vec::new(),
            tiny_clip_preset: false,
        }
    }
}

struct ModeBench {
    entropy_mode: EntropyMode,
    bundle_width: u8,
    bundle_accel: BundleAcceleration,
    encode_ms: f64,
    decode_ms: f64,
    chunk_bytes: usize,
    psnr_y: Option<f64>,
    psnr_yuv: Option<f64>,
    quantizer: u8,
    stream_bitrate_mbps: f64,
}

impl ModeBench {
    fn to_json(&self) -> Value {
        let elapsed_seconds = (self.encode_ms / 1_000.0).max(f64::MIN_POSITIVE);
        let bitrate_mbps = ((self.chunk_bytes as f64) * 8.0 / elapsed_seconds) / 1_000_000.0;

        let mut map = json::Map::new();
        map.insert(
            "entropy_mode".into(),
            Value::from(format!("{:?}", self.entropy_mode).to_lowercase()),
        );
        map.insert("bundle_width".into(), Value::from(self.bundle_width));
        map.insert(
            "bundle_acceleration".into(),
            Value::from(format!("{:?}", self.bundle_accel).to_lowercase()),
        );
        map.insert("chunk_bytes".into(), Value::from(self.chunk_bytes as u64));
        map.insert("encode_ms".into(), Value::from(self.encode_ms));
        map.insert("decode_ms".into(), Value::from(self.decode_ms));
        map.insert("bitrate_mbps".into(), Value::from(bitrate_mbps));
        map.insert(
            "stream_bitrate_mbps".into(),
            Value::from(self.stream_bitrate_mbps),
        );
        if let Some(psnr) = self.psnr_y {
            map.insert("psnr_y".into(), Value::from(psnr));
        }
        if let Some(psnr) = self.psnr_yuv {
            map.insert("psnr_yuv".into(), Value::from(psnr));
        }
        map.insert("quantizer".into(), Value::from(self.quantizer));
        Value::Object(map)
    }
}

struct ModeOutcome {
    bench: ModeBench,
    segments: Vec<EncodedSegment>,
    decoded_frames: Vec<RawFrame>,
    decoded_chroma: Vec<Chroma420Frame>,
    frame_duration_ns: u32,
}

struct QuantizerRun {
    quantizer: u8,
    outcome: ModeOutcome,
}

impl QuantizerRun {
    fn to_json(&self) -> Value {
        let mut map = json::Map::new();
        map.insert("quantizer".into(), Value::from(self.quantizer));
        map.insert("bench".into(), self.outcome.bench.to_json());
        Value::Object(map)
    }
}

/// Execute the bundled entropy benchmark.
pub fn run_entropy_bench(options: EntropyBenchOptions) -> Result<Value, Box<dyn Error>> {
    if options.frames_per_segment == 0 {
        return Err("frames_per_segment must be greater than zero".into());
    }
    if options.segments == 0 {
        return Err("segments must be greater than zero".into());
    }

    let mut rng = ChaCha20Rng::seed_from_u64(0x5EED);
    let (frame_dimensions, frames, chroma_frames, frames_per_segment, segments, bundle_width) =
        if let Some(y4m_path) = options.y4m_in.as_ref() {
            let parsed = parse_y4m_frames(y4m_path)?;
            let total_frames = parsed.frames.len();
            if total_frames == 0 {
                return Err("Y4M contained no frames".into());
            }
            (
                parsed.dimensions,
                parsed.frames,
                parsed.chroma,
                u16::try_from(total_frames)
                    .map_err(|_| "Y4M frame count exceeds u16::MAX for bench")?,
                1u16,
                options.bundle_width,
            )
        } else {
            let dimensions = if options.tiny_clip_preset {
                FrameDimensions::new(32, 16)
            } else {
                FrameDimensions::new(64, 32)
            };
            let bundle_width = if options.tiny_clip_preset {
                options.bundle_width.clamp(1, 2)
            } else {
                options.bundle_width
            };
            (
                dimensions,
                build_frames(
                    dimensions,
                    usize::from(options.frames_per_segment),
                    &mut rng,
                )?,
                build_chroma_frames(
                    dimensions,
                    usize::from(options.frames_per_segment),
                    &mut rng,
                )?,
                options.frames_per_segment,
                options.segments,
                bundle_width,
            )
        };

    let mut quantizers = options.quantizers.clone();
    if quantizers.is_empty() {
        quantizers.push(16);
    }
    if !options.target_bitrates_mbps.is_empty() && quantizers.len() == 1 {
        let base = quantizers[0];
        quantizers.push(base.saturating_add(4).min(63));
        quantizers.push(base.saturating_sub(4).max(1));
    }
    quantizers.sort_unstable();
    quantizers.dedup();

    if !BUNDLED_RANS_BUILD_AVAILABLE {
        return Err("Bundled rANS support is required; rebuild with ENABLE_RANS_BUNDLES=1".into());
    }

    let mut runs = Vec::with_capacity(quantizers.len());
    for quantizer in quantizers {
        let bundled = bench_mode(
            frame_dimensions,
            frames_per_segment,
            segments,
            bundle_width,
            BundleAcceleration::None,
            &frames,
            &chroma_frames,
            options.psnr_mode,
            quantizer,
        )?;
        runs.push(QuantizerRun {
            quantizer,
            outcome: bundled,
        });
    }

    let primary_idx = select_primary_run(&runs, options.target_bitrates_mbps.first().copied());
    let primary = runs.get(primary_idx).ok_or("no quantizer runs executed")?;

    let quantizer_attempts = runs
        .iter()
        .map(|run| {
            attempt_entry(
                run.quantizer,
                &run.outcome.bench,
                frames_per_segment,
                segments,
            )
        })
        .collect::<Vec<_>>();

    let bitrate_targets = build_bitrate_targets(&runs, &options.target_bitrates_mbps);
    let target_array = Value::Array(
        options
            .target_bitrates_mbps
            .iter()
            .copied()
            .map(Value::from)
            .collect(),
    );

    let mut root = json::Map::new();
    root.insert(
        "frames_per_segment".into(),
        json::Value::from(frames_per_segment),
    );
    root.insert("segments".into(), json::Value::from(segments));
    let mut dimensions = json::Map::new();
    dimensions.insert("width".into(), Value::from(frame_dimensions.width));
    dimensions.insert("height".into(), Value::from(frame_dimensions.height));
    root.insert("frame_dimensions".into(), Value::Object(dimensions));
    root.insert(
        "bundled_available".into(),
        json::Value::from(BUNDLED_RANS_BUILD_AVAILABLE),
    );
    root.insert(
        "psnr_mode".into(),
        json::Value::from(options.psnr_mode.as_str()),
    );
    root.insert(
        "tiny_clip_preset".into(),
        json::Value::from(options.tiny_clip_preset),
    );
    root.insert("quantizer".into(), json::Value::from(primary.quantizer));
    if !options.target_bitrates_mbps.is_empty() {
        root.insert("target_bitrates_mbps".into(), target_array);
        if let Some(first) = options.target_bitrates_mbps.first() {
            root.insert("target_bitrate_mbps".into(), json::Value::from(*first));
        }
    }
    root.insert(
        "quantizer_runs".into(),
        Value::Array(runs.iter().map(QuantizerRun::to_json).collect()),
    );
    root.insert(
        "quantizer_attempts".into(),
        Value::Array(quantizer_attempts),
    );
    root.insert("bench".into(), primary.outcome.bench.to_json());
    root.insert("bitrate_targets".into(), Value::Array(bitrate_targets));

    if let Some(out_path) = options.chunk_out {
        write_segment_bundles(
            &out_path,
            &primary.outcome.segments,
            frame_dimensions,
            primary.outcome.frame_duration_ns,
            Some(&primary.outcome.decoded_chroma),
            frames_per_segment,
        )?;
        let metadata = std::fs::metadata(&out_path)?;
        let mut chunk_info = json::Map::new();
        chunk_info.insert("path".into(), Value::from(out_path.display().to_string()));
        chunk_info.insert("bytes".into(), Value::from(metadata.len()));
        chunk_info.insert(
            "segments".into(),
            Value::from(primary.outcome.segments.len() as u64),
        );
        root.insert("chunk_out".into(), Value::Object(chunk_info));
    }
    if let Some(input) = options.y4m_in {
        root.insert("y4m_in".into(), Value::from(input.display().to_string()));
    }
    if let Some(path) = options.y4m_out.as_ref() {
        write_y4m_frames(
            path,
            frame_dimensions,
            &primary.outcome.decoded_frames,
            Some(&primary.outcome.decoded_chroma),
        )?;
    }

    Ok(json::Value::Object(root))
}

#[allow(clippy::too_many_arguments)]
fn bench_mode(
    frame_dimensions: FrameDimensions,
    frames_per_segment: u16,
    segments: u16,
    bundle_width: u8,
    bundle_accel: BundleAcceleration,
    frames: &[RawFrame],
    chroma: &[Chroma420Frame],
    psnr_mode: PsnrMode,
    quantizer: u8,
) -> Result<ModeOutcome, Box<dyn Error>> {
    if frames.is_empty() {
        return Err("reference frames must not be empty".into());
    }
    if frames.len() != usize::from(frames_per_segment) {
        return Err("frames_per_segment does not match supplied frames".into());
    }
    if chroma.len() != frames.len() {
        return Err("chroma frame count does not match supplied frames".into());
    }

    let bundle_width = bundle_width.max(2);
    let config = BaselineEncoderConfig {
        frame_dimensions,
        frames_per_segment,
        frame_duration_ns: 33_333_333,
        quantizer,
        entropy_mode: EntropyMode::RansBundled,
        bundle_width,
        bundle_acceleration: bundle_accel,
        ..BaselineEncoderConfig::default()
    };

    let mut encoder = BaselineEncoder::new(config.clone());
    let mut encoded_segments = Vec::with_capacity(usize::from(segments));
    let encode_start = Instant::now();
    for idx in 0..segments {
        encoded_segments.push(encoder.encode_segment_with_chroma(
            idx as u64,
            1_000 * u64::from(idx),
            7,
            frames,
            Some(chroma),
            None,
        )?);
    }
    let encode_ms = encode_start.elapsed().as_secs_f64() * 1_000.0;

    let chunk_bytes: usize = encoded_segments
        .iter()
        .map(|segment| {
            segment
                .chunks
                .iter()
                .map(|chunk| chunk.len())
                .sum::<usize>()
        })
        .sum();

    let decoder = BaselineDecoder::new(frame_dimensions, config.frame_duration_ns);
    let decode_start = Instant::now();
    let mut decoded_frames: Vec<RawFrame> =
        Vec::with_capacity(encoded_segments.len() * frames.len());
    let mut decoded_chroma: Vec<Chroma420Frame> =
        Vec::with_capacity(encoded_segments.len() * frames.len());
    for segment in &encoded_segments {
        let decoded = decoder.decode_segment(segment)?;
        for frame in decoded {
            decoded_frames.push(RawFrame::new(frame_dimensions, frame.luma)?);
            let chroma_frame = frame
                .chroma
                .unwrap_or_else(|| Chroma420Frame::neutral(frame_dimensions));
            decoded_chroma.push(chroma_frame);
        }
    }
    let decode_ms = decode_start.elapsed().as_secs_f64() * 1_000.0;
    let psnr_y = Some(compute_psnr_y(
        frames,
        segments,
        &decoded_frames,
        frame_dimensions,
    )?);
    let psnr_yuv = match psnr_mode {
        PsnrMode::Luma => None,
        PsnrMode::Yuv => Some(compute_psnr_yuv(
            frames,
            chroma,
            segments,
            &decoded_frames,
            &decoded_chroma,
            frame_dimensions,
        )?),
    };

    Ok(ModeOutcome {
        bench: ModeBench {
            entropy_mode: EntropyMode::RansBundled,
            bundle_width: config.bundle_width,
            bundle_accel,
            encode_ms,
            decode_ms,
            chunk_bytes,
            psnr_y,
            psnr_yuv,
            quantizer,
            stream_bitrate_mbps: stream_bitrate_mbps(
                chunk_bytes,
                frames_per_segment,
                segments,
                config.frame_duration_ns,
            ),
        },
        segments: encoded_segments,
        decoded_frames,
        decoded_chroma,
        frame_duration_ns: config.frame_duration_ns,
    })
}

struct ParsedY4m {
    dimensions: FrameDimensions,
    frames: Vec<RawFrame>,
    chroma: Vec<Chroma420Frame>,
}

fn parse_y4m_frames(path: &std::path::Path) -> Result<ParsedY4m, Box<dyn Error>> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut header = String::new();
    reader.read_line(&mut header)?;
    if !header.starts_with("YUV4MPEG2") {
        return Err("input is not a Y4M file".into());
    }
    let mut width = None;
    let mut height = None;
    let mut chroma = None;
    for token in header.split_ascii_whitespace() {
        if let Some(rest) = token.strip_prefix('W') {
            width = rest.parse::<u16>().ok();
        } else if let Some(rest) = token.strip_prefix('H') {
            height = rest.parse::<u16>().ok();
        } else if let Some(rest) = token.strip_prefix('C') {
            chroma = Some(rest.to_ascii_lowercase());
        }
    }
    let width = width.ok_or("missing W in Y4M header")?;
    let height = height.ok_or("missing H in Y4M header")?;
    let chroma = chroma.unwrap_or_else(|| "420".to_string());
    if !chroma.starts_with("420") {
        return Err(format!("unsupported chroma '{chroma}', expected 420").into());
    }
    let frame_dimensions = FrameDimensions::new(width, height);
    let luma_len = usize::from(width) * usize::from(height);
    let chroma_plane_len = luma_len / 4;
    let frame_len = luma_len + chroma_plane_len * 2;

    let mut frames = Vec::new();
    let mut chroma_frames = Vec::new();
    loop {
        let mut frame_tag = String::new();
        if reader.read_line(&mut frame_tag)? == 0 {
            break;
        }
        if !frame_tag.starts_with("FRAME") {
            break;
        }
        let mut buf = vec![0u8; frame_len];
        reader.read_exact(&mut buf)?;
        let luma = buf[..luma_len].to_vec();
        let u = buf[luma_len..(luma_len + chroma_plane_len)].to_vec();
        let v = buf[(luma_len + chroma_plane_len)..].to_vec();
        frames.push(RawFrame::new(frame_dimensions, luma)?);
        chroma_frames.push(Chroma420Frame::new(frame_dimensions, u, v)?);
    }
    Ok(ParsedY4m {
        dimensions: frame_dimensions,
        frames,
        chroma: chroma_frames,
    })
}

fn compute_psnr_y(
    reference: &[RawFrame],
    segments: u16,
    decoded: &[RawFrame],
    dimensions: FrameDimensions,
) -> Result<f64, Box<dyn Error>> {
    let expected_frames = reference.len() * usize::from(segments);
    if decoded.len() != expected_frames {
        return Err(format!(
            "decoded frame count {} does not match expected {expected_frames}",
            decoded.len()
        )
        .into());
    }
    let pixels = dimensions.pixel_count();
    if pixels == 0 {
        return Err("frame dimensions yield zero pixels".into());
    }
    let mut total_samples: usize = 0;
    let mut sum_sq_error = 0f64;
    for seg_idx in 0..usize::from(segments) {
        for (frame_idx, reference_frame) in reference.iter().enumerate() {
            let decoded_idx = seg_idx * reference.len() + frame_idx;
            let decoded_frame = &decoded[decoded_idx];
            if reference_frame.luma.len() != pixels || decoded_frame.luma.len() != pixels {
                return Err(format!(
                    "frame {decoded_idx} length mismatch (ref {}, decoded {})",
                    reference_frame.luma.len(),
                    decoded_frame.luma.len()
                )
                .into());
            }
            for (ref_px, dec_px) in reference_frame.luma.iter().zip(decoded_frame.luma.iter()) {
                let diff = f64::from(*ref_px) - f64::from(*dec_px);
                sum_sq_error += diff * diff;
            }
            total_samples += pixels;
        }
    }
    if total_samples == 0 {
        return Err("no samples available for PSNR computation".into());
    }
    if sum_sq_error == 0.0 {
        return Ok(99.0);
    }
    let mse = sum_sq_error / (total_samples as f64);
    Ok(10.0 * ((255.0_f64 * 255.0_f64) / mse).log10())
}

fn compute_psnr_yuv(
    reference: &[RawFrame],
    reference_chroma: &[Chroma420Frame],
    segments: u16,
    decoded: &[RawFrame],
    decoded_chroma: &[Chroma420Frame],
    dimensions: FrameDimensions,
) -> Result<f64, Box<dyn Error>> {
    let expected_frames = reference.len() * usize::from(segments);
    if decoded.len() != expected_frames {
        return Err(format!(
            "decoded frame count {} does not match expected {expected_frames}",
            decoded.len()
        )
        .into());
    }
    if reference_chroma.len() != reference.len() {
        return Err(format!(
            "reference chroma count {} does not match reference frames {}",
            reference_chroma.len(),
            reference.len()
        )
        .into());
    }
    if decoded_chroma.len() != expected_frames {
        return Err(format!(
            "decoded chroma count {} does not match expected {expected_frames}",
            decoded_chroma.len()
        )
        .into());
    }
    let luma_samples = dimensions.pixel_count();
    let chroma_samples =
        usize::from(dimensions.width / 2).saturating_mul(usize::from(dimensions.height / 2));
    if luma_samples == 0 || chroma_samples == 0 {
        return Err("frame dimensions yield zero samples".into());
    }
    let mut total_samples: usize = 0;
    let mut sum_sq_error = 0f64;
    for seg_idx in 0..usize::from(segments) {
        for (frame_idx, reference_frame) in reference.iter().enumerate() {
            let decoded_idx = seg_idx * reference.len() + frame_idx;
            let decoded_frame = &decoded[decoded_idx];
            let ref_chroma = &reference_chroma[frame_idx];
            let dec_chroma = &decoded_chroma[decoded_idx];
            if reference_frame.luma.len() != luma_samples
                || decoded_frame.luma.len() != luma_samples
            {
                return Err(format!(
                    "frame {decoded_idx} length mismatch (ref {}, decoded {})",
                    reference_frame.luma.len(),
                    decoded_frame.luma.len()
                )
                .into());
            }
            if ref_chroma.u.len() != chroma_samples
                || ref_chroma.v.len() != chroma_samples
                || dec_chroma.u.len() != chroma_samples
                || dec_chroma.v.len() != chroma_samples
            {
                return Err(format!(
                    "frame {decoded_idx} chroma length mismatch (ref U/V {}/{} decoded U/V {}/{})",
                    ref_chroma.u.len(),
                    ref_chroma.v.len(),
                    dec_chroma.u.len(),
                    dec_chroma.v.len(),
                )
                .into());
            }
            for (ref_px, dec_px) in reference_frame.luma.iter().zip(decoded_frame.luma.iter()) {
                let diff = f64::from(*ref_px) - f64::from(*dec_px);
                sum_sq_error += diff * diff;
            }
            for (ref_px, dec_px) in ref_chroma.u.iter().zip(dec_chroma.u.iter()) {
                let diff = f64::from(*ref_px) - f64::from(*dec_px);
                sum_sq_error += diff * diff;
            }
            for (ref_px, dec_px) in ref_chroma.v.iter().zip(dec_chroma.v.iter()) {
                let diff = f64::from(*ref_px) - f64::from(*dec_px);
                sum_sq_error += diff * diff;
            }
            total_samples += luma_samples + chroma_samples * 2;
        }
    }
    if total_samples == 0 {
        return Err("no samples available for PSNR computation".into());
    }
    if sum_sq_error == 0.0 {
        return Ok(99.0);
    }
    let mse = sum_sq_error / (total_samples as f64);
    Ok(10.0 * ((255.0_f64 * 255.0_f64) / mse).log10())
}

fn stream_bitrate_mbps(
    chunk_bytes: usize,
    frames_per_segment: u16,
    segments: u16,
    frame_duration_ns: u32,
) -> f64 {
    let total_frames = usize::from(frames_per_segment) * usize::from(segments);
    let total_ns = u128::from(frame_duration_ns) * u128::try_from(total_frames.max(1)).unwrap_or(0);
    if total_ns == 0 {
        return 0.0;
    }
    let total_seconds = total_ns as f64 / 1_000_000_000.0;
    if total_seconds == 0.0 {
        return 0.0;
    }
    ((chunk_bytes as f64) * 8.0) / total_seconds / 1_000_000.0
}

fn attempt_entry(
    quantizer: u8,
    bench: &ModeBench,
    frames_per_segment: u16,
    segments: u16,
) -> Value {
    let mut map = json::Map::new();
    map.insert("quantizer".into(), Value::from(quantizer));
    map.insert(
        "stream_bitrate_mbps".into(),
        Value::from(bench.stream_bitrate_mbps),
    );
    map.insert(
        "frames".into(),
        Value::from(u64::from(frames_per_segment) * u64::from(segments)),
    );
    Value::Object(map)
}

fn select_primary_run(runs: &[QuantizerRun], target_bitrate: Option<f64>) -> usize {
    if runs.is_empty() {
        return 0;
    }
    if let Some(target) = target_bitrate {
        runs.iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let delta_a = (a.outcome.bench.stream_bitrate_mbps - target).abs();
                let delta_b = (b.outcome.bench.stream_bitrate_mbps - target).abs();
                delta_a
                    .partial_cmp(&delta_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    } else {
        0
    }
}

fn bitrate_entry(quantizer: u8, bench: &ModeBench, target: f64) -> Value {
    let mut map = json::Map::new();
    map.insert("quantizer".into(), Value::from(quantizer));
    map.insert(
        "bitrate_mbps".into(),
        Value::from(bench.stream_bitrate_mbps),
    );
    map.insert(
        "delta_mbps".into(),
        Value::from((bench.stream_bitrate_mbps - target).abs()),
    );
    map.insert("chunk_bytes".into(), Value::from(bench.chunk_bytes as u64));
    if let Some(psnr) = bench.psnr_y {
        map.insert("psnr_y".into(), Value::from(psnr));
    }
    if let Some(psnr) = bench.psnr_yuv {
        map.insert("psnr_yuv".into(), Value::from(psnr));
    }
    map.insert(
        "entropy_mode".into(),
        Value::from(format!("{:?}", bench.entropy_mode).to_lowercase()),
    );
    Value::Object(map)
}

fn best_entry_for_target(runs: &[QuantizerRun], target: f64) -> Option<Value> {
    runs.iter()
        .map(|run| (run.quantizer, &run.outcome.bench))
        .min_by(|(_, a), (_, b)| {
            let delta_a = (a.stream_bitrate_mbps - target).abs();
            let delta_b = (b.stream_bitrate_mbps - target).abs();
            delta_a
                .partial_cmp(&delta_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(quantizer, bench)| bitrate_entry(quantizer, bench, target))
}

fn build_bitrate_targets(runs: &[QuantizerRun], targets: &[f64]) -> Vec<Value> {
    targets
        .iter()
        .copied()
        .map(|target| {
            let mut map = json::Map::new();
            map.insert("target_mbps".into(), Value::from(target));
            map.insert(
                "bench".into(),
                best_entry_for_target(runs, target).unwrap_or(Value::Null),
            );
            Value::Object(map)
        })
        .collect()
}

fn write_y4m_frames(
    path: &std::path::Path,
    dimensions: FrameDimensions,
    frames: &[RawFrame],
    chroma: Option<&[Chroma420Frame]>,
) -> Result<(), Box<dyn Error>> {
    if let Some(chroma) = chroma
        && chroma.len() != frames.len()
    {
        return Err(format!(
            "chroma frame count {} does not match luma {}",
            chroma.len(),
            frames.len()
        )
        .into());
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = std::fs::File::create(path)?;
    writeln!(
        out,
        "YUV4MPEG2 W{} H{} F30:1 Ip A0:0 C420mpeg2",
        dimensions.width, dimensions.height
    )?;
    let default_chroma = Chroma420Frame::neutral(dimensions);
    for (idx, frame) in frames.iter().enumerate() {
        let chroma_planes = chroma
            .and_then(|planes| planes.get(idx))
            .unwrap_or(&default_chroma);
        out.write_all(b"FRAME\n")?;
        out.write_all(&frame.luma)?;
        out.write_all(&chroma_planes.u)?;
        out.write_all(&chroma_planes.v)?;
    }
    Ok(())
}

fn write_segment_bundles(
    path: &std::path::Path,
    segments: &[EncodedSegment],
    dimensions: FrameDimensions,
    frame_duration_ns: u32,
    chroma: Option<&[Chroma420Frame]>,
    frames_per_segment: u16,
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    if let Some(chroma_frames) = chroma {
        let expected = usize::from(frames_per_segment) * segments.len();
        if chroma_frames.len() != expected {
            return Err(format!(
                "chroma frames ({}) do not match expected count ({expected})",
                chroma_frames.len()
            )
            .into());
        }
    }
    let mut chroma_chunks = chroma.map(|planes| planes.chunks(usize::from(frames_per_segment)));
    let bundles: Vec<SegmentBundle> = segments
        .iter()
        .map(|segment| {
            let chroma_slice = chroma_chunks
                .as_mut()
                .and_then(|iter| iter.next())
                .unwrap_or(&[]);
            segment.to_bundle_with_chroma(dimensions, frame_duration_ns, chroma_slice.to_vec())
        })
        .collect();
    let (payload, flags) = encode_with_header_flags(&bundles);
    let checksum = crc64_fallback(&payload);
    let header = Header::new(
        <Vec<SegmentBundle> as NoritoSerialize>::schema_hash(),
        payload.len() as u64,
        checksum,
    );
    let mut header_bytes = Vec::with_capacity(Header::SIZE);
    let header = Header {
        flags: header.flags | flags,
        ..header
    };
    header_bytes.extend_from_slice(&header.magic);
    header_bytes.push(header.major);
    header_bytes.push(header.minor);
    header_bytes.extend_from_slice(&header.schema);
    header_bytes.push(header.compression as u8);
    header_bytes.extend_from_slice(&header.length.to_le_bytes());
    header_bytes.extend_from_slice(&header.checksum.to_le_bytes());
    header_bytes.push(header.flags);
    let align = std::mem::align_of::<Archived<Vec<SegmentBundle>>>();
    let padding = if align <= 1 {
        0
    } else {
        let remainder = Header::SIZE % align;
        if remainder == 0 { 0 } else { align - remainder }
    };
    let mut bytes = Vec::with_capacity(Header::SIZE + padding + payload.len());
    bytes.extend_from_slice(&header_bytes);
    if padding != 0 {
        bytes.resize(bytes.len() + padding, 0);
    }
    bytes.extend_from_slice(&payload);
    decode_from_bytes::<Vec<SegmentBundle>>(&bytes)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Options driving `streaming-decode` invocations.
#[derive(Clone, Debug)]
pub struct DecodeOptions {
    /// Path to the serialized segment bundle (`Vec<SegmentBundle>`).
    pub bundle_path: std::path::PathBuf,
    /// Destination for the reconstructed Y4M clip.
    pub y4m_out: std::path::PathBuf,
    /// Optional reference Y4M to compute PSNR.
    pub reference_y4m: Option<std::path::PathBuf>,
    /// Mode controlling PSNR computation.
    pub psnr_mode: PsnrMode,
}

/// Decode a serialized bundle into a Y4M clip (and optional PSNR summary).
pub fn run_streaming_decode(options: DecodeOptions) -> Result<Value, Box<dyn Error>> {
    let bytes = std::fs::read(&options.bundle_path)?;
    let bundles: Vec<SegmentBundle> = decode_from_bytes(&bytes)?;
    if bundles.is_empty() {
        return Err("bundle contained no segments".into());
    }
    let dims = bundles[0].frame_dimensions;
    let frame_duration_ns = bundles[0].frame_duration_ns;
    let decoder = BaselineDecoder::new(dims, frame_duration_ns);
    let mut frames = Vec::new();
    let mut chroma_frames = Vec::new();
    for (idx, bundle) in bundles.into_iter().enumerate() {
        let (segment, bundle_dims, bundle_duration, bundle_chroma) =
            bundle.into_segment_with_chroma()?;
        if bundle_dims != dims || bundle_duration != frame_duration_ns {
            return Err(format!(
                "bundle {idx} dimensions/duration mismatch (expected {dims:?}/{frame_duration_ns})"
            )
            .into());
        }
        let decoded = decoder.decode_segment(&segment)?;
        if !bundle_chroma.is_empty() && bundle_chroma.len() != decoded.len() {
            return Err(format!(
                "bundle {idx} chroma frames ({}) do not match decoded frame count ({})",
                bundle_chroma.len(),
                decoded.len()
            )
            .into());
        }
        for (frame_idx, frame) in decoded.into_iter().enumerate() {
            frames.push(RawFrame::new(dims, frame.luma)?);
            let chroma_frame = match frame.chroma {
                Some(chroma) => chroma,
                None => bundle_chroma
                    .get(frame_idx)
                    .cloned()
                    .unwrap_or_else(|| Chroma420Frame::neutral(dims)),
            };
            chroma_frames.push(chroma_frame);
        }
    }

    write_y4m_frames(&options.y4m_out, dims, &frames, Some(&chroma_frames))?;

    let mut root = json::Map::new();
    root.insert(
        "input".into(),
        Value::from(options.bundle_path.display().to_string()),
    );
    root.insert(
        "y4m_out".into(),
        Value::from(options.y4m_out.display().to_string()),
    );
    root.insert("frames".into(), Value::from(frames.len() as u64));
    root.insert("width".into(), Value::from(dims.width));
    root.insert("height".into(), Value::from(dims.height));
    root.insert("frame_duration_ns".into(), Value::from(frame_duration_ns));
    root.insert("psnr_mode".into(), Value::from(options.psnr_mode.as_str()));

    if let Some(reference) = options.reference_y4m.as_ref() {
        let parsed = parse_y4m_frames(reference)?;
        let psnr = compute_psnr_y(&parsed.frames, 1, &frames, dims)?;
        root.insert("psnr_y".into(), Value::from(psnr));
        if options.psnr_mode == PsnrMode::Yuv {
            let psnr_yuv = compute_psnr_yuv(
                &parsed.frames,
                &parsed.chroma,
                1,
                &frames,
                &chroma_frames,
                dims,
            )?;
            root.insert("psnr_yuv".into(), Value::from(psnr_yuv));
        }
        root.insert(
            "reference_y4m".into(),
            Value::from(reference.display().to_string()),
        );
    }

    let report = Value::Object(root);
    Ok(report)
}

fn build_frames(
    dimensions: FrameDimensions,
    count: usize,
    rng: &mut ChaCha20Rng,
) -> Result<Vec<RawFrame>, Box<dyn Error>> {
    let mut frames = Vec::with_capacity(count);
    for _ in 0..count {
        let mut luma = vec![0u8; dimensions.pixel_count()];
        rng.fill_bytes(&mut luma);
        frames.push(RawFrame::new(dimensions, luma)?);
    }
    Ok(frames)
}

fn build_chroma_frames(
    dimensions: FrameDimensions,
    count: usize,
    rng: &mut ChaCha20Rng,
) -> Result<Vec<Chroma420Frame>, Box<dyn Error>> {
    let chroma_len =
        usize::from(dimensions.width / 2).saturating_mul(usize::from(dimensions.height / 2));
    let mut frames = Vec::with_capacity(count);
    for _ in 0..count {
        let mut u = vec![0u8; chroma_len];
        let mut v = vec![0u8; chroma_len];
        rng.fill_bytes(&mut u);
        rng.fill_bytes(&mut v);
        frames.push(Chroma420Frame::new(dimensions, u, v)?);
    }
    Ok(frames)
}
