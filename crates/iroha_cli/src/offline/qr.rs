use super::*;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use iroha_data_model::qr_stream::{
    QrPayloadKind, QrStreamAssembler, QrStreamDecodeResult, QrStreamEncoder, QrStreamEnvelope,
    QrStreamFrame, QrStreamFrameKind, QrStreamOptions,
};
use norito::json::{Map, Value};
use qrcode::{EcLevel, QrCode, render::svg};
use std::{
    fs,
    io::BufWriter,
    path::{Path, PathBuf},
};

#[derive(clap::Subcommand, Debug)]
pub enum QrCommand {
    /// Encode a payload into QR stream frames.
    Encode(QrEncodeArgs),
    /// Decode QR stream frames into the original payload.
    Decode(QrDecodeArgs),
}

impl Run for QrCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            QrCommand::Encode(args) => args.run(context),
            QrCommand::Decode(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct QrEncodeArgs {
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
    /// QR error correction level.
    #[arg(long, value_enum, default_value = "m")]
    pub ecc: QrErrorCorrection,
    /// QR frame encoding mode.
    #[arg(long, value_enum, default_value = "binary")]
    pub frame_encoding: QrFrameEncoding,
    /// Rendered QR image size in pixels.
    #[arg(long, default_value_t = 512)]
    pub dimension: u32,
    /// Output format for rendered frames.
    #[arg(long, value_enum, default_value = "frames")]
    pub format: QrOutputFormat,
    /// Render style for preview images (ignored for --format frames).
    #[arg(long, value_enum, default_value = "mono")]
    pub style: QrRenderStyle,
    /// Frames per second for animated outputs.
    #[arg(long, default_value_t = 12)]
    pub fps: u16,
}

impl QrEncodeArgs {
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
        let output_dir = &self.output;
        fs::create_dir_all(output_dir)
            .map_err(|err| eyre!("failed to create output dir {}: {err}", output_dir.display()))?;
        let frames_dir = output_dir.join("frames");
        fs::create_dir_all(&frames_dir)
            .map_err(|err| eyre!("failed to create frames dir {}: {err}", frames_dir.display()))?;

        let mut manifest_frames: Vec<Value> = Vec::new();
        let mut rendered = Vec::new();
        for (idx, frame) in frames.iter().enumerate() {
            let bytes = frame.encode();
            let file_name = format!("frame_{idx:04}.bin");
            let path = frames_dir.join(&file_name);
            fs::write(&path, &bytes).map_err(|err| {
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
                Value::from(hex::encode(&bytes)),
            );
            manifest_frames.push(Value::Object(frame_map));
            if self.format != QrOutputFormat::Frames {
                let payload_bytes = encode_frame_bytes(&bytes, self.frame_encoding)?;
                let code = QrCode::with_error_correction_level(payload_bytes.as_slice(), self.ecc.into())
                    .map_err(|err| eyre!("failed to render QR code: {err}"))?;
                rendered.push(RenderedFrame { index: idx as u32, code });
            }
        }

        match self.format {
            QrOutputFormat::Frames => {}
            QrOutputFormat::Svg => {
                let svg_dir = output_dir.join("svg");
                fs::create_dir_all(&svg_dir).map_err(|err| {
                    eyre!("failed to create svg dir {}: {err}", svg_dir.display())
                })?;
                for frame in &rendered {
                    let svg = render_svg(&frame.code, self.dimension, self.style)?;
                    let path = svg_dir.join(format!("frame_{:04}.svg", frame.index));
                    fs::write(&path, svg)
                        .map_err(|err| eyre!("failed to write {}: {err}", path.display()))?;
                }
            }
            QrOutputFormat::Png => {
                let png_dir = output_dir.join("png");
                fs::create_dir_all(&png_dir).map_err(|err| {
                    eyre!("failed to create png dir {}: {err}", png_dir.display())
                })?;
                let total_frames = rendered.len() as u32;
                for frame in &rendered {
                    let path = png_dir.join(format!("frame_{:04}.png", frame.index));
                    if matches!(
                        self.style,
                        QrRenderStyle::SakuraWind | QrRenderStyle::SakuraStorm
                    ) {
                        let image = render_stylized_rgba(
                            &frame.code,
                            self.dimension,
                            frame.index,
                            total_frames,
                            self.style,
                        );
                        write_png_rgba(&path, &image)?;
                    } else {
                        let image = render_image(&frame.code, self.dimension)?;
                        write_png(&path, &image, self.style, frame.index, total_frames)?;
                    }
                }
            }
            QrOutputFormat::Gif => {
                let path = output_dir.join("stream.gif");
                if matches!(
                    self.style,
                    QrRenderStyle::SakuraWind | QrRenderStyle::SakuraStorm
                ) {
                    let frames = render_stylized_animation(&rendered, self.dimension, self.style);
                    write_gif_rgba(&path, &frames, self.fps)?;
                } else {
                    let frames = render_animation(&rendered, self.dimension)?;
                    write_gif(&path, &frames, self.fps, self.style)?;
                }
            }
            QrOutputFormat::Apng => {
                let path = output_dir.join("stream.png");
                if matches!(
                    self.style,
                    QrRenderStyle::SakuraWind | QrRenderStyle::SakuraStorm
                ) {
                    let frames = render_stylized_animation(&rendered, self.dimension, self.style);
                    write_apng_rgba(&path, &frames, self.fps)?;
                } else {
                    let frames = render_animation(&rendered, self.dimension)?;
                    write_apng(&path, &frames, self.fps, self.style)?;
                }
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
        manifest_map.insert("chunk_size".to_string(), Value::from(self.chunk_size as u64));
        manifest_map.insert(
            "parity_group".to_string(),
            Value::from(self.parity_group as u64),
        );
        manifest_map.insert(
            "frame_encoding".to_string(),
            Value::from(self.frame_encoding.label()),
        );
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
            eyre!("failed to write manifest {}: {err}", manifest_path.display())
        })?;

        Ok(())
    }
}

#[derive(clap::Args, Debug)]
pub struct QrDecodeArgs {
    /// Directory containing raw frame bytes.
    #[arg(long, value_name = "DIR")]
    pub input_dir: PathBuf,
    /// Output file for the decoded payload.
    #[arg(long, value_name = "FILE")]
    pub output: PathBuf,
    /// Frame encoding used in the input.
    #[arg(long, value_enum, default_value = "binary")]
    pub frame_encoding: QrFrameEncoding,
    /// Optional JSON manifest output path.
    #[arg(long, value_name = "FILE")]
    pub output_manifest: Option<PathBuf>,
}

impl QrDecodeArgs {
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
                    .map(|ext| ext.eq_ignore_ascii_case("bin") || ext.eq_ignore_ascii_case("txt"))
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());

        let mut assembler = QrStreamAssembler::default();
        let mut envelope: Option<QrStreamEnvelope> = None;
        let mut result: Option<QrStreamDecodeResult> = None;
        for entry in entries {
            let bytes = fs::read(entry.path())
                .map_err(|err| eyre!("failed to read {}: {err}", entry.path().display()))?;
            let decoded = decode_frame_bytes(&bytes, self.frame_encoding)?;
            if envelope.is_none() {
                if let Ok(frame) = QrStreamFrame::decode(&decoded) {
                    if frame.kind == QrStreamFrameKind::Header {
                        if let Ok(header) = QrStreamEnvelope::decode(&frame.payload) {
                            envelope = Some(header);
                        }
                    }
                }
            }
            let step = assembler
                .ingest_bytes(&decoded)
                .map_err(|err| eyre!("failed to decode frame: {err}"))?;
            if step.is_complete() {
                result = Some(step);
            }
        }
        let result = result.ok_or_else(|| eyre!("missing frames; payload not complete"))?;
        let payload = result.payload.clone().ok_or_else(|| eyre!("missing payload"))?;
        fs::write(&self.output, &payload).map_err(|err| {
            eyre!("failed to write payload {}: {err}", self.output.display())
        })?;
        if let Some(path) = self.output_manifest {
            let mut manifest_map = Map::new();
            manifest_map.insert("version".to_string(), Value::from(1u64));
            manifest_map.insert(
                "frame_encoding".to_string(),
                Value::from(self.frame_encoding.label()),
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
pub enum QrOutputFormat {
    Frames,
    Svg,
    Png,
    Gif,
    Apng,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrRenderStyle {
    Mono,
    Sakura,
    SakuraWind,
    SakuraStorm,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum QrPayloadKindArg {
    Unspecified,
    OfflineToOnlineTransfer,
    OfflineSpendReceipt,
    OfflineEnvelope,
}

impl QrPayloadKindArg {
    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::OfflineToOnlineTransfer => "offline_to_online_transfer",
            Self::OfflineSpendReceipt => "offline_spend_receipt",
            Self::OfflineEnvelope => "offline_envelope",
        }
    }
}

impl From<QrPayloadKindArg> for QrPayloadKind {
    fn from(value: QrPayloadKindArg) -> Self {
        match value {
            QrPayloadKindArg::OfflineToOnlineTransfer => QrPayloadKind::OfflineToOnlineTransfer,
            QrPayloadKindArg::OfflineSpendReceipt => QrPayloadKind::OfflineSpendReceipt,
            QrPayloadKindArg::OfflineEnvelope => QrPayloadKind::OfflineEnvelope,
            QrPayloadKindArg::Unspecified => QrPayloadKind::Unspecified,
        }
    }
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum QrFrameEncoding {
    Binary,
    Base64,
}

impl QrFrameEncoding {
    fn label(&self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::Base64 => "base64",
        }
    }
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum QrErrorCorrection {
    L,
    M,
    Q,
    H,
}

impl From<QrErrorCorrection> for EcLevel {
    fn from(value: QrErrorCorrection) -> Self {
        match value {
            QrErrorCorrection::L => EcLevel::L,
            QrErrorCorrection::M => EcLevel::M,
            QrErrorCorrection::Q => EcLevel::Q,
            QrErrorCorrection::H => EcLevel::H,
        }
    }
}

struct RenderedFrame {
    index: u32,
    code: QrCode,
}

struct GrayImage {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

const SAKURA_BG_START: [f64; 3] = [0.98, 0.94, 0.96];
const SAKURA_BG_END: [f64; 3] = [1.0, 0.98, 0.99];
const SAKURA_PETAL: [f64; 3] = [0.98, 0.80, 0.86];
const SAKURA_INK: [f64; 3] = [0.34, 0.15, 0.20];
const SAKURA_PETAL_COUNT: usize = 6;
const SAKURA_PETAL_ORBIT: f64 = 0.28;
const SAKURA_PETAL_RADIUS: f64 = 0.11;
const SAKURA_PETAL_ALPHA: f64 = 0.12;
const SAKURA_PETAL_DRIFT: f64 = 0.035;
const SAKURA_SVG_INK: &str = "#572633";
const SAKURA_SVG_BG: &str = "#FCF5F9";
const SAKURA_WIND_PETAL_COUNT: usize = 9;
const SAKURA_WIND_PETAL_ORBIT: f64 = 0.32;
const SAKURA_WIND_PETAL_RADIUS: f64 = 0.14;
const SAKURA_WIND_PETAL_ALPHA: f64 = 0.18;
const SAKURA_WIND_PETAL_DRIFT: f64 = 0.06;
const SAKURA_WIND_STREAK_ALPHA: f64 = 0.04;
const SAKURA_WIND_STREAK_FREQ_X: f64 = 6.0;
const SAKURA_WIND_STREAK_FREQ_Y: f64 = 3.5;
const SAKURA_WIND_PETAL_MAJOR: f64 = 0.46;
const SAKURA_WIND_PETAL_MINOR: f64 = 0.24;
const SAKURA_WIND_PETAL_SECONDARY_MAJOR: f64 = 0.34;
const SAKURA_WIND_PETAL_SECONDARY_MINOR: f64 = 0.18;
const SAKURA_WIND_CORE_RADIUS: f64 = 0.18;
const SAKURA_WIND_MODULE_DRIFT: f64 = 0.12;
const SAKURA_STORM_SVG_ON: &str = "#F6EAF6";
const SAKURA_STORM_SVG_OFF: &str = "#07040D";
const SAKURA_STORM_BG_START: [f64; 3] = [0.05, 0.02, 0.08];
const SAKURA_STORM_BG_END: [f64; 3] = [0.02, 0.01, 0.04];
const SAKURA_STORM_LIGHT_MODULE: [f64; 3] = [0.09, 0.05, 0.12];
const SAKURA_STORM_DARK_MODULE: [f64; 3] = [0.26, 0.16, 0.30];
const SAKURA_STORM_DARK_CORE: [f64; 3] = [0.07, 0.03, 0.10];
const SAKURA_STORM_FUNCTIONAL_CORE: [f64; 3] = [0.96, 0.89, 0.96];
const SAKURA_STORM_GLYPH: [f64; 3] = [0.98, 0.92, 0.97];
const SAKURA_STORM_RING_BRIGHT: [f64; 3] = [0.95, 0.71, 0.87];
const SAKURA_STORM_RING_DIM: [f64; 3] = [0.42, 0.24, 0.37];
const SAKURA_STORM_QR_SCALE: f64 = 0.63;
const SAKURA_STORM_RING_RADII: [f64; 3] = [0.315, 0.372, 0.429];
const SAKURA_STORM_RING_BAND: f64 = 0.0048;
const SAKURA_STORM_RING_DOTS: [f64; 3] = [64.0, 76.0, 90.0];
const SAKURA_STORM_RING_SPEED: [f64; 3] = [1.0, 1.2, 1.45];
const SAKURA_STORM_RING_THRESHOLD: f64 = 0.24;
const SAKURA_STORM_LIGHT_VIGNETTE: f64 = 0.28;
const SAKURA_STORM_SCANLINE: f64 = 0.018;
const SAKURA_STORM_GLYPH_PADDING_RATIO: f64 = 0.17;
const SAKURA_STORM_CORE_RADIUS: f64 = 0.17;
const SAKURA_STORM_GLYPHS: [[u8; 8]; 16] = [
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

fn render_svg(code: &QrCode, dimension: u32, style: QrRenderStyle) -> Result<String> {
    let mut renderer = code.render::<svg::Color>();
    match style {
        QrRenderStyle::Sakura | QrRenderStyle::SakuraWind => {
            renderer.dark_color(svg::Color(SAKURA_SVG_INK));
            renderer.light_color(svg::Color(SAKURA_SVG_BG));
        }
        QrRenderStyle::SakuraStorm => {
            renderer.dark_color(svg::Color(SAKURA_STORM_SVG_ON));
            renderer.light_color(svg::Color(SAKURA_STORM_SVG_OFF));
        }
        QrRenderStyle::Mono => {}
    }
    let svg = renderer
        .min_dimensions(dimension, dimension)
        .max_dimensions(dimension, dimension)
        .build();
    let trimmed = svg.trim_start();
    let svg = if trimmed.starts_with("<?xml") {
        trimmed
            .find("?>")
            .map(|idx| trimmed[idx + 2..].trim_start().to_owned())
            .unwrap_or_else(|| trimmed.to_owned())
    } else {
        trimmed.to_owned()
    };
    Ok(svg)
}

fn render_image(code: &QrCode, dimension: u32) -> Result<GrayImage> {
    let image = code
        .render::<image::Luma<u8>>()
        .min_dimensions(dimension, dimension)
        .max_dimensions(dimension, dimension)
        .build();
    let (width, height) = image.dimensions();
    Ok(GrayImage {
        width,
        height,
        data: image.into_raw(),
    })
}

fn render_sakura_rgba(
    frame: &GrayImage,
    frame_index: u32,
    frame_count: u32,
) -> image::RgbaImage {
    let frame_count = frame_count.max(1);
    let phase = (frame_index % frame_count) as f64 / frame_count as f64;
    let angle = phase * std::f64::consts::TAU;
    let dir_x = angle.cos();
    let dir_y = angle.sin();
    let drift = (phase * std::f64::consts::TAU).sin() * SAKURA_PETAL_DRIFT;
    let norm = |idx: u32, max: u32| -> f64 {
        if max <= 1 {
            0.5
        } else {
            idx as f64 / (max - 1) as f64
        }
    };
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let to_u8 = |value: f64| -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 };

    let pixel_count = frame.width as usize * frame.height as usize;
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    for y in 0..frame.height {
        let ny = norm(y, frame.height);
        let cy = ny - 0.5;
        for x in 0..frame.width {
            let nx = norm(x, frame.width);
            let cx = nx - 0.5;
            let proj = cx * dir_x + cy * dir_y;
            let t = (proj + 0.5).clamp(0.0, 1.0);
            let mut r = lerp(SAKURA_BG_START[0], SAKURA_BG_END[0], t);
            let mut g = lerp(SAKURA_BG_START[1], SAKURA_BG_END[1], t);
            let mut b = lerp(SAKURA_BG_START[2], SAKURA_BG_END[2], t);

            if SAKURA_PETAL_ALPHA > 0.0 {
                let mut petal_alpha = 0.0;
                for petal_index in 0..SAKURA_PETAL_COUNT {
                    let petal_phase =
                        (petal_index as f64 / SAKURA_PETAL_COUNT as f64) + phase;
                    let petal_angle = petal_phase * std::f64::consts::TAU;
                    let cx = 0.5 + petal_angle.cos() * SAKURA_PETAL_ORBIT;
                    let cy = 0.5 + petal_angle.sin() * SAKURA_PETAL_ORBIT + drift;
                    let dx = nx - cx;
                    let dy = ny - cy;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < SAKURA_PETAL_RADIUS {
                        let alpha =
                            (1.0 - dist / SAKURA_PETAL_RADIUS) * SAKURA_PETAL_ALPHA;
                        if alpha > petal_alpha {
                            petal_alpha = alpha;
                        }
                    }
                }
                if petal_alpha > 0.0 {
                    r = lerp(r, SAKURA_PETAL[0], petal_alpha);
                    g = lerp(g, SAKURA_PETAL[1], petal_alpha);
                    b = lerp(b, SAKURA_PETAL[2], petal_alpha);
                }
            }

            let gray = frame.data[(y * frame.width + x) as usize];
            if gray < 128 {
                r = SAKURA_INK[0];
                g = SAKURA_INK[1];
                b = SAKURA_INK[2];
            }
            rgba.extend_from_slice(&[to_u8(r), to_u8(g), to_u8(b), 0xFF]);
        }
    }

    image::RgbaImage::from_raw(frame.width, frame.height, rgba)
        .expect("rgba buffer size")
}

fn render_sakura_wind_rgba(
    code: &QrCode,
    dimension: u32,
    frame_index: u32,
    frame_count: u32,
) -> image::RgbaImage {
    // Keep functional modules solid while stylizing data modules as petals.
    let modules = code.width() as u32;
    let quiet_zone = if code.version().is_micro() { 2 } else { 4 };
    let total_modules = modules + quiet_zone * 2;
    let module_size = (dimension / total_modules).max(1);
    let width = total_modules * module_size;
    let height = width;
    let module_size_f = module_size as f64;
    let colors = code.to_colors();
    let mut module_dark = vec![false; (modules * modules) as usize];
    let mut module_functional = vec![false; (modules * modules) as usize];
    for y in 0..modules {
        for x in 0..modules {
            let idx = (y * modules + x) as usize;
            module_dark[idx] = colors[idx] != qrcode::types::Color::Light;
            module_functional[idx] = code.is_functional(x as usize, y as usize);
        }
    }

    let frame_count = frame_count.max(1);
    let phase = (frame_index % frame_count) as f64 / frame_count as f64;
    let wind_angle = phase * std::f64::consts::TAU;
    let wind_dir_x = wind_angle.cos();
    let wind_dir_y = wind_angle.sin();
    let norm = |idx: u32, max: u32| -> f64 {
        if max <= 1 {
            0.5
        } else {
            idx as f64 / (max - 1) as f64
        }
    };
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let to_u8 = |value: f64| -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 };
    let petal_mask = |dx: f64, dy: f64, angle: f64, major: f64, minor: f64| -> bool {
        let (sin_a, cos_a) = angle.sin_cos();
        let rx = dx * cos_a + dy * sin_a;
        let ry = -dx * sin_a + dy * cos_a;
        let nx = rx / major;
        let ny = ry / minor;
        nx * nx + ny * ny <= 1.0
    };
    let core_radius_sq = SAKURA_WIND_CORE_RADIUS * SAKURA_WIND_CORE_RADIUS;
    let pixel_count = width as usize * height as usize;
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    for y in 0..height {
        let ny = norm(y, height);
        let cy = ny - 0.5;
        let module_y = y / module_size;
        for x in 0..width {
            let nx = norm(x, width);
            let cx = nx - 0.5;
            let proj = cx * wind_dir_x + cy * wind_dir_y;
            let t = (proj + 0.5).clamp(0.0, 1.0);
            let mut r = lerp(SAKURA_BG_START[0], SAKURA_BG_END[0], t);
            let mut g = lerp(SAKURA_BG_START[1], SAKURA_BG_END[1], t);
            let mut b = lerp(SAKURA_BG_START[2], SAKURA_BG_END[2], t);

            if SAKURA_WIND_STREAK_ALPHA > 0.0 {
                let streak = (nx * SAKURA_WIND_STREAK_FREQ_X
                    + ny * SAKURA_WIND_STREAK_FREQ_Y
                    + wind_angle)
                    .sin();
                let streak = (streak * 0.5 + 0.5) * SAKURA_WIND_STREAK_ALPHA;
                r = (r + streak).min(1.0);
                g = (g + streak).min(1.0);
                b = (b + streak).min(1.0);
            }

            if SAKURA_WIND_PETAL_ALPHA > 0.0 {
                let drift = (phase * std::f64::consts::TAU).sin() * SAKURA_WIND_PETAL_DRIFT;
                let mut petal_alpha = 0.0;
                for petal_index in 0..SAKURA_WIND_PETAL_COUNT {
                    let petal_phase =
                        (petal_index as f64 / SAKURA_WIND_PETAL_COUNT as f64) + phase;
                    let petal_angle = petal_phase * std::f64::consts::TAU;
                    let cx = 0.5 + petal_angle.cos() * SAKURA_WIND_PETAL_ORBIT + wind_dir_x * drift;
                    let cy = 0.5 + petal_angle.sin() * SAKURA_WIND_PETAL_ORBIT + wind_dir_y * drift;
                    let dx = nx - cx;
                    let dy = ny - cy;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < SAKURA_WIND_PETAL_RADIUS {
                        let alpha =
                            (1.0 - dist / SAKURA_WIND_PETAL_RADIUS) * SAKURA_WIND_PETAL_ALPHA;
                        if alpha > petal_alpha {
                            petal_alpha = alpha;
                        }
                    }
                }
                if petal_alpha > 0.0 {
                    r = lerp(r, SAKURA_PETAL[0], petal_alpha);
                    g = lerp(g, SAKURA_PETAL[1], petal_alpha);
                    b = lerp(b, SAKURA_PETAL[2], petal_alpha);
                }
            }

            let module_x = x / module_size;
            if module_x >= quiet_zone
                && module_x < quiet_zone + modules
                && module_y >= quiet_zone
                && module_y < quiet_zone + modules
            {
                let mx = module_x - quiet_zone;
                let my = module_y - quiet_zone;
                let idx = (my * modules + mx) as usize;
                if module_dark[idx] {
                    let is_functional = module_functional[idx];
                    if is_functional || module_size <= 4 {
                        r = SAKURA_INK[0];
                        g = SAKURA_INK[1];
                        b = SAKURA_INK[2];
                    } else {
                        let local_x = (x % module_size) as f64 / module_size_f - 0.5;
                        let local_y = (y % module_size) as f64 / module_size_f - 0.5;
                        let gust = ((mx + my) as f64 * 0.35 + wind_angle).sin();
                        let offset = gust * SAKURA_WIND_MODULE_DRIFT;
                        let dx = local_x - wind_dir_x * offset;
                        let dy = local_y - wind_dir_y * offset;
                        let angle = wind_angle + gust * 0.6;
                        let core = dx * dx + dy * dy <= core_radius_sq;
                        let petal =
                            petal_mask(dx, dy, angle, SAKURA_WIND_PETAL_MAJOR, SAKURA_WIND_PETAL_MINOR)
                                || petal_mask(
                                    dx,
                                    dy,
                                    angle + 1.1,
                                    SAKURA_WIND_PETAL_SECONDARY_MAJOR,
                                    SAKURA_WIND_PETAL_SECONDARY_MINOR,
                                )
                                || core;
                        if petal {
                            r = SAKURA_INK[0];
                            g = SAKURA_INK[1];
                            b = SAKURA_INK[2];
                        }
                    }
                }
            }

            rgba.extend_from_slice(&[to_u8(r), to_u8(g), to_u8(b), 0xFF]);
        }
    }

    image::RgbaImage::from_raw(width, height, rgba).expect("rgba buffer size")
}

fn sakura_storm_logo_hole(mx: u32, my: u32, modules: u32) -> bool {
    if modules == 0 {
        return false;
    }
    let nx = ((mx as f64 + 0.5) / modules as f64) * 2.0 - 1.0;
    let ny = ((my as f64 + 0.5) / modules as f64) * 2.0 - 1.0;
    let x = nx * 1.05;
    let y = ny * 1.05;
    let outer = x * x + y * y <= 0.86 * 0.86;
    if !outer {
        return false;
    }
    let top_lobe = (x + 0.06).powi(2) + (y + 0.34).powi(2) <= 0.48 * 0.48;
    let bottom_lobe = (x - 0.06).powi(2) + (y - 0.34).powi(2) <= 0.48 * 0.48;
    let bridge = x.abs() <= 0.20 && y.abs() <= 0.14;
    let body = top_lobe || bottom_lobe || bridge;
    let carve_top = (x - 0.24).powi(2) + (y + 0.34).powi(2) <= 0.27 * 0.27;
    let carve_bottom = (x + 0.24).powi(2) + (y - 0.34).powi(2) <= 0.27 * 0.27;
    body && !(carve_top || carve_bottom)
}

fn sakura_storm_glyph_index(mx: u32, my: u32, frame_index: u32, frame_count: u32) -> usize {
    let phase = frame_index.wrapping_mul(17) ^ frame_count.wrapping_mul(31);
    let mut v = mx.wrapping_mul(0x9E37_79B1)
        ^ my.wrapping_mul(0x85EB_CA77)
        ^ phase.wrapping_mul(0xC2B2_AE3D);
    v ^= v >> 16;
    v = v.wrapping_mul(0x7FEB_352D);
    v ^= v >> 15;
    (v as usize) % SAKURA_STORM_GLYPHS.len()
}

fn sakura_storm_glyph_hit(glyph_index: usize, local_x: u32, local_y: u32, module_size: u32) -> bool {
    if module_size < 6 {
        return false;
    }
    let max_pad = (module_size / 3).max(1);
    let mut padding = (module_size as f64 * SAKURA_STORM_GLYPH_PADDING_RATIO).round() as u32;
    padding = padding.clamp(1, max_pad);
    let inner = module_size.saturating_sub(padding * 2);
    if inner < 3 || local_x < padding || local_y < padding {
        return false;
    }
    if local_x >= module_size - padding || local_y >= module_size - padding {
        return false;
    }
    let gx = (((local_x - padding) * 8) / inner).min(7) as usize;
    let gy = (((local_y - padding) * 8) / inner).min(7) as usize;
    ((SAKURA_STORM_GLYPHS[glyph_index][gy] >> (7 - gx)) & 1) == 1
}

fn render_sakura_storm_rgba(
    code: &QrCode,
    dimension: u32,
    frame_index: u32,
    frame_count: u32,
) -> image::RgbaImage {
    let modules = code.width() as u32;
    let quiet_zone = if code.version().is_micro() { 2 } else { 4 };
    let total_modules = modules + quiet_zone * 2;
    let target_qr = ((dimension as f64) * SAKURA_STORM_QR_SCALE).round() as u32;
    let module_size = (target_qr / total_modules).max(1);
    let qr_size = total_modules * module_size;
    let side = dimension.max(qr_size);
    let qr_offset = (side - qr_size) / 2;
    let module_size_f = module_size as f64;
    let frame_count = frame_count.max(1);
    let phase = (frame_index % frame_count) as f64 / frame_count as f64;
    let tau = std::f64::consts::TAU;
    let core_radius_sq = SAKURA_STORM_CORE_RADIUS * SAKURA_STORM_CORE_RADIUS;
    let colors = code.to_colors();
    let mut module_dark = vec![false; (modules * modules) as usize];
    let mut module_functional = vec![false; (modules * modules) as usize];
    for y in 0..modules {
        for x in 0..modules {
            let idx = (y * modules + x) as usize;
            module_dark[idx] = colors[idx] != qrcode::types::Color::Light;
            module_functional[idx] = code.is_functional(x as usize, y as usize);
        }
    }

    let norm = |idx: u32, max: u32| -> f64 {
        if max <= 1 {
            0.5
        } else {
            idx as f64 / (max - 1) as f64
        }
    };
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let to_u8 = |value: f64| -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 };

    let pixel_count = side as usize * side as usize;
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    for y in 0..side {
        let ny = norm(y, side);
        let cy = ny - 0.5;
        for x in 0..side {
            let nx = norm(x, side);
            let cx = nx - 0.5;
            let radial = (cx * cx + cy * cy).sqrt();
            let depth = (0.67 * ny + 0.33 * nx).clamp(0.0, 1.0);
            let v = (radial / 0.72).clamp(0.0, 1.0);
            let t = (depth * (1.0 - v * 0.35)).clamp(0.0, 1.0);
            let mut r = lerp(SAKURA_STORM_BG_START[0], SAKURA_STORM_BG_END[0], t);
            let mut g = lerp(SAKURA_STORM_BG_START[1], SAKURA_STORM_BG_END[1], t);
            let mut b = lerp(SAKURA_STORM_BG_START[2], SAKURA_STORM_BG_END[2], t);
            let scan = ((y as f64 * 0.11 + phase * tau).sin() * 0.5 + 0.5)
                * SAKURA_STORM_SCANLINE
                * (1.0 - v * 0.8);
            r = (r + scan).clamp(0.0, 1.0);
            g = (g + scan).clamp(0.0, 1.0);
            b = (b + scan).clamp(0.0, 1.0);

            let angle = cy.atan2(cx);
            for ring_idx in 0..SAKURA_STORM_RING_RADII.len() {
                let dr = ((radial - SAKURA_STORM_RING_RADII[ring_idx]).abs())
                    / SAKURA_STORM_RING_BAND;
                if dr > 1.6 {
                    continue;
                }
                let dot_wave = (angle * SAKURA_STORM_RING_DOTS[ring_idx]
                    + phase * tau * SAKURA_STORM_RING_SPEED[ring_idx]
                    + ring_idx as f64 * 0.73)
                    .sin();
                if dot_wave <= SAKURA_STORM_RING_THRESHOLD {
                    continue;
                }
                let alpha = ((1.0 - dr / 1.6).clamp(0.0, 1.0))
                    * (0.45 + 0.55 * dot_wave.clamp(0.0, 1.0));
                let cardinal = ((angle * 2.0).cos().abs()).powf(18.0);
                let ring_color = if cardinal > 0.66 {
                    SAKURA_STORM_RING_BRIGHT
                } else {
                    SAKURA_STORM_RING_DIM
                };
                r = lerp(r, ring_color[0], alpha);
                g = lerp(g, ring_color[1], alpha);
                b = lerp(b, ring_color[2], alpha);
            }

            if x >= qr_offset && x < qr_offset + qr_size && y >= qr_offset && y < qr_offset + qr_size {
                let rel_x = x - qr_offset;
                let rel_y = y - qr_offset;
                let module_x = rel_x / module_size;
                let module_y = rel_y / module_size;
                if module_x >= quiet_zone
                    && module_x < quiet_zone + modules
                    && module_y >= quiet_zone
                    && module_y < quiet_zone + modules
                {
                    let mx = module_x - quiet_zone;
                    let my = module_y - quiet_zone;
                    let idx = (my * modules + mx) as usize;
                    if module_dark[idx] {
                        let local_x = rel_x % module_size;
                        let local_y = rel_y % module_size;
                        if !module_functional[idx] && sakura_storm_logo_hole(mx, my, modules) {
                            // Keep the logo region as negative space for the storm silhouette.
                        } else if module_functional[idx] {
                            r = SAKURA_STORM_FUNCTIONAL_CORE[0];
                            g = SAKURA_STORM_FUNCTIONAL_CORE[1];
                            b = SAKURA_STORM_FUNCTIONAL_CORE[2];
                            let lx = local_x as f64 / module_size_f;
                            let ly = local_y as f64 / module_size_f;
                            let edge = lx.min(1.0 - lx).min(ly).min(1.0 - ly);
                            if edge < 0.18 {
                                r = lerp(r, SAKURA_STORM_DARK_MODULE[0], 0.74);
                                g = lerp(g, SAKURA_STORM_DARK_MODULE[1], 0.74);
                                b = lerp(b, SAKURA_STORM_DARK_MODULE[2], 0.74);
                            }
                        } else {
                            r = SAKURA_STORM_DARK_MODULE[0];
                            g = SAKURA_STORM_DARK_MODULE[1];
                            b = SAKURA_STORM_DARK_MODULE[2];
                            let glyph_idx = sakura_storm_glyph_index(mx, my, frame_index, frame_count);
                            if sakura_storm_glyph_hit(glyph_idx, local_x, local_y, module_size) {
                                r = SAKURA_STORM_GLYPH[0];
                                g = SAKURA_STORM_GLYPH[1];
                                b = SAKURA_STORM_GLYPH[2];
                            }
                            let lx = local_x as f64 / module_size_f - 0.5;
                            let ly = local_y as f64 / module_size_f - 0.5;
                            if lx * lx + ly * ly <= core_radius_sq {
                                r = SAKURA_STORM_DARK_CORE[0];
                                g = SAKURA_STORM_DARK_CORE[1];
                                b = SAKURA_STORM_DARK_CORE[2];
                            }
                        }
                    } else {
                        r = lerp(r, SAKURA_STORM_LIGHT_MODULE[0], SAKURA_STORM_LIGHT_VIGNETTE);
                        g = lerp(g, SAKURA_STORM_LIGHT_MODULE[1], SAKURA_STORM_LIGHT_VIGNETTE);
                        b = lerp(b, SAKURA_STORM_LIGHT_MODULE[2], SAKURA_STORM_LIGHT_VIGNETTE);
                    }
                }
            }

            rgba.extend_from_slice(&[to_u8(r), to_u8(g), to_u8(b), 0xFF]);
        }
    }

    image::RgbaImage::from_raw(side, side, rgba).expect("rgba buffer size")
}

fn render_stylized_rgba(
    code: &QrCode,
    dimension: u32,
    frame_index: u32,
    frame_count: u32,
    style: QrRenderStyle,
) -> image::RgbaImage {
    match style {
        QrRenderStyle::SakuraWind => {
            render_sakura_wind_rgba(code, dimension, frame_index, frame_count)
        }
        QrRenderStyle::SakuraStorm => {
            render_sakura_storm_rgba(code, dimension, frame_index, frame_count)
        }
        QrRenderStyle::Mono | QrRenderStyle::Sakura => {
            unreachable!("RGBA-only render helper only supports stylized themes")
        }
    }
}

fn render_stylized_animation(
    frames: &[RenderedFrame],
    dimension: u32,
    style: QrRenderStyle,
) -> Vec<image::RgbaImage> {
    let frame_count = frames.len().max(1) as u32;
    let mut out = Vec::with_capacity(frames.len());
    for frame in frames {
        out.push(render_stylized_rgba(
            &frame.code,
            dimension,
            frame.index,
            frame_count,
            style,
        ));
    }
    out
}

fn render_animation(frames: &[RenderedFrame], dimension: u32) -> Result<Vec<GrayImage>> {
    let mut out = Vec::with_capacity(frames.len());
    for frame in frames {
        out.push(render_image(&frame.code, dimension)?);
    }
    Ok(out)
}

fn write_png(
    path: &Path,
    image: &GrayImage,
    style: QrRenderStyle,
    frame_index: u32,
    frame_count: u32,
) -> Result<()> {
    if matches!(style, QrRenderStyle::SakuraWind | QrRenderStyle::SakuraStorm) {
        return Err(eyre!(
            "sakura-wind/sakura-storm render requires the RGBA pipeline"
        ));
    }
    let file = fs::File::create(path)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, image.width, image.height);
    match style {
        QrRenderStyle::Mono => {
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&image.data)?;
        }
        QrRenderStyle::Sakura => {
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            let rgba = render_sakura_rgba(image, frame_index, frame_count);
            writer.write_image_data(rgba.as_raw())?;
        }
        QrRenderStyle::SakuraWind | QrRenderStyle::SakuraStorm => {
            unreachable!("RGBA-only guard should have returned")
        }
    }
    Ok(())
}

fn write_apng(
    path: &Path,
    frames: &[GrayImage],
    fps: u16,
    style: QrRenderStyle,
) -> Result<()> {
    if frames.is_empty() {
        return Err(eyre!("no frames to encode"));
    }
    if matches!(style, QrRenderStyle::SakuraWind | QrRenderStyle::SakuraStorm) {
        return Err(eyre!(
            "sakura-wind/sakura-storm render requires the RGBA pipeline"
        ));
    }
    let file = fs::File::create(path)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, frames[0].width, frames[0].height);
    match style {
        QrRenderStyle::Mono => encoder.set_color(png::ColorType::Grayscale),
        QrRenderStyle::Sakura => encoder.set_color(png::ColorType::Rgba),
        QrRenderStyle::SakuraWind | QrRenderStyle::SakuraStorm => {
            unreachable!("RGBA-only guard should have returned")
        }
    }
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_animated(frames.len() as u32, 0)?;
    encoder.set_sep_def_img(false)?;
    let mut writer = encoder.write_header()?;
    let delay = frame_delay_ms(fps);
    let delay_den = 1000u16;
    for (idx, frame) in frames.iter().enumerate() {
        writer.set_frame_delay(delay as u16, delay_den)?;
        match style {
            QrRenderStyle::Mono => writer.write_image_data(&frame.data)?,
            QrRenderStyle::Sakura => {
                let rgba = render_sakura_rgba(frame, idx as u32, frames.len() as u32);
                writer.write_image_data(rgba.as_raw())?;
            }
            QrRenderStyle::SakuraWind | QrRenderStyle::SakuraStorm => {
                unreachable!("RGBA-only guard should have returned")
            }
        }
    }
    Ok(())
}

fn write_gif(
    path: &Path,
    frames: &[GrayImage],
    fps: u16,
    style: QrRenderStyle,
) -> Result<()> {
    if frames.is_empty() {
        return Err(eyre!("no frames to encode"));
    }
    if matches!(style, QrRenderStyle::SakuraWind | QrRenderStyle::SakuraStorm) {
        return Err(eyre!(
            "sakura-wind/sakura-storm render requires the RGBA pipeline"
        ));
    }
    let file = fs::File::create(path)?;
    let mut encoder = image::codecs::gif::GifEncoder::new(file);
    let delay_ms = frame_delay_ms(fps) as u32;
    let delay = image::Delay::from_numer_denom_ms(delay_ms.max(1), 1);
    let frame_count = frames.len() as u32;
    let mut gif_frames = Vec::with_capacity(frames.len());
    for (idx, frame) in frames.iter().enumerate() {
        let rgba = match style {
            QrRenderStyle::Mono => grayscale_to_rgba(frame),
            QrRenderStyle::Sakura => render_sakura_rgba(frame, idx as u32, frame_count),
            QrRenderStyle::SakuraWind | QrRenderStyle::SakuraStorm => {
                unreachable!("RGBA-only guard should have returned")
            }
        };
        gif_frames.push(image::Frame::from_parts(rgba, 0, 0, delay));
    }
    encoder.encode_frames(gif_frames.into_iter())?;
    Ok(())
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

fn grayscale_to_rgba(frame: &GrayImage) -> image::RgbaImage {
    let mut rgba = Vec::with_capacity((frame.width * frame.height * 4) as usize);
    for gray in &frame.data {
        rgba.extend_from_slice(&[*gray, *gray, *gray, 0xFF]);
    }
    image::RgbaImage::from_raw(frame.width, frame.height, rgba)
        .expect("rgba buffer size")
}

fn frame_delay_ms(fps: u16) -> u16 {
    let fps = fps.max(1) as u32;
    ((1000 + fps / 2) / fps).min(u16::MAX as u32) as u16
}

fn estimate_payload_bytes_per_second(payload_bytes: usize, frame_count: usize, fps: u16) -> u64 {
    if frame_count == 0 {
        return 0;
    }
    let fps = fps.max(1) as f64;
    ((payload_bytes as f64 * fps) / frame_count as f64).round() as u64
}

const QR_TEXT_PREFIX: &str = "iroha:qr1:";

fn encode_frame_bytes(bytes: &[u8], encoding: QrFrameEncoding) -> Result<Vec<u8>> {
    match encoding {
        QrFrameEncoding::Binary => Ok(bytes.to_vec()),
        QrFrameEncoding::Base64 => {
            let encoded = BASE64_STANDARD.encode(bytes);
            Ok(format!("{QR_TEXT_PREFIX}{encoded}").into_bytes())
        }
    }
}

fn decode_frame_bytes(bytes: &[u8], encoding: QrFrameEncoding) -> Result<Vec<u8>> {
    match encoding {
        QrFrameEncoding::Binary => Ok(bytes.to_vec()),
        QrFrameEncoding::Base64 => {
            let text = std::str::from_utf8(bytes)
                .map_err(|_| eyre!("frame payload is not valid UTF-8"))?;
            let text = text.trim();
            let payload = text
                .strip_prefix(QR_TEXT_PREFIX)
                .ok_or_else(|| eyre!("missing qr text prefix"))?;
            BASE64_STANDARD
                .decode(payload.as_bytes())
                .map_err(|err| eyre!("invalid base64 payload: {err}"))
        }
    }
}

fn frame_kind_label(kind: QrStreamFrameKind) -> &'static str {
    match kind {
        QrStreamFrameKind::Header => "header",
        QrStreamFrameKind::Data => "data",
        QrStreamFrameKind::Parity => "parity",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn qr_text_roundtrip() {
        let payload = b"frame-bytes";
        let encoded = encode_frame_bytes(payload, QrFrameEncoding::Base64).expect("encode");
        let decoded = decode_frame_bytes(&encoded, QrFrameEncoding::Base64).expect("decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn qr_payload_kind_labels() {
        let cases = [
            (QrPayloadKindArg::Unspecified, "unspecified"),
            (
                QrPayloadKindArg::OfflineToOnlineTransfer,
                "offline_to_online_transfer",
            ),
            (QrPayloadKindArg::OfflineSpendReceipt, "offline_spend_receipt"),
            (QrPayloadKindArg::OfflineEnvelope, "offline_envelope"),
        ];

        for (kind, expected) in cases {
            assert_eq!(kind.label(), expected);
        }
    }

    #[test]
    fn qr_frame_decode_matches_encoder() {
        let payload = b"fixture-payload";
        let (_envelope, frames) = QrStreamEncoder::encode_frames(
            payload,
            QrStreamOptions {
                chunk_size: 16,
                parity_group: 0,
                payload_kind: QrPayloadKind::Unspecified,
                ..QrStreamOptions::default()
            },
        )
        .expect("encode");
        let mut assembler = QrStreamAssembler::default();
        let mut last = None;
        for frame in frames {
            last = Some(assembler.ingest_frame(frame).expect("frame"));
        }
        let result = last.expect("result");
        assert!(result.is_complete());
        assert_eq!(result.received_chunks, result.total_chunks);
    }

    #[test]
    fn sakura_rgba_uses_ink_for_dark_pixels() {
        let image = GrayImage {
            width: 1,
            height: 1,
            data: vec![0],
        };
        let rgba = render_sakura_rgba(&image, 0, 1);
        let pixel = rgba.get_pixel(0, 0).0;
        let to_u8 = |value: f64| -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 };
        let expected = [
            to_u8(SAKURA_INK[0]),
            to_u8(SAKURA_INK[1]),
            to_u8(SAKURA_INK[2]),
            0xFF,
        ];
        assert_eq!(pixel, expected);
    }

    #[test]
    fn sakura_rgba_uses_gradient_for_light_pixels() {
        let image = GrayImage {
            width: 1,
            height: 1,
            data: vec![255],
        };
        let rgba = render_sakura_rgba(&image, 0, 1);
        let pixel = rgba.get_pixel(0, 0).0;
        let to_u8 = |value: f64| -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 };
        let expected = [
            to_u8((SAKURA_BG_START[0] + SAKURA_BG_END[0]) * 0.5),
            to_u8((SAKURA_BG_START[1] + SAKURA_BG_END[1]) * 0.5),
            to_u8((SAKURA_BG_START[2] + SAKURA_BG_END[2]) * 0.5),
            0xFF,
        ];
        assert_eq!(pixel, expected);
    }

    #[test]
    fn qr_svg_sakura_applies_palette() {
        let code = QrCode::new(b"sakura").expect("qr code");
        for style in [QrRenderStyle::Sakura, QrRenderStyle::SakuraWind] {
            let svg = render_svg(&code, 32, style).expect("svg");
            assert!(svg.contains("fill=\"#FCF5F9\""));
            assert!(svg.contains("fill=\"#572633\""));
        }
    }

    #[test]
    fn qr_svg_sakura_storm_applies_palette() {
        let code = QrCode::new(b"sakura-storm").expect("qr code");
        let svg = render_svg(&code, 32, QrRenderStyle::SakuraStorm).expect("svg");
        assert!(svg.contains("fill=\"#07040D\""));
        assert!(svg.contains("fill=\"#F6EAF6\""));
    }

    #[test]
    fn sakura_wind_finder_modules_render_as_ink() {
        let code = QrCode::new(b"sakura-wind").expect("qr code");
        let dimension = 128;
        let image = render_sakura_wind_rgba(&code, dimension, 0, 1);
        let quiet_zone = if code.version().is_micro() { 2 } else { 4 };
        let modules = code.width() as u32;
        let total_modules = modules + quiet_zone * 2;
        let module_size = (dimension / total_modules).max(1);
        let module_x = quiet_zone + 3;
        let module_y = quiet_zone + 3;
        let x = module_x * module_size + module_size / 2;
        let y = module_y * module_size + module_size / 2;
        let pixel = image.get_pixel(x, y).0;
        let to_u8 = |value: f64| -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 };
        let expected = [
            to_u8(SAKURA_INK[0]),
            to_u8(SAKURA_INK[1]),
            to_u8(SAKURA_INK[2]),
            0xFF,
        ];
        assert_eq!(pixel, expected);
    }

    #[test]
    fn write_png_sakura_emits_rgba() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("frame.png");
        let image = GrayImage {
            width: 1,
            height: 1,
            data: vec![0],
        };
        write_png(&path, &image, QrRenderStyle::Sakura, 0, 1).expect("write");
        let file = fs::File::open(&path).expect("open");
        let decoder = png::Decoder::new(std::io::BufReader::new(file));
        let reader = decoder.read_info().expect("read info");
        assert_eq!(reader.info().color_type, png::ColorType::Rgba);
    }

    #[test]
    fn write_apng_sakura_writes_png_header() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("stream.png");
        let image = GrayImage {
            width: 1,
            height: 1,
            data: vec![0],
        };
        write_apng(&path, &[image], 12, QrRenderStyle::Sakura).expect("write");
        let bytes = fs::read(&path).expect("read");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn write_gif_sakura_writes_gif_header() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("stream.gif");
        let image = GrayImage {
            width: 1,
            height: 1,
            data: vec![0],
        };
        write_gif(&path, &[image], 12, QrRenderStyle::Sakura).expect("write");
        let bytes = fs::read(&path).expect("read");
        assert!(bytes.starts_with(b"GIF"));
    }

    #[test]
    fn write_png_sakura_wind_emits_rgba() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("frame.png");
        let code = QrCode::new(b"sakura-wind").expect("qr code");
        let image = render_sakura_wind_rgba(&code, 64, 0, 1);
        write_png_rgba(&path, &image).expect("write");
        let file = fs::File::open(&path).expect("open");
        let decoder = png::Decoder::new(std::io::BufReader::new(file));
        let reader = decoder.read_info().expect("read info");
        assert_eq!(reader.info().color_type, png::ColorType::Rgba);
    }

    #[test]
    fn write_apng_sakura_wind_writes_png_header() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("stream.png");
        let code = QrCode::new(b"sakura-wind").expect("qr code");
        let image = render_sakura_wind_rgba(&code, 64, 0, 1);
        write_apng_rgba(&path, &[image], 12).expect("write");
        let bytes = fs::read(&path).expect("read");
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn write_gif_sakura_wind_writes_gif_header() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("stream.gif");
        let code = QrCode::new(b"sakura-wind").expect("qr code");
        let image = render_sakura_wind_rgba(&code, 64, 0, 1);
        write_gif_rgba(&path, &[image], 12).expect("write");
        let bytes = fs::read(&path).expect("read");
        assert!(bytes.starts_with(b"GIF"));
    }

    #[test]
    fn sakura_storm_finder_modules_render_with_bright_core() {
        let code = QrCode::new(b"sakura-storm").expect("qr code");
        let dimension = 256;
        let image = render_sakura_storm_rgba(&code, dimension, 0, 1);
        let quiet_zone = if code.version().is_micro() { 2 } else { 4 };
        let modules = code.width() as u32;
        let total_modules = modules + quiet_zone * 2;
        let target_qr = ((dimension as f64) * SAKURA_STORM_QR_SCALE).round() as u32;
        let module_size = (target_qr / total_modules).max(1);
        let qr_size = total_modules * module_size;
        let side = dimension.max(qr_size);
        let qr_offset = (side - qr_size) / 2;
        let module_x = quiet_zone + 3;
        let module_y = quiet_zone + 3;
        let x = qr_offset + module_x * module_size + module_size / 2;
        let y = qr_offset + module_y * module_size + module_size / 2;
        let pixel = image.get_pixel(x, y).0;
        let to_u8 = |value: f64| -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 };
        let expected = [
            to_u8(SAKURA_STORM_FUNCTIONAL_CORE[0]),
            to_u8(SAKURA_STORM_FUNCTIONAL_CORE[1]),
            to_u8(SAKURA_STORM_FUNCTIONAL_CORE[2]),
            0xFF,
        ];
        assert_eq!(pixel, expected);
    }

    #[test]
    fn write_png_sakura_storm_emits_rgba() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("frame.png");
        let code = QrCode::new(b"sakura-storm").expect("qr code");
        let image = render_sakura_storm_rgba(&code, 64, 0, 1);
        write_png_rgba(&path, &image).expect("write");
        let file = fs::File::open(&path).expect("open");
        let decoder = png::Decoder::new(std::io::BufReader::new(file));
        let reader = decoder.read_info().expect("read info");
        assert_eq!(reader.info().color_type, png::ColorType::Rgba);
    }

    #[test]
    fn throughput_profile_for_three_kib_per_second() {
        let payload = vec![0_u8; 12 * 1024];
        let (_envelope, frames) = QrStreamEncoder::encode_frames(
            &payload,
            QrStreamOptions {
                chunk_size: 336,
                parity_group: 4,
                payload_kind: QrPayloadKind::Unspecified,
                ..QrStreamOptions::default()
            },
        )
        .expect("encode");
        let bytes_per_second =
            estimate_payload_bytes_per_second(payload.len(), frames.len(), 12);
        assert!(
            bytes_per_second >= 3_000,
            "expected at least 3000 B/s, got {bytes_per_second}"
        );
    }
}
