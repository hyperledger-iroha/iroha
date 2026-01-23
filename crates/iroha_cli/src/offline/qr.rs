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
                    let svg = render_svg(&frame.code, self.dimension)?;
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
                for frame in &rendered {
                    let image = render_image(&frame.code, self.dimension)?;
                    let path = png_dir.join(format!("frame_{:04}.png", frame.index));
                    write_png(&path, &image)?;
                }
            }
            QrOutputFormat::Gif => {
                let frames = render_animation(&rendered, self.dimension)?;
                let path = output_dir.join("stream.gif");
                write_gif(&path, &frames, self.fps)?;
            }
            QrOutputFormat::Apng => {
                let frames = render_animation(&rendered, self.dimension)?;
                let path = output_dir.join("stream.png");
                write_apng(&path, &frames, self.fps)?;
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
        manifest_map.insert(
            "frame_encoding".to_string(),
            Value::from(self.frame_encoding.label()),
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

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum QrPayloadKindArg {
    Unspecified,
    OfflineToOnlineTransfer,
    OfflineSpendReceipt,
    OfflineEnvelope,
}

impl QrPayloadKindArg {
    fn label(&self) -> &'static str {
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

fn render_svg(code: &QrCode, dimension: u32) -> Result<String> {
    let svg = code
        .render::<svg::Color>()
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

fn render_animation(frames: &[RenderedFrame], dimension: u32) -> Result<Vec<GrayImage>> {
    let mut out = Vec::with_capacity(frames.len());
    for frame in frames {
        out.push(render_image(&frame.code, dimension)?);
    }
    Ok(out)
}

fn write_png(path: &Path, image: &GrayImage) -> Result<()> {
    let file = fs::File::create(path)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, image.width, image.height);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image.data)?;
    Ok(())
}

fn write_apng(path: &Path, frames: &[GrayImage], fps: u16) -> Result<()> {
    if frames.is_empty() {
        return Err(eyre!("no frames to encode"));
    }
    let file = fs::File::create(path)?;
    let writer = BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, frames[0].width, frames[0].height);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_animated(frames.len() as u32, 0)?;
    encoder.set_sep_def_img(false)?;
    let mut writer = encoder.write_header()?;
    let delay = frame_delay_ms(fps);
    let delay_den = 1000u16;
    for frame in frames {
        writer.set_frame_delay(delay as u16, delay_den)?;
        writer.write_image_data(&frame.data)?;
    }
    Ok(())
}

fn write_gif(path: &Path, frames: &[GrayImage], fps: u16) -> Result<()> {
    if frames.is_empty() {
        return Err(eyre!("no frames to encode"));
    }
    let file = fs::File::create(path)?;
    let mut encoder = image::codecs::gif::GifEncoder::new(file);
    let delay_ms = frame_delay_ms(fps) as u32;
    let delay = image::Delay::from_numer_denom_ms(delay_ms.max(1), 1);
    let mut gif_frames = Vec::with_capacity(frames.len());
    for frame in frames {
        let rgba = grayscale_to_rgba(frame);
        gif_frames.push(image::Frame::from_parts(rgba, 0, 0, delay));
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

    #[test]
    fn qr_text_roundtrip() {
        let payload = b"frame-bytes";
        let encoded = encode_frame_bytes(payload, QrFrameEncoding::Base64).expect("encode");
        let decoded = decode_frame_bytes(&encoded, QrFrameEncoding::Base64).expect("decode");
        assert_eq!(decoded, payload);
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
}
