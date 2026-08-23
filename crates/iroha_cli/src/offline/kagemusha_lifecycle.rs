//! Exact canonical Kagemusha release-lifecycle transaction inputs.

use crate::{Run, RunContext};
use clap::Args;
use eyre::{Result, WrapErr as _, bail, eyre};
use iroha::data_model::{
    isi::{
        InstructionBox,
        offline::{
            CancelKagemushaRecursiveReleaseV4, DeactivateKagemushaRecursiveIssuanceV4,
            EnableKagemushaRecursiveIssuanceV4,
        },
    },
    offline::{
        KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_MAX_BYTES_V1,
        KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1, KagemushaV4IssuanceEnableWitnessV1,
        KagemushaV4ReleaseCancellationV1, KagemushaV4ReleaseDeactivationV1,
    },
};
use std::{
    fs::{self, File, Metadata},
    io::Read as _,
    path::{Path, PathBuf},
};

/// Submit one exact staged-to-enabled issuance transition.
#[derive(Args, Debug, Clone)]
pub(super) struct EnableIssuanceV4Args {
    /// Exact canonical bounded staged-to-enabled witness.
    #[arg(long, value_name = "PATH")]
    enable_witness: PathBuf,
}

/// Submit one exact staged-release cancellation transition.
#[derive(Args, Debug, Clone)]
pub(super) struct CancelReleaseV4Args {
    /// Exact canonical predecessor-bound staged-release cancellation.
    #[arg(long, value_name = "PATH")]
    cancellation: PathBuf,
}

/// Submit one exact enabled-issuance deactivation transition.
#[derive(Args, Debug, Clone)]
pub(super) struct DeactivateIssuanceV4Args {
    /// Exact canonical predecessor-bound enabled-issuance deactivation.
    #[arg(long, value_name = "PATH")]
    deactivation: PathBuf,
}

impl Run for EnableIssuanceV4Args {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = read_bounded_stable(
            &self.enable_witness,
            KAGEMUSHA_V4_ISSUANCE_ENABLE_WITNESS_MAX_BYTES_V1,
            "Kagemusha V4 issuance-enable witness",
        )?;
        let witness = KagemushaV4IssuanceEnableWitnessV1::decode_canonical(&bytes)
            .map_err(|error| eyre!(error))?;
        context.finish([InstructionBox::from(
            EnableKagemushaRecursiveIssuanceV4::new(witness),
        )])
    }
}

impl Run for CancelReleaseV4Args {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = read_bounded_stable(
            &self.cancellation,
            KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1,
            "Kagemusha V4 staged-release cancellation",
        )?;
        let cancellation = KagemushaV4ReleaseCancellationV1::decode_canonical(&bytes)
            .map_err(|error| eyre!(error))?;
        context.finish([InstructionBox::from(
            CancelKagemushaRecursiveReleaseV4::new(cancellation),
        )])
    }
}

impl Run for DeactivateIssuanceV4Args {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let bytes = read_bounded_stable(
            &self.deactivation,
            KAGEMUSHA_V4_RELEASE_TRANSITION_MAX_BYTES_V1,
            "Kagemusha V4 issuance deactivation",
        )?;
        let deactivation = KagemushaV4ReleaseDeactivationV1::decode_canonical(&bytes)
            .map_err(|error| eyre!(error))?;
        context.finish([InstructionBox::from(
            DeactivateKagemushaRecursiveIssuanceV4::new(deactivation),
        )])
    }
}

fn read_bounded_stable(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path).wrap_err_with(|| format!("inspect {label}"))?;
    if before.file_type().is_symlink() || !before.is_file() {
        bail!("{label} must be a non-symlink regular file");
    }
    let length = usize::try_from(before.len()).map_err(|_| eyre!("{label} is too large"))?;
    if length == 0 || length > maximum {
        bail!("{label} must be 1..={maximum} bytes");
    }
    let mut file = File::open(path).wrap_err_with(|| format!("open {label}"))?;
    let opened = file
        .metadata()
        .wrap_err_with(|| format!("inspect open {label}"))?;
    if !same_file_snapshot(&before, &opened) {
        bail!("{label} changed while it was opened");
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .wrap_err_with(|| format!("reserve {label} buffer"))?;
    file.by_ref()
        .take(u64::try_from(maximum)?.saturating_add(1))
        .read_to_end(&mut bytes)
        .wrap_err_with(|| format!("read {label}"))?;
    let after = file
        .metadata()
        .wrap_err_with(|| format!("reinspect open {label}"))?;
    let named_after = fs::symlink_metadata(path).wrap_err_with(|| format!("reinspect {label}"))?;
    if bytes.len() != length
        || bytes.len() > maximum
        || !same_file_snapshot(&opened, &after)
        || !same_file_snapshot(&after, &named_after)
    {
        bail!("{label} changed during bounded read");
    }
    Ok(bytes)
}

#[cfg(unix)]
fn same_file_snapshot(left: &Metadata, right: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn same_file_snapshot(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::data_model::offline::{
        KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1, KAGEMUSHA_V4_RELEASE_DEACTIVATION_SCHEMA_V1,
        KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1, KagemushaExactBytesDigestV1,
        KagemushaV4ReleaseLifecycleReasonV1,
    };
    use std::io::Write as _;

    fn exact(byte: u8) -> KagemushaExactBytesDigestV1 {
        KagemushaExactBytesDigestV1 {
            byte_len: 1,
            sha256: [byte; 32],
        }
    }

    #[test]
    fn bounded_lifecycle_input_reader_rejects_empty_and_oversized_files() {
        let empty = tempfile::NamedTempFile::new().expect("empty lifecycle input");
        assert!(read_bounded_stable(empty.path(), 1, "test lifecycle input").is_err());

        let mut oversized = tempfile::NamedTempFile::new().expect("oversized lifecycle input");
        oversized.write_all(&[1, 2]).expect("write oversized input");
        assert!(read_bounded_stable(oversized.path(), 1, "test lifecycle input").is_err());
        assert_eq!(
            read_bounded_stable(oversized.path(), 2, "test lifecycle input")
                .expect("read exact-bound lifecycle input"),
            vec![1, 2]
        );
    }

    #[test]
    fn terminal_lifecycle_inputs_decode_to_exact_typed_instructions() {
        let cancellation = KagemushaV4ReleaseCancellationV1 {
            schema: KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
            promotion_id: [1; 32],
            manifest_sha256: [2; 32],
            expected_predecessor_lifecycle: exact(3),
            transition_id: [4; 32],
            reason: KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled,
            evidence: None,
        };
        let cancellation_bytes =
            norito::encode_canonical(&cancellation).expect("encode canonical cancellation input");
        assert_eq!(
            KagemushaV4ReleaseCancellationV1::decode_canonical(&cancellation_bytes)
                .expect("decode canonical cancellation input"),
            cancellation
        );

        let deactivation = KagemushaV4ReleaseDeactivationV1 {
            schema: KAGEMUSHA_V4_RELEASE_DEACTIVATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
            promotion_id: [1; 32],
            manifest_sha256: [2; 32],
            expected_predecessor_lifecycle: exact(5),
            transition_id: [6; 32],
            reason: KagemushaV4ReleaseLifecycleReasonV1::EmergencyDeactivation,
            evidence: None,
        };
        let deactivation_bytes =
            norito::encode_canonical(&deactivation).expect("encode canonical deactivation input");
        assert_eq!(
            KagemushaV4ReleaseDeactivationV1::decode_canonical(&deactivation_bytes)
                .expect("decode canonical deactivation input"),
            deactivation
        );
    }
}
