//! Whole-generation staging and native atomic publication under the context operation lock.

use super::*;
use iroha_fs::{PrivateDirectory, PublishMode};

pub(super) const DIRECTORY: &str = "generation";
const STAGING: &str = ".preparing";

/// A missing generation may be prepared; a visible generation without its manifest is corrupt.
pub(super) fn read(directory: &PrivateDirectory) -> Result<RetainedLocalnet> {
    let generation = directory.open_child(DIRECTORY)?;
    let bytes = generation.read(MANIFEST, MAX_METADATA).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::Invalid("published managed generation is missing its immutable manifest".into())
        } else {
            error.into()
        }
    })?;
    decode(&bytes)
}

/// Discard only a never-published stage while the caller exclusively owns the context operation.
fn fresh_stage(directory: &PrivateDirectory) -> Result<PrivateDirectory> {
    match directory.open_child(STAGING) {
        Ok(stage) => {
            stage.clear_contents_preserving(&[])?;
            stage.remove_empty()?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(directory.create_child(STAGING)?)
}

pub(super) fn prepare(
    directory: &PrivateDirectory,
    request: &LocalnetRequest,
    root_kind: RootKind,
    launcher: BinaryPin,
    daemon: BinaryPin,
    ports: &LocalnetPorts,
) -> Result<RetainedLocalnet> {
    if matches!(root_kind, RootKind::Private { .. })
        && request.service_profile != crate::localnet::LocalnetServiceProfile::Standard
    {
        return Err(Error::Invalid(
            "service-authority profiles require a global managed root".into(),
        ));
    }
    let published_path = directory.path().join(DIRECTORY);
    let stage = fresh_stage(directory)?;
    let prepared = match &root_kind {
        RootKind::Global => crate::localnet::prepare_localnet_at(
            &request.name,
            stage.path(),
            ports,
            request.service_profile,
            Some(&published_path),
        )?,
        RootKind::Private { spec } => crate::localnet::prepare_private_root_at(
            &request.name,
            stage.path(),
            ports,
            spec,
            Some(&published_path),
        )?,
    };
    // Only this unpublished generation owns newly randomized, never-launched keys. Publish
    // their one-shot launch fences together with those keys; installed generations never
    // infer fresh provenance from a missing file or recreate a lost fence.
    if prepared.peers.len() != 4 {
        return Err(Error::Invalid(
            "managed generation requires four fresh peer launch fences".into(),
        ));
    }
    for index in 0..4 {
        stage.write_atomic(format!("peer{index}.launch"), b"0", PublishMode::CreateNew)?;
    }
    let retained = RetainedLocalnet {
        root_kind,
        prepared,
        launcher,
        daemon,
        startup_timeout_ms: u64::try_from(request.startup_timeout.as_millis())
            .map_err(|_| Error::Invalid("startup timeout is too large".into()))?,
    };
    stage.write_atomic(MANIFEST, &encode(&retained)?, PublishMode::CreateNew)?;
    crate::localnet::sync_private_tree(stage.path()).map_err(|_| {
        Error::Invalid("cannot durably stage the complete managed generation".into())
    })?;
    match stage.rename_to_sibling(DIRECTORY, PublishMode::CreateNew) {
        Ok(published) => {
            published.revalidate()?;
        }
        Err(error) => {
            // Native publication can succeed before a later sync fails. Reopen and authenticate
            // the complete exact generation before retrying durability; never generate new keys.
            reconcile_publication(directory, &retained).map_err(|_| Error::Io(error))?;
        }
    }
    store::validate_prepared(
        &request.name,
        directory.path(),
        &retained.prepared,
        &retained.root_kind,
    )?;
    Ok(retained)
}

fn reconcile_publication(directory: &PrivateDirectory, expected: &RetainedLocalnet) -> Result<()> {
    let published = read(directory)?;
    if encode(&published)? != encode(expected)? {
        return Err(Error::Invalid(
            "generation publication encountered a different identity".into(),
        ));
    }
    store::validate_prepared(
        &expected.prepared.context.name,
        directory.path(),
        &published.prepared,
        &published.root_kind,
    )?;
    directory.open_child(DIRECTORY)?.sync()?;
    directory.sync()?;
    Ok(())
}

#[cfg(test)]
mod tests;
