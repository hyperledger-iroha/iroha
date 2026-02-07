//! Streaming verifier for CARv2 archives.
//!
//! This module provides a state-machine based verifier that can process CAR bytes incrementally.
//! It validates the CAR header, section headers, and payload chunks against a trusted root CID/Manifest.
//! It is designed to be used in network streams where backpressure is required.

use std::convert::TryInto;

use blake3::Hasher;
use sorafs_manifest::ManifestV1;

use crate::{
    BLAKE3_256_MULTIHASH_CODE, DAG_CBOR_CODEC, HEADER_LEN, PRAGMA, RAW_CODEC,
    verifier::CarVerifyError,
};

/// Configuration for the streaming verifier.
#[derive(Debug, Clone)]
pub struct StreamingVerifierConfig {
    /// Maximum allowed size for a single chunk.
    pub max_chunk_size: usize,
}

impl Default for StreamingVerifierConfig {
    fn default() -> Self {
        Self {
            max_chunk_size: 1024 * 1024 * 4, // 4 MiB default limit
        }
    }
}

/// State of the streaming verifier.
#[derive(Debug)]
enum State {
    /// Waiting for the PRAGMA bytes.
    Pragma { buffered: usize },
    /// Waiting for the CARv2 Characteristics (16 bytes + offsets).
    Characteristics { buffered: usize },
    /// Waiting for the CARv1 header varint length.
    HeaderLen { buffered: usize },
    /// Waiting for the CARv1 header body.
    HeaderBody { len: usize, buffered: usize },
    /// Processing chunk sections (varint length -> CID -> Payload).
    SectionLen { buffered: usize },
    /// Waiting for CID.
    SectionCid { len: usize, buffered: usize },
    /// Waiting for Payload.
    SectionPayload {
        len: usize,
        cid_len: usize,
        cid_digest: [u8; 32],
        is_raw_chunk: bool,
        buffered: usize,
    },
    /// Verification complete.
    Complete,
    /// Error state.
    Error(CarVerifyError),
}

/// A streaming verifier for CARv2 archives.
pub struct StreamingCarVerifier {
    config: StreamingVerifierConfig,
    manifest: ManifestV1,
    state: State,
    buffer: Vec<u8>,
    /// Total bytes processed so far.
    processed_bytes: u64,
    /// Hasher for the entire CAR archive (integrity check).
    archive_hasher: Hasher,
    /// Hasher for the payload (content check).
    payload_hasher: Hasher,
    /// Offsets extracted from Characteristics.
    data_offset: u64,
    data_size: u64,
    index_offset: u64,
    /// Current cursor position relative to the start of the current state's data.
    #[allow(dead_code)] // Used in future optimizations
    cursor: usize,
    /// Track current section index for error reporting.
    section_index: usize,
    /// Track current chunk index (raw sections only) for chunk-specific errors.
    chunk_index: usize,
    /// Bytes read within the CARv1 data region.
    current_data_read: u64,
    /// Total payload bytes consumed across raw chunks.
    payload_bytes: u64,
    /// Whether roots have been validated against the manifest.
    roots_validated: bool,
}

impl StreamingCarVerifier {
    /// Create a new streaming verifier for the given manifest.
    pub fn new(manifest: ManifestV1, config: StreamingVerifierConfig) -> Self {
        Self {
            config,
            manifest, // Stored for future manifest checks (e.g. root CID)
            state: State::Pragma { buffered: 0 },
            buffer: Vec::with_capacity(1024), // Initial buffer
            processed_bytes: 0,
            archive_hasher: Hasher::new(),
            payload_hasher: Hasher::new(),
            data_offset: 0,
            data_size: 0,
            index_offset: 0,
            cursor: 0,
            section_index: 0,
            chunk_index: 0,
            current_data_read: 0,
            payload_bytes: 0,
            roots_validated: false,
        }
    }

    /// Process a chunk of bytes.
    /// Returns the number of bytes consumed. If less than `bytes.len()`, the caller should
    /// handle backpressure or store the remainder.
    ///
    /// If an error occurs, the verifier enters the `Error` state and returns `Err`.
    pub fn update(&mut self, bytes: &[u8]) -> Result<usize, CarVerifyError> {
        if let State::Error(_e) = &self.state {
            return Err(CarVerifyError::InternalInvariant(
                "verifier is in error state",
            ));
        }
        if let State::Complete = &self.state {
            // Consume remainder without error if complete
            self.archive_hasher.update(bytes);
            self.processed_bytes = self.processed_bytes.saturating_add(bytes.len() as u64);
            return Ok(bytes.len());
        }

        let mut consumed = 0;
        while consumed < bytes.len() {
            // If complete, drain remaining without hashing (or hash if checking archive digest,
            // but here we assume payload focus). Archive digest checks would need to process everything.
            if matches!(self.state, State::Complete) {
                self.archive_hasher.update(&bytes[consumed..]);
                self.processed_bytes = self.processed_bytes.saturating_add(bytes.len() as u64);
                return Ok(bytes.len());
            }

            // The CARv2 index lives after the CARv1 data region. Once we've consumed the full
            // data region, we stop parsing sections and switch to `State::Complete` so the
            // remaining bytes are simply hashed and consumed.
            //
            // This must happen before the byte-by-byte hashing below; otherwise we'd hash the
            // first index byte twice (once here, once in the `State::Complete` fast-path),
            // corrupting the archive digest.
            if matches!(self.state, State::SectionLen { buffered: 0 })
                && self.current_data_read == self.data_size
            {
                self.transition(State::Complete);
                continue;
            }

            let b = bytes[consumed];
            // We hash byte by byte for simplicity in this loop structure,
            // optimization would block-hash.
            self.archive_hasher.update(&[b]);

            match &mut self.state {
                State::Pragma { buffered } => {
                    self.buffer.push(b);
                    *buffered += 1;
                    consumed += 1;

                    if *buffered == PRAGMA.len() {
                        if self.buffer != PRAGMA {
                            self.transition(State::Error(CarVerifyError::InvalidPragma));
                            return Err(CarVerifyError::InvalidPragma);
                        }
                        self.buffer.clear();
                        self.transition(State::Characteristics { buffered: 0 });
                    }
                }
                State::Characteristics { buffered } => {
                    self.buffer.push(b);
                    *buffered += 1;
                    consumed += 1;

                    if *buffered == HEADER_LEN {
                        // Parse offsets
                        let data_offset_bytes: [u8; 8] = self.buffer[16..24].try_into().unwrap();
                        let data_size_bytes: [u8; 8] = self.buffer[24..32].try_into().unwrap();
                        let index_offset_bytes: [u8; 8] = self.buffer[32..40].try_into().unwrap();

                        self.data_offset = u64::from_le_bytes(data_offset_bytes);
                        self.data_size = u64::from_le_bytes(data_size_bytes);
                        self.index_offset = u64::from_le_bytes(index_offset_bytes);

                        // Validation
                        let header_start = (PRAGMA.len() + HEADER_LEN) as u64;
                        if self.data_offset != header_start {
                            self.transition(State::Error(CarVerifyError::InvalidHeader));
                            return Err(CarVerifyError::InvalidHeader);
                        }
                        let data_end = self
                            .data_offset
                            .checked_add(self.data_size)
                            .ok_or(CarVerifyError::InvalidHeader)?;
                        if self.index_offset != data_end {
                            self.transition(State::Error(CarVerifyError::InvalidIndexOffset));
                            return Err(CarVerifyError::InvalidIndexOffset);
                        }

                        self.buffer.clear();
                        self.transition(State::HeaderLen { buffered: 0 });
                    }
                }
                State::HeaderLen { buffered } => {
                    self.buffer.push(b);
                    *buffered += 1;
                    consumed += 1;
                    self.current_data_read += 1;

                    if let Ok((len, _)) = crate::verifier::decode_uleb128(&self.buffer) {
                        self.buffer.clear();
                        self.transition(State::HeaderBody {
                            len: len as usize,
                            buffered: 0,
                        });
                    } else if *buffered > 10 {
                        self.transition(State::Error(CarVerifyError::VarintOverflow));
                        return Err(CarVerifyError::VarintOverflow);
                    }
                }
                State::HeaderBody { len, buffered } => {
                    self.buffer.push(b);
                    *buffered += 1;
                    consumed += 1;
                    self.current_data_read += 1;

                    if *buffered == *len {
                        match crate::verifier::parse_carv1_header(&self.buffer) {
                            Ok(roots) => {
                                if let Err(e) = self.validate_roots(&roots) {
                                    self.transition(State::Error(e));
                                    return Err(CarVerifyError::ManifestRootMismatch);
                                }
                                self.roots_validated = true;
                            }
                            Err(e) => {
                                self.transition(State::Error(e));
                                return Err(CarVerifyError::InvalidCarv1Header(
                                    "header parse failed",
                                ));
                            }
                        }
                        self.buffer.clear();
                        self.transition(State::SectionLen { buffered: 0 });
                    }
                }
                State::SectionLen { buffered } => {
                    self.buffer.push(b);
                    *buffered += 1;
                    consumed += 1;
                    self.current_data_read += 1;

                    if let Ok((len, _)) = crate::verifier::decode_uleb128(&self.buffer) {
                        if len == 0 {
                            // Zero length section - usually padding or error?
                            // Treat as empty section, go back to len.
                            self.buffer.clear();
                            self.transition(State::SectionLen { buffered: 0 });
                        } else {
                            self.buffer.clear();
                            self.transition(State::SectionCid {
                                len: len as usize,
                                buffered: 0,
                            });
                        }
                    } else if *buffered > 10 {
                        self.transition(State::Error(CarVerifyError::VarintOverflow));
                        return Err(CarVerifyError::VarintOverflow);
                    }
                }
                State::SectionCid { len, buffered } => {
                    self.buffer.push(b);
                    *buffered += 1;
                    consumed += 1;
                    self.current_data_read += 1;

                    match crate::verifier::decode_cid(&self.buffer, self.section_index) {
                        Ok((info, cid_len)) => {
                            if info.digest.len() != 32 {
                                self.transition(State::Error(
                                    CarVerifyError::UnsupportedDigestLength {
                                        section_index: self.section_index,
                                        length: info.digest.len() as u64,
                                    },
                                ));
                                return Err(CarVerifyError::UnsupportedDigestLength {
                                    section_index: self.section_index,
                                    length: info.digest.len() as u64,
                                });
                            }
                            if info.multihash != BLAKE3_256_MULTIHASH_CODE {
                                self.transition(State::Error(
                                    CarVerifyError::UnsupportedMultihash {
                                        section_index: self.section_index,
                                        code: info.multihash,
                                    },
                                ));
                                return Err(CarVerifyError::UnsupportedMultihash {
                                    section_index: self.section_index,
                                    code: info.multihash,
                                });
                            }
                            if info.codec != RAW_CODEC && info.codec != DAG_CBOR_CODEC {
                                self.transition(State::Error(
                                    CarVerifyError::UnsupportedSectionCodec {
                                        section_index: self.section_index,
                                        codec: info.codec,
                                    },
                                ));
                                return Err(CarVerifyError::UnsupportedSectionCodec {
                                    section_index: self.section_index,
                                    codec: info.codec,
                                });
                            }

                            let digest = info.digest;
                            let payload_len = *len - cid_len;
                            if info.codec == RAW_CODEC && payload_len > self.config.max_chunk_size {
                                let section_index = self.section_index;
                                let len = payload_len as u64;
                                let max = self.config.max_chunk_size as u64;
                                self.transition(State::Error(CarVerifyError::ChunkSizeExceeded {
                                    section_index,
                                    len,
                                    max,
                                }));
                                return Err(CarVerifyError::ChunkSizeExceeded {
                                    section_index,
                                    len,
                                    max,
                                });
                            }

                            self.buffer.clear();
                            self.transition(State::SectionPayload {
                                len: payload_len,
                                cid_len,
                                cid_digest: digest,
                                is_raw_chunk: info.codec == RAW_CODEC,
                                buffered: 0,
                            });
                        }
                        Err(_) if *buffered >= *len => {
                            self.transition(State::Error(CarVerifyError::TruncatedCid {
                                section_index: self.section_index,
                            }));
                            return Err(CarVerifyError::TruncatedCid {
                                section_index: self.section_index,
                            });
                        }
                        Err(_) => {}
                    }
                }
                State::SectionPayload {
                    len,
                    cid_len: _cid_len,
                    cid_digest,
                    is_raw_chunk,
                    buffered,
                } => {
                    self.buffer.push(b);
                    if *is_raw_chunk {
                        // Only raw chunks contribute to payload digest/length.
                        self.payload_hasher.update(&[b]);
                        self.payload_bytes = self.payload_bytes.saturating_add(1);
                    }

                    *buffered += 1;
                    consumed += 1;
                    self.current_data_read += 1;

                    if *buffered == *len {
                        let hash = blake3::hash(&self.buffer);
                        if hash.as_bytes() != cid_digest {
                            if *is_raw_chunk {
                                let err = CarVerifyError::ChunkDigestMismatch {
                                    chunk_index: self.chunk_index,
                                };
                                self.transition(State::Error(err));
                                return Err(CarVerifyError::ChunkDigestMismatch {
                                    chunk_index: self.chunk_index,
                                });
                            } else {
                                let err = CarVerifyError::NodeDigestMismatch {
                                    section_index: self.section_index,
                                };
                                self.transition(State::Error(err));
                                return Err(CarVerifyError::NodeDigestMismatch {
                                    section_index: self.section_index,
                                });
                            }
                        }

                        if *is_raw_chunk {
                            if self.payload_bytes > self.manifest.content_length {
                                let err = CarVerifyError::ManifestContentLengthMismatch {
                                    expected: self.manifest.content_length,
                                    actual: self.payload_bytes,
                                };
                                self.transition(State::Error(err));
                                return Err(CarVerifyError::ManifestContentLengthMismatch {
                                    expected: self.manifest.content_length,
                                    actual: self.payload_bytes,
                                });
                            }
                            self.chunk_index += 1;
                        }
                        self.section_index += 1;
                        self.buffer.clear();
                        self.transition(State::SectionLen { buffered: 0 });
                    }
                }
                State::Complete => {
                    // Unreachable because we handle Complete check at top of loop
                    consumed += 1;
                }
                State::Error(_) => {
                    // Unreachable because we check Error at entry
                    consumed += 1;
                }
            }
        }

        self.processed_bytes += consumed as u64;
        Ok(consumed)
    }

    /// Finalize verification.
    pub fn finalize(self) -> Result<(), CarVerifyError> {
        if let State::Error(e) = self.state {
            return Err(e);
        }
        // Check if we are in a clean state
        match self.state {
            State::SectionLen { buffered: 0 } => {
                if self.current_data_read >= self.data_size {
                    // ok
                } else {
                    return Err(CarVerifyError::Truncated);
                }
            }
            State::Complete => {}
            _ => return Err(CarVerifyError::Truncated),
        }

        if !self.roots_validated {
            return Err(CarVerifyError::InvalidCarv1Header("missing roots"));
        }

        if self.manifest.chunking.multihash_code != BLAKE3_256_MULTIHASH_CODE {
            return Err(CarVerifyError::ManifestMultihashMismatch(
                self.manifest.chunking.multihash_code,
            ));
        }

        if self.payload_bytes != self.manifest.content_length {
            return Err(CarVerifyError::ManifestContentLengthMismatch {
                expected: self.manifest.content_length,
                actual: self.payload_bytes,
            });
        }

        if self.processed_bytes != self.manifest.car_size {
            return Err(CarVerifyError::ManifestCarSizeMismatch {
                expected: self.manifest.car_size,
                actual: self.processed_bytes,
            });
        }

        let archive_digest = self.archive_hasher.finalize();
        if archive_digest.as_bytes() != &self.manifest.car_digest {
            return Err(CarVerifyError::ManifestCarDigestMismatch);
        }

        Ok(())
    }

    fn transition(&mut self, new_state: State) {
        self.state = new_state;
    }

    fn validate_roots(&self, roots: &[Vec<u8>]) -> Result<(), CarVerifyError> {
        if roots.len() != 1 || roots[0] != self.manifest.root_cid {
            return Err(CarVerifyError::ManifestRootMismatch);
        }
        Ok(())
    }
}
