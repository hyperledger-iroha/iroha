//! Node-local immutable checkpoint-package source and finite delivery resources.

use crate::parameters::defaults::torii::runtime_checkpoint as defaults;
use iroha_config_base::WithOrigin;
use std::{
    num::{NonZeroU64, NonZeroUsize},
    path::PathBuf,
    time::Duration,
};

/// Immutable pre-exported package selected by the node operator, never by an HTTP request.
#[derive(Debug, Clone)]
pub struct ToriiRuntimeCheckpoint {
    /// Completed package directory; absence leaves checkpoint delivery unavailable.
    pub package_dir: Option<WithOrigin<PathBuf>>,
    /// Maximum number of artifacts in the retained inventory.
    pub max_artifacts: NonZeroUsize,
    /// Maximum complete package length.
    pub max_total_bytes: NonZeroU64,
    /// Maximum single artifact length.
    pub max_artifact_bytes: NonZeroU64,
    /// Maximum chunk served by this source, within the fixed transport ceiling.
    pub chunk_bytes: NonZeroUsize,
    /// Original pool funding inventory custody and working buffers.
    pub source_memory_bytes: NonZeroUsize,
    /// Maximum duration of one remote request, bounded by the remaining transfer deadline.
    pub request_timeout: Duration,
    /// Absolute maximum duration of the complete receive operation.
    pub transfer_timeout: Duration,
}

impl Default for ToriiRuntimeCheckpoint {
    fn default() -> Self {
        Self {
            package_dir: None,
            max_artifacts: defaults::MAX_ARTIFACTS,
            max_total_bytes: defaults::MAX_TOTAL_BYTES,
            max_artifact_bytes: defaults::MAX_ARTIFACT_BYTES,
            chunk_bytes: defaults::CHUNK_BYTES,
            source_memory_bytes: defaults::SOURCE_MEMORY_BYTES,
            request_timeout: Duration::from_millis(defaults::REQUEST_TIMEOUT_MS),
            transfer_timeout: Duration::from_millis(defaults::TRANSFER_TIMEOUT_MS),
        }
    }
}
