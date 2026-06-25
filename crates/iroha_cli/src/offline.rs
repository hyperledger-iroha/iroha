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
    PETAL_STREAM_GRID_SIZES, PetalStreamCaptureProfile, PetalStreamDecoder, PetalStreamEncoder,
    PetalStreamGrid, PetalStreamOptions, PetalStreamSampleGrid,
    render_petal_capture_samples_with_seed, score_petal_capture_profile_with_seed,
};
use norito::derive::JsonSerialize;

use crate::{Run, RunContext, cli_output::print_with_optional_text};

const SCORE_STYLES_SCHEMA: &str = "iroha.offline.petal.score_styles.v1";
const ENCODE_SCHEMA: &str = "iroha.offline.petal.encode.v1";
const EVAL_CAPTURE_SCHEMA: &str = "iroha.offline.petal.eval_capture.v1";
const SIMULATE_REALTIME_SCHEMA: &str = "iroha.offline.petal.simulate_realtime.v1";
const DEFAULT_SCORE_STYLES_FPS: u16 = 24;
const DEFAULT_TARGET_EFFECTIVE_BPS: u64 = 3_000;
const DEFAULT_ENCODE_DIMENSION: u32 = 1_024;
const MAX_ENCODE_DIMENSION: u32 = 4_096;
const MAX_ANIMATION_FRAMES: u16 = 120;
const DEFAULT_CAPTURE_DOWNSCALE_CELLS: u8 = 1;
const MAX_CAPTURE_DOWNSCALE_CELLS: u8 = 8;
const MAX_CAPTURE_BLUR_RADIUS: u8 = 4;
const MAX_CAPTURE_MOTION_BLUR_CELLS: u8 = 8;
const MAX_CAPTURE_NOISE_AMPLITUDE: u8 = 64;
const MIN_CAPTURE_EXPOSURE_OFFSET: i16 = -255;
const MAX_CAPTURE_EXPOSURE_OFFSET: i16 = 255;
const DEFAULT_STYLE_NAME: &str = "sora-temple";
const HIGH_CONTRAST_STYLE_NAME: &str = "sora-temple-high-contrast";
const KATAKANA_STYLE_NAME: &str = "sora-temple-command";
const KATAKANA_HIGH_CONTRAST_STYLE_NAME: &str = "sora-temple-command-high-contrast";

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
    /// Output format. PNG is implemented for all deterministic Petal channels.
    #[arg(long, value_enum, default_value_t = PetalEncodeFormatArg::Png)]
    format: PetalEncodeFormatArg,
    /// Renderer style. Binary grid uses `sora-temple`; Katakana uses `sora-temple-command`.
    #[arg(long, value_enum, default_value_t = PetalEncodeStyleArg::SoraTemple)]
    style: PetalEncodeStyleArg,
    /// Visual channel.
    #[arg(long, value_enum, default_value_t = PetalEncodeChannelArg::BinaryGrid)]
    channel: PetalEncodeChannelArg,
    /// Katakana layout preset. Defaults to balanced for katakana-base94.
    #[arg(long = "katakana-preset", value_enum)]
    katakana_preset: Option<PetalKatakanaPresetArg>,
    /// Square output dimension in pixels.
    #[arg(long, default_value_t = DEFAULT_ENCODE_DIMENSION)]
    dimension: u32,
    /// Frames per second metadata for downstream animation tooling.
    #[arg(long, default_value_t = DEFAULT_SCORE_STYLES_FPS)]
    fps: u16,
    /// Number of deterministic animation frames to render.
    #[arg(long = "animation-frames", default_value_t = 1)]
    animation_frames: u16,
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
    /// Visual channel used by the rendered input.
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
    /// Visual channel used by the rendered input.
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
    /// Visual channel to score.
    #[arg(long, value_enum, default_value_t = PetalEncodeChannelArg::BinaryGrid)]
    channel: PetalEncodeChannelArg,
    /// Katakana layout preset. Defaults to balanced for katakana-base94.
    #[arg(long = "katakana-preset", value_enum)]
    katakana_preset: Option<PetalKatakanaPresetArg>,
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

#[derive(Args, Debug, Clone)]
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
    /// Average capture samples into cell blocks before decoding.
    #[arg(long = "capture-downscale-cells", default_value_t = DEFAULT_CAPTURE_DOWNSCALE_CELLS)]
    downscale_cells: u8,
    /// Apply deterministic box blur over capture samples before decoding.
    #[arg(long = "capture-blur-radius", default_value_t = 0)]
    blur_radius: u8,
    /// Apply deterministic horizontal motion blur over capture samples before decoding.
    #[arg(long = "capture-motion-blur-cells", default_value_t = 0)]
    motion_blur_cells: u8,
    /// Apply deterministic per-cell sensor noise before decoding.
    #[arg(long = "capture-noise-amplitude", default_value_t = 0)]
    noise_amplitude: u8,
    /// Add a deterministic exposure offset to capture samples before decoding.
    #[arg(long = "capture-exposure-offset", default_value_t = 0)]
    exposure_offset: i16,
}

impl Default for CapturePerturbationArgs {
    fn default() -> Self {
        Self {
            perturb_capture: false,
            seed: 0,
            attempts: None,
            dark_luma: None,
            light_luma: None,
            luminance_jitter: None,
            downscale_cells: DEFAULT_CAPTURE_DOWNSCALE_CELLS,
            blur_radius: 0,
            motion_blur_cells: 0,
            noise_amplitude: 0,
            exposure_offset: 0,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PetalEncodeFormatArg {
    /// Single-frame PNG output.
    Png,
    /// Animated GIF output is implemented for the binary grid channel.
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

#[cfg(feature = "offline-visual-codecs")]
fn validate_gif_replay_available() -> Result<()> {
    Ok(())
}

#[cfg(not(feature = "offline-visual-codecs"))]
fn validate_gif_replay_available() -> Result<()> {
    Err(eyre!(
        "GIF replay requires building iroha_cli with --features offline-visual-codecs"
    ))
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PetalEncodeStyleArg {
    /// Decode-critical SORA temple grid layer.
    SoraTemple,
    /// Katakana command styling for the deterministic Katakana-base94 channel.
    SoraTempleCommand,
}

impl std::fmt::Display for PetalEncodeStyleArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SoraTemple => f.write_str("sora-temple"),
            Self::SoraTempleCommand => f.write_str(KATAKANA_STYLE_NAME),
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PetalEncodeChannelArg {
    /// Binary luminance grid suitable for scanner bring-up.
    BinaryGrid,
    /// Deterministic Katakana command-tile rendering with Petal luminance anchors.
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
pub(crate) enum PetalKatakanaPresetArg {
    /// Balanced command-tile density for ordinary camera distances.
    Balanced,
    /// Larger cells for longer-distance camera capture.
    DistanceSafe,
}

impl std::fmt::Display for PetalKatakanaPresetArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Balanced => f.write_str("balanced"),
            Self::DistanceSafe => f.write_str("distance-safe"),
        }
    }
}

impl PetalKatakanaPresetArg {
    fn min_grid_size(self) -> u16 {
        match self {
            Self::Balanced => 41,
            Self::DistanceSafe => 33,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PetalStyleSetArg {
    /// Current production Petal style family.
    SoraTempleDefault,
    /// Production Petal style family plus deterministic hardening variants.
    SoraTempleExpanded,
}

impl std::fmt::Display for PetalStyleSetArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SoraTempleDefault => f.write_str("sora-temple-default"),
            Self::SoraTempleExpanded => f.write_str("sora-temple-expanded"),
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
        let options = self.encode_options(&payload)?;
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
        let frames = write_grid_output(
            &format_dir,
            &grid,
            options,
            self.dimension,
            self.format,
            self.channel,
            self.fps,
            self.animation_frames,
            payload.len() as u64,
        )?;

        let report = EncodeReport {
            schema: ENCODE_SCHEMA.to_string(),
            input_path: self.input.display().to_string(),
            output_dir: self.output.display().to_string(),
            payload_bytes: payload.len() as u64,
            format: self.format.to_string(),
            style: self.style.to_string(),
            channel: self.channel.to_string(),
            katakana_preset: self
                .effective_katakana_preset()
                .map(|preset| preset.to_string()),
            fps: self.fps,
            animation_frames: self.animation_frames,
            dimension: self.dimension,
            grid: GridReport {
                requested_grid_size: self.grid_size,
                resolved_grid_size: grid.grid_size,
                border: self.border,
                anchor_size: self.anchor_size,
            },
            frames,
        };
        let manifest_path = self.output.join("manifest.json");
        write_encode_manifest(&manifest_path, &report)?;
        Ok(report)
    }

    fn validate(&self) -> Result<()> {
        match self.channel {
            PetalEncodeChannelArg::BinaryGrid => {
                if self.katakana_preset.is_some() {
                    return Err(eyre!(
                        "--katakana-preset requires --channel katakana-base94"
                    ));
                }
                if self.style != PetalEncodeStyleArg::SoraTemple {
                    return Err(eyre!(
                        "--channel binary-grid requires --style {}",
                        PetalEncodeStyleArg::SoraTemple
                    ));
                }
            }
            PetalEncodeChannelArg::KatakanaBase94 => {
                if self.style != PetalEncodeStyleArg::SoraTempleCommand {
                    return Err(eyre!(
                        "--channel katakana-base94 requires --style {}",
                        PetalEncodeStyleArg::SoraTempleCommand
                    ));
                }
            }
        }
        self.format.validate_available()?;
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
        if self.animation_frames == 0 {
            return Err(eyre!("--animation-frames must be greater than 0"));
        }
        if self.animation_frames > MAX_ANIMATION_FRAMES {
            return Err(eyre!(
                "--animation-frames must be <= {} for bounded offline rendering",
                MAX_ANIMATION_FRAMES
            ));
        }
        Ok(())
    }

    fn encode_options(&self, payload: &[u8]) -> Result<PetalStreamOptions> {
        let options = PetalStreamOptions {
            grid_size: self.grid_size,
            border: self.border,
            anchor_size: self.anchor_size,
        };
        let Some(preset) = self.effective_katakana_preset() else {
            return Ok(options);
        };
        if self.grid_size != 0 {
            return Ok(options);
        }
        resolve_katakana_preset_options(payload, options, preset)
    }

    fn effective_katakana_preset(&self) -> Option<PetalKatakanaPresetArg> {
        (self.channel == PetalEncodeChannelArg::KatakanaBase94).then_some(
            self.katakana_preset
                .unwrap_or(PetalKatakanaPresetArg::Balanced),
        )
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

        for (source_index, frame) in input.frames.iter().enumerate() {
            for capture_attempt_index in 0..attempts_per_frame {
                attempts = attempts.saturating_add(1);
                let result = evaluate_source_frame(
                    frame,
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
                    path: frame.report_path(),
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
            capture_downscale_cells: capture.map(|active| active.downscale_cells),
            capture_blur_radius: capture.map(|active| active.blur_radius),
            capture_motion_blur_cells: capture.map(|active| active.motion_blur_cells),
            capture_noise_amplitude: capture.map(|active| active.noise_amplitude),
            capture_exposure_offset: capture.map(|active| active.exposure_offset),
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
                summary.report_path, summary.decoded, summary.attempts, summary.planned_attempts
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
        let planned_attempts =
            planned_realtime_attempts(input.frames.len(), self.realtime_loops, attempts_per_frame)?;
        let mut attempts = 0u32;
        let mut decoded_payload: Option<Vec<u8>> = None;
        let mut first_success_loop_index = None;
        let mut first_success_source_index = None;
        let mut first_success_capture_attempt_index = None;
        let mut frames = Vec::new();

        for loop_index in 0..self.realtime_loops {
            for (source_index, frame) in input.frames.iter().enumerate() {
                for capture_attempt_index in 0..attempts_per_frame {
                    attempts = attempts.saturating_add(1);
                    let result = decode_source_frame_payload(
                        frame,
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
                        path: frame.report_path(),
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
            capture_downscale_cells: capture.map(|active| active.downscale_cells),
            capture_blur_radius: capture.map(|active| active.blur_radius),
            capture_motion_blur_cells: capture.map(|active| active.motion_blur_cells),
            capture_noise_amplitude: capture.map(|active| active.noise_amplitude),
            capture_exposure_offset: capture.map(|active| active.exposure_offset),
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
        self.validate()?;

        let base_profile = self.capture_profile();
        let options = self.score_options(&payload)?;
        let resolved_grid_size = PetalStreamEncoder::encode_grid(&payload, options)
            .map_err(|err| eyre!("failed to resolve Petal grid geometry: {err}"))?
            .grid_size;
        let effective_payload_bytes_per_second =
            (payload.len() as u64).saturating_mul(u64::from(self.fps));
        let effective_payload_bits_per_second =
            effective_payload_bytes_per_second.saturating_mul(8);
        let styles = self
            .style_set
            .style_candidates(self.channel, self.effective_katakana_preset(), base_profile)
            .into_iter()
            .map(|candidate| {
                score_style(
                    candidate,
                    &payload,
                    options,
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
            .max_by_key(|style| {
                (
                    style.overall_score_bps,
                    std::cmp::Reverse(style.style.clone()),
                )
            })
            .ok_or_else(|| eyre!("style set produced no candidates"))?;
        let recommended_style = recommended.style.clone();
        let recommended_overall_score_bps = recommended.overall_score_bps;
        let gate_passed = recommended.gate_passed;

        Ok(ScoreStylesReport {
            schema: SCORE_STYLES_SCHEMA.to_string(),
            input_path: self.input.display().to_string(),
            payload_bytes: payload.len() as u64,
            style_set: self.style_set.to_string(),
            channel: self.channel.to_string(),
            katakana_preset: self
                .effective_katakana_preset()
                .map(|preset| preset.to_string()),
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

    fn validate(&self) -> Result<()> {
        if self.channel == PetalEncodeChannelArg::BinaryGrid && self.katakana_preset.is_some() {
            return Err(eyre!(
                "--katakana-preset requires --channel katakana-base94"
            ));
        }
        if self.fps == 0 {
            return Err(eyre!("--fps must be greater than 0"));
        }
        if self.min_success_ratio_bps > PETAL_CAPTURE_RATIO_BPS_SCALE {
            return Err(eyre!("--min-success-ratio-bps exceeds 100%"));
        }
        Ok(())
    }

    fn score_options(&self, payload: &[u8]) -> Result<PetalStreamOptions> {
        let options = PetalStreamOptions {
            grid_size: self.grid_size,
            border: self.border,
            anchor_size: self.anchor_size,
        };
        let Some(preset) = self.effective_katakana_preset() else {
            return Ok(options);
        };
        if self.grid_size != 0 {
            return Ok(options);
        }
        resolve_katakana_preset_options(payload, options, preset)
    }

    fn effective_katakana_preset(&self) -> Option<PetalKatakanaPresetArg> {
        (self.channel == PetalEncodeChannelArg::KatakanaBase94).then_some(
            self.katakana_preset
                .unwrap_or(PetalKatakanaPresetArg::Balanced),
        )
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
    downscale_cells: u8,
    blur_radius: u8,
    motion_blur_cells: u8,
    noise_amplitude: u8,
    exposure_offset: i16,
}

impl CapturePerturbationArgs {
    fn validate(&self, profile_arg: PetalCaptureProfileArg) -> Result<()> {
        if !self.perturb_capture {
            if self.seed != 0
                || self.attempts.is_some()
                || self.dark_luma.is_some()
                || self.light_luma.is_some()
                || self.luminance_jitter.is_some()
                || self.downscale_cells != DEFAULT_CAPTURE_DOWNSCALE_CELLS
                || self.blur_radius != 0
                || self.motion_blur_cells != 0
                || self.noise_amplitude != 0
                || self.exposure_offset != 0
            {
                return Err(eyre!(
                    "capture perturbation overrides require --perturb-capture"
                ));
            }
            return Ok(());
        }
        let profile = self.profile(profile_arg);
        validate_capture_profile_for_cli(profile)?;
        if self.downscale_cells == 0 {
            return Err(eyre!("--capture-downscale-cells must be greater than 0"));
        }
        if self.downscale_cells > MAX_CAPTURE_DOWNSCALE_CELLS {
            return Err(eyre!(
                "--capture-downscale-cells must be <= {}",
                MAX_CAPTURE_DOWNSCALE_CELLS
            ));
        }
        if self.blur_radius > MAX_CAPTURE_BLUR_RADIUS {
            return Err(eyre!(
                "--capture-blur-radius must be <= {}",
                MAX_CAPTURE_BLUR_RADIUS
            ));
        }
        if self.motion_blur_cells > MAX_CAPTURE_MOTION_BLUR_CELLS {
            return Err(eyre!(
                "--capture-motion-blur-cells must be <= {}",
                MAX_CAPTURE_MOTION_BLUR_CELLS
            ));
        }
        if self.noise_amplitude > MAX_CAPTURE_NOISE_AMPLITUDE {
            return Err(eyre!(
                "--capture-noise-amplitude must be <= {}",
                MAX_CAPTURE_NOISE_AMPLITUDE
            ));
        }
        if !(MIN_CAPTURE_EXPOSURE_OFFSET..=MAX_CAPTURE_EXPOSURE_OFFSET)
            .contains(&self.exposure_offset)
        {
            return Err(eyre!(
                "--capture-exposure-offset must be between {} and {}",
                MIN_CAPTURE_EXPOSURE_OFFSET,
                MAX_CAPTURE_EXPOSURE_OFFSET
            ));
        }
        Ok(())
    }

    fn active(
        &self,
        profile_arg: PetalCaptureProfileArg,
    ) -> Result<Option<ActiveCapturePerturbation>> {
        self.validate(profile_arg)?;
        Ok(self.perturb_capture.then(|| ActiveCapturePerturbation {
            profile: self.profile(profile_arg),
            seed: self.seed,
            downscale_cells: self.downscale_cells,
            blur_radius: self.blur_radius,
            motion_blur_cells: self.motion_blur_cells,
            noise_amplitude: self.noise_amplitude,
            exposure_offset: self.exposure_offset,
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
        return Err(eyre!(
            "capture dark luminance must be lower than light luminance"
        ));
    }
    Ok(())
}

fn resolve_katakana_preset_options(
    payload: &[u8],
    options: PetalStreamOptions,
    preset: PetalKatakanaPresetArg,
) -> Result<PetalStreamOptions> {
    let mut last_error = None;
    for &candidate in PETAL_STREAM_GRID_SIZES {
        if candidate < preset.min_grid_size() {
            continue;
        }
        let candidate_options = PetalStreamOptions {
            grid_size: candidate,
            ..options
        };
        match PetalStreamEncoder::encode_grid(payload, candidate_options) {
            Ok(_) => return Ok(candidate_options),
            Err(err) => last_error = Some(err),
        }
    }
    Err(eyre!(
        "failed to resolve Katakana preset '{}' grid geometry: {}",
        preset,
        last_error.map_or_else(
            || "no candidate grid sizes".to_string(),
            |err| err.to_string()
        )
    ))
}

impl PetalStyleSetArg {
    fn style_candidates(
        self,
        channel: PetalEncodeChannelArg,
        katakana_preset: Option<PetalKatakanaPresetArg>,
        base_profile: PetalStreamCaptureProfile,
    ) -> Vec<PetalStyleCandidate> {
        let base = PetalStyleCandidate {
            name: base_style_name(channel),
            channel,
            katakana_preset,
            capture_profile: base_profile,
        };
        match self {
            Self::SoraTempleDefault => vec![base],
            Self::SoraTempleExpanded => vec![
                base,
                PetalStyleCandidate {
                    name: high_contrast_style_name(channel),
                    channel,
                    katakana_preset,
                    capture_profile: high_contrast_capture_profile(base_profile),
                },
            ],
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PetalStyleCandidate {
    name: &'static str,
    channel: PetalEncodeChannelArg,
    katakana_preset: Option<PetalKatakanaPresetArg>,
    capture_profile: PetalStreamCaptureProfile,
}

fn base_style_name(channel: PetalEncodeChannelArg) -> &'static str {
    match channel {
        PetalEncodeChannelArg::BinaryGrid => DEFAULT_STYLE_NAME,
        PetalEncodeChannelArg::KatakanaBase94 => KATAKANA_STYLE_NAME,
    }
}

fn high_contrast_style_name(channel: PetalEncodeChannelArg) -> &'static str {
    match channel {
        PetalEncodeChannelArg::BinaryGrid => HIGH_CONTRAST_STYLE_NAME,
        PetalEncodeChannelArg::KatakanaBase94 => KATAKANA_HIGH_CONTRAST_STYLE_NAME,
    }
}

fn high_contrast_capture_profile(
    mut profile: PetalStreamCaptureProfile,
) -> PetalStreamCaptureProfile {
    profile.dark_luma = profile.dark_luma.saturating_sub(64);
    profile.light_luma = profile.light_luma.saturating_add(64);
    profile
}

fn score_style(
    candidate: PetalStyleCandidate,
    payload: &[u8],
    options: PetalStreamOptions,
    seed: u64,
    min_success_ratio_bps: u16,
    effective_payload_bytes_per_second: u64,
    effective_payload_bits_per_second: u64,
    target_effective_bps: u64,
) -> Result<StyleScoreReport> {
    let score =
        score_petal_capture_profile_with_seed(payload, options, candidate.capture_profile, seed)
            .map_err(|err| eyre!("failed to score style '{}': {err}", candidate.name))?;
    let capture_success_ratio_bps = score.success_ratio_bps();
    let capture_gate_passed = score
        .meets_min_success_ratio_bps(min_success_ratio_bps)
        .map_err(|err| {
            eyre!(
                "failed to evaluate capture gate for style '{}': {err}",
                candidate.name
            )
        })?;
    let throughput_score_bps =
        throughput_score_bps(effective_payload_bits_per_second, target_effective_bps);
    let throughput_gate_passed =
        target_effective_bps == 0 || effective_payload_bits_per_second >= target_effective_bps;
    let overall_score_bps = capture_success_ratio_bps.min(throughput_score_bps);
    Ok(StyleScoreReport {
        style: candidate.name.to_string(),
        channel: candidate.channel.to_string(),
        katakana_preset: candidate.katakana_preset.map(|preset| preset.to_string()),
        capture_profile: CaptureProfileReport::from(candidate.capture_profile),
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
    let numerator = u64::from(attempts) * u64::from(min_success_ratio_bps);
    let required = numerator.div_ceil(u64::from(PETAL_CAPTURE_RATIO_BPS_SCALE));
    u32::try_from(required).map_err(|_| eyre!("required capture successes exceed u32"))
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
    frames: Vec<EvalCaptureSourceFrame>,
}

#[derive(Clone, Debug)]
struct EvalCaptureSourceFrame {
    path: PathBuf,
    format: PetalEncodeFormatArg,
    encoded_frame_index: u16,
}

impl EvalCaptureSourceFrame {
    fn png(path: PathBuf) -> Self {
        Self {
            path,
            format: PetalEncodeFormatArg::Png,
            encoded_frame_index: 0,
        }
    }

    fn report_path(&self) -> String {
        if self.format == PetalEncodeFormatArg::Gif {
            format!("{}#frame_{}", self.path.display(), self.encoded_frame_index)
        } else {
            self.path.display().to_string()
        }
    }
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
    let mut frames = collect_png_frames(&args.input_dir)?
        .into_iter()
        .map(EvalCaptureSourceFrame::png)
        .collect::<Vec<_>>();
    if frames.is_empty() {
        return Err(eyre!("no PNG frames found in {}", args.input_dir.display()));
    }
    frames.sort_by(|left, right| left.path.cmp(&right.path));
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
    let format = parse_manifest_format(json_str_field(&value, "format")?)?;
    if format == PetalEncodeFormatArg::Gif {
        validate_gif_replay_available()?;
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
    let payload_bytes = value
        .get("payload_bytes")
        .and_then(norito::json::Value::as_u64);
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
        let path = resolve_manifest_frame_path(manifest_parent, path);
        let encoded_frame_count =
            optional_json_u16_field(frame, "encoded_frame_count")?.unwrap_or(1);
        if encoded_frame_count == 0 {
            return Err(eyre!(
                "manifest frame {idx} encoded_frame_count must be > 0"
            ));
        }
        match format {
            PetalEncodeFormatArg::Png => {
                if encoded_frame_count != 1 {
                    return Err(eyre!(
                        "PNG manifest frame {idx} encoded_frame_count must be 1"
                    ));
                }
                frames.push(EvalCaptureSourceFrame::png(path));
            }
            PetalEncodeFormatArg::Gif => {
                for encoded_frame_index in 0..encoded_frame_count {
                    frames.push(EvalCaptureSourceFrame {
                        path: path.clone(),
                        format,
                        encoded_frame_index,
                    });
                }
            }
        }
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
    if path.is_absolute() {
        path
    } else {
        manifest_parent.join(path)
    }
}

fn evaluate_source_frame(
    frame: &EvalCaptureSourceFrame,
    options: PetalStreamOptions,
    expected_payload_bytes: Option<u64>,
    capture: Option<ActiveCapturePerturbation>,
    capture_attempt_index: u16,
) -> Result<usize> {
    decode_source_frame_payload(
        frame,
        options,
        expected_payload_bytes,
        capture,
        capture_attempt_index,
    )
    .map(|payload| payload.len())
}

fn decode_source_frame_payload(
    frame: &EvalCaptureSourceFrame,
    options: PetalStreamOptions,
    expected_payload_bytes: Option<u64>,
    capture: Option<ActiveCapturePerturbation>,
    capture_attempt_index: u16,
) -> Result<Vec<u8>> {
    let mut samples = read_petal_frame_samples(frame, options.grid_size)?;
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
    let rendered = render_petal_capture_samples_with_seed(
        &grid,
        capture.profile,
        capture_attempt_index,
        capture.seed,
    )
    .map_err(|err| eyre!("failed to perturb Petal samples: {err}"))?;
    apply_capture_sample_models(rendered, capture, capture_attempt_index)
}

fn apply_capture_sample_models(
    samples: PetalStreamSampleGrid,
    capture: ActiveCapturePerturbation,
    capture_attempt_index: u16,
) -> Result<PetalStreamSampleGrid> {
    let mut values = samples.samples;
    if capture.downscale_cells > DEFAULT_CAPTURE_DOWNSCALE_CELLS {
        values = downscale_sample_cells(&values, samples.grid_size, capture.downscale_cells)?;
    }
    if capture.blur_radius > 0 {
        values = box_blur_sample_cells(&values, samples.grid_size, capture.blur_radius)?;
    }
    if capture.motion_blur_cells > 0 {
        values = motion_blur_sample_cells(&values, samples.grid_size, capture.motion_blur_cells)?;
    }
    if capture.noise_amplitude > 0 {
        apply_sample_noise(
            &mut values,
            capture.noise_amplitude,
            capture.seed,
            capture_attempt_index,
        );
    }
    if capture.exposure_offset != 0 {
        apply_sample_exposure_offset(&mut values, capture.exposure_offset);
    }
    PetalStreamSampleGrid::new(samples.grid_size, values)
        .map_err(|err| eyre!("failed to apply Petal capture sample models: {err}"))
}

fn downscale_sample_cells(samples: &[u8], grid_size: u16, factor: u8) -> Result<Vec<u8>> {
    if factor == 0 {
        return Err(eyre!("capture downscale factor must be greater than 0"));
    }
    let grid_size = usize::from(grid_size);
    if samples.len() != grid_size * grid_size {
        return Err(eyre!("capture downscale sample grid mismatch"));
    }
    let factor = usize::from(factor);
    let mut out = vec![0u8; samples.len()];
    for y in 0..grid_size {
        let block_y = (y / factor) * factor;
        let y_end = (block_y + factor).min(grid_size);
        for x in 0..grid_size {
            let block_x = (x / factor) * factor;
            let x_end = (block_x + factor).min(grid_size);
            let mut sum = 0u32;
            let mut count = 0u32;
            for yy in block_y..y_end {
                for xx in block_x..x_end {
                    sum += u32::from(samples[yy * grid_size + xx]);
                    count += 1;
                }
            }
            out[y * grid_size + x] = u8::try_from(sum / count).unwrap_or(u8::MAX);
        }
    }
    Ok(out)
}

fn box_blur_sample_cells(samples: &[u8], grid_size: u16, radius: u8) -> Result<Vec<u8>> {
    let grid_size = usize::from(grid_size);
    if samples.len() != grid_size * grid_size {
        return Err(eyre!("capture blur sample grid mismatch"));
    }
    if radius == 0 {
        return Ok(samples.to_vec());
    }
    let radius = usize::from(radius);
    let mut out = vec![0u8; samples.len()];
    for y in 0..grid_size {
        let y_start = y.saturating_sub(radius);
        let y_end = (y + radius + 1).min(grid_size);
        for x in 0..grid_size {
            let x_start = x.saturating_sub(radius);
            let x_end = (x + radius + 1).min(grid_size);
            let mut sum = 0u32;
            let mut count = 0u32;
            for yy in y_start..y_end {
                for xx in x_start..x_end {
                    sum += u32::from(samples[yy * grid_size + xx]);
                    count += 1;
                }
            }
            out[y * grid_size + x] = u8::try_from(sum / count).unwrap_or(u8::MAX);
        }
    }
    Ok(out)
}

fn motion_blur_sample_cells(samples: &[u8], grid_size: u16, radius: u8) -> Result<Vec<u8>> {
    let grid_size = usize::from(grid_size);
    if samples.len() != grid_size * grid_size {
        return Err(eyre!("capture motion blur sample grid mismatch"));
    }
    if radius == 0 {
        return Ok(samples.to_vec());
    }
    let radius = usize::from(radius);
    let mut out = vec![0u8; samples.len()];
    for y in 0..grid_size {
        for x in 0..grid_size {
            let x_start = x.saturating_sub(radius);
            let x_end = (x + radius + 1).min(grid_size);
            let mut sum = 0u32;
            let mut count = 0u32;
            for xx in x_start..x_end {
                sum += u32::from(samples[y * grid_size + xx]);
                count += 1;
            }
            out[y * grid_size + x] = u8::try_from(sum / count).unwrap_or(u8::MAX);
        }
    }
    Ok(out)
}

fn apply_sample_noise(samples: &mut [u8], amplitude: u8, seed: u64, attempt: u16) {
    if amplitude == 0 {
        return;
    }
    let span = u32::from(amplitude) * 2 + 1;
    let seed32 = (seed as u32) ^ ((seed >> 32) as u32);
    for (index, sample) in samples.iter_mut().enumerate() {
        let mixed = (index as u32)
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223)
            .wrapping_add(u32::from(attempt).wrapping_mul(97_531))
            .wrapping_add(seed32.rotate_left(u32::from(attempt % 31)))
            .rotate_left((index as u32 + u32::from(attempt)) % 17);
        let offset = i16::try_from(mixed % span).unwrap_or(i16::MAX) - i16::from(amplitude);
        *sample = (i16::from(*sample) + offset).clamp(0, 255) as u8;
    }
}

fn apply_sample_exposure_offset(samples: &mut [u8], offset: i16) {
    for sample in samples {
        *sample = (i16::from(*sample) + offset).clamp(0, 255) as u8;
    }
}

fn read_petal_frame_samples(
    frame: &EvalCaptureSourceFrame,
    grid_size: u16,
) -> Result<PetalStreamSampleGrid> {
    match frame.format {
        PetalEncodeFormatArg::Png => read_petal_png_samples(&frame.path, grid_size),
        PetalEncodeFormatArg::Gif => {
            read_petal_gif_samples(&frame.path, grid_size, frame.encoded_frame_index)
        }
    }
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

#[cfg(feature = "offline-visual-codecs")]
fn read_petal_gif_samples(
    path: &Path,
    grid_size: u16,
    encoded_frame_index: u16,
) -> Result<PetalStreamSampleGrid> {
    use image::{AnimationDecoder, codecs::gif::GifDecoder};

    if grid_size == 0 {
        return Err(eyre!("grid size must be greater than 0"));
    }
    let file =
        fs::File::open(path).wrap_err_with(|| format!("failed to open GIF {}", path.display()))?;
    let decoder = GifDecoder::new(BufReader::new(file))
        .wrap_err_with(|| format!("failed to read GIF info {}", path.display()))?;
    let frame = decoder
        .into_frames()
        .nth(usize::from(encoded_frame_index))
        .ok_or_else(|| {
            eyre!(
                "GIF {} missing encoded frame {}",
                path.display(),
                encoded_frame_index
            )
        })?
        .wrap_err_with(|| {
            format!(
                "failed to decode GIF frame {} from {}",
                encoded_frame_index,
                path.display()
            )
        })?;
    let image = frame.into_buffer();
    let width = image.width();
    let height = image.height();
    if width != height {
        return Err(eyre!("Petal GIF frame must be square"));
    }
    if width < u32::from(grid_size) {
        return Err(eyre!(
            "GIF dimension {} is smaller than grid size {}",
            width,
            grid_size
        ));
    }
    let samples = sample_rgba_luminance_grid(image.as_raw(), width, height, grid_size)?;
    PetalStreamSampleGrid::new(grid_size, samples)
        .map_err(|err| eyre!("failed to build Petal sample grid: {err}"))
}

#[cfg(not(feature = "offline-visual-codecs"))]
fn read_petal_gif_samples(
    _path: &Path,
    _grid_size: u16,
    _encoded_frame_index: u16,
) -> Result<PetalStreamSampleGrid> {
    validate_gif_replay_available()?;
    unreachable!("validate_gif_replay_available always errors without offline-visual-codecs")
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

#[cfg(feature = "offline-visual-codecs")]
fn sample_rgba_luminance_grid(
    bytes: &[u8],
    width: u32,
    height: u32,
    grid_size: u16,
) -> Result<Vec<u8>> {
    let line_size = width as usize * 4;
    if bytes.len() < line_size * height as usize {
        return Err(eyre!("GIF sampling input is truncated"));
    }
    let mut samples = Vec::with_capacity(grid_size as usize * grid_size as usize);
    for gy in 0..u32::from(grid_size) {
        let y = cell_center_pixel(gy, height, grid_size);
        for gx in 0..u32::from(grid_size) {
            let x = cell_center_pixel(gx, width, grid_size);
            let idx = y as usize * line_size + x as usize * 4;
            let rgb = rgb_luminance(&bytes[idx..idx + 3])?;
            let alpha = bytes
                .get(idx + 3)
                .copied()
                .ok_or_else(|| eyre!("truncated GIF rgba pixel"))?;
            samples.push(composite_luminance_over_white(rgb, alpha));
        }
    }
    let expected = grid_size as usize * grid_size as usize;
    if samples.len() != expected || width == 0 {
        return Err(eyre!("GIF sampling produced invalid grid"));
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

fn parse_manifest_format(raw: &str) -> Result<PetalEncodeFormatArg> {
    match raw {
        "png" => Ok(PetalEncodeFormatArg::Png),
        "gif" => Ok(PetalEncodeFormatArg::Gif),
        _ => Err(eyre!(
            "unsupported Petal manifest format '{raw}', expected png or gif"
        )),
    }
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

fn optional_json_u16_field(value: &norito::json::Value, field: &str) -> Result<Option<u16>> {
    let Some(raw) = value.get(field) else {
        return Ok(None);
    };
    let raw = raw
        .as_u64()
        .ok_or_else(|| eyre!("manifest field `{field}` must be an integer"))?;
    u16::try_from(raw)
        .map(Some)
        .map_err(|_| eyre!("manifest field `{field}` exceeds u16"))
}

fn json_u8_field(value: &norito::json::Value, field: &str) -> Result<u8> {
    let raw = value
        .get(field)
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| eyre!("manifest missing integer field `{field}`"))?;
    u8::try_from(raw).map_err(|_| eyre!("manifest field `{field}` exceeds u8"))
}

fn write_grid_output(
    format_dir: &Path,
    grid: &PetalStreamGrid,
    options: PetalStreamOptions,
    dimension: u32,
    format: PetalEncodeFormatArg,
    channel: PetalEncodeChannelArg,
    fps: u16,
    animation_frames: u16,
    payload_bytes: u64,
) -> Result<Vec<EncodeFrameReport>> {
    match format {
        PetalEncodeFormatArg::Png => write_grid_png_frames(
            format_dir,
            grid,
            options,
            dimension,
            channel,
            animation_frames,
            payload_bytes,
        ),
        PetalEncodeFormatArg::Gif => write_grid_gif_file(
            format_dir,
            grid,
            options,
            dimension,
            channel,
            fps,
            animation_frames,
            payload_bytes,
        ),
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

fn render_katakana_base94_rgb(
    grid: &PetalStreamGrid,
    options: PetalStreamOptions,
    dimension: u32,
    frame_index: u16,
) -> Result<Vec<u8>> {
    let mut pixels = vec![255u8; dimension as usize * dimension as usize * 3];
    for cell_y in 0..grid.grid_size {
        let y0 = u32::from(cell_y).saturating_mul(dimension) / u32::from(grid.grid_size);
        let y1 = (u32::from(cell_y) + 1).saturating_mul(dimension) / u32::from(grid.grid_size);
        for cell_x in 0..grid.grid_size {
            let x0 = u32::from(cell_x).saturating_mul(dimension) / u32::from(grid.grid_size);
            let x1 = (u32::from(cell_x) + 1).saturating_mul(dimension) / u32::from(grid.grid_size);
            let cell = grid
                .get(cell_x, cell_y)
                .ok_or_else(|| eyre!("failed to sample Petal grid cell"))?;
            let calibration = is_petal_calibration_cell(cell_x, cell_y, grid.grid_size, options);
            let symbol = katakana_base94_symbol_index(grid, cell_x, cell_y, frame_index);
            let center_x = cell_center_pixel(u32::from(cell_x), dimension, grid.grid_size);
            let center_y = cell_center_pixel(u32::from(cell_y), dimension, grid.grid_size);
            let width = x1.saturating_sub(x0).max(1);
            let height = y1.saturating_sub(y0).max(1);
            for y in y0..y1.max(y0 + 1).min(dimension) {
                for x in x0..x1.max(x0 + 1).min(dimension) {
                    let local_x = ((x.saturating_sub(x0) * 8) / width).min(7) as u8;
                    let local_y = ((y.saturating_sub(y0) * 8) / height).min(7) as u8;
                    let stroke = !calibration
                        && !(x == center_x && y == center_y)
                        && katakana_tile_stroke(symbol, local_x, local_y);
                    let rgb = if stroke {
                        katakana_accent_rgb(cell, symbol)
                    } else {
                        katakana_base_rgb(cell)
                    };
                    let idx = (y as usize * dimension as usize + x as usize) * 3;
                    pixels[idx..idx + 3].copy_from_slice(&rgb);
                }
            }
        }
    }
    Ok(pixels)
}

fn is_petal_calibration_cell(x: u16, y: u16, grid_size: u16, options: PetalStreamOptions) -> bool {
    let border = u16::from(options.border);
    let anchor = u16::from(options.anchor_size);
    if x < border
        || y < border
        || x >= grid_size.saturating_sub(border)
        || y >= grid_size.saturating_sub(border)
    {
        return true;
    }
    let near_left = x >= border && x < border.saturating_add(anchor);
    let near_top = y >= border && y < border.saturating_add(anchor);
    let far_start = grid_size.saturating_sub(border.saturating_add(anchor));
    let near_right = x >= far_start && x < far_start.saturating_add(anchor);
    let near_bottom = y >= far_start && y < far_start.saturating_add(anchor);
    (near_left || near_right) && (near_top || near_bottom)
}

fn katakana_base94_symbol_index(grid: &PetalStreamGrid, x: u16, y: u16, frame_index: u16) -> u8 {
    let mut mixed = u32::from(frame_index)
        .wrapping_mul(97)
        .wrapping_add(u32::from(x).wrapping_mul(17))
        .wrapping_add(u32::from(y).wrapping_mul(31));
    for dy in -1i16..=1 {
        for dx in -1i16..=1 {
            let nx = i32::from(x) + i32::from(dx);
            let ny = i32::from(y) + i32::from(dy);
            let bit = if nx >= 0
                && ny >= 0
                && nx < i32::from(grid.grid_size)
                && ny < i32::from(grid.grid_size)
            {
                grid.get(nx as u16, ny as u16).unwrap_or(false)
            } else {
                false
            };
            mixed = mixed
                .wrapping_mul(33)
                .wrapping_add(u32::from(bit))
                .wrapping_add(((dx + 2) as u32) << 3)
                .wrapping_add((dy + 2) as u32);
        }
    }
    u8::try_from(mixed % 94).unwrap_or(0)
}

fn katakana_tile_stroke(symbol: u8, local_x: u8, local_y: u8) -> bool {
    if local_x == 0 || local_y == 0 || local_x == 7 || local_y == 7 {
        return false;
    }
    let bits = (u16::from(symbol) * 0x009d) ^ (u16::from(symbol) << 3) ^ 0x0155;
    ((bits & 0b0000_0000_01) != 0 && local_y == 1 && (2..=5).contains(&local_x))
        || ((bits & 0b0000_0000_10) != 0 && local_x == 5 && (1..=5).contains(&local_y))
        || ((bits & 0b0000_0001_00) != 0 && local_y == 5 && (1..=5).contains(&local_x))
        || ((bits & 0b0000_0010_00) != 0 && local_x == 2 && (2..=6).contains(&local_y))
        || ((bits & 0b0000_0100_00) != 0 && local_x.saturating_add(local_y) == 6)
        || ((bits & 0b0000_1000_00) != 0 && local_x == local_y && (2..=6).contains(&local_y))
        || ((bits & 0b0001_0000_00) != 0 && local_y == 3 && (2..=6).contains(&local_x))
        || ((bits & 0b0010_0000_00) != 0 && local_x == 6 && (2..=6).contains(&local_y))
        || ((bits & 0b0100_0000_00) != 0 && (local_x == 4 || local_x == 5) && local_y == 2)
        || ((bits & 0b1000_0000_00) != 0 && local_y == 6 && (3..=6).contains(&local_x))
}

fn katakana_base_rgb(cell: bool) -> [u8; 3] {
    if cell { [24, 26, 32] } else { [238, 236, 224] }
}

fn katakana_accent_rgb(cell: bool, symbol: u8) -> [u8; 3] {
    if cell {
        [
            54u8.saturating_add(symbol % 24),
            34u8.saturating_add((symbol / 3) % 20),
            74u8.saturating_add((symbol / 5) % 24),
        ]
    } else {
        [
            180u8.saturating_add(symbol % 20),
            196u8.saturating_add((symbol / 3) % 24),
            204u8.saturating_add((symbol / 5) % 20),
        ]
    }
}

fn write_grid_png_frames(
    format_dir: &Path,
    grid: &PetalStreamGrid,
    options: PetalStreamOptions,
    dimension: u32,
    channel: PetalEncodeChannelArg,
    animation_frames: u16,
    payload_bytes: u64,
) -> Result<Vec<EncodeFrameReport>> {
    let mut frames = Vec::with_capacity(usize::from(animation_frames));
    for index in 0..animation_frames {
        let frame_name = format!("frame_{index:04}.png");
        let frame_path = format_dir.join(&frame_name);
        write_grid_png(&frame_path, grid, options, dimension, channel, index)?;
        frames.push(EncodeFrameReport {
            index,
            path: format!("{}/{frame_name}", PetalEncodeFormatArg::Png),
            payload_bytes,
            encoded_frame_count: 1,
        });
    }
    Ok(frames)
}

fn write_grid_png(
    path: &Path,
    grid: &PetalStreamGrid,
    options: PetalStreamOptions,
    dimension: u32,
    channel: PetalEncodeChannelArg,
    frame_index: u16,
) -> Result<()> {
    let file = fs::File::create(path)
        .wrap_err_with(|| format!("failed to create PNG {}", path.display()))?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, dimension, dimension);
    encoder.set_color(match channel {
        PetalEncodeChannelArg::BinaryGrid => png::ColorType::Grayscale,
        PetalEncodeChannelArg::KatakanaBase94 => png::ColorType::Rgb,
    });
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .wrap_err_with(|| format!("failed to write PNG header {}", path.display()))?;
    let pixels = match channel {
        PetalEncodeChannelArg::BinaryGrid => render_grid_luma(grid, dimension)?,
        PetalEncodeChannelArg::KatakanaBase94 => {
            render_katakana_base94_rgb(grid, options, dimension, frame_index)?
        }
    };
    writer
        .write_image_data(&pixels)
        .wrap_err_with(|| format!("failed to write PNG pixels {}", path.display()))
}

#[cfg(feature = "offline-visual-codecs")]
fn render_frame_rgba(
    grid: &PetalStreamGrid,
    options: PetalStreamOptions,
    dimension: u32,
    channel: PetalEncodeChannelArg,
    frame_index: u16,
) -> Result<Vec<u8>> {
    match channel {
        PetalEncodeChannelArg::BinaryGrid => {
            let luma = render_grid_luma(grid, dimension)?;
            let mut rgba = Vec::with_capacity(luma.len() * 4);
            for value in luma {
                rgba.extend_from_slice(&[value, value, value, u8::MAX]);
            }
            Ok(rgba)
        }
        PetalEncodeChannelArg::KatakanaBase94 => {
            let rgb = render_katakana_base94_rgb(grid, options, dimension, frame_index)?;
            let mut rgba = Vec::with_capacity((rgb.len() / 3) * 4);
            for pixel in rgb.chunks_exact(3) {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], u8::MAX]);
            }
            Ok(rgba)
        }
    }
}

fn write_grid_gif_file(
    format_dir: &Path,
    grid: &PetalStreamGrid,
    options: PetalStreamOptions,
    dimension: u32,
    channel: PetalEncodeChannelArg,
    fps: u16,
    animation_frames: u16,
    payload_bytes: u64,
) -> Result<Vec<EncodeFrameReport>> {
    let path = format_dir.join("frame_0000.gif");
    write_grid_gif(
        &path,
        grid,
        options,
        dimension,
        channel,
        fps,
        animation_frames,
    )?;
    Ok(vec![EncodeFrameReport {
        index: 0,
        path: format!("{}/frame_0000.gif", PetalEncodeFormatArg::Gif),
        payload_bytes,
        encoded_frame_count: animation_frames,
    }])
}

#[cfg(feature = "offline-visual-codecs")]
fn write_grid_gif(
    path: &Path,
    grid: &PetalStreamGrid,
    options: PetalStreamOptions,
    dimension: u32,
    channel: PetalEncodeChannelArg,
    fps: u16,
    animation_frames: u16,
) -> Result<()> {
    use image::{
        Delay, Frame, RgbaImage,
        codecs::gif::{GifEncoder, Repeat},
    };

    let delay_ms = (1_000u32 / u32::from(fps)).max(1);
    let file = fs::File::create(path)
        .wrap_err_with(|| format!("failed to create GIF {}", path.display()))?;
    let mut encoder = GifEncoder::new(BufWriter::new(file));
    encoder
        .set_repeat(Repeat::Infinite)
        .wrap_err_with(|| format!("failed to set GIF repeat {}", path.display()))?;
    for frame_index in 0..animation_frames {
        let rgba = render_frame_rgba(grid, options, dimension, channel, frame_index)?;
        let image = RgbaImage::from_raw(dimension, dimension, rgba)
            .ok_or_else(|| eyre!("failed to build GIF frame buffer"))?;
        let frame = Frame::from_parts(image, 0, 0, Delay::from_numer_denom_ms(delay_ms, 1));
        encoder
            .encode_frame(frame)
            .wrap_err_with(|| format!("failed to write GIF frame {}", path.display()))?;
    }
    Ok(())
}

#[cfg(not(feature = "offline-visual-codecs"))]
fn write_grid_gif(
    _path: &Path,
    _grid: &PetalStreamGrid,
    _options: PetalStreamOptions,
    _dimension: u32,
    _channel: PetalEncodeChannelArg,
    _fps: u16,
    _animation_frames: u16,
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
    katakana_preset: Option<String>,
    fps: u16,
    animation_frames: u16,
    dimension: u32,
    grid: GridReport,
    frames: Vec<EncodeFrameReport>,
}

#[derive(Clone, Debug, JsonSerialize)]
struct EncodeFrameReport {
    index: u16,
    path: String,
    payload_bytes: u64,
    encoded_frame_count: u16,
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
    capture_downscale_cells: Option<u8>,
    capture_blur_radius: Option<u8>,
    capture_motion_blur_cells: Option<u8>,
    capture_noise_amplitude: Option<u8>,
    capture_exposure_offset: Option<i16>,
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
    capture_downscale_cells: Option<u8>,
    capture_blur_radius: Option<u8>,
    capture_motion_blur_cells: Option<u8>,
    capture_noise_amplitude: Option<u8>,
    capture_exposure_offset: Option<i16>,
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
    channel: String,
    katakana_preset: Option<String>,
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
    channel: String,
    katakana_preset: Option<String>,
    capture_profile: CaptureProfileReport,
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
            channel: PetalEncodeChannelArg::BinaryGrid,
            katakana_preset: None,
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

    fn style_score<'a>(report: &'a ScoreStylesReport, name: &str) -> &'a StyleScoreReport {
        report
            .styles
            .iter()
            .find(|style| style.style == name)
            .unwrap_or_else(|| panic!("missing style score for {name}"))
    }

    fn encode_args(input: &Path, output: &Path) -> EncodeArgs {
        EncodeArgs {
            input: input.to_path_buf(),
            output: output.to_path_buf(),
            format: PetalEncodeFormatArg::Png,
            style: PetalEncodeStyleArg::SoraTemple,
            channel: PetalEncodeChannelArg::BinaryGrid,
            katakana_preset: None,
            dimension: DEFAULT_ENCODE_DIMENSION,
            fps: DEFAULT_SCORE_STYLES_FPS,
            animation_frames: 1,
            grid_size: 0,
            border: iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_BORDER,
            anchor_size: iroha::data_model::petal_stream::PETAL_STREAM_DEFAULT_ANCHOR,
        }
    }

    fn katakana_encode_args(input: &Path, output: &Path) -> EncodeArgs {
        let mut args = encode_args(input, output);
        args.style = PetalEncodeStyleArg::SoraTempleCommand;
        args.channel = PetalEncodeChannelArg::KatakanaBase94;
        args.dimension = 256;
        args
    }

    fn decode_encoded_frame(path: &Path, grid: GridReport) -> Vec<u8> {
        let options = PetalStreamOptions {
            grid_size: grid.resolved_grid_size,
            border: grid.border,
            anchor_size: grid.anchor_size,
        };
        let samples = read_petal_png_samples(path, options.grid_size).expect("read samples");
        decode_petal_samples_payload(&samples, options).expect("decode samples")
    }

    fn decoded_png_color_type(path: &Path) -> png::ColorType {
        let file = fs::File::open(path).expect("open PNG");
        let decoder = png::Decoder::new(BufReader::new(file));
        let mut reader = decoder.read_info().expect("read PNG info");
        let mut buffer = vec![0; reader.output_buffer_size().expect("output buffer")];
        reader
            .next_frame(&mut buffer)
            .expect("decode PNG")
            .color_type
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
        assert_eq!(
            output.get("schema").and_then(Value::as_str),
            Some(ENCODE_SCHEMA)
        );
        assert_eq!(
            output.get("animation_frames").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(output["grid"]["resolved_grid_size"], Value::from(33u64));
        assert_eq!(
            output["frames"][0]["path"],
            Value::from("png/frame_0000.png")
        );
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
        assert_eq!(
            manifest["frames"][0]["path"],
            Value::from("png/frame_0000.png")
        );
        assert_eq!(
            manifest["frames"][0]["encoded_frame_count"],
            Value::from(1u64)
        );
    }

    #[test]
    fn encode_png_animation_frames_write_multiple_manifest_entries() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = encode_args(payload.path(), tempdir.path());
        args.dimension = 128;
        args.animation_frames = 3;

        let report = args.encode().expect("encode png animation frames");

        assert_eq!(report.animation_frames, 3);
        assert_eq!(report.frames.len(), 3);
        for index in 0..3 {
            let frame_path = tempdir.path().join(format!("png/frame_{index:04}.png"));
            assert!(frame_path.exists(), "expected {}", frame_path.display());
            assert_eq!(report.frames[index].index, index as u16);
            assert_eq!(
                report.frames[index].path,
                format!("png/frame_{index:04}.png")
            );
            assert_eq!(report.frames[index].encoded_frame_count, 1);
        }
        let manifest: Value = norito::json::from_slice(
            &std::fs::read(tempdir.path().join("manifest.json")).expect("read manifest"),
        )
        .expect("manifest JSON");
        assert_eq!(manifest["animation_frames"], Value::from(3u64));
        assert_eq!(
            manifest["frames"].as_array().expect("frames array").len(),
            3
        );
        assert_eq!(
            manifest["frames"][2]["path"],
            Value::from("png/frame_0002.png")
        );
    }

    #[test]
    fn encode_katakana_base94_png_writes_manifest_and_decodes() {
        let payload = b"sora-temple-capture-baseline";
        let payload_file = write_payload(payload);
        let tempdir = tempfile::tempdir().expect("temp dir");
        let args = katakana_encode_args(payload_file.path(), tempdir.path());

        let report = args.encode().expect("encode katakana png");

        assert_eq!(report.style, KATAKANA_STYLE_NAME);
        assert_eq!(report.channel, "katakana-base94");
        assert_eq!(report.format, "png");
        assert_eq!(report.frames.len(), 1);
        assert_eq!(report.frames[0].path, "png/frame_0000.png");
        let frame_path = tempdir.path().join("png/frame_0000.png");
        assert_eq!(decoded_png_color_type(&frame_path), png::ColorType::Rgb);
        assert_eq!(decode_encoded_frame(&frame_path, report.grid), payload);
        let manifest: Value = norito::json::from_slice(
            &std::fs::read(tempdir.path().join("manifest.json")).expect("read manifest"),
        )
        .expect("manifest JSON");
        assert_eq!(
            manifest.get("channel").and_then(Value::as_str),
            Some("katakana-base94")
        );
        assert_eq!(
            manifest.get("style").and_then(Value::as_str),
            Some(KATAKANA_STYLE_NAME)
        );
        assert_eq!(
            manifest.get("katakana_preset").and_then(Value::as_str),
            Some("balanced")
        );
        assert_eq!(manifest["grid"]["resolved_grid_size"], Value::from(41u64));
    }

    #[test]
    fn encode_katakana_presets_select_expected_auto_grid_floor() {
        let payload = b"sora-temple-capture-baseline";
        let payload_file = write_payload(payload);
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = katakana_encode_args(payload_file.path(), tempdir.path());
        args.katakana_preset = Some(PetalKatakanaPresetArg::DistanceSafe);

        let report = args.encode().expect("encode distance safe katakana");

        assert_eq!(report.katakana_preset.as_deref(), Some("distance-safe"));
        assert_eq!(report.grid.requested_grid_size, 0);
        assert_eq!(report.grid.resolved_grid_size, 33);
        let frame_path = tempdir.path().join("png/frame_0000.png");
        assert_eq!(decode_encoded_frame(&frame_path, report.grid), payload);
    }

    #[test]
    fn encode_katakana_explicit_grid_overrides_preset_floor() {
        let payload = b"sora-temple-capture-baseline";
        let payload_file = write_payload(payload);
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = katakana_encode_args(payload_file.path(), tempdir.path());
        args.katakana_preset = Some(PetalKatakanaPresetArg::Balanced);
        args.grid_size = 37;

        let report = args.encode().expect("encode explicit katakana grid");

        assert_eq!(report.katakana_preset.as_deref(), Some("balanced"));
        assert_eq!(report.grid.requested_grid_size, 37);
        assert_eq!(report.grid.resolved_grid_size, 37);
    }

    #[test]
    fn encode_katakana_balanced_preset_grows_for_larger_payloads() {
        let payload = vec![0xa5; 180];
        let payload_file = write_payload(&payload);
        let tempdir = tempfile::tempdir().expect("temp dir");
        let args = katakana_encode_args(payload_file.path(), tempdir.path());

        let report = args.encode().expect("encode larger katakana payload");

        assert_eq!(report.katakana_preset.as_deref(), Some("balanced"));
        assert!(report.grid.resolved_grid_size >= 41);
        assert_eq!(
            decode_encoded_frame(&tempdir.path().join("png/frame_0000.png"), report.grid),
            payload
        );
    }

    #[test]
    fn encode_rejects_mismatched_style_channel() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = encode_args(payload.path(), tempdir.path());
        args.channel = PetalEncodeChannelArg::KatakanaBase94;

        let err = args.encode().expect_err("katakana style rejected");
        assert!(err.to_string().contains("sora-temple-command"));

        let mut args = encode_args(payload.path(), tempdir.path());
        args.style = PetalEncodeStyleArg::SoraTempleCommand;
        let err = args.encode().expect_err("binary style rejected");
        assert!(err.to_string().contains("binary-grid"));

        let mut args = encode_args(payload.path(), tempdir.path());
        args.katakana_preset = Some(PetalKatakanaPresetArg::DistanceSafe);
        let err = args.encode().expect_err("binary preset rejected");
        assert!(err.to_string().contains("katakana-preset"));
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

    #[cfg(not(feature = "offline-visual-codecs"))]
    #[test]
    fn encode_gif_katakana_requires_visual_codecs_feature() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = katakana_encode_args(payload.path(), tempdir.path());
        args.format = PetalEncodeFormatArg::Gif;

        let err = args.encode().expect_err("katakana gif feature rejected");
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
        assert_eq!(report.animation_frames, 1);
        assert_eq!(report.frames[0].path, "gif/frame_0000.gif");
        assert_eq!(report.frames[0].encoded_frame_count, 1);
        let manifest: Value = norito::json::from_slice(
            &std::fs::read(tempdir.path().join("manifest.json")).expect("read manifest"),
        )
        .expect("manifest JSON");
        assert_eq!(manifest.get("format").and_then(Value::as_str), Some("gif"));
        assert_eq!(
            manifest["frames"][0]["path"],
            Value::from("gif/frame_0000.gif")
        );
    }

    #[cfg(feature = "offline-visual-codecs")]
    #[test]
    fn encode_gif_animation_frames_are_encoded_inside_single_file() {
        use image::{AnimationDecoder, codecs::gif::GifDecoder};

        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = encode_args(payload.path(), tempdir.path());
        args.format = PetalEncodeFormatArg::Gif;
        args.dimension = 128;
        args.animation_frames = 4;

        let report = args.encode().expect("encode animated gif");

        let frame_path = tempdir.path().join("gif/frame_0000.gif");
        assert_eq!(report.frames.len(), 1);
        assert_eq!(report.frames[0].encoded_frame_count, 4);
        let file = std::fs::File::open(&frame_path).expect("open GIF");
        let decoder = GifDecoder::new(BufReader::new(file)).expect("decode GIF");
        let frames = decoder
            .into_frames()
            .collect_frames()
            .expect("collect GIF frames");
        assert_eq!(frames.len(), 4);
        let manifest: Value = norito::json::from_slice(
            &std::fs::read(tempdir.path().join("manifest.json")).expect("read manifest"),
        )
        .expect("manifest JSON");
        assert_eq!(manifest["animation_frames"], Value::from(4u64));
        assert_eq!(
            manifest["frames"][0]["path"],
            Value::from("gif/frame_0000.gif")
        );
        assert_eq!(
            manifest["frames"][0]["encoded_frame_count"],
            Value::from(4u64)
        );
    }

    #[cfg(feature = "offline-visual-codecs")]
    #[test]
    fn encode_gif_katakana_base94_writes_animated_command_tiles_with_visual_codecs_feature() {
        use image::{AnimationDecoder, codecs::gif::GifDecoder};

        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = katakana_encode_args(payload.path(), tempdir.path());
        args.format = PetalEncodeFormatArg::Gif;
        args.dimension = 128;
        args.animation_frames = 3;

        let report = args.encode().expect("encode katakana gif");

        assert_eq!(report.format, "gif");
        assert_eq!(report.channel, "katakana-base94");
        assert_eq!(report.style, KATAKANA_STYLE_NAME);
        assert_eq!(report.frames.len(), 1);
        assert_eq!(report.frames[0].path, "gif/frame_0000.gif");
        assert_eq!(report.frames[0].encoded_frame_count, 3);
        let frame_path = tempdir.path().join("gif/frame_0000.gif");
        let gif = std::fs::read(&frame_path).expect("read GIF");
        assert!(gif.starts_with(b"GIF89a") || gif.starts_with(b"GIF87a"));
        let file = std::fs::File::open(&frame_path).expect("open GIF");
        let decoder = GifDecoder::new(BufReader::new(file)).expect("decode GIF");
        let frames = decoder
            .into_frames()
            .collect_frames()
            .expect("collect GIF frames");
        assert_eq!(frames.len(), 3);
        assert_ne!(frames[0].buffer().as_raw(), frames[1].buffer().as_raw());
        let manifest: Value = norito::json::from_slice(
            &std::fs::read(tempdir.path().join("manifest.json")).expect("read manifest"),
        )
        .expect("manifest JSON");
        assert_eq!(manifest.get("format").and_then(Value::as_str), Some("gif"));
        assert_eq!(
            manifest.get("channel").and_then(Value::as_str),
            Some("katakana-base94")
        );
        assert_eq!(
            manifest.get("style").and_then(Value::as_str),
            Some(KATAKANA_STYLE_NAME)
        );
        assert_eq!(
            manifest["frames"][0]["encoded_frame_count"],
            Value::from(3u64)
        );
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
    fn encode_rejects_invalid_animation_frame_counts() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut args = encode_args(payload.path(), tempdir.path());
        args.animation_frames = 0;

        let err = args.encode().expect_err("zero animation frames rejected");
        assert!(err.to_string().contains("animation-frames"));

        let mut args = encode_args(payload.path(), tempdir.path());
        args.animation_frames = MAX_ANIMATION_FRAMES + 1;
        let err = args
            .encode()
            .expect_err("too many animation frames rejected");
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
        assert_eq!(
            output.get("gate_passed").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            output.get("success_ratio_bps").and_then(Value::as_u64),
            Some(u64::from(PETAL_CAPTURE_RATIO_BPS_SCALE))
        );
        assert_eq!(output["frames"][0]["success"], Value::from(true));
    }

    #[test]
    fn eval_capture_decodes_katakana_manifest_and_passes_gate() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        katakana_encode_args(payload.path(), tempdir.path())
            .encode()
            .expect("encode katakana png");
        let mut args = eval_capture_args(&tempdir.path().join("png"));
        args.channel = PetalEncodeChannelArg::KatakanaBase94;

        let report = args.evaluate().expect("eval katakana capture");

        assert_eq!(report.channel, "katakana-base94");
        assert_eq!(report.planned_attempts, 1);
        assert_eq!(report.successes, 1);
        assert!(report.gate_passed);
        assert!(report.frames[0].success);
    }

    #[cfg(not(feature = "offline-visual-codecs"))]
    #[test]
    fn eval_capture_rejects_gif_manifest_without_visual_codecs_feature() {
        let tempdir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            tempdir.path().join("manifest.json"),
            r#"{
  "schema": "iroha.offline.petal.encode.v1",
  "payload_bytes": 28,
  "format": "gif",
  "style": "sora-temple",
  "channel": "binary-grid",
  "grid": {
    "requested_grid_size": 0,
    "resolved_grid_size": 33,
    "border": 1,
    "anchor_size": 3
  },
  "frames": [
    {
      "index": 0,
      "path": "gif/frame_0000.gif",
      "payload_bytes": 28,
      "encoded_frame_count": 2
    }
  ]
}"#,
        )
        .expect("write manifest");

        let err = eval_capture_args(tempdir.path())
            .evaluate()
            .expect_err("gif replay rejected");

        assert!(err.to_string().contains("GIF replay"));
        assert!(err.to_string().contains("offline-visual-codecs"));
    }

    #[cfg(feature = "offline-visual-codecs")]
    #[test]
    fn eval_capture_decodes_binary_gif_manifest_frames_with_visual_codecs_feature() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut encode = encode_args(payload.path(), tempdir.path());
        encode.format = PetalEncodeFormatArg::Gif;
        encode.dimension = 128;
        encode.animation_frames = 3;
        encode.encode().expect("encode gif");

        let report = eval_capture_args(tempdir.path())
            .evaluate()
            .expect("eval gif capture");

        assert_eq!(report.channel, "binary-grid");
        assert_eq!(report.planned_attempts, 3);
        assert_eq!(report.successes, 3);
        assert!(report.gate_passed);
        assert_eq!(report.frames[0].source_index, 0);
        assert_eq!(report.frames[1].source_index, 1);
        assert!(report.frames[1].path.contains("#frame_1"));
    }

    #[cfg(feature = "offline-visual-codecs")]
    #[test]
    fn eval_capture_decodes_katakana_gif_manifest_frames_with_visual_codecs_feature() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut encode = katakana_encode_args(payload.path(), tempdir.path());
        encode.format = PetalEncodeFormatArg::Gif;
        encode.dimension = 128;
        encode.animation_frames = 3;
        encode.encode().expect("encode katakana gif");
        let mut args = eval_capture_args(tempdir.path());
        args.channel = PetalEncodeChannelArg::KatakanaBase94;

        let report = args.evaluate().expect("eval katakana gif capture");

        assert_eq!(report.channel, "katakana-base94");
        assert_eq!(report.planned_attempts, 3);
        assert_eq!(report.successes, 3);
        assert!(report.gate_passed);
        assert!(report.frames[2].path.contains("#frame_2"));
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
    fn capture_sample_models_are_exact_and_deterministic() {
        let samples = vec![
            0, 64, 128, 255, 32, 96, 160, 224, 16, 80, 144, 208, 48, 112, 176, 240,
        ];

        assert_eq!(
            downscale_sample_cells(&samples, 4, 2).expect("downscale"),
            vec![
                48, 48, 191, 191, 48, 48, 191, 191, 64, 64, 192, 192, 64, 64, 192, 192,
            ]
        );
        assert_eq!(
            box_blur_sample_cells(&samples, 4, 1).expect("blur"),
            vec![
                48, 80, 154, 191, 48, 80, 151, 186, 64, 96, 160, 192, 64, 96, 160, 192,
            ]
        );
        assert_eq!(
            motion_blur_sample_cells(&samples, 4, 1).expect("motion blur"),
            vec![
                32, 64, 149, 191, 64, 96, 160, 192, 48, 80, 144, 176, 80, 112, 176, 208,
            ]
        );
        let mut noisy = vec![32, 96, 160, 224];
        apply_sample_noise(&mut noisy, 5, 7, 2);
        assert_eq!(noisy, vec![33, 96, 165, 220]);
        let mut exposed = vec![0, 10, 250, 255];
        apply_sample_exposure_offset(&mut exposed, -20);
        assert_eq!(exposed, vec![0, 0, 230, 235]);
        apply_sample_exposure_offset(&mut exposed, 40);
        assert_eq!(exposed, vec![40, 40, 255, 255]);
    }

    #[test]
    fn eval_capture_reports_bounded_capture_models() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut encode = encode_args(payload.path(), tempdir.path());
        encode.dimension = 128;
        encode.encode().expect("encode png");
        let mut args = eval_capture_args(&tempdir.path().join("png"));
        args.capture.perturb_capture = true;
        args.capture.attempts = Some(1);
        args.capture.luminance_jitter = Some(0);
        args.capture.downscale_cells = 1;
        args.capture.blur_radius = 0;
        args.capture.motion_blur_cells = 0;
        args.capture.noise_amplitude = 0;
        args.capture.exposure_offset = 12;

        let report = args.evaluate().expect("eval capture models");

        assert!(report.gate_passed);
        assert_eq!(report.capture_downscale_cells, Some(1));
        assert_eq!(report.capture_blur_radius, Some(0));
        assert_eq!(report.capture_motion_blur_cells, Some(0));
        assert_eq!(report.capture_noise_amplitude, Some(0));
        assert_eq!(report.capture_exposure_offset, Some(12));
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
        assert_eq!(
            report.get("gate_passed").and_then(Value::as_bool),
            Some(true)
        );
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
    fn eval_capture_rejects_malformed_manifest_encoded_frame_counts() {
        let tempdir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(tempdir.path().join("png")).expect("png dir");
        std::fs::write(tempdir.path().join("png/frame_0000.png"), b"not a png").expect("write png");
        std::fs::write(
            tempdir.path().join("manifest.json"),
            r#"{
  "schema": "iroha.offline.petal.encode.v1",
  "payload_bytes": 28,
  "format": "png",
  "style": "sora-temple",
  "channel": "binary-grid",
  "grid": {
    "requested_grid_size": 0,
    "resolved_grid_size": 33,
    "border": 1,
    "anchor_size": 3
  },
  "frames": [
    {
      "index": 0,
      "path": "png/frame_0000.png",
      "payload_bytes": 28,
      "encoded_frame_count": 2
    }
  ]
}"#,
        )
        .expect("write manifest");

        let err = eval_capture_args(tempdir.path())
            .evaluate()
            .expect_err("bad png encoded count rejected");

        assert!(err.to_string().contains("encoded_frame_count must be 1"));
    }

    #[test]
    fn eval_capture_rejects_manifest_channel_mismatch() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        encode_args(payload.path(), tempdir.path())
            .encode()
            .expect("encode png");
        let mut args = eval_capture_args(tempdir.path());
        args.channel = PetalEncodeChannelArg::KatakanaBase94;

        let err = args.evaluate().expect_err("channel mismatch rejected");
        assert!(err.to_string().contains("manifest channel 'binary-grid'"));
        assert!(err.to_string().contains("katakana-base94"));
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
    fn required_successes_uses_wide_threshold_math() {
        assert_eq!(
            required_successes(u32::MAX, 9_500).expect("required successes"),
            4_080_218_931
        );
    }

    #[test]
    fn manifest_frame_paths_resolve_relative_to_manifest_parent() {
        let manifest_parent = Path::new("/tmp/petal_out");

        assert_eq!(
            resolve_manifest_frame_path(manifest_parent, "png/frame_0000.png"),
            manifest_parent.join("png/frame_0000.png")
        );
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
        args.capture.downscale_cells = 2;
        let err = args
            .evaluate()
            .expect_err("inactive downscale override rejected");
        assert!(err.to_string().contains("perturb-capture"));

        let mut args = eval_capture_args(tempdir.path());
        args.capture.noise_amplitude = 1;
        let err = args
            .evaluate()
            .expect_err("inactive noise override rejected");
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

        let mut args = eval_capture_args(tempdir.path());
        args.capture.perturb_capture = true;
        args.capture.downscale_cells = 0;
        let err = args.evaluate().expect_err("zero downscale rejected");
        assert!(err.to_string().contains("capture-downscale-cells"));

        let mut args = eval_capture_args(tempdir.path());
        args.capture.perturb_capture = true;
        args.capture.downscale_cells = MAX_CAPTURE_DOWNSCALE_CELLS + 1;
        let err = args.evaluate().expect_err("large downscale rejected");
        assert!(err.to_string().contains("capture-downscale-cells"));

        let mut args = eval_capture_args(tempdir.path());
        args.capture.perturb_capture = true;
        args.capture.blur_radius = MAX_CAPTURE_BLUR_RADIUS + 1;
        let err = args.evaluate().expect_err("large blur rejected");
        assert!(err.to_string().contains("capture-blur-radius"));

        let mut args = eval_capture_args(tempdir.path());
        args.capture.perturb_capture = true;
        args.capture.motion_blur_cells = MAX_CAPTURE_MOTION_BLUR_CELLS + 1;
        let err = args.evaluate().expect_err("large motion blur rejected");
        assert!(err.to_string().contains("capture-motion-blur-cells"));

        let mut args = eval_capture_args(tempdir.path());
        args.capture.perturb_capture = true;
        args.capture.noise_amplitude = MAX_CAPTURE_NOISE_AMPLITUDE + 1;
        let err = args.evaluate().expect_err("large noise rejected");
        assert!(err.to_string().contains("capture-noise-amplitude"));

        let mut args = eval_capture_args(tempdir.path());
        args.capture.perturb_capture = true;
        args.capture.exposure_offset = MAX_CAPTURE_EXPOSURE_OFFSET + 1;
        let err = args.evaluate().expect_err("large exposure rejected");
        assert!(err.to_string().contains("capture-exposure-offset"));
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

        assert_eq!(
            std::fs::read(&output_payload).expect("read payload"),
            payload
        );
        let report: Value = norito::json::from_str(&ctx.printed[0]).expect("realtime JSON");
        assert_eq!(
            report.get("schema").and_then(Value::as_str),
            Some(SIMULATE_REALTIME_SCHEMA)
        );
        assert_eq!(report.get("decoded").and_then(Value::as_bool), Some(true));
        assert_eq!(
            report.get("planned_attempts").and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            report
                .get("first_success_loop_index")
                .and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(report["frames"][2]["loop_index"], Value::from(2u64));
    }

    #[test]
    fn simulate_realtime_decodes_katakana_manifest_and_writes_payload() {
        let payload = b"sora-temple-capture-baseline";
        let payload_file = write_payload(payload);
        let tempdir = tempfile::tempdir().expect("temp dir");
        katakana_encode_args(payload_file.path(), tempdir.path())
            .encode()
            .expect("encode katakana png");
        let output_payload = tempdir.path().join("decoded/katakana.bin");
        let mut args = simulate_realtime_args(&tempdir.path().join("png"));
        args.channel = PetalEncodeChannelArg::KatakanaBase94;
        args.output_payload = Some(output_payload.clone());

        let report = args.simulate().expect("simulate katakana realtime");

        assert_eq!(
            std::fs::read(&output_payload).expect("read payload"),
            payload
        );
        assert_eq!(report.channel, "katakana-base94");
        assert!(report.decoded);
        assert_eq!(report.planned_attempts, 1);
        assert_eq!(report.first_success_source_index, Some(0));
        assert!(report.frames[0].success);
    }

    #[cfg(feature = "offline-visual-codecs")]
    #[test]
    fn simulate_realtime_decodes_gif_manifest_and_writes_payload_with_visual_codecs_feature() {
        let payload = b"sora-temple-capture-baseline";
        let payload_file = write_payload(payload);
        let tempdir = tempfile::tempdir().expect("temp dir");
        let mut encode = katakana_encode_args(payload_file.path(), tempdir.path());
        encode.format = PetalEncodeFormatArg::Gif;
        encode.dimension = 128;
        encode.animation_frames = 2;
        encode.encode().expect("encode katakana gif");
        let output_payload = tempdir.path().join("decoded/gif.bin");
        let mut args = simulate_realtime_args(tempdir.path());
        args.channel = PetalEncodeChannelArg::KatakanaBase94;
        args.realtime_loops = 2;
        args.output_payload = Some(output_payload.clone());

        let report = args.simulate().expect("simulate gif realtime");

        assert_eq!(
            std::fs::read(&output_payload).expect("read payload"),
            payload
        );
        assert_eq!(report.channel, "katakana-base94");
        assert!(report.decoded);
        assert_eq!(report.planned_attempts, 4);
        assert_eq!(report.attempts, 4);
        assert_eq!(report.first_success_source_index, Some(0));
        assert!(report.frames[1].path.contains("#frame_1"));
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
        args.capture.noise_amplitude = 1;
        args.capture.exposure_offset = 8;

        let report = args.simulate().expect("simulate perturbed realtime");

        assert!(report.perturb_capture);
        assert_eq!(report.capture_seed, Some(7));
        assert_eq!(report.capture_attempts_per_frame, 2);
        assert_eq!(report.capture_downscale_cells, Some(1));
        assert_eq!(report.capture_blur_radius, Some(0));
        assert_eq!(report.capture_motion_blur_cells, Some(0));
        assert_eq!(report.capture_noise_amplitude, Some(1));
        assert_eq!(report.capture_exposure_offset, Some(8));
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
        assert!(
            args.simulate()
                .expect_err("zero fps")
                .to_string()
                .contains("simulate-fps")
        );

        let mut args = simulate_realtime_args(tempdir.path());
        args.realtime_loops = 0;
        assert!(
            args.simulate()
                .expect_err("zero loops")
                .to_string()
                .contains("realtime-loops")
        );
    }

    #[test]
    fn simulate_realtime_rejects_manifest_channel_mismatch() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let tempdir = tempfile::tempdir().expect("temp dir");
        encode_args(payload.path(), tempdir.path())
            .encode()
            .expect("encode png");
        let mut args = simulate_realtime_args(tempdir.path());
        args.channel = PetalEncodeChannelArg::KatakanaBase94;

        let err = args.simulate().expect_err("channel mismatch rejected");
        assert!(err.to_string().contains("manifest channel 'binary-grid'"));
        assert!(err.to_string().contains("katakana-base94"));
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
        assert_eq!(
            style.capture_success_ratio_bps,
            PETAL_CAPTURE_RATIO_BPS_SCALE
        );
        assert_eq!(report.grid.requested_grid_size, 0);
        assert_eq!(report.grid.resolved_grid_size, 33);
        assert!(style.capture_gate_passed);
        assert!(style.throughput_gate_passed);
    }

    #[test]
    fn score_styles_expanded_set_preserves_default_recommendation_for_baseline() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let mut args = score_args(payload.path());
        args.style_set = PetalStyleSetArg::SoraTempleExpanded;

        let report = args.build_report().expect("score report");

        assert_eq!(report.style_set, "sora-temple-expanded");
        assert_eq!(report.channel, "binary-grid");
        assert_eq!(report.katakana_preset, None);
        assert_eq!(report.styles.len(), 2);
        assert_eq!(report.recommended_style, DEFAULT_STYLE_NAME);
        assert!(report.gate_passed);
        let default_style = style_score(&report, DEFAULT_STYLE_NAME);
        let high_contrast = style_score(&report, HIGH_CONTRAST_STYLE_NAME);
        assert_eq!(
            default_style.capture_success_ratio_bps,
            PETAL_CAPTURE_RATIO_BPS_SCALE
        );
        assert_eq!(
            high_contrast.capture_success_ratio_bps,
            PETAL_CAPTURE_RATIO_BPS_SCALE
        );
        assert_eq!(default_style.channel, "binary-grid");
        assert_eq!(default_style.katakana_preset, None);
        assert!(default_style.gate_passed);
        assert!(high_contrast.gate_passed);
        assert!(high_contrast.capture_profile.dark_luma < default_style.capture_profile.dark_luma);
        assert!(
            high_contrast.capture_profile.light_luma > default_style.capture_profile.light_luma
        );
    }

    #[test]
    fn score_styles_expanded_set_recommends_high_contrast_under_low_contrast_profile() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let mut args = score_args(payload.path());
        args.style_set = PetalStyleSetArg::SoraTempleExpanded;
        args.dark_luma = Some(128);
        args.light_luma = Some(129);
        args.luminance_jitter = Some(0);
        args.attempts = Some(4);

        let report = args.build_report().expect("score report");

        assert_eq!(report.recommended_style, HIGH_CONTRAST_STYLE_NAME);
        assert!(report.gate_passed);
        let default_style = style_score(&report, DEFAULT_STYLE_NAME);
        let high_contrast = style_score(&report, HIGH_CONTRAST_STYLE_NAME);
        assert_eq!(default_style.capture_successes, 0);
        assert!(!default_style.capture_gate_passed);
        assert!(!default_style.gate_passed);
        assert_eq!(high_contrast.capture_profile.dark_luma, 64);
        assert_eq!(high_contrast.capture_profile.light_luma, 193);
        assert_eq!(high_contrast.capture_successes, 4);
        assert!(high_contrast.capture_gate_passed);
        assert!(high_contrast.gate_passed);
        assert_eq!(
            report.recommended_overall_score_bps,
            PETAL_CAPTURE_RATIO_BPS_SCALE
        );
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
    fn score_styles_katakana_default_uses_balanced_preset_floor() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let mut args = score_args(payload.path());
        args.channel = PetalEncodeChannelArg::KatakanaBase94;

        let report = args.build_report().expect("score katakana report");

        assert_eq!(report.channel, "katakana-base94");
        assert_eq!(report.katakana_preset.as_deref(), Some("balanced"));
        assert_eq!(report.grid.requested_grid_size, 0);
        assert_eq!(report.grid.resolved_grid_size, 41);
        assert_eq!(report.recommended_style, KATAKANA_STYLE_NAME);
        assert!(report.gate_passed);
        assert_eq!(report.styles.len(), 1);
        let style = style_score(&report, KATAKANA_STYLE_NAME);
        assert_eq!(style.channel, "katakana-base94");
        assert_eq!(style.katakana_preset.as_deref(), Some("balanced"));
        assert_eq!(style.capture_attempts, 12);
        assert_eq!(style.capture_successes, 12);
        assert_eq!(
            style.capture_success_ratio_bps,
            PETAL_CAPTURE_RATIO_BPS_SCALE
        );
    }

    #[test]
    fn score_styles_katakana_distance_safe_uses_preset_floor() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let mut args = score_args(payload.path());
        args.channel = PetalEncodeChannelArg::KatakanaBase94;
        args.katakana_preset = Some(PetalKatakanaPresetArg::DistanceSafe);

        let report = args.build_report().expect("score distance-safe katakana");

        assert_eq!(report.katakana_preset.as_deref(), Some("distance-safe"));
        assert_eq!(report.grid.requested_grid_size, 0);
        assert_eq!(report.grid.resolved_grid_size, 33);
        assert_eq!(
            style_score(&report, KATAKANA_STYLE_NAME)
                .katakana_preset
                .as_deref(),
            Some("distance-safe")
        );
    }

    #[test]
    fn score_styles_katakana_explicit_grid_overrides_preset_floor() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let mut args = score_args(payload.path());
        args.channel = PetalEncodeChannelArg::KatakanaBase94;
        args.katakana_preset = Some(PetalKatakanaPresetArg::Balanced);
        args.grid_size = 37;

        let report = args.build_report().expect("score explicit katakana grid");

        assert_eq!(report.katakana_preset.as_deref(), Some("balanced"));
        assert_eq!(report.grid.requested_grid_size, 37);
        assert_eq!(report.grid.resolved_grid_size, 37);
        assert_eq!(report.recommended_style, KATAKANA_STYLE_NAME);
    }

    #[test]
    fn score_styles_katakana_expanded_recommends_high_contrast_under_low_contrast_profile() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let mut args = score_args(payload.path());
        args.channel = PetalEncodeChannelArg::KatakanaBase94;
        args.style_set = PetalStyleSetArg::SoraTempleExpanded;
        args.dark_luma = Some(128);
        args.light_luma = Some(129);
        args.luminance_jitter = Some(0);
        args.attempts = Some(4);

        let report = args.build_report().expect("score katakana expanded");

        assert_eq!(report.recommended_style, KATAKANA_HIGH_CONTRAST_STYLE_NAME);
        assert!(report.gate_passed);
        let default_style = style_score(&report, KATAKANA_STYLE_NAME);
        let high_contrast = style_score(&report, KATAKANA_HIGH_CONTRAST_STYLE_NAME);
        assert_eq!(default_style.capture_successes, 0);
        assert!(!default_style.gate_passed);
        assert_eq!(high_contrast.channel, "katakana-base94");
        assert_eq!(high_contrast.katakana_preset.as_deref(), Some("balanced"));
        assert_eq!(high_contrast.capture_successes, 4);
        assert!(high_contrast.gate_passed);
    }

    #[test]
    fn score_styles_rejects_binary_katakana_preset() {
        let payload = write_payload(b"sora-temple-capture-baseline");
        let mut args = score_args(payload.path());
        args.katakana_preset = Some(PetalKatakanaPresetArg::DistanceSafe);

        let err = args.build_report().expect_err("binary preset rejected");

        assert!(err.to_string().contains("katakana-preset"));
    }

    #[test]
    fn high_contrast_capture_profile_expands_luma_with_saturation() {
        let profile = PetalStreamCaptureProfile {
            attempts: 3,
            dark_luma: 16,
            light_luma: 250,
            luminance_jitter: 7,
        };

        let high_contrast = high_contrast_capture_profile(profile);

        assert_eq!(high_contrast.attempts, profile.attempts);
        assert_eq!(high_contrast.dark_luma, 0);
        assert_eq!(high_contrast.light_luma, 255);
        assert_eq!(high_contrast.luminance_jitter, profile.luminance_jitter);

        let low_contrast = high_contrast_capture_profile(PetalStreamCaptureProfile {
            attempts: 4,
            dark_luma: 128,
            light_luma: 129,
            luminance_jitter: 0,
        });
        assert_eq!(low_contrast.dark_luma, 64);
        assert_eq!(low_contrast.light_luma, 193);
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
        assert_eq!(
            summary.get("gate_passed").and_then(Value::as_bool),
            Some(true)
        );
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
