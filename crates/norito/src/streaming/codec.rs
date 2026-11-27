//! Baseline streaming codec, bundled entropy, and audio implementation.
use super::{
    AudioCodecError, AudioCodecLayoutMismatchInfo, AudioEncoderSampleCountMismatchInfo, AudioFrame,
    AudioLayout, AudioTrackSummary, BundleAcceleration, Bytes, CapabilityFlags, ChunkDescriptor,
    EncryptionSuite, EntropyMode, FecScheme, FeedbackHint, Hash, ManifestV1, Multiaddr,
    NeuralBundle, NoritoJsonValue, PrivacyRoute, ProfileId, RansGroupTableV1, RdoMode,
    SegmentAudio, SegmentHeader, Signature, SignedRansTablesV1, StorageClass, StreamMetadata,
    Timestamp,
    chunk::{
        AudioFrameCadenceMismatchInfo, AudioFrameCadenceOverflowInfo, AudioSampleCountMismatchInfo,
        BlockCountMismatchInfo, ChromaDimensionsNotEvenInfo, ChunkError, CodecError,
        FrameCountOverflowInfo, FrameLengthMismatch, chunk_commitments, derive_nonce_salt,
        merkle_root,
    },
    json, saturating_usize_to_u32, saturating_usize_to_u64,
};
use crate as norito;
use norito_derive::{NoritoDeserialize, NoritoSerialize};
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    collections::BTreeMap,
    convert::TryInto,
    fs,
    path::Path,
    sync::{Arc, OnceLock},
};
use thiserror::Error;
use toml::Value as TomlValue;
pub(crate) const BLOCK_SIZE: usize = 8;
pub(crate) const BLOCK_PIXELS: usize = BLOCK_SIZE * BLOCK_SIZE;
pub(crate) const FRAME_HEADER_LEN: usize = 16;
pub(super) const FRAME_ZSTD: u8 = 0x80;
// Avoid starting a compressor for tiny plane payloads.
const MIN_FRAME_COMPRESSION_BYTES: usize = 256;
const RLE_EOB: u8 = 0xFF;
const MAX_ZERO_RUN: usize = 254;
const RLE_TOKEN_BITS: u64 = 24;
// One AC run always fits in the byte that accompanies its nonzero level.
const _: () = assert!(BLOCK_PIXELS - 1 <= MAX_ZERO_RUN);
const RDO_ENERGY_BUCKETS: [u32; RDO_BUCKET_COUNT - 1] = [64, 256, 1024, 4096];
const NEURAL_FEATURES: usize = 8;
const NEURAL_HIDDEN: usize = 32;
const NEURAL_OUTPUT: usize = 4;
const DEFAULT_NEURAL_SEED: [u8; 32] = *b"nsc-neural-predictor-v1--seed!!!";
#[cfg(not(feature = "streaming-fixed-point-dct"))]
const DCT_FACTORS: [[f64; 8]; 8] = [
    [
        0.3535533905932737,
        0.3535533905932737,
        0.3535533905932737,
        0.3535533905932737,
        0.3535533905932737,
        0.3535533905932737,
        0.3535533905932737,
        0.3535533905932737,
    ],
    [
        0.4903926402016152,
        0.4157348061512726,
        0.2777851165098011,
        0.0975451610080642,
        -0.0975451610080641,
        -0.277785116509801,
        -0.4157348061512727,
        -0.4903926402016152,
    ],
    [
        0.4619397662556434,
        0.1913417161825449,
        -0.1913417161825449,
        -0.4619397662556434,
        -0.4619397662556434,
        -0.1913417161825452,
        0.191341716182545,
        0.4619397662556433,
    ],
    [
        0.4157348061512726,
        -0.0975451610080641,
        -0.4903926402016152,
        -0.2777851165098011,
        0.2777851165098009,
        0.4903926402016152,
        0.0975451610080644,
        -0.4157348061512726,
    ],
    [
        0.3535533905932738,
        -0.3535533905932737,
        -0.3535533905932738,
        0.3535533905932737,
        0.3535533905932738,
        -0.3535533905932733,
        -0.3535533905932736,
        0.3535533905932733,
    ],
    [
        0.2777851165098011,
        -0.4903926402016152,
        0.0975451610080642,
        0.4157348061512727,
        -0.4157348061512726,
        -0.097545161008064,
        0.4903926402016153,
        -0.2777851165098008,
    ],
    [
        0.1913417161825449,
        -0.4619397662556434,
        0.4619397662556433,
        -0.1913417161825449,
        -0.1913417161825453,
        0.4619397662556434,
        -0.4619397662556432,
        0.1913417161825448,
    ],
    [
        0.0975451610080642,
        -0.2777851165098011,
        0.4157348061512727,
        -0.4903926402016153,
        0.4903926402016152,
        -0.4157348061512725,
        0.2777851165098008,
        -0.0975451610080643,
    ],
];
#[cfg(feature = "streaming-fixed-point-dct")]
const DCT_FACTORS_Q15: [[i16; 8]; 8] = [
    [11585, 11585, 11585, 11585, 11585, 11585, 11585, 11585],
    [16069, 13623, 9102, 3196, -3196, -9102, -13623, -16069],
    [15137, 6270, -6270, -15137, -15137, -6270, 6270, 15137],
    [13623, -3196, -16069, -9102, 9102, 16069, 3196, -13623],
    [11585, -11585, -11585, 11585, 11585, -11585, -11585, 11585],
    [9102, -16069, 3196, 13623, -13623, -3196, 16069, -9102],
    [6270, -15137, 15137, -6270, -6270, 15137, -15137, 6270],
    [3196, -9102, 13623, -16069, 16069, -13623, 9102, -3196],
];
#[cfg(feature = "streaming-fixed-point-dct")]
const DCT_Q_BITS: u32 = 15;
#[cfg(feature = "streaming-fixed-point-dct")]
const DCT_SHIFT: u32 = DCT_Q_BITS * 2;
#[cfg(feature = "streaming-fixed-point-dct")]
const DCT_ROUND: i64 = 1 << (DCT_SHIFT - 1);
#[derive(Debug, Error)]
pub enum BundleTableError {
    #[error("failed to read SignedRansTablesV1 artefact: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse SignedRansTablesV1 TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("invalid SignedRansTablesV1 structure: {0}")]
    InvalidStructure(&'static str),
    #[error("invalid SignedRansTablesV1 payload: {0}")]
    Json(#[from] json::Error),
    #[error("checksum mismatch for SignedRansTablesV1 payload")]
    ChecksumMismatch,
    #[error("missing rANS frequency group for bit length {bit_len}")]
    MissingGroup { bit_len: u8 },
    #[error("invalid rANS frequency group for bit length {bit_len}: {reason}")]
    InvalidGroup { bit_len: u8, reason: &'static str },
}
/// Errors returned when decoding bundled rANS streams.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum BundleDecodeError {
    /// Stream too short to contain the serialized ANS state.
    #[error("bundle rANS stream missing serialized state")]
    TruncatedState,
    /// SIMD bundle stream is missing the header or lane length metadata.
    #[error("SIMD bundle stream missing header or lane lengths")]
    InvalidSimdHeader,
    /// Declared stream lengths do not match the payload.
    #[error("bundle rANS stream length mismatch")]
    LengthMismatch,
    /// A bundle record advertises a width outside the authenticated table domain.
    #[error("invalid bundle bit length at index {index}: found {bit_len}, expected 1..={max}")]
    InvalidBitLength {
        /// Position within the bundle record stream.
        index: u32,
        /// Width advertised by the record.
        bit_len: u8,
        /// Maximum width authenticated by the selected tables.
        max: u8,
    },
    /// Table checksum does not match the telemetry-provided checksum.
    #[error("bundle table checksum mismatch: expected {expected:?}, found {found:?}")]
    ChecksumMismatch { expected: Hash, found: Hash },
    /// Ran out of bytes while renormalizing the ANS state.
    #[error("bundle rANS renormalization underflow")]
    RenormalizeUnderflow,
    /// Decoded symbol does not match the recorded bundle bits.
    #[error(
        "bundle symbol mismatch at index {index}: expected {expected:#04x}, found {found:#04x}"
    )]
    SymbolMismatch {
        /// Position within the decoded bundle stream.
        index: u32,
        /// Symbol reconstructed from the bundle record.
        expected: u8,
        /// Symbol produced by the ANS stream.
        found: u8,
    },
}
/// Errors encountered while loading or applying a bundle context remap.
#[derive(Debug, Error)]
pub enum ContextRemapError {
    /// The remap JSON could not be read from disk.
    #[error("failed to read context remap file: {0}")]
    Io(#[from] std::io::Error),
    /// The remap JSON could not be parsed.
    #[error("failed to parse context remap file: {0}")]
    Json(#[from] json::Error),
    /// The remap JSON is missing required fields.
    #[error("context remap is missing required field: {0}")]
    Missing(&'static str),
    /// A context identifier in the remap was outside the supported range.
    #[error("context remap contains an invalid context id {0}")]
    InvalidContext(u64),
}
/// Mapping from raw bundle contexts to a pruned/remapped domain.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct BundleContextRemap {
    mapping: BTreeMap<BundleContextId, BundleContextId>,
    escape_context: BundleContextId,
    remapped: u32,
    dropped: u32,
}
impl BundleContextRemap {
    /// Construct a new remap with the provided mapping table and escape context.
    pub fn new(
        mapping: BTreeMap<BundleContextId, BundleContextId>,
        escape_context: BundleContextId,
        remapped: u32,
        dropped: u32,
    ) -> Self {
        Self {
            mapping,
            escape_context,
            remapped,
            dropped,
        }
    }
    /// Apply the remap to a raw context, returning the mapped or escape context.
    #[must_use]
    pub fn map(&self, context: BundleContextId) -> BundleContextId {
        *self.mapping.get(&context).unwrap_or(&self.escape_context)
    }
    /// Escape context used for unmapped entries.
    #[must_use]
    pub const fn escape_context(&self) -> BundleContextId {
        self.escape_context
    }
    /// Number of contexts explicitly remapped.
    #[must_use]
    pub const fn remapped_count(&self) -> u32 {
        self.remapped
    }
    /// Number of contexts routed to the escape bucket.
    #[must_use]
    pub const fn dropped_count(&self) -> u32 {
        self.dropped
    }
    fn from_report(value: &NoritoJsonValue) -> Result<Self, ContextRemapError> {
        let escape_context = value
            .get("escape_context")
            .and_then(NoritoJsonValue::as_u64)
            .and_then(|raw| u16::try_from(raw).ok())
            .map(BundleContextId::new)
            .unwrap_or_else(|| BundleContextId::new(u16::MAX));
        let mut mapping = BTreeMap::new();
        let mut remapped = 0u32;
        let mut dropped = 0u32;
        let mut ingest_rows =
            |key: &str, dest: Option<BundleContextId>| -> Result<(), ContextRemapError> {
                if let Some(entries) = value.get(key).and_then(NoritoJsonValue::as_array) {
                    for entry in entries {
                        let original = entry
                            .get("original")
                            .and_then(NoritoJsonValue::as_u64)
                            .ok_or(ContextRemapError::Missing("original"))?;
                        let remapped_value = entry
                            .get("remapped")
                            .and_then(NoritoJsonValue::as_u64)
                            .and_then(|raw| u16::try_from(raw).ok())
                            .map(BundleContextId::new)
                            .or(dest);
                        let raw = u16::try_from(original)
                            .map_err(|_| ContextRemapError::InvalidContext(original))?;
                        let target = remapped_value.unwrap_or(BundleContextId::new(raw));
                        mapping.insert(BundleContextId::new(raw), target);
                        if target == escape_context {
                            dropped = dropped.saturating_add(1);
                        } else if target != BundleContextId::new(raw) {
                            remapped = remapped.saturating_add(1);
                        }
                    }
                }
                Ok(())
            };
        ingest_rows("kept", None)?;
        ingest_rows("dropped", Some(escape_context))?;
        Ok(Self::new(mapping, escape_context, remapped, dropped))
    }
}
/// Load a bundle context remap from a JSON file (output of `streaming-context-remap`).
pub fn load_bundle_context_remap_from_json<P: AsRef<Path>>(
    path: P,
) -> Result<Arc<BundleContextRemap>, ContextRemapError> {
    let text = fs::read_to_string(&path)?;
    let value: NoritoJsonValue = json::from_str(&text)?;
    let remap = BundleContextRemap::from_report(&value)?;
    Ok(Arc::new(remap))
}
#[derive(Clone, Debug)]
pub struct BundleAnsTables {
    precision_bits: u8,
    checksum: Hash,
    max_width: u8,
    tables: [SymbolTable; MAX_BUNDLE_WIDTH],
}
#[cfg(feature = "schema-structural")]
impl ::iroha_schema::TypeId for BundleAnsTables {
    fn id() -> String {
        "BundleAnsTables".to_owned()
    }
}
#[cfg(feature = "schema-structural")]
impl ::iroha_schema::IntoSchema for BundleAnsTables {
    fn type_name() -> String {
        "BundleAnsTables".to_owned()
    }
    fn update_schema_map(map: &mut ::iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(::iroha_schema::Metadata::Struct(
            ::iroha_schema::NamedFieldsMeta {
                declarations: vec![
                    ::iroha_schema::Declaration {
                        name: "precision_bits".to_owned(),
                        ty: core::any::TypeId::of::<u8>(),
                    },
                    ::iroha_schema::Declaration {
                        name: "checksum".to_owned(),
                        ty: core::any::TypeId::of::<Hash>(),
                    },
                    ::iroha_schema::Declaration {
                        name: "max_width".to_owned(),
                        ty: core::any::TypeId::of::<u8>(),
                    },
                ],
            },
        ));
        <u8 as ::iroha_schema::IntoSchema>::update_schema_map(map);
        <Hash as ::iroha_schema::IntoSchema>::update_schema_map(map);
    }
}
impl BundleAnsTables {
    fn uniform(precision_bits: u8) -> Self {
        let tables =
            core::array::from_fn(|idx| build_uniform_symbol_table((idx + 1) as u8, precision_bits));
        let checksum = hash_bundle_tables(&tables, precision_bits);
        Self {
            precision_bits,
            checksum,
            max_width: MAX_BUNDLE_WIDTH as u8,
            tables,
        }
    }
    fn from_signed(signed: &SignedRansTablesV1) -> Result<Self, BundleTableError> {
        verify_signed_tables(signed)?;
        let mut slots: [Option<SymbolTable>; MAX_BUNDLE_WIDTH] = core::array::from_fn(|_| None);
        let precision_bits = signed
            .payload
            .body
            .groups
            .first()
            .map(|g| g.precision_bits)
            .ok_or(BundleTableError::InvalidStructure(
                "SignedRansTablesV1 payload must contain at least one group",
            ))?;
        let mut configured_width = signed.payload.body.bundle_width;
        if configured_width == 0 {
            configured_width = MAX_BUNDLE_WIDTH as u8;
        }
        let configured_width = configured_width.max(2).min(MAX_BUNDLE_WIDTH as u8);
        for group in &signed.payload.body.groups {
            let group_size = usize::from(group.group_size);
            if group_size == 0 || !group_size.is_power_of_two() {
                return Err(BundleTableError::InvalidGroup {
                    bit_len: 0,
                    reason: "group_size must be a power of two",
                });
            }
            let bit_len = group_size.trailing_zeros() as u8;
            if bit_len == 0 || bit_len as usize > MAX_BUNDLE_WIDTH {
                return Err(BundleTableError::InvalidGroup {
                    bit_len,
                    reason: "unsupported bundle width",
                });
            }
            if bit_len > configured_width {
                return Err(BundleTableError::InvalidGroup {
                    bit_len,
                    reason: "group width exceeds declared bundle width",
                });
            }
            if group.precision_bits != precision_bits {
                return Err(BundleTableError::InvalidGroup {
                    bit_len,
                    reason: "precision bits mismatch across groups",
                });
            }
            let table = SymbolTable::from_group(group)?;
            slots[(bit_len - 1) as usize] = Some(table);
        }
        let mut tables: [SymbolTable; MAX_BUNDLE_WIDTH] =
            core::array::from_fn(|idx| build_uniform_symbol_table((idx + 1) as u8, precision_bits));
        for bit_len in 2..=configured_width {
            let idx = (bit_len - 1) as usize;
            tables[idx] = slots[idx]
                .take()
                .ok_or(BundleTableError::MissingGroup { bit_len })?;
        }
        tables[0] = derive_significance_table(&tables[1]);
        Ok(Self {
            precision_bits,
            checksum: signed.payload.checksum_sha256,
            max_width: configured_width,
            tables,
        })
    }
    fn table_for_bits(&self, bit_len: u8) -> &SymbolTable {
        assert!(
            bit_len <= self.max_width,
            "bundle tables missing width {} (max {})",
            bit_len,
            self.max_width
        );
        let clamped = bit_len.clamp(1, self.max_width);
        &self.tables[clamped as usize - 1]
    }
    pub fn precision_bits(&self) -> u8 {
        self.precision_bits
    }
    pub fn checksum(&self) -> Hash {
        self.checksum
    }
    pub fn max_width(&self) -> u8 {
        self.max_width
    }
    #[cfg(test)]
    pub(crate) fn from_signed_for_tests(
        signed: &SignedRansTablesV1,
    ) -> Result<Self, BundleTableError> {
        Self::from_signed(signed)
    }
    #[cfg(test)]
    pub(crate) fn freq_len_for_bits_for_tests(&self, bit_len: u8) -> Option<usize> {
        if bit_len == 0 || bit_len > self.max_width {
            return None;
        }
        Some(self.table_for_bits(bit_len).freq.len())
    }
}
pub fn load_bundle_tables_from_toml<P: AsRef<Path>>(
    path: P,
) -> Result<Arc<BundleAnsTables>, BundleTableError> {
    let contents = fs::read_to_string(path)?;
    parse_bundle_tables_from_toml(&contents)
}
/// Parse supplied signed rANS table TOML through the canonical table validator.
///
/// This function reads no files and does not resolve paths or include other inputs.
///
/// # Errors
/// Refuses malformed TOML, unknown fields, invalid table groups, or a mismatched checksum.
pub fn parse_bundle_tables_from_toml(
    contents: &str,
) -> Result<Arc<BundleAnsTables>, BundleTableError> {
    let toml_value: TomlValue = toml::from_str(contents)?;
    let json_value = toml_to_norito_value(&toml_value)?;
    let signed: SignedRansTablesV1 = json::from_value(json_value)?;
    let tables = BundleAnsTables::from_signed(&signed)?;
    Ok(Arc::new(tables))
}
/// Lazily construct the shared default bundle tables (uniform distribution).
///
/// The returned handle is backed by a `OnceLock`, so multiple callers share the
/// same allocation and precision parameters.
pub fn default_bundle_tables() -> Arc<BundleAnsTables> {
    static DEFAULT: OnceLock<Arc<BundleAnsTables>> = OnceLock::new();
    DEFAULT
        .get_or_init(|| Arc::new(BundleAnsTables::uniform(BUNDLE_ANS_PRECISION_BITS)))
        .clone()
}
fn hash_bundle_tables(tables: &[SymbolTable; MAX_BUNDLE_WIDTH], precision_bits: u8) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update([precision_bits]);
    for table in tables {
        hasher.update((table.freq.len() as u32).to_le_bytes());
        for &freq in &table.freq {
            hasher.update(freq.to_le_bytes());
        }
    }
    let digest = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&digest);
    hash
}
fn verify_signed_tables(signed: &SignedRansTablesV1) -> Result<(), BundleTableError> {
    let checksum = signed
        .payload
        .body
        .checksum_sha256()
        .map_err(|_| BundleTableError::InvalidStructure("failed to encode table body"))?;
    if checksum != signed.payload.checksum_sha256 {
        return Err(BundleTableError::ChecksumMismatch);
    }
    Ok(())
}
fn toml_to_norito_value(value: &TomlValue) -> Result<NoritoJsonValue, BundleTableError> {
    use toml::Value::{Array, Boolean, Datetime, Float, Integer, String as TomlString, Table};
    Ok(match value {
        Boolean(b) => NoritoJsonValue::Bool(*b),
        Integer(i) => {
            if *i >= 0 {
                NoritoJsonValue::Number(json::native::Number::U64(*i as u64))
            } else {
                NoritoJsonValue::Number(json::native::Number::I64(*i))
            }
        }
        Float(f) => NoritoJsonValue::Number(json::native::Number::F64(*f)),
        TomlString(s) => NoritoJsonValue::String(s.clone()),
        Datetime(dt) => NoritoJsonValue::String(dt.to_string()),
        Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(toml_to_norito_value(item)?);
            }
            NoritoJsonValue::Array(out)
        }
        Table(map) => {
            let mut out = BTreeMap::new();
            for (k, v) in map {
                out.insert(k.clone(), toml_to_norito_value(v)?);
            }
            NoritoJsonValue::Object(out)
        }
    })
}
const BASELINE_QUANT_MATRIX: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];
const MIN_AC_WEIGHT: u8 = {
    let mut min = u8::MAX;
    let mut i = 1;
    while i < BLOCK_PIXELS {
        if BASELINE_QUANT_MATRIX[i] < min {
            min = BASELINE_QUANT_MATRIX[i];
        }
        i += 1;
    }
    min
};
const ZIG_ZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];
const ZIGZAG_BIT: [u64; BLOCK_PIXELS] = {
    let mut bits = [0; BLOCK_PIXELS];
    let mut order = 1;
    while order < BLOCK_PIXELS {
        bits[ZIG_ZAG[order]] = 1 << order;
        order += 1;
    }
    bits
};

fn ac_support(levels: &[i16; BLOCK_PIXELS]) -> u64 {
    levels
        .iter()
        .zip(ZIGZAG_BIT)
        .fold(0, |bits, (&level, flag)| {
            bits | (u64::from(level != 0) * flag)
        })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub(crate) enum FrameType {
    Intra,
    Predicted,
}
impl FrameType {
    fn as_byte(self) -> u8 {
        match self {
            Self::Intra => 0,
            Self::Predicted => 1,
        }
    }
    pub(crate) fn from_byte(byte: u8) -> Result<Self, CodecError> {
        match byte & !FRAME_ZSTD {
            0 => Ok(Self::Intra),
            1 => Ok(Self::Predicted),
            _ => Err(CodecError::UnknownFrameType(byte)),
        }
    }
    pub(crate) fn reset_dc(self) -> bool {
        matches!(self, FrameType::Intra)
    }
}
/// Segment encoding parameters for the baseline profile.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct BaselineEncoderConfig {
    pub profile: ProfileId,
    pub encryption_suite: EncryptionSuite,
    pub layer_bitmap: u32,
    pub duration_ns: u32,
    pub storage_class: StorageClass,
    pub feedback_hint: FeedbackHint,
    pub frame_dimensions: FrameDimensions,
    pub frames_per_segment: u16,
    pub frame_duration_ns: u32,
    pub quantizer: u8,
    pub audio: Option<AudioEncoderConfig>,
    pub entropy_mode: EntropyMode,
    pub bundle_width: u8,
    pub bundle_tables: Arc<BundleAnsTables>,
    pub bundle_acceleration: BundleAcceleration,
    pub bundle_context_remap: Option<Arc<BundleContextRemap>>,
    /// Optional prefetch distance (in records) for the bundle ANS encoder. A value
    /// of `0` disables prefetching.
    pub bundle_prefetch_distance: u16,
    pub rdo_mode: RdoMode,
    pub rdo_neural_seed: Option<Hash>,
}
impl Default for BaselineEncoderConfig {
    fn default() -> Self {
        Self {
            profile: ProfileId::BASELINE,
            encryption_suite: EncryptionSuite::X25519ChaCha20Poly1305([0u8; 32]),
            layer_bitmap: 0b1,
            duration_ns: 250_000_000,
            storage_class: StorageClass::Ephemeral,
            feedback_hint: FeedbackHint::default(),
            frame_dimensions: FrameDimensions::new(640, 360),
            frames_per_segment: 1,
            frame_duration_ns: 33_333_333,
            quantizer: 16,
            audio: None,
            entropy_mode: EntropyMode::RansBundled,
            bundle_width: 2,
            bundle_tables: default_bundle_tables(),
            bundle_acceleration: BundleAcceleration::None,
            bundle_context_remap: None,
            bundle_prefetch_distance: 0,
            rdo_mode: RdoMode::None,
            rdo_neural_seed: None,
        }
    }
}
impl BaselineEncoderConfig {
    #[must_use]
    pub fn with_audio(mut self, audio: AudioEncoderConfig) -> Self {
        self.audio = Some(audio);
        self
    }
    #[must_use]
    pub fn with_bundle_tables(mut self, tables: Arc<BundleAnsTables>) -> Self {
        self.bundle_tables = tables;
        self
    }
    #[must_use]
    pub fn with_bundle_acceleration(mut self, accel: BundleAcceleration) -> Self {
        self.bundle_acceleration = accel;
        self
    }
    #[must_use]
    pub fn with_bundle_context_remap(mut self, remap: Arc<BundleContextRemap>) -> Self {
        self.bundle_context_remap = Some(remap);
        self
    }
    #[must_use]
    pub fn with_bundle_prefetch_distance(mut self, distance: u16) -> Self {
        self.bundle_prefetch_distance = distance;
        self
    }
    #[must_use]
    pub fn with_rdo_mode(mut self, mode: RdoMode) -> Self {
        self.rdo_mode = mode;
        self
    }
    #[must_use]
    pub fn with_rdo_neural_seed(mut self, seed: Hash) -> Self {
        self.rdo_neural_seed = Some(seed);
        self
    }
}
/// Parameters required to construct a manifest for a baseline segment.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct BaselineManifestParams {
    pub stream_id: Hash,
    pub protocol_version: u16,
    pub published_at: Timestamp,
    pub da_endpoint: Multiaddr,
    pub privacy_routes: Vec<PrivacyRoute>,
    pub public_metadata: StreamMetadata,
    pub capabilities: CapabilityFlags,
    pub signature: Signature,
    pub fec_suite: FecScheme,
    pub neural_bundle: Option<NeuralBundle>,
    pub transport_capabilities_hash: Hash,
}
impl Default for BaselineManifestParams {
    fn default() -> Self {
        Self {
            stream_id: [0u8; 32],
            protocol_version: 1,
            published_at: 0,
            da_endpoint: Multiaddr::default(),
            privacy_routes: Vec::new(),
            public_metadata: StreamMetadata::default(),
            capabilities: CapabilityFlags::default(),
            signature: [0u8; 64],
            fec_suite: FecScheme::Rs12_10,
            neural_bundle: None,
            transport_capabilities_hash: [0u8; 32],
        }
    }
}
/// Encoded segment artifacts produced by the baseline encoder.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct EncodedSegment {
    pub header: SegmentHeader,
    pub descriptors: Vec<ChunkDescriptor>,
    pub chunks: Vec<Vec<u8>>,
    pub audio: Option<SegmentAudio>,
}
impl EncodedSegment {
    /// Construct a manifest that matches the encoded segment.
    pub fn build_manifest(&self, mut params: BaselineManifestParams) -> ManifestV1 {
        assert!(
            self.header.entropy_mode.is_bundled(),
            "rANS manifests without bundled tables are not supported"
        );
        params.capabilities = params
            .capabilities
            .insert(CapabilityFlags::FEATURE_ENTROPY_BUNDLED);
        let accel_mask = CapabilityFlags::FEATURE_BUNDLE_ACCEL_CPU_SIMD
            | CapabilityFlags::FEATURE_BUNDLE_ACCEL_GPU;
        params.capabilities = params.capabilities.remove(accel_mask);
        let required = self.header.bundle_acceleration.capability_mask();
        if required != 0 {
            params.capabilities = params.capabilities.insert(required);
        }
        ManifestV1 {
            stream_id: params.stream_id,
            protocol_version: params.protocol_version,
            segment_number: self.header.segment_number,
            published_at: params.published_at,
            profile: self.header.profile,
            entropy_mode: self.header.entropy_mode,
            entropy_tables_checksum: self.header.entropy_tables_checksum,
            da_endpoint: params.da_endpoint,
            chunk_root: self.header.chunk_merkle_root,
            content_key_id: self.header.content_key_id,
            nonce_salt: self.header.nonce_salt,
            chunk_descriptors: self.descriptors.clone(),
            transport_capabilities_hash: params.transport_capabilities_hash,
            encryption_suite: self.header.encryption_suite,
            fec_suite: params.fec_suite,
            privacy_routes: params.privacy_routes,
            neural_bundle: params.neural_bundle,
            audio_summary: self.header.audio_summary,
            public_metadata: params.public_metadata,
            capabilities: params.capabilities,
            signature: params.signature,
        }
    }
    /// Verify that a manifest is consistent with the encoded segment.
    pub fn verify_manifest(&self, manifest: &ManifestV1) -> Result<(), ManifestError> {
        verify_segment(
            &self.header,
            &self.descriptors,
            &self.chunks,
            self.audio.as_ref(),
        )
        .map_err(ManifestError::from)?;
        if manifest.segment_number != self.header.segment_number {
            return Err(ManifestError::SegmentNumberMismatch);
        }
        if manifest.profile != self.header.profile {
            return Err(ManifestError::ProfileMismatch);
        }
        if manifest.entropy_mode != self.header.entropy_mode {
            return Err(ManifestError::EntropyModeMismatch);
        }
        if manifest.entropy_mode.is_bundled()
            && manifest.entropy_tables_checksum != self.header.entropy_tables_checksum
        {
            return Err(ManifestError::EntropyTablesMismatch);
        }
        if manifest.encryption_suite != self.header.encryption_suite {
            return Err(ManifestError::EncryptionSuiteMismatch);
        }
        if manifest.content_key_id != self.header.content_key_id {
            return Err(ManifestError::ContentKeyIdMismatch);
        }
        if manifest.nonce_salt != self.header.nonce_salt {
            return Err(ManifestError::NonceSaltMismatch);
        }
        if manifest.chunk_root != self.header.chunk_merkle_root {
            return Err(ManifestError::ChunkRootMismatch);
        }
        if manifest.audio_summary != self.header.audio_summary {
            return Err(ManifestError::AudioSummaryMismatch);
        }
        if manifest.chunk_descriptors.len() != self.descriptors.len() {
            return Err(ManifestError::DescriptorCountMismatch);
        }
        for (idx, (expected, actual)) in self
            .descriptors
            .iter()
            .zip(manifest.chunk_descriptors.iter())
            .enumerate()
        {
            if expected != actual {
                return Err(ManifestError::DescriptorMismatch(saturating_usize_to_u32(
                    idx,
                )));
            }
        }
        let required_bundled = self.header.entropy_mode.is_bundled();
        let found_bundled = manifest
            .capabilities
            .contains(CapabilityFlags::FEATURE_ENTROPY_BUNDLED);
        if required_bundled != found_bundled {
            return Err(ManifestError::CapabilityEntropyFlagMismatch {
                required_bundled,
                found_bundled,
            });
        }
        let accel_mask = CapabilityFlags::FEATURE_BUNDLE_ACCEL_CPU_SIMD
            | CapabilityFlags::FEATURE_BUNDLE_ACCEL_GPU;
        let advertised_mask = manifest.capabilities.bits() & accel_mask;
        let required_mask = if required_bundled {
            self.header.bundle_acceleration.capability_mask()
        } else {
            0
        };
        if advertised_mask != required_mask {
            return Err(ManifestError::CapabilityAccelerationFlagMismatch {
                entropy_mode: manifest.entropy_mode,
                required_mask,
                found_mask: advertised_mask,
            });
        }
        Ok(())
    }
    /// Wrap the encoded segment into a portable bundle for RD/decoder tooling.
    pub fn to_bundle(
        &self,
        frame_dimensions: FrameDimensions,
        frame_duration_ns: u32,
    ) -> SegmentBundle {
        self.to_bundle_with_chroma(frame_dimensions, frame_duration_ns, Vec::new())
    }
    /// Wrap the encoded segment into a portable bundle for RD/decoder tooling, attaching optional chroma sidecars.
    pub fn to_bundle_with_chroma(
        &self,
        frame_dimensions: FrameDimensions,
        frame_duration_ns: u32,
        chroma: Vec<Chroma420Frame>,
    ) -> SegmentBundle {
        SegmentBundle {
            header: self.header.clone(),
            descriptors: self.descriptors.clone(),
            chunks: self.chunks.clone(),
            audio: self.audio.clone(),
            frame_dimensions,
            frame_duration_ns,
            chroma,
        }
    }
}
/// Serialized segment bundle carrying the encoded payloads and decoder metadata.
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::SegmentBundle")]
#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct SegmentBundle {
    /// Segment header as persisted alongside chunk data.
    pub header: SegmentHeader,
    /// Chunk descriptors matching the payload ordering.
    pub descriptors: Vec<ChunkDescriptor>,
    /// Raw chunk payloads.
    pub chunks: Vec<Bytes>,
    /// Optional encoded audio track.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub audio: Option<SegmentAudio>,
    /// Frame geometry required for decode and Y4M reconstruction.
    pub frame_dimensions: FrameDimensions,
    /// Frame duration in nanoseconds.
    pub frame_duration_ns: u32,
    /// Optional 4:2:0 chroma planes matching the decoded frames.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub chroma: Vec<Chroma420Frame>,
}
impl SegmentBundle {
    /// Convert the bundle back into an encoded segment, validating chunk commitments.
    pub fn into_segment(self) -> Result<(EncodedSegment, FrameDimensions, u32), SegmentError> {
        let (segment, dims, duration, _) = self.into_segment_with_chroma()?;
        Ok((segment, dims, duration))
    }
    /// Convert the bundle back into an encoded segment and return the attached chroma frames.
    pub fn into_segment_with_chroma(
        self,
    ) -> Result<(EncodedSegment, FrameDimensions, u32, Vec<Chroma420Frame>), SegmentError> {
        let chroma = self.chroma;
        let segment = EncodedSegment {
            header: self.header,
            descriptors: self.descriptors,
            chunks: self.chunks,
            audio: self.audio,
        };
        verify_segment(
            &segment.header,
            &segment.descriptors,
            &segment.chunks,
            segment.audio.as_ref(),
        )?;
        Ok((
            segment,
            self.frame_dimensions,
            self.frame_duration_ns,
            chroma,
        ))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ChunkListCountMismatch {
    pub descriptors: u32,
    pub chunks: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct HeaderDescriptorCountMismatch {
    pub header: u16,
    pub actual: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ChunkCountOverflowInfo {
    pub found: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct DescriptorOffsetDetails {
    pub index: u32,
    pub expected: u32,
    pub actual: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct DescriptorLengthDetails {
    pub index: u32,
    pub descriptor: u32,
    pub chunk: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioSampleRateMismatchInfo {
    pub expected: u32,
    pub found: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioFrameSamplesMismatchInfo {
    pub expected: u16,
    pub found: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioFrameDurationMismatchInfo {
    pub expected: u32,
    pub found: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioFrameCountMismatchInfo {
    pub expected: u16,
    pub found: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioFecMismatchInfo {
    pub expected: u8,
    pub found: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioLayoutMismatchInfo {
    pub expected: AudioLayout,
    pub found: AudioLayout,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioTimestampMismatchInfo {
    pub index: u32,
    pub expected: u64,
    pub found: u64,
}
/// Errors emitted when verifying or decoding NSC segments.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum SegmentError {
    #[error(transparent)]
    Chunk(#[from] ChunkError),
    #[error(
            "chunk descriptor count ({descriptors}) does not match chunk list ({chunks})",
            descriptors = .0.descriptors,
            chunks = .0.chunks
        )]
    CountMismatch(ChunkListCountMismatch),
    #[error("chunk count exceeds u16 range: found {found}", found = .0.found)]
    ChunkCountOverflow(ChunkCountOverflowInfo),
    #[error(
            "header chunk count ({header}) does not match descriptor list ({actual})",
            header = .0.header,
            actual = .0.actual
        )]
    HeaderCountMismatch(HeaderDescriptorCountMismatch),
    #[error("chunk commitment mismatch at index {0}")]
    CommitmentMismatch(u32),
    #[error(
            "descriptor offset mismatch at index {index}: expected {expected}, found {actual}",
            index = .0.index,
            expected = .0.expected,
            actual = .0.actual
        )]
    DescriptorOffsetMismatch(DescriptorOffsetDetails),
    #[error(
            "descriptor length mismatch at index {index}: descriptor {descriptor}, chunk {chunk}",
            index = .0.index,
            descriptor = .0.descriptor,
            chunk = .0.chunk
        )]
    DescriptorLengthMismatch(DescriptorLengthDetails),
    #[error("chunk length at index {0} exceeds u32 range")]
    ChunkLengthOverflow(u32),
    #[error("descriptor offsets overflow while accumulating at index {0}")]
    OffsetOverflow(u32),
    #[error("merkle root mismatch")]
    MerkleMismatch,
    #[error("chunk ids must be strictly ascending")]
    UnsortedChunkIds,
    #[error("segment header expects audio track but none was provided")]
    AudioSummaryMissing,
    #[error("segment carried unexpected audio track not advertised by header")]
    AudioSummaryUnexpected,
    #[error("audio sample rate mismatch: expected {expected}, found {found}", expected = .0.expected, found = .0.found)]
    AudioSampleRateMismatch(AudioSampleRateMismatchInfo),
    #[error("audio frame sample count mismatch: expected {expected}, found {found}", expected = .0.expected, found = .0.found)]
    AudioFrameSamplesMismatch(AudioFrameSamplesMismatchInfo),
    #[error("audio frame duration mismatch: expected {expected}, found {found}", expected = .0.expected, found = .0.found)]
    AudioFrameDurationMismatch(AudioFrameDurationMismatchInfo),
    #[error("audio frame count mismatch: expected {expected}, found {found}", expected = .0.expected, found = .0.found)]
    AudioFrameCountMismatch(AudioFrameCountMismatchInfo),
    #[error("audio FEC level mismatch: expected {expected}, found {found}", expected = .0.expected, found = .0.found)]
    AudioFecMismatch(AudioFecMismatchInfo),
    #[error("audio layout mismatch: expected {expected:?}, found {found:?}", expected = .0.expected, found = .0.found)]
    AudioLayoutMismatch(AudioLayoutMismatchInfo),
    #[error("audio timestamp mismatch at index {index}: expected {expected}, found {found}", index = .0.index, expected = .0.expected, found = .0.found)]
    AudioTimestampMismatch(AudioTimestampMismatchInfo),
    #[error("audio timestamp computation overflow at index {0}")]
    AudioTimestampOverflow(u32),
}
/// Errors emitted when validating a manifest against an encoded segment.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::TypeId))]
pub enum ManifestError {
    #[error(transparent)]
    Segment(#[from] SegmentError),
    #[error("segment number mismatch")]
    SegmentNumberMismatch,
    #[error("profile mismatch")]
    ProfileMismatch,
    #[error("entropy mode mismatch")]
    EntropyModeMismatch,
    #[error("entropy tables checksum mismatch")]
    EntropyTablesMismatch,
    #[error("encryption suite mismatch")]
    EncryptionSuiteMismatch,
    #[error("content key id mismatch")]
    ContentKeyIdMismatch,
    #[error("nonce salt mismatch")]
    NonceSaltMismatch,
    #[error("chunk root mismatch")]
    ChunkRootMismatch,
    #[error("audio summary mismatch between manifest and segment")]
    AudioSummaryMismatch,
    #[error("descriptor count mismatch")]
    DescriptorCountMismatch,
    #[error("descriptor mismatch at index {0}")]
    DescriptorMismatch(u32),
    #[error(
        "manifest capabilities advertise FEATURE_ENTROPY_BUNDLED={found_bundled} but encoder requires {required_bundled}"
    )]
    CapabilityEntropyFlagMismatch {
        /// Whether the encoder expects bundled entropy.
        required_bundled: bool,
        /// Whether the manifest set the bundled capability bit.
        found_bundled: bool,
    },
    #[error(
        "manifest capabilities advertise acceleration mask {found_mask:#06x} but entropy mode {entropy_mode:?} requires mask {required_mask:#06x}"
    )]
    CapabilityAccelerationFlagMismatch {
        /// Manifest entropy mode under verification.
        entropy_mode: EntropyMode,
        /// Acceleration flag mask detected in the manifest.
        found_mask: u32,
        /// Acceleration mask required by the encoder configuration.
        required_mask: u32,
    },
}
#[cfg(feature = "schema-structural")]
#[derive(::iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct CapabilityEntropyFlagMismatchInfo {
    required_bundled: bool,
    found_bundled: bool,
}
#[cfg(feature = "schema-structural")]
#[derive(::iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct CapabilityAccelerationFlagMismatchInfo {
    entropy_mode: EntropyMode,
    found_mask: u32,
    required_mask: u32,
}
#[cfg(feature = "schema-structural")]
impl ::iroha_schema::IntoSchema for ManifestError {
    fn type_name() -> String {
        "ManifestError".to_owned()
    }
    fn update_schema_map(map: &mut ::iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(::iroha_schema::Metadata::Enum(::iroha_schema::EnumMeta {
            variants: vec![
                ::iroha_schema::EnumVariant {
                    tag: "Segment".to_owned(),
                    discriminant: 0,
                    ty: Some(core::any::TypeId::of::<SegmentError>()),
                },
                ::iroha_schema::EnumVariant {
                    tag: "SegmentNumberMismatch".to_owned(),
                    discriminant: 1,
                    ty: None,
                },
                ::iroha_schema::EnumVariant {
                    tag: "ProfileMismatch".to_owned(),
                    discriminant: 2,
                    ty: None,
                },
                ::iroha_schema::EnumVariant {
                    tag: "EntropyModeMismatch".to_owned(),
                    discriminant: 3,
                    ty: None,
                },
                ::iroha_schema::EnumVariant {
                    tag: "EntropyTablesMismatch".to_owned(),
                    discriminant: 4,
                    ty: None,
                },
                ::iroha_schema::EnumVariant {
                    tag: "EncryptionSuiteMismatch".to_owned(),
                    discriminant: 5,
                    ty: None,
                },
                ::iroha_schema::EnumVariant {
                    tag: "ContentKeyIdMismatch".to_owned(),
                    discriminant: 6,
                    ty: None,
                },
                ::iroha_schema::EnumVariant {
                    tag: "NonceSaltMismatch".to_owned(),
                    discriminant: 7,
                    ty: None,
                },
                ::iroha_schema::EnumVariant {
                    tag: "ChunkRootMismatch".to_owned(),
                    discriminant: 8,
                    ty: None,
                },
                ::iroha_schema::EnumVariant {
                    tag: "AudioSummaryMismatch".to_owned(),
                    discriminant: 9,
                    ty: None,
                },
                ::iroha_schema::EnumVariant {
                    tag: "DescriptorCountMismatch".to_owned(),
                    discriminant: 10,
                    ty: None,
                },
                ::iroha_schema::EnumVariant {
                    tag: "DescriptorMismatch".to_owned(),
                    discriminant: 11,
                    ty: Some(core::any::TypeId::of::<u32>()),
                },
                ::iroha_schema::EnumVariant {
                    tag: "CapabilityEntropyFlagMismatch".to_owned(),
                    discriminant: 12,
                    ty: Some(core::any::TypeId::of::<CapabilityEntropyFlagMismatchInfo>()),
                },
                ::iroha_schema::EnumVariant {
                    tag: "CapabilityAccelerationFlagMismatch".to_owned(),
                    discriminant: 13,
                    ty: Some(core::any::TypeId::of::<
                        CapabilityAccelerationFlagMismatchInfo,
                    >()),
                },
            ],
        }));
        <SegmentError as ::iroha_schema::IntoSchema>::update_schema_map(map);
        <u32 as ::iroha_schema::IntoSchema>::update_schema_map(map);
        <CapabilityEntropyFlagMismatchInfo as ::iroha_schema::IntoSchema>::update_schema_map(map);
        <CapabilityAccelerationFlagMismatchInfo as ::iroha_schema::IntoSchema>::update_schema_map(
            map,
        );
    }
}
/// Frame geometry used by the baseline codec (luma-only for now).
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::FrameDimensions")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct FrameDimensions {
    pub width: u16,
    pub height: u16,
}
impl FrameDimensions {
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
    pub const fn pixel_count(self) -> usize {
        self.width as usize * self.height as usize
    }
    const fn align_component(value: u16) -> u16 {
        if value == 0 {
            0
        } else {
            let value_usize = value as usize;
            let aligned = value_usize.div_ceil(BLOCK_SIZE).saturating_mul(BLOCK_SIZE);
            if aligned > u16::MAX as usize {
                u16::MAX
            } else {
                aligned as u16
            }
        }
    }
    pub const fn align_to_block(self) -> Self {
        Self {
            width: Self::align_component(self.width),
            height: Self::align_component(self.height),
        }
    }
    pub const fn block_aligned(self) -> bool {
        (self.width as usize).is_multiple_of(BLOCK_SIZE)
            && (self.height as usize).is_multiple_of(BLOCK_SIZE)
    }
    pub const fn block_count(self) -> usize {
        (self.width as usize / BLOCK_SIZE) * (self.height as usize / BLOCK_SIZE)
    }
}
/// Raw luma frame used by the baseline encoder.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct RawFrame {
    pub luma: Vec<u8>,
}
impl RawFrame {
    pub fn new(dimensions: FrameDimensions, luma: Vec<u8>) -> Result<Self, CodecError> {
        let expected = dimensions.pixel_count();
        if luma.len() != expected {
            return Err(CodecError::InvalidFrameLength(FrameLengthMismatch {
                expected: saturating_usize_to_u32(expected),
                actual: saturating_usize_to_u32(luma.len()),
            }));
        }
        Ok(Self { luma })
    }
}
/// 4:2:0 chroma planes paired with a luma frame.
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::Chroma420Frame")]
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct Chroma420Frame {
    /// U plane in row-major order.
    pub u: Bytes,
    /// V plane in row-major order.
    pub v: Bytes,
}
pub(crate) fn ensure_chroma_even_dimensions(dimensions: FrameDimensions) -> Result<(), CodecError> {
    if !dimensions.width.is_multiple_of(2) || !dimensions.height.is_multiple_of(2) {
        return Err(CodecError::ChromaDimensionsNotEven(
            ChromaDimensionsNotEvenInfo {
                width: dimensions.width,
                height: dimensions.height,
            },
        ));
    }
    Ok(())
}
impl Chroma420Frame {
    /// Construct a chroma frame, validating that both planes match the expected dimensions.
    pub fn new(dimensions: FrameDimensions, u: Bytes, v: Bytes) -> Result<Self, CodecError> {
        ensure_chroma_even_dimensions(dimensions)?;
        let expected = usize::from(dimensions.width / 2) * usize::from(dimensions.height / 2);
        let frame = Self { u, v };
        frame.validate_plane_lengths(expected)?;
        Ok(frame)
    }
    fn validate_plane_lengths(&self, expected: usize) -> Result<(), CodecError> {
        for actual in [self.u.len(), self.v.len()] {
            if actual != expected {
                return Err(CodecError::InvalidFrameLength(FrameLengthMismatch {
                    expected: saturating_usize_to_u32(expected),
                    actual: saturating_usize_to_u32(actual),
                }));
            }
        }
        Ok(())
    }
    /// Build a neutral 4:2:0 chroma frame (U/V filled with 128).
    pub fn neutral(dimensions: FrameDimensions) -> Self {
        debug_assert!(
            dimensions.width.is_multiple_of(2) && dimensions.height.is_multiple_of(2),
            "4:2:0 chroma requires even dimensions"
        );
        let expected = usize::from(dimensions.width / 2) * usize::from(dimensions.height / 2);
        let plane = vec![128u8; expected];
        Self {
            u: plane.clone(),
            v: plane,
        }
    }
}
const AUDIO_SYNC_TOLERANCE_NS: u64 = 10_000_000;
fn checked_frame_count(frame_count: usize) -> Result<u16, CodecError> {
    u16::try_from(frame_count).map_err(|_| {
        CodecError::FrameCountOverflow(FrameCountOverflowInfo {
            max: u32::from(u16::MAX),
            found: saturating_usize_to_u32(frame_count),
        })
    })
}
fn expected_audio_frame_samples(
    sample_rate: u32,
    frame_duration_ns: u32,
) -> Result<u16, CodecError> {
    const NS_PER_SEC: u128 = 1_000_000_000;
    let numerator = u128::from(sample_rate) * u128::from(frame_duration_ns);
    let expected = (numerator + NS_PER_SEC / 2) / NS_PER_SEC;
    if expected > u16::MAX as u128 {
        return Err(CodecError::AudioFrameCadenceOverflow(
            AudioFrameCadenceOverflowInfo {
                expected: expected as u64,
                sample_rate,
                frame_duration_ns,
            },
        ));
    }
    Ok(expected as u16)
}
struct AudioEncoderState {
    encoder: iroha_audio::Encoder,
    sequence: u64,
}
pub struct BaselineEncoder {
    config: BaselineEncoderConfig,
    audio: Option<AudioEncoderState>,
    bundled_telemetry: Option<RecordedSegment>,
    context_slots: Vec<u32>,
    // Allocation can survive a call. Predictor validity is local to its segment.
    reconstruction: [Vec<u8>; 3],
    sparse_prefixes: SparsePrefixes,
    bundle_tables: Arc<BundleAnsTables>,
    neural_predictor: Option<NeuralPredictor>,
}
pub(crate) fn pad_frame_luma(
    input: &[u8],
    dims: FrameDimensions,
    aligned: FrameDimensions,
) -> Vec<u8> {
    if dims == aligned {
        return input.to_vec();
    }
    let dst_w = aligned.width as usize;
    let dst_h = aligned.height as usize;
    let mut out = vec![0u8; dst_w * dst_h];
    let src_w = dims.width as usize;
    let src_h = dims.height as usize;
    if src_w == 0 || src_h == 0 {
        return out;
    }
    for y in 0..src_h {
        let src_row = &input[y * src_w..(y + 1) * src_w];
        let dst_row = &mut out[y * dst_w..(y + 1) * dst_w];
        dst_row[..src_w].copy_from_slice(src_row);
        let fill = src_row.last().copied().unwrap_or(0);
        for value in &mut dst_row[src_w..] {
            *value = fill;
        }
    }
    if src_h < dst_h {
        let last_row = out[(src_h - 1) * dst_w..src_h * dst_w].to_vec();
        for y in src_h..dst_h {
            let dst_row = &mut out[y * dst_w..(y + 1) * dst_w];
            dst_row.copy_from_slice(&last_row);
        }
    }
    out
}
pub(crate) fn crop_frame_luma(
    aligned_data: &[u8],
    dims: FrameDimensions,
    aligned: FrameDimensions,
) -> Vec<u8> {
    if dims == aligned {
        return aligned_data.to_vec();
    }
    let dst_w = dims.width as usize;
    let dst_h = dims.height as usize;
    if dst_w == 0 || dst_h == 0 {
        return Vec::new();
    }
    let src_w = aligned.width as usize;
    let mut out = Vec::with_capacity(dst_w * dst_h);
    for y in 0..dst_h {
        let start = y * src_w;
        out.extend_from_slice(&aligned_data[start..start + dst_w]);
    }
    out
}
impl BaselineEncoder {
    pub fn new(config: BaselineEncoderConfig) -> Self {
        assert!(
            config.entropy_mode.is_bundled(),
            "rANS entropy mode without bundled tables is not supported in the streaming baseline encoder"
        );
        assert!(
            config.bundle_width >= 2,
            "bundled rANS requires bundle_width >= 2"
        );
        let bundle_tables = config.bundle_tables.clone();
        let neural_seed = config.rdo_neural_seed.unwrap_or(DEFAULT_NEURAL_SEED);
        let neural_predictor =
            (config.rdo_mode == RdoMode::Neural).then(|| NeuralPredictor::from_seed(neural_seed));
        Self {
            bundle_tables,
            config,
            audio: None,
            bundled_telemetry: None,
            context_slots: Vec::new(),
            reconstruction: Default::default(),
            sparse_prefixes: Default::default(),
            neural_predictor,
        }
    }
    /// Latest bundled entropy telemetry (stats + token stream) recorded by the encoder.
    #[must_use]
    pub fn bundled_telemetry(&self) -> Option<&BundledTelemetry> {
        self.bundled_telemetry.as_ref().map(|r| &r.telemetry)
    }
    fn ensure_audio_state(
        &mut self,
        audio_cfg: &AudioEncoderConfig,
    ) -> Result<&mut AudioEncoderState, AudioCodecError> {
        if self.audio.is_none() {
            let encoder = iroha_audio::Encoder::new(iroha_audio::EncoderConfig {
                sample_rate: audio_cfg.sample_rate,
                frame_samples: audio_cfg.frame_samples,
                layout: audio_cfg.layout.into(),
                fec_level: audio_cfg.fec_level,
                target_bitrate: audio_cfg.target_bitrate,
                backend: audio_cfg.backend,
            })
            .map_err(AudioCodecError::from_backend)?;
            self.audio = Some(AudioEncoderState {
                encoder,
                sequence: 0,
            });
        }
        Ok(self.audio.as_mut().expect("audio state initialized"))
    }
    fn encode_audio_track(
        &mut self,
        audio_pcm: Option<&[i16]>,
        frame_count: usize,
        timeline_start_ns: u64,
        frame_step: u64,
    ) -> Result<Option<SegmentAudio>, CodecError> {
        let Some(audio_cfg) = self.config.audio else {
            if audio_pcm.is_some() {
                return Err(CodecError::AudioTrackUnexpected);
            }
            return Ok(None);
        };
        if audio_cfg.frame_samples == 0 {
            return Err(CodecError::Audio(AudioCodecError::InvalidSampleCount(
                AudioEncoderSampleCountMismatchInfo {
                    expected: 1,
                    found: 0,
                },
            )));
        }
        let expected_frame_samples =
            expected_audio_frame_samples(audio_cfg.sample_rate, self.config.frame_duration_ns)?;
        if audio_cfg.frame_samples != expected_frame_samples {
            return Err(CodecError::AudioFrameCadenceMismatch(
                AudioFrameCadenceMismatchInfo {
                    expected: expected_frame_samples,
                    found: audio_cfg.frame_samples,
                    sample_rate: audio_cfg.sample_rate,
                    frame_duration_ns: self.config.frame_duration_ns,
                },
            ));
        }
        let samples = audio_pcm.ok_or(CodecError::AudioTrackMissing)?;
        let channels = audio_cfg.channel_count();
        let samples_per_frame = audio_cfg.frame_samples as usize * channels;
        let expected_samples = frame_count
            .checked_mul(samples_per_frame)
            .ok_or(CodecError::AudioSampleCountOverflow)?;
        if samples.len() != expected_samples {
            return Err(CodecError::AudioSampleCountMismatch(
                AudioSampleCountMismatchInfo {
                    expected: saturating_usize_to_u64(expected_samples),
                    found: saturating_usize_to_u64(samples.len()),
                },
            ));
        }
        let state = self
            .ensure_audio_state(&audio_cfg)
            .map_err(CodecError::from)?;
        let mut frames_out = Vec::with_capacity(frame_count);
        for (idx, window) in samples.chunks(samples_per_frame).enumerate() {
            let payload = state
                .encoder
                .encode(window)
                .map_err(AudioCodecError::from_backend)
                .map_err(CodecError::from)?;
            let offset =
                frame_step
                    .checked_mul(idx as u64)
                    .ok_or(CodecError::AudioTimestampOverflow(saturating_usize_to_u32(
                        idx,
                    )))?;
            let timestamp =
                timeline_start_ns
                    .checked_add(offset)
                    .ok_or(CodecError::AudioTimestampOverflow(saturating_usize_to_u32(
                        idx,
                    )))?;
            frames_out.push(AudioFrame {
                sequence: state.sequence,
                timestamp_ns: timestamp,
                fec_level: audio_cfg.fec_level,
                channel_layout: audio_cfg.layout,
                payload,
            });
            state.sequence = state
                .sequence
                .checked_add(1)
                .ok_or(CodecError::AudioSequenceOverflow)?;
        }
        let frames_per_segment =
            u16::try_from(frame_count).map_err(|_| CodecError::AudioFrameCountOverflow)?;
        let summary = AudioTrackSummary {
            sample_rate: audio_cfg.sample_rate,
            frame_samples: audio_cfg.frame_samples,
            frame_duration_ns: self.config.frame_duration_ns,
            frames_per_segment,
            layout: audio_cfg.layout,
            fec_level: audio_cfg.fec_level,
        };
        Ok(Some(SegmentAudio {
            summary,
            frames: frames_out,
        }))
    }
    pub fn encode_segment(
        &mut self,
        segment_number: u64,
        timeline_start_ns: u64,
        content_key_id: u64,
        frames: &[RawFrame],
        audio_pcm: Option<&[i16]>,
    ) -> Result<EncodedSegment, CodecError> {
        self.encode_segment_with_chroma(
            segment_number,
            timeline_start_ns,
            content_key_id,
            frames,
            None,
            audio_pcm,
        )
    }
    pub fn encode_segment_with_chroma(
        &mut self,
        segment_number: u64,
        timeline_start_ns: u64,
        content_key_id: u64,
        frames: &[RawFrame],
        chroma: Option<&[Chroma420Frame]>,
        audio_pcm: Option<&[i16]>,
    ) -> Result<EncodedSegment, CodecError> {
        if frames.is_empty() {
            return Err(CodecError::Segment(SegmentError::Chunk(
                ChunkError::EmptyTree,
            )));
        }
        let frame_count = checked_frame_count(frames.len())?;
        if let Some(chroma_frames) = chroma
            && chroma_frames.len() != frames.len()
        {
            return Err(CodecError::InvalidFrameLength(FrameLengthMismatch {
                expected: saturating_usize_to_u32(frames.len()),
                actual: saturating_usize_to_u32(chroma_frames.len()),
            }));
        }
        if chroma.is_some() {
            ensure_chroma_even_dimensions(self.config.frame_dimensions)?;
        }
        let dims = self.config.frame_dimensions;
        let frame_len = dims.pixel_count();
        let aligned_dims = dims.align_to_block();
        let blocks_per_frame = aligned_dims.block_count();
        let chunk_count_u16: u16 = blocks_per_frame.try_into().map_err(|_| {
            CodecError::BlockCountMismatch(BlockCountMismatchInfo {
                expected: saturating_usize_to_u32(blocks_per_frame),
                found: u32::MAX,
            })
        })?;
        let frame_step = u64::from(self.config.frame_duration_ns);
        let chroma_frames = chroma.unwrap_or(&[]);
        let chroma_dims = FrameDimensions::new(dims.width / 2, dims.height / 2);
        let chroma_aligned = chroma_dims.align_to_block();
        let mut chunks = Vec::with_capacity(frames.len());
        let [luma_ref, u_ref, v_ref] = &mut self.reconstruction;
        let mut bundler = {
            let width = self
                .config
                .bundle_width
                .min(self.bundle_tables.max_width())
                .max(1);
            BundleStreamRecorder::new(
                &mut self.context_slots,
                width,
                aligned_dims,
                self.config.quantizer,
                self.bundle_tables.clone(),
                self.config.bundle_acceleration,
                self.config.bundle_prefetch_distance,
                self.config.bundle_context_remap.clone(),
            )
        };
        bundler.cached_prefixes = Some(&mut self.sparse_prefixes);
        let mut rdo = BlockRdo::new(
            self.config.rdo_mode,
            self.config.quantizer,
            self.config.bundle_width,
            self.neural_predictor.as_ref(),
        );
        let mut noop_hooks = NoopBundledHooks;
        let mut chunk = Vec::new();
        let mut compressor = None;
        for (idx, frame) in frames.iter().enumerate() {
            if frame.luma.len() != frame_len {
                return Err(CodecError::InvalidFrameLength(FrameLengthMismatch {
                    expected: saturating_usize_to_u32(frame_len),
                    actual: saturating_usize_to_u32(frame.luma.len()),
                }));
            }
            let luma = padded_plane(&frame.luma, dims, aligned_dims);
            let chroma = chroma_frames
                .get(idx)
                .map(|planes| -> Result<_, CodecError> {
                    planes.validate_plane_lengths(chroma_dims.pixel_count())?;
                    Ok((
                        padded_plane(&planes.u, chroma_dims, chroma_aligned),
                        padded_plane(&planes.v, chroma_dims, chroma_aligned),
                    ))
                })
                .transpose()?;
            // Plane storage survives the call. Prediction starts with frame 1 of each segment.
            let frame_type = if idx == 0 {
                FrameType::Intra
            } else if self.config.quantizer == 0 {
                FrameType::Predicted
            } else {
                let mut costs = PredictionEnergy::default();
                costs.add_plane(&luma, luma_ref, aligned_dims);
                if let Some((u, v)) = &chroma {
                    costs.add_plane(u, u_ref, chroma_aligned);
                    costs.add_plane(v, v_ref, chroma_aligned);
                }
                costs.frame_type()
            };
            let idx_delta = frame_step
                .checked_mul(idx as u64)
                .ok_or(CodecError::FramePtsOverflow(saturating_usize_to_u32(idx)))?;
            let pts = timeline_start_ns
                .checked_add(idx_delta)
                .ok_or(CodecError::FramePtsOverflow(saturating_usize_to_u32(idx)))?;
            // Prediction references live only until the next frame in this segment.
            let retain_reference = idx + 1 < frames.len();
            let plane_blocks = blocks_per_frame
                + chroma
                    .as_ref()
                    .map_or(0, |_| 2 * chroma_aligned.block_count());
            chunk.clear();
            chunk.reserve(FRAME_HEADER_LEN + 8 + plane_blocks * 8);
            chunk.extend_from_slice(&(idx as u32).to_le_bytes());
            chunk.extend_from_slice(&pts.to_le_bytes());
            chunk.push(frame_type.as_byte());
            chunk.push(self.config.quantizer);
            chunk.extend_from_slice(&chunk_count_u16.to_le_bytes());
            Self::encode_frame_payload(
                &mut chunk,
                &luma,
                frame_type,
                luma_ref,
                aligned_dims,
                self.config.quantizer,
                retain_reference,
                &mut bundler,
                rdo.as_mut(),
            );
            if let Some((u, v)) = chroma {
                // The wire format places both plane lengths before either payload.
                let lengths = chunk.len();
                chunk.extend_from_slice(&[0; 8]);
                let u_start = chunk.len();
                Self::encode_frame_payload(
                    &mut chunk,
                    &u,
                    frame_type,
                    u_ref,
                    chroma_aligned,
                    self.config.quantizer,
                    retain_reference,
                    &mut noop_hooks,
                    None,
                );
                let v_start = chunk.len();
                Self::encode_frame_payload(
                    &mut chunk,
                    &v,
                    frame_type,
                    v_ref,
                    chroma_aligned,
                    self.config.quantizer,
                    retain_reference,
                    &mut noop_hooks,
                    None,
                );
                let u_len = (v_start - u_start) as u32;
                let v_len = (chunk.len() - v_start) as u32;
                chunk[lengths..lengths + 4].copy_from_slice(&u_len.to_le_bytes());
                chunk[lengths + 4..u_start].copy_from_slice(&v_len.to_le_bytes());
            }
            chunks.push(compress_frame_body(&mut chunk, &mut compressor)?);
            #[cfg(feature = "streaming-neural-filter")]
            if retain_reference {
                apply_neural_filter(luma_ref, aligned_dims);
            }
        }
        let mut recorded = bundler.finish_segment(self.bundled_telemetry.take());
        recorded.telemetry.rdo = rdo.map(|optimizer| optimizer.telemetry);
        self.bundled_telemetry = Some(recorded);
        let offsets_and_lengths = compute_offsets(&chunks);
        let chunk_ids: Vec<u16> = (0..frame_count).collect();
        let nonce_salt = derive_nonce_salt(segment_number, frames.len(), &chunks);
        let payload_refs: Vec<(u16, &[u8])> = chunk_ids
            .iter()
            .zip(chunks.iter())
            .map(|(id, payload)| (*id, payload.as_slice()))
            .collect();
        let commitments = chunk_commitments(segment_number, &payload_refs);
        let root = merkle_root(&commitments)
            .map_err(SegmentError::from)
            .map_err(CodecError::from)?;
        let audio =
            self.encode_audio_track(audio_pcm, frames.len(), timeline_start_ns, frame_step)?;
        let descriptors: Vec<ChunkDescriptor> = offsets_and_lengths
            .into_iter()
            .zip(commitments.iter())
            .zip(chunk_ids.iter())
            .map(|((meta, commitment), chunk_id)| ChunkDescriptor {
                chunk_id: *chunk_id,
                offset: meta.offset,
                length: meta.length,
                commitment: *commitment,
                parity: false,
            })
            .collect();
        let entropy_tables_checksum = self
            .config
            .entropy_mode
            .is_bundled()
            .then(|| self.bundle_tables.checksum());
        let header = SegmentHeader {
            segment_number,
            profile: self.config.profile,
            entropy_mode: self.config.entropy_mode,
            entropy_tables_checksum,
            encryption_suite: self.config.encryption_suite,
            layer_bitmap: self.config.layer_bitmap,
            chunk_merkle_root: root,
            chunk_count: frame_count,
            timeline_start_ns,
            duration_ns: if self.config.duration_ns == 0 {
                self.config
                    .frame_duration_ns
                    .saturating_mul(frames.len() as u32)
                    .max(1)
            } else {
                self.config.duration_ns
            },
            feedback_hint: self.config.feedback_hint.clone(),
            content_key_id,
            nonce_salt,
            storage_class: self.config.storage_class,
            audio_summary: audio.as_ref().map(|track| track.summary),
            bundle_acceleration: self.config.bundle_acceleration,
        };
        Ok(EncodedSegment {
            header,
            descriptors,
            chunks,
            audio,
        })
    }
    #[allow(clippy::too_many_arguments)]
    fn encode_frame_payload(
        payload: &mut Vec<u8>,
        frame: &[u8],
        frame_type: FrameType,
        reference: &mut Vec<u8>,
        dims: FrameDimensions,
        quantizer: u8,
        retain_reference: bool,
        hooks: &mut impl BundledHooks,
        rdo: Option<&mut BlockRdo<'_>>,
    ) {
        if quantizer == 16 && rdo.is_none() {
            Self::encode_plane::<true>(
                payload,
                frame,
                frame_type,
                reference,
                dims,
                16,
                retain_reference,
                hooks,
                None,
            );
        } else {
            Self::encode_plane::<false>(
                payload,
                frame,
                frame_type,
                reference,
                dims,
                quantizer,
                retain_reference,
                hooks,
                rdo,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn encode_plane<const DEFAULT_Q16: bool>(
        payload: &mut Vec<u8>,
        frame: &[u8],
        frame_type: FrameType,
        reference: &mut Vec<u8>,
        dims: FrameDimensions,
        quantizer: u8,
        retain_reference: bool,
        hooks: &mut impl BundledHooks,
        rdo: Option<&mut BlockRdo<'_>>,
    ) {
        let quantizer = if DEFAULT_Q16 { 16 } else { quantizer };
        let mut rdo = if DEFAULT_Q16 { None } else { rdo };
        debug_assert_eq!(frame.len(), dims.pixel_count());
        // Prediction reads only the collocated block. Once captured, its old
        // pixels can be replaced in place, including on a scene refresh.
        if retain_reference {
            reference.resize(dims.pixel_count(), 0);
        }
        let mut prev_dc = 0i16;
        let mut dc_levels = [0; BLOCK_PIXELS];
        let geometry = BlockQuantization::new(quantizer);
        let stride = usize::from(dims.width);
        let blocks_per_row = stride / BLOCK_SIZE;
        let mut moments = vec![(0, 0); blocks_per_row];
        let mut next_block = 0;
        for band in 0..usize::from(dims.height) / BLOCK_SIZE {
            let band_start = band * BLOCK_SIZE * stride;
            if quantizer != 0 {
                let range = band_start..band_start + BLOCK_SIZE * stride;
                let prediction =
                    (frame_type == FrameType::Predicted).then_some(reference.as_slice());
                let source = &frame[range.clone()];
                let reference = prediction.map(|p| &p[range]);
                #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
                let shortcut = if let Some(prev) = reference
                    && std::arch::is_aarch64_feature_detected!("dotprod")
                {
                    // SAFETY: the CPU supports dotprod and the plane slices contain the whole band.
                    unsafe {
                        predicted_moments(
                            source,
                            prev,
                            stride,
                            geometry.zero_squares,
                            &mut moments,
                        );
                    }
                    true
                } else {
                    false
                };
                #[cfg(not(all(feature = "simd-accel", target_arch = "aarch64")))]
                let shortcut = false;
                if !shortcut {
                    row_moments(source, reference, stride, &mut moments);
                }
            }
            let skip_zero = frame_type == FrameType::Predicted && quantizer != 0 && rdo.is_none();
            for (chunk, positions) in moments.chunks(64).enumerate() {
                let mut selected = if skip_zero {
                    geometry.possible_residuals(positions)
                } else {
                    u64::MAX >> (64 - positions.len())
                };
                while selected != 0 {
                    let position = selected.trailing_zeros() as usize;
                    selected &= selected - 1;
                    let column = chunk * 64 + position;
                    let moments = positions[position];
                    let block_index = band * blocks_per_row + column;
                    let prediction =
                        (frame_type == FrameType::Predicted).then_some(reference.as_slice());
                    let decision = geometry.classify(frame, prediction, dims, block_index, moments);
                    if matches!(decision, BlockSpectrum::DcLevel(0))
                        && frame_type == FrameType::Predicted
                        && rdo.is_none()
                    {
                        continue;
                    }
                    if next_block < block_index {
                        emit_zero_run(next_block..block_index, &mut prev_dc, payload, hooks);
                    }
                    next_block = block_index + 1;
                    match decision {
                        BlockSpectrum::DcLevel(level) => {
                            dc_levels[0] = level;
                            // RDO observes certified blocks even when all AC levels are zero.
                            if let Some(optimizer) = rdo.as_deref_mut() {
                                optimizer.select_levels(
                                    &[0; BLOCK_PIXELS],
                                    &mut dc_levels,
                                    frame_type,
                                    block_index,
                                );
                            }
                        }
                        BlockSpectrum::WeightsThrough(limit) => {
                            let start = band_start + column * BLOCK_SIZE;
                            let rows = ResidualRows::new(frame, prediction, stride, start);
                            let (quantized, active) = if let Some(optimizer) = rdo.as_deref_mut() {
                                let coeffs = forward_dct(&rows.block(), limit, quantizer);
                                let mut levels = quantize_coeffs(&coeffs, quantizer);
                                optimizer.select_levels(
                                    &coeffs,
                                    &mut levels,
                                    frame_type,
                                    block_index,
                                );
                                (levels, ac_support(&levels))
                            } else {
                                geometry.quantize_rows(&rows, moments, limit)
                            };
                            if active != 0 {
                                encode_active_block(
                                    &quantized,
                                    active,
                                    &mut prev_dc,
                                    payload,
                                    hooks,
                                    block_index,
                                    frame_type,
                                );
                                if retain_reference {
                                    let spatial =
                                        inverse_dct_full(&dequantize_coeffs(&quantized, quantizer));
                                    reconstruct_ac(reference, stride, start, frame_type, &spatial);
                                }
                                continue;
                            }
                            dc_levels[0] = quantized[0];
                        }
                    }
                    emit_dc_block(
                        &dc_levels,
                        &mut prev_dc,
                        payload,
                        hooks,
                        block_index,
                        frame_type,
                    );
                    if retain_reference && (frame_type == FrameType::Intra || dc_levels[0] != 0) {
                        let offset = inverse_dc_sample(i32::from(dc_levels[0]) * geometry.dc_step);
                        reconstruct_dc(reference, dims, block_index, frame_type, offset);
                    }
                }
            }
        }
        if next_block < dims.block_count() {
            emit_zero_run(next_block..dims.block_count(), &mut prev_dc, payload, hooks);
        }
    }
}
/// Pack the whole plane payload before computing chunk offsets and commitments.
fn compress_frame_body(
    chunk: &mut Vec<u8>,
    compressor: &mut Option<zstd::bulk::Compressor<'static>>,
) -> Result<Vec<u8>, CodecError> {
    let body = &chunk[FRAME_HEADER_LEN..];
    if body.len() >= MIN_FRAME_COMPRESSION_BYTES {
        let compressor = match compressor {
            Some(compressor) => compressor,
            None => compressor.insert(
                zstd::bulk::Compressor::new(1).map_err(|_| CodecError::FrameCompressionFailed)?,
            ),
        };
        // Write the independent Zstandard frame directly after its uncompressed header.
        let mut packed =
            Vec::with_capacity(FRAME_HEADER_LEN + zstd::zstd_safe::compress_bound(body.len()));
        packed.extend_from_slice(&chunk[..FRAME_HEADER_LEN]);
        let mut cursor = std::io::Cursor::new(&mut packed);
        cursor.set_position(FRAME_HEADER_LEN as u64);
        let written = compressor
            .compress_to_buffer(body, &mut cursor)
            .map_err(|_| CodecError::FrameCompressionFailed)?;
        if written < body.len() {
            packed[12] |= FRAME_ZSTD;
            packed.shrink_to_fit();
            return Ok(packed);
        }
    }
    Ok(std::mem::take(chunk))
}

/// Open a single sized Zstandard frame within the geometry's largest possible RLE payload.
pub(super) fn decompress_frame_body(
    body: &[u8],
    luma_blocks: usize,
    chroma_blocks: usize,
) -> Result<Vec<u8>, CodecError> {
    // Each block holds a DC delta, up to 63 AC records and an end marker.
    let max_bytes = (luma_blocks + 2 * chroma_blocks) * (2 + 3 * BLOCK_PIXELS) + 8;
    let min_bytes = (5 * luma_blocks).max(1);
    let size = zstd::decompressed_size(body)
        .and_then(|size| usize::try_from(size).ok())
        .filter(|size| (min_bytes..=max_bytes).contains(size))
        .ok_or(CodecError::InvalidCompressedFrame)?;
    if zstd::zstd_safe::find_frame_compressed_size(body) != Ok(body.len()) {
        return Err(CodecError::InvalidCompressedFrame);
    }
    let decoded =
        zstd::bulk::decompress(body, size).map_err(|_| CodecError::InvalidCompressedFrame)?;
    if decoded.len() != size {
        return Err(CodecError::InvalidCompressedFrame);
    }
    Ok(decoded)
}

/// Borrow an aligned input plane or extend its edge pixels once for analysis and encoding.
fn padded_plane(input: &[u8], dims: FrameDimensions, aligned: FrameDimensions) -> Cow<'_, [u8]> {
    if dims == aligned {
        Cow::Borrowed(input)
    } else {
        Cow::Owned(pad_frame_luma(input, dims, aligned))
    }
}

#[derive(Default)]
struct PredictionEnergy {
    intra: u64,
    predicted: u64,
}

impl PredictionEnergy {
    // The DC delta always costs two bytes. Estimate AC energy, i.e. variance
    // about each block's own mean, for the source and previous-frame residual.
    fn add_plane(&mut self, source: &[u8], previous: &[u8], dims: FrameDimensions) {
        let stride = usize::from(dims.width);
        let cols = stride / BLOCK_SIZE;
        let rows = usize::from(dims.height) / BLOCK_SIZE;
        let grid_x = cols.min(4);
        let grid_y = rows.min(4);
        for y in 0..grid_y {
            let top = y * rows / grid_y;
            let bottom = (y + 1) * rows / grid_y;
            for x in 0..grid_x {
                let left = x * cols / grid_x;
                let right = (x + 1) * cols / grid_x;
                let offset = ((top + bottom) / 2 * stride + (left + right) / 2) * BLOCK_SIZE;
                let weight = ((right - left) * (bottom - top)) as u64;
                let (mut sum, mut squares) = (0_i32, 0_i32);
                let (mut diff_sum, mut diff_squares) = (0_i32, 0_i32);
                for dy in 0..BLOCK_SIZE {
                    let start = offset + dy * stride;
                    for i in start..start + BLOCK_SIZE {
                        let pixel = i32::from(source[i]);
                        let diff = pixel - i32::from(previous[i]);
                        sum += pixel;
                        squares += pixel * pixel;
                        diff_sum += diff;
                        diff_squares += diff * diff;
                    }
                }
                self.intra += weight * (BLOCK_PIXELS as i32 * squares - sum * sum) as u64;
                self.predicted +=
                    weight * (BLOCK_PIXELS as i32 * diff_squares - diff_sum * diff_sum) as u64;
            }
        }
    }

    fn frame_type(&self) -> FrameType {
        // A margin avoids switching references on small changes in sampled texture.
        if 5 * self.intra < 4 * self.predicted {
            FrameType::Intra
        } else {
            FrameType::Predicted
        }
    }
}
pub(crate) fn block_origin(dims: FrameDimensions, block_index: usize) -> (usize, usize) {
    let blocks_per_row = dims.width as usize / BLOCK_SIZE;
    let row = block_index / blocks_per_row;
    let col = block_index % blocks_per_row;
    (col * BLOCK_SIZE, row * BLOCK_SIZE)
}
/// A collocated 8×8 window in the source and optional prediction planes.
struct ResidualRows<'a> {
    source: &'a [u8],
    reference: Option<&'a [u8]>,
    stride: usize,
}

impl<'a> ResidualRows<'a> {
    fn new(source: &'a [u8], reference: Option<&'a [u8]>, stride: usize, start: usize) -> Self {
        let span = start..start + (BLOCK_SIZE - 1) * stride + BLOCK_SIZE;
        Self {
            source: &source[span.clone()],
            reference: reference.map(|r| &r[span]),
            stride,
        }
    }

    #[inline]
    fn row(&self, y: usize) -> [i16; BLOCK_SIZE] {
        let offset = y * self.stride;
        let src = self.source[offset..]
            .first_chunk::<BLOCK_SIZE>()
            .expect("complete source row");
        match self.reference {
            Some(reference) => {
                let pred = reference[offset..]
                    .first_chunk::<BLOCK_SIZE>()
                    .expect("complete reference row");
                std::array::from_fn(|x| i16::from(src[x]) - i16::from(pred[x]))
            }
            None => src.map(i16::from),
        }
    }

    #[cfg(all(
        feature = "simd-accel",
        target_arch = "aarch64",
        not(feature = "streaming-fixed-point-dct")
    ))]
    #[target_feature(enable = "dotprod")]
    unsafe fn narrow_q16(
        &self,
        moments: (i32, i32),
        limit: u8,
        geometry: &BlockQuantization,
    ) -> ([i16; 64], u64) {
        use std::arch::aarch64::*;
        let (sum, squares) = moments;
        let mut remaining = (64 * squares - sum * sum + 63) / 64 + 1;
        let half = self.reference.is_some() && limit > 15;
        let center = (sum / 64) as u8;
        // SAFETY: each plane contains all eight rows established by new().
        let pair = |p: &[u8], y: usize| unsafe {
            vcombine_u8(
                vld1_u8(p.as_ptr().add(2 * y * self.stride)),
                vld1_u8(p.as_ptr().add((2 * y + 1) * self.stride)),
            )
        };
        let samples: [_; 4] = std::array::from_fn(|i| {
            let s = pair(self.source, i);
            let v = if let Some(p) = self.reference {
                let prev = pair(p, i);
                if half {
                    vsubq_u8(vhsubq_u8(s, prev), vdupq_n_u8(((sum / 64) >> 1) as u8))
                } else {
                    vsubq_u8(vsubq_u8(s, prev), vdupq_n_u8(center))
                }
            } else {
                veorq_u8(s, vdupq_n_u8(128))
            };
            vreinterpretq_s8_u8(v)
        });
        let mut levels = [0; 64];
        let mut active = 0;
        for kernel in &NARROW_KERNELS {
            if kernel.weight > limit || remaining <= (8 * i32::from(kernel.weight)).pow(2) {
                break;
            }
            let mut d = vdupq_n_s32(0);
            for (r, x) in kernel.rows.iter().zip(samples) {
                // SAFETY: every pair contains sixteen signed byte weights.
                d = vdotq_s32(d, x, unsafe { vld1q_s8(r.as_ptr()) });
            }
            let v = vaddvq_s32(d) << u32::from(half);
            let mag = v.unsigned_abs();
            let error =
                32 * (u32::from(limit) + 1) + 1 + if half { kernel.l1.div_ceil(2) } else { 0 };
            let lower = (mag.saturating_sub(error) >> 9) as i32;
            remaining -= lower * lower;
            let midpoint = u32::from(kernel.weight) * 4096 + 256;
            if mag + error < midpoint {
                continue;
            }
            let slot = kernel.slot;
            let level = if mag > midpoint + error {
                if v < 0 { -1 } else { 1 }
            } else {
                quantize_ac(
                    direct_dct_coefficient(
                        &self.block(),
                        &DCT_FACTORS[slot / 8],
                        &DCT_FACTORS[slot % 8],
                    ),
                    i32::from(kernel.weight) * 16,
                )
            };
            levels[slot] = level;
            active |= u64::from(level != 0) * ZIGZAG_BIT[slot];
        }
        levels[0] = geometry.quantized_dc(i64::from(sum), || self.block());
        (levels, active)
    }

    fn folded(&self) -> FoldedBlock {
        #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
        {
            use std::arch::aarch64::*;
            // SAFETY: ResidualRows::new checked both planes through the end of row seven.
            // The fold requests exactly those eight complete rows.
            let load =
                |plane: &[u8], y: usize| unsafe { vld1_u8(plane.as_ptr().add(y * self.stride)) };
            if let Some(reference) = self.reference {
                fold_vectors(|y| unsafe {
                    vreinterpretq_s16_u16(vsubl_u8(load(self.source, y), load(reference, y)))
                })
            } else {
                fold_vectors(|y| unsafe { vreinterpretq_s16_u16(vmovl_u8(load(self.source, y))) })
            }
        }
        #[cfg(not(all(feature = "simd-accel", target_arch = "aarch64")))]
        {
            fold_scalar(|y| self.row(y))
        }
    }

    fn block(&self) -> [i16; BLOCK_PIXELS] {
        let mut block = [0; BLOCK_PIXELS];
        for (y, row) in block.as_chunks_mut::<BLOCK_SIZE>().0.iter_mut().enumerate() {
            *row = self.row(y);
        }
        block
    }
}

#[cfg(test)]
fn build_residual_block(
    frame: &[u8],
    prev: Option<&[u8]>,
    dims: FrameDimensions,
    index: usize,
    kind: FrameType,
) -> ([i16; BLOCK_PIXELS], [i16; BLOCK_PIXELS]) {
    let (x, y) = block_origin(dims, index);
    let stride = usize::from(dims.width);
    let rows = ResidualRows::new(
        frame,
        prev.filter(|_| kind == FrameType::Predicted),
        stride,
        y * stride + x,
    );
    (rows.block(), predictor_block(prev, dims, index, kind))
}

pub(crate) fn predictor_block(
    prev_frame: Option<&[u8]>,
    dims: FrameDimensions,
    block_index: usize,
    frame_type: FrameType,
) -> [i16; BLOCK_PIXELS] {
    if let (FrameType::Predicted, Some(prev)) = (frame_type, prev_frame) {
        let mut predictor = [0i16; BLOCK_PIXELS];
        let (origin_x, origin_y) = block_origin(dims, block_index);
        let stride = dims.width as usize;
        for y in 0..BLOCK_SIZE {
            for x in 0..BLOCK_SIZE {
                let idx = (origin_y + y) * stride + (origin_x + x);
                predictor[y * BLOCK_SIZE + x] = prev[idx] as i16;
            }
        }
        predictor
    } else {
        [0i16; BLOCK_PIXELS]
    }
}
pub(crate) fn write_reconstructed_block(
    reconstructed: &mut [u8],
    spatial: &[i32; BLOCK_PIXELS],
    predictor: &[i16; BLOCK_PIXELS],
    dims: FrameDimensions,
    block_index: usize,
) {
    let (origin_x, origin_y) = block_origin(dims, block_index);
    let stride = dims.width as usize;
    for y in 0..BLOCK_SIZE {
        for x in 0..BLOCK_SIZE {
            let idx = (origin_y + y) * stride + (origin_x + x);
            let sample_idx = y * BLOCK_SIZE + x;
            let predicted = predictor[sample_idx] as i32;
            reconstructed[idx] = clamp_pixel(spatial[sample_idx] + predicted);
        }
    }
}
#[cfg(test)]
mod block_tests {
    use super::*;
    fn linear_frame(dims: FrameDimensions, offset: u8) -> Vec<u8> {
        (0..dims.pixel_count())
            .map(|idx| offset.wrapping_add(idx as u8))
            .collect()
    }
    fn block_pixels(frame: &[u8], dims: FrameDimensions, block_index: usize) -> Vec<u8> {
        let (origin_x, origin_y) = block_origin(dims, block_index);
        let stride = dims.width as usize;
        let mut out = Vec::with_capacity(BLOCK_PIXELS);
        for y in 0..BLOCK_SIZE {
            for x in 0..BLOCK_SIZE {
                let idx = (origin_y + y) * stride + (origin_x + x);
                out.push(frame[idx]);
            }
        }
        out
    }
    #[test]
    fn block_origin_maps_index_to_coordinates() {
        let dims = FrameDimensions::new(16, 16);
        assert_eq!(block_origin(dims, 0), (0, 0));
        assert_eq!(block_origin(dims, 1), (BLOCK_SIZE, 0));
        assert_eq!(block_origin(dims, 2), (0, BLOCK_SIZE));
        assert_eq!(block_origin(dims, 3), (BLOCK_SIZE, BLOCK_SIZE));
    }
    #[test]
    fn build_residual_block_tracks_previous_frame_for_predicted() {
        let dims = FrameDimensions::new(16, 8);
        let prev = linear_frame(dims, 5);
        let current: Vec<u8> = prev.iter().map(|value| value.saturating_add(2)).collect();
        let (residual, predictor) =
            build_residual_block(&current, Some(&prev), dims, 1, FrameType::Predicted);
        assert!(residual.iter().all(|&value| value == 2));
        let expected_block = block_pixels(&prev, dims, 1);
        for (actual, expected) in predictor.iter().zip(expected_block) {
            assert_eq!(*actual, i16::from(expected));
        }
    }
    #[test]
    fn build_residual_block_resets_predictor_for_intra_frames() {
        let dims = FrameDimensions::new(8, 8);
        let current = linear_frame(dims, 10);
        let prev = linear_frame(dims, 3);
        let (residual, predictor) =
            build_residual_block(&current, Some(&prev), dims, 0, FrameType::Intra);
        assert!(predictor.iter().all(|&value| value == 0));
        for (idx, &value) in residual.iter().enumerate() {
            assert_eq!(value, i16::from(current[idx]));
        }
    }
    #[test]
    fn predictor_block_only_uses_previous_frame_for_predicted() {
        let dims = FrameDimensions::new(16, 8);
        let prev = linear_frame(dims, 7);
        let predicted = predictor_block(Some(&prev), dims, 1, FrameType::Predicted);
        let expected_block = block_pixels(&prev, dims, 1);
        for (actual, expected) in predicted.iter().zip(expected_block) {
            assert_eq!(*actual, i16::from(expected));
        }
        let intra = predictor_block(Some(&prev), dims, 1, FrameType::Intra);
        assert!(intra.iter().all(|&value| value == 0));
    }
    #[test]
    fn write_reconstructed_block_clamps_and_combines_samples() {
        let dims = FrameDimensions::new(8, 8);
        let mut reconstructed = vec![0u8; dims.pixel_count()];
        let mut spatial = [20i32; BLOCK_PIXELS];
        spatial[0] = 400;
        spatial[1] = -400;
        let mut predictor = [10i16; BLOCK_PIXELS];
        predictor[2] = 5;
        write_reconstructed_block(&mut reconstructed, &spatial, &predictor, dims, 0);
        assert_eq!(reconstructed[0], 255, "values above 255 must clamp");
        assert_eq!(reconstructed[1], 0, "values below 0 must clamp");
        assert_eq!(reconstructed[2], 25);
        assert_eq!(reconstructed[3], 30);
    }
    #[cfg(feature = "streaming-neural-filter")]
    #[test]
    fn neural_filter_is_deterministic_and_effectful() {
        let dims = FrameDimensions::new(4, 4);
        let mut frame: Vec<u8> = (0..dims.pixel_count())
            .map(|idx| (idx as u8).saturating_mul(3))
            .collect();
        let mut second = frame.clone();
        apply_neural_filter(&mut frame, dims);
        apply_neural_filter(&mut second, dims);
        assert_eq!(frame, second, "neural filter must be deterministic");
        let untouched: Vec<u8> = (0..dims.pixel_count())
            .map(|idx| (idx as u8).saturating_mul(3))
            .collect();
        assert_ne!(frame, untouched, "neural filter should change the frame");
    }
}
#[cfg(feature = "streaming-fixed-point-dct")]
fn round_q30(sum: i64) -> i64 {
    if sum >= 0 {
        (sum + DCT_ROUND) >> DCT_SHIFT
    } else {
        (sum - DCT_ROUND) >> DCT_SHIFT
    }
}

pub(crate) fn forward_dct(
    block: &[i16; BLOCK_PIXELS],
    limit: u8,
    quantizer: u8,
) -> [i32; BLOCK_PIXELS] {
    let mut coeffs = [0; BLOCK_PIXELS];
    if quantizer == 0 && block.iter().all(|&sample| sample == 0) {
        return coeffs;
    }
    let rows = block.as_chunks::<BLOCK_SIZE>().0;
    let folded = fold_rows(|y| rows[y]);
    let sum = folded_sum(&folded);
    visit_ac(
        &folded,
        sum,
        limit,
        quantizer,
        false,
        || *block,
        |slot, value, _| coeffs[slot] = value,
    );
    #[cfg(feature = "streaming-fixed-point-dct")]
    let dc = {
        let basis = i64::from(DCT_FACTORS_Q15[0][0]);
        round_q30(i64::from(sum) * basis * basis) as i32
    };
    #[cfg(not(feature = "streaming-fixed-point-dct"))]
    let dc = {
        // The constant basis makes DC equal to sum/8 up to rounding. Only
        // a half-integer can depend on the original table and addition order.
        if sum & 7 == 4 {
            direct_dct_coefficient(block, &DCT_FACTORS[0], &DCT_FACTORS[0])
        } else {
            (sum + 4) >> 3
        }
    };
    coeffs[0] = dc;
    coeffs
}

/// All 64 residuals lie in [-255, 255], so their squared sum fits in i32.
fn block_moments(
    samples: &[u8],
    prediction: Option<&[u8]>,
    stride: usize,
    column: usize,
) -> (i32, i32) {
    let x = column * BLOCK_SIZE;
    let mut sums = [0_i32; BLOCK_SIZE];
    let mut squares = [0_i32; BLOCK_SIZE];
    for dy in 0..BLOCK_SIZE {
        let start = dy * stride + x;
        let samples = samples[start..]
            .first_chunk::<BLOCK_SIZE>()
            .expect("complete block row");
        let predictors = prediction.map_or(&[0; BLOCK_SIZE], |prev| {
            prev[start..]
                .first_chunk::<BLOCK_SIZE>()
                .expect("complete prediction row")
        });
        for dx in 0..BLOCK_SIZE {
            let delta = i32::from(samples[dx]) - i32::from(predictors[dx]);
            sums[dx] += delta;
            squares[dx] += delta * delta;
        }
    }
    (sums.iter().sum(), squares.iter().sum())
}

/// Compute the moments before any collocated predictor in this band is overwritten.
fn row_moments(
    samples: &[u8],
    prediction: Option<&[u8]>,
    stride: usize,
    output: &mut [(i32, i32)],
) {
    #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
    if std::arch::is_aarch64_feature_detected!("dotprod") {
        // SAFETY: the selected CPU supports the integer dot-product instructions.
        unsafe { dot_row_moments(samples, prediction, stride, output) };
        return;
    }
    for (column, moments) in output.iter_mut().enumerate() {
        *moments = block_moments(samples, prediction, stride, column);
    }
}

/// Two adjacent 8×8 blocks occupy the two halves of each vector load. Dot the
/// absolute differences with themselves, then add neighboring four-byte lanes.
#[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
#[target_feature(enable = "dotprod")]
unsafe fn dot_row_moments(
    samples: &[u8],
    reference: Option<&[u8]>,
    stride: usize,
    row: &mut [(i32, i32)],
) {
    use std::arch::aarch64::*;
    assert!(row.len() <= stride / BLOCK_SIZE);
    let band_len = stride.checked_mul(BLOCK_SIZE).expect("valid band length");
    let samples = &samples[..band_len];
    let reference = reference.map(|p| &p[..band_len]);
    let ones = vdupq_n_u8(1);
    for (pair, dst) in row.chunks_exact_mut(2).enumerate() {
        let mut sum = vdupq_n_u32(0);
        let mut prev_sum = vdupq_n_u32(0);
        let mut squares = vdupq_n_u32(0);
        for y in 0..BLOCK_SIZE {
            let offset = y * stride + pair * 2 * BLOCK_SIZE;
            // SAFETY: the pair contains 16 bytes within a checked eight-row band.
            let pixels = unsafe { vld1q_u8(samples.as_ptr().add(offset)) };
            sum = vdotq_u32(sum, pixels, ones);
            let magnitude = if let Some(previous) = reference {
                // SAFETY: the reference band has the same checked length.
                let predictors = unsafe { vld1q_u8(previous.as_ptr().add(offset)) };
                prev_sum = vdotq_u32(prev_sum, predictors, ones);
                vabdq_u8(pixels, predictors)
            } else {
                pixels
            };
            squares = vdotq_u32(squares, magnitude, magnitude);
        }
        let totals = vpaddq_s32(
            vreinterpretq_s32_u32(vsubq_u32(sum, prev_sum)),
            vreinterpretq_s32_u32(squares),
        );
        dst[0] = (vgetq_lane_s32::<0>(totals), vgetq_lane_s32::<2>(totals));
        dst[1] = (vgetq_lane_s32::<1>(totals), vgetq_lane_s32::<3>(totals));
    }
    if row.len() % 2 != 0 {
        let column = row.len() - 1;
        row[column] = block_moments(samples, reference, stride, column);
    }
}

/// A small uncentered squared norm certifies both zero AC and zero DC:
/// E <= 64 * squares and sum² <= 64 * squares. Use zero moments as a
/// representative only for those blocks, otherwise supply the exact moments.
#[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
#[target_feature(enable = "dotprod")]
unsafe fn predicted_moments(
    src: &[u8],
    pred: &[u8],
    stride: usize,
    limit: u32,
    out: &mut [(i32, i32)],
) {
    use std::arch::aarch64::*;
    let span = stride.checked_mul(8).expect("band length");
    let src = &src[..span];
    let pred = &pred[..span];
    assert!(out.len() <= stride / 8);
    let ones = vdupq_n_u8(1);
    for (pair, dst) in out.chunks_exact_mut(2).enumerate() {
        let x = pair * 16;
        // SAFETY: both bands contain eight rows and x..x+16 is inside each.
        unsafe {
            let current: [_; 8] =
                std::array::from_fn(|y| vld1q_u8(src.as_ptr().add(y * stride + x)));
            let old: [_; 8] = std::array::from_fn(|y| vld1q_u8(pred.as_ptr().add(y * stride + x)));
            let mut squares = vdupq_n_u32(0);
            for i in 0..8 {
                let d = vabdq_u8(current[i], old[i]);
                squares = vdotq_u32(squares, d, d);
            }
            let sq = vpaddq_u32(squares, squares);
            let first = vgetq_lane_u32::<0>(sq);
            let second = vgetq_lane_u32::<1>(sq);
            if first <= limit && second <= limit {
                dst.fill((0, 0));
            } else {
                let mut a = vdupq_n_u32(0);
                let mut b = vdupq_n_u32(0);
                for i in 0..8 {
                    a = vdotq_u32(a, current[i], ones);
                    b = vdotq_u32(b, old[i], ones);
                }
                let delta = vreinterpretq_s32_u32(vsubq_u32(a, b));
                let sums = vpaddq_s32(delta, delta);
                dst[0] = (vgetq_lane_s32::<0>(sums), first as i32);
                dst[1] = (vgetq_lane_s32::<1>(sums), second as i32);
            }
        }
    }
    if out.len() % 2 != 0 {
        let i = out.len() - 1;
        out[i] = block_moments(src, Some(pred), stride, i);
    }
}

enum BlockSpectrum {
    DcLevel(i16),
    WeightsThrough(u8),
}

struct BlockQuantization {
    quantizer: u8,
    dc_step: i32,
    #[cfg(not(feature = "streaming-fixed-point-dct"))]
    dc_reciprocal: u64,
    #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
    zero_squares: u32,
    zero_dc_sum: u64,
    dc_only_energy: i64,
}

impl BlockQuantization {
    fn new(quantizer: u8) -> Self {
        let half_step = i64::from(MIN_AC_WEIGHT) * i64::from(quantizer) / 2;
        let radius = (half_step - 2) * 64 / 65;
        let dc_step = quantization_step(0, quantizer);
        Self {
            quantizer,
            dc_step,
            #[cfg(not(feature = "streaming-fixed-point-dct"))]
            dc_reciprocal: (1_u64 << 32).div_ceil(dc_step as u64),
            // DC is sum / 8 up to the table and signed rounding error.
            zero_dc_sum: 8 * (dc_step as u64 / 2).saturating_sub(2),
            #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
            zero_squares: (radius * radius).min((8 * (dc_step as i64 / 2 - 2).max(0)).pow(2) / 64)
                as u32,
            dc_only_energy: BLOCK_PIXELS as i64 * radius * radius,
        }
    }

    /// Keep only collocated blocks whose exact moments can produce a level.
    /// Pixel residuals fit in signed 32-bit lanes even after multiplying by 64.
    fn possible_residuals(&self, moments: &[(i32, i32)]) -> u64 {
        debug_assert!(moments.len() <= 64);
        let mut result = 0;
        for (i, chunk) in moments.chunks(8).enumerate() {
            let mut lanes = [0u8; 8];
            for (bit, (&(s, q), lane)) in chunk.iter().zip(&mut lanes).enumerate() {
                let energy = 64 * q - s * s;
                let keep = (energy as i64 > self.dc_only_energy)
                    | (u64::from(s.unsigned_abs()) > self.zero_dc_sum);
                *lane = u8::from(keep) << bit;
            }
            let packed = lanes.into_iter().fold(0, |a, b| a | b);
            result |= u64::from(packed) << (8 * i);
        }
        result
    }

    #[inline(always)]
    fn classify(
        &self,
        frame: &[u8],
        prediction: Option<&[u8]>,
        dims: FrameDimensions,
        index: usize,
        moments: (i32, i32),
    ) -> BlockSpectrum {
        if self.quantizer == 0 {
            if let Some(prev) = prediction {
                let (x, y) = block_origin(dims, index);
                let stride = usize::from(dims.width);
                if (0..BLOCK_SIZE).all(|dy| {
                    let start = (y + dy) * stride + x;
                    frame[start..start + BLOCK_SIZE] == prev[start..start + BLOCK_SIZE]
                }) {
                    return BlockSpectrum::DcLevel(0);
                }
            }
            return BlockSpectrum::WeightsThrough(u8::MAX);
        }
        let (sum, squares) = (i64::from(moments.0), i64::from(moments.1));
        match self.ac_limit(sum, squares) {
            Some(limit) => BlockSpectrum::WeightsThrough(limit),
            None if sum.unsigned_abs() <= self.zero_dc_sum => BlockSpectrum::DcLevel(0),
            None => BlockSpectrum::DcLevel(self.quantized_dc(sum, || {
                let (x, y) = block_origin(dims, index);
                let stride = usize::from(dims.width);
                ResidualRows::new(frame, prediction, stride, y * stride + x).block()
            })),
        }
    }

    #[inline(always)]
    fn ac_limit(&self, sum: i64, squares: i64) -> Option<u8> {
        if self.quantizer == 0 {
            return Some(u8::MAX);
        }
        let energy = BLOCK_PIXELS as i64 * squares - sum * sum;
        if energy <= self.dc_only_energy {
            return None;
        }
        // ||x - mean(x)|| = sqrt(energy)/8; the basis norm is at most 65/64.
        // E <= 2^42 for signed i16 input. Consecutive integer energies near
        // a square have roots separated by more than 1/(2^22+1), well over one ulp.
        let radius_upper = (energy as f64).sqrt() as u64 + 1;
        let bound = (65 * radius_upper).div_ceil(512) + 2;
        Some((2 * bound / u64::from(self.quantizer)).min(u64::from(u8::MAX)) as u8)
    }

    fn quantize_rows(
        &self,
        rows: &ResidualRows<'_>,
        moments: (i32, i32),
        limit: u8,
    ) -> ([i16; BLOCK_PIXELS], u64) {
        #[cfg(all(
            feature = "simd-accel",
            target_arch = "aarch64",
            not(feature = "streaming-fixed-point-dct")
        ))]
        if self.quantizer == 16
            && limit <= NARROW_LIMIT
            && std::arch::is_aarch64_feature_detected!("dotprod")
        {
            // SAFETY: dotprod is detected. The centered energy cutoff is for these source rows.
            return unsafe { rows.narrow_q16(moments, limit, self) };
        }
        let folded = rows.folded();
        let sum = if self.quantizer == 0 {
            folded_sum(&folded)
        } else {
            moments.0
        };
        self.quantize_folded(&folded, sum, limit, || rows.block())
    }

    /// Quantize visited projections directly. Reference-order evaluation only needs
    /// the original residual at a rounding boundary.
    fn quantize_folded(
        &self,
        folded: &FoldedBlock,
        sum: i32,
        limit: u8,
        residual: impl Fn() -> [i16; BLOCK_PIXELS],
    ) -> ([i16; BLOCK_PIXELS], u64) {
        #[cfg(all(
            feature = "simd-accel",
            target_arch = "aarch64",
            not(feature = "streaming-fixed-point-dct")
        ))]
        if self.quantizer != 0 && std::arch::is_aarch64_feature_detected!("dotprod") {
            // SAFETY: the CPU advertises the integer dot-product instructions.
            let (mut levels, active) =
                unsafe { dot_ac_levels(folded, sum, limit, self.quantizer, &residual) };
            levels[0] = self.quantized_dc(i64::from(sum), residual);
            return (levels, active);
        }
        let mut levels = [0; BLOCK_PIXELS];
        let mut active = 0;
        if self.quantizer == 0 && *folded == [[[0; 4]; 4]; 4] {
            return (levels, active);
        }
        visit_ac(
            folded,
            sum,
            limit,
            self.quantizer,
            true,
            &residual,
            |slot, value, step| {
                levels[slot] = if self.quantizer == 0 {
                    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
                } else {
                    quantize_ac(value, step as i32)
                };
                active |= u64::from(levels[slot] != 0) * ZIGZAG_BIT[slot];
            },
        );
        levels[0] = self.quantized_dc(i64::from(sum), residual);
        (levels, active)
    }

    #[inline(always)]
    fn quantized_dc(&self, sum: i64, _residual: impl FnOnce() -> [i16; BLOCK_PIXELS]) -> i16 {
        #[cfg(feature = "streaming-fixed-point-dct")]
        {
            let basis = i64::from(DCT_FACTORS_Q15[0][0]);
            quantize_level(round_q30(sum * basis * basis) as i32, self.dc_step)
        }
        #[cfg(not(feature = "streaming-fixed-point-dct"))]
        {
            let step = self.dc_step as u64;
            let biased = (sum.unsigned_abs() + 4) / 8 + step / 2;
            // For any i16 block, biased * step < 2^32. Rounding the reciprocal
            // up therefore adds less than 1/step to the exact quotient.
            let level = (biased * self.dc_reciprocal) >> 32;
            // At a half-integer DC, the reference can round magnitude down by one.
            // Only an exact quantizer boundary would then change the level.
            if sum & 7 == 4 && biased == level * step {
                let dc = direct_dct_coefficient(&_residual(), &DCT_FACTORS[0], &DCT_FACTORS[0]);
                return quantize_level(dc, self.dc_step);
            }
            let signed = if sum < 0 {
                -(level as i32)
            } else {
                level as i32
            };
            signed.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
        }
    }
}

/// Prediction uses the pixel being replaced, so reconstruction needs no predictor block.
fn reconstruct_ac(
    frame: &mut [u8],
    stride: usize,
    start: usize,
    kind: FrameType,
    spatial: &[i32; BLOCK_PIXELS],
) {
    for (y, values) in spatial.as_chunks::<BLOCK_SIZE>().0.iter().enumerate() {
        let offset = start + y * stride;
        for (pixel, &value) in frame[offset..offset + BLOCK_SIZE].iter_mut().zip(values) {
            let pred = if kind == FrameType::Predicted {
                *pixel
            } else {
                0
            };
            *pixel = clamp_pixel(i32::from(pred) + value);
        }
    }
}

fn reconstruct_dc(
    frame: &mut [u8],
    dims: FrameDimensions,
    index: usize,
    kind: FrameType,
    offset: i32,
) {
    let (x, y) = block_origin(dims, index);
    let stride = usize::from(dims.width);
    for dy in 0..BLOCK_SIZE {
        let start = (y + dy) * stride + x;
        let row = &mut frame[start..start + BLOCK_SIZE];
        if kind == FrameType::Intra {
            row.fill(clamp_pixel(offset));
        } else {
            for pixel in row {
                *pixel = clamp_pixel(i32::from(*pixel) + offset);
            }
        }
    }
}

#[cfg(not(feature = "streaming-fixed-point-dct"))]
use self::DCT_FACTORS as FORWARD_BASIS;
#[cfg(feature = "streaming-fixed-point-dct")]
use self::DCT_FACTORS_Q15 as FORWARD_BASIS;

#[derive(Clone, Copy)]
struct DctProjection {
    slot: usize,
    weight: u32,
    factors: [i32; 16],
    axis: [i32; 4],
}

/// Within each reflection group, sort the AC projections by quantization weight
/// so that the first excluded entry also rules out every one that follows.
static AC_PROJECTIONS: [[DctProjection; 16]; 4] = {
    let empty = DctProjection {
        slot: 0,
        weight: 256,
        factors: [0; 16],
        axis: [0; 4],
    };
    // The even/even group has only 15 AC slots. Its last weight exceeds every u8 limit.
    let mut groups = [[empty; 16]; 4];
    let mut slot = 1;
    while slot < BLOCK_PIXELS {
        let (u, v) = (slot / 8, slot % 8);
        let weight = BASELINE_QUANT_MATRIX[slot] as u32;
        let group = &mut groups[(u % 2) * 2 + v % 2];
        let mut index = 0;
        while group[index].weight <= weight {
            index += 1;
        }
        let mut end = 15;
        while end > index {
            group[end] = group[end - 1];
            end -= 1;
        }
        let mut factors = [0; 16];
        let mut i = 0;
        while i < 16 {
            let a = FORWARD_BASIS[u][i / 4];
            let b = FORWARD_BASIS[v][i % 4];
            #[cfg(feature = "streaming-fixed-point-dct")]
            {
                factors[i] = (a as i32) * (b as i32);
            }
            #[cfg(not(feature = "streaming-fixed-point-dct"))]
            {
                factors[i] = (a * b * 4_294_967_296.0).round() as i32;
            }
            i += 1;
        }
        group[index] = DctProjection {
            slot,
            weight,
            axis: if v == 0 {
                [factors[0], factors[4], factors[8], factors[12]]
            } else {
                [factors[0], factors[1], factors[2], factors[3]]
            },
            factors,
        };
        slot += 1;
    }
    groups
};

/// Four projections per dot-product lane group. Each group of four input
/// samples multiplies four adjacent weights for each of the four frequencies.
#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(feature = "streaming-fixed-point-dct")
))]
static DOT_WEIGHTS: [[[[i8; 16]; 4]; 4]; 4] = {
    let mut out = [[[[0; 16]; 4]; 4]; 4];
    let mut g = 0;
    while g < 4 {
        let mut i = 0;
        while i < 16 {
            let p = AC_PROJECTIONS[g][i];
            let mut s = 0;
            while s < 16 {
                let u = p.slot / 8;
                let v = p.slot % 8;
                let w = DCT_FACTORS[u][s / 4] * DCT_FACTORS[v][s % 4];
                out[g][i / 4][s / 4][4 * (i % 4) + s % 4] = (w * 512.0).round() as i8;
                s += 1;
            }
            i += 1;
        }
        g += 1;
    }
    out
};

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(feature = "streaming-fixed-point-dct")
))]
const BATCH_WEIGHTS: [[[u32; 4]; 4]; 4] = {
    let mut weights = [[[0; 4]; 4]; 4];
    let mut g = 0;
    while g < 4 {
        let mut i = 0;
        while i < 16 {
            weights[g][i / 4][i % 4] = AC_PROJECTIONS[g][i].weight;
            i += 1;
        }
        g += 1;
    }
    weights
};

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(feature = "streaming-fixed-point-dct")
))]
#[target_feature(enable = "dotprod")]
unsafe fn dot_ac_levels(
    folded: &FoldedBlock,
    sum: i32,
    limit: u8,
    qp: u8,
    residual: &impl Fn() -> [i16; BLOCK_PIXELS],
) -> ([i16; BLOCK_PIXELS], u64) {
    use std::arch::aarch64::*;
    let mut levels = [0; BLOCK_PIXELS];
    let mut active = 0;
    let scale = u32::from(qp);
    for (g, samples) in folded.iter().enumerate() {
        let projections = &AC_PROJECTIONS[g];
        if projections[0].weight > u32::from(limit) {
            continue;
        }
        let center = if g == 0 { sum / 16 } else { 0 };
        // SAFETY: each of the four source rows and weight vectors is complete.
        unsafe {
            let rows = samples.map(|r| vsubq_s32(vld1q_s32(r.as_ptr()), vdupq_n_s32(center)));
            let abs = rows.map(|r| vabsq_s32(r));
            let l1 = vaddvq_s32(vaddq_s32(
                vaddq_s32(abs[0], abs[1]),
                vaddq_s32(abs[2], abs[3]),
            )) as u32;
            let threshold = (u32::from(limit) * scale).min(2 * (l1.div_ceil(4) + 2));
            if projections[0].weight * scale > threshold {
                continue;
            }
            let max = vmaxvq_s32(vmaxq_s32(
                vmaxq_s32(abs[0], abs[1]),
                vmaxq_s32(abs[2], abs[3]),
            )) as u32;
            // Shift by the minimum number of bits that leaves room for a sign.
            let shift = (32 - max.leading_zeros()).saturating_sub(7);
            let shrink = vdupq_n_s32(-(shift as i32));
            let lo = vcombine_s16(
                vmovn_s32(vshlq_s32(rows[0], shrink)),
                vmovn_s32(vshlq_s32(rows[1], shrink)),
            );
            let hi = vcombine_s16(
                vmovn_s32(vshlq_s32(rows[2], shrink)),
                vmovn_s32(vshlq_s32(rows[3], shrink)),
            );
            let data = vcombine_s8(vmovn_s16(lo), vmovn_s16(hi));
            // In Q9 units: rounded weights cost <= L1/2. Truncating each
            // input costs at most 2^shift-1 samples at |weight| <= 128.
            let error = l1.div_ceil(2) + 16 * 128 * ((1 << shift) - 1) + 1;
            for (batch, weights) in DOT_WEIGHTS[g].iter().enumerate() {
                let first = 4 * batch;
                if projections[first].weight * scale > threshold {
                    break;
                }
                let mut dots =
                    vdotq_laneq_s32::<0>(vdupq_n_s32(0), vld1q_s8(weights[0].as_ptr()), data);
                dots = vdotq_laneq_s32::<1>(dots, vld1q_s8(weights[1].as_ptr()), data);
                dots = vdotq_laneq_s32::<2>(dots, vld1q_s8(weights[2].as_ptr()), data);
                dots = vdotq_laneq_s32::<3>(dots, vld1q_s8(weights[3].as_ptr()), data);
                // Compare four certified intervals to their quantizer cells
                // before scattering only the lanes with a possible nonzero.
                let weight = vld1q_u32(BATCH_WEIGHTS[g][batch].as_ptr());
                let stepv = vmulq_n_u32(weight, scale);
                let magnitude = vshlq_u32(
                    vreinterpretq_u32_s32(vabsq_s32(dots)),
                    vdupq_n_s32(shift as i32),
                );
                let high = vaddq_u32(magnitude, vdupq_n_u32(error));
                let half = vshrq_n_u32::<1>(stepv);
                let zero = vaddq_u32(vshlq_n_u32::<9>(half), vdupq_n_u32(256));
                let possible = vandq_u32(
                    vcleq_u32(stepv, vdupq_n_u32(threshold)),
                    vcgeq_u32(high, zero),
                );
                if vmaxvq_u32(possible) == 0 {
                    continue;
                }
                let unit_end = vsubq_u32(
                    vshlq_n_u32::<9>(vsubq_u32(vaddq_u32(stepv, stepv), half)),
                    vdupq_n_u32(256),
                );
                let unit = vandq_u32(
                    vcgtq_u32(magnitude, vaddq_u32(zero, vdupq_n_u32(error))),
                    vcltq_u32(high, unit_end),
                );
                let mut flags = [0; 4];
                let mut signs = [0; 4];
                vst1q_u32(
                    flags.as_mut_ptr(),
                    vandq_u32(possible, vorrq_u32(unit, vdupq_n_u32(1))),
                );
                vst1q_s32(signs.as_mut_ptr(), dots);
                for (i, flag) in flags.into_iter().enumerate() {
                    if flag == 0 {
                        continue;
                    }
                    let p = &projections[first + i];
                    let step = p.weight * scale;
                    let level = if flag == u32::MAX {
                        if signs[i] < 0 { -1 } else { 1 }
                    } else {
                        let wide = folded_dot(samples, &p.factors);
                        let q = if (wide as u32).abs_diff(1 << 31) <= (1 << 20) + 128 {
                            direct_dct_coefficient(
                                &residual(),
                                &FORWARD_BASIS[p.slot / 8],
                                &FORWARD_BASIS[p.slot % 8],
                            )
                        } else {
                            ((wide + (1 << 31)) >> 32) as i32
                        };
                        quantize_ac(q, step as i32)
                    };
                    levels[p.slot] = level;
                    active |= u64::from(level != 0) * ZIGZAG_BIT[p.slot];
                }
            }
        }
    }
    (levels, active)
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(feature = "streaming-fixed-point-dct")
))]
const NARROW_LIMIT: u8 = 29;
#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(feature = "streaming-fixed-point-dct")
))]
#[derive(Clone, Copy)]
struct NarrowKernel {
    slot: usize,
    weight: u8,
    l1: u32,
    rows: [[i8; 16]; 4],
}
#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(feature = "streaming-fixed-point-dct")
))]
const NARROW_COUNT: usize = {
    let mut n = 0;
    let mut i = 1;
    while i < 64 {
        if BASELINE_QUANT_MATRIX[i] <= NARROW_LIMIT {
            n += 1;
        }
        i += 1;
    }
    n
};
#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(feature = "streaming-fixed-point-dct")
))]
static NARROW_KERNELS: [NarrowKernel; NARROW_COUNT] = {
    let mut ks = [NarrowKernel {
        slot: 0,
        weight: 0,
        l1: 0,
        rows: [[0; 16]; 4],
    }; NARROW_COUNT];
    let mut k = 0;
    let mut w = MIN_AC_WEIGHT;
    while w <= NARROW_LIMIT {
        let mut slot = 1;
        while slot < 64 {
            if BASELINE_QUANT_MATRIX[slot] == w {
                let mut sum = 0;
                let mut i = 0;
                while i < 64 {
                    let x = (512.0 * DCT_FACTORS[slot / 8][i / 8] * DCT_FACTORS[slot % 8][i % 8])
                        .round() as i8;
                    ks[k].rows[i / 16][i % 16] = x;
                    ks[k].l1 += x.unsigned_abs() as u32;
                    sum += x as i32;
                    i += 1;
                }
                assert!(sum == 0);
                ks[k].slot = slot;
                ks[k].weight = w;
                k += 1;
            }
            slot += 1;
        }
        w += 1;
    }
    ks
};

type FoldedBlock = [[[i32; 4]; 4]; 4];

#[cfg(any(test, not(all(feature = "simd-accel", target_arch = "aarch64"))))]
fn fold_scalar(row: impl Fn(usize) -> [i16; BLOCK_SIZE]) -> FoldedBlock {
    const HALF: usize = BLOCK_SIZE / 2;
    let mut folded = [[[0_i32; HALF]; HALF]; 4];
    for y in 0..HALF {
        let upper = row(y);
        let lower = row(BLOCK_SIZE - 1 - y);
        for x in 0..HALF {
            let mirror = BLOCK_SIZE - 1 - x;
            let a = i32::from(upper[x]);
            let b = i32::from(upper[mirror]);
            let c = i32::from(lower[x]);
            let d = i32::from(lower[mirror]);
            let (top_sum, bottom_sum) = (a + b, c + d);
            let (top_diff, bottom_diff) = (a - b, c - d);
            folded[0][y][x] = top_sum + bottom_sum;
            folded[1][y][x] = top_diff + bottom_diff;
            folded[2][y][x] = top_sum - bottom_sum;
            folded[3][y][x] = top_diff - bottom_diff;
        }
    }
    folded
}

#[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
fn fold_rows(row: impl Fn(usize) -> [i16; BLOCK_SIZE]) -> FoldedBlock {
    use std::arch::aarch64::*;
    fold_vectors(|y| unsafe { vld1q_s16(row(y).as_ptr()) })
}

#[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
#[inline(always)]
fn fold_vectors(row: impl Fn(usize) -> std::arch::aarch64::int16x8_t) -> FoldedBlock {
    use std::arch::aarch64::*;
    let mut folded = [[[0; 4]; 4]; 4];
    // SAFETY: AArch64 has baseline NEON. Each load covers a full eight-sample
    // row, and each store writes one complete four-lane row of a parity group.
    unsafe {
        for y in 0..4 {
            let upper = row(y);
            let lower = row(7 - y);
            let (a, b) = (vget_low_s16(upper), vrev64_s16(vget_high_s16(upper)));
            let (c, d) = (vget_low_s16(lower), vrev64_s16(vget_high_s16(lower)));
            let (upper_sum, lower_sum) = (vaddl_s16(a, b), vaddl_s16(c, d));
            let (upper_diff, lower_diff) = (vsubl_s16(a, b), vsubl_s16(c, d));
            vst1q_s32(folded[0][y].as_mut_ptr(), vaddq_s32(upper_sum, lower_sum));
            vst1q_s32(folded[1][y].as_mut_ptr(), vaddq_s32(upper_diff, lower_diff));
            vst1q_s32(folded[2][y].as_mut_ptr(), vsubq_s32(upper_sum, lower_sum));
            vst1q_s32(folded[3][y].as_mut_ptr(), vsubq_s32(upper_diff, lower_diff));
        }
    }
    folded
}

#[cfg(not(all(feature = "simd-accel", target_arch = "aarch64")))]
fn fold_rows(row: impl Fn(usize) -> [i16; BLOCK_SIZE]) -> FoldedBlock {
    fold_scalar(row)
}

fn folded_sum(folded: &FoldedBlock) -> i32 {
    folded[0].as_flattened().iter().sum()
}

/// Visit the retained AC coefficients of a folded block.
fn visit_ac(
    folded: &FoldedBlock,
    sum: i32,
    limit: u8,
    quantizer: u8,
    levels_only: bool,
    _residual: impl Fn() -> [i16; BLOCK_PIXELS],
    mut visit: impl FnMut(usize, i32, u32),
) {
    #[inline(always)]
    fn group<const G: usize>(
        folded: &FoldedBlock,
        sum: i32,
        limit: u8,
        quantizer: u8,
        _levels_only: bool,
        _residual: &impl Fn() -> [i16; BLOCK_PIXELS],
        visit: &mut impl FnMut(usize, i32, u32),
    ) {
        let scale = u32::from(quantizer.max(1));
        let block_threshold = u32::from(limit) * scale;
        let samples = &folded[G];
        let projections = &AC_PROJECTIONS[G];
        if projections[0].weight > u32::from(limit) {
            return;
        }
        let mut threshold = block_threshold;
        if quantizer != 0 {
            let center = if G == 0 { sum / 16 } else { 0 };
            let l1 = folded_l1(samples, center);
            threshold = threshold.min(2 * (l1.div_ceil(4) + 2));
        }
        // Constant horizontal/vertical basis rows factor out of the dot
        // product. Each collapsed axis needs four products instead of sixteen.
        let horizontal: [i32; 4] = std::array::from_fn(|x| samples.iter().map(|row| row[x]).sum());
        let vertical = samples.map(|row| row.iter().sum::<i32>());
        for projection in projections {
            let step = scale * projection.weight;
            if step > threshold {
                break;
            }
            let slot = projection.slot;
            let value = if G < 2 && slot < BLOCK_SIZE {
                dot_axis(&horizontal, &projection.axis)
            } else if G % 2 == 0 && slot % BLOCK_SIZE == 0 {
                dot_axis(&vertical, &projection.axis)
            } else {
                folded_dot(samples, &projection.factors)
            };
            #[cfg(feature = "streaming-fixed-point-dct")]
            let coefficient = round_q30(value) as i32;
            #[cfg(not(feature = "streaming-fixed-point-dct"))]
            let coefficient = {
                // Quantizing the weights costs at most half a Q32 unit per
                // absolute folded sample. The extra 128 cover table asymmetry
                // and binary64 summation for the full i16 input range.
                let floor = (value >> 32) as i32;
                if (value as u32).abs_diff(1 << 31) <= (1 << 20) + 128
                    && !(_levels_only
                        && quantizer != 0
                        && quantize_ac(floor, step as i32) == quantize_ac(floor + 1, step as i32))
                {
                    direct_dct_coefficient(
                        &_residual(),
                        &FORWARD_BASIS[slot / 8],
                        &FORWARD_BASIS[slot % 8],
                    )
                } else {
                    ((value + (1 << 31)) >> 32) as i32
                }
            };
            visit(slot, coefficient, step);
        }
    }
    group::<0>(
        folded,
        sum,
        limit,
        quantizer,
        levels_only,
        &_residual,
        &mut visit,
    );
    group::<1>(
        folded,
        sum,
        limit,
        quantizer,
        levels_only,
        &_residual,
        &mut visit,
    );
    group::<2>(
        folded,
        sum,
        limit,
        quantizer,
        levels_only,
        &_residual,
        &mut visit,
    );
    group::<3>(
        folded,
        sum,
        limit,
        quantizer,
        levels_only,
        &_residual,
        &mut visit,
    );
}

#[inline(always)]
fn dot_axis(samples: &[i32; 4], weights: &[i32; 4]) -> i64 {
    samples
        .iter()
        .zip(weights)
        .map(|(&x, &w)| i64::from(x) * i64::from(w))
        .sum()
}

fn folded_l1(samples: &[[i32; 4]; 4], center: i32) -> u32 {
    #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
    {
        use std::arch::aarch64::*;
        // SAFETY: all four elements of each row are initialized and in bounds.
        unsafe {
            let center = vdupq_n_s32(center);
            let mut total = vdupq_n_s32(0);
            for row in samples {
                total = vabaq_s32(total, vld1q_s32(row.as_ptr()), center);
            }
            vaddvq_s32(total) as u32
        }
    }
    #[cfg(not(all(feature = "simd-accel", target_arch = "aarch64")))]
    {
        samples
            .as_flattened()
            .iter()
            .map(|&v| v.abs_diff(center))
            .sum()
    }
}

/// The product sum fits in i64 for all i16 blocks, so partial sums can be
/// combined in any order. Widen before multiplication on every target.
fn folded_dot(samples: &[[i32; 4]; 4], weights: &[i32; 16]) -> i64 {
    #[cfg(all(feature = "simd-accel", target_arch = "aarch64"))]
    {
        use std::arch::aarch64::*;
        // SAFETY: each row and its corresponding four weights are in bounds.
        unsafe {
            let mut low = vdupq_n_s64(0);
            let mut high = vdupq_n_s64(0);
            for (row, factors) in samples.iter().zip(weights.as_chunks::<4>().0) {
                let x = vld1q_s32(row.as_ptr());
                let w = vld1q_s32(factors.as_ptr());
                low = vmlal_s32(low, vget_low_s32(x), vget_low_s32(w));
                high = vmlal_high_s32(high, x, w);
            }
            vaddvq_s64(vaddq_s64(low, high))
        }
    }
    #[cfg(not(all(feature = "simd-accel", target_arch = "aarch64")))]
    {
        samples
            .as_flattened()
            .iter()
            .zip(weights)
            .map(|(&x, &w)| i64::from(x) * i64::from(w))
            .sum()
    }
}

/// Preserve the reference multiplication and summation order at rounding boundaries.
#[cfg(not(feature = "streaming-fixed-point-dct"))]
fn direct_dct_coefficient(
    block: &[i16; BLOCK_PIXELS],
    basis_u: &[f64; BLOCK_SIZE],
    basis_v: &[f64; BLOCK_SIZE],
) -> i32 {
    let mut sum = 0.0;
    for (&factor_u, samples) in basis_u.iter().zip(block.chunks_exact(BLOCK_SIZE)) {
        for (&sample, &factor_v) in samples.iter().zip(basis_v) {
            sum += f64::from(sample) * factor_u * factor_v;
        }
    }
    sum.round() as i32
}

fn inverse_dc_sample(dc: i32) -> i32 {
    #[cfg(feature = "streaming-fixed-point-dct")]
    {
        let basis = i64::from(DCT_FACTORS_Q15[0][0]);
        round_q30(i64::from(dc) * basis * basis).clamp(-32768, 32767) as i32
    }
    #[cfg(not(feature = "streaming-fixed-point-dct"))]
    {
        let basis = DCT_FACTORS[0][0];
        (f64::from(dc) * basis * basis)
            .round()
            .clamp(-32768.0, 32767.0) as i32
    }
}

pub(crate) fn inverse_dct(coeffs: &[i32; BLOCK_PIXELS]) -> [i32; BLOCK_PIXELS] {
    if coeffs[1..].iter().all(|&ac| ac == 0) {
        return [inverse_dc_sample(coeffs[0]); BLOCK_PIXELS];
    }
    inverse_dct_full(coeffs)
}

#[cfg(feature = "streaming-fixed-point-dct")]
fn inverse_dct_full(coeffs: &[i32; BLOCK_PIXELS]) -> [i32; BLOCK_PIXELS] {
    let mut sums = [0_i64; BLOCK_PIXELS];
    for (u, row) in coeffs.chunks_exact(BLOCK_SIZE).enumerate() {
        for (v, &coeff) in row.iter().enumerate() {
            if coeff != 0 {
                for (samples, &factor_u) in
                    sums.chunks_exact_mut(BLOCK_SIZE).zip(&DCT_FACTORS_Q15[u])
                {
                    // An i32 coefficient times two Q15 factors fits in i64.
                    let scaled = i64::from(coeff) * i64::from(factor_u);
                    for (sum, &factor_v) in samples.iter_mut().zip(&DCT_FACTORS_Q15[v]) {
                        *sum = sum.saturating_add(scaled * i64::from(factor_v));
                    }
                }
            }
        }
    }
    std::array::from_fn(|i| round_q30(sums[i]).clamp(-32768, 32767) as i32)
}
#[cfg(not(feature = "streaming-fixed-point-dct"))]
fn inverse_dct_full(coeffs: &[i32; BLOCK_PIXELS]) -> [i32; BLOCK_PIXELS] {
    // Each coefficient contributes one scaled outer product. Visit only active
    // coefficients, in the original raster order for every output sample.
    let mut sums = [0.0; BLOCK_PIXELS];
    for (u, row) in coeffs.chunks_exact(BLOCK_SIZE).enumerate() {
        for (v, &coeff) in row.iter().enumerate() {
            if coeff != 0 {
                for (samples, &factor_u) in sums.chunks_exact_mut(BLOCK_SIZE).zip(&DCT_FACTORS[u]) {
                    let scaled = f64::from(coeff) * factor_u;
                    for (sum, &factor_v) in samples.iter_mut().zip(&DCT_FACTORS[v]) {
                        *sum += scaled * factor_v;
                    }
                }
            }
        }
    }
    std::array::from_fn(|i| sums[i].round().clamp(-32768.0, 32767.0) as i32)
}
fn quantization_step(slot: usize, quantizer: u8) -> i32 {
    if quantizer == 0 {
        1
    } else {
        i32::from(BASELINE_QUANT_MATRIX[slot]) * i32::from(quantizer)
    }
}
fn quantize_level(coeff: i32, step: i32) -> i16 {
    let adjusted = if coeff >= 0 {
        coeff.saturating_add(step / 2)
    } else {
        coeff.saturating_sub(step / 2)
    };
    (adjusted / step).clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

pub(crate) fn quantize_coeffs(coeffs: &[i32; BLOCK_PIXELS], quantizer: u8) -> [i16; BLOCK_PIXELS] {
    std::array::from_fn(|slot| quantized_coefficient(slot, coeffs[slot], quantizer))
}

fn quantized_coefficient(slot: usize, coeff: i32, quantizer: u8) -> i16 {
    if quantizer == 0 {
        return coeff.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
    }
    let step = quantization_step(slot, quantizer);
    if slot == 0 {
        quantize_level(coeff, step)
    } else {
        quantize_ac(coeff, step)
    }
}

fn quantize_ac(coeff: i32, step: i32) -> i16 {
    let magnitude = coeff.unsigned_abs();
    // The midpoint at zero saves a token. Most surviving AC levels have magnitude one.
    if magnitude <= (step / 2) as u32 {
        0
    } else if magnitude < (2 * step - step / 2) as u32 {
        if coeff < 0 { -1 } else { 1 }
    } else {
        quantize_level(coeff, step)
    }
}

pub(crate) fn dequantize_coeffs(
    coeffs: &[i16; BLOCK_PIXELS],
    quantizer: u8,
) -> [i32; BLOCK_PIXELS] {
    let mut out = [0i32; BLOCK_PIXELS];
    if quantizer == 0 {
        for (idx, coeff) in coeffs.iter().enumerate() {
            out[idx] = i32::from(*coeff);
        }
        return out;
    }
    for (idx, coeff) in coeffs.iter().enumerate() {
        let step = quantization_step(idx, quantizer);
        out[idx] = i32::from(*coeff) * step;
    }
    out
}
pub(crate) trait BundledHooks {
    fn record_dc(&mut self, _diff: i16) {}
    fn record_ac(&mut self, _zeros: u8, _value: i16) {}
    fn record_eob(&mut self) {}
    fn record_zero_run(&mut self, _blocks: std::ops::Range<usize>, _first_diff: i16) {}
    fn record_dc_only(
        &mut self,
        _levels: &[i16; BLOCK_PIXELS],
        _diff: i16,
        _index: usize,
        _kind: FrameType,
    ) {
    }
    fn record_block_coeffs(
        &mut self,
        _coeffs: &[i16; BLOCK_PIXELS],
        _active: u64,
        _block_index: usize,
        _frame_type: FrameType,
    ) {
    }
}
#[derive(Default)]
struct NoopBundledHooks;
impl BundledHooks for NoopBundledHooks {}
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::BundledStats")]
#[derive(Clone, Copy, Debug, Default, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct BundledStats {
    pub bundle_width: u8,
    pub bundles_total: u64,
    pub blocks_encoded: u64,
    pub significance_zero: u64,
    pub significance_one: u64,
    pub sign_negative: u64,
    pub sign_positive: u64,
    pub parity_one: u64,
    pub geq2_one: u64,
    pub dc_events: u64,
    pub ac_events: u64,
    pub nonzero_levels: u64,
    pub zero_run_total: u64,
    pub max_zero_run: u8,
    #[norito(default)]
    pub significance_rle: u64,
    pub flush_type_complete: u64,
    pub flush_context_switch: u64,
    pub flush_end_of_block: u64,
}
impl BundledStats {
    fn record_flush(&mut self, reason: BundleFlushReason, count: u64) {
        match reason {
            BundleFlushReason::TypeComplete => {
                self.flush_type_complete = self.flush_type_complete.saturating_add(count);
            }
            BundleFlushReason::ContextSwitch => {
                self.flush_context_switch = self.flush_context_switch.saturating_add(count);
            }
            BundleFlushReason::EndOfBlock => {
                self.flush_end_of_block = self.flush_end_of_block.saturating_add(count);
            }
        }
    }
}
pub(crate) const RDO_BUCKET_COUNT: usize = 5;
pub(crate) const RDO_MAX_SYMBOLS: usize = 1 << MAX_BUNDLE_WIDTH;
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::RdoTelemetry")]
#[derive(Clone, Debug, Default, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct RdoTelemetry {
    pub mode: RdoMode,
    pub lambda_bits: f32,
    pub blocks_optimized: u64,
    pub energy_histogram: [u64; RDO_BUCKET_COUNT],
    /// Luma block payload bits before AC selection, including DC and end markers.
    pub before_rate_bits: u64,
    /// Luma RLE bits after AC selection, before frame compression and excluding chroma.
    pub after_rate_bits: u64,
    /// Sum of squared discarded quantized levels.
    pub distortion_penalty: u64,
    pub neural_class_histogram: Vec<u64>,
}
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::ContextFrequency")]
#[derive(Clone, Copy, Debug, Default, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ContextFrequency {
    pub context: BundleContextId,
    pub bundles: u64,
    pub total_bits: u64,
    pub symbol_counts: [u64; RDO_MAX_SYMBOLS],
}
/// Summary describing the context remap applied during bundle recording.
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::ContextRemapSummary")]
#[derive(Clone, Copy, Debug, Default, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ContextRemapSummary {
    pub escape_context: BundleContextId,
    pub remapped: u32,
    pub dropped: u32,
}
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::BundledTelemetry")]
#[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct BundledTelemetry {
    pub stats: BundledStats,
    pub tokens: Vec<BundledToken>,
    pub bundles: Vec<BundleRecord>,
    pub context_stats: Vec<BundleContextStats>,
    pub context_frequencies: Vec<ContextFrequency>,
    pub ans_stream: Vec<u8>,
    pub ans_precision_bits: u8,
    pub tables_checksum: Hash,
    #[norito(default)]
    pub acceleration: BundleAcceleration,
    #[norito(default)]
    pub prefetch_distance: u16,
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub context_remap: Option<ContextRemapSummary>,
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub rdo: Option<RdoTelemetry>,
}
impl BundledTelemetry {
    /// Decode the ANS stream into bundle symbols using the supplied tables.
    pub fn decode_symbols(&self, tables: &BundleAnsTables) -> Result<Vec<u8>, BundleDecodeError> {
        if self.tables_checksum != tables.checksum() {
            return Err(BundleDecodeError::ChecksumMismatch {
                expected: self.tables_checksum,
                found: tables.checksum(),
            });
        }
        decode_bundle_stream(&self.ans_stream, &self.bundles, tables)
    }
    /// Verify that the ANS stream reproduces the recorded bundle bits.
    pub fn verify_stream(&self, tables: &BundleAnsTables) -> Result<(), BundleDecodeError> {
        let decoded = self.decode_symbols(tables)?;
        for (idx, (record, decoded_bits)) in self.bundles.iter().zip(decoded.iter()).enumerate() {
            let mask = (1u8 << record.bit_len) - 1;
            let expected = record.bits & mask;
            let found = decoded_bits & mask;
            if expected != found {
                return Err(BundleDecodeError::SymbolMismatch {
                    index: saturating_usize_to_u32(idx),
                    expected,
                    found,
                });
            }
        }
        Ok(())
    }
}
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::BundledToken")]
#[derive(Clone, Copy, Debug, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::TypeId))]
pub enum BundledToken {
    DcDiff(i16),
    Ac { run: u8, value: i16 },
    EndOfBlock,
}
#[cfg(feature = "schema-structural")]
#[derive(::iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct BundledAcTokenSchema {
    run: u8,
    value: i16,
}
#[cfg(feature = "schema-structural")]
impl ::iroha_schema::IntoSchema for BundledToken {
    fn type_name() -> String {
        "BundledToken".to_owned()
    }
    fn update_schema_map(map: &mut ::iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(::iroha_schema::Metadata::Enum(::iroha_schema::EnumMeta {
            variants: vec![
                ::iroha_schema::EnumVariant {
                    tag: "DcDiff".to_owned(),
                    discriminant: 0,
                    ty: Some(core::any::TypeId::of::<i16>()),
                },
                ::iroha_schema::EnumVariant {
                    tag: "Ac".to_owned(),
                    discriminant: 1,
                    ty: Some(core::any::TypeId::of::<BundledAcTokenSchema>()),
                },
                ::iroha_schema::EnumVariant {
                    tag: "EndOfBlock".to_owned(),
                    discriminant: 2,
                    ty: None,
                },
            ],
        }));
        <i16 as ::iroha_schema::IntoSchema>::update_schema_map(map);
        <BundledAcTokenSchema as ::iroha_schema::IntoSchema>::update_schema_map(map);
    }
}
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::BundleType")]
#[derive(Clone, Copy, Debug, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum BundleType {
    SignificanceOnly,
    SignAndMagnitude,
    SignParity,
    SignParityLevel,
    SignificanceRle,
}
impl BundleType {
    const fn as_u8(self) -> u8 {
        match self {
            Self::SignificanceOnly => 0,
            Self::SignAndMagnitude => 1,
            Self::SignParity => 2,
            Self::SignParityLevel => 3,
            Self::SignificanceRle => 4,
        }
    }
}
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::BundleFlushReason")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum BundleFlushReason {
    TypeComplete,
    ContextSwitch,
    EndOfBlock,
}
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::BundleContextId")]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    NoritoSerialize,
    NoritoDeserialize,
)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct BundleContextId(pub u16);
impl BundleContextId {
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }
}
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::BundleRecord")]
#[derive(Clone, Copy, Debug, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct BundleRecord {
    pub bundle_type: BundleType,
    pub context: BundleContextId,
    pub bits: u8,
    pub bit_len: u8,
    pub flush: BundleFlushReason,
}
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito::streaming::codec::BundleContextStats")]
#[derive(Clone, Copy, Debug, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct BundleContextStats {
    pub context: BundleContextId,
    pub bundles_total: u64,
    pub type_significance_only: u64,
    pub type_sign_and_magnitude: u64,
    pub type_sign_parity: u64,
    pub type_sign_parity_level: u64,
    #[norito(default)]
    pub type_significance_rle: u64,
    pub flush_type_complete: u64,
    pub flush_context_switch: u64,
    pub flush_end_of_block: u64,
}
impl BundleContextStats {
    const fn new(context: BundleContextId) -> Self {
        Self {
            context,
            bundles_total: 0,
            type_significance_only: 0,
            type_sign_and_magnitude: 0,
            type_sign_parity: 0,
            type_sign_parity_level: 0,
            type_significance_rle: 0,
            flush_type_complete: 0,
            flush_context_switch: 0,
            flush_end_of_block: 0,
        }
    }
    fn record_bundle(&mut self, bundle_type: BundleType, flush: BundleFlushReason, count: u64) {
        self.bundles_total = self.bundles_total.saturating_add(count);
        match bundle_type {
            BundleType::SignificanceOnly => {
                self.type_significance_only = self.type_significance_only.saturating_add(count);
            }
            BundleType::SignAndMagnitude => {
                self.type_sign_and_magnitude = self.type_sign_and_magnitude.saturating_add(count);
            }
            BundleType::SignParity => {
                self.type_sign_parity = self.type_sign_parity.saturating_add(count);
            }
            BundleType::SignParityLevel => {
                self.type_sign_parity_level = self.type_sign_parity_level.saturating_add(count);
            }
            BundleType::SignificanceRle => {
                self.type_significance_rle = self.type_significance_rle.saturating_add(count);
            }
        }
        self.record_flush(flush, count);
    }
    fn record_flush(&mut self, flush: BundleFlushReason, count: u64) {
        match flush {
            BundleFlushReason::TypeComplete => {
                self.flush_type_complete = self.flush_type_complete.saturating_add(count);
            }
            BundleFlushReason::ContextSwitch => {
                self.flush_context_switch = self.flush_context_switch.saturating_add(count);
            }
            BundleFlushReason::EndOfBlock => {
                self.flush_end_of_block = self.flush_end_of_block.saturating_add(count);
            }
        }
    }
}
struct BlockRdo<'a> {
    quantizer: u8,
    bundle_width: u8,
    predictor: Option<&'a NeuralPredictor>,
    lambda_hundredths: u32,
    telemetry: RdoTelemetry,
}
impl<'a> BlockRdo<'a> {
    fn new(
        mode: RdoMode,
        quantizer: u8,
        bundle_width: u8,
        predictor: Option<&'a NeuralPredictor>,
    ) -> Option<Self> {
        if !mode.is_enabled() {
            return None;
        }
        let lambda_hundredths = rdo_lambda_hundredths(mode, quantizer);
        let telemetry = RdoTelemetry {
            mode,
            lambda_bits: lambda_hundredths as f32 / 100.0,
            neural_class_histogram: if predictor.is_some() {
                vec![0; NEURAL_OUTPUT]
            } else {
                Vec::new()
            },
            ..Default::default()
        };
        Some(Self {
            quantizer,
            bundle_width,
            predictor,
            lambda_hundredths,
            telemetry,
        })
    }
    fn select_levels(
        &mut self,
        transform: &[i32; BLOCK_PIXELS],
        levels: &mut [i16; BLOCK_PIXELS],
        frame_type: FrameType,
        block_index: usize,
    ) {
        let report = &mut self.telemetry;
        let energy = block_energy(levels);
        let bucket = energy_bucket(energy);
        report.energy_histogram[bucket] = report.energy_histogram[bucket].saturating_add(1);
        let class_scale = if let Some(net) = self.predictor {
            let features = build_neural_features(
                levels,
                energy,
                self.bundle_width,
                self.quantizer,
                frame_type,
                block_index,
            );
            let class = net.predict(&features);
            report.neural_class_histogram[class] =
                report.neural_class_histogram[class].saturating_add(1);
            [85, 100, 112, 128][class]
        } else {
            100
        };
        // The existing lambda schedule budgets one 24-bit token for each level
        // whose squared magnitude warrants its rate. Price it in 1/10,000 units.
        let token_cost = RLE_TOKEN_BITS * u64::from(self.lambda_hundredths * class_scale);
        let mut candidates = [(0_i64, 0_usize); BLOCK_PIXELS - 1];
        let mut count = 0;
        let mut budget = 0;
        for (slot, &level) in levels.iter().enumerate().skip(1) {
            if level == 0 {
                continue;
            }
            let magnitude = u64::from(level.unsigned_abs());
            budget += usize::from(magnitude * magnitude * 10_000 > token_cost);
            let reconstructed =
                i64::from(level) * i64::from(quantization_step(slot, self.quantizer));
            // c² - (c - reconstructed)² is the transform SSE saved by this token.
            let gain = reconstructed * (2 * i64::from(transform[slot]) - reconstructed);
            candidates[count] = (gain, slot);
            count += 1;
        }

        // All tokens cost the same. The largest gains minimize SSE at the given
        // budget. Break equal gains by raster slot for reproducible selection.
        if budget < count {
            let candidates = &mut candidates[..count];
            if budget != 0 {
                candidates
                    .select_nth_unstable_by(budget, |&(a, i), &(b, j)| b.cmp(&a).then(i.cmp(&j)));
            }
            for &(_, slot) in &candidates[budget..] {
                let level = i64::from(levels[slot]);
                report.distortion_penalty = report
                    .distortion_penalty
                    .saturating_add(level.pow(2) as u64);
                levels[slot] = 0;
            }
        }
        // The DC delta and end-of-block record are present even on a dense block.
        let block_bits = |ac_count: usize| 16 + RLE_TOKEN_BITS * (1 + ac_count as u64);
        report.before_rate_bits = report.before_rate_bits.saturating_add(block_bits(count));
        report.after_rate_bits = report.after_rate_bits.saturating_add(block_bits(budget));
        report.blocks_optimized = report.blocks_optimized.saturating_add(1);
    }
}
const fn energy_bucket(energy: u32) -> usize {
    let mut idx = 0usize;
    while idx < RDO_ENERGY_BUCKETS.len() {
        if energy <= RDO_ENERGY_BUCKETS[idx] {
            return idx;
        }
        idx += 1;
    }
    RDO_BUCKET_COUNT - 1
}
fn rdo_lambda_hundredths(mode: RdoMode, quantizer: u8) -> u32 {
    let bucket = usize::from(quantizer_to_bucket(quantizer));
    match mode {
        RdoMode::Perceptual => [42, 65, 95, 115][bucket],
        _ => [45, 75, 110, 135][bucket],
    }
}
fn block_energy(coeffs: &[i16; BLOCK_PIXELS]) -> u32 {
    coeffs
        .iter()
        .enumerate()
        .skip(1)
        .map(|(_, &coeff)| coeff.unsigned_abs())
        .map(u32::from)
        .sum()
}
fn max_zero_run_in_block(coeffs: &[i16; BLOCK_PIXELS]) -> u8 {
    let mut max_run = 0u8;
    let mut run = 0u8;
    for &slot in ZIG_ZAG.iter().skip(1) {
        if coeffs[slot] == 0 {
            run = run.saturating_add(1);
            max_run = max_run.max(run);
        } else {
            run = 0;
        }
    }
    max_run
}
struct TinyPrng(u64);
impl TinyPrng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    fn next_i8(&mut self) -> i8 {
        let raw = self.next();
        let span = 11i16;
        let value = (raw % (2 * span as u32 + 1)) as i16 - span;
        value as i8
    }
    fn next_i16(&mut self) -> i16 {
        let raw = self.next();
        let span = 127i32;
        let value = (raw % (2 * span as u32 + 1)) as i32 - span;
        value as i16
    }
}
#[derive(Clone)]
struct NeuralPredictor {
    weights1: [[i8; NEURAL_FEATURES]; NEURAL_HIDDEN],
    weights2: [[i8; NEURAL_HIDDEN]; NEURAL_OUTPUT],
    bias1: [i16; NEURAL_HIDDEN],
    bias2: [i16; NEURAL_OUTPUT],
    activation_scale: i16,
}
impl NeuralPredictor {
    fn from_seed(seed: [u8; 32]) -> Self {
        let mut seed_bytes = [0u8; 8];
        seed_bytes.copy_from_slice(&seed[..8]);
        let mut prng = TinyPrng::new(u64::from_le_bytes(seed_bytes));
        let mut weights1 = [[0i8; NEURAL_FEATURES]; NEURAL_HIDDEN];
        let mut weights2 = [[0i8; NEURAL_HIDDEN]; NEURAL_OUTPUT];
        let mut bias1 = [0i16; NEURAL_HIDDEN];
        let mut bias2 = [0i16; NEURAL_OUTPUT];
        for row in &mut weights1 {
            for weight in row {
                *weight = prng.next_i8();
            }
        }
        for row in &mut weights2 {
            for weight in row {
                *weight = prng.next_i8();
            }
        }
        for bias in &mut bias1 {
            *bias = prng.next_i16();
        }
        for bias in &mut bias2 {
            *bias = prng.next_i16();
        }
        let activation_scale = (prng.next() % 7 + 9) as i16;
        Self {
            weights1,
            weights2,
            bias1,
            bias2,
            activation_scale,
        }
    }
    fn predict(&self, features: &[i8; NEURAL_FEATURES]) -> usize {
        let mut hidden = [0i16; NEURAL_HIDDEN];
        for (idx, (row, bias)) in self.weights1.iter().zip(self.bias1.iter()).enumerate() {
            let mut acc = i32::from(*bias);
            for (&w, &f) in row.iter().zip(features.iter()) {
                acc = acc.saturating_add(i32::from(w) * i32::from(f));
            }
            let scaled = acc / i32::from(self.activation_scale.max(1));
            let relu = scaled.max(0).min(i32::from(i16::MAX));
            hidden[idx] = relu as i16;
        }
        let mut best = (0usize, i32::MIN);
        for (class, (row, bias)) in self.weights2.iter().zip(self.bias2.iter()).enumerate() {
            let mut acc = i32::from(*bias);
            for (&w, &h) in row.iter().zip(hidden.iter()) {
                acc = acc.saturating_add(i32::from(w) * i32::from(h));
            }
            if acc > best.1 {
                best = (class, acc);
            }
        }
        best.0
    }
}
fn build_neural_features(
    coeffs: &[i16; BLOCK_PIXELS],
    energy: u32,
    bundle_width: u8,
    quantizer: u8,
    frame_type: FrameType,
    block_index: usize,
) -> [i8; NEURAL_FEATURES] {
    let mut features = [0i8; NEURAL_FEATURES];
    features[0] = energy_bucket(energy) as i8;
    features[1] = max_zero_run_in_block(coeffs) as i8;
    features[2] = quantizer_to_bucket(quantizer) as i8;
    features[3] = bundle_width.clamp(1, MAX_BUNDLE_WIDTH as u8) as i8;
    let non_zero = coeffs.iter().skip(1).filter(|&&c| c != 0).count();
    features[4] = non_zero.min(i8::MAX as usize) as i8;
    features[5] = ((block_index as u8) & 0x0F) as i8;
    features[6] = if matches!(frame_type, FrameType::Intra) {
        0
    } else {
        1
    };
    let tail_energy: u32 = coeffs
        .iter()
        .skip(BLOCK_PIXELS.saturating_sub(8))
        .map(|&c| c.unsigned_abs())
        .map(u32::from)
        .sum();
    features[7] = tail_energy.min(127) as i8;
    features
}
#[cfg(test)]
mod rdo_tests {
    use super::*;
    #[test]
    fn token_budget_spends_the_only_token_on_the_larger_reconstruction_gain() {
        // Quantizer 1 gives steps 11 and 99. Magnitude alone would retain
        // level 4, although the negative level 2 saves much more squared error.
        let mut transform = [0; BLOCK_PIXELS];
        transform[0] = 160;
        transform[1] = 44;
        transform[63] = -198;
        let mut levels = quantize_coeffs(&transform, 1);
        let mut optimizer = BlockRdo::new(RdoMode::DynamicProgramming, 1, 2, None).unwrap();
        optimizer.select_levels(&transform, &mut levels, FrameType::Intra, 0);
        assert_eq!((levels[0], levels[1], levels[63]), (10, 0, -2));
        assert_eq!(
            (
                optimizer.telemetry.before_rate_bits,
                optimizer.telemetry.after_rate_bits
            ),
            (88, 64)
        );
        assert_eq!(optimizer.telemetry.distortion_penalty, 16);
        let mut block = Vec::new();
        encode_block_rle(
            &levels,
            &mut 0,
            &mut block,
            &mut NoopBundledHooks,
            0,
            FrameType::Intra,
        );
        assert_eq!(optimizer.telemetry.after_rate_bits, block.len() as u64 * 8);
    }
    #[test]
    fn neural_predictor_is_deterministic() {
        let predictor = NeuralPredictor::from_seed(DEFAULT_NEURAL_SEED);
        let coeffs = [0i16; BLOCK_PIXELS];
        let features = build_neural_features(&coeffs, 0, 2, 8, FrameType::Intra, 1);
        let first = predictor.predict(&features);
        let second = predictor.predict(&features);
        assert_eq!(first, second);
    }
    #[test]
    fn context_frequency_records_symbols() {
        let dims = FrameDimensions::new(8, 8);
        let tables = default_bundle_tables();
        let mut slots = Vec::new();
        let mut recorder = BundleStreamRecorder::new(
            &mut slots,
            2,
            dims,
            4,
            tables,
            BundleAcceleration::None,
            0,
            None,
        );
        recorder.record_bundle(
            BundleType::SignAndMagnitude,
            BundleContextId::new(7),
            0b11,
            2,
            BundleFlushReason::TypeComplete,
        );
        let telemetry = recorder.finish(None);
        let entry = telemetry
            .context_frequencies
            .iter()
            .find(|entry| entry.context == BundleContextId::new(7))
            .expect("context frequency recorded");
        assert_eq!(entry.bundles, 1);
        assert_eq!(entry.symbol_counts[3], 1);
    }
    #[test]
    fn context_remap_loader_maps_and_escapes() {
        let path = std::env::temp_dir().join("remap_loader.json");
        let json = r#"
            {
                "escape_context": 65535,
                "kept": [{ "original": 7, "remapped": 1 }],
                "dropped": [{ "original": 9 }]
            }"#;
        std::fs::write(&path, json).expect("write remap json");
        let remap =
            load_bundle_context_remap_from_json(&path).expect("remap should parse correctly");
        assert_eq!(remap.map(BundleContextId::new(7)), BundleContextId::new(1));
        assert_eq!(
            remap.map(BundleContextId::new(9)),
            BundleContextId::new(u16::MAX)
        );
        assert_eq!(remap.remapped_count(), 1);
        assert_eq!(remap.dropped_count(), 1);
        let _ = std::fs::remove_file(&path);
    }
}
/// Literal range and counters for one frequently occurring low-frequency pattern.
#[derive(Clone, Copy, Default)]
struct SparseAcPattern {
    // A restored prefix gets its first occurrence counted with the repeats.
    deferred_prefix: bool,
    key: u16,
    last_slot: usize,
    nonzero: u8,
    negative: u8,
    start: usize,
    end: usize,
    repeats: u64,
}

// For support inside the upper-left 3x3, orders 25 and 26 are zero, with
// identical context inputs except for their zigzag order. Without a remap
// they always end separate RLE records. Every subsequent context is neutral.
const ZERO_TAIL_ORDER: usize = 26;
// DC and the first horizontal and vertical AC levels cannot influence orders
// 11 or later. Orders 11 and 12 have identical neutral inputs except for order.
const FIRST_AC_TAIL_ORDER: usize = 12;
const fn isolated_prefix(end: usize) -> u64 {
    let mut mask = 0;
    let mut z = 0;
    while z < BLOCK_PIXELS {
        let slot = ZIG_ZAG[z];
        let x = slot % 8;
        let y = slot / 8;
        let mut clean = z + 1 < end - 1;
        let mut w = end - 1;
        while w < 64 && clean {
            let s = ZIG_ZAG[w];
            let (a, b) = (s % 8, s / 8);
            if (a == x + 1 && b == y) || (b == y + 1 && a.abs_diff(x) <= 1) {
                clean = false;
            }
            w += 1;
        }
        if clean {
            mask |= 1 << z;
        }
        z += 1;
    }
    mask
}

const TILE_CLASSES: usize = 64 * 2;
const FIRST_FIVE_CLASSES: usize = 3_usize.pow(5);
// Suffix keys follow the 256 tile/frame/DC keys per class. u16::MAX marks an unkeyed span.
const TAIL_KEY: u16 = (FIRST_FIVE_CLASSES * 256) as u16;
// Internal repeat tape instruction, followed by a starting tile key and run length.
const DC_RUN_KEY: u16 = u16::MAX - 1;
const _: () = assert!(FIRST_FIVE_CLASSES * 256 + 2 * TILE_CLASSES <= u16::MAX as usize);

/// Both full pattern keys and standalone suffix keys name the same neutral tails.
fn pattern_tail(key: u16) -> usize {
    if key >= TAIL_KEY {
        usize::from(key - TAIL_KEY)
    } else {
        // The low seven bits identify tile and frame. Classes 0..9 use only H and V.
        usize::from(key & 127) + if key >> 8 >= 9 { TILE_CLASSES } else { 0 }
    }
}

#[derive(Clone, Copy, Default)]
struct ZeroTail {
    start: usize,
    end: usize,
    last_slot: usize,
    repeats: u64,
}

// For ternary levels the occupied positions determine all context IDs.
// Signs affect only the sign bit in the corresponding nonzero records.
const SPARSE_MASKS: [(u8, u8); FIRST_FIVE_CLASSES] = {
    let mut masks = [(0, 0); FIRST_FIVE_CLASSES];
    let mut class = 0;
    while class < masks.len() {
        let mut digits = class;
        let mut order = 0;
        while order < 5 {
            let trit = digits % 3;
            masks[class].0 |= ((trit != 0) as u8) << order;
            masks[class].1 |= ((trit == 2) as u8) << order;
            digits /= 3;
            order += 1;
        }
        class += 1;
    }
    masks
};

#[derive(Clone, Copy, Default)]
struct PrefixSpan {
    start: u32,
    len: u8,
    signs: u8,
    sign_positions: [u8; 5],
}

#[derive(Default)]
struct SparsePrefixes {
    // At most 32 support masks × 256 tile/frame/DC combinations per encoder.
    pages: [Vec<PrefixSpan>; 32],
    records: Vec<BundleRecord>,
}

impl SparsePrefixes {
    fn append(
        &self,
        key: usize,
        signs: u8,
        width: u8,
        dest: &mut Vec<BundleRecord>,
    ) -> Option<BundleContextId> {
        let span = self.pages[key / 256].get(key % 256)?;
        if span.len == 0 {
            return None;
        }
        let start = span.start as usize;
        let prefix = &self.records[start..start + usize::from(span.len)];
        let out = dest.len();
        dest.extend_from_slice(prefix);
        if width >= 2 {
            let mut changed = signs ^ span.signs;
            while changed != 0 {
                let position = span.sign_positions[changed.trailing_zeros() as usize];
                dest[out + usize::from(position)].bits ^= 1 << (width - 2);
                changed &= changed - 1;
            }
        }
        Some(prefix[prefix.len() - 1].context)
    }

    fn insert(&mut self, key: usize, signs: u8, prefix: &[BundleRecord]) {
        let page = &mut self.pages[key / 256];
        if page.is_empty() {
            page.resize(256, PrefixSpan::default());
        }
        let mut span = PrefixSpan {
            start: self.records.len() as u32,
            len: prefix.len() as u8,
            signs,
            sign_positions: [0; 5],
        };
        let mut active = key / 256;
        for (index, record) in prefix.iter().enumerate() {
            if !matches!(record.bundle_type, BundleType::SignificanceRle) {
                span.sign_positions[active.trailing_zeros() as usize] = index as u8;
                active &= active - 1;
            }
        }
        debug_assert_eq!(active, 0);
        self.records.extend_from_slice(prefix);
        page[key % 256] = span;
    }
}

/// Exact sparse-pattern keys for aligned spans in this encoder's completed report.
struct RecordedSegment {
    telemetry: BundledTelemetry,
    repeat_keys: Vec<u16>,
}

/// Reuse a completed report's allocation when it fits this segment at its exact size.
fn telemetry_buffer<T>(mut previous: Vec<T>, len: usize) -> Vec<T> {
    if previous.capacity() == len {
        previous.clear();
        previous
    } else {
        Vec::with_capacity(len)
    }
}

/// Most AC records describe a single zero. Keep its multiplicity and the
/// inter-record transitions together; allocate a full histogram only for exceptions.
#[derive(Default)]
struct ContextTally {
    stays: u64,
    ends: u64,
    detail: Option<std::num::NonZeroU32>,
}

struct DcSpan {
    start: usize,
    end: usize,
    kind: FrameType,
    significant: bool,
}

struct BundleStreamRecorder<'a> {
    stats: BundledStats,
    tokens: Vec<BundledToken>,
    bundles: Vec<BundleRecord>,
    // Singleton counts use wire IDs directly. Only exceptional transitions need a tally slot.
    context_slots: &'a mut [u32],
    active_contexts: Vec<u64>,
    singleton_counts: Vec<u64>,
    contexts: Vec<ContextTally>,
    details: Vec<(BundleContextStats, ContextFrequency)>,
    // Tile, frame class, DC significance and five ternary AC levels identify the records.
    sparse_patterns: Vec<SparseAcPattern>,
    pattern_pages: [Vec<u16>; FIRST_FIVE_CLASSES],
    cached_prefixes: Option<&'a mut SparsePrefixes>,
    zero_tails: Vec<ZeroTail>,
    omitted_tail_records: usize,
    // Template keys and compact raster DC runs. Literal spans mark the gaps between repeats.
    repeats: Vec<u16>,
    literal_spans: Vec<(usize, usize)>,
    width: u8,
    blocks_per_row: usize,
    quantizer_bucket: u8,
    tables: Arc<BundleAnsTables>,
    acceleration: BundleAcceleration,
    prefetch_distance: u16,
    pending_dc: Option<DcSpan>,
    last_context_slot: Option<usize>,
    context_remap: Option<Arc<BundleContextRemap>>,
}
impl<'a> BundleStreamRecorder<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        context_slots: &'a mut Vec<u32>,
        bundle_width: u8,
        dims: FrameDimensions,
        quantizer: u8,
        tables: Arc<BundleAnsTables>,
        acceleration: BundleAcceleration,
        prefetch_distance: u16,
        context_remap: Option<Arc<BundleContextRemap>>,
    ) -> Self {
        context_slots.clear();
        context_slots.resize(usize::from(u16::MAX) + 1, 0);
        Self {
            stats: BundledStats {
                bundle_width,
                ..BundledStats::default()
            },
            tokens: Vec::with_capacity(dims.block_count() * 3),
            bundles: Vec::new(),
            singleton_counts: vec![0; 1 << 16],
            active_contexts: vec![0; (usize::from(u16::MAX) + 1) / 64],
            context_slots,
            contexts: Vec::with_capacity(dims.block_count().min(8192)),
            details: Vec::new(),
            sparse_patterns: vec![SparseAcPattern::default(); 64 * 2 * 2],
            pattern_pages: std::array::from_fn(|_| Vec::new()),
            cached_prefixes: None,
            zero_tails: vec![ZeroTail::default(); 2 * TILE_CLASSES],
            omitted_tail_records: 0,
            repeats: Vec::with_capacity(dims.block_count()),
            literal_spans: Vec::new(),
            width: bundle_width.clamp(1, 4),
            blocks_per_row: (dims.width as usize / BLOCK_SIZE).max(1),
            quantizer_bucket: quantizer_to_bucket(quantizer),
            tables,
            acceleration,
            prefetch_distance,
            pending_dc: None,
            last_context_slot: None,
            context_remap,
        }
    }
    #[inline(always)]
    fn context_slot(&mut self, context: BundleContextId) -> usize {
        let id = usize::from(context.0);
        self.active_contexts[id / 64] |= 1_u64 << (id % 64);
        id
    }

    fn exceptional_slot(&mut self, id: usize) -> usize {
        let slot = &mut self.context_slots[id];
        if *slot == 0 {
            self.contexts.push(ContextTally::default());
            *slot = self.contexts.len() as u32;
        }
        (*slot - 1) as usize
    }
    fn exceptional(&mut self, id: usize) -> &mut ContextTally {
        let slot = self.exceptional_slot(id);
        &mut self.contexts[slot]
    }
    fn map_context(&self, context: BundleContextId) -> BundleContextId {
        if let Some(remap) = self.context_remap.as_ref() {
            remap.map(context)
        } else {
            context
        }
    }
    #[cfg(test)]
    fn finish(self, previous: Option<BundledTelemetry>) -> BundledTelemetry {
        self.finish_segment(previous.map(|telemetry| RecordedSegment {
            telemetry,
            repeat_keys: Vec::new(),
        }))
        .telemetry
    }

    fn count_repeated_patterns(&mut self) {
        for index in 0..self.sparse_patterns.len() {
            let pattern = self.sparse_patterns[index];
            let count = pattern.repeats + u64::from(pattern.deferred_prefix);
            if count == 0 {
                continue;
            }
            let nonzero = u64::from(pattern.nonzero);
            let negative = u64::from(pattern.negative);
            self.stats.significance_one = self
                .stats
                .significance_one
                .saturating_add(nonzero.saturating_mul(count));
            // Repeated sparse blocks share their neutral suffix across all AC classes.
            let (prefix_end, ac_slots) = if self.context_remap.is_none() {
                let tail_index = pattern_tail(pattern.key);
                let tail = &mut self.zero_tails[tail_index];
                tail.repeats = tail.repeats.saturating_add(pattern.repeats);
                (
                    pattern.end - (tail.end - tail.start),
                    if tail_index < TILE_CLASSES {
                        FIRST_AC_TAIL_ORDER - 1
                    } else {
                        ZERO_TAIL_ORDER - 1
                    } as u64,
                )
            } else {
                (pattern.end, (BLOCK_PIXELS - 1) as u64)
            };
            self.stats.significance_zero = self
                .stats
                .significance_zero
                .saturating_add((ac_slots - nonzero).saturating_mul(count));
            self.stats.sign_negative = self
                .stats
                .sign_negative
                .saturating_add(negative.saturating_mul(count));
            self.stats.sign_positive = self
                .stats
                .sign_positive
                .saturating_add((ac_slots - negative).saturating_mul(count));
            if self.width >= 3 {
                self.stats.parity_one = self
                    .stats
                    .parity_one
                    .saturating_add(nonzero.saturating_mul(count));
            }
            // A block starts after EOB. Its internal context switches and histogram
            // are additive; record_eob has already counted its terminal context.
            self.last_context_slot = None;
            for i in pattern.start..prefix_end {
                self.count_bundle(self.bundles[i], count);
            }
        }
        for i in 0..self.zero_tails.len() {
            let tail = self.zero_tails[i];
            if tail.repeats != 0 {
                let start_order = if i < TILE_CLASSES {
                    FIRST_AC_TAIL_ORDER
                } else {
                    ZERO_TAIL_ORDER
                };
                let zeros = ((BLOCK_PIXELS - start_order) as u64).saturating_mul(tail.repeats);
                self.stats.significance_zero = self.stats.significance_zero.saturating_add(zeros);
                self.stats.sign_positive = self.stats.sign_positive.saturating_add(zeros);
                for record in &self.bundles[tail.start..tail.end] {
                    let id = usize::from(record.context.0);
                    self.active_contexts[id / 64] |= 1 << (id % 64);
                    self.singleton_counts[id] =
                        self.singleton_counts[id].saturating_add(tail.repeats);
                }
            }
        }
    }

    fn context_reports(
        &mut self,
        previous: Option<&mut BundledTelemetry>,
        unfinished: Option<usize>,
    ) -> (Vec<BundleContextStats>, Vec<ContextFrequency>) {
        let (mut context_stats, mut context_frequencies) = previous
            .map(|p| {
                (
                    std::mem::take(&mut p.context_stats),
                    std::mem::take(&mut p.context_frequencies),
                )
            })
            .unwrap_or_default();
        let live: usize = self
            .active_contexts
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum();
        // Most contexts contain only singleton zeros. Initialize their unused
        // histogram bins together before writing the occupied bins in wire order.
        context_stats.clear();
        context_frequencies.clear();
        if context_stats.capacity() < live {
            // SAFETY: all fields are unsigned integers or a BundleContextId(u16).
            context_stats =
                unsafe { Box::<[BundleContextStats]>::new_zeroed_slice(live).assume_init() }
                    .into_vec();
        } else {
            context_stats.resize(live, BundleContextStats::new(BundleContextId(0)));
        }
        if context_frequencies.capacity() < live {
            // SAFETY: the ID, counts, and histogram elements all admit zero.
            context_frequencies =
                unsafe { Box::<[ContextFrequency]>::new_zeroed_slice(live).assume_init() }
                    .into_vec();
        } else {
            context_frequencies.resize(live, ContextFrequency::default());
        }
        let mut reports = context_stats.iter_mut().zip(&mut context_frequencies);
        let empty = ContextTally::default();
        // Visit only live contexts, in increasing wire-ID order.
        for (word, &occupied) in self.active_contexts.iter().enumerate() {
            let mut remaining = occupied;
            while remaining != 0 {
                let id = word * 64 + remaining.trailing_zeros() as usize;
                remaining &= remaining - 1;
                let slot = self.context_slots[id];
                let context = BundleContextId(id as u16);
                let tally = if slot == 0 {
                    &empty
                } else {
                    &self.contexts[slot as usize - 1]
                };
                let (stats, freq) = reports.next().expect("one report per occupied context");
                if let Some(entry) = tally.detail {
                    (*stats, *freq) = self.details[entry.get() as usize - 1];
                } else {
                    stats.context = context;
                    freq.context = context;
                }
                let zeros = self.singleton_counts[id];
                stats.bundles_total = stats.bundles_total.saturating_add(zeros);
                stats.type_significance_rle = stats.type_significance_rle.saturating_add(zeros);
                stats.flush_type_complete = stats.flush_type_complete.saturating_add(zeros);
                self.stats.bundles_total =
                    self.stats.bundles_total.saturating_add(stats.bundles_total);
                self.stats.significance_rle = self
                    .stats
                    .significance_rle
                    .saturating_add(stats.type_significance_rle);
                self.stats
                    .record_flush(BundleFlushReason::TypeComplete, stats.flush_type_complete);
                // The global EOB count already includes the explicit block terminators.
                self.stats
                    .record_flush(BundleFlushReason::EndOfBlock, stats.flush_end_of_block);
                // Partition outgoing edges into switches, same-context edges and block ends.
                // A final unterminated record has no outgoing edge either.
                let switches = stats.bundles_total
                    - tally.stays
                    - tally.ends
                    - u64::from(unfinished == Some(id));
                stats.flush_context_switch = stats.flush_context_switch.saturating_add(switches);
                self.stats
                    .record_flush(BundleFlushReason::ContextSwitch, stats.flush_context_switch);
                stats.flush_end_of_block = stats.flush_end_of_block.saturating_add(tally.ends);
                freq.bundles = stats.bundles_total;
                freq.total_bits = freq
                    .total_bits
                    .saturating_add(zeros.saturating_mul(u64::from(self.width.max(2))));
                freq.symbol_counts[1] = freq.symbol_counts[1].saturating_add(zeros);
            }
        }
        (context_stats, context_frequencies)
    }

    fn expand_repeats(
        &self,
        previous: Option<RecordedSegment>,
    ) -> (Vec<BundleRecord>, Vec<u8>, Vec<u16>) {
        // Each repeat contains SignificanceRle. Its entropy stream is the
        // masked symbol projection, which distributes over concatenation.
        let symbols: Vec<_> = self.bundles.iter().map(bundle_symbol).collect();
        let total = self
            .sparse_patterns
            .iter()
            .fold(self.bundles.len() + self.omitted_tail_records, |len, p| {
                len.saturating_add((p.end - p.start).saturating_mul(p.repeats as usize))
            });
        let (old_records, old_stream, old_keys) = previous
            .map(|r| (r.telemetry.bundles, r.telemetry.ans_stream, r.repeat_keys))
            .unwrap_or_default();
        let overwrite = old_records.len() == total
            && old_stream.len() == total
            && old_records.capacity() == total
            && old_stream.capacity() == total;
        let (mut records, mut stream) = if overwrite {
            (old_records, old_stream)
        } else {
            (
                telemetry_buffer(old_records, total),
                telemetry_buffer(old_stream, total),
            )
        };
        if !overwrite {
            // In a full-length block each zero is one significance-RLE record for symbol 1.
            stream.resize(total, 1);
        }
        const SLOTS: usize = BLOCK_PIXELS - 1;
        // Each row covers the eight tile columns for one frame/DC class.
        // Consecutive DC-only blocks can be copied from a contiguous period,
        // even when a literal interrupts the run before it completes a cycle.
        let mut dc_rows = [None; 32];
        let mut periods = Vec::new();
        for (group, row) in self.sparse_patterns[..256].chunks_exact(8).enumerate() {
            if row.iter().all(|p| p.end - p.start == SLOTS)
                && row.iter().map(|p| p.repeats).sum::<u64>() >= 16
            {
                dc_rows[group] = Some(periods.len());
                if !overwrite {
                    let start = periods.len();
                    for p in row {
                        periods.extend_from_slice(&self.bundles[p.start..p.end]);
                    }
                    // Keep a contiguous 32-block seed at each of the eight phases.
                    for _ in 0..2 {
                        periods.extend_from_within(start..);
                    }
                    periods.extend_from_within(start..start + 8 * SLOTS);
                }
            }
        }
        // These entries describe the old output until their block has been
        // visited. Equal runs can retain both the records and their keys.
        let reuse_keys = overwrite && old_keys.len() == total / SLOTS;
        let mut repeat_keys = if reuse_keys {
            old_keys
        } else {
            vec![u16::MAX; total / SLOTS]
        };
        let mut written = 0_usize;
        enum Source {
            Span(usize, usize, Option<u16>),
            DcRun(u16, usize, usize),
        }
        let mut append = |source| {
            let (start, end, key) = match source {
                Source::Span(start, end, key) => (start, end, key),
                Source::DcRun(key, len, row) => {
                    let n = len * SLOTS;
                    let aligned = written.is_multiple_of(SLOTS);
                    let slot = written / SLOTS;
                    let period: [u16; 8] =
                        std::array::from_fn(|i| (key & !7) | ((key + i as u16) & 7));
                    if overwrite {
                        if !aligned
                            || !reuse_keys
                            || !repeat_keys.get(slot..slot + len).is_some_and(|prior| {
                                prior.chunks(8).all(|chunk| chunk == &period[..chunk.len()])
                            })
                        {
                            return false;
                        }
                    } else {
                        let start = row + usize::from(key & 7) * SLOTS;
                        let seed = n.min(32 * SLOTS);
                        records.extend_from_slice(&periods[start..start + seed]);
                        while records.len() < written + n {
                            let len = (records.len() - written).min(written + n - records.len());
                            records.extend_from_within(written..written + len);
                        }
                    }
                    if aligned && !overwrite {
                        for chunk in repeat_keys[slot..slot + len].chunks_mut(8) {
                            chunk.copy_from_slice(&period[..chunk.len()]);
                        }
                    }
                    written += n;
                    return true;
                }
            };
            let dest = written;
            written += end - start;
            let mut copy_end = written;
            if let Some(key) = key
                && written.is_multiple_of(SLOTS)
                && written > 0
                && (key >= TAIL_KEY || end - start == SLOTS)
            {
                let slot = written / SLOTS - 1;
                let old_key = std::mem::replace(&mut repeat_keys[slot], key);
                if reuse_keys {
                    if key == old_key {
                        return true;
                    }
                    if self.context_remap.is_none() && old_key != u16::MAX {
                        let tail_id = pattern_tail(key);
                        if tail_id == pattern_tail(old_key) {
                            let tail = self.zero_tails[tail_id];
                            copy_end -= tail.end - tail.start;
                        }
                    }
                }
            } else if reuse_keys {
                repeat_keys[dest / SLOTS..written / SLOTS].fill(u16::MAX);
            }
            if overwrite {
                let n = copy_end - dest;
                records[dest..copy_end].copy_from_slice(&self.bundles[start..start + n]);
                stream[dest..copy_end].copy_from_slice(&symbols[start..start + n]);
            } else {
                if end - start == SLOTS {
                    let block = self.bundles[start..]
                        .first_chunk::<SLOTS>()
                        .expect("complete template");
                    records.extend_from_slice(block);
                } else {
                    records.extend_from_slice(&self.bundles[start..end]);
                }
                let symbol_len = match key {
                    Some(k) if k < TAIL_KEY && end - start == SLOTS => {
                        // A sparse template can have nonzeros only in the first five AC orders.
                        if k >> 8 == 0 { 0 } else { 5 }
                    }
                    Some(k) if k >= TAIL_KEY => {
                        let order = if pattern_tail(k) < TILE_CLASSES {
                            FIRST_AC_TAIL_ORDER
                        } else {
                            ZERO_TAIL_ORDER
                        };
                        if end - start == BLOCK_PIXELS - order {
                            0
                        } else {
                            end - start
                        }
                    }
                    _ => end - start,
                };
                stream[dest..dest + symbol_len]
                    .copy_from_slice(&symbols[start..start + symbol_len]);
            }
            true
        };
        let (mut repeat_start, mut literal_start) = (0, 0);
        for (repeat_end, literal_end) in self
            .literal_spans
            .iter()
            .copied()
            .chain(std::iter::once((self.repeats.len(), self.bundles.len())))
        {
            let mut keys = &self.repeats[repeat_start..repeat_end];
            while let Some((&key, rest)) = keys.split_first() {
                if key == DC_RUN_KEY {
                    let (first, len) = (rest[0], usize::from(rest[1]));
                    if !dc_rows[usize::from(first) / 8]
                        .is_some_and(|row| append(Source::DcRun(first, len, row)))
                    {
                        for i in 0..len {
                            let k = (first & !7) | ((first + i as u16) & 7);
                            let p = self.sparse_patterns[usize::from(k)];
                            append(Source::Span(p.start, p.end, Some(p.key)));
                        }
                    }
                    keys = &rest[2..];
                    continue;
                }

                if key < 256
                    && keys.len() >= 4
                    && let Some(row) = dc_rows[usize::from(key) / 8]
                {
                    let base = key & !7;
                    let mut len = 1;
                    while len < keys.len() && keys[len] == base | (key.wrapping_add(len as u16) & 7)
                    {
                        len += 1;
                    }
                    if len >= 4 && append(Source::DcRun(key, len, row)) {
                        keys = &keys[len..];
                        continue;
                    }
                    // A changed prior key requires the usual per-block suffix check.
                    if len >= 4 {
                        for &k in &keys[..len] {
                            let p = self.sparse_patterns[usize::from(k)];
                            append(Source::Span(p.start, p.end, Some(p.key)));
                        }
                        keys = &keys[len..];
                        continue;
                    }
                }
                {
                    if key >= TAIL_KEY {
                        let tail = self.zero_tails[usize::from(key - TAIL_KEY)];
                        append(Source::Span(tail.start, tail.end, Some(key)));
                    } else {
                        let p = self.sparse_patterns[usize::from(key)];
                        append(Source::Span(p.start, p.end, Some(p.key)));
                    }
                    keys = rest;
                }
            }
            append(Source::Span(literal_start, literal_end, None));
            (repeat_start, literal_start) = (repeat_end, literal_end);
        }
        (records, stream, repeat_keys)
    }

    fn finish_segment(mut self, mut previous: Option<RecordedSegment>) -> RecordedSegment {
        self.flush_dc();
        let unfinished = self.last_context_slot;
        self.count_repeated_patterns();
        let (context_stats, context_frequencies) =
            self.context_reports(previous.as_mut().map(|p| &mut p.telemetry), unfinished);
        let (bundles, ans_stream, repeat_keys, used_acceleration) = if self.repeats.is_empty() {
            let (stream, acceleration) = encode_bundle_stream_with_opts(
                &self.tables,
                &self.bundles,
                self.acceleration,
                self.prefetch_distance,
            );
            (self.bundles, stream, Vec::new(), acceleration)
        } else {
            let (bundles, stream, keys) = self.expand_repeats(previous);
            (bundles, stream, keys, BundleAcceleration::None)
        };
        let remap = self.context_remap.as_ref().map(|map| ContextRemapSummary {
            escape_context: map.escape_context(),
            remapped: map.remapped_count(),
            dropped: map.dropped_count(),
        });
        let telemetry = BundledTelemetry {
            stats: self.stats,
            tokens: self.tokens,
            bundles,
            context_frequencies,
            ans_stream,
            ans_precision_bits: self.tables.precision_bits(),
            tables_checksum: self.tables.checksum(),
            context_stats,
            acceleration: used_acceleration,
            prefetch_distance: self.prefetch_distance,
            context_remap: remap,
            rdo: None,
        };
        RecordedSegment {
            telemetry,
            repeat_keys,
        }
    }
    fn record_bundle(
        &mut self,
        bundle_type: BundleType,
        context: BundleContextId,
        bits: u8,
        bit_len: u8,
        flush: BundleFlushReason,
    ) {
        let record = BundleRecord {
            bundle_type,
            context,
            bits,
            bit_len,
            flush,
        };
        self.count_bundle(record, 1);
        self.bundles.push(record);
    }
    fn count_bundle(&mut self, record: BundleRecord, count: u64) {
        let slot = self.context_slot(record.context);
        self.count_transition(slot, count);
        if matches!(record.bundle_type, BundleType::SignificanceRle)
            && record.bits == 1
            && record.bit_len == self.width.max(2)
            && matches!(record.flush, BundleFlushReason::TypeComplete)
        {
            self.singleton_counts[slot] = self.singleton_counts[slot].saturating_add(count);
        } else {
            self.count_detail(slot, record, count);
        }
    }

    #[inline]
    fn count_transition(&mut self, slot: usize, count: u64) {
        if self.last_context_slot.replace(slot) == Some(slot) {
            let stays = &mut self.exceptional(slot).stays;
            *stays = stays.saturating_add(count);
        }
    }

    fn count_detail(&mut self, slot: usize, record: BundleRecord, count: u64) {
        let slot = self.exceptional_slot(slot);
        let tally = &mut self.contexts[slot];
        let entry = *tally.detail.get_or_insert_with(|| {
            self.details.push((
                BundleContextStats::new(record.context),
                ContextFrequency {
                    context: record.context,
                    ..ContextFrequency::default()
                },
            ));
            std::num::NonZeroU32::new(self.details.len() as u32).expect("at most 65536 contexts")
        });
        let (stats, freq) = &mut self.details[entry.get() as usize - 1];
        stats.record_bundle(record.bundle_type, record.flush, count);
        freq.total_bits = freq
            .total_bits
            .saturating_add(u64::from(record.bit_len).saturating_mul(count));
        let symbol_idx = usize::from(record.bits) & (RDO_MAX_SYMBOLS - 1);
        freq.symbol_counts[symbol_idx] = freq.symbol_counts[symbol_idx].saturating_add(count);
    }
    fn count_eob(&mut self) {
        self.stats.blocks_encoded = self.stats.blocks_encoded.saturating_add(1);
        if let Some(slot) = self.last_context_slot.take() {
            let tally = self.exceptional(slot);
            tally.ends = tally.ends.saturating_add(1);
        }
        self.stats.record_flush(BundleFlushReason::EndOfBlock, 1);
    }
    #[cfg(test)]
    fn record_block(&mut self, index: usize, coeffs: &[i16; BLOCK_PIXELS], kind: FrameType) {
        self.record_masked_block(index, coeffs, kind, ac_support(coeffs))
    }

    fn record_masked_block(
        &mut self,
        block_index: usize,
        coeffs: &[i16; BLOCK_PIXELS],
        frame_type: FrameType,
        active: u64,
    ) {
        self.flush_dc();
        let (h, v, vv, hv, hh) = (coeffs[1], coeffs[8], coeffs[16], coeffs[9], coeffs[2]);
        if self.last_context_slot.is_some()
            || [h, v, vv, hv, hh]
                .iter()
                .any(|level| level.unsigned_abs() > 1)
            || active >= 1 << 6
        {
            self.record_ac_slots(self.tile(block_index), coeffs, frame_type, active);
            return;
        }
        // The first five zigzag AC levels form a base-three key.
        let digit = |level: i16| usize::from(level != 0) + usize::from(level < 0);
        let class = digit(h) + 3 * digit(v) + 9 * digit(vv) + 27 * digit(hv) + 81 * digit(hh);
        self.record_pattern(block_index, coeffs, frame_type, class);
    }
    fn tile(&self, block_index: usize) -> u8 {
        let (row, col) = (
            block_index / self.blocks_per_row,
            block_index % self.blocks_per_row,
        );
        ((col & 7) | ((row & 7) << 3)) as u8
    }
    fn mark_literals(&mut self) {
        if self.literal_spans.last().map(|&(_, end)| end) != Some(self.bundles.len()) {
            self.literal_spans
                .push((self.repeats.len(), self.bundles.len()));
        }
    }

    fn flush_dc(&mut self) {
        if let Some(span) = self.pending_dc.take() {
            self.record_dc_patterns(span);
        }
    }

    fn queue_dc(&mut self, blocks: std::ops::Range<usize>, significant: bool, kind: FrameType) {
        if let Some(span) = &mut self.pending_dc
            && span.end == blocks.start
            && span.kind == kind
            && span.significant == significant
        {
            span.end = blocks.end;
        } else {
            self.flush_dc();
            self.pending_dc = Some(DcSpan {
                start: blocks.start,
                end: blocks.end,
                kind,
                significant,
            });
        }
    }

    /// Within a raster row, the eight DC tile keys repeat. Count
    /// each complete cycle once per tile and append its keys in raster order.
    fn record_dc_patterns(&mut self, span: DcSpan) {
        let blocks = span.start..span.end;
        let count = blocks.len() as u64;
        self.stats.dc_events = self.stats.dc_events.saturating_add(count);
        self.stats.blocks_encoded = self.stats.blocks_encoded.saturating_add(count);
        self.stats
            .record_flush(BundleFlushReason::EndOfBlock, count);
        let mut levels = [0; BLOCK_PIXELS];
        levels[0] = i16::from(span.significant);
        let mut first = blocks.start;
        while first < blocks.end {
            let column = first % self.blocks_per_row;
            let mut len = (self.blocks_per_row - column).min(blocks.end - first);
            let base = (usize::from(span.significant) << 7)
                | (usize::from(span.kind.as_byte()) << 6)
                | (((first / self.blocks_per_row) & 7) << 3);
            if len >= 16
                && self.sparse_patterns[base..base + 8]
                    .iter()
                    .all(|p| p.end != 0)
            {
                self.mark_literals();
                self.repeats.extend_from_slice(&[
                    DC_RUN_KEY,
                    (base | (column & 7)) as u16,
                    len as u16,
                ]);
                for phase in 0..8 {
                    let times = ((len + 7 - phase) / 8) as u64;
                    let p = &mut self.sparse_patterns[base | ((column + phase) & 7)];
                    p.repeats += times;
                    let last = p.last_slot;
                    let tally = self.exceptional(last);
                    tally.ends = tally.ends.saturating_add(times);
                }
            } else {
                // One tile period populates every missing template in this row.
                len = len.min(8);
                for i in first..first + len {
                    self.record_pattern(i, &levels, span.kind, 0);
                    let slot = self.last_context_slot.take().expect("completed AC pattern");
                    let tally = self.exceptional(slot);
                    tally.ends = tally.ends.saturating_add(1);
                }
            }
            first += len;
        }
    }

    fn record_pattern(
        &mut self,
        block_index: usize,
        coeffs: &[i16; BLOCK_PIXELS],
        frame_type: FrameType,
        class: usize,
    ) {
        let tile = usize::from(self.tile(block_index));
        let key = (class << 8)
            | tile
            | (usize::from(frame_type.as_byte()) << 6)
            | (usize::from(coeffs[0] != 0) << 7);
        let slot = if class == 0 {
            key
        } else {
            let page = &mut self.pattern_pages[class];
            if page.is_empty() {
                page.resize(256, 0);
            }
            let entry = &mut page[key & 255];
            if *entry == 0 {
                *entry = self.sparse_patterns.len() as u16;
                self.sparse_patterns.push(SparseAcPattern::default());
            }
            usize::from(*entry)
        };
        let pattern = &mut self.sparse_patterns[slot];
        if pattern.end != 0 {
            pattern.repeats += 1;
            let last_slot = pattern.last_slot;
            self.mark_literals();
            self.repeats.push(slot as u16);
            self.last_context_slot = Some(last_slot);
        } else {
            let start = self.bundles.len();
            let (support, signs) = SPARSE_MASKS[class];
            let mut deferred_prefix = false;
            if self.context_remap.is_some() {
                self.record_ac_range(tile as u8, coeffs, frame_type, 1..BLOCK_PIXELS);
            } else {
                let tail = pattern_tail(key as u16);
                let end = if tail < TILE_CLASSES {
                    FIRST_AC_TAIL_ORDER
                } else {
                    ZERO_TAIL_ORDER
                };
                let structural_key = usize::from(support) * 256 + (key & 255);
                let context = self
                    .cached_prefixes
                    .as_deref()
                    .and_then(|cache| {
                        cache.append(structural_key, signs, self.width, &mut self.bundles)
                    })
                    .unwrap_or_else(|| {
                        self.write_sparse_prefix(
                            tile as u8,
                            frame_type,
                            support,
                            signs,
                            coeffs[0] != 0,
                            end,
                        );
                        let prefix = &self.bundles[start..];
                        if let Some(cache) = self.cached_prefixes.as_deref_mut() {
                            cache.insert(structural_key, signs, prefix);
                        }
                        prefix.last().expect("nonempty sparse prefix").context
                    });
                self.last_context_slot = Some(self.context_slot(context));
                deferred_prefix = true;
                self.record_neutral_tail(tile as u8, frame_type, end, tail, true);
            }
            self.sparse_patterns[slot] = SparseAcPattern {
                deferred_prefix,
                key: key as u16,
                last_slot: self
                    .last_context_slot
                    .expect("every AC block emits a bundle"),
                nonzero: support.count_ones() as u8,
                negative: signs.count_ones() as u8,
                start,
                end: self.bundles.len(),
                repeats: 0,
            };
        }
    }

    /// Unmapped ternary blocks have only 32 AC support masks and two DC states.
    /// Emit the prefix now and count it once with the pattern's full multiplicity.
    fn write_sparse_prefix(
        &mut self,
        tile: u8,
        frame: FrameType,
        support: u8,
        signs: u8,
        dc: bool,
        end: usize,
    ) {
        let geometry = &SPARSE_CONTEXT_OFFSETS[usize::from(support) + 32 * usize::from(dc)];
        let state = usize::from(self.quantizer_bucket) * 2 + usize::from(frame.as_byte());
        let table = &CONTEXT_PREFIX[state];
        let positive = bundle_bits(1, self.width);
        let negative = bundle_bits(-1, self.width);
        let zero_width = self.width.max(2);
        let mut run = 0;
        let mut last = BundleContextId::default();
        self.bundles.reserve(end - 1);
        for (i, &offset) in geometry[..end - 1].iter().enumerate() {
            let bit = 1_u32 << i;
            let nonzero = u32::from(support) & bit != 0;
            let (kind, bits, bit_len) = if !nonzero {
                (BundleType::SignificanceOnly, 1, zero_width)
            } else if u32::from(signs) & bit != 0 {
                negative
            } else {
                positive
            };
            let prefix = table[usize::from(kind.as_u8())][usize::from(offset)];
            let context = if self.width == 2 && self.quantizer_bucket == 1 {
                BundleContextId(
                    Q16_TILE_HASH[frame.as_byte() as usize][tile as usize][usize::from(nonzero)]
                        [usize::from(offset)]
                        ^ (i + 1) as u16,
                )
            } else {
                finish_context(prefix, tile, nonzero, i + 1)
            };
            if !nonzero && run != 0 && context == last && run < (1 << zero_width) - 1 {
                run += 1;
                self.bundles.last_mut().expect("started zero run").bits = run;
            } else {
                self.bundles.push(BundleRecord {
                    bundle_type: if nonzero {
                        kind
                    } else {
                        BundleType::SignificanceRle
                    },
                    context,
                    bits,
                    bit_len,
                    flush: BundleFlushReason::TypeComplete,
                });
                run = u8::from(!nonzero);
            }
            last = context;
        }
    }

    fn record_ac_slots(
        &mut self,
        tile: u8,
        coeffs: &[i16; BLOCK_PIXELS],
        frame_type: FrameType,
        active: u64,
    ) {
        let tile_key = usize::from(tile) | (usize::from(frame_type.as_byte()) << 6);
        const LOW: u64 = isolated_prefix(ZERO_TAIL_ORDER);
        const FIRST: u64 = isolated_prefix(FIRST_AC_TAIL_ORDER);
        if self.context_remap.is_some() || active & !LOW != 0 {
            self.record_ac_range(tile, coeffs, frame_type, 1..BLOCK_PIXELS);
            return;
        }
        let first_ac_only = active & !FIRST == 0;
        let (key, start_order) = if first_ac_only {
            (tile_key, FIRST_AC_TAIL_ORDER)
        } else {
            (TILE_CLASSES + tile_key, ZERO_TAIL_ORDER)
        };
        self.record_ac_range(tile, coeffs, frame_type, 1..start_order);
        self.record_neutral_tail(tile, frame_type, start_order, key, false);
    }

    fn record_neutral_tail(
        &mut self,
        tile: u8,
        frame_type: FrameType,
        start_order: usize,
        key: usize,
        contiguous: bool,
    ) {
        let tail = &mut self.zero_tails[key];
        if tail.end != 0 {
            tail.repeats += 1;
            self.last_context_slot = Some(tail.last_slot);
            if contiguous {
                self.bundles.extend_from_within(tail.start..tail.end);
            } else {
                self.omitted_tail_records += tail.end - tail.start;
                self.mark_literals();
                self.repeats.push(TAIL_KEY + key as u16);
            }
        } else {
            let start = self.bundles.len();
            let state = usize::from(self.quantizer_bucket) * 2 + usize::from(frame_type.as_byte());
            let contexts = &NEUTRAL_IDS[state][usize::from(tile)];
            debug_assert_eq!(
                self.last_context_slot,
                Some(usize::from(contexts[start_order - 1].0))
            );
            self.bundles
                .extend(contexts[start_order..].iter().map(|&context| BundleRecord {
                    bundle_type: BundleType::SignificanceRle,
                    context,
                    bits: 1,
                    bit_len: self.width.max(2),
                    flush: BundleFlushReason::TypeComplete,
                }));
            let last = usize::from(contexts[BLOCK_PIXELS - 1].0);
            self.last_context_slot = Some(last);
            self.zero_tails[key] = ZeroTail {
                start,
                end: self.bundles.len(),
                last_slot: last,
                repeats: 1,
            };
        }
    }
    fn record_ac_range(
        &mut self,
        tile: u8,
        coeffs: &[i16; BLOCK_PIXELS],
        frame_type: FrameType,
        orders: std::ops::Range<usize>,
    ) {
        if self.width == 2 && self.quantizer_bucket == 1 && self.context_remap.is_none() {
            self.record_ac_range_impl::<true>(tile, coeffs, frame_type, orders);
        } else {
            self.record_ac_range_impl::<false>(tile, coeffs, frame_type, orders);
        }
    }
    fn record_ac_range_impl<const DEFAULT: bool>(
        &mut self,
        tile: u8,
        coeffs: &[i16; BLOCK_PIXELS],
        frame_type: FrameType,
        orders: std::ops::Range<usize>,
    ) {
        let table = &Q16_TILE_HASH[frame_type.as_byte() as usize][tile as usize];
        let neighbors = neighbor_counts(coeffs);
        let slots = orders.len() as u64;
        let (mut nonzero, mut negative, mut odd, mut geq2) = (0_u64, 0_u64, 0_u64, 0_u64);
        let mut prev_level_abs = 0u8;
        let mut zero_context = BundleContextId::default();
        let mut zero_run = 0;
        for order in orders {
            let slot = ZIG_ZAG[order];
            let coeff = coeffs[slot];
            let (bundle_type, bits, bit_len) = bundle_bits(coeff, self.width);
            let context = if DEFAULT {
                let offset = CONTEXT_POSITION[order]
                    + 4 * usize::from(neighbors[slot].min(3))
                    + usize::from(level_bucket(prev_level_abs));
                BundleContextId(table[usize::from(coeff != 0)][offset] ^ order as u16)
            } else {
                self.map_context(self.compute_context(
                    bundle_type,
                    tile,
                    order,
                    neighbors[slot],
                    coeff,
                    prev_level_abs,
                    frame_type,
                ))
            };
            if zero_run > 0 && (coeff != 0 || context != zero_context) {
                self.emit_zero_run_bundle(zero_context, zero_run);
                zero_run = 0;
            }
            if coeff == 0 {
                zero_run += 1;
                zero_context = context;
            } else {
                nonzero += 1;
                negative += u64::from(coeff < 0);
                odd += u64::from(coeff.unsigned_abs() & 1);
                geq2 += u64::from(coeff.unsigned_abs() >= 2);
                self.record_bundle(
                    bundle_type,
                    context,
                    bits,
                    bit_len,
                    BundleFlushReason::TypeComplete,
                );
            }
            prev_level_abs = coeff.unsigned_abs().min(255) as u8;
        }
        self.stats.significance_one = self.stats.significance_one.saturating_add(nonzero);
        self.stats.significance_zero = self.stats.significance_zero.saturating_add(slots - nonzero);
        self.stats.sign_negative = self.stats.sign_negative.saturating_add(negative);
        self.stats.sign_positive = self.stats.sign_positive.saturating_add(slots - negative);
        if self.width >= 3 {
            self.stats.parity_one = self.stats.parity_one.saturating_add(odd);
        }
        if self.width >= 4 {
            self.stats.geq2_one = self.stats.geq2_one.saturating_add(geq2);
        }
        self.emit_zero_run_bundle(zero_context, zero_run);
    }
    #[inline]
    fn emit_zero_run_bundle(&mut self, context: BundleContextId, run: u16) {
        if run == 1 {
            let slot = self.context_slot(context);
            self.count_transition(slot, 1);
            let tally = &mut self.singleton_counts[slot];
            *tally = tally.saturating_add(1);
            self.bundles.push(BundleRecord {
                bundle_type: BundleType::SignificanceRle,
                context,
                bits: 1,
                bit_len: self.width.max(2),
                flush: BundleFlushReason::TypeComplete,
            });
            return;
        }
        if run > 1 {
            self.emit_long_zero_run(context, run);
        }
    }

    fn emit_long_zero_run(&mut self, context: BundleContextId, mut run: u16) {
        while run > 0 {
            let (bits, bit_len, consumed) = bundle_zero_run_symbol(run, self.width);
            self.record_bundle(
                BundleType::SignificanceRle,
                context,
                bits,
                bit_len,
                BundleFlushReason::TypeComplete,
            );
            run = run.saturating_sub(consumed);
        }
    }
    fn compute_context(
        &self,
        bundle_type: BundleType,
        block_tile: u8,
        zigzag_order: usize,
        neighbor_nz: u8,
        coeff: i16,
        prev_level_abs: u8,
        frame_type: FrameType,
    ) -> BundleContextId {
        let class = CONTEXT_POSITION[zigzag_order];
        let prev = level_bucket(prev_level_abs) as usize;
        let state = usize::from(self.quantizer_bucket) * 2 + usize::from(frame_type.as_byte());
        let prefix = CONTEXT_PREFIX[state][bundle_type.as_u8() as usize]
            [class + usize::from(neighbor_nz.min(3)) * 4 + prev];
        finish_context(prefix, block_tile, coeff != 0, zigzag_order)
    }
}
impl BundledHooks for BundleStreamRecorder<'_> {
    fn record_zero_run(&mut self, blocks: std::ops::Range<usize>, first_diff: i16) {
        let count = blocks.len();
        self.tokens
            .extend_from_slice(&[BundledToken::DcDiff(first_diff), BundledToken::EndOfBlock]);
        append_repeated(
            &mut self.tokens,
            &[[BundledToken::DcDiff(0), BundledToken::EndOfBlock]; 32],
            count - 1,
        );
        self.queue_dc(blocks, false, FrameType::Predicted);
    }
    fn record_dc_only(
        &mut self,
        levels: &[i16; BLOCK_PIXELS],
        diff: i16,
        index: usize,
        kind: FrameType,
    ) {
        self.queue_dc(index..index + 1, levels[0] != 0, kind);
        self.tokens
            .extend_from_slice(&[BundledToken::DcDiff(diff), BundledToken::EndOfBlock]);
    }
    fn record_block_coeffs(
        &mut self,
        coeffs: &[i16; BLOCK_PIXELS],
        active: u64,
        index: usize,
        frame: FrameType,
    ) {
        self.record_masked_block(index, coeffs, frame, active);
    }
    fn record_dc(&mut self, diff: i16) {
        self.stats.dc_events = self.stats.dc_events.saturating_add(1);
        self.tokens.push(BundledToken::DcDiff(diff));
    }
    fn record_ac(&mut self, zeros: u8, value: i16) {
        self.stats.ac_events = self.stats.ac_events.saturating_add(1);
        self.stats.zero_run_total = self.stats.zero_run_total.saturating_add(u64::from(zeros));
        self.stats.max_zero_run = self.stats.max_zero_run.max(zeros);
        if value != 0 {
            self.stats.nonzero_levels = self.stats.nonzero_levels.saturating_add(1);
        }
        self.tokens.push(BundledToken::Ac { run: zeros, value });
    }
    fn record_eob(&mut self) {
        self.count_eob();
        self.tokens.push(BundledToken::EndOfBlock);
    }
}
const BUNDLE_ANS_PRECISION_BITS: u8 = 12;
const BUNDLE_RANS_BYTE_L: u32 = 1 << 23;
#[derive(Clone, Debug)]
struct SymbolTable {
    freq: Vec<u16>,
    start: Vec<u16>,
    precision_bits: u8,
    decode: Vec<AnsSymbol>,
}
#[derive(Clone, Copy, Debug)]
struct AnsSymbol {
    symbol: u16,
    start: u16,
    freq: u16,
}
pub(super) const MAX_BUNDLE_WIDTH: usize = 4;
impl SymbolTable {
    fn from_group(group: &RansGroupTableV1) -> Result<Self, BundleTableError> {
        let width_bits = match group.width_bits {
            0 => infer_group_width_bits(group.group_size)?,
            bits => bits,
        };
        let expected = 1usize << width_bits;
        if expected == 0 || group.frequencies.len() != expected {
            return Err(BundleTableError::InvalidGroup {
                bit_len: 0,
                reason: "group frequencies length mismatch",
            });
        }
        if group.cumulative.len() != expected + 1 {
            return Err(BundleTableError::InvalidGroup {
                bit_len: 0,
                reason: "group cumulative length mismatch",
            });
        }
        if usize::from(group.group_size) != expected {
            return Err(BundleTableError::InvalidGroup {
                bit_len: 0,
                reason: "group size does not match width bits",
            });
        }
        let total = 1u32.checked_shl(group.precision_bits.into()).ok_or(
            BundleTableError::InvalidGroup {
                bit_len: 0,
                reason: "precision bits overflow",
            },
        )?;
        if *group.cumulative.first().unwrap_or(&1) != 0
            || *group.cumulative.last().unwrap_or(&0) != total
        {
            return Err(BundleTableError::InvalidGroup {
                bit_len: 0,
                reason: "cumulative range mismatch",
            });
        }
        let freq_sum: u32 = group
            .frequencies
            .iter()
            .map(|&value| u32::from(value))
            .sum();
        if freq_sum != total {
            return Err(BundleTableError::InvalidGroup {
                bit_len: 0,
                reason: "frequency sum mismatch",
            });
        }
        let mut start = Vec::with_capacity(expected);
        for &value in group.cumulative.iter().take(expected) {
            start.push(
                u16::try_from(value).map_err(|_| BundleTableError::InvalidGroup {
                    bit_len: 0,
                    reason: "cumulative value exceeds u16 range",
                })?,
            );
        }
        let table =
            SymbolTable::from_frequencies(group.frequencies.clone(), start, group.precision_bits);
        Ok(table)
    }
    fn from_frequencies(freq: Vec<u16>, start: Vec<u16>, precision_bits: u8) -> Self {
        let decode = build_decode_table(&freq, &start, precision_bits);
        Self {
            freq,
            start,
            precision_bits,
            decode,
        }
    }
}
fn infer_group_width_bits(group_size: u16) -> Result<u8, BundleTableError> {
    if group_size == 0 {
        return Err(BundleTableError::InvalidGroup {
            bit_len: 0,
            reason: "group size must be > 0",
        });
    }
    let value = u32::from(group_size);
    if value & (value - 1) != 0 {
        return Err(BundleTableError::InvalidGroup {
            bit_len: 0,
            reason: "group size must be a power of two",
        });
    }
    Ok(value.trailing_zeros() as u8)
}
fn build_uniform_symbol_table(bit_len: u8, precision_bits: u8) -> SymbolTable {
    let alphabet = 1usize << bit_len.max(1);
    let precision = 1usize << precision_bits;
    let base_freq = (precision / alphabet) as u16;
    let mut freq = vec![base_freq; alphabet];
    let mut remainder = precision - base_freq as usize * alphabet;
    let mut idx = 0usize;
    while remainder > 0 {
        freq[idx] = freq[idx].saturating_add(1);
        remainder -= 1;
        idx += 1;
        if idx == alphabet {
            idx = 0;
        }
    }
    let mut start = Vec::with_capacity(alphabet);
    let mut cumulative = 0u32;
    for &value in &freq {
        start.push(cumulative as u16);
        cumulative += u32::from(value);
    }
    SymbolTable::from_frequencies(freq, start, precision_bits)
}
fn derive_significance_table(source: &SymbolTable) -> SymbolTable {
    let mut freq = vec![0u16; 2];
    for (symbol, &value) in source.freq.iter().enumerate() {
        let sig = symbol & 1;
        freq[sig] = freq[sig].saturating_add(value);
    }
    let mut start = Vec::with_capacity(2);
    let mut cumulative = 0u32;
    for &value in &freq {
        start.push(cumulative as u16);
        cumulative += u32::from(value);
    }
    SymbolTable::from_frequencies(freq, start, source.precision_bits)
}
fn build_decode_table(freq: &[u16], start: &[u16], precision_bits: u8) -> Vec<AnsSymbol> {
    let precision = 1usize << precision_bits;
    let mut table = vec![
        AnsSymbol {
            symbol: 0,
            start: 0,
            freq: 1,
        };
        precision
    ];
    for (symbol, (&begin, &frequency)) in start.iter().zip(freq.iter()).enumerate() {
        table
            .iter_mut()
            .skip(begin as usize)
            .take(frequency as usize)
            .for_each(|slot| {
                *slot = AnsSymbol {
                    symbol: symbol as u16,
                    start: begin,
                    freq: frequency,
                };
            });
    }
    table
}
#[inline]
fn prefetch_bundle_record(record: &BundleRecord) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
        _mm_prefetch(record as *const _ as *const i8, _MM_HINT_T0);
    }
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!(
            "prfm pldl1keep, [{0}]",
            in(reg) record,
            options(readonly, nostack, preserves_flags)
        );
    }
}
struct BundleRansEncoder<'a> {
    tables: &'a BundleAnsTables,
    precision_bits: u8,
    state: u32,
    buffer: Vec<u8>,
    table_cache: [Option<&'a SymbolTable>; MAX_BUNDLE_WIDTH + 1],
}
impl<'a> BundleRansEncoder<'a> {
    fn new(tables: &'a BundleAnsTables) -> Self {
        let mut table_cache: [Option<&'a SymbolTable>; MAX_BUNDLE_WIDTH + 1] =
            core::array::from_fn(|_| None);
        for bit_len in 1..=tables.max_width() {
            table_cache[bit_len as usize] = Some(tables.table_for_bits(bit_len));
        }
        Self {
            tables,
            precision_bits: tables.precision_bits(),
            state: BUNDLE_RANS_BYTE_L,
            buffer: Vec::new(),
            table_cache,
        }
    }
    fn encode_record(&mut self, record: &BundleRecord) {
        let table = self
            .table_cache
            .get(record.bit_len as usize)
            .and_then(Option::as_ref)
            .copied()
            .unwrap_or_else(|| self.tables.table_for_bits(record.bit_len));
        let symbol = (record.bits & ((1 << record.bit_len) - 1)) as usize;
        self.encode_symbol(symbol, table);
    }
    fn encode_symbol(&mut self, symbol: usize, table: &SymbolTable) {
        let freq = table.freq[symbol] as u32;
        let start = table.start[symbol] as u32;
        while self.state >= (freq << (32 - self.precision_bits)) {
            self.buffer.push((self.state & 0xFF) as u8);
            self.state >>= 8;
        }
        self.state = ((self.state / freq) << self.precision_bits) + (self.state % freq) + start;
    }
    fn finish(mut self) -> Vec<u8> {
        for _ in 0..4 {
            self.buffer.push((self.state & 0xFF) as u8);
            self.state >>= 8;
        }
        self.buffer
    }
}
struct BundleRansDecoder<'a> {
    tables: &'a BundleAnsTables,
    precision_bits: u8,
    buffer: &'a [u8],
    cursor: usize,
    state: u32,
}
impl<'a> BundleRansDecoder<'a> {
    fn new(buffer: &'a [u8], tables: &'a BundleAnsTables) -> Result<Self, BundleDecodeError> {
        if buffer.len() < 4 {
            return Err(BundleDecodeError::TruncatedState);
        }
        let len = buffer.len();
        let mut state = 0u32;
        for shift in 0..4 {
            state |= u32::from(buffer[len - 4 + shift]) << (shift * 8);
        }
        let cursor = len - 4;
        Ok(Self {
            tables,
            precision_bits: tables.precision_bits(),
            buffer,
            cursor,
            state,
        })
    }
    fn decode_record(&mut self, record: &BundleRecord) -> Result<u8, BundleDecodeError> {
        let table = self.tables.table_for_bits(record.bit_len);
        self.decode_symbol(table).map(|sym| sym as u8)
    }
    fn decode_symbol(&mut self, table: &SymbolTable) -> Result<u16, BundleDecodeError> {
        let mask = (1u32 << self.precision_bits) - 1;
        let idx = (self.state & mask) as usize;
        let sym = table.decode[idx];
        let scaled = self.state >> self.precision_bits;
        self.state = sym.freq as u32 * scaled + (idx as u32 - sym.start as u32);
        self.renormalize()?;
        Ok(sym.symbol)
    }
    fn renormalize(&mut self) -> Result<(), BundleDecodeError> {
        while self.state < BUNDLE_RANS_BYTE_L && self.cursor > 0 {
            self.cursor -= 1;
            self.state = (self.state << 8) | u32::from(self.buffer[self.cursor]);
        }
        if self.state < BUNDLE_RANS_BYTE_L && self.cursor == 0 {
            return Err(BundleDecodeError::RenormalizeUnderflow);
        }
        Ok(())
    }
}
const SIMD_BUNDLE_MAGIC: [u8; 4] = *b"BR4\x01";
#[cfg(test)]
fn encode_bundle_stream(tables: &BundleAnsTables, bundles: &[BundleRecord]) -> Vec<u8> {
    let (stream, _acceleration) =
        encode_bundle_stream_with_opts(tables, bundles, BundleAcceleration::None, 0);
    stream
}
fn bundle_symbol(record: &BundleRecord) -> u8 {
    record.bits & ((1u8 << record.bit_len) - 1)
}

fn encode_bundle_stream_with_opts(
    tables: &BundleAnsTables,
    bundles: &[BundleRecord],
    requested: BundleAcceleration,
    prefetch_distance: u16,
) -> (Vec<u8>, BundleAcceleration) {
    let has_rle = bundles
        .iter()
        .any(|record| matches!(record.bundle_type, BundleType::SignificanceRle));
    if has_rle {
        let stream = bundles.iter().map(bundle_symbol).collect();
        // RLE bundles are scalar-only until SIMD tables are calibrated.
        return (stream, BundleAcceleration::None);
    }
    if requested == BundleAcceleration::CpuSimd && cpu_simd_supported() {
        let stream = encode_bundle_stream_simd(tables, bundles, prefetch_distance);
        let acceleration = if stream.starts_with(&SIMD_BUNDLE_MAGIC) {
            BundleAcceleration::CpuSimd
        } else {
            BundleAcceleration::None
        };
        (stream, acceleration)
    } else {
        (
            encode_bundle_stream_scalar(tables, bundles, prefetch_distance),
            BundleAcceleration::None,
        )
    }
}
fn encode_bundle_stream_scalar(
    tables: &BundleAnsTables,
    bundles: &[BundleRecord],
    prefetch_distance: u16,
) -> Vec<u8> {
    let mut encoder = BundleRansEncoder::new(tables);
    let distance = prefetch_distance as usize;
    for rev_idx in 0..bundles.len() {
        let idx = bundles.len() - 1 - rev_idx;
        if distance > 0
            && let Some(prefetch_idx) = idx.checked_sub(distance)
        {
            prefetch_bundle_record(&bundles[prefetch_idx]);
        }
        encoder.encode_record(&bundles[idx]);
    }
    encoder.finish()
}
fn encode_bundle_stream_simd(
    tables: &BundleAnsTables,
    bundles: &[BundleRecord],
    prefetch_distance: u16,
) -> Vec<u8> {
    let mut states = [BUNDLE_RANS_BYTE_L; 4];
    let mut buffers: [Vec<u8>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
    let distance = prefetch_distance as usize;
    let precision_bits = tables.precision_bits();
    let mut table_cache: [Option<&SymbolTable>; MAX_BUNDLE_WIDTH + 1] =
        core::array::from_fn(|_| None);
    for bit_len in 1..=tables.max_width() {
        table_cache[bit_len as usize] = Some(tables.table_for_bits(bit_len));
    }
    for rev_idx in 0..bundles.len() {
        let idx = bundles.len() - 1 - rev_idx;
        if distance > 0
            && let Some(prefetch_idx) = idx.checked_sub(distance)
        {
            prefetch_bundle_record(&bundles[prefetch_idx]);
        }
        let lane = rev_idx & 3;
        let record = &bundles[idx];
        let table = table_cache[record.bit_len as usize]
            .unwrap_or_else(|| tables.table_for_bits(record.bit_len));
        let symbol = (record.bits & ((1 << record.bit_len) - 1)) as usize;
        let state = &mut states[lane];
        let buffer = &mut buffers[lane];
        let freq = table.freq[symbol] as u32;
        let start = table.start[symbol] as u32;
        while *state >= (freq << (32 - precision_bits)) {
            buffer.push((*state & 0xFF) as u8);
            *state >>= 8;
        }
        *state = ((*state / freq) << precision_bits) + (*state % freq) + start;
    }
    for (state, buffer) in states.iter_mut().zip(buffers.iter_mut()) {
        for _ in 0..4 {
            buffer.push((*state & 0xFF) as u8);
            *state >>= 8;
        }
    }
    let mut payload_len = SIMD_BUNDLE_MAGIC.len();
    let mut lane_lengths = [0usize; 4];
    let mut overflowed = false;
    for (idx, buf) in buffers.iter().enumerate() {
        lane_lengths[idx] = buf.len();
        if lane_lengths[idx] > u32::MAX as usize {
            overflowed = true;
        }
        payload_len = payload_len.saturating_add(4).saturating_add(buf.len());
    }
    if overflowed {
        // SIMD header stores lane lengths as u32, so fall back to scalar on overflow.
        return encode_bundle_stream_scalar(tables, bundles, prefetch_distance);
    }
    let mut out = Vec::with_capacity(payload_len);
    out.extend_from_slice(&SIMD_BUNDLE_MAGIC);
    for len in lane_lengths {
        let len_u32 = len as u32;
        out.extend_from_slice(&len_u32.to_le_bytes());
    }
    for buf in buffers {
        out.extend_from_slice(&buf);
    }
    out
}
fn cpu_simd_supported() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return std::arch::is_x86_feature_detected!("avx2");
    }
    #[cfg(target_arch = "aarch64")]
    {
        return std::arch::is_aarch64_feature_detected!("neon");
    }
    #[allow(unreachable_code)]
    false
}
/// Decode a bundled rANS stream back into the raw bundle symbols.
pub fn decode_bundle_stream(
    stream: &[u8],
    bundles: &[BundleRecord],
    tables: &BundleAnsTables,
) -> Result<Vec<u8>, BundleDecodeError> {
    let max_width = tables.max_width();
    for (index, record) in bundles.iter().enumerate() {
        if record.bit_len == 0 || record.bit_len > max_width {
            return Err(BundleDecodeError::InvalidBitLength {
                index: saturating_usize_to_u32(index),
                bit_len: record.bit_len,
                max: max_width,
            });
        }
    }
    if bundles
        .iter()
        .any(|record| matches!(record.bundle_type, BundleType::SignificanceRle))
    {
        return Ok(bundles
            .iter()
            .map(|record| record.bits & ((1u8 << record.bit_len) - 1))
            .collect());
    }
    if stream.starts_with(&SIMD_BUNDLE_MAGIC) {
        return decode_bundle_stream_simd(stream, bundles, tables);
    }
    let mut decoder = BundleRansDecoder::new(stream, tables)?;
    let mut out = Vec::with_capacity(bundles.len());
    for record in bundles {
        out.push(decoder.decode_record(record)?);
    }
    Ok(out)
}
fn decode_bundle_stream_simd(
    stream: &[u8],
    bundles: &[BundleRecord],
    tables: &BundleAnsTables,
) -> Result<Vec<u8>, BundleDecodeError> {
    let header_len = SIMD_BUNDLE_MAGIC.len();
    if stream.len() < header_len + 16 {
        return Err(BundleDecodeError::InvalidSimdHeader);
    }
    let mut cursor = header_len;
    let mut lengths = [0usize; 4];
    for slot in lengths.iter_mut() {
        *slot = read_simd_bundle_lane_len(stream, &mut cursor)?;
    }
    let total_len = lengths
        .iter()
        .try_fold(0usize, |acc, len| acc.checked_add(*len))
        .ok_or(BundleDecodeError::LengthMismatch)?;
    let expected_end = cursor
        .checked_add(total_len)
        .ok_or(BundleDecodeError::LengthMismatch)?;
    if stream.len() != expected_end {
        return Err(BundleDecodeError::LengthMismatch);
    }
    let mut decoders: [Option<BundleRansDecoder<'_>>; 4] = [None, None, None, None];
    let mut lane_cursor = cursor;
    for (idx, len) in lengths.iter().enumerate() {
        let next = lane_cursor
            .checked_add(*len)
            .ok_or(BundleDecodeError::LengthMismatch)?;
        let lane_slice = stream
            .get(lane_cursor..next)
            .ok_or(BundleDecodeError::LengthMismatch)?;
        decoders[idx] = Some(BundleRansDecoder::new(lane_slice, tables)?);
        lane_cursor = next;
    }
    let mut out = Vec::with_capacity(bundles.len());
    for (idx, record) in bundles.iter().enumerate() {
        let lane = (bundles.len() - 1 - idx) & 3;
        let decoder = decoders[lane]
            .as_mut()
            .ok_or(BundleDecodeError::InvalidSimdHeader)?;
        out.push(decoder.decode_record(record)?);
    }
    Ok(out)
}
fn read_simd_bundle_lane_len(
    stream: &[u8],
    cursor: &mut usize,
) -> Result<usize, BundleDecodeError> {
    let end = (*cursor)
        .checked_add(4)
        .ok_or(BundleDecodeError::InvalidSimdHeader)?;
    let slice = stream
        .get(*cursor..end)
        .ok_or(BundleDecodeError::InvalidSimdHeader)?;
    let mut buf = [0u8; 4];
    buf.copy_from_slice(slice);
    *cursor = end;
    Ok(u32::from_le_bytes(buf) as usize)
}
fn bundle_bits(value: i16, width: u8) -> (BundleType, u8, u8) {
    let nonzero = value != 0;
    let mut bits = (nonzero as u8) & 0x1;
    let mut len = 1u8;
    if !nonzero || width <= 1 {
        return (BundleType::SignificanceOnly, bits, len);
    }
    let negative = value < 0;
    bits = (bits << 1) | (negative as u8);
    len += 1;
    if width <= 2 {
        return (BundleType::SignAndMagnitude, bits, len);
    }
    let parity = (value.saturating_abs() as u16 & 1) as u8;
    bits = (bits << 1) | parity;
    len += 1;
    if width <= 3 {
        return (BundleType::SignParity, bits, len);
    }
    let geq2 = (value.saturating_abs() >= 2) as u8;
    bits = (bits << 1) | geq2;
    len += 1;
    (BundleType::SignParityLevel, bits, len)
}
fn bundle_zero_run_symbol(run: u16, width: u8) -> (u8, u8, u16) {
    let bit_len = width.max(2).min(MAX_BUNDLE_WIDTH as u8);
    let max_symbol = (1u16 << bit_len) - 1;
    let consumed = run.min(max_symbol);
    let bits = u8::try_from(consumed).unwrap_or(u8::MAX);
    (bits, bit_len, consumed)
}
fn quantizer_to_bucket(qp: u8) -> u8 {
    match qp {
        0..=12 => 0,
        13..=24 => 1,
        25..=36 => 2,
        _ => 3,
    }
}
const fn position_class(order: usize) -> u8 {
    match order {
        0 => 0,
        1..=4 => 1,
        5..=15 => 2,
        16..=31 => 3,
        _ => 4,
    }
}
const fn subband_class(slot: usize) -> u8 {
    let row = slot / BLOCK_SIZE;
    let col = slot % BLOCK_SIZE;
    let low_cut = 3;
    match (row <= low_cut, col <= low_cut) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    }
}
// Only the low 16 bits of FNV reach the wire. Multiplication and XOR cannot
// propagate discarded high bits downward, so each invariant prefix fits in u16.
const CONTEXT_POSITION: [usize; BLOCK_PIXELS] = {
    let mut offsets = [0; BLOCK_PIXELS];
    let mut order = 0;
    while order < BLOCK_PIXELS {
        let pos = position_class(order) as usize;
        let band = subband_class(ZIG_ZAG[order]) as usize;
        offsets[order] = (pos * 4 + band) * 16;
        order += 1;
    }
    offsets
};
// Prefix positions depend solely on support and DC significance. The first AC
// has no preceding AC level; later ternary levels have magnitude zero or one.
const SPARSE_CONTEXT_OFFSETS: [[u16; ZERO_TAIL_ORDER - 1]; 64] = {
    let mut offsets = [[0; ZERO_TAIL_ORDER - 1]; 64];
    let mut mask = 0;
    while mask < 64 {
        let mut occupied = [0; BLOCK_PIXELS];
        occupied[0] = mask >> 5;
        let mut i = 0;
        while i < 5 {
            occupied[ZIG_ZAG[i + 1]] = (mask >> i) & 1;
            i += 1;
        }
        let mut order = 1;
        let mut previous = 0;
        while order < ZERO_TAIL_ORDER {
            let slot = ZIG_ZAG[order];
            let (x, y) = (slot % BLOCK_SIZE, slot / BLOCK_SIZE);
            let mut neighbors = 0;
            if x > 0 {
                neighbors += occupied[slot - 1];
            }
            if y > 0 {
                neighbors += occupied[slot - BLOCK_SIZE];
                if x > 0 {
                    neighbors += occupied[slot - BLOCK_SIZE - 1];
                }
                if x + 1 < BLOCK_SIZE {
                    neighbors += occupied[slot - BLOCK_SIZE + 1];
                }
            }
            offsets[mask][order - 1] = (CONTEXT_POSITION[order]
                + 4 * (if neighbors > 3 { 3 } else { neighbors })
                + previous) as u16;
            previous = occupied[slot];
            order += 1;
        }
        mask += 1;
    }
    offsets
};

const fn finish_context(prefix: u16, tile: u8, nonzero: bool, order: usize) -> BundleContextId {
    let hash = hash_mix(prefix as u32, tile as u32);
    let hash = hash_mix(hash, nonzero as u32);
    BundleContextId::new(hash_mix(hash, (order & 63) as u32) as u16)
}
// A neutral coefficient has no occupied neighbor or predecessor. Check
// every tile and state when building the table: each zero in a neutral
// suffix emits one singleton and its predecessor has a different context.
static NEUTRAL_IDS: [[[BundleContextId; BLOCK_PIXELS]; 64]; 8] = {
    let mut out = [[[BundleContextId(0); BLOCK_PIXELS]; 64]; 8];
    let mut state = 0;
    while state < 8 {
        let mut tile = 0;
        while tile < 64 {
            let mut z = 1;
            while z < BLOCK_PIXELS {
                let ctx = finish_context(
                    CONTEXT_PREFIX[state][0][CONTEXT_POSITION[z]],
                    tile as u8,
                    false,
                    z,
                );
                if z >= FIRST_AC_TAIL_ORDER {
                    assert!(out[state][tile][z - 1].0 != ctx.0);
                }
                out[state][tile][z] = ctx;
                z += 1;
            }
            tile += 1;
        }
        state += 1;
    }
    out
};

// Within a Q16 tile the hash through significance depends only on the
// context geometry and whether the coefficient is occupied at bundle width two.
static Q16_TILE_HASH: [[[[u16; 320]; 2]; 64]; 2] = {
    let mut table = [[[[0; 320]; 2]; 64]; 2];
    let mut f = 0;
    while f < 2 {
        let mut t = 0;
        while t < 64 {
            let mut occupied = 0;
            while occupied < 2 {
                let mut k = 0;
                while k < 320 {
                    let prefix = CONTEXT_PREFIX[2 + f][occupied][k];
                    table[f][t][occupied][k] = hash_mix(
                        hash_mix(hash_mix(prefix as u32, t as u32), occupied as u32),
                        0,
                    ) as u16;
                    k += 1;
                }
                occupied += 1;
            }
            t += 1;
        }
        f += 1;
    }
    table
};

static CONTEXT_PREFIX: [[[u16; 5 * 4 * 16]; 5]; 8] = {
    let mut table = [[[0; 5 * 4 * 16]; 5]; 8];
    let mut state = 0;
    while state < table.len() {
        let mut kind = 0;
        while kind < table[state].len() {
            let mut index = 0;
            while index < table[state][kind].len() {
                let band = (index / 16) % 4;
                let pos = index / 64;
                let neighbors = (index / 4) % 4;
                let previous = index % 4;
                let mut hash = hash_mix(0x811C9DC5, kind as u32);
                hash = hash_mix(hash, band as u32);
                hash = hash_mix(hash, pos as u32);
                hash = hash_mix(hash, neighbors as u32);
                hash = hash_mix(hash, previous as u32);
                hash = hash_mix(hash, (state / 2) as u32);
                table[state][kind][index] = hash_mix(hash, (state % 2) as u32) as u16;
                index += 1;
            }
            kind += 1;
        }
        state += 1;
    }
    table
};
/// Eight independent byte lanes sum the left neighbor and three neighbors
/// from the row above. A lane holds at most four, so addition cannot carry.
fn neighbor_counts(coeffs: &[i16; BLOCK_PIXELS]) -> [u8; BLOCK_PIXELS] {
    let mut counts = [0; BLOCK_PIXELS];
    let mut above = 0;
    for (coeff_row, out) in coeffs
        .as_chunks::<BLOCK_SIZE>()
        .0
        .iter()
        .zip(counts.as_chunks_mut::<BLOCK_SIZE>().0)
    {
        let row = u64::from_le_bytes(coeff_row.map(|v| u8::from(v != 0)));
        *out = ((row << 8) + (above << 8) + above + (above >> 8)).to_le_bytes();
        above = row;
    }
    counts
}
fn level_bucket(value: u8) -> u8 {
    match value {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 3,
    }
}
const fn hash_mix(acc: u32, value: u32) -> u32 {
    acc.wrapping_mul(16777619) ^ value
}
/// Seed short runs from a constant slab and double longer runs from the output.
fn append_repeated<T: Copy, const N: usize>(
    out: &mut Vec<T>,
    periods: &[[T; N]; 32],
    count: usize,
) {
    let start = out.len();
    let total = count * N;
    out.reserve(total);
    let seed = count.min(periods.len());
    out.extend_from_slice(periods[..seed].as_flattened());
    let mut written = seed * N;
    while written < total {
        let next = written.min(total - written);
        out.extend_from_within(start..start + next);
        written += next;
    }
}

/// Emit consecutive predicted blocks whose quantized residuals are identically zero.
fn emit_zero_run(
    blocks: std::ops::Range<usize>,
    prev_dc: &mut i16,
    payload: &mut Vec<u8>,
    hooks: &mut impl BundledHooks,
) {
    let first_diff = prev_dc.wrapping_neg();
    let count = blocks.len();
    hooks.record_zero_run(blocks, first_diff);
    let [lo, hi] = first_diff.to_le_bytes();
    payload.extend_from_slice(&[lo, hi, RLE_EOB, 0, 0]);
    append_repeated(payload, &[[0, 0, RLE_EOB, 0, 0]; 32], count - 1);
    *prev_dc = 0;
}

fn emit_dc_block(
    levels: &[i16; BLOCK_PIXELS],
    prev_dc: &mut i16,
    payload: &mut Vec<u8>,
    hooks: &mut impl BundledHooks,
    block_index: usize,
    frame_type: FrameType,
) {
    let diff = levels[0].wrapping_sub(*prev_dc);
    hooks.record_dc_only(levels, diff, block_index, frame_type);
    let [lo, hi] = diff.to_le_bytes();
    payload.extend_from_slice(&[lo, hi, RLE_EOB, 0, 0]);
    *prev_dc = levels[0];
}

#[cfg(test)]
pub(crate) fn encode_block_rle(
    coeffs: &[i16; BLOCK_PIXELS],
    prev_dc: &mut i16,
    out: &mut Vec<u8>,
    hooks: &mut impl BundledHooks,
    block_index: usize,
    frame_type: FrameType,
) {
    encode_active_block(
        coeffs,
        ac_support(coeffs),
        prev_dc,
        out,
        hooks,
        block_index,
        frame_type,
    )
}

fn encode_active_block(
    coeffs: &[i16; BLOCK_PIXELS],
    mut active: u64,
    prev_dc: &mut i16,
    out: &mut Vec<u8>,
    hooks: &mut impl BundledHooks,
    block_index: usize,
    frame_type: FrameType,
) {
    hooks.record_block_coeffs(coeffs, active, block_index, frame_type);
    let dc = coeffs[0];
    let diff = dc.wrapping_sub(*prev_dc);
    out.extend_from_slice(&diff.to_le_bytes());
    hooks.record_dc(diff);
    *prev_dc = dc;
    let mut first = 1;
    while active != 0 {
        let order = active.trailing_zeros() as usize;
        let zeros = (order - first) as u8;
        let level = coeffs[ZIG_ZAG[order]];
        let [lo, hi] = level.to_le_bytes();
        out.extend_from_slice(&[zeros, lo, hi]);
        hooks.record_ac(zeros, level);
        active &= active - 1;
        first = order + 1;
    }
    out.extend_from_slice(&[RLE_EOB, 0, 0]);
    hooks.record_eob();
}
fn take_block_i16_le(
    bytes: &[u8],
    offset: &mut usize,
    block_index: u32,
) -> Result<i16, CodecError> {
    let end = (*offset)
        .checked_add(2)
        .ok_or(CodecError::TruncatedBlock(block_index))?;
    let slice = bytes
        .get(*offset..end)
        .ok_or(CodecError::TruncatedBlock(block_index))?;
    let mut raw = [0u8; 2];
    raw.copy_from_slice(slice);
    *offset = end;
    Ok(i16::from_le_bytes(raw))
}
fn take_rle_record(
    bytes: &[u8],
    offset: &mut usize,
    block_index: u32,
) -> Result<(u8, i16), CodecError> {
    let end = (*offset)
        .checked_add(3)
        .ok_or(CodecError::TruncatedBlock(block_index))?;
    let record = bytes
        .get(*offset..end)
        .ok_or(CodecError::TruncatedBlock(block_index))?;
    let mut raw = [0u8; 2];
    raw.copy_from_slice(&record[1..3]);
    *offset = end;
    Ok((record[0], i16::from_le_bytes(raw)))
}
pub(crate) fn decode_block_rle(
    bytes: &[u8],
    offset: &mut usize,
    prev_dc: &mut i16,
    block_index: u32,
) -> Result<[i16; BLOCK_PIXELS], CodecError> {
    let mut coeffs = [0i16; BLOCK_PIXELS];
    let dc_diff = take_block_i16_le(bytes, offset, block_index)?;
    let dc = prev_dc.wrapping_add(dc_diff);
    coeffs[0] = dc;
    *prev_dc = dc;
    let mut pos = 1usize;
    let mut finished = false;
    while pos < BLOCK_PIXELS {
        let (run, value) = take_rle_record(bytes, offset, block_index)?;
        if run == RLE_EOB {
            finished = true;
            break;
        }
        let advance = run as usize;
        if pos + advance >= BLOCK_PIXELS {
            return Err(CodecError::RleOverflow(block_index));
        }
        pos += advance;
        coeffs[ZIG_ZAG[pos]] = value;
        pos += 1;
    }
    if !finished {
        if pos < BLOCK_PIXELS {
            return Err(CodecError::TruncatedBlock(block_index));
        }
        if *offset + 3 <= bytes.len() && bytes[*offset] == RLE_EOB {
            *offset += 3;
            return Ok(coeffs);
        }
        return Err(CodecError::MissingEndOfBlock(block_index));
    }
    Ok(coeffs)
}
#[inline]
fn clamp_pixel(value: i32) -> u8 {
    value.clamp(0, 255) as u8
}
#[cfg(feature = "streaming-neural-filter")]
pub(super) fn apply_neural_filter(frame: &mut [u8], dims: FrameDimensions) {
    let width = usize::from(dims.width);
    let height = usize::from(dims.height);
    if width == 0 || height == 0 {
        return;
    }
    let len = width
        .checked_mul(height)
        .unwrap_or_default()
        .min(frame.len());
    if len == 0 || frame.len() < len {
        return;
    }
    const KERNEL: [i8; 9] = [1, 2, 1, 2, 4, 2, 1, 2, 1];
    const SHIFT: i32 = 4;
    const BIAS: i32 = 8;
    let mut out = vec![0u8; len];
    for y in 0..height {
        for x in 0..width {
            let mut acc = 0i32;
            for ky in 0..3 {
                for kx in 0..3 {
                    let nx = x.saturating_add(kx).saturating_sub(1).min(width - 1);
                    let ny = y.saturating_add(ky).saturating_sub(1).min(height - 1);
                    let pixel = frame[ny * width + nx] as i32;
                    let weight = KERNEL[ky * 3 + kx] as i32;
                    acc += pixel * weight;
                }
            }
            let value = ((acc + BIAS) >> SHIFT).clamp(0, 255) as u8;
            out[y * width + x] = value;
        }
    }
    frame[..len].copy_from_slice(&out);
}
#[cfg_attr(not(test), allow(dead_code))]
fn layout_channel_count(layout: AudioLayout) -> usize {
    match layout {
        AudioLayout::Mono => 1,
        AudioLayout::Stereo => 2,
        AudioLayout::FirstOrderAmbisonics => 4,
    }
}
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioEncoderConfig {
    pub sample_rate: u32,
    pub frame_samples: u16,
    pub layout: AudioLayout,
    pub fec_level: u8,
    pub target_bitrate: Option<u32>,
    pub backend: iroha_audio::BackendPreference,
}
impl Default for AudioEncoderConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            frame_samples: 240,
            layout: AudioLayout::Stereo,
            fec_level: 0,
            target_bitrate: None,
            backend: iroha_audio::BackendPreference::Auto,
        }
    }
}
impl AudioEncoderConfig {
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.layout.channel_count()
    }
}
pub struct AudioEncoder {
    config: AudioEncoderConfig,
    backend: iroha_audio::Encoder,
}
impl AudioEncoder {
    pub fn new(config: AudioEncoderConfig) -> Result<Self, AudioCodecError> {
        let backend = iroha_audio::Encoder::new(iroha_audio::EncoderConfig {
            sample_rate: config.sample_rate,
            frame_samples: config.frame_samples,
            layout: config.layout.into(),
            fec_level: config.fec_level,
            target_bitrate: config.target_bitrate,
            backend: config.backend,
        })
        .map_err(AudioCodecError::from_backend)?;
        Ok(Self { config, backend })
    }
    pub fn encode_frame(
        &mut self,
        sequence: u64,
        timestamp_ns: u64,
        pcm: &[i16],
    ) -> Result<AudioFrame, AudioCodecError> {
        let payload = self
            .backend
            .encode(pcm)
            .map_err(AudioCodecError::from_backend)?;
        Ok(AudioFrame {
            sequence,
            timestamp_ns,
            fec_level: self.config.fec_level,
            channel_layout: self.config.layout,
            payload,
        })
    }
}
pub struct AudioDecoder {
    config: AudioEncoderConfig,
    backend: iroha_audio::Decoder,
}
impl AudioDecoder {
    pub fn new(config: AudioEncoderConfig) -> Result<Self, AudioCodecError> {
        let backend = iroha_audio::Decoder::new(iroha_audio::EncoderConfig {
            sample_rate: config.sample_rate,
            frame_samples: config.frame_samples,
            layout: config.layout.into(),
            fec_level: config.fec_level,
            target_bitrate: None,
            backend: iroha_audio::BackendPreference::Auto,
        })
        .map_err(AudioCodecError::from_backend)?;
        Ok(Self { config, backend })
    }
    pub fn decode_frame(&mut self, frame: &AudioFrame) -> Result<Vec<i16>, AudioCodecError> {
        if frame.channel_layout != self.config.layout {
            return Err(AudioCodecError::LayoutMismatch(
                AudioCodecLayoutMismatchInfo {
                    expected: self.config.layout,
                    found: frame.channel_layout,
                },
            ));
        }
        self.backend
            .decode(&frame.payload)
            .map_err(AudioCodecError::from_backend)
    }
}
fn checked_chunk_count(chunk_count: usize) -> Result<u16, SegmentError> {
    u16::try_from(chunk_count).map_err(|_| {
        SegmentError::ChunkCountOverflow(ChunkCountOverflowInfo {
            found: saturating_usize_to_u32(chunk_count),
        })
    })
}
pub fn verify_segment(
    header: &SegmentHeader,
    descriptors: &[ChunkDescriptor],
    chunks: &[Vec<u8>],
    audio: Option<&SegmentAudio>,
) -> Result<(), SegmentError> {
    if descriptors.len() != chunks.len() {
        return Err(SegmentError::CountMismatch(ChunkListCountMismatch {
            descriptors: saturating_usize_to_u32(descriptors.len()),
            chunks: saturating_usize_to_u32(chunks.len()),
        }));
    }
    let descriptor_count = checked_chunk_count(descriptors.len())?;
    if header.chunk_count != descriptor_count {
        return Err(SegmentError::HeaderCountMismatch(
            HeaderDescriptorCountMismatch {
                header: header.chunk_count,
                actual: saturating_usize_to_u32(descriptors.len()),
            },
        ));
    }
    let mut payload_refs = Vec::with_capacity(descriptors.len());
    let mut expected_offset = 0u32;
    let mut last_chunk_id: Option<u16> = None;
    for (idx, (descriptor, chunk)) in descriptors.iter().zip(chunks.iter()).enumerate() {
        if let Some(last) = last_chunk_id
            && descriptor.chunk_id <= last
        {
            return Err(SegmentError::UnsortedChunkIds);
        }
        last_chunk_id = Some(descriptor.chunk_id);
        let chunk_len = u32::try_from(chunk.len())
            .map_err(|_| SegmentError::ChunkLengthOverflow(saturating_usize_to_u32(idx)))?;
        if descriptor.offset != expected_offset {
            return Err(SegmentError::DescriptorOffsetMismatch(
                DescriptorOffsetDetails {
                    index: saturating_usize_to_u32(idx),
                    expected: expected_offset,
                    actual: descriptor.offset,
                },
            ));
        }
        if descriptor.length != chunk_len {
            return Err(SegmentError::DescriptorLengthMismatch(
                DescriptorLengthDetails {
                    index: saturating_usize_to_u32(idx),
                    descriptor: descriptor.length,
                    chunk: saturating_usize_to_u32(chunk.len()),
                },
            ));
        }
        expected_offset = expected_offset
            .checked_add(chunk_len)
            .ok_or(SegmentError::OffsetOverflow(saturating_usize_to_u32(idx)))?;
        payload_refs.push((descriptor.chunk_id, chunk.as_slice()));
    }
    let commitments = chunk_commitments(header.segment_number, &payload_refs);
    for (idx, (descriptor, commitment)) in descriptors.iter().zip(commitments.iter()).enumerate() {
        if &descriptor.commitment != commitment {
            return Err(SegmentError::CommitmentMismatch(saturating_usize_to_u32(
                idx,
            )));
        }
    }
    let root = merkle_root(&commitments)?;
    if header.chunk_merkle_root != root {
        return Err(SegmentError::MerkleMismatch);
    }
    match (header.audio_summary.as_ref(), audio) {
        (Some(summary), Some(track)) => {
            if track.summary.sample_rate != summary.sample_rate {
                return Err(SegmentError::AudioSampleRateMismatch(
                    AudioSampleRateMismatchInfo {
                        expected: summary.sample_rate,
                        found: track.summary.sample_rate,
                    },
                ));
            }
            if track.summary.frame_samples != summary.frame_samples {
                return Err(SegmentError::AudioFrameSamplesMismatch(
                    AudioFrameSamplesMismatchInfo {
                        expected: summary.frame_samples,
                        found: track.summary.frame_samples,
                    },
                ));
            }
            if track.summary.frame_duration_ns != summary.frame_duration_ns {
                return Err(SegmentError::AudioFrameDurationMismatch(
                    AudioFrameDurationMismatchInfo {
                        expected: summary.frame_duration_ns,
                        found: track.summary.frame_duration_ns,
                    },
                ));
            }
            if track.summary.fec_level != summary.fec_level {
                return Err(SegmentError::AudioFecMismatch(AudioFecMismatchInfo {
                    expected: summary.fec_level,
                    found: track.summary.fec_level,
                }));
            }
            if track.summary.layout != summary.layout {
                return Err(SegmentError::AudioLayoutMismatch(AudioLayoutMismatchInfo {
                    expected: summary.layout,
                    found: track.summary.layout,
                }));
            }
            if track.summary.frames_per_segment != summary.frames_per_segment {
                return Err(SegmentError::AudioFrameCountMismatch(
                    AudioFrameCountMismatchInfo {
                        expected: summary.frames_per_segment,
                        found: track.summary.frames_per_segment,
                    },
                ));
            }
            let expected_frames = usize::from(summary.frames_per_segment);
            if track.frames.len() != expected_frames {
                return Err(SegmentError::AudioFrameCountMismatch(
                    AudioFrameCountMismatchInfo {
                        expected: summary.frames_per_segment,
                        found: track.frames.len().try_into().unwrap_or(u16::MAX),
                    },
                ));
            }
            let frame_step = u64::from(summary.frame_duration_ns);
            for (idx, frame) in track.frames.iter().enumerate() {
                if frame.channel_layout != summary.layout {
                    return Err(SegmentError::AudioLayoutMismatch(AudioLayoutMismatchInfo {
                        expected: summary.layout,
                        found: frame.channel_layout,
                    }));
                }
                if frame.fec_level != summary.fec_level {
                    return Err(SegmentError::AudioFecMismatch(AudioFecMismatchInfo {
                        expected: summary.fec_level,
                        found: frame.fec_level,
                    }));
                }
                let idx_u32 = saturating_usize_to_u32(idx);
                let offset = frame_step
                    .checked_mul(idx as u64)
                    .ok_or(SegmentError::AudioTimestampOverflow(idx_u32))?;
                let expected_ts = header
                    .timeline_start_ns
                    .checked_add(offset)
                    .ok_or(SegmentError::AudioTimestampOverflow(idx_u32))?;
                let delta = frame.timestamp_ns.abs_diff(expected_ts);
                if delta > AUDIO_SYNC_TOLERANCE_NS {
                    return Err(SegmentError::AudioTimestampMismatch(
                        AudioTimestampMismatchInfo {
                            index: idx_u32,
                            expected: expected_ts,
                            found: frame.timestamp_ns,
                        },
                    ));
                }
            }
        }
        (Some(_), None) => return Err(SegmentError::AudioSummaryMissing),
        (None, Some(_)) => return Err(SegmentError::AudioSummaryUnexpected),
        (None, None) => {}
    }
    Ok(())
}
#[derive(Clone, Copy)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
struct OffsetMeta {
    offset: u32,
    length: u32,
}
fn compute_offsets(chunks: &[Vec<u8>]) -> Vec<OffsetMeta> {
    let mut offset = 0u32;
    chunks
        .iter()
        .map(|chunk| {
            let len = chunk.len() as u32;
            let meta = OffsetMeta {
                offset,
                length: len,
            };
            offset = offset.saturating_add(len);
            meta
        })
        .collect()
}
#[cfg(test)]
mod tests;
