//! Offline command surfaces.

use std::{
    fs,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use clap::{Args, Subcommand, ValueEnum};
use eyre::{Result, WrapErr, eyre};
use iroha::data_model::petal_stream::{
    PETAL_CAPTURE_DEFAULT_MIN_SUCCESS_RATIO_BPS, PETAL_CAPTURE_RATIO_BPS_SCALE,
    PetalStreamCaptureProfile, PetalStreamDecoder, PetalStreamEncoder, PetalStreamGrid,
    PetalStreamOptions, PetalStreamSampleGrid, render_petal_capture_samples_with_seed,
    score_petal_capture_profile_with_seed,
};
use norito::derive::JsonSerialize;

use crate::{
    Run, RunContext,
    cli_output::print_with_optional_text,
};

const SCORE_STYLES_SCHEMA: &str = "iroha.offline.petal.score_styles.v1";
const ENCODE_SCHEMA: &str = "iroha.offline.petal.encode.v1";
const EVAL_CAPTURE_SCHEMA: &str = "iroha.offline.petal.eval_capture.v1";
const SIMULATE_REALTIME_SCHEMA: &str = "iroha.offline.petal.simulate_realtime.v1";
const DEFAULT_SCORE_STYLES_FPS: u16 = 24;
const DEFAULT_TARGET_EFFECTIVE_BPS: u64 = 3_000;
const DEFAULT_ENCODE_DIMENSION: u32 = 1_024;
const MAX_ENCODE_DIMENSION: u32 = 4_096;
const DEFAULT_STYLE_NAME: &str = "sora-temple";

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Petal Stream optical handoff tooling.
    #[command(subcommand)]
    Petal(PetalCommand),
}

impl Command {
    pub(crate) fn allows_fallback_config(&self) -> bool {
        true
    }
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Petal(command) => Run::run(command, context),
        }
    }
}

#[derive(Subcommand, Debug)]
pub(crate) enum PetalCommand {
    /// Encode one payload into a deterministic Petal PNG and manifest.
    Encode(EncodeArgs),
    /// Evaluate rendered Petal PNG frames through the deterministic decoder.
    EvalCapture(EvalCaptureArgs),
    /// Replay rendered Petal PNG frames in deterministic realtime loop order.
    SimulateRealtime(SimulateRealtimeArgs),
    /// Score the published Petal style set against deterministic capture gates.
    ScoreStyles(ScoreStylesArgs),
}

impl Run for PetalCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Encode(args) => args.run(context),
            Self::EvalCapture(args) => args.run(context),
            Self::SimulateRealtime(args) => args.run(context),
            Self::ScoreStyles(args) => args.run(context),
        }
    }
}

#[derive(Args, Debug, Clone)]
pub(crate) struct EncodeArgs {
    /// Payload bytes to encode into the Petal Stream grid.
    #[arg(long, value_name = "PATH")]
    input: PathBuf,
    /// Output directory for rendered frames and manifest.
    #[arg(long, value_name = "DIR")]
    output: PathBuf,
    /// Output format. PNG is implemented for the core deterministic grid.
    #[arg(long, value_enum, default_value_t = PetalEncodeFormatArg::Png)]
    format: PetalEncodeFormatArg,
    /// Renderer style. Current PNG output uses the decode-critical grid layer.
    #[arg(long, value_enum, default_value_t = PetalEncodeStyleArg::SoraTemple)]
    style: PetalEncodeStyleArg,
    /// Visual channel. Current PNG output supports only the binary Petal grid.
    #[arg(long, value_enum, default_value_t = PetalEncodeChannelArg::BinaryGrid)]
    channel: PetalEncodeChannelArg,
    /// Square output dimension in pixels.
    #[arg(long, default_value_t = DEFAULT_ENCODE_DIMENSION)]
    dimension: u32,
    /// Frames per second metadata for downstream animation tooling.
    #[arg(long, default_value_t = DEFAULT_SCORE_STYLES_FPS)]
    fps: u16,
    /// Override the Petal grid size. Zero keeps automatic sizing.
    #[arg(long = "grid-size", default_value_t = 0)]
    grid_size: u16,
    /// Override the Petal grid border thickness.
    #[arg(long, default_value_t = iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_BORDER)]
    border: u8,
    /// Override the Petal grid anchor size.
    #[arg(long = "anchor-size", default_value_t = iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_ANCHOR)]
    anchor_size: u8,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct EvalCaptureArgs {
    /// Directory containing rendered PNG frames, or an encode output directory with manifest.json.
    #[arg(long = "input-dir", value_name = "DIR")]
    input_dir: PathBuf,
    /// Visual channel. Current evaluation supports only the binary Petal grid.
    #[arg(long, value_enum, default_value_t = PetalEncodeChannelArg::BinaryGrid)]
    channel: PetalEncodeChannelArg,
    /// Deterministic capture profile label recorded in the report.
    #[arg(long, value_enum, default_value_t = PetalCaptureProfileArg::Default)]
    profile: PetalCaptureProfileArg,
    /// Deterministic capture perturbation controls.
    #[command(flatten)]
    capture: CapturePerturbationArgs,
    /// Minimum success ratio as a decimal in [0, 1], e.g. 0.95.
    #[arg(long = "min-success-ratio", value_name = "RATIO")]
    min_success_ratio: Option<String>,
    /// Minimum success ratio in basis points.
    #[arg(long = "min-success-ratio-bps")]
    min_success_ratio_bps: Option<u16>,
    /// Optional JSON report path. The full report is printed to stdout when omitted.
    #[arg(long = "output-report", value_name = "PATH")]
    output_report: Option<PathBuf>,
    /// Petal grid size for manifest-free directories. Zero requires a manifest.
    #[arg(long = "grid-size", default_value_t = 0)]
    grid_size: u16,
    /// Petal grid border thickness for manifest-free directories.
    #[arg(long, default_value_t = iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_BORDER)]
    border: u8,
    /// Petal grid anchor size for manifest-free directories.
    #[arg(long = "anchor-size", default_value_t = iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_ANCHOR)]
    anchor_size: u8,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct SimulateRealtimeArgs {
    /// Directory containing rendered PNG frames, or an encode output directory with manifest.json.
    #[arg(long = "input-dir", value_name = "DIR")]
    input_dir: PathBuf,
    /// Visual channel. Current realtime simulation supports only the binary Petal grid.
    #[arg(long, value_enum, default_value_t = PetalEncodeChannelArg::BinaryGrid)]
    channel: PetalEncodeChannelArg,
    /// Deterministic capture profile label recorded in the report.
    #[arg(long, value_enum, default_value_t = PetalCaptureProfileArg::Default)]
    profile: PetalCaptureProfileArg,
    /// Deterministic capture perturbation controls.
    #[command(flatten)]
    capture: CapturePerturbationArgs,
    /// Replayed frame rate metadata.
    #[arg(long = "simulate-fps", default_value_t = DEFAULT_SCORE_STYLES_FPS)]
    simulate_fps: u16,
    /// Number of deterministic playback loops.
    #[arg(long = "realtime-loops", default_value_t = 1)]
    realtime_loops: u16,
    /// Optional path for the first successfully decoded payload.
    #[arg(long = "output-payload", value_name = "PATH")]
    output_payload: Option<PathBuf>,
    /// Optional JSON report path. The full report is printed to stdout when omitted.
    #[arg(long = "output-report", value_name = "PATH")]
    output_report: Option<PathBuf>,
    /// Petal grid size for manifest-free directories. Zero requires a manifest.
    #[arg(long = "grid-size", default_value_t = 0)]
    grid_size: u16,
    /// Petal grid border thickness for manifest-free directories.
    #[arg(long, default_value_t = iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_BORDER)]
    border: u8,
    /// Petal grid anchor size for manifest-free directories.
    #[arg(long = "anchor-size", default_value_t = iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_ANCHOR)]
    anchor_size: u8,
}

#[derive(Args, Debug, Clone)]
pub(crate) struct ScoreStylesArgs {
    /// Payload bytes to encode into the Petal Stream grid.
    #[arg(long, value_name = "PATH")]
    input: PathBuf,
    /// Optional JSON report path. The full report is printed to stdout when omitted.
    #[arg(long = "output-report", value_name = "PATH")]
    output_report: Option<PathBuf>,
    /// Published style set to score.
    #[arg(long = "style-set", value_enum, default_value_t = PetalStyleSetArg::SoraTempleDefault)]
    style_set: PetalStyleSetArg,
    /// Deterministic capture profile to apply.
    #[arg(long, value_enum, default_value_t = PetalCaptureProfileArg::Default)]
    profile: PetalCaptureProfileArg,
    /// Deterministic seed mixed into luminance jitter.
    #[arg(long, default_value_t = 0)]
    seed: u64,
    /// Frames per second used for the effective-payload rate estimate.
    #[arg(long, default_value_t = DEFAULT_SCORE_STYLES_FPS)]
    fps: u16,
    /// Target effective payload bits per second for throughput scoring.
    #[arg(long = "target-effective-bps", default_value_t = DEFAULT_TARGET_EFFECTIVE_BPS)]
    target_effective_bps: u64,
    /// Minimum capture success ratio, in basis points.
    #[arg(long = "min-success-ratio-bps", default_value_t = PETAL_CAPTURE_DEFAULT_MIN_SUCCESS_RATIO_BPS)]
    min_success_ratio_bps: u16,
    /// Override the Petal grid size. Zero keeps automatic sizing.
    #[arg(long = "grid-size", default_value_t = 0)]
    grid_size: u16,
    /// Override the Petal grid border thickness.
    #[arg(long, default_value_t = iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_BORDER)]
    border: u8,
    /// Override the Petal grid anchor size.
    #[arg(long = "anchor-size", default_value_t = iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_ANCHOR)]
    anchor_size: u8,
    /// Override capture attempt count.
    #[arg(long)]
    attempts: Option<u16>,
    /// Override dark-cell luminance.
    #[arg(long = "dark-luma")]
    dark_luma: Option<u8>,
    /// Override light-cell luminance.
    #[arg(long = "light-luma")]
    light_luma: Option<u8>,
    /// Override deterministic luminance jitter.
    #[arg(long = "luminance-jitter")]
    luminance_jitter: Option<u8>,
}

#[derive(Args, Debug, Clone, Default)]
struct CapturePerturbationArgs {
    /// Apply deterministic profile luminance perturbation before decoding.
    #[arg(long = "perturb-capture", default_value_t = false)]
    perturb_capture: bool,
    /// Deterministic seed mixed into capture perturbation.
    #[arg(long = "capture-seed", default_value_t = 0)]
    seed: u64,
    /// Override perturbed capture attempt count per rendered source frame.
    #[arg(long = "capture-attempts")]
    attempts: Option<u16>,
    /// Override perturbed dark-cell luminance.
    #[arg(long = "capture-dark-luma")]
    dark_luma: Option<u8>,
    /// Override perturbed light-cell luminance.
    #[arg(long = "capture-light-luma")]
    light_luma: Option<u8>,
    /// Override perturbed deterministic luminance jitter.
    #[arg(long = "capture-luminance-jitter")]
    luminance_jitter: Option<u8>,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PetalEncodeFormatArg {
    /// Single-frame PNG output.
    Png,
    /// Animated GIF output is reserved for the renderer-backed path.
    Gif,
}

impl std::fmt::Display for PetalEncodeFormatArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Png => f.write_str("png"),
            Self::Gif => f.write_str("gif"),
        }
    }
}

impl PetalEncodeFormatArg {
    fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Gif => "gif",
        }
    }

    fn validate_available(self) -> Result<()> {
        match self {
            Self::Png => Ok(()),
            Self::Gif => validate_gif_available(),
        }
    }
}

#[cfg(feature = "offline-visual-codecs")]
fn validate_gif_available() -> Result<()> {
    Ok(())
}

#[cfg(not(feature = "offline-visual-codecs"))]
fn validate_gif_available() -> Result<()> {
    Err(eyre!(
        "--format gif requires building iroha_cli with --features offline-visual-codecs"
    ))
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PetalEncodeStyleArg {
    /// Decode-critical SORA temple grid layer.
    SoraTemple,
    /// Katakana command styling is reserved for the renderer-backed path.
    SoraTempleCommand,
}

impl std::fmt::Display for PetalEncodeStyleArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SoraTemple => f.write_str("sora-temple"),
            Self::SoraTempleCommand => f.write_str("sora-temple-command"),
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PetalEncodeChannelArg {
    /// Binary luminance grid suitable for scanner bring-up.
    BinaryGrid,
    /// Katakana base94 rendering is reserved for the renderer-backed path.
    KatakanaBase94,
}

impl std::fmt::Display for PetalEncodeChannelArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BinaryGrid => f.write_str("binary-grid"),
            Self::KatakanaBase94 => f.write_str("katakana-base94"),
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PetalStyleSetArg {
    /// Current production Petal style family.
    SoraTempleDefault,
}

impl std::fmt::Display for PetalStyleSetArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SoraTempleDefault => f.write_str("sora-temple-default"),
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PetalCaptureProfileArg {
    /// Default production deterministic capture profile.
    Default,
}

impl std::fmt::Display for PetalCaptureProfileArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => f.write_str("default"),
        }
    }
}

impl EncodeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let report = self.encode()?;
        let text = format!(
            "wrote Petal {} frame to {} (resolved_grid_size={} payload_bytes={})",
            report.format,
            report.frames[0].path,
            report.grid.resolved_grid_size,
            report.payload_bytes
        );
        print_with_optional_text(context, Some(text), &report)
    }

    fn encode(&self) -> Result<EncodeReport> {
        self.validate()?;
        let payload = fs::read(&self.input)
            .wrap_err_with(|| format!("failed to read input payload {}", self.input.display()))?;
        let options = PetalStreamOptions {
            grid_size: self.grid_size,
            border: self.border,
            anchor_size: self.anchor_size,
        };
        let grid = PetalStreamEncoder::encode_grid(&payload, options)
            .map_err(|err| eyre!("failed to encode Petal grid: {err}"))?;
        if self.dimension < u32::from(grid.grid_size) {
            return Err(eyre!(
                "--dimension {} is smaller than resolved grid size {}",
                self.dimension,
                grid.grid_size
            ));
        }

        fs::create_dir_all(&self.output)
            .wrap_err_with(|| format!("failed to create {}", self.output.display()))?;
        let format_dir = self.output.join(self.format.to_string());
        fs::create_dir_all(&format_dir)
            .wrap_err_with(|| format!("failed to create {}", format_dir.display()))?;
        let frame_path = format_dir.join(format!("frame_0000.{}", self.format.extension()));
        write_grid_image(&frame_path, &grid, self.dimension, self.format, self.fps)?;

        let report = EncodeReport {
            schema: ENCODE_SCHEMA.to_string(),
            input_path: self.input.display().to_string(),
            output_dir: self.output.display().to_string(),
            payload_bytes: payload.len() as u64,
            format: self.format.to_string(),
            style: self.style.to_string(),
            channel: self.channel.to_string(),
            fps: self.fps,
            dimension: self.dimension,
            grid: GridReport {
                requested_grid_size: self.grid_size,
                resolved_grid_size: grid.grid_size,
                border: self.border,
                anchor_size: self.anchor_size,
            },
            frames: vec![EncodeFrameReport {
                index: 0,
                path: frame_path.display().to_string(),
                payload_bytes: payload.len() as u64,
            }],
        };
        let manifest_path = self.output.join("manifest.json");
        write_encode_manifest(&manifest_path, &report)?;
        Ok(report)
    }

    fn validate(&self) -> Result<()> {
        self.format.validate_available()?;
        if self.style != PetalEncodeStyleArg::SoraTemple {
            return Err(eyre!(
                "--style {} is planned for renderer-backed output; use --style sora-temple",
                self.style
            ));
        }
        if self.channel != PetalEncodeChannelArg::BinaryGrid {
            return Err(eyre!(
                "--channel {} is planned for renderer-backed output; use --channel binary-grid",
                self.channel
            ));
        }
        if self.dimension == 0 {
            return Err(eyre!("--dimension must be greater than 0"));
        }
        if self.dimension > MAX_ENCODE_DIMENSION {
            return Err(eyre!(
                "--dimension must be <= {} for bounded offline rendering",
                MAX_ENCODE_DIMENSION
            ));
        }
        if self.fps == 0 {
            return Err(eyre!("--fps must be greater than 0"));
        }
        Ok(())
    }
}

impl EvalCaptureArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let report = self.evaluate()?;
        if let Some(path) = &self.output_report {
            write_eval_capture_report(path, &report)?;
            let summary = EvalCaptureWriteSummary {
                report_path: path.display().to_string(),
                gate_passed: report.gate_passed,
                success_ratio_bps: report.success_ratio_bps,
                attempts: report.attempts,
                planned_attempts: report.planned_attempts,
                aborted_early: report.aborted_early,
            };
            let text = format!(
                "wrote Petal capture report to {} (gate_passed={} success_ratio_bps={} attempts={}/{})",
                summary.report_path,
                summary.gate_passed,
                summary.success_ratio_bps,
                summary.attempts,
                summary.planned_attempts
            );
            print_with_optional_text(context, Some(text), &summary)
        } else {
            let text = format!(
                "gate_passed={} success_ratio_bps={} attempts={}/{} aborted_early={}",
                report.gate_passed,
                report.success_ratio_bps,
                report.attempts,
                report.planned_attempts,
                report.aborted_early
            );
            print_with_optional_text(context, Some(text), &report)
        }
    }

    fn evaluate(&self) -> Result<EvalCaptureReport> {
        self.validate()?;
        let min_success_ratio_bps = self.min_success_ratio_bps()?;
        let input = resolve_eval_capture_input(self)?;
        let capture = self.capture.active(self.profile)?;
        let attempts_per_frame = capture.map_or(1, |active| active.profile.attempts);
        let planned_attempts = planned_capture_attempts(input.frames.len(), attempts_per_frame)?;
        let required_successes = required_successes(planned_attempts, min_success_ratio_bps)?;
        let options = PetalStreamOptions {
            grid_size: input.grid.resolved_grid_size,
            border: input.grid.border,
            anchor_size: input.grid.anchor_size,
        };

        let mut successes = 0u32;
        let mut attempts = 0u32;
        let mut aborted_early = false;
        let mut frames = Vec::with_capacity(planned_attempts as usize);

        for (source_index, path) in input.frames.iter().enumerate() {
            for capture_attempt_index in 0..attempts_per_frame {
                attempts = attempts.saturating_add(1);
                let result = evaluate_png_frame(
                    path,
                    options,
                    input.payload_bytes,
                    capture,
                    capture_attempt_index,
                );
                let (success, decoded_payload_bytes, error) = match result {
                    Ok(decoded_len) => {
                        successes = successes.saturating_add(1);
                        (true, Some(decoded_len as u64), None)
                    }
                    Err(err) => (false, None, Some(err.to_string())),
                };
                frames.push(EvalCaptureFrameReport {
                    index: attempts.saturating_sub(1),
                    source_index: u16::try_from(source_index).unwrap_or(u16::MAX),
                    capture_attempt_index,
                    path: path.display().to_string(),
                    success,
                    decoded_payload_bytes,
                    error,
                });

                let remaining = planned_attempts.saturating_sub(attempts);
                if remaining > 0 && successes.saturating_add(remaining) < required_successes {
                    aborted_early = true;
                    break;
                }
            }
            if aborted_early {
                break;
            }
        }

        let success_ratio_bps = ratio_bps(successes, planned_attempts);
        let gate_passed = successes >= required_successes;
        Ok(EvalCaptureReport {
            schema: EVAL_CAPTURE_SCHEMA.to_string(),
            input_dir: self.input_dir.display().to_string(),
            manifest_path: input.manifest_path.map(|path| path.display().to_string()),
            channel: self.channel.to_string(),
            profile: self.profile.to_string(),
            perturb_capture: capture.is_some(),
            capture_seed: capture.map(|active| active.seed),
            capture_attempts_per_frame: attempts_per_frame,
            capture_profile: capture.map(|active| CaptureProfileReport::from(active.profile)),
            min_success_ratio_bps,
            planned_attempts,
            attempts,
            successes,
            required_successes,
            success_ratio_bps,
            gate_passed,
            aborted_early,
            grid: input.grid,
            frames,
        })
    }

    fn validate(&self) -> Result<()> {
        if self.channel != PetalEncodeChannelArg::BinaryGrid {
            return Err(eyre!(
                "--channel {} is planned for renderer-backed evaluation; use --channel binary-grid",
                self.channel
            ));
        }
        if self.min_success_ratio.is_some() && self.min_success_ratio_bps.is_some() {
            return Err(eyre!(
                "pass only one of --min-success-ratio or --min-success-ratio-bps"
            ));
        }
        self.capture.validate(self.profile)?;
        Ok(())
    }

    fn min_success_ratio_bps(&self) -> Result<u16> {
        if let Some(raw) = &self.min_success_ratio {
            parse_success_ratio_bps(raw)
        } else if let Some(value) = self.min_success_ratio_bps {
            validate_success_ratio_bps(value)
        } else {
            Ok(PETAL_CAPTURE_DEFAULT_MIN_SUCCESS_RATIO_BPS)
        }
    }
}

impl SimulateRealtimeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let report = self.simulate()?;
        if let Some(path) = &self.output_report {
            write_simulate_realtime_report(path, &report)?;
            let summary = SimulateRealtimeWriteSummary {
                report_path: path.display().to_string(),
                output_payload_path: report.output_payload_path.clone(),
                decoded: report.decoded,
                attempts: report.attempts,
                planned_attempts: report.planned_attempts,
                payload_bytes: report.payload_bytes,
            };
            let text = format!(
                "wrote Petal realtime report to {} (decoded={} attempts={}/{})",
                summary.report_path,
                summary.decoded,
                summary.attempts,
                summary.planned_attempts
            );
            print_with_optional_text(context, Some(text), &summary)
        } else {
            let text = format!(
                "decoded={} attempts={}/{} payload_bytes={}",
                report.decoded,
                report.attempts,
                report.planned_attempts,
                report.payload_bytes.unwrap_or(0)
            );
            print_with_optional_text(context, Some(text), &report)
        }
    }

    fn simulate(&self) -> Result<SimulateRealtimeReport> {
        self.validate()?;
        let input = resolve_eval_capture_input(&EvalCaptureArgs {
            input_dir: self.input_dir.clone(),
            channel: self.channel,
            profile: self.profile,
            capture: self.capture.clone(),
            min_success_ratio: None,
            min_success_ratio_bps: None,
            output_report: None,
            grid_size: self.grid_size,
            border: self.border,
            anchor_size: self.anchor_size,
        })?;
        let options = PetalStreamOptions {
            grid_size: input.grid.resolved_grid_size,
            border: input.grid.border,
            anchor_size: input.grid.anchor_size,
        };
        let capture = self.capture.active(self.profile)?;
        let attempts_per_frame = capture.map_or(1, |active| active.profile.attempts);
        let planned_attempts = planned_realtime_attempts(
            input.frames.len(),
            self.realtime_loops,
            attempts_per_frame,
        )?;
        let mut attempts = 0u32;
        let mut decoded_payload: Option<Vec<u8>> = None;
        let mut first_success_loop_index = None;
        let mut first_success_source_index = None;
        let mut first_success_capture_attempt_index = None;
        let mut frames = Vec::new();

        for loop_index in 0..self.realtime_loops {
            for (source_index, path) in input.frames.iter().enumerate() {
                for capture_attempt_index in 0..attempts_per_frame {
                    attempts = attempts.saturating_add(1);
                    let result = decode_png_frame_payload(
                        path,
                        options,
                        input.payload_bytes,
                        capture,
                        capture_attempt_index,
                    );
                    let (success, decoded_payload_bytes, error) = match result {
                        Ok(payload) => {
                            let len = payload.len() as u64;
                            if decoded_payload.is_none() {
                                first_success_loop_index = Some(loop_index);
                                first_success_source_index =
                                    Some(u16::try_from(source_index).unwrap_or(u16::MAX));
                                first_success_capture_attempt_index = Some(capture_attempt_index);
                                decoded_payload = Some(payload);
                            }
                            (true, Some(len), None)
                        }
                        Err(err) => (false, None, Some(err.to_string())),
                    };
                    frames.push(SimulateRealtimeFrameReport {
                        loop_index,
                        source_index: u16::try_from(source_index).unwrap_or(u16::MAX),
                        capture_attempt_index,
                        path: path.display().to_string(),
                        success,
                        decoded_payload_bytes,
                        error,
                    });
                }
            }
        }

        let output_payload_path = if let Some(path) = &self.output_payload {
            let payload = decoded_payload
                .as_ref()
                .ok_or_else(|| eyre!("no Petal frame decoded successfully; not writing payload"))?;
            write_payload_file(path, payload)?;
            Some(path.display().to_string())
        } else {
            None
        };
        let payload_bytes = decoded_payload.as_ref().map(|payload| payload.len() as u64);
        Ok(SimulateRealtimeReport {
            schema: SIMULATE_REALTIME_SCHEMA.to_string(),
            input_dir: self.input_dir.display().to_string(),
            manifest_path: input.manifest_path.map(|path| path.display().to_string()),
            channel: self.channel.to_string(),
            profile: self.profile.to_string(),
            perturb_capture: capture.is_some(),
            capture_seed: capture.map(|active| active.seed),
            capture_attempts_per_frame: attempts_per_frame,
            capture_profile: capture.map(|active| CaptureProfileReport::from(active.profile)),
            simulate_fps: self.simulate_fps,
            realtime_loops: self.realtime_loops,
            planned_attempts,
            attempts,
            decoded: decoded_payload.is_some(),
            payload_bytes,
            output_payload_path,
            first_success_loop_index,
            first_success_source_index,
            first_success_capture_attempt_index,
            grid: input.grid,
            frames,
        })
    }

    fn validate(&self) -> Result<()> {
        if self.channel != PetalEncodeChannelArg::BinaryGrid {
            return Err(eyre!(
                "--channel {} is planned for renderer-backed realtime simulation; use --channel binary-grid",
                self.channel
            ));
        }
        if self.simulate_fps == 0 {
            return Err(eyre!("--simulate-fps must be greater than 0"));
        }
        if self.realtime_loops == 0 {
            return Err(eyre!("--realtime-loops must be greater than 0"));
        }
        self.capture.validate(self.profile)?;
        Ok(())
    }
}

impl ScoreStylesArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let report = self.build_report()?;
        if let Some(path) = &self.output_report {
            write_report(path, &report)?;
            let summary = ScoreStylesWriteSummary {
                report_path: path.display().to_string(),
                recommended_style: report.recommended_style.clone(),
                gate_passed: report.gate_passed,
                overall_score_bps: report.recommended_overall_score_bps,
            };
            let text = format!(
                "wrote Petal style score report to {} (recommended_style={} gate_passed={} overall_score_bps={})",
                summary.report_path,
                summary.recommended_style,
                summary.gate_passed,
                summary.overall_score_bps
            );
            print_with_optional_text(context, Some(text), &summary)
        } else {
            let text = format!(
                "recommended_style={} gate_passed={} overall_score_bps={}",
                report.recommended_style, report.gate_passed, report.recommended_overall_score_bps
            );
            print_with_optional_text(context, Some(text), &report)
        }
    }

    fn build_report(&self) -> Result<ScoreStylesReport> {
        let payload = fs::read(&self.input)
            .wrap_err_with(|| format!("failed to read input payload {}", self.input.display()))?;
        if self.fps == 0 {
            return Err(eyre!("--fps must be greater than 0"));
        }
        if self.min_success_ratio_bps > PETAL_CAPTURE_RATIO_BPS_SCALE {
            return Err(eyre!("--min-success-ratio-bps exceeds 100%"));
        }

        let base_profile = self.capture_profile();
        let options = PetalStreamOptions {
            grid_size: self.grid_size,
            border: self.border,
            anchor_size: self.anchor_size,
        };
        let resolved_grid_size = PetalStreamEncoder::encode_grid(&payload, options)
            .map_err(|err| eyre!("failed to resolve Petal grid geometry: {err}"))?
            .grid_size;
        let effective_payload_bytes_per_second =
            (payload.len() as u64).saturating_mul(u64::from(self.fps));
        let effective_payload_bits_per_second =
            effective_payload_bytes_per_second.saturating_mul(8);
        let styles = self
            .style_set
            .style_names()
            .into_iter()
            .map(|style| {
                score_style(
                    style,
                    &payload,
                    options,
                    base_profile,
                    self.seed,
                    self.min_success_ratio_bps,
                    effective_payload_bytes_per_second,
                    effective_payload_bits_per_second,
                    self.target_effective_bps,
                )
            })
            .collect::<Result<Vec<_>>>()?;

        let recommended = styles
            .iter()
            .max_by_key(|style| (style.overall_score_bps, std::cmp::Reverse(style.style.clone())))
            .ok_or_else(|| eyre!("style set produced no candidates"))?;
        let recommended_style = recommended.style.clone();
        let recommended_overall_score_bps = recommended.overall_score_bps;
        let gate_passed = recommended.gate_passed;

        Ok(ScoreStylesReport {
            schema: SCORE_STYLES_SCHEMA.to_string(),
            input_path: self.input.display().to_string(),
            payload_bytes: payload.len() as u64,
            style_set: self.style_set.to_string(),
            profile: self.profile.to_string(),
            seed: self.seed,
            fps: self.fps,
            target_effective_bps: self.target_effective_bps,
            min_success_ratio_bps: self.min_success_ratio_bps,
            grid: GridReport {
                requested_grid_size: self.grid_size,
                resolved_grid_size,
                border: self.border,
                anchor_size: self.anchor_size,
            },
            capture_profile: CaptureProfileReport::from(base_profile),
            styles,
            recommended_style,
            recommended_overall_score_bps,
            gate_passed,
        })
    }

    fn capture_profile(&self) -> PetalStreamCaptureProfile {
        let mut profile = match self.profile {
            PetalCaptureProfileArg::Default => PetalStreamCaptureProfile::default(),
        };
        if let Some(attempts) = self.attempts {
            profile.attempts = attempts;
        }
        if let Some(dark_luma) = self.dark_luma {
            profile.dark_luma = dark_luma;
        }
        if let Some(light_luma) = self.light_luma {
            profile.light_luma = light_luma;
        }
        if let Some(luminance_jitter) = self.luminance_jitter {
            profile.luminance_jitter = luminance_jitter;
        }
        profile
    }
}

#[derive(Clone, Copy, Debug)]
struct ActiveCapturePerturbation {
    profile: PetalStreamCaptureProfile,
    seed: u64,
}

impl CapturePerturbationArgs {
    fn validate(&self, profile_arg: PetalCaptureProfileArg) -> Result<()> {
        if !self.perturb_capture {
            if self.seed != 0
                || self.attempts.is_some()
                || self.dark_luma.is_some()
                || self.light_luma.is_some()
                || self.luminance_jitter.is_some()
            {
                return Err(eyre!(
                    "capture perturbation overrides require --perturb-capture"
                ));
            }
            return Ok(());
        }
        let profile = self.profile(profile_arg);
        validate_capture_profile_for_cli(profile)
    }

    fn active(&self, profile_arg: PetalCaptureProfileArg) -> Result<Option<ActiveCapturePerturbation>> {
        self.validate(profile_arg)?;
        Ok(self.perturb_capture.then(|| ActiveCapturePerturbation {
            profile: self.profile(profile_arg),
            seed: self.seed,
        }))
    }

    fn profile(&self, profile_arg: PetalCaptureProfileArg) -> PetalStreamCaptureProfile {
        let mut profile = match profile_arg {
            PetalCaptureProfileArg::Default => PetalStreamCaptureProfile::default(),
        };
        if let Some(attempts) = self.attempts {
            profile.attempts = attempts;
        }
        if let Some(dark_luma) = self.dark_luma {
            profile.dark_luma = dark_luma;
        }
        if let Some(light_luma) = self.light_luma {
            profile.light_luma = light_luma;
        }
        if let Some(luminance_jitter) = self.luminance_jitter {
            profile.luminance_jitter = luminance_jitter;
        }
        profile
    }
}

fn validate_capture_profile_for_cli(profile: PetalStreamCaptureProfile) -> Result<()> {
    if profile.attempts == 0 {
        return Err(eyre!("capture attempts must be > 0"));
    }
    if profile.dark_luma >= profile.light_luma {
        return Err(eyre!("capture dark luminance must be lower than light luminance"));
    }
    Ok(())
}

impl PetalStyleSetArg {
    fn style_names(self) -> Vec<&'static str> {
        match self {
            Self::SoraTempleDefault => vec![DEFAULT_STYLE_NAME],
        }
    }
}

fn score_style(
    style: &str,
    payload: &[u8],
    options: PetalStreamOptions,
    profile: PetalStreamCaptureProfile,
    seed: u64,
    min_success_ratio_bps: u16,
    effective_payload_bytes_per_second: u64,
    effective_payload_bits_per_second: u64,
    target_effective_bps: u64,
) -> Result<StyleScoreReport> {
    let score = score_petal_capture_profile_with_seed(payload, options, profile, seed)
        .map_err(|err| eyre!("failed to score style '{style}': {err}"))?;
    let capture_success_ratio_bps = score.success_ratio_bps();
    let capture_gate_passed = score
        .meets_min_success_ratio_bps(min_success_ratio_bps)
        .map_err(|err| eyre!("failed to evaluate capture gate for style '{style}': {err}"))?;
    let throughput_score_bps =
        throughput_score_bps(effective_payload_bits_per_second, target_effective_bps);
    let throughput_gate_passed =
        target_effective_bps == 0 || effective_payload_bits_per_second >= target_effective_bps;
    let overall_score_bps = capture_success_ratio_bps.min(throughput_score_bps);
    Ok(StyleScoreReport {
        style: style.to_string(),
        capture_attempts: score.attempts,
        capture_successes: score.successes,
        capture_success_ratio_bps,
        capture_gate_passed,
        effective_payload_bytes_per_second,
        effective_payload_bits_per_second,
        throughput_score_bps,
        throughput_gate_passed,
        overall_score_bps,
        gate_passed: capture_gate_passed && throughput_gate_passed,
    })
}

fn throughput_score_bps(effective_payload_bits_per_second: u64, target_effective_bps: u64) -> u16 {
    if target_effective_bps == 0 {
        return PETAL_CAPTURE_RATIO_BPS_SCALE;
    }
    let numerator = u128::from(effective_payload_bits_per_second)
        .saturating_mul(u128::from(PETAL_CAPTURE_RATIO_BPS_SCALE));
    let ratio = numerator / u128::from(target_effective_bps);
    u16::try_from(ratio.min(u128::from(PETAL_CAPTURE_RATIO_BPS_SCALE)))
        .unwrap_or(PETAL_CAPTURE_RATIO_BPS_SCALE)
}

fn planned_capture_attempts(source_frames: usize, attempts_per_frame: u16) -> Result<u32> {
    let frames = u32::try_from(source_frames).map_err(|_| eyre!("too many Petal frames"))?;
    frames
        .checked_mul(u32::from(attempts_per_frame))
        .ok_or_else(|| eyre!("capture attempt count exceeds u32"))
}

fn planned_realtime_attempts(
    source_frames: usize,
    realtime_loops: u16,
    attempts_per_frame: u16,
) -> Result<u32> {
    planned_capture_attempts(source_frames, attempts_per_frame)?
        .checked_mul(u32::from(realtime_loops))
        .ok_or_else(|| eyre!("realtime attempt count exceeds u32"))
}

fn ratio_bps(successes: u32, attempts: u32) -> u16 {
    if attempts == 0 {
        return 0;
    }
    let numerator = u64::from(successes) * u64::from(PETAL_CAPTURE_RATIO_BPS_SCALE);
    u16::try_from(numerator / u64::from(attempts)).unwrap_or(PETAL_CAPTURE_RATIO_BPS_SCALE)
}

fn required_successes(attempts: u32, min_success_ratio_bps: u16) -> Result<u32> {
    validate_success_ratio_bps(min_success_ratio_bps)?;
    let numerator = attempts * u32::from(min_success_ratio_bps);
    Ok(numerator.div_ceil(u32::from(PETAL_CAPTURE_RATIO_BPS_SCALE)))
}

fn validate_success_ratio_bps(value: u16) -> Result<u16> {
    if value > PETAL_CAPTURE_RATIO_BPS_SCALE {
        return Err(eyre!("capture success ratio exceeds 100%"));
    }
    Ok(value)
}

fn parse_success_ratio_bps(raw: &str) -> Result<u16> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(eyre!("--min-success-ratio must not be empty"));
    }
    let (whole, fraction) = trimmed
        .split_once('.')
        .map_or((trimmed, ""), |(whole, fraction)| (whole, fraction));
    if whole != "0" && whole != "1" {
        return Err(eyre!("--min-success-ratio must be between 0 and 1"));
    }
    if !fraction.chars().all(|c| c.is_ascii_digit()) {
        return Err(eyre!("--min-success-ratio must be a decimal value"));
    }
    if fraction.len() > 4 {
        return Err(eyre!(
            "--min-success-ratio supports at most four decimal places"
        ));
    }
    let mut bps = if whole == "1" {
        PETAL_CAPTURE_RATIO_BPS_SCALE
    } else {
        0
    };
    if !fraction.is_empty() {
        if whole == "1" && fraction.chars().any(|c| c != '0') {
            return Err(eyre!("--min-success-ratio must not exceed 1"));
        }
        if whole == "1" {
            return Ok(PETAL_CAPTURE_RATIO_BPS_SCALE);
        }
        let mut padded = fraction.to_string();
        while padded.len() < 4 {
            padded.push('0');
        }
        bps = padded
            .parse::<u16>()
            .wrap_err("failed to parse --min-success-ratio")?;
    }
    validate_success_ratio_bps(bps)
}

struct EvalCaptureInput {
    manifest_path: Option<PathBuf>,
    payload_bytes: Option<u64>,
    grid: GridReport,
    frames: Vec<PathBuf>,
}

fn resolve_eval_capture_input(args: &EvalCaptureArgs) -> Result<EvalCaptureInput> {
    if !args.input_dir.is_dir() {
        return Err(eyre!(
            "--input-dir {} is not a directory",
            args.input_dir.display()
        ));
    }
    if let Some(manifest_path) = find_encode_manifest(&args.input_dir) {
        return resolve_eval_capture_manifest_input(args, manifest_path);
    }
    if args.grid_size == 0 {
        return Err(eyre!(
            "manifest.json not found; pass --grid-size for manifest-free PNG directories"
        ));
    }
    let mut frames = collect_png_frames(&args.input_dir)?;
    if frames.is_empty() {
        return Err(eyre!("no PNG frames found in {}", args.input_dir.display()));
    }
    frames.sort();
    Ok(EvalCaptureInput {
        manifest_path: None,
        payload_bytes: None,
        grid: GridReport {
            requested_grid_size: args.grid_size,
            resolved_grid_size: args.grid_size,
            border: args.border,
            anchor_size: args.anchor_size,
        },
        frames,
    })
}

fn find_encode_manifest(input_dir: &Path) -> Option<PathBuf> {
    let direct = input_dir.join("manifest.json");
    if direct.is_file() {
        return Some(direct);
    }
    let parent = input_dir.parent()?;
    let sibling = parent.join("manifest.json");
    sibling.is_file().then_some(sibling)
}

fn resolve_eval_capture_manifest_input(
    args: &EvalCaptureArgs,
    manifest_path: PathBuf,
) -> Result<EvalCaptureInput> {
    let raw = fs::read_to_string(&manifest_path)
        .wrap_err_with(|| format!("failed to read {}", manifest_path.display()))?;
    let value: norito::json::Value =
        norito::json::from_str(&raw).wrap_err("failed to parse Petal encode manifest JSON")?;
    let schema = json_str_field(&value, "schema")?;
    if schema != ENCODE_SCHEMA {
        return Err(eyre!(
            "unsupported Petal manifest schema '{schema}', expected {ENCODE_SCHEMA}"
        ));
    }
    let format = json_str_field(&value, "format")?;
    if format != "png" {
        return Err(eyre!("eval-capture supports PNG manifests, found '{format}'"));
    }
    let channel = json_str_field(&value, "channel")?;
    if channel != args.channel.to_string() {
        return Err(eyre!(
            "manifest channel '{channel}' does not match --channel {}",
            args.channel
        ));
    }
    let grid_value = value
        .get("grid")
        .ok_or_else(|| eyre!("manifest missing grid"))?;
    let grid = GridReport {
        requested_grid_size: json_u16_field(grid_value, "requested_grid_size")?,
        resolved_grid_size: json_u16_field(grid_value, "resolved_grid_size")?,
        border: json_u8_field(grid_value, "border")?,
        anchor_size: json_u8_field(grid_value, "anchor_size")?,
    };
    let payload_bytes = value.get("payload_bytes").and_then(norito::json::Value::as_u64);
    let frames_value = value
        .get("frames")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("manifest missing frames array"))?;
    let manifest_parent = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut frames = Vec::with_capacity(frames_value.len());
    for (idx, frame) in frames_value.iter().enumerate() {
        let path = frame
            .get("path")
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("manifest frame {idx} missing path"))?;
        frames.push(resolve_manifest_frame_path(manifest_parent, path));
    }
    if frames.is_empty() {
        return Err(eyre!("manifest contains no frames"));
    }
    Ok(EvalCaptureInput {
        manifest_path: Some(manifest_path),
        payload_bytes,
        grid,
        frames,
    })
}

fn collect_png_frames(input_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut frames = Vec::new();
    for entry in fs::read_dir(input_dir)
        .wrap_err_with(|| format!("failed to read {}", input_dir.display()))?
    {
        let path = entry?.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            frames.push(path);
        }
    }
    Ok(frames)
}

fn resolve_manifest_frame_path(manifest_parent: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() || path.exists() {
        path
    } else {
        manifest_parent.join(path)
    }
}

fn evaluate_png_frame(
    path: &Path,
    options: PetalStreamOptions,
    expected_payload_bytes: Option<u64>,
    capture: Option<ActiveCapturePerturbation>,
    capture_attempt_index: u16,
) -> Result<usize> {
    decode_png_frame_payload(
        path,
        options,
        expected_payload_bytes,
        capture,
        capture_attempt_index,
    )
    .map(|payload| payload.len())
}

fn decode_png_frame_payload(
    path: &Path,
    options: PetalStreamOptions,
    expected_payload_bytes: Option<u64>,
    capture: Option<ActiveCapturePerturbation>,
    capture_attempt_index: u16,
) -> Result<Vec<u8>> {
    let mut samples = read_petal_png_samples(path, options.grid_size)?;
    if let Some(capture) = capture {
        samples = perturb_petal_samples(&samples, capture, capture_attempt_index)?;
    }
    let decoded = decode_petal_samples_payload(&samples, options)
        .map_err(|err| eyre!("failed to decode Petal frame: {err}"))?;
    if let Some(expected) = expected_payload_bytes
        && decoded.len() as u64 != expected
    {
        return Err(eyre!(
            "decoded payload length {} does not match manifest payload_bytes {}",
            decoded.len(),
            expected
        ));
    }
    Ok(decoded)
}

fn decode_petal_samples_payload(
    samples: &PetalStreamSampleGrid,
    options: PetalStreamOptions,
) -> std::result::Result<Vec<u8>, iroha::data_model::petal_stream::PetalStreamError> {
    PetalStreamDecoder::decode_samples(samples, options)
}

fn perturb_petal_samples(
    samples: &PetalStreamSampleGrid,
    capture: ActiveCapturePerturbation,
    capture_attempt_index: u16,
) -> Result<PetalStreamSampleGrid> {
    let cells = samples
        .samples
        .iter()
        .map(|sample| *sample < 128)
        .collect::<Vec<_>>();
    let grid = PetalStreamGrid::new(samples.grid_size, cells)
        .map_err(|err| eyre!("failed to build Petal perturbation grid: {err}"))?;
    render_petal_capture_samples_with_seed(
        &grid,
        capture.profile,
        capture_attempt_index,
        capture.seed,
    )
    .map_err(|err| eyre!("failed to perturb Petal samples: {err}"))
}

fn read_petal_png_samples(path: &Path, grid_size: u16) -> Result<PetalStreamSampleGrid> {
    if grid_size == 0 {
        return Err(eyre!("grid size must be greater than 0"));
    }
    let file =
        fs::File::open(path).wrap_err_with(|| format!("failed to open PNG {}", path.display()))?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder
        .read_info()
        .wrap_err_with(|| format!("failed to read PNG info {}", path.display()))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| eyre!("PNG output buffer size unavailable"))?;
    let mut buffer = vec![0; size];
    let info = reader
        .next_frame(&mut buffer)
        .wrap_err_with(|| format!("failed to decode PNG {}", path.display()))?;
    if info.width != info.height {
        return Err(eyre!("Petal PNG must be square"));
    }
    if info.width < u32::from(grid_size) {
        return Err(eyre!(
            "PNG dimension {} is smaller than grid size {}",
            info.width,
            grid_size
        ));
    }
    if info.bit_depth != png::BitDepth::Eight {
        return Err(eyre!("Petal PNG must decode to 8-bit samples"));
    }
    let bytes = &buffer[..info.buffer_size()];
    let samples = sample_png_luminance_grid(bytes, &info, grid_size)?;
    PetalStreamSampleGrid::new(grid_size, samples)
        .map_err(|err| eyre!("failed to build Petal sample grid: {err}"))
}

fn sample_png_luminance_grid(
    bytes: &[u8],
    info: &png::OutputInfo,
    grid_size: u16,
) -> Result<Vec<u8>> {
    let channels = info.color_type.samples();
    let mut samples = Vec::with_capacity(grid_size as usize * grid_size as usize);
    for gy in 0..u32::from(grid_size) {
        let y = cell_center_pixel(gy, info.height, grid_size);
        for gx in 0..u32::from(grid_size) {
            let x = cell_center_pixel(gx, info.width, grid_size);
            let idx = y as usize * info.line_size + x as usize * channels;
            samples.push(pixel_luminance(info.color_type, &bytes[idx..])?);
        }
    }
    let expected = grid_size as usize * grid_size as usize;
    if samples.len() != expected || info.width == 0 {
        return Err(eyre!("PNG sampling produced invalid grid"));
    }
    Ok(samples)
}

fn cell_center_pixel(cell: u32, dimension: u32, grid_size: u16) -> u32 {
    let grid_size = u32::from(grid_size);
    (((cell * 2 + 1) * dimension) / (grid_size * 2)).min(dimension.saturating_sub(1))
}

fn pixel_luminance(color_type: png::ColorType, pixel: &[u8]) -> Result<u8> {
    match color_type {
        png::ColorType::Grayscale => pixel
            .first()
            .copied()
            .ok_or_else(|| eyre!("truncated grayscale pixel")),
        png::ColorType::Rgb => rgb_luminance(pixel),
        png::ColorType::GrayscaleAlpha => {
            let gray = *pixel
                .first()
                .ok_or_else(|| eyre!("truncated grayscale-alpha pixel"))?;
            let alpha = *pixel
                .get(1)
                .ok_or_else(|| eyre!("truncated grayscale-alpha pixel"))?;
            Ok(composite_luminance_over_white(gray, alpha))
        }
        png::ColorType::Rgba => {
            let rgb = rgb_luminance(pixel)?;
            let alpha = *pixel.get(3).ok_or_else(|| eyre!("truncated rgba pixel"))?;
            Ok(composite_luminance_over_white(rgb, alpha))
        }
        png::ColorType::Indexed => Err(eyre!("indexed PNG output is not supported")),
    }
}

fn rgb_luminance(pixel: &[u8]) -> Result<u8> {
    let r = *pixel.first().ok_or_else(|| eyre!("truncated rgb pixel"))?;
    let g = *pixel.get(1).ok_or_else(|| eyre!("truncated rgb pixel"))?;
    let b = *pixel.get(2).ok_or_else(|| eyre!("truncated rgb pixel"))?;
    Ok(((u16::from(r) * 77 + u16::from(g) * 150 + u16::from(b) * 29) >> 8) as u8)
}

fn composite_luminance_over_white(luminance: u8, alpha: u8) -> u8 {
    let foreground = u16::from(luminance) * u16::from(alpha);
    let background = u16::from(u8::MAX) * u16::from(u8::MAX - alpha);
    u8::try_from((foreground + background) / u16::from(u8::MAX)).unwrap_or(u8::MAX)
}

fn json_str_field<'a>(value: &'a norito::json::Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("manifest missing string field `{field}`"))
}

fn json_u16_field(value: &norito::json::Value, field: &str) -> Result<u16> {
    let raw = value
        .get(field)
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| eyre!("manifest missing integer field `{field}`"))?;
    u16::try_from(raw).map_err(|_| eyre!("manifest field `{field}` exceeds u16"))
}

fn json_u8_field(value: &norito::json::Value, field: &str) -> Result<u8> {
    let raw = value
        .get(field)
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| eyre!("manifest missing integer field `{field}`"))?;
    u8::try_from(raw).map_err(|_| eyre!("manifest field `{field}` exceeds u8"))
}

fn write_grid_image(
    path: &Path,
    grid: &PetalStreamGrid,
    dimension: u32,
    format: PetalEncodeFormatArg,
    fps: u16,
) -> Result<()> {
    match format {
        PetalEncodeFormatArg::Png => write_grid_png(path, grid, dimension),
        PetalEncodeFormatArg::Gif => write_grid_gif(path, grid, dimension, fps),
    }
}

fn render_grid_luma(grid: &PetalStreamGrid, dimension: u32) -> Result<Vec<u8>> {
    let mut pixels = vec![255u8; dimension as usize * dimension as usize];
    let grid_size = u32::from(grid.grid_size);
    for y in 0..dimension {
        let grid_y = y.saturating_mul(grid_size) / dimension;
        for x in 0..dimension {
            let grid_x = x.saturating_mul(grid_size) / dimension;
            let cell = grid
                .get(grid_x as u16, grid_y as u16)
                .ok_or_else(|| eyre!("failed to sample Petal grid cell"))?;
            let idx = y as usize * dimension as usize + x as usize;
            pixels[idx] = if cell { 0 } else { 255 };
        }
    }
    Ok(pixels)
}

fn write_grid_png(path: &Path, grid: &PetalStreamGrid, dimension: u32) -> Result<()> {
    let pixels = render_grid_luma(grid, dimension)?;
    let file = fs::File::create(path)
        .wrap_err_with(|| format!("failed to create PNG {}", path.display()))?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, dimension, dimension);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .wrap_err_with(|| format!("failed to write PNG header {}", path.display()))?;
    writer
        .write_image_data(&pixels)
        .wrap_err_with(|| format!("failed to write PNG pixels {}", path.display()))
}

#[cfg(feature = "offline-visual-codecs")]
fn write_grid_gif(path: &Path, grid: &PetalStreamGrid, dimension: u32, fps: u16) -> Result<()> {
    use image::{
        Delay, Frame, RgbaImage,
        codecs::gif::{GifEncoder, Repeat},
    };

    let luma = render_grid_luma(grid, dimension)?;
    let mut rgba = Vec::with_capacity(luma.len() * 4);
    for value in luma {
        rgba.extend_from_slice(&[value, value, value, u8::MAX]);
    }
    let image = RgbaImage::from_raw(dimension, dimension, rgba)
        .ok_or_else(|| eyre!("failed to build GIF frame buffer"))?;
    let delay_ms = (1_000u32 / u32::from(fps)).max(1);
    let frame = Frame::from_parts(
        image,
        0,
        0,
        Delay::from_numer_denom_ms(delay_ms, 1),
    );
    let file = fs::File::create(path)
        .wrap_err_with(|| format!("failed to create GIF {}", path.display()))?;
    let mut encoder = GifEncoder::new(BufWriter::new(file));
    encoder
        .set_repeat(Repeat::Infinite)
        .wrap_err_with(|| format!("failed to set GIF repeat {}", path.display()))?;
    encoder
        .encode_frame(frame)
        .wrap_err_with(|| format!("failed to write GIF frame {}", path.display()))
}

#[cfg(not(feature = "offline-visual-codecs"))]
fn write_grid_gif(
    _path: &Path,
    _grid: &PetalStreamGrid,
    _dimension: u32,
    _fps: u16,
) -> Result<()> {
    Err(eyre!(
        "--format gif requires building iroha_cli with --features offline-visual-codecs"
    ))
}

fn write_encode_manifest(path: &Path, report: &EncodeReport) -> Result<()> {
    let json = norito::json::to_vec_pretty(report)
        .map_err(|err| eyre!("failed to encode Petal manifest: {err}"))?;
    fs::write(path, json).wrap_err_with(|| format!("failed to write {}", path.display()))
}

fn write_eval_capture_report(path: &Path, report: &EvalCaptureReport) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
    }
    let json = norito::json::to_vec_pretty(report)
        .map_err(|err| eyre!("failed to encode Petal capture report: {err}"))?;
    fs::write(path, json).wrap_err_with(|| format!("failed to write {}", path.display()))
}

fn write_simulate_realtime_report(path: &Path, report: &SimulateRealtimeReport) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
    }
    let json = norito::json::to_vec_pretty(report)
        .map_err(|err| eyre!("failed to encode Petal realtime report: {err}"))?;
    fs::write(path, json).wrap_err_with(|| format!("failed to write {}", path.display()))
}

fn write_payload_file(path: &Path, payload: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, payload).wrap_err_with(|| format!("failed to write {}", path.display()))
}

fn write_report(path: &Path, report: &ScoreStylesReport) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create {}", parent.display()))?;
    }
    let json = norito::json::to_vec_pretty(report)
        .map_err(|err| eyre!("failed to encode Petal style score report: {err}"))?;
    fs::write(path, json).wrap_err_with(|| format!("failed to write {}", path.display()))
}

#[derive(Clone, Debug, JsonSerialize)]
struct EncodeReport {
    schema: String,
    input_path: String,
    output_dir: String,
    payload_bytes: u64,
    format: String,
    style: String,
    channel: String,
    fps: u16,
    dimension: u32,
    grid: GridReport,
    frames: Vec<EncodeFrameReport>,
}

#[derive(Clone, Debug, JsonSerialize)]
struct EncodeFrameReport {
    index: u16,
    path: String,
    payload_bytes: u64,
}

#[derive(Clone, Debug, JsonSerialize)]
struct EvalCaptureReport {
    schema: String,
    input_dir: String,
    manifest_path: Option<String>,
    channel: String,
    profile: String,
    perturb_capture: bool,
    capture_seed: Option<u64>,
    capture_attempts_per_frame: u16,
    capture_profile: Option<CaptureProfileReport>,
    min_success_ratio_bps: u16,
    planned_attempts: u32,
    attempts: u32,
    successes: u32,
    required_successes: u32,
    success_ratio_bps: u16,
    gate_passed: bool,
    aborted_early: bool,
    grid: GridReport,
    frames: Vec<EvalCaptureFrameReport>,
}

#[derive(Clone, Debug, JsonSerialize)]
struct EvalCaptureFrameReport {
    index: u32,
    source_index: u16,
    capture_attempt_index: u16,
    path: String,
    success: bool,
    decoded_payload_bytes: Option<u64>,
    error: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize)]
struct EvalCaptureWriteSummary {
    report_path: String,
    gate_passed: bool,
    success_ratio_bps: u16,
    attempts: u32,
    planned_attempts: u32,
    aborted_early: bool,
}

#[derive(Clone, Debug, JsonSerialize)]
struct SimulateRealtimeReport {
    schema: String,
    input_dir: String,
    manifest_path: Option<String>,
    channel: String,
    profile: String,
    perturb_capture: bool,
    capture_seed: Option<u64>,
    capture_attempts_per_frame: u16,
    capture_profile: Option<CaptureProfileReport>,
    simulate_fps: u16,
    realtime_loops: u16,
    planned_attempts: u32,
    attempts: u32,
    decoded: bool,
    payload_bytes: Option<u64>,
    output_payload_path: Option<String>,
    first_success_loop_index: Option<u16>,
    first_success_source_index: Option<u16>,
    first_success_capture_attempt_index: Option<u16>,
    grid: GridReport,
    frames: Vec<SimulateRealtimeFrameReport>,
}

#[derive(Clone, Debug, JsonSerialize)]
struct SimulateRealtimeFrameReport {
    loop_index: u16,
    source_index: u16,
    capture_attempt_index: u16,
    path: String,
    success: bool,
    decoded_payload_bytes: Option<u64>,
    error: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize)]
struct SimulateRealtimeWriteSummary {
    report_path: String,
    output_payload_path: Option<String>,
    decoded: bool,
    attempts: u32,
    planned_attempts: u32,
    payload_bytes: Option<u64>,
}

#[derive(Clone, Debug, JsonSerialize)]
struct ScoreStylesReport {
    schema: String,
    input_path: String,
    payload_bytes: u64,
    style_set: String,
    profile: String,
    seed: u64,
    fps: u16,
    target_effective_bps: u64,
    min_success_ratio_bps: u16,
    grid: GridReport,
    capture_profile: CaptureProfileReport,
    styles: Vec<StyleScoreReport>,
    recommended_style: String,
    recommended_overall_score_bps: u16,
    gate_passed: bool,
}

#[derive(Clone, Debug, JsonSerialize)]
struct ScoreStylesWriteSummary {
    report_path: String,
    recommended_style: String,
    gate_passed: bool,
    overall_score_bps: u16,
}

#[derive(Clone, Debug, JsonSerialize)]
struct GridReport {
    requested_grid_size: u16,
    resolved_grid_size: u16,
    border: u8,
    anchor_size: u8,
}

#[derive(Clone, Copy, Debug, JsonSerialize)]
struct CaptureProfileReport {
    attempts: u16,
    dark_luma: u8,
    light_luma: u8,
    luminance_jitter: u8,
}

impl From<PetalStreamCaptureProfile> for CaptureProfileReport {
    fn from(profile: PetalStreamCaptureProfile) -> Self {
        Self {
            attempts: profile.attempts,
            dark_luma: profile.dark_luma,
            light_luma: profile.light_luma,
            luminance_jitter: profile.luminance_jitter,
        }
    }
}

#[derive(Clone, Debug, JsonSerialize)]
struct StyleScoreReport {
    style: String,
    capture_attempts: u16,
    capture_successes: u16,
    capture_success_ratio_bps: u16,
    capture_gate_passed: bool,
    effective_payload_bytes_per_second: u64,
    effective_payload_bits_per_second: u64,
    throughput_score_bps: u16,
    throughput_gate_passed: bool,
    overall_score_bps: u16,
    gate_passed: bool,
}

#[cfg(test)]
mod tests {
    use std::fmt::Display;

    use iroha_i18n::{Bundle, Language, Localizer};
    use norito::json::Value;

    use super::*;

    struct TestContext {
        output_format: crate::CliOutputFormat,
        printed: Vec<String>,
        config: iroha::config::Config,
        i18n: Localizer,
    }

    impl TestContext {
        fn new(output_format: crate::CliOutputFormat) -> Self {
            Self {
                output_format,
                printed: Vec::new(),
                config: crate::fallback_config(),
                i18n: Localizer::new(Bundle::Cli, Language::English),
            }
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &iroha::config::Config {
            &self.config
        }

        fn transaction_metadata(&self) -> Option<&iroha::data_model::metadata::Metadata> {
            None
        }

        fn input_instructions(&self) -> bool {
            false
        }

        fn output_instructions(&self) -> bool {
            false
        }

        fn i18n(&self) -> &Localizer {
            &self.i18n
        }

        fn output_format(&self) -> crate::CliOutputFormat {
            self.output_format
        }

        fn print_data<T>(&mut self, data: &T) -> eyre::Result<()>
        where
            T: norito::json::JsonSerialize + ?Sized,
        {
            self.printed.push(norito::json::to_json_pretty(data)?);
            Ok(())
        }

        fn println(&mut self, data: impl Display) -> eyre::Result<()> {
            self.printed.push(data.to_string());
            Ok(())
        }
    }

    fn write_payload(bytes: &[u8]) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().expect("temp payload");
        std::io::Write::write_all(&mut file, bytes).expect("write payload");
        file
    }

    fn score_args(input: &Path) -> ScoreStylesArgs {
        ScoreStylesArgs {
            input: input.to_path_buf(),
            output_report: None,
            style_set: PetalStyleSetArg::SoraTempleDefault,
            profile: PetalCaptureProfileArg::Default,
            seed: 0,
            fps: DEFAULT_SCORE_STYLES_FPS,
            target_effective_bps: DEFAULT_TARGET_EFFECTIVE_BPS,
            min_success_ratio_bps: PETAL_CAPTURE_DEFAULT_MIN_SUCCESS_RATIO_BPS,
            grid_size: 0,
            border: iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_BORDER,
            anchor_size: iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_ANCHOR,
            attempts: None,
            dark_luma: None,
            light_luma: None,
            luminance_jitter: None,
        }
    }

    fn encode_args(input: &Path, output: &Path) -> EncodeArgs {
        EncodeArgs {
            input: input.to_path_buf(),
            output: output.to_path_buf(),
            format: PetalEncodeFormatArg::Png,
            style: PetalEncodeStyleArg::SoraTemple,
            channel: PetalEncodeChannelArg::BinaryGrid,
            dimension: DEFAULT_ENCODE_DIMENSION,
            fps: DEFAULT_SCORE_STYLES_FPS,
            grid_size: 0,
            border: iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_BORDER,
            anchor_size: iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_ANCHOR,
        }
    }

    fn eval_capture_args(input_dir: &Path) -> EvalCaptureArgs {
        EvalCaptureArgs {
            input_dir: input_dir.to_path_buf(),
            channel: PetalEncodeChannelArg::BinaryGrid,
            profile: PetalCaptureProfileArg::Default,
            capture: CapturePerturbationArgs::default(),
            min_success_ratio: None,
            min_success_ratio_bps: None,
            output_report: None,
            grid_size: 0,
            border: iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_BORDER,
            anchor_size: iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_ANCHOR,
        }
    }

    fn simulate_realtime_args(input_dir: &Path) -> SimulateRealtimeArgs {
        SimulateRealtimeArgs {
            input_dir: input_dir.to_path_buf(),
            channel: PetalEncodeChannelArg::BinaryGrid,
            profile: PetalCaptureProfileArg::Default,
            capture: CapturePerturbationArgs::default(),
            simulate_fps: DEFAULT_SCORE_STYLES_FPS,
            realtime_loops: 1,
            output_payload: None,
            output_report: None,
            grid_size: 0,
            border: iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_BORDER,
            anchor_size: iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_ANCHOR,
        }
    }

    #[test]
    fn encode_png_writes_manifest_and_frame() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let args = encode_args(payload.path(), tempdir.path());
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);

        args.run(&mut ctx).expect("run encode");

        let output: Value = norito::json::from_str(&ctx.printed[0]).expect("encode JSON");
        assert_eq!(output.get("schema").and_then(Value::as_str), Some(ENCODE_SCHEMA));
        assert_eq!(output["grid"]["resolved_grid_size"], Value::from(33u64));
        let frame_path = tempdir.path().join("png/frame_0000.png");
        let png = std::fs::read(&frame_path).expect("read PNG");
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

        let manifest_path = tempdir.path().join("manifest.json");
        let manifest: Value =
            norito::json::from_slice(&std::fs::read(manifest_path).expect("read manifest"))
                .expect("manifest JSON");
        assert_eq!(
            manifest.get("schema").and_then(Value::as_str),
            Some(ENCODE_SCHEMA)
        );
        assert_eq!(manifest["frames"][0]["path"], Value::from(frame_path.display().to_string()));
    }

    #[test]
    fn encode_rejects_renderer_backed_channel_until_wired() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = encode_args(payload.path(), tempdir.path());
        args.channel = PetalEncodeChannelArg::KatakanaBase94;

        let err = args.encode().expect_err("katakana channel rejected");
        assert!(err.to_string().contains("renderer-backed"));
    }

    #[cfg(not(feature = "offline-visual-codecs"))]
    #[test]
    fn encode_gif_requires_visual_codecs_feature() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = encode_args(payload.path(), tempdir.path());
        args.format = PetalEncodeFormatArg::Gif;

        let err = args.encode().expect_err("gif feature rejected");
        assert!(err.to_string().contains("offline-visual-codecs"));
    }

    #[cfg(feature = "offline-visual-codecs")]
    #[test]
    fn encode_gif_writes_manifest_and_frame_with_visual_codecs_feature() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = encode_args(payload.path(), tempdir.path());
        args.format = PetalEncodeFormatArg::Gif;
        args.dimension = 128;

        let report = args.encode().expect("encode gif");

        let frame_path = tempdir.path().join("gif/frame_0000.gif");
        let gif = std::fs::read(&frame_path).expect("read GIF");
        assert!(gif.starts_with(b"GIF89a") || gif.starts_with(b"GIF87a"));
        assert_eq!(report.format, "gif");
        assert_eq!(report.frames[0].path, frame_path.display().to_string());
        let manifest: Value =
            norito::json::from_slice(&std::fs::read(tempdir.path().join("manifest.json")).expect("read manifest"))
                .expect("manifest JSON");
        assert_eq!(manifest.get("format").and_then(Value::as_str), Some("gif"));
    }

    #[test]
    fn encode_rejects_dimension_smaller_than_grid() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = encode_args(payload.path(), tempdir.path());
        args.dimension = 16;

        let err = args.encode().expect_err("small dimension rejected");
        assert!(err.to_string().contains("resolved grid size"));
    }

    #[test]
    fn encode_rejects_unbounded_dimension() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = encode_args(payload.path(), tempdir.path());
        args.dimension = MAX_ENCODE_DIMENSION + 1;

        let err = args.encode().expect_err("large dimension rejected");
        assert!(err.to_string().contains("bounded offline rendering"));
    }

    #[test]
    fn eval_capture_decodes_encoded_png_and_passes_gate() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut encode = encode_args(payload.path(), tempdir.path());
        encode.dimension = 128;
        encode.encode().expect("encode png");
        let args = eval_capture_args(&tempdir.path().join("png"));
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);

        args.run(&mut ctx).expect("eval capture");

        let output: Value = norito::json::from_str(&ctx.printed[0]).expect("eval JSON");
        assert_eq!(
            output.get("schema").and_then(Value::as_str),
            Some(EVAL_CAPTURE_SCHEMA)
        );
        assert_eq!(output.get("gate_passed").and_then(Value::as_bool), Some(true));
        assert_eq!(
            output.get("success_ratio_bps").and_then(Value::as_u64),
            Some(u64::from(PETAL_CAPTURE_RATIO_BPS_SCALE))
        );
        assert_eq!(output["frames"][0]["success"], Value::from(true));
    }

    #[test]
    fn eval_capture_perturbed_default_profile_runs_all_attempts_and_passes_gate() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut encode = encode_args(payload.path(), tempdir.path());
        encode.dimension = 128;
        encode.encode().expect("encode png");
        let mut args = eval_capture_args(&tempdir.path().join("png"));
        args.capture.perturb_capture = true;
        args.capture.seed = 42;

        let report = args.evaluate().expect("eval perturbed capture");

        assert!(report.perturb_capture);
        assert_eq!(report.capture_seed, Some(42));
        assert_eq!(
            report.capture_attempts_per_frame,
            PetalStreamCaptureProfile::default().attempts
        );
        assert_eq!(
            report.planned_attempts,
            u32::from(PetalStreamCaptureProfile::default().attempts)
        );
        assert_eq!(report.attempts, report.planned_attempts);
        assert_eq!(report.successes, report.planned_attempts);
        assert!(report.gate_passed);
        assert_eq!(report.frames[11].source_index, 0);
        assert_eq!(report.frames[11].capture_attempt_index, 11);
        assert_eq!(
            report.capture_profile.expect("profile").luminance_jitter,
            PetalStreamCaptureProfile::default().luminance_jitter
        );
    }

    #[test]
    fn eval_capture_perturbed_low_contrast_profile_fails_early() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut encode = encode_args(payload.path(), tempdir.path());
        encode.dimension = 128;
        encode.encode().expect("encode png");
        let mut args = eval_capture_args(tempdir.path());
        args.capture.perturb_capture = true;
        args.capture.attempts = Some(4);
        args.capture.dark_luma = Some(128);
        args.capture.light_luma = Some(129);
        args.capture.luminance_jitter = Some(0);

        let report = args.evaluate().expect("eval low contrast capture");

        assert_eq!(report.planned_attempts, 4);
        assert_eq!(report.required_successes, 4);
        assert_eq!(report.attempts, 1);
        assert_eq!(report.successes, 0);
        assert!(report.aborted_early);
        assert!(!report.gate_passed);
        assert!(report.frames[0].error.is_some());
    }

    #[test]
    fn eval_capture_writes_report_summary() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        encode_args(payload.path(), tempdir.path())
            .encode()
            .expect("encode png");
        let report_path = tempdir.path().join("capture/eval.json");
        let mut args = eval_capture_args(tempdir.path());
        args.output_report = Some(report_path.clone());
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);

        args.run(&mut ctx).expect("eval capture");

        let report: Value =
            norito::json::from_slice(&std::fs::read(&report_path).expect("read report"))
                .expect("report JSON");
        assert_eq!(report.get("gate_passed").and_then(Value::as_bool), Some(true));
        let summary: Value = norito::json::from_str(&ctx.printed[0]).expect("summary JSON");
        assert_eq!(
            summary.get("report_path").and_then(Value::as_str),
            Some(report_path.display().to_string().as_str())
        );
    }

    #[test]
    fn eval_capture_corrupt_png_fails_gate() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        encode_args(payload.path(), tempdir.path())
            .encode()
            .expect("encode png");
        std::fs::write(tempdir.path().join("png/frame_0000.png"), b"not a png")
            .expect("tamper png");
        let report = eval_capture_args(tempdir.path())
            .evaluate()
            .expect("eval corrupt frame");

        assert_eq!(report.planned_attempts, 1);
        assert_eq!(report.attempts, 1);
        assert_eq!(report.successes, 0);
        assert!(!report.gate_passed);
        assert!(!report.aborted_early);
        assert!(report.frames[0].error.is_some());
    }

    #[test]
    fn eval_capture_aborts_when_gate_unreachable() {
        let tempdir = tempfile::tempdir().expect("temp dir");
        std::fs::write(tempdir.path().join("a.png"), b"not a png").expect("write a");
        std::fs::write(tempdir.path().join("b.png"), b"not a png").expect("write b");
        let mut args = eval_capture_args(tempdir.path());
        args.grid_size = 33;

        let report = args.evaluate().expect("eval corrupt frames");

        assert_eq!(report.planned_attempts, 2);
        assert_eq!(report.attempts, 1);
        assert_eq!(report.successes, 0);
        assert_eq!(report.required_successes, 2);
        assert!(report.aborted_early);
        assert!(!report.gate_passed);
    }

    #[test]
    fn eval_capture_requires_manifest_or_grid_size() {
        let tempdir = tempfile::tempdir().expect("temp dir");
        std::fs::write(tempdir.path().join("frame.png"), b"not a png").expect("write frame");

        let err = eval_capture_args(tempdir.path())
            .evaluate()
            .expect_err("missing grid");
        assert!(err.to_string().contains("--grid-size"));
    }

    #[test]
    fn eval_capture_rejects_renderer_backed_channel_until_wired() {
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = eval_capture_args(tempdir.path());
        args.channel = PetalEncodeChannelArg::KatakanaBase94;

        let err = args.evaluate().expect_err("katakana eval rejected");
        assert!(err.to_string().contains("renderer-backed"));
    }

    #[test]
    fn eval_capture_parses_decimal_ratio_and_rejects_invalid_ratio() {
        assert_eq!(parse_success_ratio_bps("0.95").expect("ratio"), 9_500);
        assert_eq!(
            parse_success_ratio_bps("1.0000").expect("ratio"),
            PETAL_CAPTURE_RATIO_BPS_SCALE
        );
        assert!(parse_success_ratio_bps("1.0001").is_err());
        assert!(parse_success_ratio_bps("0.12345").is_err());
    }

    #[test]
    fn eval_capture_rejects_inactive_and_invalid_capture_perturbation_options() {
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = eval_capture_args(tempdir.path());
        args.capture.dark_luma = Some(32);
        let err = args
            .evaluate()
            .expect_err("inactive perturbation override rejected");
        assert!(err.to_string().contains("perturb-capture"));

        let mut args = eval_capture_args(tempdir.path());
        args.capture.perturb_capture = true;
        args.capture.attempts = Some(0);
        let err = args.evaluate().expect_err("zero capture attempts rejected");
        assert!(err.to_string().contains("capture attempts"));

        let mut args = eval_capture_args(tempdir.path());
        args.capture.perturb_capture = true;
        args.capture.dark_luma = Some(200);
        args.capture.light_luma = Some(128);
        let err = args
            .evaluate()
            .expect_err("inverted capture luminance rejected");
        assert!(err.to_string().contains("dark luminance"));
    }

    #[test]
    fn simulate_realtime_decodes_looped_png_and_writes_payload() {
        let payload = b"sora-temple-capture-baseline";
        let payload_file = write_payload(payload);
        let tempdir = tempfile::tempdir().expect("temp dir");
        encode_args(payload_file.path(), tempdir.path())
            .encode()
            .expect("encode png");
        let output_payload = tempdir.path().join("decoded/payload.bin");
        let mut args = simulate_realtime_args(&tempdir.path().join("png"));
        args.realtime_loops = 3;
        args.output_payload = Some(output_payload.clone());
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);

        args.run(&mut ctx).expect("simulate realtime");

        assert_eq!(std::fs::read(&output_payload).expect("read payload"), payload);
        let report: Value = norito::json::from_str(&ctx.printed[0]).expect("realtime JSON");
        assert_eq!(
            report.get("schema").and_then(Value::as_str),
            Some(SIMULATE_REALTIME_SCHEMA)
        );
        assert_eq!(report.get("decoded").and_then(Value::as_bool), Some(true));
        assert_eq!(report.get("planned_attempts").and_then(Value::as_u64), Some(3));
        assert_eq!(
            report.get("first_success_loop_index").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(report["frames"][2]["loop_index"], Value::from(2u64));
    }

    #[test]
    fn simulate_realtime_perturbed_default_profile_reports_capture_attempts() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut encode = encode_args(payload.path(), tempdir.path());
        encode.dimension = 128;
        encode.encode().expect("encode png");
        let mut args = simulate_realtime_args(&tempdir.path().join("png"));
        args.realtime_loops = 2;
        args.capture.perturb_capture = true;
        args.capture.seed = 7;
        args.capture.attempts = Some(2);

        let report = args.simulate().expect("simulate perturbed realtime");

        assert!(report.perturb_capture);
        assert_eq!(report.capture_seed, Some(7));
        assert_eq!(report.capture_attempts_per_frame, 2);
        assert_eq!(report.planned_attempts, 4);
        assert_eq!(report.attempts, 4);
        assert!(report.decoded);
        assert_eq!(report.first_success_loop_index, Some(0));
        assert_eq!(report.first_success_source_index, Some(0));
        assert_eq!(report.first_success_capture_attempt_index, Some(0));
        assert_eq!(report.frames[3].loop_index, 1);
        assert_eq!(report.frames[3].source_index, 0);
        assert_eq!(report.frames[3].capture_attempt_index, 1);
    }

    #[test]
    fn simulate_realtime_writes_report_summary() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        encode_args(payload.path(), tempdir.path())
            .encode()
            .expect("encode png");
        let report_path = tempdir.path().join("realtime/report.json");
        let mut args = simulate_realtime_args(tempdir.path());
        args.output_report = Some(report_path.clone());
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);

        args.run(&mut ctx).expect("simulate realtime");

        let report: Value =
            norito::json::from_slice(&std::fs::read(&report_path).expect("read report"))
                .expect("report JSON");
        assert_eq!(report.get("decoded").and_then(Value::as_bool), Some(true));
        let summary: Value = norito::json::from_str(&ctx.printed[0]).expect("summary JSON");
        assert_eq!(
            summary.get("report_path").and_then(Value::as_str),
            Some(report_path.display().to_string().as_str())
        );
    }

    #[test]
    fn simulate_realtime_corrupt_frames_do_not_write_payload() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        encode_args(payload.path(), tempdir.path())
            .encode()
            .expect("encode png");
        std::fs::write(tempdir.path().join("png/frame_0000.png"), b"not a png")
            .expect("tamper png");
        let mut args = simulate_realtime_args(tempdir.path());
        args.output_payload = Some(tempdir.path().join("decoded.bin"));

        let err = args.simulate().expect_err("payload write rejected");
        assert!(err.to_string().contains("no Petal frame decoded"));
        assert!(!tempdir.path().join("decoded.bin").exists());
    }

    #[test]
    fn simulate_realtime_reports_decode_failure_without_payload_output() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        encode_args(payload.path(), tempdir.path())
            .encode()
            .expect("encode png");
        std::fs::write(tempdir.path().join("png/frame_0000.png"), b"not a png")
            .expect("tamper png");

        let report = simulate_realtime_args(tempdir.path())
            .simulate()
            .expect("report failure");

        assert!(!report.decoded);
        assert_eq!(report.attempts, 1);
        assert!(report.frames[0].error.is_some());
    }

    #[test]
    fn simulate_realtime_rejects_zero_fps_and_loops() {
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = simulate_realtime_args(tempdir.path());
        args.simulate_fps = 0;
        assert!(args.simulate().expect_err("zero fps").to_string().contains("simulate-fps"));

        let mut args = simulate_realtime_args(tempdir.path());
        args.realtime_loops = 0;
        assert!(args
            .simulate()
            .expect_err("zero loops")
            .to_string()
            .contains("realtime-loops"));
    }

    #[test]
    fn simulate_realtime_rejects_renderer_backed_channel_until_wired() {
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = simulate_realtime_args(tempdir.path());
        args.channel = PetalEncodeChannelArg::KatakanaBase94;

        let err = args.simulate().expect_err("katakana realtime rejected");
        assert!(err.to_string().contains("renderer-backed"));
    }

    #[test]
    fn simulate_realtime_requires_manifest_or_grid_size() {
        let tempdir = tempfile::tempdir().expect("temp dir");
        std::fs::write(tempdir.path().join("frame.png"), b"not a png").expect("write frame");

        let err = simulate_realtime_args(tempdir.path())
            .simulate()
            .expect_err("missing grid");
        assert!(err.to_string().contains("--grid-size"));
    }

    #[test]
    fn score_styles_default_report_passes_gate() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let report = score_args(payload.path())
            .build_report()
            .expect("score report");

        assert_eq!(report.schema, SCORE_STYLES_SCHEMA);
        assert_eq!(report.recommended_style, DEFAULT_STYLE_NAME);
        assert!(report.gate_passed);
        assert_eq!(report.styles.len(), 1);
        let style = &report.styles[0];
        assert_eq!(style.capture_attempts, 12);
        assert_eq!(style.capture_successes, 12);
        assert_eq!(style.capture_success_ratio_bps, PETAL_CAPTURE_RATIO_BPS_SCALE);
        assert_eq!(report.grid.requested_grid_size, 0);
        assert_eq!(report.grid.resolved_grid_size, 33);
        assert!(style.capture_gate_passed);
        assert!(style.throughput_gate_passed);
    }

    #[test]
    fn score_styles_low_contrast_profile_fails_capture_gate() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let mut args = score_args(payload.path());
        args.dark_luma = Some(128);
        args.light_luma = Some(129);
        args.luminance_jitter = Some(0);
        args.attempts = Some(4);

        let report = args.build_report().expect("score report");
        let style = &report.styles[0];
        assert_eq!(style.capture_attempts, 4);
        assert_eq!(style.capture_successes, 0);
        assert!(!style.capture_gate_passed);
        assert!(!report.gate_passed);
    }

    #[test]
    fn score_styles_rejects_invalid_min_success_ratio() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let mut args = score_args(payload.path());
        args.min_success_ratio_bps = PETAL_CAPTURE_RATIO_BPS_SCALE + 1;

        let err = args.build_report().expect_err("invalid threshold");
        assert!(err.to_string().contains("min-success-ratio-bps"));
    }

    #[test]
    fn score_styles_rejects_zero_fps() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let mut args = score_args(payload.path());
        args.fps = 0;

        let err = args.build_report().expect_err("invalid fps");
        assert!(err.to_string().contains("--fps"));
    }

    #[test]
    fn score_styles_rejects_impossible_grid_geometry() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let mut args = score_args(payload.path());
        args.grid_size = 4;

        let err = args.build_report().expect_err("invalid grid");
        assert!(err.to_string().contains("grid geometry"));
    }

    #[test]
    fn score_styles_writes_json_report_and_summary() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let report_path = tempdir.path().join("nested/style_score.json");
        let mut args = score_args(payload.path());
        args.output_report = Some(report_path.clone());
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);

        args.run(&mut ctx).expect("run score styles");

        let report_bytes = fs::read(&report_path).expect("read report");
        let report: Value = norito::json::from_slice(&report_bytes).expect("report JSON");
        assert_eq!(
            report.get("recommended_style").and_then(Value::as_str),
            Some(DEFAULT_STYLE_NAME)
        );
        let summary: Value = norito::json::from_str(&ctx.printed[0]).expect("summary JSON");
        assert_eq!(
            summary.get("report_path").and_then(Value::as_str),
            Some(report_path.display().to_string().as_str())
        );
        assert_eq!(summary.get("gate_passed").and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn throughput_score_is_clamped_and_allows_zero_target() {
        assert_eq!(throughput_score_bps(1, 0), PETAL_CAPTURE_RATIO_BPS_SCALE);
        assert_eq!(throughput_score_bps(500, 1_000), 5_000);
        assert_eq!(
            throughput_score_bps(2_000, 1_000),
            PETAL_CAPTURE_RATIO_BPS_SCALE
        );
    }
}
