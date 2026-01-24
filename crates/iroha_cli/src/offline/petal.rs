use super::*;

use iroha_data_model::{
    petal_stream::{
        PetalStreamDecoder, PetalStreamEncoder, PetalStreamGrid, PetalStreamOptions,
        PetalStreamSampleGrid, PETAL_STREAM_GRID_SIZES,
    },
    qr_stream::{
        QrStreamAssembler, QrStreamDecodeResult, QrStreamEncoder, QrStreamEnvelope,
        QrStreamFrame, QrStreamFrameKind, QrStreamOptions,
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
}

impl Run for PetalCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            PetalCommand::Encode(args) => args.run(context),
            PetalCommand::Decode(args) => args.run(context),
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
    #[arg(long, default_value_t = 360)]
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
    #[arg(long, value_enum, default_value = "sakura-wind")]
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
        fs::create_dir_all(output_dir)
            .map_err(|err| eyre!("failed to create output dir {}: {err}", output_dir.display()))?;
        let frames_dir = output_dir.join("frames");
        fs::create_dir_all(&frames_dir)
            .map_err(|err| eyre!("failed to create frames dir {}: {err}", frames_dir.display()))?;

        let mut manifest_frames: Vec<Value> = Vec::new();
        let mut rendered = Vec::new();
        for (idx, (frame, bytes)) in frames.iter().zip(encoded_frames.iter()).enumerate() {
            let file_name = format!("frame_{idx:04}.bin");
            let path = frames_dir.join(&file_name);
            fs::write(&path, bytes).map_err(|err| {
                eyre!("failed to write frame {}: {err}", path.display())
            })?;
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
            frame_map.insert(
                "bytes_hex".to_string(),
                Value::from(hex::encode(bytes)),
            );
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
                    let image = render_petal_frame(&frame.grid, self.dimension, frame.index, total, self.style);
                    let path = png_dir.join(format!("frame_{:04}.png", frame.index));
                    write_png_rgba(&path, &image)?;
                }
            }
            PetalOutputFormat::Gif => {
                let total = rendered.len().max(1) as u32;
                let frames = rendered
                    .iter()
                    .map(|frame| {
                        render_petal_frame(&frame.grid, self.dimension, frame.index, total, self.style)
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
                        render_petal_frame(&frame.grid, self.dimension, frame.index, total, self.style)
                    })
                    .collect::<Vec<_>>();
                let path = output_dir.join("stream.png");
                write_apng_rgba(&path, &frames, self.fps)?;
            }
        }

        let mut manifest_map = Map::new();
        manifest_map.insert("version".to_string(), Value::from(1u64));
        manifest_map.insert(
            "payload_kind".to_string(),
            Value::from(self.payload_kind.label()),
        );
        manifest_map.insert("chunk_size".to_string(), Value::from(self.chunk_size as u64));
        manifest_map.insert(
            "parity_group".to_string(),
            Value::from(self.parity_group as u64),
        );
        manifest_map.insert("grid_size".to_string(), Value::from(resolved_grid_size as u64));
        manifest_map.insert("border".to_string(), Value::from(petal_options.border as u64));
        manifest_map.insert(
            "anchor_size".to_string(),
            Value::from(petal_options.anchor_size as u64),
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
            eyre!("failed to write manifest {}: {err}", manifest_path.display())
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
            grid_size: if self.grid_size == 0 { 0 } else { self.grid_size },
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
        fs::write(&self.output, payload.as_slice()).map_err(|err| {
            eyre!("failed to write {}: {err}", self.output.display())
        })?;

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
            fs::write(&path, format!("{rendered}\n")).map_err(|err| {
                eyre!("failed to write manifest {}: {err}", path.display())
            })?;
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
}

struct RenderedFrame {
    index: u32,
    grid: PetalStreamGrid,
}

const SAKURA_BG_START: [f64; 3] = [0.99, 0.93, 0.96];
const SAKURA_BG_END: [f64; 3] = [1.0, 0.98, 0.99];
const SAKURA_PETAL: [f64; 3] = [1.0, 0.79, 0.87];
const SAKURA_INK: [f64; 3] = [0.26, 0.08, 0.14];
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
    _style: PetalRenderStyle,
) -> image::RgbaImage {
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
    for y in 0..grid_size {
        for x in 0..grid_size {
            let idx = y as usize * grid_size as usize + x as usize;
            let bit = grid.cells[idx];
            let mut seed = cell_seed(x, y);
            let jitter_x = (sample_unit(&mut seed) - 0.5) * DATA_PETAL_JITTER;
            let jitter_y = (sample_unit(&mut seed) - 0.5) * DATA_PETAL_JITTER;
            let angle = (sample_unit(&mut seed) - 0.5) * std::f64::consts::TAU * DATA_PETAL_ROTATION;
            let sway = (sample_unit(&mut seed) - 0.5) * 2.0;
            cell_params.push(CellParam {
                bit,
                jitter_x,
                jitter_y,
                angle,
                sway,
            });
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

            let streak = (nx * SAKURA_WIND_STREAK_FREQ_X
                + ny * SAKURA_WIND_STREAK_FREQ_Y
                + wind_angle)
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
                let petal_angle = petal_phase + (petal_index as f64 / SAKURA_BG_PETAL_COUNT as f64) * std::f64::consts::TAU;
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
                        let glow = (1.0 - dist / glow_radius_sq)
                            .clamp(0.0, 1.0)
                            * DATA_PETAL_GLOW_ALPHA;
                        r = lerp(r, SAKURA_PETAL[0], glow);
                        g = lerp(g, SAKURA_PETAL[1], glow);
                        b = lerp(b, SAKURA_PETAL[2], glow);
                    }
                    if dist <= 1.0 {
                        r = SAKURA_INK[0];
                        g = SAKURA_INK[1];
                        b = SAKURA_INK[2];
                        let highlight = (1.0 - dist)
                            .clamp(0.0, 1.0)
                            * DATA_PETAL_HIGHLIGHT_ALPHA;
                        r = lerp(r, SAKURA_PETAL[0], highlight);
                        g = lerp(g, SAKURA_PETAL[1], highlight);
                        b = lerp(b, SAKURA_PETAL[2], highlight);
                    }
                }
            }

            rgba.extend_from_slice(&[
                to_u8(r),
                to_u8(g),
                to_u8(b),
                0xFF,
            ]);
        }
    }

    image::RgbaImage::from_raw(dimension, dimension, rgba).expect("rgba buffer size")
}

fn sample_grid_from_rgba(
    image: &image::RgbaImage,
    grid_size: u16,
) -> Result<PetalStreamSampleGrid> {
    let width = image.width();
    let height = image.height();
    let size = width.min(height);
    let offset_x = (width - size) / 2;
    let offset_y = (height - size) / 2;
    let cell_size = (size / grid_size as u32).max(1);
    let mut samples = Vec::with_capacity(grid_size as usize * grid_size as usize);
    for y in 0..grid_size {
        for x in 0..grid_size {
            let mut sum = 0u64;
            let mut count = 0u64;
            for oy in [0.25, 0.5, 0.75] {
                for ox in [0.25, 0.5, 0.75] {
                    let px = offset_x
                        + ((x as f64 + ox) * cell_size as f64).floor() as u32;
                    let py = offset_y
                        + ((y as f64 + oy) * cell_size as f64).floor() as u32;
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
    PetalStreamSampleGrid::new(grid_size, samples).map_err(|err| eyre!("{err}"))
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
    let samples = sample_grid_from_rgba(image, grid_size)?;
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
        if let Ok(samples) = sample_grid_from_rgba(image, candidate) {
            if let Ok(bytes) = PetalStreamDecoder::decode_samples(&samples, options) {
                return Ok((candidate, bytes));
            }
        }
    }
    Err(eyre!(
        "failed to auto-detect grid size; try --grid-size <cells>"
    ))
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
        let image = render_petal_frame(&grid, 64, 0, 1, PetalRenderStyle::SakuraWind);
        assert_eq!(image.width(), 64);
        assert_eq!(image.height(), 64);
        assert_eq!(image.as_raw().len(), 64 * 64 * 4);
    }

    #[test]
    fn write_png_rgba_emits_rgba() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("frame.png");
        let payload = b"petal-cli";
        let grid = PetalStreamEncoder::encode_grid(payload, PetalStreamOptions::default())
            .expect("encode");
        let image = render_petal_frame(&grid, 32, 0, 1, PetalRenderStyle::SakuraWind);
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
        let image = render_petal_frame(&grid, 96, 0, 1, PetalRenderStyle::SakuraWind);
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
        let image = render_petal_frame(&grid, 128, 0, 1, PetalRenderStyle::SakuraWind);
        let samples = sample_grid_from_rgba(&image, grid.grid_size).expect("samples");
        let decoded = PetalStreamDecoder::decode_samples(&samples, PetalStreamOptions::default())
            .expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn decode_frame_with_grid_roundtrip() {
        let payload = b"petal-cli-grid";
        let grid = PetalStreamEncoder::encode_grid(payload, PetalStreamOptions::default())
            .expect("encode");
        let image = render_petal_frame(&grid, 128, 0, 1, PetalRenderStyle::SakuraWind);
        let decoded = decode_frame_with_grid(&image, grid.grid_size, PetalStreamOptions::default())
            .expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn decode_frame_auto_detects_grid_size() {
        let payload = b"petal-cli-auto";
        let grid = PetalStreamEncoder::encode_grid(payload, PetalStreamOptions::default())
            .expect("encode");
        let image = render_petal_frame(&grid, 128, 0, 1, PetalRenderStyle::SakuraWind);
        let (grid_size, decoded) =
            decode_frame_auto(&image, PetalStreamOptions::default()).expect("auto");
        assert_eq!(grid_size, grid.grid_size);
        assert_eq!(decoded, payload);
    }
}
