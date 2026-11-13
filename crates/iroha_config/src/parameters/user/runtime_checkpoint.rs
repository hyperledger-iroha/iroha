//! File-configured immutable checkpoint serving with finite node-local resource limits.

use super::{ParseError, actual, defaults, emit_torii_config_error};
use iroha_config_base::{
    ReadConfig, WithOrigin,
    util::{DurationMs, Emitter},
};
use std::{
    num::{NonZeroU64, NonZeroUsize},
    path::PathBuf,
};

/// The node operator selects one already-exported package; requests cannot select paths.
#[derive(Debug, ReadConfig)]
pub struct ToriiRuntimeCheckpoint {
    /// Completed private export directory; no source is configured by default.
    pub package_dir: Option<WithOrigin<PathBuf>>,
    /// Maximum number of artifacts accepted from that source.
    #[config(default = "defaults::torii::runtime_checkpoint::MAX_ARTIFACTS")]
    pub max_artifacts: NonZeroUsize,
    /// Maximum total package bytes.
    #[config(default = "defaults::torii::runtime_checkpoint::MAX_TOTAL_BYTES")]
    pub max_total_bytes: NonZeroU64,
    /// Maximum single artifact bytes.
    #[config(default = "defaults::torii::runtime_checkpoint::MAX_ARTIFACT_BYTES")]
    pub max_artifact_bytes: NonZeroU64,
    /// Maximum response chunk, bounded by the fixed 4 MiB transport ceiling.
    #[config(default = "defaults::torii::runtime_checkpoint::CHUNK_BYTES")]
    pub chunk_bytes: NonZeroUsize,
    /// Original source inventory and working-buffer memory pool.
    #[config(default = "defaults::torii::runtime_checkpoint::SOURCE_MEMORY_BYTES")]
    pub source_memory_bytes: NonZeroUsize,
    /// Maximum duration of one remote request in milliseconds.
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::torii::runtime_checkpoint::REQUEST_TIMEOUT_MS))"
    )]
    pub request_timeout_ms: DurationMs,
    /// Absolute maximum duration of complete checkpoint reception in milliseconds.
    #[config(
        default = "DurationMs(std::time::Duration::from_millis(defaults::torii::runtime_checkpoint::TRANSFER_TIMEOUT_MS))"
    )]
    pub transfer_timeout_ms: DurationMs,
}

impl ToriiRuntimeCheckpoint {
    pub(super) fn parse(self, emitter: &mut Emitter<ParseError>) -> actual::ToriiRuntimeCheckpoint {
        if self.request_timeout_ms.get().is_zero()
            || self.transfer_timeout_ms.get() < self.request_timeout_ms.get()
        {
            emit_torii_config_error(
                emitter,
                "torii.runtime_checkpoint deadlines must be nonzero and transfer_timeout_ms must be at least request_timeout_ms",
            );
        }
        if std::time::Instant::now()
            .checked_add(self.transfer_timeout_ms.get())
            .is_none()
        {
            emit_torii_config_error(
                emitter,
                "torii.runtime_checkpoint.transfer_timeout_ms exceeds the local clock domain",
            );
        }
        if self
            .package_dir
            .as_ref()
            .is_some_and(|path| path.value().as_os_str().is_empty())
        {
            emit_torii_config_error(
                emitter,
                "torii.runtime_checkpoint.package_dir must not be empty",
            );
        }
        if self.max_artifact_bytes > self.max_total_bytes {
            emit_torii_config_error(
                emitter,
                "torii.runtime_checkpoint.max_artifact_bytes exceeds max_total_bytes",
            );
        }
        if self.chunk_bytes.get() > defaults::torii::runtime_checkpoint::MAX_CHUNK_BYTES {
            emit_torii_config_error(
                emitter,
                "torii.runtime_checkpoint.chunk_bytes exceeds the 4 MiB transport ceiling",
            );
        }
        if self.chunk_bytes > self.source_memory_bytes {
            emit_torii_config_error(
                emitter,
                "torii.runtime_checkpoint.source_memory_bytes cannot fund one chunk",
            );
        }
        // A checkpoint needs H>=2, five snapshot artifacts, canonical journals and its marker.
        if self.max_artifacts.get() < 18 {
            emit_torii_config_error(
                emitter,
                "torii.runtime_checkpoint.max_artifacts must accommodate at least 18 artifacts",
            );
        }
        actual::ToriiRuntimeCheckpoint {
            package_dir: self.package_dir,
            max_artifacts: self.max_artifacts,
            max_total_bytes: self.max_total_bytes,
            max_artifact_bytes: self.max_artifact_bytes,
            chunk_bytes: self.chunk_bytes,
            source_memory_bytes: self.source_memory_bytes,
            request_timeout: self.request_timeout_ms.get(),
            transfer_timeout: self.transfer_timeout_ms.get(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config_base::{read::ConfigReader, toml::TomlSource};

    fn read(text: &str) -> ToriiRuntimeCheckpoint {
        ConfigReader::new()
            .with_toml_source(TomlSource::inline(text.parse().unwrap()))
            .read_and_complete::<ToriiRuntimeCheckpoint>()
            .unwrap()
    }

    #[test]
    fn runtime_checkpoint_defaults_have_no_source_and_preserve_finite_limits() {
        let mut errors = Emitter::new();
        let actual = read("").parse(&mut errors);
        errors.into_result().unwrap();
        assert!(actual.package_dir.is_none());
        let expected = actual::ToriiRuntimeCheckpoint::default();
        assert_eq!(actual.max_artifacts, expected.max_artifacts);
        assert_eq!(actual.max_total_bytes, expected.max_total_bytes);
        assert_eq!(actual.max_artifact_bytes, expected.max_artifact_bytes);
        assert_eq!(actual.chunk_bytes, expected.chunk_bytes);
        assert_eq!(actual.source_memory_bytes, expected.source_memory_bytes);
        assert_eq!(actual.request_timeout, expected.request_timeout);
        assert_eq!(actual.transfer_timeout, expected.transfer_timeout);
    }

    #[test]
    fn runtime_checkpoint_rejects_impossible_or_unbounded_transport_settings() {
        for text in [
            "package_dir = ''",
            "max_artifacts = 17",
            "max_artifact_bytes = 100\nmax_total_bytes = 99",
            "chunk_bytes = 4194305",
            "source_memory_bytes = 1024",
            "request_timeout_ms = 0",
            "transfer_timeout_ms = 1",
        ] {
            let mut errors = Emitter::new();
            read(text).parse(&mut errors);
            assert!(errors.into_result().is_err(), "{text}");
        }
        for field in [
            "max_artifacts",
            "max_total_bytes",
            "max_artifact_bytes",
            "chunk_bytes",
            "source_memory_bytes",
        ] {
            assert!(
                ConfigReader::new()
                    .with_toml_source(TomlSource::inline(format!("{field} = 0").parse().unwrap()))
                    .read_and_complete::<ToriiRuntimeCheckpoint>()
                    .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn runtime_checkpoint_preserves_explicit_source_and_resource_policy() {
        let mut errors = Emitter::new();
        let actual = read("package_dir = '/checkpoint/export'\nmax_artifacts = 30\nmax_total_bytes = 8192\nmax_artifact_bytes = 4096\nchunk_bytes = 2048\nsource_memory_bytes = 1048576").parse(&mut errors);
        errors.into_result().unwrap();
        assert_eq!(
            actual.package_dir.unwrap().resolve_relative_path(),
            PathBuf::from("/checkpoint/export")
        );
        assert_eq!(actual.max_artifacts.get(), 30);
        assert_eq!(actual.max_total_bytes.get(), 8192);
        assert_eq!(actual.max_artifact_bytes.get(), 4096);
        assert_eq!(actual.chunk_bytes.get(), 2048);
        assert_eq!(actual.source_memory_bytes.get(), 1048576);
    }
}
