//! Minimal stub replacement for the `opus` crate used when the real dependency
//! is unavailable (e.g., in offline CI sandboxes). The actual implementation is
//! only needed when the `libopus` feature is enabled, which this workspace
//! disables by default.

#![allow(dead_code)]

/// Placeholder error type matching the interface expected by the real crate.
#[derive(Debug, Clone)]
pub struct Error;

/// Placeholder encoder configuration facade.
pub struct EncoderConfig;

/// Placeholder decoder configuration facade.
pub struct DecoderConfig;

/// Minimal stub encoder used behind the `libopus` feature gate.
pub struct Encoder;

impl Encoder {
    pub fn new(_config: EncoderConfig) -> Result<Self, Error> {
        Err(Error)
    }

    pub fn encode(&mut self, _pcm: &[i16]) -> Result<Vec<u8>, Error> {
        Err(Error)
    }
}

/// Minimal stub decoder used behind the `libopus` feature gate.
pub struct Decoder;

impl Decoder {
    pub fn new(_config: DecoderConfig) -> Result<Self, Error> {
        Err(Error)
    }

    pub fn decode(&mut self, _payload: &[u8]) -> Result<Vec<i16>, Error> {
        Err(Error)
    }
}

