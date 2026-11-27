//! Cold C-session recovery from a simulated loss in a fresh copy of the host fixture.
use super::super::*;
use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};

fn copy_contents(source: &Path, destination: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let path = destination.join(entry.file_name());
        if kind.is_dir() {
            std::fs::DirBuilder::new().mode(0o700).create(&path)?;
            copy_contents(&entry.path(), &path)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), path)?;
        } else {
            return Err(std::io::Error::other("unexpected file in host fixture"));
        }
    }
    Ok(())
}

pub(in super::super) fn check<P: KagemushaWalletPlatformV1 + 'static>(
    owner: &Owner<'_, P>,
    current: &[u8; 32],
    historical: &[u8; 32],
    original_payment: &[u8],
    witness: &native::ExactOperationWitness,
    predecessor: &[u8],
) -> Result<()> {
    // The real fixture journal and its three process-launch results stay intact.
    let parent = owner
        .custody
        .parent()
        .ok_or("missing host fixture parent")?;
    let scratch = tempfile::Builder::new()
        .prefix("simulated-completion-loss-")
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir_in(parent)
        .map_err(fail)?;
    let copy = scratch.path();
    copy_contents(owner.custody, copy).map_err(fail)?;
    let slot = &owner.admission.slot;
    let copied_owner = Owner {
        custody: copy,
        ..*owner
    };
    let (mut warm, _) = copied_owner.open()?;
    let balance = warm.output(&slot.0, &[0; 32], 3)?;
    if balance.status != 101 || warm.next_load_ordinal(&slot.0)? != 1 {
        return Err("funded wallet unavailable before simulated loss".into());
    }
    for relative in advance::test_support::completion_copy_paths(slot, current) {
        let path = copy.join(relative);
        if !path.is_file() {
            return Err("host fixture is missing a completion copy before fault injection".into());
        }
        std::fs::write(path, b"simulated unreadable fixture completion").map_err(fail)?;
    }
    if warm.next_load_ordinal(&slot.0).is_ok()
        || warm.output(&slot.0, &[0; 32], 3).is_ok_and(|r| r.status == 101)
    {
        return Err("a previously verified head bypassed loss of its protected completion".into());
    }
    warm.close()?;
    drop(warm);
    let (mut runtime, _) = copied_owner.open()?;
    if !runtime.retained_outputs_only() {
        return Err("completion loss did not project restricted account access".into());
    }
    if runtime.next_load_ordinal(&slot.0).is_ok() {
        return Err("restricted account exposed a next Load ordinal".into());
    }
    if runtime.activation(&slot.0).is_ok() {
        return Err("restricted account issued an activation".into());
    }
    let loss = runtime.output(&slot.0, current, 2)?;
    let retry = runtime.output(&slot.0, historical, 1)?;
    if loss.status != 5
        || !loss.bytes.is_empty()
        || retry.status != 1
        || retry.bytes != original_payment
    {
        return Err("restricted account failed exact historical output recovery".into());
    }
    if runtime
        .prepare(&slot.0, &witness.encode().map_err(fail)?, predecessor)
        .is_ok()
    {
        return Err("restricted account accepted a monetary review".into());
    }
    runtime.close()?;
    scratch.close().map_err(fail)?;
    eprintln!("Recovered original outgoing Payment through cold restricted C account admission");
    Ok(())
}
