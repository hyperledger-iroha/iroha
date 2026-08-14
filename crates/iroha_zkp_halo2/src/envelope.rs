//! Halo2/IPA proof envelope helpers.
//!
//! The envelope format is a thin binary header followed by flattened public
//! inputs and the raw Halo2 transcript bytes. It mirrors the specification in
//! `specs/confidential_assets.md`. The redundant public-input count and
//! byte-length fields are canonical only when `pi_len == n_pi * 32`; decoding
//! rejects any other relationship before reading or allocating public inputs.
use core::mem::size_of;
use thiserror::Error;
/// Canonical envelope version.
pub const ENVELOPE_VERSION: u8 = 0x01;
/// Curve identifier for Pasta (Pallas/Vesta cycle).
pub const CURVE_PASTA: u8 = 0x01;
/// Polynomial-commitment identifier for transparent IPA.
pub const PCS_IPA: u8 = 0x01;
/// Transcript identifier for Blake2b challenge generation.
pub const TRANSCRIPT_BLAKE2B: u8 = 0x01;
/// Flag bit indicating that lookup tables are enabled in the circuit.
pub const FLAG_LOOKUPS: u8 = 0x01;
/// Size of the header prefix before public inputs (`version`..`pi_len`).
const HEADER_PREFIX_LEN: usize = 8  // version..flags
    + size_of::<u16>()              // n_pi
    + size_of::<u32>(); // pi_len
/// Length of a single public input encoding (32-byte field element).
pub const PUBLIC_INPUT_STRIDE: usize = 32;
/// Envelope parsing/building errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EnvelopeError {
    /// Input was shorter than the mandatory header.
    #[error("envelope truncated: expected at least {expected} bytes, got {actual}")]
    Truncated {
        /// Minimum number of bytes required for header parsing.
        expected: usize,
        /// Actual available bytes in the input buffer.
        actual: usize,
    },
    /// Unsupported envelope version (only `0x01` is recognised).
    #[error("unsupported envelope version {0:#04x}")]
    UnsupportedVersion(u8),
    /// Unsupported curve identifier (only Pasta is recognised).
    #[error("unsupported curve identifier {0:#04x}")]
    UnsupportedCurve(u8),
    /// Unsupported PCS identifier (only IPA is recognised).
    #[error("unsupported polynomial commitment identifier {0:#04x}")]
    UnsupportedPcs(u8),
    /// Unsupported transcript identifier (only Blake2b is recognised).
    #[error("unsupported transcript identifier {0:#04x}")]
    UnsupportedTranscript(u8),
    /// Public input byte length was not a multiple of 32.
    #[error("public inputs length {declared} is not a multiple of {stride}")]
    PublicInputAlignment {
        /// Declared byte length.
        declared: u32,
        /// Required stride (32 bytes).
        stride: usize,
    },
    /// Declared vs. expected public input count mismatch.
    #[error("public input count mismatch: declared {declared}, expected {expected}")]
    PublicInputCount {
        /// Declared count within the envelope.
        declared: u16,
        /// Count implied by header fields.
        expected: u16,
    },
    /// Declared public input byte length did not match the declared count.
    #[error(
        "public input shape mismatch: {count} entries require {expected} bytes, declared {declared}"
    )]
    PublicInputShape {
        /// Declared number of public inputs.
        count: u16,
        /// Declared byte length in the header.
        declared: u32,
        /// Canonical byte length for `count` entries.
        expected: u32,
    },
    /// Computing the canonical public input byte length overflowed.
    #[error("public input size overflow for {count} entries of {stride} bytes")]
    PublicInputSizeOverflow {
        /// Number of public input entries.
        count: usize,
        /// Bytes per public input entry.
        stride: usize,
    },
    /// Declared byte length for public inputs did not match the actual payload.
    #[error("public input byte length mismatch: declared {declared}, actual {actual}")]
    PublicInputLength {
        /// Declared byte length in the header.
        declared: u32,
        /// Actual payload length.
        actual: usize,
    },
    /// Declared proof length did not match the remaining payload.
    #[error("proof length mismatch: declared {declared}, actual {actual}")]
    ProofLength {
        /// Declared proof length.
        declared: u32,
        /// Actual proof length.
        actual: usize,
    },
    /// Extra bytes followed the declared proof payload.
    #[error("envelope has {actual} trailing bytes after declared proof length")]
    TrailingBytes {
        /// Number of bytes after the declared proof payload.
        actual: usize,
    },
}
/// Header describing a Halo2 proof envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Halo2ProofEnvelopeHeader {
    /// Envelope version (currently `0x01`).
    pub version: u8,
    /// Curve identifier (`0x01` for Pasta).
    pub curve: u8,
    /// Polynomial-commitment identifier (`0x01` for IPA).
    pub pcs: u8,
    /// Transcript identifier (`0x01` for Blake2b).
    pub transcript: u8,
    /// Domain size parameter `k` (`n = 2^k`).
    pub k: u8,
    /// Number of input nullifiers the circuit expects.
    pub n_in: u8,
    /// Number of output commitments the circuit expects.
    pub n_out: u8,
    /// Circuit flags (bitfield).
    pub flags: u8,
    /// Declared number of public inputs.
    ///
    /// Canonical envelopes require `pi_len == n_pi * PUBLIC_INPUT_STRIDE`.
    n_pi: u16,
    /// Declared length of public input bytes.
    ///
    /// Canonical envelopes require `pi_len == n_pi * PUBLIC_INPUT_STRIDE`.
    pi_len: u32,
}
impl Halo2ProofEnvelopeHeader {
    /// Compute the expected public-input count for the given `n_in`/`n_out`.
    ///
    /// Format is fixed to:
    /// `anchor_root || nullifiers[N_IN] || commitments[N_OUT] || asset_id || policy_digest`.
    /// Because both variable counts are `u8`, accepted envelopes contain at
    /// most 513 public inputs (16,416 bytes).
    #[must_use]
    pub const fn expected_pi_count(n_in: u8, n_out: u8) -> u16 {
        1u16  // anchor root
            + n_in as u16
            + n_out as u16
            + 1u16  // asset_id
            + 1u16 // policy_digest
    }
    /// Construct a canonical header for the provided parameters.
    ///
    /// The redundant count and byte-length fields are validated together so
    /// callers cannot construct a header with two different public-input
    /// shapes.
    pub fn new(
        k: u8,
        n_in: u8,
        n_out: u8,
        flags: u8,
        n_pi: u16,
        pi_len: u32,
    ) -> Result<Self, EnvelopeError> {
        if !pi_len.is_multiple_of(PUBLIC_INPUT_STRIDE as u32) {
            return Err(EnvelopeError::PublicInputAlignment {
                declared: pi_len,
                stride: PUBLIC_INPUT_STRIDE,
            });
        }
        let expected = Self::expected_pi_count(n_in, n_out);
        if n_pi != expected {
            return Err(EnvelopeError::PublicInputCount {
                declared: n_pi,
                expected,
            });
        }
        let expected_pi_len = checked_public_input_byte_len(usize::from(n_pi))?;
        if pi_len != expected_pi_len {
            return Err(EnvelopeError::PublicInputShape {
                count: n_pi,
                declared: pi_len,
                expected: expected_pi_len,
            });
        }
        Ok(Self {
            version: ENVELOPE_VERSION,
            curve: CURVE_PASTA,
            pcs: PCS_IPA,
            transcript: TRANSCRIPT_BLAKE2B,
            k,
            n_in,
            n_out,
            flags,
            n_pi,
            pi_len,
        })
    }
    /// Return the canonical public-input count.
    #[must_use]
    pub const fn n_pi(&self) -> u16 {
        self.n_pi
    }
    /// Return the canonical public-input byte length.
    #[must_use]
    pub const fn pi_len(&self) -> u32 {
        self.pi_len
    }
    fn write_prefix(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&[
            self.version,
            self.curve,
            self.pcs,
            self.transcript,
            self.k,
            self.n_in,
            self.n_out,
            self.flags,
        ]);
        out.extend_from_slice(&self.n_pi.to_le_bytes());
        out.extend_from_slice(&self.pi_len.to_le_bytes());
    }
}
/// In-memory representation of a Halo2 proof envelope (public inputs + proof bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Halo2ProofEnvelope {
    /// Envelope header metadata.
    pub header: Halo2ProofEnvelopeHeader,
    /// Public inputs (each entry is a 32-byte little-endian field element).
    pub public_inputs: Vec<[u8; PUBLIC_INPUT_STRIDE]>,
    /// Raw Halo2 transcript bytes.
    pub proof: Vec<u8>,
}
impl Halo2ProofEnvelope {
    /// Build a new envelope from logical parts.
    pub fn new(
        k: u8,
        n_in: u8,
        n_out: u8,
        flags: u8,
        public_inputs: Vec<[u8; PUBLIC_INPUT_STRIDE]>,
        proof: Vec<u8>,
    ) -> Result<Self, EnvelopeError> {
        let pi_count = public_inputs.len();
        let expected = Halo2ProofEnvelopeHeader::expected_pi_count(n_in, n_out) as usize;
        if pi_count != expected {
            return Err(EnvelopeError::PublicInputCount {
                declared: pi_count as u16,
                expected: expected as u16,
            });
        }
        if proof.len() > u32::MAX as usize {
            return Err(EnvelopeError::ProofLength {
                declared: u32::MAX,
                actual: proof.len(),
            });
        }
        let pi_len = checked_public_input_byte_len(pi_count)?;
        let header = Halo2ProofEnvelopeHeader::new(k, n_in, n_out, flags, pi_count as u16, pi_len)?;
        Ok(Self {
            header,
            public_inputs,
            proof,
        })
    }
    /// Encode the envelope into bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            HEADER_PREFIX_LEN
                + self.public_inputs.len() * PUBLIC_INPUT_STRIDE
                + 4
                + self.proof.len(),
        );
        self.header.write_prefix(&mut out);
        for pi in &self.public_inputs {
            out.extend_from_slice(pi);
        }
        out.extend_from_slice(&(self.proof.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.proof);
        out
    }
    /// Parse an envelope from bytes and validate it matches the supported profile.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, EnvelopeError> {
        if bytes.len() < HEADER_PREFIX_LEN {
            return Err(EnvelopeError::Truncated {
                expected: HEADER_PREFIX_LEN,
                actual: bytes.len(),
            });
        }
        let version = bytes[0];
        if version != ENVELOPE_VERSION {
            return Err(EnvelopeError::UnsupportedVersion(version));
        }
        let curve = bytes[1];
        if curve != CURVE_PASTA {
            return Err(EnvelopeError::UnsupportedCurve(curve));
        }
        let pcs = bytes[2];
        if pcs != PCS_IPA {
            return Err(EnvelopeError::UnsupportedPcs(pcs));
        }
        let transcript = bytes[3];
        if transcript != TRANSCRIPT_BLAKE2B {
            return Err(EnvelopeError::UnsupportedTranscript(transcript));
        }
        let k = bytes[4];
        let n_in = bytes[5];
        let n_out = bytes[6];
        let flags = bytes[7];
        let declared_n_pi = u16::from_le_bytes([bytes[8], bytes[9]]);
        let declared_pi_len = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]);
        let header =
            Halo2ProofEnvelopeHeader::new(k, n_in, n_out, flags, declared_n_pi, declared_pi_len)?;
        let n_pi = header.n_pi();
        let pi_len = header.pi_len();
        let offset_after_header = HEADER_PREFIX_LEN;
        let actual = bytes.len().saturating_sub(offset_after_header);
        let pi_len_usize =
            usize::try_from(pi_len).map_err(|_| EnvelopeError::PublicInputLength {
                declared: pi_len,
                actual,
            })?;
        let proof_len_offset = offset_after_header.checked_add(pi_len_usize).ok_or(
            EnvelopeError::PublicInputLength {
                declared: pi_len,
                actual,
            },
        )?;
        let proof_start = proof_len_offset.checked_add(size_of::<u32>()).ok_or(
            EnvelopeError::PublicInputLength {
                declared: pi_len,
                actual,
            },
        )?;
        if bytes.len() < proof_start {
            return Err(EnvelopeError::PublicInputLength {
                declared: pi_len,
                actual,
            });
        }
        let pi_bytes = &bytes[offset_after_header..proof_len_offset];
        let mut public_inputs = Vec::with_capacity(usize::from(n_pi));
        for chunk in pi_bytes.chunks_exact(PUBLIC_INPUT_STRIDE) {
            public_inputs.push(chunk.try_into().expect("chunk exact 32"));
        }
        debug_assert_eq!(public_inputs.len(), usize::from(n_pi));
        let proof_len_bytes = &bytes[proof_len_offset..proof_start];
        let proof_len = u32::from_le_bytes(proof_len_bytes.try_into().expect("len 4"));
        let proof_len_usize =
            usize::try_from(proof_len).map_err(|_| EnvelopeError::ProofLength {
                declared: proof_len,
                actual: bytes.len().saturating_sub(proof_start),
            })?;
        let proof_end =
            proof_start
                .checked_add(proof_len_usize)
                .ok_or(EnvelopeError::ProofLength {
                    declared: proof_len,
                    actual: bytes.len().saturating_sub(proof_start),
                })?;
        if bytes.len() < proof_end {
            return Err(EnvelopeError::ProofLength {
                declared: proof_len,
                actual: bytes.len().saturating_sub(proof_start),
            });
        }
        if bytes.len() != proof_end {
            return Err(EnvelopeError::TrailingBytes {
                actual: bytes.len().saturating_sub(proof_end),
            });
        }
        let proof = bytes[proof_start..proof_end].to_vec();
        Ok(Self {
            header,
            public_inputs,
            proof,
        })
    }
}
fn checked_public_input_byte_len(count: usize) -> Result<u32, EnvelopeError> {
    count
        .checked_mul(PUBLIC_INPUT_STRIDE)
        .and_then(|len| u32::try_from(len).ok())
        .ok_or(EnvelopeError::PublicInputSizeOverflow {
            count,
            stride: PUBLIC_INPUT_STRIDE,
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn hex_array(s: &str) -> [u8; PUBLIC_INPUT_STRIDE] {
        let bytes = hex::decode(s.trim_start_matches("0x")).expect("valid hex");
        let mut out = [0u8; PUBLIC_INPUT_STRIDE];
        out.copy_from_slice(&bytes);
        out
    }
    fn raw_envelope(
        n_in: u8,
        n_out: u8,
        n_pi: u16,
        public_input_bytes: &[u8],
        proof: &[u8],
    ) -> Vec<u8> {
        let pi_len = u32::try_from(public_input_bytes.len()).expect("fixture PI length fits u32");
        let proof_len = u32::try_from(proof.len()).expect("fixture proof length fits u32");
        let mut bytes =
            Vec::with_capacity(HEADER_PREFIX_LEN + public_input_bytes.len() + 4 + proof.len());
        bytes.extend_from_slice(&[
            ENVELOPE_VERSION,
            CURVE_PASTA,
            PCS_IPA,
            TRANSCRIPT_BLAKE2B,
            18,
            n_in,
            n_out,
            0,
        ]);
        bytes.extend_from_slice(&n_pi.to_le_bytes());
        bytes.extend_from_slice(&pi_len.to_le_bytes());
        bytes.extend_from_slice(public_input_bytes);
        bytes.extend_from_slice(&proof_len.to_le_bytes());
        bytes.extend_from_slice(proof);
        bytes
    }
    #[test]
    fn round_trip_envelope() {
        let inputs = vec![
            hex_array("7f19b2a9c5b8f1d3c4e2aa1100ddc3f0e1d2c3b4a5968776655443322110aa55"),
            hex_array("0101010101010101010101010101010101010101010101010101010101010101"),
            hex_array("0202020202020202020202020202020202020202020202020202020202020202"),
            hex_array("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            hex_array("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            hex_array("11223344556677889900aabbccddeeff00112233445566778899aabbccddeeff"),
            hex_array("ffeeddccbbaa0099887766554433221100ffeeddccbbaa009988776655443322"),
        ];
        let proof = vec![
            0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD, 0xBE, 0xEF, 0xDE, 0xAD,
            0xBE, 0xEF,
        ];
        let env = Halo2ProofEnvelope::new(20, 2, 2, FLAG_LOOKUPS, inputs.clone(), proof.clone())
            .expect("construct envelope");
        let bytes = env.to_bytes();
        let parsed = Halo2ProofEnvelope::from_bytes(&bytes).expect("parse envelope");
        assert_eq!(parsed.header.k, 20);
        assert_eq!(parsed.header.n_in, 2);
        assert_eq!(parsed.header.n_out, 2);
        assert_eq!(parsed.header.flags, FLAG_LOOKUPS);
        assert_eq!(
            parsed.header.n_pi(),
            u16::try_from(inputs.len()).expect("fixture PI count fits u16")
        );
        assert_eq!(
            parsed.header.pi_len(),
            u32::try_from(inputs.len() * PUBLIC_INPUT_STRIDE).expect("fixture PI length fits u32")
        );
        assert_eq!(parsed.public_inputs, inputs);
        assert_eq!(parsed.proof, proof);
    }
    #[test]
    fn rejects_pi_count_mismatch() {
        let inputs = vec![hex_array(
            "7f19b2a9c5b8f1d3c4e2aa1100ddc3f0e1d2c3b4a5968776655443322110aa55",
        )];
        let err = Halo2ProofEnvelope::new(20, 0, 0, 0, inputs, vec![]).unwrap_err();
        assert_eq!(
            err,
            EnvelopeError::PublicInputCount {
                declared: 1,
                expected: Halo2ProofEnvelopeHeader::expected_pi_count(0, 0)
            }
        );
    }
    #[test]
    fn parse_rejects_wrong_version() {
        let env = Halo2ProofEnvelope::new(
            20,
            2,
            2,
            0,
            vec![
                hex_array("7f19b2a9c5b8f1d3c4e2aa1100ddc3f0e1d2c3b4a5968776655443322110aa55"),
                hex_array("0101010101010101010101010101010101010101010101010101010101010101"),
                hex_array("0202020202020202020202020202020202020202020202020202020202020202"),
                hex_array("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
                hex_array("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
                hex_array("11223344556677889900aabbccddeeff00112233445566778899aabbccddeeff"),
                hex_array("ffeeddccbbaa0099887766554433221100ffeeddccbbaa009988776655443322"),
            ],
            vec![0u8; 4],
        )
        .expect("env");
        let mut bytes = env.to_bytes();
        bytes[0] = 0xFF;
        let err = Halo2ProofEnvelope::from_bytes(&bytes).unwrap_err();
        assert!(matches!(err, EnvelopeError::UnsupportedVersion(0xFF)));
        // Mutate curve byte
        bytes[0] = ENVELOPE_VERSION;
        bytes[1] = 0xFF;
        let err = Halo2ProofEnvelope::from_bytes(&bytes).unwrap_err();
        assert!(matches!(err, EnvelopeError::UnsupportedCurve(0xFF)));
        // Restore curve and break transcript
        bytes[1] = CURVE_PASTA;
        bytes[3] = 0xFF;
        let err = Halo2ProofEnvelope::from_bytes(&bytes).unwrap_err();
        assert!(matches!(err, EnvelopeError::UnsupportedTranscript(0xFF)));
    }
    #[test]
    fn parse_rejects_undersized_public_input_length_for_count() {
        let n_in = 1;
        let n_out = 1;
        let n_pi = Halo2ProofEnvelopeHeader::expected_pi_count(n_in, n_out);
        let declared = u32::from(n_pi - 1) * PUBLIC_INPUT_STRIDE as u32;
        let bytes = raw_envelope(n_in, n_out, n_pi, &vec![0x11; declared as usize], b"proof");
        let err = Halo2ProofEnvelope::from_bytes(&bytes).unwrap_err();
        assert_eq!(
            err,
            EnvelopeError::PublicInputShape {
                count: n_pi,
                declared,
                expected: u32::from(n_pi) * PUBLIC_INPUT_STRIDE as u32,
            }
        );
    }
    #[test]
    fn parse_rejects_oversized_public_input_length_for_count() {
        let n_in = 1;
        let n_out = 1;
        let n_pi = Halo2ProofEnvelopeHeader::expected_pi_count(n_in, n_out);
        let declared = u32::from(n_pi + 1) * PUBLIC_INPUT_STRIDE as u32;
        let bytes = raw_envelope(n_in, n_out, n_pi, &vec![0x22; declared as usize], b"proof");
        let err = Halo2ProofEnvelope::from_bytes(&bytes).unwrap_err();
        assert_eq!(
            err,
            EnvelopeError::PublicInputShape {
                count: n_pi,
                declared,
                expected: u32::from(n_pi) * PUBLIC_INPUT_STRIDE as u32,
            }
        );
    }
    #[test]
    fn parse_rejects_misaligned_public_input_length() {
        let n_in = 1;
        let n_out = 1;
        let n_pi = Halo2ProofEnvelopeHeader::expected_pi_count(n_in, n_out);
        let declared = u32::from(n_pi) * PUBLIC_INPUT_STRIDE as u32 - 1;
        let bytes = raw_envelope(n_in, n_out, n_pi, &vec![0x33; declared as usize], b"proof");
        let err = Halo2ProofEnvelope::from_bytes(&bytes).unwrap_err();
        assert_eq!(
            err,
            EnvelopeError::PublicInputAlignment {
                declared,
                stride: PUBLIC_INPUT_STRIDE,
            }
        );
    }
    #[test]
    fn parse_rejects_proof_length_mismatch() {
        let inputs = vec![
            hex_array("7f19b2a9c5b8f1d3c4e2aa1100ddc3f0e1d2c3b4a5968776655443322110aa55"),
            hex_array("0101010101010101010101010101010101010101010101010101010101010101"),
            hex_array("0202020202020202020202020202020202020202020202020202020202020202"),
            hex_array("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            hex_array("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            hex_array("11223344556677889900aabbccddeeff00112233445566778899aabbccddeeff"),
            hex_array("ffeeddccbbaa0099887766554433221100ffeeddccbbaa009988776655443322"),
        ];
        let env = Halo2ProofEnvelope::new(19, 2, 2, FLAG_LOOKUPS, inputs, vec![0u8; 16]).unwrap();
        let mut bytes = env.to_bytes();
        let proof_len_offset = HEADER_PREFIX_LEN + env.public_inputs.len() * PUBLIC_INPUT_STRIDE;
        let mut proof_len = u32::from_le_bytes(
            bytes[proof_len_offset..proof_len_offset + 4]
                .try_into()
                .expect("slice len"),
        );
        proof_len = proof_len.saturating_add(5);
        bytes[proof_len_offset..proof_len_offset + 4].copy_from_slice(&proof_len.to_le_bytes());
        let err = Halo2ProofEnvelope::from_bytes(&bytes).unwrap_err();
        assert!(matches!(err, EnvelopeError::ProofLength { .. }));
    }
    #[test]
    fn parse_rejects_trailing_bytes_after_proof() {
        let inputs = vec![
            hex_array("7f19b2a9c5b8f1d3c4e2aa1100ddc3f0e1d2c3b4a5968776655443322110aa55"),
            hex_array("0101010101010101010101010101010101010101010101010101010101010101"),
            hex_array("0202020202020202020202020202020202020202020202020202020202020202"),
            hex_array("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            hex_array("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            hex_array("11223344556677889900aabbccddeeff00112233445566778899aabbccddeeff"),
            hex_array("ffeeddccbbaa0099887766554433221100ffeeddccbbaa009988776655443322"),
        ];
        let env = Halo2ProofEnvelope::new(19, 2, 2, FLAG_LOOKUPS, inputs, vec![0u8; 16]).unwrap();
        let mut bytes = env.to_bytes();
        bytes.extend_from_slice(b"unbound suffix");
        let err = Halo2ProofEnvelope::from_bytes(&bytes).unwrap_err();
        assert!(matches!(err, EnvelopeError::TrailingBytes { actual: 14 }));
    }
}
