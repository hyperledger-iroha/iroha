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
    time::Instant,
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
    /// Simulate live camera reading and decode frame-by-frame in real time.
    SimulateRealtime(PetalSimulateRealtimeArgs),
    /// Score render styles with deterministic capture simulation and throughput metrics.
    ScoreStyles(PetalScoreStylesArgs),
}

impl Run for PetalCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            PetalCommand::Encode(args) => args.run(context),
            PetalCommand::Decode(args) => args.run(context),
            PetalCommand::EvalCapture(args) => args.run(context),
            PetalCommand::SimulateRealtime(args) => args.run(context),
            PetalCommand::ScoreStyles(args) => args.run(context),
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
    /// Data channel used for data cells in rendered outputs.
    #[arg(long, value_enum, default_value = "binary")]
    pub channel: PetalDataChannel,
    /// Katakana channel tuning preset used when grid/chunk are left at defaults.
    #[arg(long, value_enum, default_value = "balanced")]
    pub katakana_preset: PetalKatakanaPreset,
}

impl PetalEncodeArgs {
    fn run<C: RunContext>(self, _context: &mut C) -> Result<()> {
        let payload = fs::read(&self.input)
            .map_err(|err| eyre!("failed to read payload {}: {err}", self.input.display()))?;
        let use_katakana_preset_defaults =
            uses_katakana_preset_defaults(self.channel, self.chunk_size, self.grid_size);
        let resolved_chunk_size = resolve_encode_chunk_size(
            self.channel,
            self.chunk_size,
            self.grid_size,
            self.katakana_preset,
            use_katakana_preset_defaults,
        );
        let options = QrStreamOptions {
            chunk_size: resolved_chunk_size,
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
        let max_len = encoded_frames.iter().map(Vec::len).max().unwrap_or(0);
        let resolved_grid_size = if self.grid_size == 0 {
            if self.channel == PetalDataChannel::KatakanaBase94 {
                let mode = if use_katakana_preset_defaults {
                    katakana_grid_sizing_mode(self.katakana_preset)
                } else {
                    KatakanaGridSizingMode::CapacityFirst
                };
                resolve_katakana_base94_grid_size(max_len, base_petal_options, mode)?
            } else {
                let dummy = vec![0u8; max_len];
                let grid = PetalStreamEncoder::encode_grid(&dummy, base_petal_options)?;
                grid.grid_size
            }
        } else {
            self.grid_size
        };
        let petal_options = PetalStreamOptions {
            grid_size: resolved_grid_size,
            ..base_petal_options
        };
        let katakana_template_grid = if self.channel == PetalDataChannel::KatakanaBase94 {
            Some(PetalStreamEncoder::encode_grid(&[], petal_options)?)
        } else {
            None
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
                let grid = if let Some(template) = katakana_template_grid.as_ref() {
                    template.clone()
                } else {
                    PetalStreamEncoder::encode_grid(bytes, petal_options)?
                };
                rendered.push(RenderedFrame {
                    index: idx as u32,
                    grid,
                    frame_bytes: bytes.clone(),
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
                    let image = render_petal_frame_with_channel(
                        &frame.grid,
                        self.dimension,
                        frame.index,
                        total,
                        self.style,
                        petal_options,
                        self.channel,
                        Some(&frame.frame_bytes),
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
                        render_petal_frame_with_channel(
                            &frame.grid,
                            self.dimension,
                            frame.index,
                            total,
                            self.style,
                            petal_options,
                            self.channel,
                            Some(&frame.frame_bytes),
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
                        render_petal_frame_with_channel(
                            &frame.grid,
                            self.dimension,
                            frame.index,
                            total,
                            self.style,
                            petal_options,
                            self.channel,
                            Some(&frame.frame_bytes),
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
            Value::from(resolved_chunk_size as u64),
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
        manifest_map.insert("channel".to_string(), Value::from(self.channel.label()));
        if self.channel == PetalDataChannel::KatakanaBase94 {
            manifest_map.insert(
                "katakana_preset".to_string(),
                Value::from(self.katakana_preset.label()),
            );
        }
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
    /// Data channel used by rendered frames.
    #[arg(long, value_enum, default_value = "binary")]
    pub channel: PetalDataChannel,
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
            let frame_bytes = if self.channel == PetalDataChannel::Binary {
                if resolved_grid_size == 0 {
                    let (grid_size, bytes) = decode_frame_auto(&image, petal_options)?;
                    resolved_grid_size = grid_size;
                    bytes
                } else {
                    decode_frame_with_grid(&image, resolved_grid_size, petal_options)?
                }
            } else if resolved_grid_size == 0 {
                let (grid_size, bytes) = decode_katakana_base94_frame_auto(&image, petal_options)?;
                resolved_grid_size = grid_size;
                bytes
            } else {
                decode_katakana_base94_frame_with_grid(&image, resolved_grid_size, petal_options)?
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
            manifest_map.insert("channel".to_string(), Value::from(self.channel.label()));
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
    /// Data channel used by rendered frames.
    #[arg(long, value_enum, default_value = "binary")]
    pub channel: PetalDataChannel,
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
            let bytes = if self.channel == PetalDataChannel::Binary {
                if resolved_grid_size == 0 {
                    let (grid_size, bytes) = decode_frame_auto(&image, base_options)?;
                    resolved_grid_size = grid_size;
                    bytes
                } else {
                    decode_frame_with_grid(&image, resolved_grid_size, base_options)?
                }
            } else if resolved_grid_size == 0 {
                let (grid_size, bytes) = decode_katakana_base94_frame_auto(&image, base_options)?;
                resolved_grid_size = grid_size;
                bytes
            } else {
                decode_katakana_base94_frame_with_grid(&image, resolved_grid_size, base_options)?
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
            self.channel,
            self.profile,
            self.seed,
            trials_per_frame,
        )?;

        let report = metrics.to_json(
            self.channel,
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

#[derive(clap::Args, Debug)]
pub struct PetalSimulateRealtimeArgs {
    /// Directory containing rendered PNG frames.
    #[arg(long, value_name = "DIR")]
    pub input_dir: PathBuf,
    /// Optional output file for the decoded payload.
    #[arg(long, value_name = "FILE")]
    pub output_payload: Option<PathBuf>,
    /// Optional JSON report output path.
    #[arg(long, value_name = "FILE")]
    pub output_report: Option<PathBuf>,
    /// Grid size in cells (0 to auto-detect from the first frame).
    #[arg(long, default_value_t = 0)]
    pub grid_size: u16,
    /// Border thickness in cells.
    #[arg(long, default_value_t = 1)]
    pub border: u8,
    /// Anchor size in cells.
    #[arg(long, default_value_t = 3)]
    pub anchor_size: u8,
    /// Data channel used by rendered frames.
    #[arg(long, value_enum, default_value = "binary")]
    pub channel: PetalDataChannel,
    /// Capture perturbation profile used to emulate a moving camera read.
    #[arg(long, value_enum, default_value = "default")]
    pub profile: PetalCaptureProfile,
    /// Deterministic seed for capture perturbation sampling.
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
    /// Simulated camera frame rate used to compute timeline metrics.
    #[arg(long, default_value_t = 24)]
    pub simulate_fps: u16,
    /// Optional cap on number of frames to process from the input directory.
    #[arg(long)]
    pub frame_limit: Option<usize>,
    /// Disable capture perturbation and decode pristine frames only.
    #[arg(long)]
    pub disable_capture_perturbation: bool,
    /// Allow incomplete stream reconstruction without returning an error.
    #[arg(long)]
    pub allow_incomplete: bool,
}

impl PetalSimulateRealtimeArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        if self.simulate_fps == 0 {
            return Err(eyre!("simulate-fps must be > 0"));
        }
        if self.frame_limit == Some(0) {
            return Err(eyre!("frame-limit must be > 0 when provided"));
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
        if let Some(limit) = self.frame_limit {
            entries.truncate(limit);
        }
        if entries.is_empty() {
            return Err(eyre!("no png frames found in {}", self.input_dir.display()));
        }

        let mut images = Vec::with_capacity(entries.len());
        for entry in &entries {
            images.push(load_png(entry.path())?);
        }

        let mut resolved_grid_size = self.grid_size;
        let base_options = PetalStreamOptions {
            grid_size: if self.grid_size == 0 {
                0
            } else {
                self.grid_size
            },
            border: self.border,
            anchor_size: self.anchor_size,
        };
        if resolved_grid_size == 0 {
            let first = images
                .first()
                .ok_or_else(|| eyre!("no frames loaded for realtime simulation"))?;
            resolved_grid_size = if self.channel == PetalDataChannel::Binary {
                decode_frame_auto(first, base_options)?.0
            } else {
                decode_katakana_base94_frame_auto(first, base_options)?.0
            };
        }
        if resolved_grid_size == 0 {
            return Err(eyre!("failed to resolve grid size for realtime simulation"));
        }

        let options = PetalStreamOptions {
            grid_size: resolved_grid_size,
            border: self.border,
            anchor_size: self.anchor_size,
        };
        let metrics = simulate_realtime_decode(
            &images,
            options,
            self.channel,
            self.profile,
            self.seed,
            self.simulate_fps,
            !self.disable_capture_perturbation,
        )?;
        if let Some(path) = self.output_payload {
            let payload = metrics.payload.as_ref().ok_or_else(|| {
                eyre!("realtime simulation did not reconstruct a complete payload")
            })?;
            fs::write(&path, payload.as_slice())
                .map_err(|err| eyre!("failed to write {}: {err}", path.display()))?;
        }

        let report = metrics.to_json(
            self.channel,
            self.profile,
            resolved_grid_size,
            self.seed,
            self.simulate_fps,
            !self.disable_capture_perturbation,
            self.allow_incomplete,
        );
        if let Some(path) = self.output_report {
            let rendered = norito::json::to_string_pretty(&report)?;
            fs::write(&path, format!("{rendered}\n"))
                .map_err(|err| eyre!("failed to write report {}: {err}", path.display()))?;
        }
        context.print_data(&report)?;

        if !self.allow_incomplete && !metrics.stream_complete {
            return Err(eyre!(
                "realtime simulation did not complete stream reconstruction ({} / {} chunks)",
                metrics.stream_received_chunks,
                metrics.stream_total_chunks
            ));
        }

        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct PetalScoreStylesArgs {
    /// Path to payload bytes used for style scoring.
    #[arg(long, value_name = "FILE")]
    pub input: PathBuf,
    /// JSON report path for scored styles.
    #[arg(long, value_name = "FILE")]
    pub output_report: PathBuf,
    /// Styles to evaluate (repeat flag). Empty means the default temple style set.
    #[arg(long, value_enum)]
    pub style: Vec<PetalRenderStyle>,
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
    /// Frames per second used for effective throughput scoring.
    #[arg(long, default_value_t = 24)]
    pub fps: u16,
    /// Capture perturbation profile.
    #[arg(long, value_enum, default_value = "default")]
    pub profile: PetalCaptureProfile,
    /// Deterministic seed for perturbation sampling.
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
    /// Number of perturbation trials per frame (0 uses profile default).
    #[arg(long, default_value_t = 0)]
    pub trials_per_frame: u16,
    /// Minimum capture success ratio used for the pass gate in the report.
    #[arg(long, default_value_t = 0.95)]
    pub min_success_ratio: f64,
    /// Target effective throughput used to normalize throughput scoring.
    #[arg(long, default_value_t = 3_000)]
    pub target_effective_bps: u64,
}

impl PetalScoreStylesArgs {
    fn run<C: RunContext>(self, _context: &mut C) -> Result<()> {
        if !(0.0..=1.0).contains(&self.min_success_ratio) {
            return Err(eyre!("min-success-ratio must be between 0.0 and 1.0"));
        }
        if self.target_effective_bps == 0 {
            return Err(eyre!("target-effective-bps must be > 0"));
        }

        let payload = fs::read(&self.input)
            .map_err(|err| eyre!("failed to read payload {}: {err}", self.input.display()))?;
        let stream_options = QrStreamOptions {
            chunk_size: self.chunk_size,
            parity_group: self.parity_group,
            payload_kind: self.payload_kind.into(),
            ..QrStreamOptions::default()
        };
        let (_envelope, frames) = QrStreamEncoder::encode_frames(&payload, stream_options)?;
        let encoded_frames: Vec<Vec<u8>> = frames.iter().map(QrStreamFrame::encode).collect();
        if encoded_frames.is_empty() {
            return Err(eyre!("no frames generated for scoring"));
        }

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

        let trials_per_frame = if self.trials_per_frame == 0 {
            self.profile.default_trials()
        } else {
            self.trials_per_frame
        };
        if trials_per_frame == 0 {
            return Err(eyre!("trials-per-frame must be > 0"));
        }

        let styles = resolve_score_styles(&self.style);
        let estimated_payload_bps =
            estimate_payload_bytes_per_second(payload.len(), encoded_frames.len(), self.fps);
        let frame_count = encoded_frames.len().max(1) as u32;
        let mut rows = Vec::with_capacity(styles.len());
        for style in styles {
            let mut images = Vec::with_capacity(encoded_frames.len());
            for (index, frame_bytes) in encoded_frames.iter().enumerate() {
                let grid = PetalStreamEncoder::encode_grid(frame_bytes, petal_options)?;
                images.push(render_petal_frame(
                    &grid,
                    self.dimension,
                    index as u32,
                    frame_count,
                    style,
                    petal_options,
                ));
            }
            let metrics = evaluate_capture_robustness(
                &images,
                &encoded_frames,
                resolved_grid_size,
                petal_options,
                PetalDataChannel::Binary,
                self.profile,
                self.seed,
                trials_per_frame,
            )?;
            let aesthetic_score = images
                .first()
                .map(|image| estimate_style_aesthetic_score(image, style))
                .unwrap_or(0.0);
            let decode_completion_score = if metrics.frame_count == 0 {
                0.0
            } else if metrics.stream_complete {
                1.0
            } else {
                metrics.recovered_frames as f64 / metrics.frame_count as f64
            };
            let effective_payload_bps = estimated_payload_bps as f64 * metrics.success_ratio();
            let effective_bps_score =
                (effective_payload_bps / self.target_effective_bps as f64).clamp(0.0, 1.0);
            let overall_score = style_overall_score(
                aesthetic_score,
                decode_completion_score,
                effective_bps_score,
            );
            let passes_gate = metrics.stream_complete
                && metrics.success_ratio() + f64::EPSILON >= self.min_success_ratio;
            rows.push(PetalStyleScoreRow {
                style,
                aesthetic_score,
                decode_completion_score,
                capture_success_ratio: metrics.success_ratio(),
                frame_success_ratio: metrics.frame_success_ratio(),
                effective_payload_bps,
                effective_bps_score,
                overall_score,
                passes_gate,
                metrics,
            });
        }

        rows.sort_by(style_score_row_ordering);
        let recommended_style = rows
            .iter()
            .find(|row| row.passes_gate)
            .or_else(|| rows.first())
            .map(|row| row.style)
            .ok_or_else(|| eyre!("style score report is empty"))?;

        let report = build_style_score_report(
            &rows,
            PetalStyleScoreReportContext {
                payload_kind: self.payload_kind.label(),
                payload_length: payload.len(),
                frame_count: encoded_frames.len(),
                chunk_size: self.chunk_size,
                parity_group: self.parity_group,
                grid_size: resolved_grid_size,
                border: self.border,
                anchor_size: self.anchor_size,
                dimension: self.dimension,
                fps: self.fps,
                profile: self.profile,
                seed: self.seed,
                trials_per_frame,
                min_success_ratio: self.min_success_ratio,
                estimated_payload_bps,
                target_effective_bps: self.target_effective_bps,
                recommended_style,
            },
        );
        let rendered = norito::json::to_string_pretty(&report)?;
        fs::write(&self.output_report, format!("{rendered}\n")).map_err(|err| {
            eyre!(
                "failed to write style score report {}: {err}",
                self.output_report.display()
            )
        })?;

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
pub enum PetalDataChannel {
    Binary,
    KatakanaBase94,
}

impl PetalDataChannel {
    fn label(self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::KatakanaBase94 => "katakana-base94",
        }
    }
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetalKatakanaPreset {
    Balanced,
    DistanceSafe,
}

impl PetalKatakanaPreset {
    fn label(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::DistanceSafe => "distance-safe",
        }
    }
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

const DEFAULT_STYLE_SCORE_STYLES: [PetalRenderStyle; 7] = [
    PetalRenderStyle::SoraTemple,
    PetalRenderStyle::SoraTempleGhost,
    PetalRenderStyle::SoraTempleAegis,
    PetalRenderStyle::SoraTempleRadiant,
    PetalRenderStyle::SoraTempleBold,
    PetalRenderStyle::SoraTempleCommand,
    PetalRenderStyle::SoraTempleMinimal,
];

fn resolve_score_styles(requested: &[PetalRenderStyle]) -> Vec<PetalRenderStyle> {
    let source: &[PetalRenderStyle] = if requested.is_empty() {
        &DEFAULT_STYLE_SCORE_STYLES
    } else {
        requested
    };
    let mut styles = Vec::with_capacity(source.len());
    for &style in source {
        if !styles.contains(&style) {
            styles.push(style);
        }
    }
    styles
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
    frame_bytes: Vec<u8>,
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

fn count_data_cells(grid_size: u16, options: PetalStreamOptions) -> usize {
    let mut count = 0usize;
    for y in 0..grid_size {
        for x in 0..grid_size {
            if render_cell_role(x, y, grid_size, options) == RenderCellRole::Data {
                count += 1;
            }
        }
    }
    count
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
    render_petal_frame_with_channel(
        grid,
        dimension,
        frame_index,
        frame_count,
        style,
        options,
        PetalDataChannel::Binary,
        None,
    )
}

fn render_petal_frame_with_channel(
    grid: &PetalStreamGrid,
    dimension: u32,
    frame_index: u32,
    frame_count: u32,
    style: PetalRenderStyle,
    options: PetalStreamOptions,
    channel: PetalDataChannel,
    frame_bytes: Option<&[u8]>,
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
        return render_sora_temple_frame(
            grid,
            dimension,
            frame_index,
            frame_count,
            style,
            options,
            channel,
            frame_bytes,
        );
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
    channel: PetalDataChannel,
    frame_bytes: Option<&[u8]>,
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
    let data_cell_count = count_data_cells(grid.grid_size, options);
    let katakana_digits = if channel == PetalDataChannel::KatakanaBase94 {
        frame_bytes.and_then(|bytes| katakana_base94_encode_digits(bytes, data_cell_count).ok())
    } else {
        None
    };
    let mut katakana_cursor = 0usize;

    let stream_bits = collect_stream_data_bits(grid, options);
    let stream_signature = stream_bit_signature(&stream_bits);
    let mut cell_params = Vec::with_capacity((grid_size * grid_size) as usize);
    let mut cell_roles = Vec::with_capacity((grid_size * grid_size) as usize);
    for y in 0..grid_size {
        for x in 0..grid_size {
            let idx = y as usize * grid_size as usize + x as usize;
            let role = render_cell_role(x as u16, y as u16, grid.grid_size, options);
            let mut bit = grid.cells[idx];
            let nx = (x as f64 + 0.5) / grid_size as f64;
            let ny = (y as f64 + 0.5) / grid_size as f64;
            let logo = role == RenderCellRole::Data && sora_ten_logo_disk_mask(nx, ny);
            let logo_glyph = role == RenderCellRole::Data && sora_ten_logo_glyph_mask(nx, ny);
            let mut seed = cell_seed(x, y)
                ^ stream_signature
                ^ (u64::from(frame_index) << 11)
                ^ (u64::from(bit as u8) << 3);
            let (katakana_dense, kana_pattern, kana_state) = if role == RenderCellRole::Data {
                if let Some(digits) = katakana_digits.as_ref() {
                    let state = digits.get(katakana_cursor).copied().unwrap_or_default();
                    katakana_cursor += 1;
                    bit = state as usize >= IROHA_KATAKANA.len();
                    let kana_symbol = state as usize % IROHA_KATAKANA.len();
                    (true, katakana_pattern_id(kana_symbol), state)
                } else {
                    let kana_index = select_kana_index_for_bit(bit, &mut seed);
                    (false, katakana_pattern_id(kana_index), kana_index as u8)
                }
            } else {
                let kana_index = select_kana_index_for_bit(bit, &mut seed);
                (false, katakana_pattern_id(kana_index), kana_index as u8)
            };
            cell_params.push(TempleCellParam {
                bit,
                logo,
                logo_glyph,
                katakana_dense,
                kana_pattern,
                kana_state,
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
            if cell_size >= 8 {
                let logo_mask_here = sora_ten_logo_disk_mask(nx, ny);
                if logo_mask_here {
                    blend_rgb(&mut rgb, style_cfg.ring_bright, 0.070);
                } else {
                    let halo_nx = (nx - 0.5) * 0.90 + 0.5;
                    let halo_ny = (ny - 0.5) * 0.90 + 0.5;
                    if sora_ten_logo_disk_mask(halo_nx, halo_ny) {
                        blend_rgb(&mut rgb, style_cfg.logo_core_bg, 0.042);
                    }
                }
            }
            // Add a shallow horizon haze to create depth behind the data grid.
            let horizon = ((ny - 0.22) * 2.8).clamp(0.0, 1.0);
            let horizon_falloff = (1.0 - radial / 0.78).clamp(0.0, 1.0).powf(1.7);
            blend_rgb(
                &mut rgb,
                style_cfg.ring_bright,
                horizon * horizon_falloff * 0.055,
            );
            let scanline = (ny * 210.0 + phase * std::f64::consts::TAU).sin() * 0.5 + 0.5;
            blend_rgb(
                &mut rgb,
                style_cfg.ring_dim,
                scanline * style_cfg.scanline_alpha,
            );
            // Add a soft aurora sweep so the temple background feels less flat.
            let aurora_wave =
                ((nx * 3.9 + ny * 2.4 + phase * std::f64::consts::TAU).sin() * 0.5 + 0.5).powf(2.1);
            blend_rgb(&mut rgb, style_cfg.ring_bright, aurora_wave * 0.035);
            let diagonal_glow = ((nx * 2.1 - ny * 1.6 + phase * 1.4).sin() * 0.5 + 0.5).powf(3.0);
            blend_rgb(&mut rgb, style_cfg.ring_dim, diagonal_glow * 0.025);
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
                let grid_u = (px - offset_x) as f64 / width.max(1) as f64;
                let grid_v = (py - offset_y) as f64 / height.max(1) as f64;
                let panel_edge = grid_u.min(1.0 - grid_u).min(grid_v.min(1.0 - grid_v));
                // Give the full payload area a subtle instrument-panel plate.
                blend_rgb(&mut rgb, style_cfg.ring_dim, 0.052);
                let panel_focus = ((grid_u - 0.5).powi(2) + (grid_v - 0.5).powi(2)).sqrt();
                let panel_focus = (1.0 - panel_focus / 0.73).clamp(0.0, 1.0).powf(2.0);
                blend_rgb(&mut rgb, style_cfg.ring_bright, panel_focus * 0.022);
                if panel_edge < 0.020 {
                    blend_rgb(&mut rgb, style_cfg.ring_bright, 0.075);
                }
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
    logo_glyph: bool,
    katakana_dense: bool,
    kana_pattern: usize,
    kana_state: u8,
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
            bg_start: [0.012, 0.005, 0.035],
            bg_end: [0.074, 0.027, 0.111],
            ring_bright: [1.0, 0.87, 0.95],
            ring_dim: [0.24, 0.14, 0.28],
            tile_light_bg: [0.965, 0.935, 0.978],
            tile_light_fg: [0.08, 0.032, 0.12],
            tile_dark_bg: [0.058, 0.024, 0.089],
            tile_dark_fg: [0.99, 0.93, 0.99],
            logo_tint: [1.0, 0.94, 0.985],
            scanline_alpha: 0.010,
            vignette: 0.36,
            ring_band: 0.0067,
            ring_on_alpha: 0.78,
            ring_off_alpha: 0.18,
            tile_margin_outer: SORA_TILE_MARGIN_OUTER,
            tile_margin_logo: SORA_TILE_MARGIN_LOGO,
            tile_glyph_alpha: 0.30,
            tile_alpha_regular: 0.985,
            tile_alpha_logo: 0.998,
            logo_tint_alpha: 0.30,
            logo_light_bg: [0.96, 0.87, 0.90],
            logo_dark_bg: [0.19, 0.03, 0.06],
            logo_tile_mix: 0.75,
            border_dark_mix: 0.24,
            border_light_mix: 0.16,
            logo_core_bg: [0.84, 0.12, 0.24],
            logo_core_alpha: 0.66,
            logo_core_radius: 0.50,
            logo_glyph_boost: 0.24,
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
        margin = 0.0;
    }
    if cell_size >= 10 {
        margin = 0.0;
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
    let inner_u = ((local_x - margin) / (1.0 - 2.0 * margin)).clamp(0.0, 0.9999);
    let inner_v = ((local_y - margin) / (1.0 - 2.0 * margin)).clamp(0.0, 0.9999);
    if param.katakana_dense {
        blend_rgb(rgb, tile_bg, 1.0);
        if param.logo {
            let logo_bg = if param.bit {
                style_cfg.logo_dark_bg
            } else {
                style_cfg.logo_light_bg
            };
            blend_rgb(rgb, logo_bg, style_cfg.logo_tile_mix * 0.55);
            blend_rgb(rgb, [0.91, 0.11, 0.18], 0.28);
        }
        if param.logo_glyph {
            blend_rgb(rgb, [0.02, 0.02, 0.02], if param.bit { 0.34 } else { 0.16 });
        }
        let bitmap = katakana_symbol_bitmap(param.kana_pattern, gx, gy);
        if katakana_bitmap_hit(bitmap, inner_u, inner_v) {
            blend_rgb(rgb, glyph_fg, 1.0);
        }
        if let Some(bit_index) = katakana_signature_marker_index(inner_u, inner_v) {
            let marker_on = param.kana_state & (1u8 << bit_index) != 0;
            let marker_color = if marker_on {
                [0.82, 0.82, 0.82]
            } else {
                [0.18, 0.18, 0.18]
            };
            blend_rgb(rgb, marker_color, 1.0);
        }
        return;
    }
    if param.logo {
        if cell_size >= 8 {
            let logo_bg = if param.bit {
                style_cfg.logo_dark_bg
            } else {
                style_cfg.logo_light_bg
            };
            blend_rgb(rgb, logo_bg, style_cfg.logo_tile_mix);
            blend_rgb(rgb, style_cfg.logo_tint, style_cfg.logo_tint_alpha);
            blend_rgb(rgb, [0.91, 0.11, 0.18], 0.42);
            let logo_radial =
                ((inner_u - 0.5) * (inner_u - 0.5) + (inner_v - 0.5) * (inner_v - 0.5)).sqrt();
            let logo_glow = (1.0 - logo_radial / 0.70).clamp(0.0, 1.0).powf(2.2);
            blend_rgb(rgb, style_cfg.ring_bright, logo_glow * 0.16);
        } else {
            // Keep low-resolution frames decode-friendly while hinting the disk tint.
            blend_rgb(rgb, [0.91, 0.11, 0.18], 0.12);
        }
    }
    if param.logo_glyph {
        if cell_size >= 8 {
            // Bias toward black ink while preserving a decode-relevant light/dark split.
            let ink_alpha = if param.bit { 0.84 } else { 0.30 };
            blend_rgb(rgb, [0.01, 0.01, 0.01], ink_alpha);
            blend_rgb(rgb, style_cfg.ring_dim, if param.bit { 0.18 } else { 0.04 });
        } else {
            // At tiny cells, keep contrast for reliable decode.
            let ink_alpha = if param.bit { 0.22 } else { 0.07 };
            blend_rgb(rgb, [0.01, 0.01, 0.01], ink_alpha);
        }
    }
    if cell_size >= 8 && !param.katakana_dense {
        let holder_scale = ((cell_size as f64 - 8.0) / 40.0).clamp(0.0, 1.0);
        let frame_dist = inner_u.min(1.0 - inner_u).min(inner_v.min(1.0 - inner_v));
        let frame_thickness = if param.logo {
            lerp(0.046, 0.054, holder_scale)
        } else {
            lerp(0.058, 0.070, holder_scale)
        };
        if frame_dist < frame_thickness {
            let frame_alpha = if param.logo {
                lerp(0.14, 0.17, holder_scale)
            } else {
                lerp(0.15, 0.18, holder_scale)
            };
            blend_rgb(rgb, frame_color, frame_alpha);
        }
        let inset_band = frame_thickness + lerp(0.030, 0.042, holder_scale);
        if frame_dist >= frame_thickness && frame_dist < inset_band {
            blend_rgb(rgb, frame_color, if param.logo { 0.045 } else { 0.055 });
        }
        let bevel_band = frame_thickness + lerp(0.012, 0.020, holder_scale);
        if inner_u < bevel_band || inner_v < bevel_band {
            blend_rgb(
                rgb,
                style_cfg.tile_light_fg,
                if param.logo { 0.050 } else { 0.042 },
            );
        }
        if inner_u > 1.0 - bevel_band || inner_v > 1.0 - bevel_band {
            blend_rgb(
                rgb,
                style_cfg.ring_dim,
                if param.logo { 0.055 } else { 0.046 },
            );
        }
        let bracket_len = if param.logo {
            lerp(0.50, 0.54, holder_scale)
        } else {
            lerp(0.56, 0.60, holder_scale)
        };
        let bracket_w = if param.logo {
            lerp(0.092, 0.100, holder_scale)
        } else {
            lerp(0.110, 0.120, holder_scale)
        };
        let corner_bracket = (inner_u < bracket_len && inner_v < bracket_w)
            || (inner_u < bracket_w && inner_v < bracket_len)
            || (inner_u > 1.0 - bracket_len && inner_v < bracket_w)
            || (inner_u > 1.0 - bracket_w && inner_v < bracket_len)
            || (inner_u < bracket_len && inner_v > 1.0 - bracket_w)
            || (inner_u < bracket_w && inner_v > 1.0 - bracket_len)
            || (inner_u > 1.0 - bracket_len && inner_v > 1.0 - bracket_w)
            || (inner_u > 1.0 - bracket_w && inner_v > 1.0 - bracket_len);
        if corner_bracket {
            let corner_alpha = if param.logo {
                lerp(0.23, 0.26, holder_scale)
            } else {
                lerp(0.26, 0.29, holder_scale)
            };
            blend_rgb(rgb, frame_color, corner_alpha);
        }
        let notch_w = bracket_w * 0.62;
        let corner_notch = (inner_u < notch_w && inner_v < notch_w)
            || (inner_u > 1.0 - notch_w && inner_v < notch_w)
            || (inner_u < notch_w && inner_v > 1.0 - notch_w)
            || (inner_u > 1.0 - notch_w && inner_v > 1.0 - notch_w);
        if corner_notch {
            blend_rgb(
                rgb,
                style_cfg.ring_dim,
                if param.logo { 0.11 } else { 0.10 },
            );
        }
    }

    let bitmap = if param.katakana_dense {
        katakana_symbol_bitmap(param.kana_pattern, gx, gy)
    } else {
        katakana_pattern_bitmap(param.kana_pattern, gx, gy)
    };
    if katakana_bitmap_hit(bitmap, inner_u, inner_v) {
        let glyph_alpha = if param.katakana_dense {
            if param.bit { 0.96 } else { 0.88 }
        } else if param.logo {
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

fn katakana_pattern_bitmap(pattern_id: usize, x: usize, y: usize) -> [u8; 8] {
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

const KATAKANA_SYMBOL_SIGNATURE_MARKERS: [(i32, i32); 7] =
    [(0, 0), (6, 0), (0, 6), (6, 6), (3, 0), (0, 3), (6, 3)];

fn katakana_symbol_bitmap(pattern_id: usize, x: usize, y: usize) -> [u8; 8] {
    katakana_pattern_bitmap(pattern_id, x, y)
}

fn katakana_signature_marker_index(u: f64, v: f64) -> Option<usize> {
    let gx = (u * 8.0).floor() as i32;
    let gy = (v * 8.0).floor() as i32;
    for (index, &(mx, my)) in KATAKANA_SYMBOL_SIGNATURE_MARKERS.iter().enumerate() {
        if gx >= mx && gx <= mx + 1 && gy >= my && gy <= my + 1 {
            return Some(index);
        }
    }
    None
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
    // A soft halo ties ring layers together before drawing per-dot signals.
    for ring_radius in SORA_RING_RADII {
        let halo_dist = (radius - ring_radius).abs();
        let halo_span = style_cfg.ring_band * 6.0;
        if halo_dist <= halo_span {
            let halo = (1.0 - halo_dist / halo_span).powi(2);
            blend_rgb(rgb, style_cfg.ring_dim, halo * 0.032);
        }
    }
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
        let ring_t = ring_index as f64 / (SORA_RING_RADII.len().saturating_sub(1).max(1)) as f64;
        let profile_power = lerp(2.2, 2.8, ring_t);
        let dot_profile = (1.0 - angular_dist * 2.0).max(0.0).powf(profile_power);
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
    sora_ten_logo_disk_mask(nx, ny)
}

fn sora_ten_logo_disk_mask(nx: f64, ny: f64) -> bool {
    let x = nx * 2.0 - 1.0;
    let y = ny * 2.0 - 1.0;
    x * x + y * y <= 0.968 * 0.968
}

fn sora_ten_logo_glyph_mask(nx: f64, ny: f64) -> bool {
    if !sora_ten_logo_disk_mask(nx, ny) {
        return false;
    }
    let x = nx * 2.0 - 1.0;
    let y = ny * 2.0 - 1.0;

    let top_bar = y >= -0.56 && y <= -0.38 && x >= -0.62 && x <= 0.62;
    let mid_band = y >= -0.20 && y <= -0.02 && x >= -0.62 && x <= 0.62;
    let mid_gap = y >= -0.20 && y <= -0.02 && x.abs() < 0.10;
    let stem_upper = y >= -0.38 && y <= -0.22 && x.abs() <= 0.09;
    let stem_lower = y >= -0.01 && y <= 0.07 && x.abs() <= 0.09;

    let leg_y = y.clamp(0.0, 0.92);
    let leg_half_width = lerp(0.13, 0.10, (leg_y / 0.92).clamp(0.0, 1.0));
    let left_axis = -0.05 - 0.74 * leg_y;
    let right_axis = 0.05 + 0.74 * leg_y;
    let left_leg = y >= 0.02 && y <= 0.92 && (x - left_axis).abs() <= leg_half_width;
    let right_leg = y >= 0.02 && y <= 0.92 && (x - right_axis).abs() <= leg_half_width;

    top_bar || (mid_band && !mid_gap) || stem_upper || stem_lower || left_leg || right_leg
}

fn estimate_style_aesthetic_score(image: &image::RgbaImage, style: PetalRenderStyle) -> f64 {
    if image.width() == 0 || image.height() == 0 {
        return 0.0;
    }
    let style_cfg = temple_style_config(style);
    let ring_band = (style_cfg.ring_band * 2.4).max(0.004);
    let mut logo_sum = 0.0;
    let mut logo_sq_sum = 0.0;
    let mut logo_count = 0u64;
    let mut bg_sum = 0.0;
    let mut bg_count = 0u64;
    let mut ring_sum = 0.0;
    let mut ring_count = 0u64;
    let width = image.width();
    let height = image.height();
    for py in 0..height {
        let ny = if height <= 1 {
            0.5
        } else {
            py as f64 / (height - 1) as f64
        };
        for px in 0..width {
            let nx = if width <= 1 {
                0.5
            } else {
                px as f64 / (width - 1) as f64
            };
            let pixel = image.get_pixel(px, py).0;
            let luma = pixel_luma_norm(pixel);
            if sora_ten_logo_mask(nx, ny) {
                logo_sum += luma;
                logo_sq_sum += luma * luma;
                logo_count += 1;
            } else {
                bg_sum += luma;
                bg_count += 1;
            }
            let cx = nx - 0.5;
            let cy = ny - 0.5;
            let radius = (cx * cx + cy * cy).sqrt();
            if SORA_RING_RADII
                .iter()
                .any(|ring_radius| (radius - ring_radius).abs() <= ring_band)
            {
                ring_sum += luma;
                ring_count += 1;
            }
        }
    }
    if logo_count == 0 || bg_count == 0 {
        return 0.0;
    }
    let logo_mean = logo_sum / logo_count as f64;
    let bg_mean = bg_sum / bg_count as f64;
    let ring_mean = if ring_count == 0 {
        bg_mean
    } else {
        ring_sum / ring_count as f64
    };
    let logo_var = (logo_sq_sum / logo_count as f64) - logo_mean * logo_mean;
    let logo_std = logo_var.max(0.0).sqrt();
    let logo_contrast = (logo_mean - bg_mean).abs();
    let ring_contrast = (ring_mean - bg_mean).abs();
    let texture = (logo_std / 0.22).clamp(0.0, 1.0);
    (logo_contrast * 0.58 + ring_contrast * 0.22 + texture * 0.20).clamp(0.0, 1.0)
}

fn pixel_luma_norm(pixel: [u8; 4]) -> f64 {
    let luma = (77u32 * pixel[0] as u32 + 150u32 * pixel[1] as u32 + 29u32 * pixel[2] as u32) >> 8;
    luma as f64 / 255.0
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
    let luma = sample_katakana_cell_luma(image, cell_x, cell_y, cell_size, offset_x, offset_y)?;
    let mut best_true = f64::NEG_INFINITY;
    let mut best_false = f64::NEG_INFINITY;
    for pattern_id in 0..SORA_KATAKANA_BITMAPS.len() {
        let bitmap = katakana_pattern_bitmap(pattern_id, cell_x as usize, cell_y as usize);
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

fn sample_katakana_cell_luma(
    image: &image::RgbaImage,
    cell_x: u16,
    cell_y: u16,
    cell_size: u32,
    offset_x: u32,
    offset_y: u32,
) -> Option<[u16; 64]> {
    if cell_size == 0 {
        return None;
    }
    let margin = if cell_size <= 6 { 0.05 } else { 0.035 };
    sample_katakana_cell_luma_with_margin(
        image, cell_x, cell_y, cell_size, offset_x, offset_y, margin,
    )
}

fn sample_katakana_cell_luma_dense(
    image: &image::RgbaImage,
    cell_x: u16,
    cell_y: u16,
    cell_size: u32,
    offset_x: u32,
    offset_y: u32,
) -> Option<[u16; 64]> {
    sample_katakana_cell_luma_with_margin(image, cell_x, cell_y, cell_size, offset_x, offset_y, 0.0)
}

fn sample_katakana_cell_luma_with_margin(
    image: &image::RgbaImage,
    cell_x: u16,
    cell_y: u16,
    cell_size: u32,
    offset_x: u32,
    offset_y: u32,
    margin: f64,
) -> Option<[u16; 64]> {
    if cell_size == 0 {
        return None;
    }
    let width = image.width();
    let height = image.height();
    let span = 1.0 - margin * 2.0;
    if span <= 0.0 {
        return None;
    }

    let mut luma = [0u16; 64];
    const SUB_OFFSETS: [f64; 3] = [-0.22, 0.0, 0.22];
    for sy in 0..8usize {
        for sx in 0..8usize {
            let mut sample_sum = 0u32;
            let mut sample_count = 0u32;
            for &oy in &SUB_OFFSETS {
                for &ox in &SUB_OFFSETS {
                    let u = ((sx as f64 + 0.5 + ox) / 8.0).clamp(0.0, 0.999_999);
                    let v = ((sy as f64 + 0.5 + oy) / 8.0).clamp(0.0, 0.999_999);
                    let fx = cell_x as f64 + margin + span * u;
                    let fy = cell_y as f64 + margin + span * v;
                    let px = (offset_x as f64 + fx * cell_size as f64).floor() as u32;
                    let py = (offset_y as f64 + fy * cell_size as f64).floor() as u32;
                    let px = px.min(width.saturating_sub(1));
                    let py = py.min(height.saturating_sub(1));
                    let pixel = image.get_pixel(px, py).0;
                    let value = (77u32 * pixel[0] as u32
                        + 150u32 * pixel[1] as u32
                        + 29u32 * pixel[2] as u32)
                        >> 8;
                    sample_sum += value;
                    sample_count += 1;
                }
            }
            luma[sy * 8 + sx] = (sample_sum / sample_count.max(1)) as u16;
        }
    }
    Some(luma)
}

fn infer_katakana_cell_symbol(
    image: &image::RgbaImage,
    cell_x: u16,
    cell_y: u16,
    cell_size: u32,
    offset_x: u32,
    offset_y: u32,
) -> Option<(usize, f64)> {
    let luma =
        sample_katakana_cell_luma_dense(image, cell_x, cell_y, cell_size, offset_x, offset_y)?;
    let signature = decode_katakana_symbol_signature(&luma);
    let mut best_symbol = 0usize;
    let mut best_dist = u32::MAX;
    for symbol in 0..katakana_symbol_base() as usize {
        let dist = (signature ^ symbol as u8).count_ones();
        if dist < best_dist {
            best_dist = dist;
            best_symbol = symbol;
        }
    }
    let confidence = (1.0 - best_dist as f64 / 7.0).clamp(0.0, 1.0);
    Some((best_symbol, confidence))
}

fn decode_katakana_symbol_signature(luma: &[u16; 64]) -> u8 {
    const MARKER_ON_THRESHOLD: f64 = 128.0;
    let mut signature = 0u8;
    for (bit_index, &(mx, my)) in KATAKANA_SYMBOL_SIGNATURE_MARKERS.iter().enumerate() {
        let marker_x = mx.max(0) as usize;
        let marker_y = my.max(0) as usize;
        let mut marker_sum = 0u32;
        let mut marker_count = 0u32;
        for dy in 0..=1usize {
            for dx in 0..=1usize {
                let x = (marker_x + dx).min(7);
                let y = (marker_y + dy).min(7);
                marker_sum += u32::from(luma[y * 8 + x]);
                marker_count += 1;
            }
        }
        let marker_avg = marker_sum as f64 / marker_count as f64;
        if marker_avg >= MARKER_ON_THRESHOLD {
            signature |= 1u8 << bit_index;
        }
    }
    signature
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

fn decode_frame_with_grid_and_channel(
    image: &image::RgbaImage,
    grid_size: u16,
    options: PetalStreamOptions,
    channel: PetalDataChannel,
) -> Result<Vec<u8>> {
    match channel {
        PetalDataChannel::Binary => decode_frame_with_grid(image, grid_size, options),
        PetalDataChannel::KatakanaBase94 => {
            decode_katakana_base94_frame_with_grid(image, grid_size, options)
        }
    }
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

fn katakana_symbol_base() -> u32 {
    (IROHA_KATAKANA.len() as u32) * 2
}

const BINARY_DEFAULT_CHUNK_SIZE: u16 = 140;
const KATAKANA_BALANCED_DEFAULT_CHUNK_SIZE: u16 = 176;
const KATAKANA_DISTANCE_SAFE_DEFAULT_CHUNK_SIZE: u16 = 96;
const KATAKANA_BALANCED_MIN_GRID_SIZE: u16 = 41;
const KATAKANA_DISTANCE_SAFE_MIN_GRID_SIZE: u16 = 33;
const KATAKANA_BASE94_REPETITION: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KatakanaGridSizingMode {
    CapacityFirst,
    Balanced,
    DistanceSafe,
}

fn uses_katakana_preset_defaults(
    channel: PetalDataChannel,
    chunk_size: u16,
    grid_size: u16,
) -> bool {
    channel == PetalDataChannel::KatakanaBase94
        && grid_size == 0
        && chunk_size == BINARY_DEFAULT_CHUNK_SIZE
}

fn resolve_encode_chunk_size(
    channel: PetalDataChannel,
    chunk_size: u16,
    grid_size: u16,
    preset: PetalKatakanaPreset,
    use_katakana_preset_defaults: bool,
) -> u16 {
    if channel == PetalDataChannel::KatakanaBase94 && grid_size == 0 && use_katakana_preset_defaults
    {
        match preset {
            PetalKatakanaPreset::Balanced => KATAKANA_BALANCED_DEFAULT_CHUNK_SIZE,
            PetalKatakanaPreset::DistanceSafe => KATAKANA_DISTANCE_SAFE_DEFAULT_CHUNK_SIZE,
        }
    } else {
        chunk_size
    }
}

fn katakana_grid_sizing_mode(preset: PetalKatakanaPreset) -> KatakanaGridSizingMode {
    match preset {
        PetalKatakanaPreset::Balanced => KatakanaGridSizingMode::Balanced,
        PetalKatakanaPreset::DistanceSafe => KatakanaGridSizingMode::DistanceSafe,
    }
}

fn katakana_grid_candidates(mode: KatakanaGridSizingMode) -> Vec<u16> {
    let mut candidates = Vec::with_capacity(PETAL_STREAM_GRID_SIZES.len() + 1);
    let min_grid = match mode {
        KatakanaGridSizingMode::CapacityFirst => None,
        KatakanaGridSizingMode::Balanced => Some(KATAKANA_BALANCED_MIN_GRID_SIZE),
        KatakanaGridSizingMode::DistanceSafe => Some(KATAKANA_DISTANCE_SAFE_MIN_GRID_SIZE),
    };
    if let Some(min_grid) = min_grid {
        candidates.push(min_grid);
    }
    for &candidate in PETAL_STREAM_GRID_SIZES {
        if candidate == 0 {
            continue;
        }
        if let Some(min_grid) = min_grid {
            if candidate < min_grid {
                continue;
            }
        }
        candidates.push(candidate);
    }
    candidates.sort_unstable();
    candidates.dedup();
    if candidates.is_empty() {
        for &candidate in PETAL_STREAM_GRID_SIZES {
            if candidate != 0 {
                candidates.push(candidate);
            }
        }
        candidates.sort_unstable();
        candidates.dedup();
    }
    candidates
}

fn katakana_decode_grid_candidates() -> Vec<u16> {
    let mut candidates = Vec::with_capacity(PETAL_STREAM_GRID_SIZES.len() + 2);
    candidates.push(KATAKANA_DISTANCE_SAFE_MIN_GRID_SIZE);
    candidates.push(KATAKANA_BALANCED_MIN_GRID_SIZE);
    for &candidate in PETAL_STREAM_GRID_SIZES {
        if candidate != 0 {
            candidates.push(candidate);
        }
    }
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

fn katakana_usable_digit_capacity(data_cells: usize) -> usize {
    data_cells - (data_cells % 4)
}

fn katakana_payload_checksum(payload: &[u8]) -> u32 {
    let mut hash = 0x811C_9DC5u32;
    for &byte in payload {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn katakana_base94_frame_capacity_bytes(grid_size: u16, options: PetalStreamOptions) -> usize {
    let data_cells = count_data_cells(grid_size, options);
    let usable_digits = katakana_usable_digit_capacity(data_cells);
    let lane_len = usable_digits / KATAKANA_BASE94_REPETITION;
    let payload_digits = (lane_len / 4) * 4;
    payload_digits / 4 * 3
}

fn resolve_katakana_base94_grid_size(
    max_frame_len: usize,
    base_options: PetalStreamOptions,
    mode: KatakanaGridSizingMode,
) -> Result<u16> {
    for candidate in katakana_grid_candidates(mode) {
        let options = PetalStreamOptions {
            grid_size: candidate,
            ..base_options
        };
        let capacity = katakana_base94_frame_capacity_bytes(candidate, options);
        // 2 bytes length + 4 bytes checksum in katakana packet.
        if capacity >= max_frame_len + 6 {
            return Ok(candidate);
        }
    }
    Err(eyre!(
        "katakana-base94 grid sizing failed: frame payload {max_frame_len} bytes exceeds capacity"
    ))
}

fn katakana_base94_encode_digits(frame_bytes: &[u8], data_cells: usize) -> Result<Vec<u8>> {
    let len = u16::try_from(frame_bytes.len()).map_err(|_| {
        eyre!(
            "frame payload too large for katakana-base94 header: {} bytes",
            frame_bytes.len()
        )
    })?;
    let usable_digits = katakana_usable_digit_capacity(data_cells);
    let mut packet = Vec::with_capacity(frame_bytes.len() + 6);
    packet.extend_from_slice(&len.to_le_bytes());
    packet.extend_from_slice(&katakana_payload_checksum(frame_bytes).to_le_bytes());
    packet.extend_from_slice(frame_bytes);

    let required_digits = packet.len().div_ceil(3) * 4;
    let repeated_digits = required_digits * KATAKANA_BASE94_REPETITION;
    let lane_len = usable_digits / KATAKANA_BASE94_REPETITION;
    if repeated_digits > usable_digits || required_digits > lane_len {
        return Err(eyre!(
            "katakana-base94 capacity exceeded: need {} digits (with repetition), have {}",
            repeated_digits,
            usable_digits
        ));
    }
    let base = katakana_symbol_base();
    let mut payload_digits = Vec::with_capacity(required_digits);
    for chunk in packet.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let mut value = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        for _ in 0..4 {
            payload_digits.push((value % base) as u8);
            value /= base;
        }
    }
    let mut digits = Vec::with_capacity(data_cells);
    digits.resize(data_cells, 0);
    for (digit_index, &digit) in payload_digits.iter().enumerate() {
        for repetition in 0..KATAKANA_BASE94_REPETITION {
            let out_index = repetition * lane_len + digit_index;
            digits[out_index] = digit;
        }
    }
    Ok(digits)
}

fn katakana_base94_decode_digits(digits: &[u8]) -> Result<Vec<u8>> {
    let repeated_usable = katakana_usable_digit_capacity(digits.len());
    if repeated_usable < KATAKANA_BASE94_REPETITION {
        return Err(eyre!(
            "katakana-base94 decode requires at least one repeated digit"
        ));
    }
    let repeated_usable = repeated_usable - (repeated_usable % KATAKANA_BASE94_REPETITION);
    let base = katakana_symbol_base();
    let symbol_count = repeated_usable / KATAKANA_BASE94_REPETITION;
    let mut collapsed = Vec::with_capacity(symbol_count);
    for symbol_index in 0..symbol_count {
        let mut counts = vec![0u8; base as usize];
        for repetition in 0..KATAKANA_BASE94_REPETITION {
            let digit = digits[repetition * symbol_count + symbol_index];
            if u32::from(digit) >= base {
                return Err(eyre!("invalid katakana-base94 digit value {digit}"));
            }
            counts[digit as usize] = counts[digit as usize].saturating_add(1);
        }
        let mut best_digit = 0usize;
        let mut best_count = 0u8;
        for (digit, &count) in counts.iter().enumerate() {
            if count > best_count {
                best_count = count;
                best_digit = digit;
            }
        }
        collapsed.push(best_digit as u8);
    }

    let usable_digits = katakana_usable_digit_capacity(collapsed.len());
    if usable_digits == 0 {
        return Err(eyre!(
            "katakana-base94 decode requires at least 4 collapsed digits"
        ));
    }
    let mut packet = Vec::with_capacity((usable_digits / 4) * 3);
    for chunk in collapsed[..usable_digits].chunks_exact(4) {
        let mut value = 0u32;
        let mut factor = 1u32;
        for &digit in chunk {
            value = value.saturating_add(u32::from(digit) * factor);
            factor = factor.saturating_mul(base);
        }
        if value > 0x00FF_FFFF {
            return Err(eyre!("invalid katakana-base94 packed value {value}"));
        }
        packet.push((value >> 16) as u8);
        packet.push((value >> 8) as u8);
        packet.push(value as u8);
    }
    if packet.len() < 6 {
        return Err(eyre!("katakana-base94 packet too short"));
    }
    let len = u16::from_le_bytes([packet[0], packet[1]]) as usize;
    let checksum = u32::from_le_bytes([packet[2], packet[3], packet[4], packet[5]]);
    if len > packet.len().saturating_sub(6) {
        return Err(eyre!(
            "katakana-base94 length out of range: {len} > {}",
            packet.len().saturating_sub(6)
        ));
    }
    let payload = packet[6..6 + len].to_vec();
    let actual_checksum = katakana_payload_checksum(&payload);
    if actual_checksum != checksum {
        return Err(eyre!(
            "katakana-base94 checksum mismatch: expected {checksum:#010x}, got {actual_checksum:#010x}"
        ));
    }
    Ok(payload)
}

fn decode_katakana_base94_frame_with_grid(
    image: &image::RgbaImage,
    grid_size: u16,
    options: PetalStreamOptions,
) -> Result<Vec<u8>> {
    if grid_size == 0 {
        return Err(eyre!("grid size must be > 0"));
    }
    let digits = extract_katakana_base94_digits_with_grid(image, grid_size, options)?;
    katakana_base94_decode_digits(&digits)
}

fn extract_katakana_base94_digits_with_grid(
    image: &image::RgbaImage,
    grid_size: u16,
    options: PetalStreamOptions,
) -> Result<Vec<u8>> {
    if grid_size == 0 {
        return Err(eyre!("grid size must be > 0"));
    }
    let _sampled = sample_grid_from_rgba(image, grid_size, options)?;
    let size = image.width().min(image.height());
    let cell_size = (size / grid_size as u32).max(1);
    let grid_pixels = grid_size as u32 * cell_size;
    let offset_x = (size.saturating_sub(grid_pixels)) / 2;
    let offset_y = (size.saturating_sub(grid_pixels)) / 2;

    let mut digits = Vec::with_capacity(count_data_cells(grid_size, options));
    for y in 0..grid_size {
        for x in 0..grid_size {
            if render_cell_role(x, y, grid_size, options) != RenderCellRole::Data {
                continue;
            }
            let (state, _confidence) =
                infer_katakana_cell_symbol(image, x, y, cell_size, offset_x, offset_y)
                    .ok_or_else(|| eyre!("failed to infer katakana symbol at cell ({x},{y})"))?;
            if state >= katakana_symbol_base() as usize {
                return Err(eyre!(
                    "katakana symbol state out of range at cell ({x},{y}): {state}"
                ));
            }
            digits.push(state as u8);
        }
    }
    Ok(digits)
}

fn decode_katakana_base94_frame_auto(
    image: &image::RgbaImage,
    options: PetalStreamOptions,
) -> Result<(u16, Vec<u8>)> {
    for candidate in katakana_decode_grid_candidates() {
        if let Ok(bytes) = decode_katakana_base94_frame_with_grid(image, candidate, options) {
            return Ok((candidate, bytes));
        }
    }
    Err(eyre!(
        "failed to auto-detect katakana-base94 grid size; try --grid-size <cells>"
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

#[derive(Clone, Debug)]
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
        channel: PetalDataChannel,
        profile: PetalCaptureProfile,
        grid_size: u16,
        trials_per_frame: u16,
        min_success_ratio: f64,
        seed: u64,
    ) -> Value {
        let mut map = Map::new();
        map.insert("version".to_string(), Value::from(1u64));
        map.insert("channel".to_string(), Value::from(channel.label()));
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

#[derive(Clone, Debug)]
struct RealtimeFrameMetrics {
    index: usize,
    scenario: &'static str,
    decoded: bool,
    ingested: bool,
    decode_ms: f64,
}

#[derive(Clone, Debug)]
struct RealtimeDecodeMetrics {
    frame_count: usize,
    decoded_frames: usize,
    ingested_frames: usize,
    stream_complete: bool,
    stream_received_chunks: usize,
    stream_total_chunks: usize,
    simulated_duration_ms: u64,
    wall_clock_ms: u64,
    avg_decode_ms: f64,
    payload: Option<Vec<u8>>,
    frames: Vec<RealtimeFrameMetrics>,
}

impl RealtimeDecodeMetrics {
    fn to_json(
        &self,
        channel: PetalDataChannel,
        profile: PetalCaptureProfile,
        grid_size: u16,
        seed: u64,
        simulate_fps: u16,
        capture_perturbation: bool,
        allow_incomplete: bool,
    ) -> Value {
        let mut map = Map::new();
        map.insert("version".to_string(), Value::from(1u64));
        map.insert("mode".to_string(), Value::from("realtime-sim"));
        map.insert("channel".to_string(), Value::from(channel.label()));
        map.insert("profile".to_string(), Value::from(profile.label()));
        map.insert("seed".to_string(), Value::from(seed));
        map.insert("grid_size".to_string(), Value::from(grid_size as u64));
        map.insert("simulate_fps".to_string(), Value::from(simulate_fps as u64));
        map.insert(
            "capture_perturbation".to_string(),
            Value::from(capture_perturbation),
        );
        map.insert(
            "allow_incomplete".to_string(),
            Value::from(allow_incomplete),
        );
        map.insert(
            "frame_count".to_string(),
            Value::from(self.frame_count as u64),
        );
        map.insert(
            "decoded_frames".to_string(),
            Value::from(self.decoded_frames as u64),
        );
        map.insert(
            "ingested_frames".to_string(),
            Value::from(self.ingested_frames as u64),
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
        map.insert(
            "simulated_duration_ms".to_string(),
            Value::from(self.simulated_duration_ms),
        );
        map.insert("wall_clock_ms".to_string(), Value::from(self.wall_clock_ms));
        map.insert("avg_decode_ms".to_string(), Value::from(self.avg_decode_ms));
        map.insert(
            "payload_length".to_string(),
            Value::from(self.payload.as_ref().map_or(0usize, Vec::len) as u64),
        );
        let frame_rows = self
            .frames
            .iter()
            .map(|frame| {
                let mut frame_map = Map::new();
                frame_map.insert("index".to_string(), Value::from(frame.index as u64));
                frame_map.insert("scenario".to_string(), Value::from(frame.scenario));
                frame_map.insert("decoded".to_string(), Value::from(frame.decoded));
                frame_map.insert("ingested".to_string(), Value::from(frame.ingested));
                frame_map.insert("decode_ms".to_string(), Value::from(frame.decode_ms));
                Value::Object(frame_map)
            })
            .collect();
        map.insert("frames".to_string(), Value::Array(frame_rows));
        Value::Object(map)
    }
}

#[derive(Clone, Debug)]
struct PetalStyleScoreRow {
    style: PetalRenderStyle,
    aesthetic_score: f64,
    decode_completion_score: f64,
    capture_success_ratio: f64,
    frame_success_ratio: f64,
    effective_payload_bps: f64,
    effective_bps_score: f64,
    overall_score: f64,
    passes_gate: bool,
    metrics: CaptureEvalMetrics,
}

#[derive(Clone, Copy)]
struct PetalStyleScoreReportContext {
    payload_kind: &'static str,
    payload_length: usize,
    frame_count: usize,
    chunk_size: u16,
    parity_group: u8,
    grid_size: u16,
    border: u8,
    anchor_size: u8,
    dimension: u32,
    fps: u16,
    profile: PetalCaptureProfile,
    seed: u64,
    trials_per_frame: u16,
    min_success_ratio: f64,
    estimated_payload_bps: u64,
    target_effective_bps: u64,
    recommended_style: PetalRenderStyle,
}

fn style_score_row_ordering(a: &PetalStyleScoreRow, b: &PetalStyleScoreRow) -> std::cmp::Ordering {
    b.overall_score
        .total_cmp(&a.overall_score)
        .then_with(|| {
            b.decode_completion_score
                .total_cmp(&a.decode_completion_score)
        })
        .then_with(|| b.capture_success_ratio.total_cmp(&a.capture_success_ratio))
        .then_with(|| b.effective_payload_bps.total_cmp(&a.effective_payload_bps))
        .then_with(|| a.style.label().cmp(b.style.label()))
}

fn style_overall_score(
    aesthetic_score: f64,
    decode_completion_score: f64,
    effective_bps_score: f64,
) -> f64 {
    (aesthetic_score.clamp(0.0, 1.0) * 0.25
        + decode_completion_score.clamp(0.0, 1.0) * 0.45
        + effective_bps_score.clamp(0.0, 1.0) * 0.30)
        .clamp(0.0, 1.0)
}

fn build_style_score_report(
    rows: &[PetalStyleScoreRow],
    context: PetalStyleScoreReportContext,
) -> Value {
    let mut map = Map::new();
    map.insert("version".to_string(), Value::from(1u64));
    map.insert(
        "payload_kind".to_string(),
        Value::from(context.payload_kind),
    );
    map.insert(
        "payload_length".to_string(),
        Value::from(context.payload_length as u64),
    );
    map.insert(
        "frame_count".to_string(),
        Value::from(context.frame_count as u64),
    );
    map.insert(
        "chunk_size".to_string(),
        Value::from(context.chunk_size as u64),
    );
    map.insert(
        "parity_group".to_string(),
        Value::from(context.parity_group as u64),
    );
    map.insert(
        "grid_size".to_string(),
        Value::from(context.grid_size as u64),
    );
    map.insert("border".to_string(), Value::from(context.border as u64));
    map.insert(
        "anchor_size".to_string(),
        Value::from(context.anchor_size as u64),
    );
    map.insert(
        "dimension".to_string(),
        Value::from(context.dimension as u64),
    );
    map.insert("fps".to_string(), Value::from(context.fps as u64));
    map.insert("profile".to_string(), Value::from(context.profile.label()));
    map.insert("seed".to_string(), Value::from(context.seed));
    map.insert(
        "trials_per_frame".to_string(),
        Value::from(context.trials_per_frame as u64),
    );
    map.insert(
        "min_success_ratio".to_string(),
        Value::from(context.min_success_ratio),
    );
    map.insert(
        "estimated_payload_bytes_per_second".to_string(),
        Value::from(context.estimated_payload_bps),
    );
    map.insert(
        "target_effective_bps".to_string(),
        Value::from(context.target_effective_bps),
    );
    map.insert(
        "recommended_style".to_string(),
        Value::from(context.recommended_style.label()),
    );
    map.insert(
        "styles_evaluated".to_string(),
        Value::Array(
            rows.iter()
                .map(|row| Value::from(row.style.label()))
                .collect::<Vec<_>>(),
        ),
    );
    map.insert(
        "rows".to_string(),
        Value::Array(rows.iter().map(style_score_row_to_json).collect()),
    );
    Value::Object(map)
}

fn style_score_row_to_json(row: &PetalStyleScoreRow) -> Value {
    let mut map = Map::new();
    map.insert("style".to_string(), Value::from(row.style.label()));
    map.insert(
        "aesthetic_score".to_string(),
        Value::from(row.aesthetic_score),
    );
    map.insert(
        "decode_completion_score".to_string(),
        Value::from(row.decode_completion_score),
    );
    map.insert(
        "capture_success_ratio".to_string(),
        Value::from(row.capture_success_ratio),
    );
    map.insert(
        "frame_success_ratio".to_string(),
        Value::from(row.frame_success_ratio),
    );
    map.insert(
        "effective_payload_bytes_per_second".to_string(),
        Value::from(row.effective_payload_bps),
    );
    map.insert(
        "effective_bps_score".to_string(),
        Value::from(row.effective_bps_score),
    );
    map.insert("overall_score".to_string(), Value::from(row.overall_score));
    map.insert("passes_gate".to_string(), Value::from(row.passes_gate));
    map.insert(
        "stream_complete".to_string(),
        Value::from(row.metrics.stream_complete),
    );
    map.insert(
        "recovered_frames".to_string(),
        Value::from(row.metrics.recovered_frames as u64),
    );
    map.insert(
        "stream_received_chunks".to_string(),
        Value::from(row.metrics.stream_received_chunks as u64),
    );
    map.insert(
        "stream_total_chunks".to_string(),
        Value::from(row.metrics.stream_total_chunks as u64),
    );
    map.insert("attempts".to_string(), Value::from(row.metrics.attempts));
    map.insert("successes".to_string(), Value::from(row.metrics.successes));
    let mut scenario_rows = Vec::with_capacity(row.metrics.scenario.len());
    for scenario in &row.metrics.scenario {
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

fn simulate_realtime_decode(
    images: &[image::RgbaImage],
    options: PetalStreamOptions,
    channel: PetalDataChannel,
    profile: PetalCaptureProfile,
    seed: u64,
    simulate_fps: u16,
    capture_perturbation: bool,
) -> Result<RealtimeDecodeMetrics> {
    if images.is_empty() {
        return Err(eyre!("realtime simulation requires at least one frame"));
    }
    if options.grid_size == 0 {
        return Err(eyre!("realtime simulation requires resolved grid size"));
    }
    if simulate_fps == 0 {
        return Err(eyre!("simulate-fps must be > 0"));
    }

    let scenarios = capture_scenarios(profile);
    if scenarios.is_empty() {
        return Err(eyre!("capture profile has no scenarios"));
    }

    let search_radius = capture_alignment_search_radius(profile);
    let frame_interval_ms = 1000u64 / u64::from(simulate_fps);
    let simulated_duration_ms =
        frame_interval_ms.saturating_mul(images.len().saturating_sub(1) as u64);

    let wall_start = Instant::now();
    let mut assembler = QrStreamAssembler::default();
    let mut final_result: Option<QrStreamDecodeResult> = None;
    let mut decoded_frames = 0usize;
    let mut ingested_frames = 0usize;
    let mut decode_ms_total = 0.0;
    let mut frame_metrics = Vec::with_capacity(images.len());

    for (frame_index, image) in images.iter().enumerate() {
        let scenario = scenarios[frame_index % scenarios.len()];
        let simulated = if capture_perturbation {
            simulate_capture_frame(image, scenario, frame_index, 0, seed)
        } else {
            image.clone()
        };

        let decode_started = Instant::now();
        let decoded = decode_frame_with_alignment_search(
            &simulated,
            options.grid_size,
            options,
            channel,
            search_radius,
        );
        let decode_ms = decode_started.elapsed().as_secs_f64() * 1_000.0;
        decode_ms_total += decode_ms;

        let mut decoded_ok = false;
        let mut ingested = false;
        if let Ok(bytes) = decoded {
            decoded_ok = true;
            decoded_frames += 1;
            if let Ok(frame) = QrStreamFrame::decode(&bytes) {
                if let Ok(result) = assembler.ingest_frame(frame) {
                    ingested = true;
                    ingested_frames += 1;
                    final_result = Some(result);
                }
            }
        }

        frame_metrics.push(RealtimeFrameMetrics {
            index: frame_index,
            scenario: scenario.name,
            decoded: decoded_ok,
            ingested,
            decode_ms,
        });
    }

    let wall_clock_ms = wall_start.elapsed().as_millis() as u64;
    let avg_decode_ms = if images.is_empty() {
        0.0
    } else {
        decode_ms_total / images.len() as f64
    };
    let (stream_complete, stream_received_chunks, stream_total_chunks, payload) =
        if let Some(result) = final_result {
            (
                result.is_complete(),
                result.received_chunks,
                result.total_chunks,
                result.payload,
            )
        } else {
            (false, 0, 0, None)
        };

    Ok(RealtimeDecodeMetrics {
        frame_count: images.len(),
        decoded_frames,
        ingested_frames,
        stream_complete,
        stream_received_chunks,
        stream_total_chunks,
        simulated_duration_ms,
        wall_clock_ms,
        avg_decode_ms,
        payload,
        frames: frame_metrics,
    })
}

fn evaluate_capture_robustness(
    images: &[image::RgbaImage],
    expected_bytes: &[Vec<u8>],
    grid_size: u16,
    options: PetalStreamOptions,
    channel: PetalDataChannel,
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
            let decoded = decode_frame_with_alignment_search(
                &simulated,
                grid_size,
                options,
                channel,
                search_radius,
            );
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
    channel: PetalDataChannel,
    search_radius: i32,
) -> Result<Vec<u8>> {
    if let Ok(bytes) = decode_frame_with_grid_and_channel(image, grid_size, options, channel) {
        return Ok(bytes);
    }
    for radius in 1..=search_radius.max(0) {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dy.abs() != radius {
                    continue;
                }
                let shifted = translate_image(image, dx, dy, image::Rgba([4, 1, 8, 0xFF]));
                if let Ok(bytes) =
                    decode_frame_with_grid_and_channel(&shifted, grid_size, options, channel)
                {
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
    use std::{fs, path::Path};
    use tempfile::tempdir;

    fn sample_style_metrics(
        frame_count: usize,
        successes: u64,
        attempts: u64,
        stream_complete: bool,
    ) -> CaptureEvalMetrics {
        CaptureEvalMetrics {
            frame_count,
            frame_successes: frame_count,
            attempts,
            successes,
            recovered_frames: frame_count,
            stream_complete,
            stream_received_chunks: frame_count,
            stream_total_chunks: frame_count,
            scenario: vec![CaptureScenarioMetrics {
                name: "sample",
                attempts,
                successes,
            }],
        }
    }

    fn workspace_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
    }

    fn fixture_aggregate_proof_payload() -> Vec<u8> {
        let fixture = workspace_root().join("fixtures/offline_bundle/aggregate_proof_fixture.json");
        fs::read(&fixture).expect("offline aggregate proof fixture should exist")
    }

    fn decode_stream_payload(frame_bytes: &[Vec<u8>]) -> Vec<u8> {
        let mut assembler = QrStreamAssembler::default();
        let mut result = None;
        for bytes in frame_bytes {
            let frame = QrStreamFrame::decode(bytes).expect("decode frame bytes");
            result = Some(assembler.ingest_frame(frame).expect("ingest frame"));
        }
        let result = result.expect("stream decode result");
        assert!(result.is_complete(), "decoded stream must be complete");
        result.payload.expect("decoded payload")
    }

    fn render_katakana_stream(
        payload: &[u8],
        dimension: u32,
        style: PetalRenderStyle,
        chunk_size: u16,
        mode: KatakanaGridSizingMode,
    ) -> (Vec<image::RgbaImage>, Vec<Vec<u8>>, PetalStreamOptions) {
        let stream_options = QrStreamOptions {
            chunk_size,
            parity_group: 0,
            ..QrStreamOptions::default()
        };
        let (_envelope, frames) =
            QrStreamEncoder::encode_frames(payload, stream_options).expect("encode stream");
        let frame_bytes: Vec<Vec<u8>> = frames.iter().map(QrStreamFrame::encode).collect();
        let max_len = frame_bytes.iter().map(Vec::len).max().unwrap_or_default();
        let grid_size =
            resolve_katakana_base94_grid_size(max_len, PetalStreamOptions::default(), mode)
                .expect("resolve katakana grid");
        let petal_options = PetalStreamOptions {
            grid_size,
            ..PetalStreamOptions::default()
        };
        let template_grid = PetalStreamEncoder::encode_grid(&[], petal_options).expect("grid");
        let frame_count = frame_bytes.len().max(1) as u32;
        let mut images = Vec::with_capacity(frame_bytes.len());
        for (index, bytes) in frame_bytes.iter().enumerate() {
            images.push(render_petal_frame_with_channel(
                &template_grid,
                dimension,
                index as u32,
                frame_count,
                style,
                petal_options,
                PetalDataChannel::KatakanaBase94,
                Some(bytes),
            ));
        }
        (images, frame_bytes, petal_options)
    }

    fn render_katakana_balanced_stream(
        payload: &[u8],
        dimension: u32,
        style: PetalRenderStyle,
    ) -> (Vec<image::RgbaImage>, Vec<Vec<u8>>, PetalStreamOptions) {
        render_katakana_stream(
            payload,
            dimension,
            style,
            KATAKANA_BALANCED_DEFAULT_CHUNK_SIZE,
            KatakanaGridSizingMode::Balanced,
        )
    }

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
    fn katakana_base94_symbol_codec_roundtrip() {
        let payload: Vec<u8> = (0u8..=127u8).collect();
        let digits = katakana_base94_encode_digits(&payload, 1024).expect("encode digits");
        let decoded = katakana_base94_decode_digits(&digits).expect("decode digits");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn katakana_base94_capacity_is_high_for_standard_grid() {
        let options = PetalStreamOptions {
            grid_size: 53,
            ..PetalStreamOptions::default()
        };
        let capacity = katakana_base94_frame_capacity_bytes(53, options);
        assert!(
            capacity >= 320,
            "unexpected katakana-base94 capacity: {capacity}"
        );
    }

    #[test]
    fn sora_temple_katakana_base94_roundtrip_decodes_frame_bytes() {
        let payload: Vec<u8> = (0u8..=180u8).collect();
        let options = PetalStreamOptions::default();
        let grid = PetalStreamEncoder::encode_grid(&payload, options).expect("grid");
        let image = render_petal_frame_with_channel(
            &grid,
            848,
            1,
            8,
            PetalRenderStyle::SoraTemple,
            options,
            PetalDataChannel::KatakanaBase94,
            Some(&payload),
        );
        let decoded = decode_katakana_base94_frame_with_grid(
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
    fn katakana_base94_symbol_inference_accuracy_is_high() {
        let payload: Vec<u8> = (0u8..=180u8).collect();
        let options = PetalStreamOptions::default();
        let grid = PetalStreamEncoder::encode_grid(&payload, options).expect("grid");
        let data_cells = count_data_cells(grid.grid_size, options);
        let expected_digits = katakana_base94_encode_digits(&payload, data_cells).expect("digits");
        let image = render_petal_frame_with_channel(
            &grid,
            848,
            1,
            8,
            PetalRenderStyle::SoraTemple,
            options,
            PetalDataChannel::KatakanaBase94,
            Some(&payload),
        );
        let actual_digits = extract_katakana_base94_digits_with_grid(
            &image,
            grid.grid_size,
            PetalStreamOptions {
                grid_size: grid.grid_size,
                ..options
            },
        )
        .expect("extract digits");
        assert_eq!(actual_digits.len(), expected_digits.len());
        let mut mismatches = 0usize;
        let mut toggled_47 = 0usize;
        for (a, b) in actual_digits.iter().zip(expected_digits.iter()) {
            if a != b {
                mismatches += 1;
                if (*a as i32 - *b as i32).abs() == IROHA_KATAKANA.len() as i32 {
                    toggled_47 += 1;
                }
            }
        }
        let error_rate = mismatches as f64 / expected_digits.len() as f64;
        assert!(
            error_rate < 0.02,
            "katakana-base94 symbol error rate too high: {error_rate:.4}; toggled_47={toggled_47}; expected_head={:?}; actual_head={:?}",
            &expected_digits[..expected_digits.len().min(12)],
            &actual_digits[..actual_digits.len().min(12)]
        );
    }

    #[test]
    fn katakana_balanced_defaults_resolve_expected_chunk_and_grid() {
        assert!(uses_katakana_preset_defaults(
            PetalDataChannel::KatakanaBase94,
            BINARY_DEFAULT_CHUNK_SIZE,
            0
        ));
        let resolved_chunk = resolve_encode_chunk_size(
            PetalDataChannel::KatakanaBase94,
            BINARY_DEFAULT_CHUNK_SIZE,
            0,
            PetalKatakanaPreset::Balanced,
            true,
        );
        assert_eq!(resolved_chunk, KATAKANA_BALANCED_DEFAULT_CHUNK_SIZE);

        let payload = fixture_aggregate_proof_payload();
        let stream_options = QrStreamOptions {
            chunk_size: resolved_chunk,
            parity_group: 0,
            ..QrStreamOptions::default()
        };
        let (_envelope, frames) =
            QrStreamEncoder::encode_frames(&payload, stream_options).expect("encode stream");
        let max_len = frames
            .iter()
            .map(|frame| QrStreamFrame::encode(frame).len())
            .max()
            .unwrap_or_default();
        let grid_size = resolve_katakana_base94_grid_size(
            max_len,
            PetalStreamOptions::default(),
            KatakanaGridSizingMode::Balanced,
        )
        .expect("resolve grid");
        assert_eq!(
            grid_size, KATAKANA_BALANCED_MIN_GRID_SIZE,
            "balanced preset should pick the 41-cell grid when capacity allows"
        );
    }

    #[test]
    fn katakana_distance_safe_defaults_resolve_expected_chunk_and_grid() {
        assert!(uses_katakana_preset_defaults(
            PetalDataChannel::KatakanaBase94,
            BINARY_DEFAULT_CHUNK_SIZE,
            0
        ));
        let resolved_chunk = resolve_encode_chunk_size(
            PetalDataChannel::KatakanaBase94,
            BINARY_DEFAULT_CHUNK_SIZE,
            0,
            PetalKatakanaPreset::DistanceSafe,
            true,
        );
        assert_eq!(resolved_chunk, KATAKANA_DISTANCE_SAFE_DEFAULT_CHUNK_SIZE);

        let payload = fixture_aggregate_proof_payload();
        let stream_options = QrStreamOptions {
            chunk_size: resolved_chunk,
            parity_group: 0,
            ..QrStreamOptions::default()
        };
        let (_envelope, frames) =
            QrStreamEncoder::encode_frames(&payload, stream_options).expect("encode stream");
        let max_len = frames
            .iter()
            .map(|frame| QrStreamFrame::encode(frame).len())
            .max()
            .unwrap_or_default();
        let grid_size = resolve_katakana_base94_grid_size(
            max_len,
            PetalStreamOptions::default(),
            KatakanaGridSizingMode::DistanceSafe,
        )
        .expect("resolve grid");
        assert_eq!(
            grid_size, KATAKANA_DISTANCE_SAFE_MIN_GRID_SIZE,
            "distance-safe preset should pick the 33-cell grid when capacity allows"
        );
    }

    #[test]
    fn katakana_balanced_frame_count_is_lower_than_binary_baseline() {
        let payload = fixture_aggregate_proof_payload();
        let binary_options = QrStreamOptions {
            chunk_size: BINARY_DEFAULT_CHUNK_SIZE,
            parity_group: 0,
            ..QrStreamOptions::default()
        };
        let katakana_options = QrStreamOptions {
            chunk_size: KATAKANA_BALANCED_DEFAULT_CHUNK_SIZE,
            parity_group: 0,
            ..QrStreamOptions::default()
        };
        let (_, binary_frames) =
            QrStreamEncoder::encode_frames(&payload, binary_options).expect("binary encode");
        let (_, katakana_frames) =
            QrStreamEncoder::encode_frames(&payload, katakana_options).expect("katakana encode");
        assert!(
            katakana_frames.len() < binary_frames.len(),
            "katakana balanced chunking should reduce frame count (binary={}, katakana={})",
            binary_frames.len(),
            katakana_frames.len()
        );
    }

    #[test]
    fn katakana_balanced_boxes_are_larger_than_high_density_grid() {
        let balanced_cell_size = (1024u32 / u32::from(KATAKANA_BALANCED_MIN_GRID_SIZE)).max(1);
        let dense_cell_size = (1024u32 / 47).max(1);
        assert!(
            balanced_cell_size > dense_cell_size,
            "balanced cell size should exceed dense baseline (balanced={balanced_cell_size}, dense={dense_cell_size})"
        );
    }

    #[test]
    fn katakana_balanced_roundtrip_matches_real_fixture_payload() {
        let payload = fixture_aggregate_proof_payload();
        let (images, frame_bytes, petal_options) =
            render_katakana_balanced_stream(&payload, 1024, PetalRenderStyle::SoraTempleCommand);
        let mut decoded_frames = Vec::with_capacity(frame_bytes.len());
        for image in &images {
            decoded_frames.push(
                decode_katakana_base94_frame_with_grid(
                    image,
                    petal_options.grid_size,
                    petal_options,
                )
                .expect("decode katakana frame"),
            );
        }
        let decoded_payload = decode_stream_payload(&decoded_frames);
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn realtime_simulation_reconstructs_katakana_stream_without_capture_perturbation() {
        let payload = b"petal-realtime-distance-safe-katakana";
        let (images, _frame_bytes, petal_options) = render_katakana_stream(
            payload,
            640,
            PetalRenderStyle::SoraTempleCommand,
            KATAKANA_DISTANCE_SAFE_DEFAULT_CHUNK_SIZE,
            KatakanaGridSizingMode::DistanceSafe,
        );
        let metrics = simulate_realtime_decode(
            &images,
            petal_options,
            PetalDataChannel::KatakanaBase94,
            PetalCaptureProfile::Default,
            11,
            24,
            false,
        )
        .expect("realtime simulation");
        assert_eq!(metrics.frame_count, images.len());
        assert_eq!(metrics.decoded_frames, images.len());
        assert!(
            metrics.stream_complete,
            "stream should complete in realtime simulation"
        );
        assert_eq!(metrics.payload.expect("payload"), payload);
    }

    #[test]
    fn katakana_balanced_capture_stress_passes_default_gate() {
        let payload = fixture_aggregate_proof_payload();
        let (images, frame_bytes, petal_options) =
            render_katakana_balanced_stream(&payload, 768, PetalRenderStyle::SoraTempleCommand);
        let sample_len = images.len().min(4);
        let images = images[..sample_len].to_vec();
        let frame_bytes = frame_bytes[..sample_len].to_vec();
        let metrics = evaluate_capture_robustness(
            &images,
            &frame_bytes,
            petal_options.grid_size,
            petal_options,
            PetalDataChannel::KatakanaBase94,
            PetalCaptureProfile::Default,
            42,
            2,
        )
        .expect("evaluate capture");
        assert!(
            metrics.success_ratio() >= 0.50,
            "balanced katakana capture success ratio below gate: {:.3}",
            metrics.success_ratio()
        );
    }

    #[test]
    fn sora_ten_logo_glyph_matches_reference_landmarks() {
        // Circle body
        assert!(sora_ten_logo_disk_mask(0.50, 0.50));
        // Top bar
        assert!(sora_ten_logo_glyph_mask(0.50, 0.24));
        // Split middle bar
        assert!(sora_ten_logo_glyph_mask(0.24, 0.42));
        assert!(sora_ten_logo_glyph_mask(0.76, 0.42));
        assert!(!sora_ten_logo_glyph_mask(0.50, 0.42));
        // Center stem
        assert!(sora_ten_logo_glyph_mask(0.50, 0.34));
        // Legs and center opening
        assert!(sora_ten_logo_glyph_mask(0.34, 0.70));
        assert!(sora_ten_logo_glyph_mask(0.66, 0.70));
        assert!(!sora_ten_logo_glyph_mask(0.50, 0.86));
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
            PetalDataChannel::Binary,
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

    #[test]
    fn resolve_score_styles_uses_defaults_and_deduplicates() {
        let defaults = resolve_score_styles(&[]);
        assert_eq!(
            defaults.first().copied(),
            Some(PetalRenderStyle::SoraTemple)
        );
        assert!(defaults.contains(&PetalRenderStyle::SoraTempleGhost));
        let deduped = resolve_score_styles(&[
            PetalRenderStyle::SoraTemple,
            PetalRenderStyle::SoraTemple,
            PetalRenderStyle::SoraTempleGhost,
        ]);
        assert_eq!(
            deduped,
            vec![
                PetalRenderStyle::SoraTemple,
                PetalRenderStyle::SoraTempleGhost
            ]
        );
    }

    #[test]
    fn aesthetic_score_is_normalized() {
        let payload = b"petal-style-aesthetic";
        let options = PetalStreamOptions::default();
        let grid = PetalStreamEncoder::encode_grid(payload, options).expect("encode");
        let image = render_petal_frame(&grid, 192, 0, 6, PetalRenderStyle::SoraTemple, options);
        let score = estimate_style_aesthetic_score(&image, PetalRenderStyle::SoraTemple);
        assert!((0.0..=1.0).contains(&score));
        assert!(score > 0.08, "unexpectedly low aesthetic score: {score:.3}");
    }

    #[test]
    fn style_score_rows_sort_by_overall_then_decode() {
        let mut rows = vec![
            PetalStyleScoreRow {
                style: PetalRenderStyle::SoraTempleCommand,
                aesthetic_score: 0.72,
                decode_completion_score: 0.90,
                capture_success_ratio: 0.88,
                frame_success_ratio: 0.90,
                effective_payload_bps: 2_640.0,
                effective_bps_score: 0.88,
                overall_score: 0.84,
                passes_gate: false,
                metrics: sample_style_metrics(8, 88, 100, false),
            },
            PetalStyleScoreRow {
                style: PetalRenderStyle::SoraTemple,
                aesthetic_score: 0.70,
                decode_completion_score: 1.0,
                capture_success_ratio: 0.94,
                frame_success_ratio: 1.0,
                effective_payload_bps: 2_820.0,
                effective_bps_score: 0.94,
                overall_score: 0.91,
                passes_gate: true,
                metrics: sample_style_metrics(8, 94, 100, true),
            },
        ];
        rows.sort_by(style_score_row_ordering);
        assert_eq!(rows[0].style, PetalRenderStyle::SoraTemple);
        let report = build_style_score_report(
            &rows,
            PetalStyleScoreReportContext {
                payload_kind: "unspecified",
                payload_length: 1024,
                frame_count: 8,
                chunk_size: 140,
                parity_group: 0,
                grid_size: 41,
                border: 1,
                anchor_size: 3,
                dimension: 512,
                fps: 24,
                profile: PetalCaptureProfile::Default,
                seed: 7,
                trials_per_frame: 5,
                min_success_ratio: 0.95,
                estimated_payload_bps: 3_220,
                target_effective_bps: 3_000,
                recommended_style: rows[0].style,
            },
        );
        let json = norito::json::to_string(&report).expect("json");
        assert!(json.contains("\"recommended_style\":\"sora-temple\""));
        assert!(json.contains("\"styles_evaluated\":[\"sora-temple\",\"sora-temple-command\"]"));
    }
}
