//! Provider admission registry loading and verification for SoraFS adverts.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use iroha_logger::{trace, warn};
pub use sorafs_manifest::ProviderAdmissionAdvertError as AdmissionCheckError;
use sorafs_manifest::{
    AdmissionRecord, ProviderAdmissionEnvelopeError, ProviderAdmissionEnvelopeV1, ProviderAdvertV1,
    verify_advert_against_record,
};
use thiserror::Error;

/// Admission registry loaded from governance envelopes.
#[derive(Debug, Clone)]
pub struct AdmissionRegistry {
    by_provider: HashMap<[u8; 32], Arc<AdmissionRecord>>,
}

impl AdmissionRegistry {
    /// Construct an empty registry (used when admission is optional).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            by_provider: HashMap::new(),
        }
    }

    /// Construct a registry from an iterator of admission envelopes.
    ///
    /// # Errors
    ///
    /// Returns a [`SingleEnvelopeError`] when any envelope is invalid or when
    /// multiple envelopes declare the same provider identifier.
    pub fn from_envelopes<I>(envelopes: I) -> Result<Self, SingleEnvelopeError>
    where
        I: IntoIterator<Item = ProviderAdmissionEnvelopeV1>,
    {
        let mut by_provider = HashMap::new();
        for envelope in envelopes {
            let (provider_id, record) = prepare_entry(envelope)?;
            if by_provider.insert(provider_id, Arc::new(record)).is_some() {
                return Err(SingleEnvelopeError::DuplicateProvider { provider_id });
            }
        }
        Ok(Self { by_provider })
    }

    /// Populate the registry from the provided directory.
    ///
    /// # Errors
    ///
    /// Returns an [`AdmissionRegistryError`] when the directory cannot be read,
    /// an envelope fails to decode, or multiple envelopes declare the same
    /// provider identifier.
    pub fn load_from_dir(dir: &Path) -> Result<Self, AdmissionRegistryError> {
        let mut by_provider = HashMap::new();
        let read_dir = fs::read_dir(dir).map_err(|err| AdmissionRegistryError::ReadDir {
            dir: dir.into(),
            err,
        })?;

        for entry in read_dir {
            let entry = entry.map_err(|err| AdmissionRegistryError::ReadDirEntry {
                dir: dir.into(),
                err,
            })?;
            if !entry.path().is_file() {
                continue;
            }
            let path = entry.path();
            match load_single_envelope(&path) {
                Ok((provider_id, record)) => {
                    trace!(?path, "loaded provider admission envelope");
                    if by_provider.insert(provider_id, Arc::new(record)).is_some() {
                        return Err(AdmissionRegistryError::DuplicateProvider {
                            provider_id,
                            path,
                        });
                    }
                }
                Err(err) => {
                    return Err(AdmissionRegistryError::LoadEnvelope { path, source: err });
                }
            }
        }

        if by_provider.is_empty() {
            warn!(
                ?dir,
                "provider admission registry directory contains no envelopes"
            );
        }

        Ok(Self { by_provider })
    }

    /// Look up an admission entry for the given provider identifier.
    #[must_use]
    pub fn entry(&self, provider_id: &[u8; 32]) -> Option<Arc<AdmissionRecord>> {
        self.by_provider.get(provider_id).cloned()
    }
}

fn load_single_envelope(path: &Path) -> Result<([u8; 32], AdmissionRecord), SingleEnvelopeError> {
    let bytes = fs::read(path).map_err(|err| SingleEnvelopeError::Read { err })?;
    let envelope = decode_envelope(&bytes).map_err(SingleEnvelopeError::Decode)?;
    prepare_entry(envelope)
}

fn prepare_entry(
    envelope: ProviderAdmissionEnvelopeV1,
) -> Result<([u8; 32], AdmissionRecord), SingleEnvelopeError> {
    let record = AdmissionRecord::new(envelope).map_err(SingleEnvelopeError::Verify)?;
    let provider_id = *record.provider_id();
    Ok((provider_id, record))
}

/// Verify that the given advert is authorised by the provided admission entry.
///
/// # Errors
///
/// Returns [`AdmissionCheckError`] when the advert metadata does not match the
/// admission record.
pub fn verify_advert_against_envelope(
    advert: &ProviderAdvertV1,
    record: &AdmissionRecord,
) -> Result<(), AdmissionCheckError> {
    verify_advert_against_record(advert, record)
}

fn decode_envelope(bytes: &[u8]) -> Result<ProviderAdmissionEnvelopeV1, EnvelopeDecodeError> {
    norito::decode_from_bytes(bytes).map_err(EnvelopeDecodeError::Norito)
}

/// Errors raised while constructing the admission registry.
#[derive(Debug, Error)]
pub enum AdmissionRegistryError {
    /// Reading the admission directory failed.
    #[error("failed to read admission directory {dir:?}: {err}")]
    ReadDir {
        /// Directory whose contents could not be read.
        dir: PathBuf,
        /// Underlying I/O error.
        err: std::io::Error,
    },
    /// Iterating over directory entries failed.
    #[error("failed to list admission directory {dir:?}: {err}")]
    ReadDirEntry {
        /// Directory whose entries could not be enumerated.
        dir: PathBuf,
        /// Underlying I/O error.
        err: std::io::Error,
    },
    /// A second envelope for the same provider was encountered.
    #[error("duplicate provider admission envelope for {provider_id:?} at {path:?}")]
    DuplicateProvider {
        /// Identifier of the provider with duplicate envelope.
        provider_id: [u8; 32],
        /// Path to the conflicting envelope file.
        path: PathBuf,
    },
    /// Loading or verifying an individual envelope failed.
    #[error("failed to load admission envelope {path:?}: {source}")]
    LoadEnvelope {
        /// Path to the envelope being processed.
        path: PathBuf,
        /// Reason the envelope could not be loaded or verified.
        source: SingleEnvelopeError,
    },
}

/// Errors emitted while loading a single envelope.
#[derive(Debug, Error)]
pub enum SingleEnvelopeError {
    /// Failed to read the envelope file.
    #[error("failed to read envelope: {err}")]
    Read {
        /// Underlying I/O error.
        err: std::io::Error,
    },
    /// Envelope contents could not be decoded.
    #[error("failed to decode envelope: {0}")]
    Decode(EnvelopeDecodeError),
    /// Envelope verification failed.
    #[error("envelope verification failed: {0}")]
    Verify(ProviderAdmissionEnvelopeError),
    /// The envelope duplicates an existing provider identifier.
    #[error("duplicate provider identifier {provider_id:02x?} in admission registry")]
    DuplicateProvider {
        /// Conflicting provider identifier.
        provider_id: [u8; 32],
    },
}

/// Errors that occur while decoding a provider admission envelope.
#[derive(Debug, Error)]
pub enum EnvelopeDecodeError {
    /// Decoding the envelope with the Norito codec failed.
    #[error("failed to decode provider admission envelope: {0}")]
    Norito(#[from] norito::core::Error),
}
