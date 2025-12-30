#![doc = "Deterministic audio helpers used by the Norito streaming pipeline."]

use std::sync::Arc;

#[cfg(feature = "libopus")]
use opus_backend as opus;

/// Maximum number of channels supported by the deterministic encoder/decoder.
pub const MAX_CHANNELS: u8 = 4;

/// Audio layout (mono, stereo, or first-order ambisonics).
///
/// The helper keeps channel handling generic so higher layers can map these
/// layouts to their protocol-specific enums.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum ChannelLayout {
    Mono,
    Stereo,
    FirstOrderAmbisonics,
}

impl ChannelLayout {
    /// Returns the number of channels associated with the layout.
    #[must_use]
    pub const fn channel_count(self) -> usize {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
            Self::FirstOrderAmbisonics => 4,
        }
    }
}

/// Preferred backend when constructing encoders/decoders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum BackendPreference {
    /// Select the best supported backend automatically. Falls back to ADPCM if
    /// the libopus backend is unavailable or does not support the requested layout.
    #[default]
    Auto,
    /// Force the deterministic ADPCM backend regardless of libopus availability.
    Adpcm,
    /// Use the native Norito audio codec.
    Native,
    /// Require the libopus backend; construction fails if libopus is unavailable.
    Libopus,
}

/// Encoder configuration shared across the deterministic helpers.
#[derive(Clone, Copy, Debug)]
pub struct EncoderConfig {
    pub sample_rate: u32,
    pub frame_samples: u16,
    pub layout: ChannelLayout,
    /// In-band FEC aggressiveness requested by the caller (`0 = disabled`).
    pub fec_level: u8,
    /// Target bitrate in bits per second. `None` applies the backend default.
    pub target_bitrate: Option<u32>,
    /// Backend preference for this encoder/decoder.
    pub backend: BackendPreference,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            frame_samples: 240,
            layout: ChannelLayout::Stereo,
            fec_level: 0,
            target_bitrate: None,
            backend: BackendPreference::Auto,
        }
    }
}

/// Errors emitted by the deterministic audio helpers.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum CodecError {
    #[error("expected {expected} PCM samples, got {found}")]
    InvalidPcmLength { expected: usize, found: usize },
    #[error("unsupported audio payload version {0}")]
    UnsupportedVersion(u8),
    #[error("audio packet too short")]
    PacketTooShort,
    #[error("invalid channel count: expected {expected:?}, found {found}")]
    InvalidChannelCount { expected: Option<u8>, found: u8 },
    #[error("invalid frame sample count: expected {expected}, found {found}")]
    InvalidSampleCount { expected: u16, found: u16 },
    #[error("audio backend unavailable for layout {layout:?}")]
    BackendUnavailable { layout: ChannelLayout },
    #[error("audio backend does not support layout {layout:?}")]
    UnsupportedLayout { layout: ChannelLayout },
    #[error("libopus backend error: {message}")]
    LibopusError { message: Arc<str> },
}

impl CodecError {
    fn invalid_pcm_length(expected: usize, found: usize) -> Self {
        Self::InvalidPcmLength { expected, found }
    }
}

enum EncoderBackend {
    Adpcm(adpcm::Encoder),
    Native(native::Encoder),
    #[cfg(feature = "libopus")]
    Libopus(opus::Encoder),
}

enum DecoderBackend {
    Adpcm(adpcm::Decoder),
    Native(native::Decoder),
    #[cfg(feature = "libopus")]
    Libopus(opus::Decoder),
}

/// Deterministic audio encoder facade.
pub struct Encoder {
    config: EncoderConfig,
    backend: EncoderBackend,
}

impl Encoder {
    /// Construct a new encoder using the preferred backend.
    pub fn new(config: EncoderConfig) -> Result<Self, CodecError> {
        let backend = select_encoder_backend(&config)?;
        Ok(Self { config, backend })
    }

    /// Returns a reference to the configuration used by this encoder.
    #[must_use]
    pub fn config(&self) -> &EncoderConfig {
        &self.config
    }

    /// Encode raw PCM samples into the selected backend payload.
    ///
    /// The caller is responsible for attaching sequence numbers, timestamps,
    /// and higher-level metadata.
    pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>, CodecError> {
        match &mut self.backend {
            EncoderBackend::Adpcm(inner) => inner.encode(pcm),
            EncoderBackend::Native(inner) => inner.encode(pcm),
            #[cfg(feature = "libopus")]
            EncoderBackend::Libopus(inner) => inner.encode(pcm),
        }
    }
}

/// Deterministic audio decoder facade.
pub struct Decoder {
    backend: DecoderBackend,
}

impl Decoder {
    /// Construct a new decoder mirroring the encoder configuration.
    pub fn new(config: EncoderConfig) -> Result<Self, CodecError> {
        let backend = select_decoder_backend(&config)?;
        Ok(Self { backend })
    }

    /// Decode an encoded payload back into signed 16-bit PCM samples.
    pub fn decode(&mut self, payload: &[u8]) -> Result<Vec<i16>, CodecError> {
        match &mut self.backend {
            DecoderBackend::Adpcm(inner) => inner.decode(payload),
            DecoderBackend::Native(inner) => inner.decode(payload),
            #[cfg(feature = "libopus")]
            DecoderBackend::Libopus(inner) => inner.decode(payload),
        }
    }
}

fn select_encoder_backend(config: &EncoderConfig) -> Result<EncoderBackend, CodecError> {
    match config.backend {
        BackendPreference::Adpcm => adpcm::Encoder::new(config).map(EncoderBackend::Adpcm),
        BackendPreference::Native => native::Encoder::new(config).map(EncoderBackend::Native),
        BackendPreference::Libopus => {
            #[cfg(feature = "libopus")]
            {
                opus::Encoder::new(config).map(EncoderBackend::Libopus)
            }
            #[cfg(not(feature = "libopus"))]
            {
                Err(CodecError::BackendUnavailable {
                    layout: config.layout,
                })
            }
        }
        BackendPreference::Auto => {
            if let Ok(encoder) = native::Encoder::new(config) {
                return Ok(EncoderBackend::Native(encoder));
            }
            #[cfg(feature = "libopus")]
            {
                match opus::Encoder::new(config) {
                    Ok(encoder) => Ok(EncoderBackend::Libopus(encoder)),
                    Err(CodecError::UnsupportedLayout { .. }) => {
                        adpcm::Encoder::new(config).map(EncoderBackend::Adpcm)
                    }
                    Err(CodecError::BackendUnavailable { .. }) => {
                        adpcm::Encoder::new(config).map(EncoderBackend::Adpcm)
                    }
                    Err(err) => Err(err),
                }
            }
            #[cfg(not(feature = "libopus"))]
            {
                adpcm::Encoder::new(config).map(EncoderBackend::Adpcm)
            }
        }
    }
}

fn select_decoder_backend(config: &EncoderConfig) -> Result<DecoderBackend, CodecError> {
    match config.backend {
        BackendPreference::Adpcm => adpcm::Decoder::new(config).map(DecoderBackend::Adpcm),
        BackendPreference::Native => native::Decoder::new(config).map(DecoderBackend::Native),
        BackendPreference::Libopus => {
            #[cfg(feature = "libopus")]
            {
                opus::Decoder::new(config).map(DecoderBackend::Libopus)
            }
            #[cfg(not(feature = "libopus"))]
            {
                Err(CodecError::BackendUnavailable {
                    layout: config.layout,
                })
            }
        }
        BackendPreference::Auto => {
            if let Ok(decoder) = native::Decoder::new(config) {
                return Ok(DecoderBackend::Native(decoder));
            }
            #[cfg(feature = "libopus")]
            {
                match opus::Decoder::new(config) {
                    Ok(decoder) => Ok(DecoderBackend::Libopus(decoder)),
                    Err(CodecError::UnsupportedLayout { .. }) => {
                        adpcm::Decoder::new(config).map(DecoderBackend::Adpcm)
                    }
                    Err(CodecError::BackendUnavailable { .. }) => {
                        adpcm::Decoder::new(config).map(DecoderBackend::Adpcm)
                    }
                    Err(err) => Err(err),
                }
            }
            #[cfg(not(feature = "libopus"))]
            {
                adpcm::Decoder::new(config).map(DecoderBackend::Adpcm)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_pcm(frame_samples: usize, channels: usize) -> Vec<i16> {
        let mut pcm = Vec::with_capacity(channels * frame_samples);
        for i in 0..frame_samples {
            let angle = (i as f32) * std::f32::consts::PI * 2.0 / 32.0;
            let sample = (angle.sin() * 12_000.0) as i16;
            for ch in 0..channels {
                if ch % 2 == 0 {
                    pcm.push(sample);
                } else {
                    pcm.push(sample.wrapping_neg());
                }
            }
        }
        pcm
    }

    #[test]
    fn adpcm_roundtrip_stereo() {
        let config = EncoderConfig {
            frame_samples: 64,
            layout: ChannelLayout::Stereo,
            backend: BackendPreference::Adpcm,
            ..EncoderConfig::default()
        };
        let mut encoder = Encoder::new(config).expect("encoder");
        let mut decoder = Decoder::new(config).expect("decoder");
        let pcm = sine_pcm(64, 2);
        let encoded = encoder.encode(&pcm).expect("encode");
        let decoded = decoder.decode(&encoded).expect("decode");
        assert_eq!(decoded.len(), pcm.len());
        let max_err = decoded
            .iter()
            .zip(pcm.iter())
            .map(|(a, b)| (i32::from(*a) - i32::from(*b)).abs())
            .max()
            .unwrap();
        assert!(max_err <= 11_000);
    }

    #[test]
    fn adpcm_encoder_rejects_invalid_length() {
        let config = EncoderConfig {
            frame_samples: 32,
            layout: ChannelLayout::Stereo,
            backend: BackendPreference::Adpcm,
            ..EncoderConfig::default()
        };
        let mut encoder = Encoder::new(config).expect("encoder");
        let err = encoder.encode(&[0i16; 10]).expect_err("length mismatch");
        assert!(matches!(err, CodecError::InvalidPcmLength { .. }));
    }

    #[test]
    fn adpcm_decoder_rejects_bad_header() {
        let config = EncoderConfig {
            backend: BackendPreference::Adpcm,
            ..EncoderConfig::default()
        };
        let mut decoder = Decoder::new(config).expect("decoder");
        let err = decoder.decode(&[1, 0, 0, 0]).expect_err("version mismatch");
        assert!(matches!(err, CodecError::UnsupportedVersion(1)));
    }

    #[test]
    fn adpcm_golden_payload_is_stable() {
        let config = EncoderConfig {
            frame_samples: 48,
            backend: BackendPreference::Adpcm,
            ..EncoderConfig::default()
        };
        let mut encoder = Encoder::new(config).expect("encoder");
        let pcm = sine_pcm(config.frame_samples as usize, config.layout.channel_count());
        let encoded = encoder.encode(&pcm).expect("encode");
        let golden: &[u8] = &[
            0, 2, 48, 0, 0, 0, 0, 0, 0, 0, 0, 247, 247, 247, 247, 247, 247, 247, 247, 247, 8, 8,
            25, 8, 42, 25, 42, 42, 42, 42, 59, 25, 42, 25, 8, 128, 162, 162, 213, 179, 196, 196,
            179, 196, 179, 179, 162, 179, 162, 145, 128, 25, 25, 59, 110, 59, 93, 59,
        ];
        assert_eq!(encoded, golden);
    }

    #[test]
    fn native_roundtrip_stereo() {
        let config = EncoderConfig {
            frame_samples: 96,
            layout: ChannelLayout::Stereo,
            backend: BackendPreference::Native,
            ..EncoderConfig::default()
        };
        let mut encoder = Encoder::new(config).expect("native encoder");
        let mut decoder = Decoder::new(config).expect("native decoder");
        let pcm = sine_pcm(96, 2);
        let encoded = encoder.encode(&pcm).expect("encode");
        assert!(!encoded.is_empty(), "native codec must produce payload");
        let decoded = decoder.decode(&encoded).expect("decode");
        assert_eq!(decoded.len(), pcm.len());
        let max_err = decoded
            .iter()
            .zip(pcm.iter())
            .map(|(a, b)| (i32::from(*a) - i32::from(*b)).abs())
            .max()
            .unwrap();
        assert!(
            max_err <= 6_000,
            "max reconstruction error too high: {max_err}"
        );
    }

    #[test]
    fn native_decoder_rejects_bad_version() {
        let config = EncoderConfig {
            backend: BackendPreference::Native,
            ..EncoderConfig::default()
        };
        let mut decoder = Decoder::new(config).expect("decoder");
        let payload = [1, 2, 0, 0, 0];
        let err = decoder.decode(&payload).expect_err("version mismatch");
        assert!(matches!(err, CodecError::UnsupportedVersion(1)));
    }

    #[test]
    fn native_golden_payload_is_stable() {
        let config = EncoderConfig {
            frame_samples: 48,
            backend: BackendPreference::Native,
            ..EncoderConfig::default()
        };
        let mut encoder = Encoder::new(config).expect("encoder");
        let pcm = sine_pcm(config.frame_samples as usize, config.layout.channel_count());
        let encoded = encoder.encode(&pcm).expect("encode");
        let golden: &[u8] = &[
            0, 2, 48, 0, 0, 0, 0, 79, 1, 0, 0, 79, 1, 151, 151, 166, 181, 181, 211, 226, 241, 31,
            46, 61, 91, 91, 106, 121, 121, 121, 121, 106, 91, 91, 61, 46, 31, 241, 226, 211, 181,
            181, 166, 151, 151, 151, 151, 166, 181, 181, 211, 226, 241, 31, 46, 61, 91, 91, 106,
            121,
        ];
        assert_eq!(encoded, golden);
    }

    #[cfg(feature = "libopus")]
    #[test]
    fn opus_roundtrip_stereo() {
        let config = EncoderConfig {
            frame_samples: 240,
            layout: ChannelLayout::Stereo,
            backend: BackendPreference::Libopus,
            ..EncoderConfig::default()
        };
        let mut encoder = Encoder::new(config).expect("libopus encoder");
        let mut decoder = Decoder::new(config).expect("libopus decoder");
        let pcm = sine_pcm(240, 2);
        let encoded = encoder.encode(&pcm).expect("encode");
        assert!(
            !encoded.is_empty(),
            "encoded opus payload must not be empty"
        );
        let decoded = decoder.decode(&encoded).expect("decode");
        assert_eq!(decoded.len(), pcm.len());
    }

    #[cfg(not(feature = "libopus"))]
    #[test]
    fn libopus_preference_requires_feature() {
        let err = match Encoder::new(EncoderConfig {
            backend: BackendPreference::Libopus,
            ..EncoderConfig::default()
        }) {
            Ok(_) => panic!("libopus backend must be unavailable without feature"),
            Err(err) => err,
        };
        assert!(matches!(err, CodecError::BackendUnavailable { .. }));
    }
}

mod adpcm {
    use std::cmp;

    use super::{CodecError, EncoderConfig, MAX_CHANNELS};

    #[derive(Clone, Copy, Debug)]
    struct AdpcmState {
        predictor: i32,
        index: u8,
    }

    impl AdpcmState {
        fn new() -> Self {
            Self {
                predictor: 0,
                index: 0,
            }
        }
    }

    const STEP_TABLE: [i16; 89] = [
        7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60,
        66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371,
        408, 449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878,
        2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845,
        8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086,
        29794, 32767,
    ];

    const INDEX_TABLE: [i8; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

    pub(crate) struct Encoder {
        frame_samples: u16,
        channels: usize,
        state: Vec<AdpcmState>,
    }

    impl Encoder {
        pub(crate) fn new(config: &EncoderConfig) -> Result<Self, CodecError> {
            let channels = config.layout.channel_count();
            if channels > usize::from(MAX_CHANNELS) {
                return Err(CodecError::InvalidChannelCount {
                    expected: Some(MAX_CHANNELS),
                    found: channels as u8,
                });
            }
            Ok(Self {
                frame_samples: config.frame_samples,
                channels,
                state: vec![AdpcmState::new(); channels],
            })
        }

        pub(crate) fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>, CodecError> {
            let samples_per_channel = self.frame_samples as usize;
            let total_samples = samples_per_channel * self.channels;
            if pcm.len() != total_samples {
                return Err(CodecError::invalid_pcm_length(total_samples, pcm.len()));
            }

            let header_len = 4 + self.channels * 3;
            let mut payload = Vec::with_capacity(header_len + total_samples.div_ceil(2));
            payload.push(0); // format version
            payload.push(self.channels as u8);
            payload.extend_from_slice(&self.frame_samples.to_le_bytes());
            for state in &self.state {
                payload.extend_from_slice(&(state.predictor as i16).to_le_bytes());
                payload.push(state.index);
            }

            let mut nibble_acc = 0u8;
            let mut has_low_nibble = false;
            let mut local_state = self.state.clone();

            for sample_idx in 0..samples_per_channel {
                for (channel_idx, state) in local_state.iter_mut().enumerate() {
                    let pcm_index = sample_idx * self.channels + channel_idx;
                    let sample = pcm[pcm_index];
                    let nibble = encode_ima_sample(sample, state);
                    if !has_low_nibble {
                        nibble_acc = nibble;
                        has_low_nibble = true;
                    } else {
                        payload.push(nibble_acc | (nibble << 4));
                        has_low_nibble = false;
                    }
                }
            }

            if has_low_nibble {
                payload.push(nibble_acc);
            }

            self.state = local_state;
            Ok(payload)
        }
    }

    pub(crate) struct Decoder {
        frame_samples: u16,
        channels: usize,
        state: Vec<AdpcmState>,
    }

    impl Decoder {
        pub(crate) fn new(config: &EncoderConfig) -> Result<Self, CodecError> {
            let channels = config.layout.channel_count();
            if channels > usize::from(MAX_CHANNELS) {
                return Err(CodecError::InvalidChannelCount {
                    expected: Some(MAX_CHANNELS),
                    found: channels as u8,
                });
            }
            Ok(Self {
                frame_samples: config.frame_samples,
                channels,
                state: vec![AdpcmState::new(); channels],
            })
        }

        pub(crate) fn decode(&mut self, payload: &[u8]) -> Result<Vec<i16>, CodecError> {
            if payload.len() < 4 {
                return Err(CodecError::PacketTooShort);
            }
            let version = payload[0];
            if version != 0 {
                return Err(CodecError::UnsupportedVersion(version));
            }

            let channel_count = payload[1];
            if channel_count as usize != self.channels {
                return Err(CodecError::InvalidChannelCount {
                    expected: Some(self.channels as u8),
                    found: channel_count,
                });
            }
            let mut offset = 2;
            let mut sample_bytes = [0u8; 2];
            sample_bytes.copy_from_slice(&payload[offset..offset + 2]);
            offset += 2;
            let samples_per_channel = u16::from_le_bytes(sample_bytes);
            if samples_per_channel != self.frame_samples {
                return Err(CodecError::InvalidSampleCount {
                    expected: self.frame_samples,
                    found: samples_per_channel,
                });
            }

            let mut local_state = self.state.clone();
            for state in &mut local_state {
                if offset + 3 > payload.len() {
                    return Err(CodecError::PacketTooShort);
                }
                let predictor =
                    i16::from_le_bytes(payload[offset..offset + 2].try_into().unwrap()) as i32;
                let index = payload[offset + 2].min((STEP_TABLE.len() - 1) as u8);
                offset += 3;
                *state = AdpcmState { predictor, index };
            }

            let total_samples = samples_per_channel as usize * self.channels;
            let mut pcm = vec![0i16; total_samples];
            let mut has_low_nibble = false;
            let mut current_byte = 0u8;

            for sample_idx in 0..samples_per_channel as usize {
                for (channel_idx, state) in local_state.iter_mut().enumerate() {
                    let nibble = if !has_low_nibble {
                        if offset >= payload.len() {
                            return Err(CodecError::PacketTooShort);
                        }
                        current_byte = payload[offset];
                        offset += 1;
                        has_low_nibble = true;
                        current_byte & 0x0F
                    } else {
                        has_low_nibble = false;
                        (current_byte >> 4) & 0x0F
                    };
                    let sample = decode_ima_nibble(nibble, state);
                    pcm[sample_idx * self.channels + channel_idx] = sample;
                }
            }

            self.state = local_state;
            Ok(pcm)
        }
    }

    fn encode_ima_sample(sample: i16, state: &mut AdpcmState) -> u8 {
        let mut step = STEP_TABLE[state.index as usize] as i32;
        let mut diff = sample as i32 - state.predictor;
        let mut nibble = 0u8;
        if diff < 0 {
            nibble |= 0x8;
            diff = -diff;
        }

        let mut delta = step >> 3;
        if diff >= step {
            nibble |= 0x4;
            diff -= step;
            delta += step;
        }
        step >>= 1;
        if diff >= step {
            nibble |= 0x2;
            diff -= step;
            delta += step;
        }
        step >>= 1;
        if diff >= step {
            nibble |= 0x1;
            delta += step;
        }

        let predictor = if (nibble & 0x8) != 0 {
            state.predictor - delta
        } else {
            state.predictor + delta
        };
        state.predictor = predictor.clamp(i16::MIN as i32, i16::MAX as i32);

        let mut index = state.index as i32 + i32::from(INDEX_TABLE[nibble as usize]);
        index = cmp::max(0, cmp::min(index, (STEP_TABLE.len() - 1) as i32));
        state.index = index as u8;
        nibble
    }

    fn decode_ima_nibble(nibble: u8, state: &mut AdpcmState) -> i16 {
        let mut step = STEP_TABLE[state.index as usize] as i32;
        let mut diff = step >> 3;
        if (nibble & 0x4) != 0 {
            diff += step;
        }
        step >>= 1;
        if (nibble & 0x2) != 0 {
            diff += step;
        }
        step >>= 1;
        if (nibble & 0x1) != 0 {
            diff += step;
        }

        if (nibble & 0x8) != 0 {
            state.predictor -= diff;
        } else {
            state.predictor += diff;
        }
        state.predictor = state.predictor.clamp(i16::MIN as i32, i16::MAX as i32);

        let mut index = i32::from(state.index) + i32::from(INDEX_TABLE[nibble as usize]);
        index = cmp::max(0, cmp::min(index, (STEP_TABLE.len() - 1) as i32));
        state.index = index as u8;
        state.predictor as i16
    }
}

mod native {
    use super::{CodecError, EncoderConfig, MAX_CHANNELS};

    const VERSION: u8 = 0;
    const HEADER_LEN: usize = 5;

    pub(crate) struct Encoder {
        frame_samples: u16,
        channels: usize,
    }

    impl Encoder {
        pub(crate) fn new(config: &EncoderConfig) -> Result<Self, CodecError> {
            let channels = config.layout.channel_count();
            if channels > usize::from(MAX_CHANNELS) {
                return Err(CodecError::InvalidChannelCount {
                    expected: Some(MAX_CHANNELS),
                    found: channels as u8,
                });
            }
            if config.frame_samples == 0 {
                return Err(CodecError::InvalidSampleCount {
                    expected: 1,
                    found: 0,
                });
            }
            Ok(Self {
                frame_samples: config.frame_samples,
                channels,
            })
        }

        pub(crate) fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>, CodecError> {
            let samples_per_channel = self.frame_samples as usize;
            let total_samples = samples_per_channel * self.channels;
            if pcm.len() != total_samples {
                return Err(CodecError::invalid_pcm_length(total_samples, pcm.len()));
            }

            let header_size = HEADER_LEN + self.channels * 4;
            let nibble_count = samples_per_channel
                .saturating_sub(1)
                .saturating_mul(self.channels);
            let payload_bytes = nibble_count.div_ceil(2);
            let mut payload = Vec::with_capacity(header_size + payload_bytes);
            payload.push(VERSION);
            payload.push(self.channels as u8);
            payload.extend_from_slice(&self.frame_samples.to_le_bytes());
            payload.push(0); // reserved flags

            let mut baselines = Vec::with_capacity(self.channels);
            let mut quant_steps = Vec::with_capacity(self.channels);
            for channel in 0..self.channels {
                let baseline = pcm[channel];
                let max_abs_diff = max_abs_delta(pcm, channel, self.channels, samples_per_channel);
                let step = compute_step(max_abs_diff);
                baselines.push(baseline);
                quant_steps.push(step);
                payload.extend_from_slice(&baseline.to_le_bytes());
                payload.extend_from_slice(&step.to_le_bytes());
            }

            let mut previous: Vec<i32> = baselines.iter().map(|&s| i32::from(s)).collect();
            let mut nibble_acc = 0u8;
            let mut has_low_nibble = false;

            for sample_idx in 1..samples_per_channel {
                for channel in 0..self.channels {
                    let sample = i32::from(pcm[sample_idx * self.channels + channel]);
                    let step = i32::from(quant_steps[channel]);
                    let diff = sample - previous[channel];
                    let q = quantize(diff, step);
                    previous[channel] = (previous[channel] + i32::from(q) * step)
                        .clamp(i16::MIN as i32, i16::MAX as i32);
                    let nibble = encode_nibble(q);
                    if !has_low_nibble {
                        nibble_acc = nibble;
                        has_low_nibble = true;
                    } else {
                        payload.push(nibble_acc | (nibble << 4));
                        has_low_nibble = false;
                    }
                }
            }

            if has_low_nibble {
                payload.push(nibble_acc);
            }

            Ok(payload)
        }
    }

    pub(crate) struct Decoder {
        frame_samples: u16,
        channels: usize,
    }

    impl Decoder {
        pub(crate) fn new(config: &EncoderConfig) -> Result<Self, CodecError> {
            let channels = config.layout.channel_count();
            if channels > usize::from(MAX_CHANNELS) {
                return Err(CodecError::InvalidChannelCount {
                    expected: Some(MAX_CHANNELS),
                    found: channels as u8,
                });
            }
            if config.frame_samples == 0 {
                return Err(CodecError::InvalidSampleCount {
                    expected: 1,
                    found: 0,
                });
            }
            Ok(Self {
                frame_samples: config.frame_samples,
                channels,
            })
        }

        pub(crate) fn decode(&mut self, payload: &[u8]) -> Result<Vec<i16>, CodecError> {
            if payload.len() < HEADER_LEN {
                return Err(CodecError::PacketTooShort);
            }
            let version = payload[0];
            if version != VERSION {
                return Err(CodecError::UnsupportedVersion(version));
            }
            let channel_count = payload[1];
            if channel_count as usize != self.channels {
                return Err(CodecError::InvalidChannelCount {
                    expected: Some(self.channels as u8),
                    found: channel_count,
                });
            }
            let frame_samples = u16::from_le_bytes([payload[2], payload[3]]);
            if frame_samples != self.frame_samples {
                return Err(CodecError::InvalidSampleCount {
                    expected: self.frame_samples,
                    found: frame_samples,
                });
            }
            let mut offset = HEADER_LEN;
            if payload.len() < offset + self.channels * 4 {
                return Err(CodecError::PacketTooShort);
            }

            let mut baselines = Vec::with_capacity(self.channels);
            let mut steps = Vec::with_capacity(self.channels);
            for _ in 0..self.channels {
                let baseline = i16::from_le_bytes([payload[offset], payload[offset + 1]]);
                offset += 2;
                let step = u16::from_le_bytes([payload[offset], payload[offset + 1]]);
                offset += 2;
                baselines.push(baseline);
                steps.push(step.max(1));
            }

            let samples_per_channel = self.frame_samples as usize;
            let total_samples = samples_per_channel * self.channels;
            let expected_nibbles = samples_per_channel
                .saturating_sub(1)
                .saturating_mul(self.channels);
            let expected_bytes = expected_nibbles.div_ceil(2);
            if payload.len() < offset + expected_bytes {
                return Err(CodecError::PacketTooShort);
            }
            let nibble_buffer = &payload[offset..offset + expected_bytes];

            let mut output = Vec::with_capacity(total_samples);
            let mut previous: Vec<i32> = baselines.iter().map(|&s| i32::from(s)).collect();
            for (channel, baseline) in baselines.iter().enumerate() {
                output.push(*baseline);
                if samples_per_channel == 1 {
                    previous[channel] = i32::from(*baseline);
                }
            }

            if samples_per_channel == 1 {
                return Ok(output);
            }

            let mut nibble_iter = NibbleIterator::new(nibble_buffer);
            for _sample in 1..samples_per_channel {
                for channel in 0..self.channels {
                    let nibble = nibble_iter.next().ok_or(CodecError::PacketTooShort)?;
                    let quant = decode_nibble(nibble);
                    let step = i32::from(steps[channel]);
                    let predicted = previous[channel] + i32::from(quant) * step;
                    let clamped = predicted.clamp(i16::MIN as i32, i16::MAX as i32);
                    previous[channel] = clamped;
                    output.push(clamped as i16);
                }
            }

            Ok(output)
        }
    }

    fn max_abs_delta(
        pcm: &[i16],
        channel: usize,
        channels: usize,
        samples_per_channel: usize,
    ) -> i32 {
        let mut prev = i32::from(pcm[channel]);
        let mut max_abs = 0i32;
        for sample_idx in 1..samples_per_channel {
            let sample = i32::from(pcm[sample_idx * channels + channel]);
            let diff = sample - prev;
            let abs = diff.abs();
            if abs > max_abs {
                max_abs = abs;
            }
            prev = sample;
        }
        max_abs
    }

    fn compute_step(max_abs_diff: i32) -> u16 {
        if max_abs_diff <= 7 {
            1
        } else {
            let value = ((max_abs_diff as u32 + 7) / 7).min(u16::MAX as u32);
            value as u16
        }
    }

    fn quantize(diff: i32, step: i32) -> i8 {
        if step <= 0 {
            return 0;
        }
        let half = step / 2;
        let mut q = if diff >= 0 {
            (diff + half) / step
        } else {
            (diff - half) / step
        };
        q = q.clamp(-8, 7);
        q as i8
    }

    fn encode_nibble(value: i8) -> u8 {
        (value as u8) & 0x0F
    }

    fn decode_nibble(nibble: u8) -> i8 {
        ((nibble << 4) as i8) >> 4
    }

    struct NibbleIterator<'a> {
        bytes: &'a [u8],
        index: usize,
        use_high: bool,
    }

    impl<'a> NibbleIterator<'a> {
        fn new(bytes: &'a [u8]) -> Self {
            Self {
                bytes,
                index: 0,
                use_high: false,
            }
        }
    }

    impl<'a> Iterator for NibbleIterator<'a> {
        type Item = u8;

        fn next(&mut self) -> Option<Self::Item> {
            if self.index >= self.bytes.len() {
                return None;
            }
            let byte = self.bytes[self.index];
            if self.use_high {
                self.index += 1;
                self.use_high = false;
                Some(byte >> 4)
            } else {
                self.use_high = true;
                Some(byte & 0x0F)
            }
        }
    }
}

#[cfg(feature = "libopus")]
mod opus_backend {
    use std::sync::Arc;

    use opus::{Application, Channels, Decoder as LibOpusDecoder, Encoder as LibOpusEncoder};

    use super::{ChannelLayout, CodecError, EncoderConfig};

    const DEFAULT_OPUS_PACKET: usize = 1276;

    pub(crate) struct Encoder {
        inner: LibOpusEncoder,
        channels: usize,
        frame_samples: u16,
        max_packet_size: usize,
    }

    impl Encoder {
        pub(crate) fn new(config: &EncoderConfig) -> Result<Self, CodecError> {
            let (channels, opus_channels) = opus_layout(config.layout)?;
            let mut inner =
                LibOpusEncoder::new(config.sample_rate, opus_channels, Application::Audio)
                    .map_err(libopus_error)?;

            let bitrate = config
                .target_bitrate
                .unwrap_or_else(|| default_bitrate(config.layout));
            let bitrate_bits = bitrate.min(i32::MAX as u32) as i32;
            inner
                .set_bitrate(opus::Bitrate::Bits(bitrate_bits))
                .map_err(libopus_error)?;
            inner
                .set_inband_fec(config.fec_level > 0)
                .map_err(libopus_error)?;
            let packet_loss = packet_loss_from_level(config.fec_level).min(i32::MAX as u32) as i32;
            inner
                .set_packet_loss_perc(packet_loss)
                .map_err(libopus_error)?;
            inner.set_vbr(false).map_err(libopus_error)?;
            inner.set_vbr_constraint(true).map_err(libopus_error)?;

            Ok(Self {
                inner,
                channels,
                frame_samples: config.frame_samples,
                max_packet_size: max_packet_size(config.layout),
            })
        }

        pub(crate) fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>, CodecError> {
            let expected = usize::from(self.frame_samples) * self.channels;
            if pcm.len() != expected {
                return Err(CodecError::InvalidPcmLength {
                    expected,
                    found: pcm.len(),
                });
            }
            let mut buffer = vec![0u8; self.max_packet_size];
            let len = self.inner.encode(pcm, &mut buffer).map_err(libopus_error)?;
            buffer.truncate(len);
            Ok(buffer)
        }
    }

    pub(crate) struct Decoder {
        inner: LibOpusDecoder,
        channels: usize,
        frame_samples: u16,
    }

    impl Decoder {
        pub(crate) fn new(config: &EncoderConfig) -> Result<Self, CodecError> {
            let (channels, opus_channels) = opus_layout(config.layout)?;
            let inner =
                LibOpusDecoder::new(config.sample_rate, opus_channels).map_err(libopus_error)?;
            Ok(Self {
                inner,
                channels,
                frame_samples: config.frame_samples,
            })
        }

        pub(crate) fn decode(&mut self, payload: &[u8]) -> Result<Vec<i16>, CodecError> {
            if payload.is_empty() {
                return Err(CodecError::PacketTooShort);
            }
            let mut pcm = vec![0i16; usize::from(self.frame_samples) * self.channels];
            let decoded_samples = self
                .inner
                .decode(payload, &mut pcm, false)
                .map_err(libopus_error)?;
            pcm.truncate(decoded_samples * self.channels);
            Ok(pcm)
        }
    }

    fn opus_layout(layout: ChannelLayout) -> Result<(usize, Channels), CodecError> {
        match layout {
            ChannelLayout::Mono => Ok((1, Channels::Mono)),
            ChannelLayout::Stereo => Ok((2, Channels::Stereo)),
            ChannelLayout::FirstOrderAmbisonics => Err(CodecError::UnsupportedLayout { layout }),
        }
    }

    fn packet_loss_from_level(level: u8) -> u32 {
        match level {
            0 => 0,
            1 => 5,
            2 => 10,
            _ => 20,
        }
    }

    fn default_bitrate(layout: ChannelLayout) -> u32 {
        match layout {
            ChannelLayout::Mono => 64_000,
            ChannelLayout::Stereo => 96_000,
            ChannelLayout::FirstOrderAmbisonics => 192_000,
        }
    }

    fn max_packet_size(layout: ChannelLayout) -> usize {
        match layout {
            ChannelLayout::Mono | ChannelLayout::Stereo => DEFAULT_OPUS_PACKET,
            ChannelLayout::FirstOrderAmbisonics => DEFAULT_OPUS_PACKET * 2,
        }
    }

    fn libopus_error(err: opus::Error) -> CodecError {
        CodecError::LibopusError {
            message: Arc::from(err.to_string()),
        }
    }
}
