//! Retained local-NTFS model inputs with no namespace fallback or permission repair.
//!
//! Root, nested directory and single-link regular-file owners stay alive through
//! strict decoding. Checks bracket each model; this is not an atomic tree snapshot.

use super::*;
use iroha_fs::{FileSnapshot, ReaderDirectory};

#[derive(Debug)]
pub(super) struct ValidatedArtifactRoot {
    directory: ReaderDirectory,
    snapshot: FileSnapshot,
}

fn root_error(path: &Path, error: io::Error) -> ModerationRunnerError {
    ModerationRunnerError::InvalidArtifactRoot {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}

fn input_error(path: &Path, error: io::Error) -> ModerationRunnerError {
    ModerationRunnerError::ArtifactIo {
        path: path.to_path_buf(),
        source: error,
    }
}

pub(super) fn validate_artifact_root(
    path: &Path,
) -> Result<ValidatedArtifactRoot, ModerationRunnerError> {
    // The native owner rejects unsupported namespaces before any pathname probe.
    let directory = ReaderDirectory::open(path).map_err(|error| root_error(path, error))?;
    let snapshot = directory
        .snapshot()
        .map_err(|error| root_error(path, error))?;
    Ok(ValidatedArtifactRoot {
        directory,
        snapshot,
    })
}

pub(super) fn verify_artifact_root(
    root: &ValidatedArtifactRoot,
) -> Result<(), ModerationRunnerError> {
    let path = root.directory.path();
    let current = root
        .directory
        .snapshot()
        .map_err(|error| root_error(path, error))?;
    if current != root.snapshot {
        return Err(ModerationRunnerError::InvalidArtifactRoot {
            path: path.to_path_buf(),
            reason: "root changed during artefact loading".to_owned(),
        });
    }
    root.directory
        .revalidate()
        .map_err(|error| root_error(path, error))
}

pub(super) fn load_model(
    root: &ValidatedArtifactRoot,
    fingerprint: &ModerationModelFingerprintV1,
) -> Result<ModerationModelArtifactV1, ModerationRunnerError> {
    load_model_with_post_open_hook(root, fingerprint, |_| {})
}

pub(super) fn load_model_with_post_open_hook<F>(
    root: &ValidatedArtifactRoot,
    fingerprint: &ModerationModelFingerprintV1,
    post_open_hook: F,
) -> Result<ModerationModelArtifactV1, ModerationRunnerError>
where
    F: FnOnce(&Path),
{
    if !is_canonical_moderation_artifact_path_v1(&fingerprint.artifact_path) {
        return Err(ModerationRunnerError::UnsafeArtifactPath {
            path: PathBuf::from(&fingerprint.artifact_path),
            reason: "path is not a canonical portable relative artefact path".to_owned(),
        });
    }
    if fingerprint.artifact_bytes == 0
        || fingerprint.artifact_bytes > MODERATION_MODEL_MAX_ARTIFACT_BYTES_V1
    {
        return Err(ModerationRunnerError::ArtifactDecode {
            path: PathBuf::from(&fingerprint.artifact_path),
            reason: "signed artefact length is outside the bounded model budget".to_owned(),
        });
    }
    let path = root.directory.path().join(&fingerprint.artifact_path);
    let mut directories: Vec<(ReaderDirectory, FileSnapshot)> = Vec::new();
    let mut file = None;
    let mut opened = None;
    let result = (|| {
        verify_artifact_root(root)?;
        let mut components = fingerprint.artifact_path.split('/');
        let leaf =
            components
                .next_back()
                .ok_or_else(|| ModerationRunnerError::UnsafeArtifactPath {
                    path: path.clone(),
                    reason: "artefact path has no file component".to_owned(),
                })?;
        for component in components {
            let parent = directories
                .last()
                .map_or(&root.directory, |(directory, _)| directory);
            let child = parent
                .open_child(component)
                .map_err(|error| input_error(&path, error))?;
            let snapshot = child
                .snapshot()
                .map_err(|error| input_error(child.path(), error))?;
            directories.push((child, snapshot));
        }
        let parent = directories
            .last()
            .map_or(&root.directory, |(directory, _)| directory);
        let original = file.insert(
            parent
                .open_retained_regular(leaf)
                .map_err(|error| input_error(&path, error))?,
        );
        let snapshot = original
            .snapshot()
            .map_err(|error| input_error(&path, error))?;
        opened = Some(snapshot);
        let length = snapshot.local_parts().2;
        if length != fingerprint.artifact_bytes {
            return Err(ModerationRunnerError::ArtifactSize {
                path: path.clone(),
                expected: fingerprint.artifact_bytes,
                found: length,
            });
        }
        post_open_hook(&path);
        // Borrow the original descriptor: its native owner and all ancestors
        // remain retained until after the decoder and final custody checks.
        let bytes = read_exact_bounded(original.file_mut(), fingerprint.artifact_bytes, &path)?;
        decode_model(bytes, path.clone(), fingerprint)
    })();
    let file_check = (|| {
        if let Some(original) = &file {
            let current = original
                .snapshot()
                .map_err(|error| input_error(&path, error))?;
            if opened.is_some_and(|snapshot| snapshot != current) {
                return Err(ModerationRunnerError::ArtifactChanged { path: path.clone() });
            }
            original
                .revalidate()
                .map_err(|error| input_error(&path, error))?;
        }
        Ok(())
    })();
    let directory_check = (|| {
        for (directory, snapshot) in &directories {
            if directory
                .snapshot()
                .map_err(|error| input_error(directory.path(), error))?
                != *snapshot
            {
                return Err(ModerationRunnerError::ArtifactChanged {
                    path: directory.path().to_path_buf(),
                });
            }
            directory
                .revalidate()
                .map_err(|error| input_error(directory.path(), error))?;
        }
        Ok(())
    })();
    let root_check = verify_artifact_root(root);
    let final_check = file_check.and(directory_check).and(root_check);
    // A later custody refusal never erases an earlier open/read/decode error.
    result.and_then(|artifact| final_check.map(|()| artifact))
}
