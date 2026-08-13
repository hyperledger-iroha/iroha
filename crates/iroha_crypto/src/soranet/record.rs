//! Post-handshake authenticated records for `SoraNet` application streams.
//!
//! QUIC protects the transport with TLS, while this layer extends the hybrid
//! `SoraNet` handshake's confidentiality and integrity guarantees to application
//! data.  Every QUIC stream and wire direction receives an independent key.
use std::fmt;
use aead::{AeadInOut, KeyInit};
use chacha20poly1305::ChaCha20Poly1305;
use hkdf::Hkdf;
use sha2::Sha256;
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};
use crate::SessionKey;
mod io;
#[doc(hidden)]
pub use zeroize::Zeroize as __RecordZeroize;
/// Magic and protocol version carried by every protected record.
pub const RECORD_MAGIC: [u8; 4] = *b"SNR1";
/// Number of bytes in the authenticated record header.
pub const RECORD_HEADER_LEN: usize = 16;
/// ChaCha20-Poly1305 authentication-tag length.
pub const RECORD_TAG_LEN: usize = 16;
/// Maximum plaintext carried by one record.
pub const MAX_RECORD_PLAINTEXT_LEN: usize = 64 * 1024;
const SESSION_KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const KDF_SALT: &[u8] = b"iroha.soranet.record.hkdf-sha256.v1";
const KDF_INFO: &[u8] = b"iroha.soranet.record.chacha20poly1305.key.v1";
/// Endpoint role used to assign unambiguous wire directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordEndpoint {
    /// The endpoint that initiated the `SoraNet` connection.
    Client = 0,
    /// The relay accepting the `SoraNet` connection.
    Relay = 1,
}
/// QUIC stream directionality committed into record-key derivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordStreamKind {
    /// A bidirectional QUIC stream.
    Bidirectional = 0,
    /// A unidirectional QUIC stream.
    Unidirectional = 1,
}
/// Stable QUIC stream identity used for per-stream key separation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordStreamContext {
    initiator: RecordEndpoint,
    kind: RecordStreamKind,
    index: u64,
}
impl RecordStreamContext {
    /// Construct a context from the QUIC initiator, directionality, and stream index.
    #[must_use]
    pub const fn new(initiator: RecordEndpoint, kind: RecordStreamKind, index: u64) -> Self {
        Self {
            initiator,
            kind,
            index,
        }
    }
    fn encoded(self) -> [u8; 10] {
        let mut encoded = [0_u8; 10];
        encoded[0] = self.initiator as u8;
        encoded[1] = self.kind as u8;
        encoded[2..].copy_from_slice(&self.index.to_be_bytes());
        encoded
    }
}
/// Failures produced by the `SoraNet` record protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RecordError {
    /// Session key has {actual} bytes; exactly 32 are required
    #[error("session key has {actual} bytes; exactly 32 are required")]
    InvalidSessionKeyLength {
        /// Actual key length.
        actual: usize,
    },
    /// Failed to derive a stream record key
    #[error("failed to derive a stream record key")]
    KeyDerivation,
    /// Record plaintext length {actual} exceeds the {maximum}-byte limit
    #[error("record plaintext length {actual} exceeds the {maximum}-byte limit")]
    PlaintextTooLarge {
        /// Actual plaintext length.
        actual: usize,
        /// Protocol maximum.
        maximum: usize,
    },
    /// Record is truncated: expected at least {expected} bytes, received {actual}
    #[error("record is truncated: expected at least {expected} bytes, received {actual}")]
    Truncated {
        /// Minimum required length.
        expected: usize,
        /// Received length.
        actual: usize,
    },
    /// Record magic or version is not SNR1
    #[error("record magic or version is not SNR1")]
    InvalidMagic,
    /// Record sequence {actual} does not match the expected sequence {expected}
    #[error("record sequence {actual} does not match the expected sequence {expected}")]
    SequenceMismatch {
        /// Expected sequence number.
        expected: u64,
        /// Received sequence number.
        actual: u64,
    },
    /// Record sequence space is exhausted
    #[error("record sequence space is exhausted")]
    SequenceExhausted,
    /// Record body length {actual} does not match the expected length {expected}
    #[error("record body length {actual} does not match the expected length {expected}")]
    LengthMismatch {
        /// Expected ciphertext-and-tag length.
        expected: usize,
        /// Actual ciphertext-and-tag length.
        actual: usize,
    },
    /// Record encryption failed
    #[error("record encryption failed")]
    Encryption,
    /// Record authentication failed
    #[error("record authentication failed")]
    Authentication,
}
#[derive(Clone, Copy)]
#[repr(u8)]
enum WireDirection {
    ClientToRelay = 0,
    RelayToClient = 1,
}
/// Root state from which direction- and stream-specific record keys are derived.
pub struct RecordLayer {
    endpoint: RecordEndpoint,
    session_key: Zeroizing<[u8; SESSION_KEY_LEN]>,
}
impl fmt::Debug for RecordLayer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecordLayer")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}
impl RecordLayer {
    /// Bind a record layer to a negotiated `SoraNet` session key and local role.
    ///
    /// # Errors
    ///
    /// Returns [`RecordError::InvalidSessionKeyLength`] unless the handshake
    /// supplied the 32-byte key required by the first-release protocol.
    pub fn new(session_key: &SessionKey, endpoint: RecordEndpoint) -> Result<Self, RecordError> {
        let payload = session_key.payload();
        if payload.len() != SESSION_KEY_LEN {
            return Err(RecordError::InvalidSessionKeyLength {
                actual: payload.len(),
            });
        }
        let mut key = Zeroizing::new([0_u8; SESSION_KEY_LEN]);
        key.copy_from_slice(payload);
        Ok(Self {
            endpoint,
            session_key: key,
        })
    }
    /// Derive independent sending and receiving state for one QUIC stream.
    ///
    /// # Errors
    ///
    /// Returns [`RecordError::KeyDerivation`] if HKDF cannot produce a key.
    pub fn stream(&self, context: RecordStreamContext) -> Result<DuplexRecordLayer, RecordError> {
        let (send_direction, receive_direction) = match self.endpoint {
            RecordEndpoint::Client => (WireDirection::ClientToRelay, WireDirection::RelayToClient),
            RecordEndpoint::Relay => (WireDirection::RelayToClient, WireDirection::ClientToRelay),
        };
        let send_key = self.derive_key(context, send_direction)?;
        let receive_key = self.derive_key(context, receive_direction)?;
        Ok(DuplexRecordLayer {
            sealer: RecordSealer::new(&send_key),
            opener: RecordOpener::new(&receive_key),
        })
    }
    fn derive_key(
        &self,
        context: RecordStreamContext,
        direction: WireDirection,
    ) -> Result<Zeroizing<[u8; SESSION_KEY_LEN]>, RecordError> {
        let hkdf = Hkdf::<Sha256>::new(Some(KDF_SALT), self.session_key.as_ref());
        let context = context.encoded();
        let mut info = [0_u8; KDF_INFO.len() + 1 + 10];
        info[..KDF_INFO.len()].copy_from_slice(KDF_INFO);
        info[KDF_INFO.len()] = direction as u8;
        info[KDF_INFO.len() + 1..].copy_from_slice(&context);
        let mut key = Zeroizing::new([0_u8; SESSION_KEY_LEN]);
        hkdf.expand(&info, key.as_mut())
            .map_err(|_| RecordError::KeyDerivation)?;
        Ok(key)
    }
}
/// Sending and receiving state for one protected QUIC stream.
pub struct DuplexRecordLayer {
    /// State for records sent by the local endpoint.
    pub sealer: RecordSealer,
    /// State for records received by the local endpoint.
    pub opener: RecordOpener,
}
/// Stateful authenticated-record encoder.
pub struct RecordSealer {
    cipher: ChaCha20Poly1305,
    next_sequence: u64,
}
impl fmt::Debug for RecordSealer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecordSealer")
            .field("next_sequence", &self.next_sequence)
            .finish_non_exhaustive()
    }
}
impl RecordSealer {
    fn new(key: &Zeroizing<[u8; SESSION_KEY_LEN]>) -> Self {
        let cipher = ChaCha20Poly1305::new_from_slice(key.as_ref())
            .expect("record key has the algorithm's fixed key length");
        Self {
            cipher,
            next_sequence: 0,
        }
    }
    /// Seal one plaintext into a complete `header || ciphertext || tag` record.
    ///
    /// # Errors
    ///
    /// Rejects oversized plaintext, sequence exhaustion, or cipher failure.
    pub fn seal(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, RecordError> {
        let mut output = Vec::new();
        self.seal_into(plaintext, &mut output)?;
        Ok(output)
    }
    /// Seal one plaintext while reusing the caller-provided output allocation.
    ///
    /// # Errors
    ///
    /// Rejects oversized plaintext, sequence exhaustion, or cipher failure.
    pub fn seal_into(&mut self, plaintext: &[u8], output: &mut Vec<u8>) -> Result<(), RecordError> {
        output.zeroize();
        output.clear();
        if plaintext.len() > MAX_RECORD_PLAINTEXT_LEN {
            return Err(RecordError::PlaintextTooLarge {
                actual: plaintext.len(),
                maximum: MAX_RECORD_PLAINTEXT_LEN,
            });
        }
        if self.next_sequence == u64::MAX {
            return Err(RecordError::SequenceExhausted);
        }
        let header = encode_header(self.next_sequence, plaintext.len());
        output.reserve(RECORD_HEADER_LEN + plaintext.len() + RECORD_TAG_LEN);
        output.extend_from_slice(plaintext);
        let nonce = nonce_for_sequence(self.next_sequence);
        if self
            .cipher
            .encrypt_in_place(&nonce, &header, output)
            .is_err()
        {
            output.zeroize();
            output.clear();
            return Err(RecordError::Encryption);
        }
        let ciphertext_len = output.len();
        output.reserve(RECORD_HEADER_LEN);
        output.resize(ciphertext_len + RECORD_HEADER_LEN, 0);
        output.copy_within(0..ciphertext_len, RECORD_HEADER_LEN);
        output[..RECORD_HEADER_LEN].copy_from_slice(&header);
        self.next_sequence += 1;
        Ok(())
    }
}
/// Stateful authenticated-record decoder.
pub struct RecordOpener {
    cipher: ChaCha20Poly1305,
    next_sequence: u64,
}
impl fmt::Debug for RecordOpener {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecordOpener")
            .field("next_sequence", &self.next_sequence)
            .finish_non_exhaustive()
    }
}
impl RecordOpener {
    fn new(key: &Zeroizing<[u8; SESSION_KEY_LEN]>) -> Self {
        let cipher = ChaCha20Poly1305::new_from_slice(key.as_ref())
            .expect("record key has the algorithm's fixed key length");
        Self {
            cipher,
            next_sequence: 0,
        }
    }
    /// Validate a header and return the ciphertext-and-tag length to read.
    ///
    /// This does not advance the expected sequence; only successful
    /// authentication advances record state.
    ///
    /// # Errors
    ///
    /// Rejects an invalid version, unexpected sequence, exhausted sequence
    /// space, or an oversized advertised plaintext.
    pub fn ciphertext_len(&self, header: &[u8; RECORD_HEADER_LEN]) -> Result<usize, RecordError> {
        let (_, plaintext_len) = self.parse_header(header)?;
        Ok(plaintext_len + RECORD_TAG_LEN)
    }
    /// Authenticate and open a complete record.
    ///
    /// # Errors
    ///
    /// Rejects malformed, out-of-order, oversized, or unauthenticated records.
    pub fn open(&mut self, record: &[u8]) -> Result<Vec<u8>, RecordError> {
        if record.len() < RECORD_HEADER_LEN {
            return Err(RecordError::Truncated {
                expected: RECORD_HEADER_LEN,
                actual: record.len(),
            });
        }
        let (header, ciphertext) = record.split_at(RECORD_HEADER_LEN);
        let header: &[u8; RECORD_HEADER_LEN] = header
            .try_into()
            .expect("split produced a fixed-size header");
        let mut output = Vec::new();
        self.open_parts_into(header, ciphertext, &mut output)?;
        Ok(output)
    }
    /// Authenticate a header and body while reusing the output allocation.
    ///
    /// # Errors
    ///
    /// Rejects malformed, out-of-order, oversized, or unauthenticated records.
    pub fn open_parts_into(
        &mut self,
        header: &[u8; RECORD_HEADER_LEN],
        ciphertext: &[u8],
        output: &mut Vec<u8>,
    ) -> Result<(), RecordError> {
        output.zeroize();
        output.clear();
        let (sequence, plaintext_len) = self.parse_header(header)?;
        let expected_len = plaintext_len + RECORD_TAG_LEN;
        if ciphertext.len() != expected_len {
            return Err(RecordError::LengthMismatch {
                expected: expected_len,
                actual: ciphertext.len(),
            });
        }
        output.extend_from_slice(ciphertext);
        let nonce = nonce_for_sequence(sequence);
        if self
            .cipher
            .decrypt_in_place(&nonce, header, output)
            .is_err()
        {
            output.zeroize();
            output.clear();
            return Err(RecordError::Authentication);
        }
        if output.len() != plaintext_len {
            let actual = output.len();
            output.zeroize();
            output.clear();
            return Err(RecordError::LengthMismatch {
                expected: plaintext_len,
                actual,
            });
        }
        self.next_sequence += 1;
        Ok(())
    }
    fn parse_header(&self, header: &[u8; RECORD_HEADER_LEN]) -> Result<(u64, usize), RecordError> {
        if header[..RECORD_MAGIC.len()] != RECORD_MAGIC {
            return Err(RecordError::InvalidMagic);
        }
        let sequence = u64::from_be_bytes(
            header[4..12]
                .try_into()
                .expect("record sequence has a fixed-width field"),
        );
        if sequence == u64::MAX {
            return Err(RecordError::SequenceExhausted);
        }
        if sequence != self.next_sequence {
            return Err(RecordError::SequenceMismatch {
                expected: self.next_sequence,
                actual: sequence,
            });
        }
        let plaintext_len = usize::try_from(u32::from_be_bytes(
            header[12..16]
                .try_into()
                .expect("record length has a fixed-width field"),
        ))
        .expect("u32 record length is representable as usize");
        if plaintext_len > MAX_RECORD_PLAINTEXT_LEN {
            return Err(RecordError::PlaintextTooLarge {
                actual: plaintext_len,
                maximum: MAX_RECORD_PLAINTEXT_LEN,
            });
        }
        Ok((sequence, plaintext_len))
    }
}
fn encode_header(sequence: u64, plaintext_len: usize) -> [u8; RECORD_HEADER_LEN] {
    let plaintext_len =
        u32::try_from(plaintext_len).expect("bounded record plaintext length fits in u32");
    let mut header = [0_u8; RECORD_HEADER_LEN];
    header[..4].copy_from_slice(&RECORD_MAGIC);
    header[4..12].copy_from_slice(&sequence.to_be_bytes());
    header[12..16].copy_from_slice(&plaintext_len.to_be_bytes());
    header
}
fn nonce_for_sequence(sequence: u64) -> aead::Nonce<ChaCha20Poly1305> {
    let mut bytes = [0_u8; NONCE_LEN];
    bytes[4..].copy_from_slice(&sequence.to_be_bytes());
    aead::Nonce::<ChaCha20Poly1305>::try_from(bytes.as_slice())
        .expect("record nonce has the algorithm's fixed nonce length")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn layers() -> (RecordLayer, RecordLayer) {
        let key = SessionKey::new((0_u8..32).collect());
        (
            RecordLayer::new(&key, RecordEndpoint::Client).expect("client layer"),
            RecordLayer::new(&key, RecordEndpoint::Relay).expect("relay layer"),
        )
    }
    #[test]
    fn client_and_relay_records_roundtrip_in_both_directions() {
        let (client, relay) = layers();
        let context =
            RecordStreamContext::new(RecordEndpoint::Client, RecordStreamKind::Bidirectional, 7);
        let mut client = client.stream(context).expect("client stream");
        let mut relay = relay.stream(context).expect("relay stream");
        let request = client.sealer.seal(b"request").expect("seal request");
        assert_eq!(
            relay.opener.open(&request).expect("open request"),
            b"request"
        );
        let response = relay.sealer.seal(b"response").expect("seal response");
        assert_eq!(
            client.opener.open(&response).expect("open response"),
            b"response"
        );
    }
    #[test]
    fn first_release_record_vector_is_stable() {
        let (client, _) = layers();
        let context =
            RecordStreamContext::new(RecordEndpoint::Client, RecordStreamKind::Bidirectional, 7);
        let mut stream = client.stream(context).expect("client stream");
        let record = stream.sealer.seal(b"request").expect("seal");
        assert_eq!(
            hex::encode(record),
            concat!(
                "534e5231000000000000000000000007",
                "d463dd57a2f870",
                "ad365ed5b833a40a8b707d999a06f442"
            )
        );
    }
    #[test]
    fn stream_and_direction_keys_are_separated() {
        let (client, relay) = layers();
        let first =
            RecordStreamContext::new(RecordEndpoint::Client, RecordStreamKind::Bidirectional, 1);
        let second =
            RecordStreamContext::new(RecordEndpoint::Client, RecordStreamKind::Bidirectional, 2);
        let mut client_first = client.stream(first).expect("client first");
        let mut relay_first = relay.stream(first).expect("relay first");
        let mut relay_second = relay.stream(second).expect("relay second");
        let record = client_first.sealer.seal(b"bound").expect("seal");
        assert!(matches!(
            relay_second.opener.open(&record),
            Err(RecordError::Authentication)
        ));
        let reflection = relay_first
            .sealer
            .seal(b"reflection")
            .expect("seal reflection");
        assert!(matches!(
            relay_first.opener.open(&reflection),
            Err(RecordError::Authentication)
        ));
    }
    #[test]
    fn tampering_and_replay_fail_closed_without_advancing_state() {
        let (client, relay) = layers();
        let context =
            RecordStreamContext::new(RecordEndpoint::Client, RecordStreamKind::Unidirectional, 3);
        let mut client = client.stream(context).expect("client stream");
        let mut relay = relay.stream(context).expect("relay stream");
        let record = client.sealer.seal(b"authenticated").expect("seal");
        let mut tampered = record.clone();
        *tampered.last_mut().expect("tag byte") ^= 1;
        assert!(matches!(
            relay.opener.open(&tampered),
            Err(RecordError::Authentication)
        ));
        assert_eq!(
            relay.opener.open(&record).expect("valid retry"),
            b"authenticated"
        );
        assert!(matches!(
            relay.opener.open(&record),
            Err(RecordError::SequenceMismatch {
                expected: 1,
                actual: 0
            })
        ));
    }
    #[test]
    fn malformed_lengths_are_rejected_before_allocation_or_decryption() {
        let (client, relay) = layers();
        let context =
            RecordStreamContext::new(RecordEndpoint::Client, RecordStreamKind::Bidirectional, 0);
        let mut client = client.stream(context).expect("client stream");
        let relay = relay.stream(context).expect("relay stream");
        let mut record = client.sealer.seal(b"small").expect("seal");
        let oversized = u32::try_from(MAX_RECORD_PLAINTEXT_LEN).expect("protocol maximum") + 1;
        record[12..16].copy_from_slice(&oversized.to_be_bytes());
        let header: &[u8; RECORD_HEADER_LEN] =
            record[..RECORD_HEADER_LEN].try_into().expect("header");
        assert!(matches!(
            relay.opener.ciphertext_len(header),
            Err(RecordError::PlaintextTooLarge { .. })
        ));
    }
    #[test]
    fn invalid_session_key_length_is_rejected() {
        let key = SessionKey::new(vec![0xAA; 31]);
        assert!(matches!(
            RecordLayer::new(&key, RecordEndpoint::Client),
            Err(RecordError::InvalidSessionKeyLength { actual: 31 })
        ));
    }
}
