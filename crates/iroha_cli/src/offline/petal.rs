use super::*;

use iroha_data_model::{
    petal_stream::{
        PETAL_STREAM_GRID_SIZES, PetalStreamDecoder, PetalStreamEncoder, PetalStreamGrid,
        PetalStreamOptions, PetalStreamSampleGrid,
    },
    qr_stream::{
        QrStreamAssembler, QrStreamDecodeResult, QrStreamEncoder, QrStreamEnvelope, QrStreamFrame,
        QrStreamFrameKind, QrStreamOptions,
    },
};
use norito::json::{Map, Value};
use std::{
    fs,
    io::BufWriter,
    path::{Path, PathBuf},
};

use super::qr::QrPayloadKindArg;

#[derive(clap::Subcommand, Debug)]
pub enum PetalCommand {
    /// Encode a payload into petal stream frames.
    Encode(PetalEncodeArgs),
    /// Decode petal stream frames into the original payload.
    Decode(PetalDecodeArgs),
    /// Evaluate decode robustness under simulated distant/moving capture.
    EvalCapture(PetalEvalCaptureArgs),
}

impl Run for PetalCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            PetalCommand::Encode(args) => args.run(context),
            PetalCommand::Decode(args) => args.run(context),
            PetalCommand::EvalCapture(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct PetalEncodeArgs {
    /// Path to the payload bytes to encode.
    #[arg(long, value_name = "FILE")]
    pub input: PathBuf,
    /// Output directory for generated frames and artifacts.
    #[arg(long, value_name = "DIR")]
    pub output: PathBuf,
    /// Payload kind tag embedded in the envelope.
    #[arg(long, value_enum, default_value = "unspecified")]
    pub payload_kind: QrPayloadKindArg,
    /// Chunk size in bytes.
    #[arg(long, default_value_t = 140)]
    pub chunk_size: u16,
    /// Parity group size (0 disables parity frames).
    #[arg(long, default_value_t = 0)]
    pub parity_group: u8,
    /// Grid size in cells (0 selects automatic sizing).
    #[arg(long, default_value_t = 0)]
    pub grid_size: u16,
    /// Border thickness in cells.
    #[arg(long, default_value_t = 1)]
    pub border: u8,
    /// Anchor size in cells.
    #[arg(long, default_value_t = 3)]
    pub anchor_size: u8,
    /// Rendered frame size in pixels.
    #[arg(long, default_value_t = 512)]
    pub dimension: u32,
    /// Output format for rendered frames.
    #[arg(long, value_enum, default_value = "frames")]
    pub format: PetalOutputFormat,
    /// Frames per second for animated outputs.
    #[arg(long, default_value_t = 24)]
    pub fps: u16,
    /// Render style for preview images (ignored for --format frames).
    #[arg(long, value_enum, default_value = "sora-temple")]
    pub style: PetalRenderStyle,
}

impl PetalEncodeArgs {
    fn run<C: RunContext>(self, _context: &mut C) -> Result<()> {
        let payload = fs::read(&self.input)
            .map_err(|err| eyre!("failed to read payload {}: {err}", self.input.display()))?;
        let options = QrStreamOptions {
            chunk_size: self.chunk_size,
            parity_group: self.parity_group,
            payload_kind: self.payload_kind.into(),
            ..QrStreamOptions::default()
        };
        let (envelope, frames) = QrStreamEncoder::encode_frames(&payload, options)?;
        let encoded_frames: Vec<Vec<u8>> = frames.iter().map(QrStreamFrame::encode).collect();
        let base_petal_options = PetalStreamOptions {
            grid_size: 0,
            border: self.border,
            anchor_size: self.anchor_size,
        };
        let resolved_grid_size = if self.grid_size == 0 {
            let max_len = encoded_frames.iter().map(Vec::len).max().unwrap_or(0);
            let dummy = vec![0u8; max_len];
            let grid = PetalStreamEncoder::encode_grid(&dummy, base_petal_options)?;
            grid.grid_size
        } else {
            self.grid_size
        };
        let petal_options = PetalStreamOptions {
            grid_size: resolved_grid_size,
            ..base_petal_options
        };

        let output_dir = &self.output;
        fs::create_dir_all(output_dir).map_err(|err| {
            eyre!(
                "failed to create output dir {}: {err}",
                output_dir.display()
            )
        })?;
        let frames_dir = output_dir.join("frames");
        fs::create_dir_all(&frames_dir).map_err(|err| {
            eyre!(
                "failed to create frames dir {}: {err}",
                frames_dir.display()
            )
        })?;

        let mut manifest_frames: Vec<Value> = Vec::new();
        let mut rendered = Vec::new();
        for (idx, (frame, bytes)) in frames.iter().zip(encoded_frames.iter()).enumerate() {
            let file_name = format!("frame_{idx:04}.bin");
            let path = frames_dir.join(&file_name);
            fs::write(&path, bytes)
                .map_err(|err| eyre!("failed to write frame {}: {err}", path.display()))?;
            let mut frame_map = Map::new();
            frame_map.insert("index".to_string(), Value::from(idx as u64));
            frame_map.insert(
                "kind".to_string(),
                Value::from(frame_kind_label(frame.kind)),
            );
            frame_map.insert(
                "file".to_string(),
                Value::from(format!("frames/{file_name}")),
            );
            frame_map.insert("bytes_hex".to_string(), Value::from(hex::encode(bytes)));
            manifest_frames.push(Value::Object(frame_map));
            if self.format != PetalOutputFormat::Frames {
                let grid = PetalStreamEncoder::encode_grid(bytes, petal_options)?;
                rendered.push(RenderedFrame {
                    index: idx as u32,
                    grid,
                });
            }
        }

        match self.format {
            PetalOutputFormat::Frames => {}
            PetalOutputFormat::Png => {
                let png_dir = output_dir.join("png");
                fs::create_dir_all(&png_dir).map_err(|err| {
                    eyre!("failed to create png dir {}: {err}", png_dir.display())
                })?;
                let total = rendered.len().max(1) as u32;
                for frame in &rendered {
                    let image = render_petal_frame(
                        &frame.grid,
                        self.dimension,
                        frame.index,
                        total,
                        self.style,
                        petal_options,
                    );
                    let path = png_dir.join(format!("frame_{:04}.png", frame.index));
                    write_png_rgba(&path, &image)?;
                }
            }
            PetalOutputFormat::Gif => {
                let total = rendered.len().max(1) as u32;
                let frames = rendered
                    .iter()
                    .map(|frame| {
                        render_petal_frame(
                            &frame.grid,
                            self.dimension,
                            frame.index,
                            total,
                            self.style,
                            petal_options,
                        )
                    })
                    .collect::<Vec<_>>();
                let path = output_dir.join("stream.gif");
                write_gif_rgba(&path, &frames, self.fps)?;
            }
            PetalOutputFormat::Apng => {
                let total = rendered.len().max(1) as u32;
                let frames = rendered
                    .iter()
                    .map(|frame| {
                        render_petal_frame(
                            &frame.grid,
                            self.dimension,
                            frame.index,
                            total,
                            self.style,
                            petal_options,
                        )
                    })
                    .collect::<Vec<_>>();
                let path = output_dir.join("stream.png");
                write_apng_rgba(&path, &frames, self.fps)?;
            }
        }

        let mut manifest_map = Map::new();
        let estimated_payload_bps =
            estimate_payload_bytes_per_second(payload.len(), frames.len(), self.fps);
        manifest_map.insert("version".to_string(), Value::from(1u64));
        manifest_map.insert(
            "payload_kind".to_string(),
            Value::from(self.payload_kind.label()),
        );
        manifest_map.insert(
            "chunk_size".to_string(),
            Value::from(self.chunk_size as u64),
        );
        manifest_map.insert(
            "parity_group".to_string(),
            Value::from(self.parity_group as u64),
        );
        manifest_map.insert(
            "grid_size".to_string(),
            Value::from(resolved_grid_size as u64),
        );
        manifest_map.insert(
            "border".to_string(),
            Value::from(petal_options.border as u64),
        );
        manifest_map.insert(
            "anchor_size".to_string(),
            Value::from(petal_options.anchor_size as u64),
        );
        manifest_map.insert("render_style".to_string(), Value::from(self.style.label()));
        manifest_map.insert(
            "estimated_payload_bytes_per_second".to_string(),
            Value::from(estimated_payload_bps),
        );
        manifest_map.insert("frame_count".to_string(), Value::from(frames.len() as u64));
        manifest_map.insert(
            "payload_length".to_string(),
            Value::from(payload.len() as u64),
        );
        manifest_map.insert(
            "payload_hash_hex".to_string(),
            Value::from(hex::encode(envelope.payload_hash)),
        );
        manifest_map.insert(
            "envelope_hex".to_string(),
            Value::from(hex::encode(envelope.encode())),
        );
        manifest_map.insert("frames".to_string(), Value::Array(manifest_frames));
        let manifest = Value::Object(manifest_map);
        let manifest_path = output_dir.join("manifest.json");
        let manifest_json =
            norito::json::to_string_pretty(&manifest).map_err(|err| eyre!("{err}"))?;
        fs::write(&manifest_path, format!("{manifest_json}\n")).map_err(|err| {
            eyre!(
                "failed to write manifest {}: {err}",
                manifest_path.display()
            )
        })?;

        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct PetalDecodeArgs {
    /// Directory containing PNG frames.
    #[arg(long, value_name = "DIR")]
    pub input_dir: PathBuf,
    /// Output file for the decoded payload.
    #[arg(long, value_name = "FILE")]
    pub output: PathBuf,
    /// Grid size in cells (0 to auto-detect).
    #[arg(long, default_value_t = 0)]
    pub grid_size: u16,
    /// Border thickness in cells.
    #[arg(long, default_value_t = 1)]
    pub border: u8,
    /// Anchor size in cells.
    #[arg(long, default_value_t = 3)]
    pub anchor_size: u8,
    /// Optional JSON manifest output path.
    #[arg(long, value_name = "FILE")]
    pub output_manifest: Option<PathBuf>,
}

impl PetalDecodeArgs {
    fn run<C: RunContext>(self, _context: &mut C) -> Result<()> {
        let mut entries = fs::read_dir(&self.input_dir)
            .map_err(|err| eyre!("failed to read {}: {err}", self.input_dir.display()))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("png"))
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());

        let mut resolved_grid_size = self.grid_size;
        let petal_options = PetalStreamOptions {
            grid_size: if self.grid_size == 0 {
                0
            } else {
                self.grid_size
            },
            border: self.border,
            anchor_size: self.anchor_size,
        };
        let mut assembler = QrStreamAssembler::default();
        let mut envelope: Option<QrStreamEnvelope> = None;
        let mut result: Option<QrStreamDecodeResult> = None;
        for entry in entries {
            let image = load_png(entry.path())?;
            let frame_bytes = if resolved_grid_size == 0 {
                let (grid_size, bytes) = decode_frame_auto(&image, petal_options)?;
                resolved_grid_size = grid_size;
                bytes
            } else {
                decode_frame_with_grid(&image, resolved_grid_size, petal_options)?
            };
            let frame = QrStreamFrame::decode(&frame_bytes)?;
            if envelope.is_none() && frame.kind == QrStreamFrameKind::Header {
                if let Ok(header) = QrStreamEnvelope::decode(&frame.payload) {
                    envelope = Some(header);
                }
            }
            result = Some(assembler.ingest_frame(frame)?);
        }

        let result = result.ok_or_else(|| eyre!("no frames decoded"))?;
        if !result.is_complete() {
            return Err(eyre!(
                "petal stream incomplete: received {} / {}",
                result.received_chunks,
                result.total_chunks
            ));
        }
        let payload = result.payload.ok_or_else(|| eyre!("payload missing"))?;
        fs::write(&self.output, payload.as_slice())
            .map_err(|err| eyre!("failed to write {}: {err}", self.output.display()))?;

        if let Some(path) = self.output_manifest {
            let mut manifest_map = Map::new();
            manifest_map.insert("version".to_string(), Value::from(1u64));
            manifest_map.insert(
                "grid_size".to_string(),
                Value::from(resolved_grid_size as u64),
            );
            manifest_map.insert("border".to_string(), Value::from(self.border as u64));
            manifest_map.insert(
                "anchor_size".to_string(),
                Value::from(self.anchor_size as u64),
            );
            manifest_map.insert(
                "payload_length".to_string(),
                Value::from(payload.len() as u64),
            );
            manifest_map.insert(
                "payload_hex".to_string(),
                Value::from(hex::encode(&payload)),
            );
            manifest_map.insert(
                "envelope_hex".to_string(),
                Value::from(
                    envelope
                        .as_ref()
                        .map(|env| hex::encode(env.encode()))
                        .unwrap_or_default(),
                ),
            );
            let manifest = Value::Object(manifest_map);
            let rendered = norito::json::to_string_pretty(&manifest)?;
            fs::write(&path, format!("{rendered}\n"))
                .map_err(|err| eyre!("failed to write manifest {}: {err}", path.display()))?;
        }

        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct PetalEvalCaptureArgs {
    /// Directory containing rendered PNG frames.
    #[arg(long, value_name = "DIR")]
    pub input_dir: PathBuf,
    /// Grid size in cells (0 to auto-detect from pristine frames).
    #[arg(long, default_value_t = 0)]
    pub grid_size: u16,
    /// Border thickness in cells.
    #[arg(long, default_value_t = 1)]
    pub border: u8,
    /// Anchor size in cells.
    #[arg(long, default_value_t = 3)]
    pub anchor_size: u8,
    /// Capture perturbation profile.
    #[arg(long, value_enum, default_value = "default")]
    pub profile: PetalCaptureProfile,
    /// Deterministic seed for perturbation sampling.
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
    /// Number of perturbation trials per frame (0 uses profile default).
    #[arg(long, default_value_t = 0)]
    pub trials_per_frame: u16,
    /// Minimum successful decode ratio required to pass.
    #[arg(long, default_value_t = 0.95)]
    pub min_success_ratio: f64,
    /// Optional JSON report output path.
    #[arg(long, value_name = "FILE")]
    pub output_report: Option<PathBuf>,
}

impl PetalEvalCaptureArgs {
    fn run<C: RunContext>(self, _context: &mut C) -> Result<()> {
        if !(0.0..=1.0).contains(&self.min_success_ratio) {
            return Err(eyre!("min-success-ratio must be between 0.0 and 1.0"));
        }
        let mut entries = fs::read_dir(&self.input_dir)
            .map_err(|err| eyre!("failed to read {}: {err}", self.input_dir.display()))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("png"))
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        if entries.is_empty() {
            return Err(eyre!("no png frames found in {}", self.input_dir.display()));
        }

        let mut resolved_grid_size = self.grid_size;
        let mut images = Vec::with_capacity(entries.len());
        let mut expected_bytes = Vec::with_capacity(entries.len());
        let base_options = PetalStreamOptions {
            grid_size: if self.grid_size == 0 {
                0
            } else {
                self.grid_size
            },
            border: self.border,
            anchor_size: self.anchor_size,
        };
        for entry in &entries {
            let image = load_png(entry.path())?;
            let bytes = if resolved_grid_size == 0 {
                let (grid_size, bytes) = decode_frame_auto(&image, base_options)?;
                resolved_grid_size = grid_size;
                bytes
            } else {
                decode_frame_with_grid(&image, resolved_grid_size, base_options)?
            };
            images.push(image);
            expected_bytes.push(bytes);
        }
        if resolved_grid_size == 0 {
            return Err(eyre!("failed to resolve grid size from pristine frames"));
        }

        let trials_per_frame = if self.trials_per_frame == 0 {
            self.profile.default_trials()
        } else {
            self.trials_per_frame
        };
        if trials_per_frame == 0 {
            return Err(eyre!("trials-per-frame must be > 0"));
        }

        let options = PetalStreamOptions {
            grid_size: resolved_grid_size,
            border: self.border,
            anchor_size: self.anchor_size,
        };
        let metrics = evaluate_capture_robustness(
            &images,
            &expected_bytes,
            resolved_grid_size,
            options,
            self.profile,
            self.seed,
            trials_per_frame,
        )?;

        let report = metrics.to_json(
            self.profile,
            resolved_grid_size,
            trials_per_frame,
            self.min_success_ratio,
            self.seed,
        );
        if let Some(path) = self.output_report {
            let rendered = norito::json::to_string_pretty(&report)?;
            fs::write(&path, format!("{rendered}\n"))
                .map_err(|err| eyre!("failed to write report {}: {err}", path.display()))?;
        }

        if metrics.success_ratio() + f64::EPSILON < self.min_success_ratio {
            return Err(eyre!(
                "capture decode success ratio {:.3} below required {:.3}",
                metrics.success_ratio(),
                self.min_success_ratio
            ));
        }
        if !metrics.stream_complete {
            return Err(eyre!(
                "capture evaluation could not reconstruct complete stream ({} / {} chunks)",
                metrics.stream_received_chunks,
                metrics.stream_total_chunks
            ));
        }

        Ok(())
    }
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetalOutputFormat {
    Frames,
    Png,
    Gif,
    Apng,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetalRenderStyle {
    SakuraWind,
    SoraTemple,
    SoraTempleBold,
    SoraTempleMinimal,
    SoraTempleRadiant,
    SoraTempleCommand,
    SoraTempleAegis,
    SoraTempleGhost,
}

impl PetalRenderStyle {
    fn label(self) -> &'static str {
        match self {
            Self::SakuraWind => "sakura-wind",
            Self::SoraTemple => "sora-temple",
            Self::SoraTempleBold => "sora-temple-bold",
            Self::SoraTempleMinimal => "sora-temple-minimal",
            Self::SoraTempleRadiant => "sora-temple-radiant",
            Self::SoraTempleCommand => "sora-temple-command",
            Self::SoraTempleAegis => "sora-temple-aegis",
            Self::SoraTempleGhost => "sora-temple-ghost",
        }
    }
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetalCaptureProfile {
    Default,
    Aggressive,
}

impl PetalCaptureProfile {
    fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Aggressive => "aggressive",
        }
    }

    fn default_trials(self) -> u16 {
        match self {
            Self::Default => 5,
            Self::Aggressive => 8,
        }
    }
}

struct RenderedFrame {
    index: u32,
    grid: PetalStreamGrid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RenderCellRole {
    Border,
    AnchorDark,
    AnchorLight,
    Data,
}

fn render_cell_role(x: u16, y: u16, grid_size: u16, options: PetalStreamOptions) -> RenderCellRole {
    let border = options.border as u16;
    let anchor = options.anchor_size as u16;
    if x < border
        || y < border
        || x >= grid_size.saturating_sub(border)
        || y >= grid_size.saturating_sub(border)
    {
        return RenderCellRole::Border;
    }
    let right = grid_size.saturating_sub(border + anchor);
    let bottom = grid_size.saturating_sub(border + anchor);
    let in_left = x >= border && x < border + anchor;
    let in_right = x >= right && x < right + anchor;
    let in_top = y >= border && y < border + anchor;
    let in_bottom = y >= bottom && y < bottom + anchor;
    if in_left && in_top {
        return RenderCellRole::AnchorDark;
    }
    if in_left && in_bottom {
        return RenderCellRole::AnchorDark;
    }
    if in_right && in_top {
        return RenderCellRole::AnchorLight;
    }
    if in_right && in_bottom {
        return RenderCellRole::AnchorLight;
    }
    RenderCellRole::Data
}

const SAKURA_BG_START: [f64; 3] = [0.99, 0.93, 0.96];
const SAKURA_BG_END: [f64; 3] = [1.0, 0.98, 0.99];
const SAKURA_PETAL: [f64; 3] = [1.0, 0.79, 0.87];
const SAKURA_INK: [f64; 3] = [0.26, 0.08, 0.14];
const ANCHOR_DARK: [f64; 3] = [0.06, 0.02, 0.04];
const ANCHOR_LIGHT: [f64; 3] = [0.98, 0.98, 0.99];
const DATA_DARK: [f64; 3] = [0.16, 0.05, 0.09];
const DATA_LIGHT: [f64; 3] = [0.95, 0.95, 0.97];
const DATA_CELL_ALPHA: f64 = 0.85;
const SAKURA_WIND_STREAK_ALPHA: f64 = 0.08;
const SAKURA_WIND_STREAK_FREQ_X: f64 = 6.0;
const SAKURA_WIND_STREAK_FREQ_Y: f64 = 3.5;
const SAKURA_BG_PETAL_COUNT: usize = 16;
const SAKURA_BG_PETAL_ORBIT: f64 = 0.36;
const SAKURA_BG_PETAL_RADIUS: f64 = 0.26;
const SAKURA_BG_PETAL_ALPHA: f64 = 0.20;
const SAKURA_TECH_RING_ALPHA: f64 = 0.035;
const SAKURA_TECH_RING_FREQ: f64 = 10.0;
const DATA_PETAL_MAJOR: f64 = 0.52;
const DATA_PETAL_MINOR: f64 = 0.28;
const DATA_PETAL_JITTER: f64 = 0.10;
const DATA_PETAL_ROTATION: f64 = 0.45;
const DATA_PETAL_SWAY: f64 = 0.08;
const DATA_PETAL_GLOW_RADIUS: f64 = 1.6;
const DATA_PETAL_GLOW_ALPHA: f64 = 0.12;
const DATA_PETAL_HIGHLIGHT_ALPHA: f64 = 0.08;

fn render_petal_frame(
    grid: &PetalStreamGrid,
    dimension: u32,
    frame_index: u32,
    frame_count: u32,
    style: PetalRenderStyle,
    options: PetalStreamOptions,
) -> image::RgbaImage {
    if matches!(
        style,
        PetalRenderStyle::SoraTemple
            | PetalRenderStyle::SoraTempleBold
            | PetalRenderStyle::SoraTempleMinimal
            | PetalRenderStyle::SoraTempleRadiant
            | PetalRenderStyle::SoraTempleCommand
            | PetalRenderStyle::SoraTempleAegis
            | PetalRenderStyle::SoraTempleGhost
    ) {
        return render_sora_temple_frame(grid, dimension, frame_index, frame_count, style, options);
    }
    let grid_size = grid.grid_size as u32;
    let cell_size = (dimension / grid_size).max(1);
    let width = grid_size * cell_size;
    let height = width;
    let offset_x = (dimension.saturating_sub(width)) / 2;
    let offset_y = (dimension.saturating_sub(height)) / 2;
    let phase = (frame_index % frame_count.max(1)) as f64 / frame_count.max(1) as f64;
    let wind_angle = phase * std::f64::consts::TAU;
    let wind_dir = (wind_angle.cos(), wind_angle.sin());
    let glow_radius_sq = DATA_PETAL_GLOW_RADIUS * DATA_PETAL_GLOW_RADIUS;

    let mut cell_params = Vec::with_capacity(grid.cells.len());
    let mut cell_roles = Vec::with_capacity(grid.cells.len());
    for y in 0..grid_size {
        for x in 0..grid_size {
            let idx = y as usize * grid_size as usize + x as usize;
            let bit = grid.cells[idx];
            let mut seed = cell_seed(x, y);
            let jitter_x = (sample_unit(&mut seed) - 0.5) * DATA_PETAL_JITTER;
            let jitter_y = (sample_unit(&mut seed) - 0.5) * DATA_PETAL_JITTER;
            let angle =
                (sample_unit(&mut seed) - 0.5) * std::f64::consts::TAU * DATA_PETAL_ROTATION;
            let sway = (sample_unit(&mut seed) - 0.5) * 2.0;
            cell_params.push(CellParam {
                bit,
                jitter_x,
                jitter_y,
                angle,
                sway,
            });
            cell_roles.push(render_cell_role(
                x as u16,
                y as u16,
                grid.grid_size,
                options,
            ));
        }
    }

    let mut rgba = Vec::with_capacity((dimension * dimension * 4) as usize);
    for py in 0..dimension {
        let ny = if dimension <= 1 {
            0.5
        } else {
            py as f64 / (dimension - 1) as f64
        };
        for px in 0..dimension {
            let nx = if dimension <= 1 {
                0.5
            } else {
                px as f64 / (dimension - 1) as f64
            };
            let cx = nx - 0.5;
            let cy = ny - 0.5;
            let proj = cx * wind_dir.0 + cy * wind_dir.1;
            let t = (proj + 0.5).clamp(0.0, 1.0);
            let mut r = lerp(SAKURA_BG_START[0], SAKURA_BG_END[0], t);
            let mut g = lerp(SAKURA_BG_START[1], SAKURA_BG_END[1], t);
            let mut b = lerp(SAKURA_BG_START[2], SAKURA_BG_END[2], t);

            let streak =
                (nx * SAKURA_WIND_STREAK_FREQ_X + ny * SAKURA_WIND_STREAK_FREQ_Y + wind_angle)
                    .sin();
            let streak = (streak * 0.5 + 0.5) * SAKURA_WIND_STREAK_ALPHA;
            r = (r + streak).min(1.0);
            g = (g + streak).min(1.0);
            b = (b + streak).min(1.0);

            if SAKURA_TECH_RING_ALPHA > 0.0 {
                let radius = (cx * cx + cy * cy).sqrt();
                let ring = (radius * SAKURA_TECH_RING_FREQ + wind_angle).sin();
                let ring = (ring * 0.5 + 0.5) * SAKURA_TECH_RING_ALPHA;
                r = (r + ring).min(1.0);
                g = (g + ring * 0.9).min(1.0);
                b = (b + ring * 1.1).min(1.0);
            }

            let petal_phase = phase * std::f64::consts::TAU;
            for petal_index in 0..SAKURA_BG_PETAL_COUNT {
                let petal_angle = petal_phase
                    + (petal_index as f64 / SAKURA_BG_PETAL_COUNT as f64) * std::f64::consts::TAU;
                let drift = petal_phase.sin() * 0.05;
                let petal_cx = 0.5 + petal_angle.cos() * SAKURA_BG_PETAL_ORBIT + wind_dir.0 * drift;
                let petal_cy = 0.5
                    + petal_angle.sin() * SAKURA_BG_PETAL_ORBIT
                    + wind_dir.1 * drift
                    + petal_phase.cos() * 0.03;
                let dx = nx - petal_cx;
                let dy = ny - petal_cy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < SAKURA_BG_PETAL_RADIUS {
                    let alpha = (1.0 - dist / SAKURA_BG_PETAL_RADIUS) * SAKURA_BG_PETAL_ALPHA;
                    r = lerp(r, SAKURA_PETAL[0], alpha);
                    g = lerp(g, SAKURA_PETAL[1], alpha);
                    b = lerp(b, SAKURA_PETAL[2], alpha);
                }
            }

            if px >= offset_x && py >= offset_y && px < offset_x + width && py < offset_y + height {
                let gx = (px - offset_x) / cell_size;
                let gy = (py - offset_y) / cell_size;
                let idx = gy as usize * grid_size as usize + gx as usize;
                let param = &cell_params[idx];
                if param.bit {
                    let lx = (px - offset_x) as f64 / cell_size as f64;
                    let ly = (py - offset_y) as f64 / cell_size as f64;
                    let sway = param.sway * DATA_PETAL_SWAY * (wind_angle + param.angle).sin();
                    let cell_x = gx as f64 + 0.5 + param.jitter_x + wind_dir.0 * sway;
                    let cell_y = gy as f64 + 0.5 + param.jitter_y + wind_dir.1 * sway;
                    let dx = lx - cell_x;
                    let dy = ly - cell_y;
                    let (sin_a, cos_a) = (wind_angle + param.angle).sin_cos();
                    let rx = dx * cos_a + dy * sin_a;
                    let ry = -dx * sin_a + dy * cos_a;
                    let nx = rx / DATA_PETAL_MAJOR;
                    let ny = ry / DATA_PETAL_MINOR;
                    let dist = nx * nx + ny * ny;
                    if dist <= glow_radius_sq {
                        let glow =
                            (1.0 - dist / glow_radius_sq).clamp(0.0, 1.0) * DATA_PETAL_GLOW_ALPHA;
                        r = lerp(r, SAKURA_PETAL[0], glow);
                        g = lerp(g, SAKURA_PETAL[1], glow);
                        b = lerp(b, SAKURA_PETAL[2], glow);
                    }
                    if dist <= 1.0 {
                        r = SAKURA_INK[0];
                        g = SAKURA_INK[1];
                        b = SAKURA_INK[2];
                        let highlight = (1.0 - dist).clamp(0.0, 1.0) * DATA_PETAL_HIGHLIGHT_ALPHA;
                        r = lerp(r, SAKURA_PETAL[0], highlight);
                        g = lerp(g, SAKURA_PETAL[1], highlight);
                        b = lerp(b, SAKURA_PETAL[2], highlight);
                    }
                }
                match cell_roles[idx] {
                    RenderCellRole::AnchorDark => {
                        r = ANCHOR_DARK[0];
                        g = ANCHOR_DARK[1];
                        b = ANCHOR_DARK[2];
                    }
                    RenderCellRole::AnchorLight => {
                        r = ANCHOR_LIGHT[0];
                        g = ANCHOR_LIGHT[1];
                        b = ANCHOR_LIGHT[2];
                    }
                    RenderCellRole::Data => {
                        let target = if param.bit { DATA_DARK } else { DATA_LIGHT };
                        r = lerp(r, target[0], DATA_CELL_ALPHA);
                        g = lerp(g, target[1], DATA_CELL_ALPHA);
                        b = lerp(b, target[2], DATA_CELL_ALPHA);
                    }
                    RenderCellRole::Border => {}
                }
            }

            rgba.extend_from_slice(&[to_u8(r), to_u8(g), to_u8(b), 0xFF]);
        }
    }

    image::RgbaImage::from_raw(dimension, dimension, rgba).expect("rgba buffer size")
}

const SORA_BG_START: [f64; 3] = [0.02, 0.01, 0.05];
const SORA_BG_END: [f64; 3] = [0.05, 0.02, 0.08];
const SORA_RING_BRIGHT: [f64; 3] = [0.95, 0.76, 0.89];
const SORA_RING_DIM: [f64; 3] = [0.32, 0.18, 0.31];
const SORA_TILE_LIGHT_BG: [f64; 3] = [0.95, 0.92, 0.97];
const SORA_TILE_LIGHT_FG: [f64; 3] = [0.10, 0.04, 0.14];
const SORA_TILE_DARK_BG: [f64; 3] = [0.07, 0.03, 0.10];
const SORA_TILE_DARK_FG: [f64; 3] = [0.97, 0.91, 0.98];
const SORA_LOGO_TINT: [f64; 3] = [0.98, 0.91, 0.97];
const SORA_RING_RADII: [f64; 4] = [0.35, 0.42, 0.49, 0.56];
const SORA_RING_DOTS: [f64; 4] = [68.0, 84.0, 102.0, 122.0];
const SORA_RING_SPEED: [f64; 4] = [0.64, 0.89, 1.08, 1.30];
const SORA_RING_BAND: f64 = 0.0058;
const SORA_SCANLINE_ALPHA: f64 = 0.02;
const SORA_VIGNETTE: f64 = 0.26;
const SORA_TILE_MARGIN_OUTER: f64 = 0.024;
const SORA_TILE_MARGIN_LOGO: f64 = 0.005;
const SORA_TILE_GLYPH_ALPHA: f64 = 0.28;
const SORA_KATAKANA_UNCERTAIN_BAND: u8 = 28;
const SORA_KATAKANA_CONFIDENCE_MIN: f64 = 0.08;
const SORA_KATAKANA_CONFIDENCE_STRONG: f64 = 0.16;
const SORA_KATAKANA_OVERRIDE_PUSH: u8 = 68;
const SORA_KATAKANA_MIN_ASSIST_SAMPLES: usize = 24;
const SORA_KATAKANA_SHIFT_X: i32 = 0;
const SORA_KATAKANA_SHIFT_Y: i32 = 1;
const SORA_KATAKANA_BITMAPS: [[u8; 8]; 16] = [
    [0x3C, 0x08, 0x1C, 0x08, 0x08, 0x08, 0x00, 0x00],
    [0x10, 0x10, 0x20, 0x20, 0x40, 0x40, 0x00, 0x00],
    [0x3C, 0x04, 0x1C, 0x04, 0x04, 0x3C, 0x00, 0x00],
    [0x3C, 0x10, 0x1C, 0x10, 0x10, 0x3C, 0x00, 0x00],
    [0x10, 0x3C, 0x10, 0x1C, 0x12, 0x3C, 0x00, 0x00],
    [0x20, 0x3C, 0x24, 0x24, 0x24, 0x06, 0x00, 0x00],
    [0x3C, 0x10, 0x3C, 0x10, 0x0C, 0x30, 0x00, 0x00],
    [0x3C, 0x04, 0x08, 0x10, 0x20, 0x3C, 0x00, 0x00],
    [0x24, 0x24, 0x3C, 0x24, 0x24, 0x06, 0x00, 0x00],
    [0x3C, 0x04, 0x04, 0x04, 0x04, 0x3C, 0x00, 0x00],
    [0x24, 0x24, 0x3C, 0x10, 0x10, 0x0C, 0x00, 0x00],
    [0x04, 0x04, 0x0C, 0x14, 0x24, 0x20, 0x00, 0x00],
    [0x3C, 0x04, 0x1C, 0x20, 0x20, 0x3C, 0x00, 0x00],
    [0x3C, 0x10, 0x3C, 0x10, 0x10, 0x0C, 0x00, 0x00],
    [0x04, 0x08, 0x10, 0x24, 0x24, 0x18, 0x00, 0x00],
    [0x3C, 0x04, 0x3C, 0x24, 0x24, 0x06, 0x00, 0x00],
];

const IROHA_KATAKANA: [char; 47] = [
    'イ', 'ロ', 'ハ', 'ニ', 'ホ', 'ヘ', 'ト', 'チ', 'リ', 'ヌ', 'ル', 'ヲ', 'ワ', 'カ', 'ヨ', 'タ',
    'レ', 'ソ', 'ツ', 'ネ', 'ナ', 'ラ', 'ム', 'ウ', 'ヰ', 'ノ', 'オ', 'ク', 'ヤ', 'マ', 'ケ', 'フ',
    'コ', 'エ', 'テ', 'ア', 'サ', 'キ', 'ユ', 'メ', 'ミ', 'シ', 'ヱ', 'ヒ', 'モ', 'セ', 'ス',
];

fn render_sora_temple_frame(
    grid: &PetalStreamGrid,
    dimension: u32,
    frame_index: u32,
    frame_count: u32,
    style: PetalRenderStyle,
    options: PetalStreamOptions,
) -> image::RgbaImage {
    let style_cfg = temple_style_config(style);
    let grid_size = grid.grid_size as u32;
    let cell_size = (dimension / grid_size).max(1);
    let width = grid_size * cell_size;
    let height = width;
    let offset_x = (dimension.saturating_sub(width)) / 2;
    let offset_y = (dimension.saturating_sub(height)) / 2;
    let frame_count = frame_count.max(1);
    let phase = (frame_index % frame_count) as f64 / frame_count as f64;

    let stream_bits = collect_stream_data_bits(grid, options);
    let stream_signature = stream_bit_signature(&stream_bits);
    let mut cell_params = Vec::with_capacity((grid_size * grid_size) as usize);
    let mut cell_roles = Vec::with_capacity((grid_size * grid_size) as usize);
    for y in 0..grid_size {
        for x in 0..grid_size {
            let idx = y as usize * grid_size as usize + x as usize;
            let role = render_cell_role(x as u16, y as u16, grid.grid_size, options);
            let bit = grid.cells[idx];
            let nx = (x as f64 + 0.5) / grid_size as f64;
            let ny = (y as f64 + 0.5) / grid_size as f64;
            let logo = role == RenderCellRole::Data && sora_ten_logo_mask(nx, ny);
            let mut seed = cell_seed(x, y)
                ^ stream_signature
                ^ (u64::from(frame_index) << 11)
                ^ (u64::from(bit as u8) << 3);
            let kana_index = select_kana_index_for_bit(bit, &mut seed);
            cell_params.push(TempleCellParam {
                bit,
                logo,
                kana_pattern: katakana_pattern_id(kana_index),
            });
            cell_roles.push(role);
        }
    }

    let mut rgba = Vec::with_capacity((dimension * dimension * 4) as usize);
    for py in 0..dimension {
        let ny = if dimension <= 1 {
            0.5
        } else {
            py as f64 / (dimension - 1) as f64
        };
        for px in 0..dimension {
            let nx = if dimension <= 1 {
                0.5
            } else {
                px as f64 / (dimension - 1) as f64
            };
            let cx = nx - 0.5;
            let cy = ny - 0.5;
            let radial = (cx * cx + cy * cy).sqrt();
            let mut rgb = [
                lerp(style_cfg.bg_start[0], style_cfg.bg_end[0], nx),
                lerp(style_cfg.bg_start[1], style_cfg.bg_end[1], ny),
                lerp(
                    style_cfg.bg_start[2],
                    style_cfg.bg_end[2],
                    1.0 - radial.min(1.0),
                ),
            ];
            let core = (1.0 - radial / style_cfg.logo_core_radius).clamp(0.0, 1.0);
            if core > 0.0 {
                blend_rgb(
                    &mut rgb,
                    style_cfg.logo_core_bg,
                    core.powi(2) * style_cfg.logo_core_alpha,
                );
            }
            let scanline = (ny * 210.0 + phase * std::f64::consts::TAU).sin() * 0.5 + 0.5;
            blend_rgb(
                &mut rgb,
                style_cfg.ring_dim,
                scanline * style_cfg.scanline_alpha,
            );
            let vignette = (radial / 0.74).clamp(0.0, 1.0).powi(2) * style_cfg.vignette;
            rgb[0] = lerp(rgb[0], style_cfg.bg_start[0], vignette);
            rgb[1] = lerp(rgb[1], style_cfg.bg_start[1], vignette);
            rgb[2] = lerp(rgb[2], style_cfg.bg_start[2], vignette);

            blend_sora_rings(
                &mut rgb,
                cx,
                cy,
                phase,
                &stream_bits,
                stream_signature,
                style_cfg,
            );

            if px >= offset_x && py >= offset_y && px < offset_x + width && py < offset_y + height {
                let gx = (px - offset_x) / cell_size;
                let gy = (py - offset_y) / cell_size;
                let idx = gy as usize * grid_size as usize + gx as usize;
                let role = cell_roles[idx];
                match role {
                    RenderCellRole::AnchorDark => {
                        rgb = [0.04, 0.02, 0.06];
                    }
                    RenderCellRole::AnchorLight => {
                        rgb = [0.96, 0.93, 0.97];
                    }
                    RenderCellRole::Border => {
                        let bit = grid.cells[idx];
                        let border_mix = if bit {
                            style_cfg.border_dark_mix
                        } else {
                            style_cfg.border_light_mix
                        };
                        blend_rgb(&mut rgb, style_cfg.ring_dim, border_mix);
                    }
                    RenderCellRole::Data => {
                        let local_x = (px - offset_x) as f64 / cell_size as f64 - gx as f64;
                        let local_y = (py - offset_y) as f64 / cell_size as f64 - gy as f64;
                        let param = cell_params[idx];
                        blend_sora_data_tile(
                            &mut rgb,
                            local_x,
                            local_y,
                            gx as usize,
                            gy as usize,
                            cell_size,
                            param,
                            style_cfg,
                        );
                    }
                }
            }

            rgba.extend_from_slice(&[to_u8(rgb[0]), to_u8(rgb[1]), to_u8(rgb[2]), 0xFF]);
        }
    }

    image::RgbaImage::from_raw(dimension, dimension, rgba).expect("rgba buffer size")
}

#[derive(Clone, Copy)]
struct TempleCellParam {
    bit: bool,
    logo: bool,
    kana_pattern: usize,
}

#[derive(Clone, Copy)]
struct TempleStyleConfig {
    bg_start: [f64; 3],
    bg_end: [f64; 3],
    ring_bright: [f64; 3],
    ring_dim: [f64; 3],
    tile_light_bg: [f64; 3],
    tile_light_fg: [f64; 3],
    tile_dark_bg: [f64; 3],
    tile_dark_fg: [f64; 3],
    logo_tint: [f64; 3],
    scanline_alpha: f64,
    vignette: f64,
    ring_band: f64,
    ring_on_alpha: f64,
    ring_off_alpha: f64,
    tile_margin_outer: f64,
    tile_margin_logo: f64,
    tile_glyph_alpha: f64,
    tile_alpha_regular: f64,
    tile_alpha_logo: f64,
    logo_tint_alpha: f64,
    logo_light_bg: [f64; 3],
    logo_dark_bg: [f64; 3],
    logo_tile_mix: f64,
    border_dark_mix: f64,
    border_light_mix: f64,
    logo_core_bg: [f64; 3],
    logo_core_alpha: f64,
    logo_core_radius: f64,
    logo_glyph_boost: f64,
}

fn temple_style_config(style: PetalRenderStyle) -> TempleStyleConfig {
    match style {
        PetalRenderStyle::SoraTemple => TempleStyleConfig {
            bg_start: SORA_BG_START,
            bg_end: SORA_BG_END,
            ring_bright: SORA_RING_BRIGHT,
            ring_dim: SORA_RING_DIM,
            tile_light_bg: SORA_TILE_LIGHT_BG,
            tile_light_fg: SORA_TILE_LIGHT_FG,
            tile_dark_bg: SORA_TILE_DARK_BG,
            tile_dark_fg: SORA_TILE_DARK_FG,
            logo_tint: SORA_LOGO_TINT,
            scanline_alpha: SORA_SCANLINE_ALPHA,
            vignette: SORA_VIGNETTE,
            ring_band: SORA_RING_BAND,
            ring_on_alpha: 0.65,
            ring_off_alpha: 0.28,
            tile_margin_outer: SORA_TILE_MARGIN_OUTER,
            tile_margin_logo: SORA_TILE_MARGIN_LOGO,
            tile_glyph_alpha: SORA_TILE_GLYPH_ALPHA,
            tile_alpha_regular: 0.976,
            tile_alpha_logo: 0.998,
            logo_tint_alpha: 0.24,
            logo_light_bg: [0.96, 0.87, 0.90],
            logo_dark_bg: [0.19, 0.03, 0.06],
            logo_tile_mix: 0.68,
            border_dark_mix: 0.18,
            border_light_mix: 0.12,
            logo_core_bg: [0.78, 0.11, 0.20],
            logo_core_alpha: 0.54,
            logo_core_radius: 0.45,
            logo_glyph_boost: 0.22,
        },
        PetalRenderStyle::SoraTempleBold => TempleStyleConfig {
            bg_start: [0.015, 0.006, 0.045],
            bg_end: [0.06, 0.02, 0.10],
            ring_bright: [0.99, 0.84, 0.93],
            ring_dim: [0.38, 0.22, 0.36],
            tile_light_bg: [0.97, 0.94, 0.98],
            tile_light_fg: [0.08, 0.03, 0.12],
            tile_dark_bg: [0.055, 0.025, 0.08],
            tile_dark_fg: [0.99, 0.93, 0.99],
            logo_tint: [1.0, 0.95, 0.99],
            scanline_alpha: 0.026,
            vignette: 0.30,
            ring_band: 0.0072,
            ring_on_alpha: 0.82,
            ring_off_alpha: 0.42,
            tile_margin_outer: 0.022,
            tile_margin_logo: 0.004,
            tile_glyph_alpha: 0.32,
            tile_alpha_regular: 0.985,
            tile_alpha_logo: 0.998,
            logo_tint_alpha: 0.28,
            logo_light_bg: [0.97, 0.86, 0.90],
            logo_dark_bg: [0.22, 0.03, 0.07],
            logo_tile_mix: 0.74,
            border_dark_mix: 0.23,
            border_light_mix: 0.16,
            logo_core_bg: [0.85, 0.10, 0.21],
            logo_core_alpha: 0.66,
            logo_core_radius: 0.46,
            logo_glyph_boost: 0.26,
        },
        PetalRenderStyle::SoraTempleMinimal => TempleStyleConfig {
            bg_start: [0.03, 0.02, 0.06],
            bg_end: [0.055, 0.025, 0.075],
            ring_bright: [0.88, 0.73, 0.86],
            ring_dim: [0.22, 0.13, 0.24],
            tile_light_bg: [0.955, 0.93, 0.965],
            tile_light_fg: [0.09, 0.04, 0.13],
            tile_dark_bg: [0.075, 0.035, 0.11],
            tile_dark_fg: [0.96, 0.90, 0.97],
            logo_tint: [0.96, 0.90, 0.95],
            scanline_alpha: 0.008,
            vignette: 0.16,
            ring_band: 0.0042,
            ring_on_alpha: 0.30,
            ring_off_alpha: 0.12,
            tile_margin_outer: 0.028,
            tile_margin_logo: 0.006,
            tile_glyph_alpha: 0.24,
            tile_alpha_regular: 0.955,
            tile_alpha_logo: 0.992,
            logo_tint_alpha: 0.18,
            logo_light_bg: [0.95, 0.88, 0.91],
            logo_dark_bg: [0.17, 0.04, 0.07],
            logo_tile_mix: 0.58,
            border_dark_mix: 0.10,
            border_light_mix: 0.06,
            logo_core_bg: [0.70, 0.12, 0.20],
            logo_core_alpha: 0.42,
            logo_core_radius: 0.44,
            logo_glyph_boost: 0.20,
        },
        PetalRenderStyle::SoraTempleRadiant => TempleStyleConfig {
            bg_start: [0.03, 0.01, 0.07],
            bg_end: [0.10, 0.03, 0.14],
            ring_bright: [1.0, 0.86, 0.94],
            ring_dim: [0.44, 0.25, 0.40],
            tile_light_bg: [0.965, 0.94, 0.975],
            tile_light_fg: [0.11, 0.04, 0.14],
            tile_dark_bg: [0.08, 0.03, 0.12],
            tile_dark_fg: [0.98, 0.91, 0.98],
            logo_tint: [1.0, 0.93, 0.98],
            scanline_alpha: 0.024,
            vignette: 0.24,
            ring_band: 0.0065,
            ring_on_alpha: 0.74,
            ring_off_alpha: 0.34,
            tile_margin_outer: 0.024,
            tile_margin_logo: 0.005,
            tile_glyph_alpha: 0.34,
            tile_alpha_regular: 0.978,
            tile_alpha_logo: 0.997,
            logo_tint_alpha: 0.26,
            logo_light_bg: [0.97, 0.86, 0.91],
            logo_dark_bg: [0.20, 0.03, 0.08],
            logo_tile_mix: 0.72,
            border_dark_mix: 0.19,
            border_light_mix: 0.13,
            logo_core_bg: [0.82, 0.10, 0.22],
            logo_core_alpha: 0.62,
            logo_core_radius: 0.47,
            logo_glyph_boost: 0.26,
        },
        PetalRenderStyle::SoraTempleCommand => TempleStyleConfig {
            bg_start: [0.016, 0.007, 0.040],
            bg_end: [0.070, 0.020, 0.108],
            ring_bright: [0.99, 0.80, 0.92],
            ring_dim: [0.30, 0.15, 0.29],
            tile_light_bg: [0.97, 0.94, 0.98],
            tile_light_fg: [0.08, 0.03, 0.12],
            tile_dark_bg: [0.06, 0.02, 0.09],
            tile_dark_fg: [0.99, 0.93, 0.99],
            logo_tint: [1.0, 0.92, 0.98],
            scanline_alpha: 0.030,
            vignette: 0.30,
            ring_band: 0.0060,
            ring_on_alpha: 0.78,
            ring_off_alpha: 0.26,
            tile_margin_outer: 0.022,
            tile_margin_logo: 0.004,
            tile_glyph_alpha: 0.34,
            tile_alpha_regular: 0.985,
            tile_alpha_logo: 0.998,
            logo_tint_alpha: 0.30,
            logo_light_bg: [0.97, 0.86, 0.90],
            logo_dark_bg: [0.21, 0.03, 0.08],
            logo_tile_mix: 0.76,
            border_dark_mix: 0.22,
            border_light_mix: 0.15,
            logo_core_bg: [0.86, 0.11, 0.23],
            logo_core_alpha: 0.68,
            logo_core_radius: 0.47,
            logo_glyph_boost: 0.28,
        },
        PetalRenderStyle::SoraTempleAegis => TempleStyleConfig {
            bg_start: [0.017, 0.009, 0.045],
            bg_end: [0.062, 0.023, 0.095],
            ring_bright: [0.95, 0.76, 0.89],
            ring_dim: [0.24, 0.12, 0.25],
            tile_light_bg: [0.96, 0.93, 0.97],
            tile_light_fg: [0.09, 0.04, 0.12],
            tile_dark_bg: [0.05, 0.02, 0.08],
            tile_dark_fg: [0.97, 0.91, 0.98],
            logo_tint: [0.98, 0.91, 0.97],
            scanline_alpha: 0.018,
            vignette: 0.34,
            ring_band: 0.0067,
            ring_on_alpha: 0.70,
            ring_off_alpha: 0.30,
            tile_margin_outer: 0.024,
            tile_margin_logo: 0.005,
            tile_glyph_alpha: 0.30,
            tile_alpha_regular: 0.982,
            tile_alpha_logo: 0.998,
            logo_tint_alpha: 0.26,
            logo_light_bg: [0.96, 0.87, 0.90],
            logo_dark_bg: [0.18, 0.03, 0.07],
            logo_tile_mix: 0.72,
            border_dark_mix: 0.24,
            border_light_mix: 0.18,
            logo_core_bg: [0.80, 0.11, 0.22],
            logo_core_alpha: 0.60,
            logo_core_radius: 0.46,
            logo_glyph_boost: 0.24,
        },
        PetalRenderStyle::SoraTempleGhost => TempleStyleConfig {
            bg_start: [0.022, 0.012, 0.050],
            bg_end: [0.074, 0.024, 0.106],
            ring_bright: [0.90, 0.73, 0.86],
            ring_dim: [0.20, 0.10, 0.22],
            tile_light_bg: [0.95, 0.92, 0.96],
            tile_light_fg: [0.10, 0.04, 0.13],
            tile_dark_bg: [0.07, 0.03, 0.10],
            tile_dark_fg: [0.95, 0.89, 0.96],
            logo_tint: [0.96, 0.90, 0.95],
            scanline_alpha: 0.012,
            vignette: 0.22,
            ring_band: 0.0052,
            ring_on_alpha: 0.58,
            ring_off_alpha: 0.20,
            tile_margin_outer: 0.026,
            tile_margin_logo: 0.006,
            tile_glyph_alpha: 0.26,
            tile_alpha_regular: 0.972,
            tile_alpha_logo: 0.996,
            logo_tint_alpha: 0.23,
            logo_light_bg: [0.95, 0.88, 0.92],
            logo_dark_bg: [0.17, 0.04, 0.07],
            logo_tile_mix: 0.70,
            border_dark_mix: 0.16,
            border_light_mix: 0.10,
            logo_core_bg: [0.74, 0.11, 0.20],
            logo_core_alpha: 0.50,
            logo_core_radius: 0.45,
            logo_glyph_boost: 0.20,
        },
        PetalRenderStyle::SakuraWind => TempleStyleConfig {
            bg_start: SORA_BG_START,
            bg_end: SORA_BG_END,
            ring_bright: SORA_RING_BRIGHT,
            ring_dim: SORA_RING_DIM,
            tile_light_bg: SORA_TILE_LIGHT_BG,
            tile_light_fg: SORA_TILE_LIGHT_FG,
            tile_dark_bg: SORA_TILE_DARK_BG,
            tile_dark_fg: SORA_TILE_DARK_FG,
            logo_tint: SORA_LOGO_TINT,
            scanline_alpha: SORA_SCANLINE_ALPHA,
            vignette: SORA_VIGNETTE,
            ring_band: SORA_RING_BAND,
            ring_on_alpha: 0.65,
            ring_off_alpha: 0.28,
            tile_margin_outer: SORA_TILE_MARGIN_OUTER,
            tile_margin_logo: SORA_TILE_MARGIN_LOGO,
            tile_glyph_alpha: SORA_TILE_GLYPH_ALPHA,
            tile_alpha_regular: 0.976,
            tile_alpha_logo: 0.998,
            logo_tint_alpha: 0.24,
            logo_light_bg: [0.96, 0.87, 0.90],
            logo_dark_bg: [0.19, 0.03, 0.06],
            logo_tile_mix: 0.68,
            border_dark_mix: 0.18,
            border_light_mix: 0.12,
            logo_core_bg: [0.78, 0.11, 0.20],
            logo_core_alpha: 0.54,
            logo_core_radius: 0.45,
            logo_glyph_boost: 0.22,
        },
    }
}

fn blend_sora_data_tile(
    rgb: &mut [f64; 3],
    local_x: f64,
    local_y: f64,
    gx: usize,
    gy: usize,
    cell_size: u32,
    param: TempleCellParam,
    style_cfg: TempleStyleConfig,
) {
    let style_margin = if param.logo {
        style_cfg.tile_margin_logo
    } else {
        style_cfg.tile_margin_outer
    };
    let mut margin = if cell_size <= 4 {
        0.0
    } else if cell_size <= 6 {
        style_margin.min(0.04)
    } else {
        style_margin
    };
    if cell_size >= 8 {
        // At larger render sizes, open up each tile so boxes read bolder on screen.
        margin = (margin * 0.58).max(0.006);
    }
    if cell_size >= 10 {
        margin = (margin * 0.82).max(0.005);
    }
    if local_x < margin || local_x > 1.0 - margin || local_y < margin || local_y > 1.0 - margin {
        return;
    }

    let tile_alpha = if param.logo {
        style_cfg.tile_alpha_logo
    } else {
        style_cfg.tile_alpha_regular
    };
    let (tile_bg, glyph_fg) = if param.bit {
        (style_cfg.tile_dark_bg, style_cfg.tile_dark_fg)
    } else {
        (style_cfg.tile_light_bg, style_cfg.tile_light_fg)
    };
    blend_rgb(rgb, tile_bg, tile_alpha);

    // Add angular UI framing so each tile reads like a sci-fi interface cell.
    let frame_color = if param.bit {
        style_cfg.tile_dark_fg
    } else {
        style_cfg.tile_light_fg
    };
    if param.logo {
        let logo_bg = if param.bit {
            style_cfg.logo_dark_bg
        } else {
            style_cfg.logo_light_bg
        };
        blend_rgb(rgb, logo_bg, style_cfg.logo_tile_mix);
        blend_rgb(rgb, style_cfg.logo_tint, style_cfg.logo_tint_alpha);
    }

    let inner_u = ((local_x - margin) / (1.0 - 2.0 * margin)).clamp(0.0, 0.9999);
    let inner_v = ((local_y - margin) / (1.0 - 2.0 * margin)).clamp(0.0, 0.9999);
    if cell_size >= 8 {
        let frame_dist = inner_u.min(1.0 - inner_u).min(inner_v.min(1.0 - inner_v));
        if frame_dist < if param.logo { 0.024 } else { 0.030 } {
            blend_rgb(rgb, frame_color, if param.logo { 0.08 } else { 0.09 });
        }
        let bracket_len = if param.logo { 0.24 } else { 0.26 };
        let bracket_w = if param.logo { 0.046 } else { 0.052 };
        let corner_bracket = (inner_u < bracket_len && inner_v < bracket_w)
            || (inner_u < bracket_w && inner_v < bracket_len)
            || (inner_u > 1.0 - bracket_len && inner_v < bracket_w)
            || (inner_u > 1.0 - bracket_w && inner_v < bracket_len)
            || (inner_u < bracket_len && inner_v > 1.0 - bracket_w)
            || (inner_u < bracket_w && inner_v > 1.0 - bracket_len)
            || (inner_u > 1.0 - bracket_len && inner_v > 1.0 - bracket_w)
            || (inner_u > 1.0 - bracket_w && inner_v > 1.0 - bracket_len);
        if corner_bracket {
            blend_rgb(rgb, frame_color, if param.logo { 0.12 } else { 0.14 });
        }
    }

    let bitmap = katakana_bitmap(param.kana_pattern, gx, gy);
    if katakana_bitmap_hit(bitmap, inner_u, inner_v) {
        let glyph_alpha = if param.logo {
            style_cfg.tile_glyph_alpha + style_cfg.logo_glyph_boost
        } else {
            style_cfg.tile_glyph_alpha
        };
        blend_rgb(rgb, glyph_fg, glyph_alpha);
    }
}

fn katakana_pattern_id(kana_index: usize) -> usize {
    let char_seed = IROHA_KATAKANA[kana_index % IROHA_KATAKANA.len()] as usize;
    char_seed % SORA_KATAKANA_BITMAPS.len()
}

fn select_kana_index_for_bit(bit: bool, seed: &mut u64) -> usize {
    let desired_parity = usize::from(bit);
    let mut candidates = [0usize; IROHA_KATAKANA.len()];
    let mut count = 0usize;
    for index in 0..IROHA_KATAKANA.len() {
        if katakana_pattern_id(index) % 2 == desired_parity {
            candidates[count] = index;
            count += 1;
        }
    }
    if count == 0 {
        return (sample_u32(seed) as usize) % IROHA_KATAKANA.len();
    }
    candidates[(sample_u32(seed) as usize) % count]
}

fn katakana_bitmap(pattern_id: usize, x: usize, y: usize) -> [u8; 8] {
    let pattern = SORA_KATAKANA_BITMAPS[pattern_id % SORA_KATAKANA_BITMAPS.len()];
    let rotate = (x.wrapping_mul(3) + y.wrapping_mul(5)) % 3;
    if rotate == 0 {
        return shift_katakana_bitmap(pattern, SORA_KATAKANA_SHIFT_X, SORA_KATAKANA_SHIFT_Y);
    }
    let mut out = [0u8; 8];
    for row in 0..8 {
        let src = pattern[(row + rotate) % 8];
        out[row] = src.rotate_left(rotate as u32);
    }
    shift_katakana_bitmap(out, SORA_KATAKANA_SHIFT_X, SORA_KATAKANA_SHIFT_Y)
}

fn shift_katakana_bitmap(bitmap: [u8; 8], shift_x: i32, shift_y: i32) -> [u8; 8] {
    if shift_x == 0 && shift_y == 0 {
        return bitmap;
    }
    let mut out = [0u8; 8];
    for y in 0..8i32 {
        let row = bitmap[y as usize];
        for x in 0..8i32 {
            if row & (1u8 << (7 - x)) == 0 {
                continue;
            }
            let nx = x + shift_x;
            let ny = y + shift_y;
            if !(0..8).contains(&nx) || !(0..8).contains(&ny) {
                continue;
            }
            out[ny as usize] |= 1u8 << (7 - nx);
        }
    }
    out
}

fn katakana_bitmap_hit(bitmap: [u8; 8], u: f64, v: f64) -> bool {
    let gx = (u * 8.0).floor() as usize;
    let gy = (v * 8.0).floor() as usize;
    let gx = gx.min(7);
    let gy = gy.min(7);
    let row = bitmap[gy];
    row & (1u8 << (7 - gx)) != 0
}

fn blend_sora_rings(
    rgb: &mut [f64; 3],
    cx: f64,
    cy: f64,
    phase: f64,
    stream_bits: &[bool],
    stream_signature: u64,
    style_cfg: TempleStyleConfig,
) {
    if stream_bits.is_empty() {
        return;
    }
    let radius = (cx * cx + cy * cy).sqrt();
    let angle = cy.atan2(cx);
    let angle_u = (angle / std::f64::consts::TAU).rem_euclid(1.0);
    let mut ring_offset = 0usize;
    for ring_index in 0..SORA_RING_RADII.len() {
        let ring_radius = SORA_RING_RADII[ring_index];
        let dist = (radius - ring_radius).abs();
        if dist > style_cfg.ring_band * 2.2 {
            ring_offset += SORA_RING_DOTS[ring_index] as usize;
            continue;
        }
        let dots = SORA_RING_DOTS[ring_index];
        let spin = (angle_u + phase * SORA_RING_SPEED[ring_index]).rem_euclid(1.0);
        let slot = spin * dots;
        let slot_center = slot.round();
        let slot_index = slot_center as usize % dots as usize;
        let angular_dist = (slot - slot_center).abs().min(0.5);
        let dot_profile = (1.0 - angular_dist * 2.0).max(0.0).powf(2.2);
        let bit_index =
            (ring_offset + slot_index + (stream_signature as usize % 23)) % stream_bits.len();
        let bit = stream_bits[bit_index];
        let ring_color = if bit {
            style_cfg.ring_bright
        } else {
            style_cfg.ring_dim
        };
        let alpha = dot_profile.powi(2)
            * (1.0 - dist / (style_cfg.ring_band * 2.2))
            * if bit {
                style_cfg.ring_on_alpha
            } else {
                style_cfg.ring_off_alpha
            };
        blend_rgb(rgb, ring_color, alpha);
        ring_offset += dots as usize;
    }
}

fn blend_rgb(rgb: &mut [f64; 3], target: [f64; 3], alpha: f64) {
    let alpha = alpha.clamp(0.0, 1.0);
    rgb[0] = lerp(rgb[0], target[0], alpha);
    rgb[1] = lerp(rgb[1], target[1], alpha);
    rgb[2] = lerp(rgb[2], target[2], alpha);
}

fn sora_ten_logo_mask(nx: f64, ny: f64) -> bool {
    let x = (nx * 2.0 - 1.0) / 0.93;
    let y = (ny * 2.0 - 1.0) / 0.93;
    let top_bar = y >= -0.72 && y <= -0.50 && x.abs() <= 0.82;
    let mid_bar = y >= -0.37 && y <= -0.18 && x.abs() <= 0.58;
    let stem = y >= -0.20 && y <= 0.00 && x.abs() <= 0.15;
    let left_leg = y >= -0.02 && y <= 0.92 && (x + 0.64 * y).abs() <= 0.13;
    let right_leg = y >= -0.02 && y <= 0.92 && (x - 0.64 * y).abs() <= 0.13;
    let foot = y >= 0.75 && y <= 0.93 && x.abs() <= 0.58;
    top_bar || mid_bar || stem || left_leg || right_leg || foot
}

fn collect_stream_data_bits(grid: &PetalStreamGrid, options: PetalStreamOptions) -> Vec<bool> {
    let mut bits = Vec::new();
    for y in 0..grid.grid_size {
        for x in 0..grid.grid_size {
            if render_cell_role(x, y, grid.grid_size, options) == RenderCellRole::Data {
                let idx = y as usize * grid.grid_size as usize + x as usize;
                bits.push(grid.cells[idx]);
            }
        }
    }
    bits
}

fn stream_bit_signature(bits: &[bool]) -> u64 {
    let mut sig = 0x6D_61_74_73_75u64;
    for (idx, bit) in bits.iter().enumerate().take(128) {
        if *bit {
            sig ^= 1u64 << (idx % 63);
        }
        sig = splitmix64(sig ^ idx as u64);
    }
    sig
}

fn sample_u32(seed: &mut u64) -> u32 {
    *seed = splitmix64(*seed);
    (*seed >> 32) as u32
}

fn sample_grid_from_rgba(
    image: &image::RgbaImage,
    grid_size: u16,
    options: PetalStreamOptions,
) -> Result<PetalStreamSampleGrid> {
    let width = image.width();
    let height = image.height();
    let size = width.min(height);
    let cell_size = (size / grid_size as u32).max(1);
    let grid_pixels = grid_size as u32 * cell_size;
    let offset_x = (size.saturating_sub(grid_pixels)) / 2;
    let offset_y = (size.saturating_sub(grid_pixels)) / 2;
    let mut samples = Vec::with_capacity(grid_size as usize * grid_size as usize);
    for y in 0..grid_size {
        for x in 0..grid_size {
            let mut sum = 0u64;
            let mut count = 0u64;
            for oy in [0.25, 0.5, 0.75] {
                for ox in [0.25, 0.5, 0.75] {
                    let px = offset_x + ((x as f64 + ox) * cell_size as f64).floor() as u32;
                    let py = offset_y + ((y as f64 + oy) * cell_size as f64).floor() as u32;
                    let px = px.min(width - 1);
                    let py = py.min(height - 1);
                    let pixel = image.get_pixel(px, py).0;
                    let luma = (77u32 * pixel[0] as u32
                        + 150u32 * pixel[1] as u32
                        + 29u32 * pixel[2] as u32)
                        >> 8;
                    sum += luma as u64;
                    count += 1;
                }
            }
            let avg = if count == 0 { 0 } else { (sum / count) as u8 };
            samples.push(avg);
        }
    }
    apply_katakana_assist_to_samples(
        image,
        grid_size,
        options,
        cell_size,
        offset_x,
        offset_y,
        &mut samples,
    );
    PetalStreamSampleGrid::new(grid_size, samples).map_err(|err| eyre!("{err}"))
}

fn derive_anchor_threshold(
    samples: &[u8],
    grid_size: u16,
    options: PetalStreamOptions,
) -> Option<u8> {
    let mut dark_sum = 0u64;
    let mut dark_count = 0u64;
    let mut light_sum = 0u64;
    let mut light_count = 0u64;
    for y in 0..grid_size {
        for x in 0..grid_size {
            let idx = y as usize * grid_size as usize + x as usize;
            match render_cell_role(x, y, grid_size, options) {
                RenderCellRole::AnchorDark => {
                    dark_sum += u64::from(samples[idx]);
                    dark_count += 1;
                }
                RenderCellRole::AnchorLight => {
                    light_sum += u64::from(samples[idx]);
                    light_count += 1;
                }
                _ => {}
            }
        }
    }
    if dark_count == 0 || light_count == 0 {
        return None;
    }
    let dark_avg = dark_sum / dark_count;
    let light_avg = light_sum / light_count;
    if dark_avg + 1 >= light_avg {
        return None;
    }
    u8::try_from((dark_avg + light_avg) / 2).ok()
}

fn apply_katakana_assist_to_samples(
    image: &image::RgbaImage,
    grid_size: u16,
    options: PetalStreamOptions,
    cell_size: u32,
    offset_x: u32,
    offset_y: u32,
    samples: &mut [u8],
) {
    if cell_size < 6 {
        return;
    }
    let Some(threshold) = derive_anchor_threshold(samples, grid_size, options) else {
        return;
    };

    let mut overrides = Vec::new();
    for y in 0..grid_size {
        for x in 0..grid_size {
            if render_cell_role(x, y, grid_size, options) != RenderCellRole::Data {
                continue;
            }
            let idx = y as usize * grid_size as usize + x as usize;
            let sample = samples[idx];
            if sample.abs_diff(threshold) > SORA_KATAKANA_UNCERTAIN_BAND {
                continue;
            }
            let Some((bit, confidence)) =
                infer_katakana_cell_bit(image, x, y, cell_size, offset_x, offset_y)
            else {
                continue;
            };
            if confidence < SORA_KATAKANA_CONFIDENCE_MIN {
                continue;
            }
            overrides.push((idx, bit, confidence));
        }
    }
    if overrides.len() < SORA_KATAKANA_MIN_ASSIST_SAMPLES {
        return;
    }

    for (idx, bit, confidence) in overrides {
        let sample = samples[idx];
        if confidence < SORA_KATAKANA_CONFIDENCE_STRONG
            && sample.abs_diff(threshold) > SORA_KATAKANA_UNCERTAIN_BAND
        {
            continue;
        }
        samples[idx] = if bit {
            threshold.saturating_sub(SORA_KATAKANA_OVERRIDE_PUSH)
        } else {
            threshold.saturating_add(SORA_KATAKANA_OVERRIDE_PUSH)
        };
    }
}

fn infer_katakana_cell_bit(
    image: &image::RgbaImage,
    cell_x: u16,
    cell_y: u16,
    cell_size: u32,
    offset_x: u32,
    offset_y: u32,
) -> Option<(bool, f64)> {
    if cell_size == 0 {
        return None;
    }
    let width = image.width();
    let height = image.height();
    let margin = if cell_size <= 6 { 0.05 } else { 0.035 };
    let span = 1.0 - margin * 2.0;
    if span <= 0.0 {
        return None;
    }

    let mut luma = [0u16; 64];
    for sy in 0..8usize {
        for sx in 0..8usize {
            let u = (sx as f64 + 0.5) / 8.0;
            let v = (sy as f64 + 0.5) / 8.0;
            let fx = cell_x as f64 + margin + span * u;
            let fy = cell_y as f64 + margin + span * v;
            let px = (offset_x as f64 + fx * cell_size as f64).floor() as u32;
            let py = (offset_y as f64 + fy * cell_size as f64).floor() as u32;
            let px = px.min(width.saturating_sub(1));
            let py = py.min(height.saturating_sub(1));
            let pixel = image.get_pixel(px, py).0;
            let value =
                (77u32 * pixel[0] as u32 + 150u32 * pixel[1] as u32 + 29u32 * pixel[2] as u32) >> 8;
            luma[sy * 8 + sx] = value as u16;
        }
    }

    let mut best_true = f64::NEG_INFINITY;
    let mut best_false = f64::NEG_INFINITY;
    for pattern_id in 0..SORA_KATAKANA_BITMAPS.len() {
        let bitmap = katakana_bitmap(pattern_id, cell_x as usize, cell_y as usize);
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let mut on_sum = 0u32;
                let mut on_count = 0u32;
                let mut off_sum = 0u32;
                let mut off_count = 0u32;
                for sy in 0..8i32 {
                    for sx in 0..8i32 {
                        let sample = u32::from(luma[sy as usize * 8 + sx as usize]);
                        let bx = sx - dx;
                        let by = sy - dy;
                        let hit = if (0..8).contains(&bx) && (0..8).contains(&by) {
                            bitmap[by as usize] & (1u8 << (7 - bx as u32)) != 0
                        } else {
                            false
                        };
                        if hit {
                            on_sum += sample;
                            on_count += 1;
                        } else {
                            off_sum += sample;
                            off_count += 1;
                        }
                    }
                }
                if on_count == 0 || off_count == 0 {
                    continue;
                }
                let contrast = on_sum as f64 / on_count as f64 - off_sum as f64 / off_count as f64;
                if pattern_id % 2 == 1 {
                    best_true = best_true.max(contrast);
                } else {
                    best_false = best_false.max(-contrast);
                }
            }
        }
    }
    if !best_true.is_finite() || !best_false.is_finite() {
        return None;
    }
    let (bit, best, other) = if best_true >= best_false {
        (true, best_true, best_false)
    } else {
        (false, best_false, best_true)
    };
    if best < 3.0 {
        return None;
    }
    let confidence = ((best - other) / 255.0).clamp(0.0, 1.0);
    Some((bit, confidence))
}

fn load_png(path: impl AsRef<Path>) -> Result<image::RgbaImage> {
    let reader = image::ImageReader::open(&path)
        .map_err(|err| eyre!("failed to read {}: {err}", path.as_ref().display()))?
        .with_guessed_format()
        .map_err(|err| eyre!("failed to detect image format: {err}"))?;
    let image = reader
        .decode()
        .map_err(|err| eyre!("failed to decode {}: {err}", path.as_ref().display()))?;
    Ok(image.to_rgba8())
}

fn decode_frame_with_grid(
    image: &image::RgbaImage,
    grid_size: u16,
    options: PetalStreamOptions,
) -> Result<Vec<u8>> {
    if grid_size == 0 {
        return Err(eyre!("grid size must be > 0"));
    }
    let samples = sample_grid_from_rgba(image, grid_size, options)?;
    PetalStreamDecoder::decode_samples(&samples, options).map_err(|err| eyre!("{err}"))
}

fn decode_frame_auto(
    image: &image::RgbaImage,
    options: PetalStreamOptions,
) -> Result<(u16, Vec<u8>)> {
    for &candidate in PETAL_STREAM_GRID_SIZES {
        if candidate == 0 {
            continue;
        }
        if let Ok(samples) = sample_grid_from_rgba(image, candidate, options) {
            if let Ok(bytes) = PetalStreamDecoder::decode_samples(&samples, options) {
                return Ok((candidate, bytes));
            }
        }
    }
    Err(eyre!(
        "failed to auto-detect grid size; try --grid-size <cells>"
    ))
}

#[derive(Clone, Copy)]
struct CaptureScenario {
    name: &'static str,
    min_scale: f64,
    max_scale: f64,
    min_blur_sigma: f32,
    max_blur_sigma: f32,
    max_motion_pixels: u8,
    max_jitter_pixels: i32,
    min_brightness_shift: f64,
    max_brightness_shift: f64,
    min_contrast: f64,
    max_contrast: f64,
    min_gamma: f64,
    max_gamma: f64,
    max_noise: f64,
}

#[derive(Clone, Debug)]
struct CaptureScenarioMetrics {
    name: &'static str,
    attempts: u64,
    successes: u64,
}

#[derive(Debug)]
struct CaptureEvalMetrics {
    frame_count: usize,
    frame_successes: usize,
    attempts: u64,
    successes: u64,
    recovered_frames: usize,
    stream_complete: bool,
    stream_received_chunks: usize,
    stream_total_chunks: usize,
    scenario: Vec<CaptureScenarioMetrics>,
}

impl CaptureEvalMetrics {
    fn success_ratio(&self) -> f64 {
        if self.attempts == 0 {
            return 0.0;
        }
        self.successes as f64 / self.attempts as f64
    }

    fn frame_success_ratio(&self) -> f64 {
        if self.frame_count == 0 {
            return 0.0;
        }
        self.frame_successes as f64 / self.frame_count as f64
    }

    fn to_json(
        &self,
        profile: PetalCaptureProfile,
        grid_size: u16,
        trials_per_frame: u16,
        min_success_ratio: f64,
        seed: u64,
    ) -> Value {
        let mut map = Map::new();
        map.insert("version".to_string(), Value::from(1u64));
        map.insert("profile".to_string(), Value::from(profile.label()));
        map.insert("seed".to_string(), Value::from(seed));
        map.insert("grid_size".to_string(), Value::from(grid_size as u64));
        map.insert(
            "trials_per_frame".to_string(),
            Value::from(trials_per_frame as u64),
        );
        map.insert(
            "min_success_ratio".to_string(),
            Value::from(min_success_ratio),
        );
        map.insert(
            "frame_count".to_string(),
            Value::from(self.frame_count as u64),
        );
        map.insert(
            "frame_successes".to_string(),
            Value::from(self.frame_successes as u64),
        );
        map.insert(
            "frame_success_ratio".to_string(),
            Value::from(self.frame_success_ratio()),
        );
        map.insert("attempts".to_string(), Value::from(self.attempts));
        map.insert("successes".to_string(), Value::from(self.successes));
        map.insert(
            "success_ratio".to_string(),
            Value::from(self.success_ratio()),
        );
        map.insert(
            "recovered_frames".to_string(),
            Value::from(self.recovered_frames as u64),
        );
        map.insert(
            "stream_complete".to_string(),
            Value::from(self.stream_complete),
        );
        map.insert(
            "stream_received_chunks".to_string(),
            Value::from(self.stream_received_chunks as u64),
        );
        map.insert(
            "stream_total_chunks".to_string(),
            Value::from(self.stream_total_chunks as u64),
        );
        let mut scenario_rows = Vec::with_capacity(self.scenario.len());
        for scenario in &self.scenario {
            let mut scenario_map = Map::new();
            scenario_map.insert("name".to_string(), Value::from(scenario.name));
            scenario_map.insert("attempts".to_string(), Value::from(scenario.attempts));
            scenario_map.insert("successes".to_string(), Value::from(scenario.successes));
            let ratio = if scenario.attempts == 0 {
                0.0
            } else {
                scenario.successes as f64 / scenario.attempts as f64
            };
            scenario_map.insert("success_ratio".to_string(), Value::from(ratio));
            scenario_rows.push(Value::Object(scenario_map));
        }
        map.insert("scenarios".to_string(), Value::Array(scenario_rows));
        Value::Object(map)
    }
}

const DEFAULT_CAPTURE_SCENARIOS: [CaptureScenario; 4] = [
    CaptureScenario {
        name: "distance_soft",
        min_scale: 0.72,
        max_scale: 0.90,
        min_blur_sigma: 0.10,
        max_blur_sigma: 0.75,
        max_motion_pixels: 1,
        max_jitter_pixels: 2,
        min_brightness_shift: -0.05,
        max_brightness_shift: 0.04,
        min_contrast: 0.92,
        max_contrast: 1.08,
        min_gamma: 0.92,
        max_gamma: 1.08,
        max_noise: 0.012,
    },
    CaptureScenario {
        name: "pan_lateral",
        min_scale: 0.70,
        max_scale: 0.88,
        min_blur_sigma: 0.15,
        max_blur_sigma: 0.95,
        max_motion_pixels: 2,
        max_jitter_pixels: 4,
        min_brightness_shift: -0.08,
        max_brightness_shift: 0.03,
        min_contrast: 0.90,
        max_contrast: 1.06,
        min_gamma: 0.94,
        max_gamma: 1.12,
        max_noise: 0.016,
    },
    CaptureScenario {
        name: "shake_noise",
        min_scale: 0.74,
        max_scale: 0.91,
        min_blur_sigma: 0.20,
        max_blur_sigma: 1.10,
        max_motion_pixels: 2,
        max_jitter_pixels: 5,
        min_brightness_shift: -0.10,
        max_brightness_shift: 0.02,
        min_contrast: 0.88,
        max_contrast: 1.02,
        min_gamma: 0.95,
        max_gamma: 1.15,
        max_noise: 0.020,
    },
    CaptureScenario {
        name: "lowlight_scan",
        min_scale: 0.76,
        max_scale: 0.93,
        min_blur_sigma: 0.00,
        max_blur_sigma: 0.65,
        max_motion_pixels: 1,
        max_jitter_pixels: 3,
        min_brightness_shift: -0.14,
        max_brightness_shift: -0.02,
        min_contrast: 0.90,
        max_contrast: 1.04,
        min_gamma: 1.00,
        max_gamma: 1.20,
        max_noise: 0.018,
    },
];

const AGGRESSIVE_CAPTURE_SCENARIOS: [CaptureScenario; 4] = [
    CaptureScenario {
        name: "distance_hard",
        min_scale: 0.56,
        max_scale: 0.82,
        min_blur_sigma: 0.35,
        max_blur_sigma: 1.55,
        max_motion_pixels: 3,
        max_jitter_pixels: 7,
        min_brightness_shift: -0.16,
        max_brightness_shift: 0.04,
        min_contrast: 0.82,
        max_contrast: 1.06,
        min_gamma: 0.95,
        max_gamma: 1.20,
        max_noise: 0.025,
    },
    CaptureScenario {
        name: "moving_camera",
        min_scale: 0.58,
        max_scale: 0.84,
        min_blur_sigma: 0.15,
        max_blur_sigma: 1.20,
        max_motion_pixels: 4,
        max_jitter_pixels: 9,
        min_brightness_shift: -0.12,
        max_brightness_shift: 0.03,
        min_contrast: 0.84,
        max_contrast: 1.10,
        min_gamma: 0.90,
        max_gamma: 1.18,
        max_noise: 0.030,
    },
    CaptureScenario {
        name: "tilt_glare",
        min_scale: 0.62,
        max_scale: 0.86,
        min_blur_sigma: 0.30,
        max_blur_sigma: 1.45,
        max_motion_pixels: 3,
        max_jitter_pixels: 6,
        min_brightness_shift: -0.04,
        max_brightness_shift: 0.12,
        min_contrast: 0.80,
        max_contrast: 1.14,
        min_gamma: 0.85,
        max_gamma: 1.08,
        max_noise: 0.028,
    },
    CaptureScenario {
        name: "noisy_lowlight",
        min_scale: 0.60,
        max_scale: 0.84,
        min_blur_sigma: 0.20,
        max_blur_sigma: 1.10,
        max_motion_pixels: 2,
        max_jitter_pixels: 8,
        min_brightness_shift: -0.20,
        max_brightness_shift: -0.02,
        min_contrast: 0.80,
        max_contrast: 1.02,
        min_gamma: 1.02,
        max_gamma: 1.26,
        max_noise: 0.035,
    },
];

fn capture_scenarios(profile: PetalCaptureProfile) -> &'static [CaptureScenario] {
    match profile {
        PetalCaptureProfile::Default => &DEFAULT_CAPTURE_SCENARIOS,
        PetalCaptureProfile::Aggressive => &AGGRESSIVE_CAPTURE_SCENARIOS,
    }
}

fn evaluate_capture_robustness(
    images: &[image::RgbaImage],
    expected_bytes: &[Vec<u8>],
    grid_size: u16,
    options: PetalStreamOptions,
    profile: PetalCaptureProfile,
    seed: u64,
    trials_per_frame: u16,
) -> Result<CaptureEvalMetrics> {
    if images.len() != expected_bytes.len() {
        return Err(eyre!("frame/image length mismatch"));
    }
    let scenarios = capture_scenarios(profile);
    if scenarios.is_empty() {
        return Err(eyre!("capture profile has no scenarios"));
    }
    let mut scenario_metrics = scenarios
        .iter()
        .map(|scenario| CaptureScenarioMetrics {
            name: scenario.name,
            attempts: 0,
            successes: 0,
        })
        .collect::<Vec<_>>();

    let mut attempts = 0u64;
    let mut successes = 0u64;
    let mut frame_successes = 0usize;
    let mut recovered = vec![None; images.len()];
    let search_radius = capture_alignment_search_radius(profile);
    for (frame_index, (image, expected)) in images.iter().zip(expected_bytes.iter()).enumerate() {
        let mut frame_ok = false;
        for trial in 0..trials_per_frame {
            let scenario_idx = (frame_index + usize::from(trial)) % scenarios.len();
            let scenario = scenarios[scenario_idx];
            scenario_metrics[scenario_idx].attempts += 1;
            attempts += 1;
            let simulated =
                simulate_capture_frame(image, scenario, frame_index, usize::from(trial), seed);
            let decoded =
                decode_frame_with_alignment_search(&simulated, grid_size, options, search_radius);
            let ok = decoded.map(|bytes| bytes == *expected).unwrap_or(false);
            if ok {
                successes += 1;
                frame_ok = true;
                scenario_metrics[scenario_idx].successes += 1;
                recovered[frame_index] = Some(expected.clone());
            }
        }
        if frame_ok {
            frame_successes += 1;
        }
    }

    let recovered_frames = recovered.iter().filter(|entry| entry.is_some()).count();
    let (stream_complete, stream_received_chunks, stream_total_chunks) =
        reconstruct_stream_completion(&recovered)?;

    Ok(CaptureEvalMetrics {
        frame_count: images.len(),
        frame_successes,
        attempts,
        successes,
        recovered_frames,
        stream_complete,
        stream_received_chunks,
        stream_total_chunks,
        scenario: scenario_metrics,
    })
}

fn capture_alignment_search_radius(profile: PetalCaptureProfile) -> i32 {
    match profile {
        PetalCaptureProfile::Default => 4,
        PetalCaptureProfile::Aggressive => 7,
    }
}

fn decode_frame_with_alignment_search(
    image: &image::RgbaImage,
    grid_size: u16,
    options: PetalStreamOptions,
    search_radius: i32,
) -> Result<Vec<u8>> {
    if let Ok(bytes) = decode_frame_with_grid(image, grid_size, options) {
        return Ok(bytes);
    }
    for radius in 1..=search_radius.max(0) {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dy.abs() != radius {
                    continue;
                }
                let shifted = translate_image(image, dx, dy, image::Rgba([4, 1, 8, 0xFF]));
                if let Ok(bytes) = decode_frame_with_grid(&shifted, grid_size, options) {
                    return Ok(bytes);
                }
            }
        }
    }
    Err(eyre!("decode failed after alignment search"))
}

fn reconstruct_stream_completion(recovered: &[Option<Vec<u8>>]) -> Result<(bool, usize, usize)> {
    let mut assembler = QrStreamAssembler::default();
    let mut final_result: Option<QrStreamDecodeResult> = None;
    for bytes in recovered.iter().flatten() {
        let frame = QrStreamFrame::decode(bytes)?;
        final_result = Some(assembler.ingest_frame(frame)?);
    }
    if let Some(result) = final_result {
        Ok((
            result.is_complete(),
            result.received_chunks,
            result.total_chunks,
        ))
    } else {
        Ok((false, 0, 0))
    }
}

fn simulate_capture_frame(
    source: &image::RgbaImage,
    scenario: CaptureScenario,
    frame_index: usize,
    trial_index: usize,
    seed: u64,
) -> image::RgbaImage {
    let mut local_seed = seed
        ^ (frame_index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (trial_index as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let width = source.width();
    let height = source.height();

    let scale = sample_range(&mut local_seed, scenario.min_scale, scenario.max_scale);
    let resized_w = ((width as f64 * scale).round() as u32).clamp(24, width.max(24));
    let resized_h = ((height as f64 * scale).round() as u32).clamp(24, height.max(24));
    let mut image = image::imageops::resize(
        source,
        resized_w,
        resized_h,
        image::imageops::FilterType::Triangle,
    );
    image = image::imageops::resize(
        &image,
        width,
        height,
        image::imageops::FilterType::CatmullRom,
    );

    let blur_sigma = sample_range(
        &mut local_seed,
        scenario.min_blur_sigma as f64,
        scenario.max_blur_sigma as f64,
    ) as f32;
    if blur_sigma > 0.01 {
        image = image::imageops::blur(&image, blur_sigma);
    }

    if scenario.max_motion_pixels > 0 {
        let motion_span =
            sample_range(&mut local_seed, 0.0, f64::from(scenario.max_motion_pixels)).round() as u8;
        if motion_span > 0 {
            image = apply_motion_blur(&image, motion_span, &mut local_seed);
        }
    }

    if scenario.max_jitter_pixels > 0 {
        let dx = sample_i32_range(
            &mut local_seed,
            -scenario.max_jitter_pixels,
            scenario.max_jitter_pixels,
        );
        let dy = sample_i32_range(
            &mut local_seed,
            -scenario.max_jitter_pixels,
            scenario.max_jitter_pixels,
        );
        image = translate_image(&image, dx, dy, image::Rgba([4, 1, 8, 0xFF]));
    }

    let brightness = sample_range(
        &mut local_seed,
        scenario.min_brightness_shift,
        scenario.max_brightness_shift,
    );
    let contrast = sample_range(
        &mut local_seed,
        scenario.min_contrast,
        scenario.max_contrast,
    );
    let gamma = sample_range(&mut local_seed, scenario.min_gamma, scenario.max_gamma);
    let noise = sample_range(&mut local_seed, 0.0, scenario.max_noise);
    apply_capture_tone(
        &mut image,
        brightness,
        contrast,
        gamma,
        noise,
        &mut local_seed,
    );
    image
}

fn apply_capture_tone(
    image: &mut image::RgbaImage,
    brightness: f64,
    contrast: f64,
    gamma: f64,
    noise: f64,
    seed: &mut u64,
) {
    let width = image.width().max(1);
    let height = image.height().max(1);
    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel_mut(x, y);
            let nx = x as f64 / (width - 1).max(1) as f64 - 0.5;
            let ny = y as f64 / (height - 1).max(1) as f64 - 0.5;
            let vignette = ((nx * nx + ny * ny) * 1.8).clamp(0.0, 1.0) * 0.18;
            for channel in 0..3 {
                let mut value = f64::from(pixel[channel]) / 255.0;
                value = value.powf(gamma);
                value = (value - 0.5) * contrast + 0.5 + brightness - vignette;
                if noise > 0.0 {
                    value += (sample_unit(seed) - 0.5) * 2.0 * noise;
                }
                pixel[channel] = to_u8(value);
            }
            pixel[3] = 0xFF;
        }
    }
}

fn apply_motion_blur(
    image: &image::RgbaImage,
    motion_span: u8,
    seed: &mut u64,
) -> image::RgbaImage {
    let directions = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];
    let dir = directions[(sample_u32(seed) as usize) % directions.len()];
    let radius = i32::from(motion_span);
    let width = image.width();
    let height = image.height();
    let mut out = image::RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let mut acc = [0u32; 4];
            let mut count = 0u32;
            for offset in -radius..=radius {
                let sx = clamp_i32(x as i32 + dir.0 * offset, 0, width as i32 - 1) as u32;
                let sy = clamp_i32(y as i32 + dir.1 * offset, 0, height as i32 - 1) as u32;
                let p = image.get_pixel(sx, sy).0;
                for channel in 0..4 {
                    acc[channel] += u32::from(p[channel]);
                }
                count += 1;
            }
            let mut pixel = [0u8; 4];
            for channel in 0..4 {
                pixel[channel] = (acc[channel] / count.max(1)) as u8;
            }
            out.put_pixel(x, y, image::Rgba(pixel));
        }
    }
    out
}

fn translate_image(
    image: &image::RgbaImage,
    dx: i32,
    dy: i32,
    fill: image::Rgba<u8>,
) -> image::RgbaImage {
    let width = image.width();
    let height = image.height();
    let mut out = image::RgbaImage::from_pixel(width, height, fill);
    for y in 0..height {
        let src_y = y as i32 - dy;
        if src_y < 0 || src_y >= height as i32 {
            continue;
        }
        for x in 0..width {
            let src_x = x as i32 - dx;
            if src_x < 0 || src_x >= width as i32 {
                continue;
            }
            let pixel = image.get_pixel(src_x as u32, src_y as u32);
            out.put_pixel(x, y, *pixel);
        }
    }
    out
}

fn sample_range(seed: &mut u64, min: f64, max: f64) -> f64 {
    if (max - min).abs() <= f64::EPSILON {
        return min;
    }
    let t = sample_unit(seed);
    min + (max - min) * t
}

fn sample_i32_range(seed: &mut u64, min: i32, max: i32) -> i32 {
    if min >= max {
        return min;
    }
    let span = (max - min + 1) as f64;
    min + (sample_unit(seed) * span).floor() as i32
}

fn clamp_i32(value: i32, min: i32, max: i32) -> i32 {
    value.clamp(min, max)
}

fn write_png_rgba(path: &Path, image: &image::RgbaImage) -> Result<()> {
    let file = fs::File::create(path)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, image.width(), image.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(image.as_raw())?;
    Ok(())
}

fn write_apng_rgba(path: &Path, frames: &[image::RgbaImage], fps: u16) -> Result<()> {
    if frames.is_empty() {
        return Err(eyre!("no frames to encode"));
    }
    let file = fs::File::create(path)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, frames[0].width(), frames[0].height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_animated(frames.len() as u32, 0)?;
    encoder.set_sep_def_img(false)?;
    let mut writer = encoder.write_header()?;
    let delay = frame_delay_ms(fps);
    let delay_den = 1000u16;
    for frame in frames {
        writer.set_frame_delay(delay as u16, delay_den)?;
        writer.write_image_data(frame.as_raw())?;
    }
    Ok(())
}

fn write_gif_rgba(path: &Path, frames: &[image::RgbaImage], fps: u16) -> Result<()> {
    if frames.is_empty() {
        return Err(eyre!("no frames to encode"));
    }
    let file = fs::File::create(path)?;
    let mut encoder = image::codecs::gif::GifEncoder::new(file);
    let delay_ms = frame_delay_ms(fps) as u32;
    let delay = image::Delay::from_numer_denom_ms(delay_ms.max(1), 1);
    let mut gif_frames = Vec::with_capacity(frames.len());
    for frame in frames {
        gif_frames.push(image::Frame::from_parts(frame.clone(), 0, 0, delay));
    }
    encoder.encode_frames(gif_frames.into_iter())?;
    Ok(())
}

fn frame_delay_ms(fps: u16) -> u16 {
    let fps = fps.max(1) as u32;
    ((1000 + fps / 2) / fps).min(u16::MAX as u32) as u16
}

fn estimate_payload_bytes_per_second(payload_bytes: usize, frame_count: usize, fps: u16) -> u64 {
    if payload_bytes == 0 || frame_count == 0 {
        return 0;
    }
    let fps = fps.max(1) as f64;
    ((payload_bytes as f64 * fps) / frame_count as f64).round() as u64
}

fn cell_seed(x: u32, y: u32) -> u64 {
    (u64::from(x) << 32) ^ u64::from(y)
}

fn splitmix64(mut seed: u64) -> u64 {
    seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn sample_unit(seed: &mut u64) -> f64 {
    *seed = splitmix64(*seed);
    let value = *seed >> 11;
    value as f64 / ((1u64 << 53) as f64)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn to_u8(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn frame_kind_label(kind: QrStreamFrameKind) -> &'static str {
    match kind {
        QrStreamFrameKind::Header => "header",
        QrStreamFrameKind::Data => "data",
        QrStreamFrameKind::Parity => "parity",
    }
}

struct CellParam {
    bit: bool,
    jitter_x: f64,
    jitter_y: f64,
    angle: f64,
    sway: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn render_petal_frame_outputs_rgba() {
        let payload = b"petal-cli";
        let grid = PetalStreamEncoder::encode_grid(payload, PetalStreamOptions::default())
            .expect("encode");
        let image = render_petal_frame(
            &grid,
            64,
            0,
            1,
            PetalRenderStyle::SakuraWind,
            PetalStreamOptions::default(),
        );
        assert_eq!(image.width(), 64);
        assert_eq!(image.height(), 64);
        assert_eq!(image.as_raw().len(), 64 * 64 * 4);
    }

    #[test]
    fn render_sora_temple_roundtrip_decodes_payload() {
        let payload = b"petal-cli-sora-temple";
        let options = PetalStreamOptions::default();
        let grid = PetalStreamEncoder::encode_grid(payload, options).expect("encode");
        let image = render_petal_frame(&grid, 128, 3, 12, PetalRenderStyle::SoraTemple, options);
        let decoded = decode_frame_with_grid(
            &image,
            grid.grid_size,
            PetalStreamOptions {
                grid_size: grid.grid_size,
                ..options
            },
        )
        .expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn sora_temple_render_is_deterministic_per_frame() {
        let payload = b"petal-cli-sora-deterministic";
        let options = PetalStreamOptions::default();
        let grid = PetalStreamEncoder::encode_grid(payload, options).expect("encode");
        let image_a = render_petal_frame(&grid, 160, 5, 24, PetalRenderStyle::SoraTemple, options);
        let image_b = render_petal_frame(&grid, 160, 5, 24, PetalRenderStyle::SoraTemple, options);
        assert_eq!(image_a.as_raw(), image_b.as_raw());
    }

    #[test]
    fn katakana_channel_bit_inference_tracks_data_bits() {
        let payload = b"petal-cli-katakana-channel";
        let options = PetalStreamOptions::default();
        let grid = PetalStreamEncoder::encode_grid(payload, options).expect("encode");
        let image = render_petal_frame(&grid, 320, 2, 12, PetalRenderStyle::SoraTemple, options);
        let grid_size = grid.grid_size;
        let cell_size = (image.width() / u32::from(grid_size)).max(1);
        let grid_pixels = u32::from(grid_size) * cell_size;
        let offset_x = (image.width().saturating_sub(grid_pixels)) / 2;
        let offset_y = (image.height().saturating_sub(grid_pixels)) / 2;

        let mut compared = 0usize;
        let mut matched = 0usize;
        for y in 0..grid_size {
            for x in 0..grid_size {
                if render_cell_role(x, y, grid_size, options) != RenderCellRole::Data {
                    continue;
                }
                let idx = y as usize * grid_size as usize + x as usize;
                if let Some((bit, confidence)) =
                    infer_katakana_cell_bit(&image, x, y, cell_size, offset_x, offset_y)
                {
                    if confidence < SORA_KATAKANA_CONFIDENCE_MIN {
                        continue;
                    }
                    compared += 1;
                    if bit == grid.cells[idx] {
                        matched += 1;
                    }
                }
            }
        }
        assert!(compared > 128, "insufficient katakana samples: {compared}");
        let accuracy = matched as f64 / compared as f64;
        assert!(
            accuracy > 0.72,
            "katakana bit accuracy too low: {accuracy:.3}"
        );
    }

    #[test]
    fn write_png_rgba_emits_rgba() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("frame.png");
        let payload = b"petal-cli";
        let grid = PetalStreamEncoder::encode_grid(payload, PetalStreamOptions::default())
            .expect("encode");
        let image = render_petal_frame(
            &grid,
            32,
            0,
            1,
            PetalRenderStyle::SakuraWind,
            PetalStreamOptions::default(),
        );
        write_png_rgba(&path, &image).expect("write");
        let file = fs::File::open(&path).expect("open");
        let decoder = png::Decoder::new(std::io::BufReader::new(file));
        let reader = decoder.read_info().expect("read info");
        assert_eq!(reader.info().color_type, png::ColorType::Rgba);
    }

    #[test]
    fn load_png_roundtrips_frame_dimensions() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("frame.png");
        let payload = b"petal-cli-load";
        let grid = PetalStreamEncoder::encode_grid(payload, PetalStreamOptions::default())
            .expect("encode");
        let image = render_petal_frame(
            &grid,
            96,
            0,
            1,
            PetalRenderStyle::SakuraWind,
            PetalStreamOptions::default(),
        );
        write_png_rgba(&path, &image).expect("write");
        let decoded = load_png(&path).expect("load");
        assert_eq!(decoded.width(), image.width());
        assert_eq!(decoded.height(), image.height());
    }

    #[test]
    fn sample_grid_roundtrip_decodes_payload() {
        let payload = b"petal-cli-decode";
        let grid = PetalStreamEncoder::encode_grid(payload, PetalStreamOptions::default())
            .expect("encode");
        let image = render_petal_frame(
            &grid,
            128,
            0,
            1,
            PetalRenderStyle::SakuraWind,
            PetalStreamOptions::default(),
        );
        let samples = sample_grid_from_rgba(&image, grid.grid_size, PetalStreamOptions::default())
            .expect("samples");
        let decoded = PetalStreamDecoder::decode_samples(&samples, PetalStreamOptions::default())
            .expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn decode_frame_with_grid_roundtrip() {
        let payload = b"petal-cli-grid";
        let grid = PetalStreamEncoder::encode_grid(payload, PetalStreamOptions::default())
            .expect("encode");
        let image = render_petal_frame(
            &grid,
            128,
            0,
            1,
            PetalRenderStyle::SakuraWind,
            PetalStreamOptions::default(),
        );
        let decoded = decode_frame_with_grid(&image, grid.grid_size, PetalStreamOptions::default())
            .expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn decode_frame_auto_detects_grid_size() {
        let payload = b"petal-cli-auto";
        let grid = PetalStreamEncoder::encode_grid(payload, PetalStreamOptions::default())
            .expect("encode");
        let image = render_petal_frame(
            &grid,
            128,
            0,
            1,
            PetalRenderStyle::SakuraWind,
            PetalStreamOptions::default(),
        );
        let (grid_size, decoded) =
            decode_frame_auto(&image, PetalStreamOptions::default()).expect("auto");
        assert_eq!(grid_size, grid.grid_size);
        assert_eq!(decoded, payload);
    }

    #[test]
    fn capture_eval_reports_metrics_for_small_stream() {
        let payload = vec![0xA5; 220];
        let stream_options = QrStreamOptions {
            chunk_size: 72,
            parity_group: 0,
            ..QrStreamOptions::default()
        };
        let (_envelope, frames) =
            QrStreamEncoder::encode_frames(&payload, stream_options).expect("encode stream");
        let frame_bytes = frames
            .iter()
            .take(4)
            .map(QrStreamFrame::encode)
            .collect::<Vec<_>>();
        let max_len = frame_bytes.iter().map(Vec::len).max().unwrap_or_default();
        let resolved_grid_size =
            PetalStreamEncoder::encode_grid(&vec![0u8; max_len], PetalStreamOptions::default())
                .expect("grid resolve")
                .grid_size;
        let petal_options = PetalStreamOptions {
            grid_size: resolved_grid_size,
            ..PetalStreamOptions::default()
        };

        let mut images = Vec::new();
        for (index, bytes) in frame_bytes.iter().enumerate() {
            let grid = PetalStreamEncoder::encode_grid(bytes, petal_options).expect("grid");
            images.push(render_petal_frame(
                &grid,
                192,
                index as u32,
                frame_bytes.len() as u32,
                PetalRenderStyle::SoraTemple,
                petal_options,
            ));
        }

        let metrics = evaluate_capture_robustness(
            &images,
            &frame_bytes,
            resolved_grid_size,
            petal_options,
            PetalCaptureProfile::Default,
            1337,
            2,
        )
        .expect("evaluate");
        assert_eq!(metrics.frame_count, frame_bytes.len());
        assert!(metrics.attempts > 0);
        assert!(metrics.successes > 0);
        assert!(
            metrics.success_ratio() > 0.55,
            "success_ratio={:.3}",
            metrics.success_ratio()
        );
    }

    #[test]
    fn estimated_payload_rate_meets_three_kb_target() {
        let payload = vec![0x5A; 12_000];
        let options = QrStreamOptions {
            chunk_size: 360,
            parity_group: 0,
            ..QrStreamOptions::default()
        };
        let (_envelope, frames) =
            QrStreamEncoder::encode_frames(&payload, options).expect("encode");
        let bps = estimate_payload_bytes_per_second(payload.len(), frames.len(), 24);
        assert!(bps >= 3_000, "estimated throughput too low: {bps} B/s");
    }
}
