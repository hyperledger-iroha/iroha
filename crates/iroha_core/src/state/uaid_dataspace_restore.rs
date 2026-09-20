//! Rebuild derived UAID bindings from both authoritative MV cuts at startup.

use super::*;

fn bindings<'a>(
    accounts: impl Iterator<Item = (&'a AccountId, &'a AccountValue)>,
    manifests: impl Iterator<Item = (&'a UniversalAccountId, &'a SpaceDirectoryManifestSet)>,
) -> core::result::Result<(BTreeMap<UniversalAccountId, UaidDataspaceBindings>, usize), String> {
    let mut accounts_by_uaid = BTreeMap::<UniversalAccountId, Vec<&AccountId>>::new();
    for (account, value) in accounts {
        if let Some(uaid) = value.as_ref().uaid() {
            accounts_by_uaid.entry(*uaid).or_default().push(account);
        }
    }
    let mut result = BTreeMap::new();
    let mut count = 0_usize;
    for (uaid, manifests) in manifests {
        count = count.checked_add(1).ok_or("UAID manifest count overflow")?;
        // Keep the existing explicit rejection diagnostic for this retired sentinel.
        for (dataspace, record) in manifests.iter() {
            let digest = record.manifest_hash.as_ref();
            if record.is_active()
                && digest[..Hash::LENGTH - 1].iter().all(|byte| *byte == 0)
                && digest[Hash::LENGTH - 1] == 1
            {
                return Err(format!(
                    "Space Directory manifest for UAID {uaid} dataspace {} uses the retired zero-hash sentinel",
                    dataspace.as_u64()
                ));
            }
        }
        if !snapshot_storage::manifest_set_matches_key(uaid, manifests) {
            return Err(
                "Space Directory manifest key or canonical hash differs from its record".into(),
            );
        }
        let Some(accounts) = accounts_by_uaid.get(uaid) else {
            continue;
        };
        let mut derived = UaidDataspaceBindings::default();
        for (dataspace, _) in manifests.iter().filter(|(_, record)| record.is_active()) {
            for account in accounts {
                derived.bind_account(*dataspace, (*account).clone());
            }
        }
        if !derived.is_empty() {
            result.insert(*uaid, derived);
        }
    }
    Ok((result, count))
}

/// Install both derived cuts atomically after validating both authoritative cuts.
/// The existing derived cache contributes neither values nor the touched-key inventory.
pub(super) fn rebuild(world: &mut World) -> core::result::Result<usize, String> {
    let (current, previous, touched, count) = {
        let accounts = world.accounts.snapshot();
        let manifests = world.space_directory_manifests.snapshot();
        let (current, count) = bindings(accounts.current().iter(), manifests.current().iter())?;
        let (previous, _) = bindings(
            accounts
                .current()
                .iter()
                .filter(|(key, _)| !accounts.revert_map().contains_key(*key))
                .chain(
                    accounts
                        .revert_map()
                        .iter()
                        .filter_map(|(key, value)| value.as_ref().map(|value| (key, value))),
                ),
            manifests
                .current()
                .iter()
                .filter(|(key, _)| !manifests.revert_map().contains_key(*key))
                .chain(
                    manifests
                        .revert_map()
                        .iter()
                        .filter_map(|(key, value)| value.as_ref().map(|value| (key, value))),
                ),
        )?;
        let mut touched: BTreeSet<_> = manifests.revert_map().keys().copied().collect();
        for (key, previous_account) in accounts.revert_map().iter() {
            for value in [previous_account.as_ref(), accounts.current().get(key)]
                .into_iter()
                .flatten()
            {
                if let Some(uaid) = value.as_ref().uaid() {
                    touched.insert(*uaid);
                }
            }
        }
        (current, previous, touched, count)
    };
    // Source touches preserve explicit absence, including a redundant removal.
    // Also retain every semantic difference even if a future authoritative owner
    // changes which of its source keys it records as touched.
    let mut touched = touched;
    for key in current.keys().chain(previous.keys()) {
        if current.get(key) != previous.get(key) {
            touched.insert(*key);
        }
    }
    let undo = touched
        .into_iter()
        .map(|key| (key, previous.get(&key).cloned()))
        .collect();
    world.uaid_dataspaces = Storage::from_snapshot_parts(current, undo);
    Ok(count)
}

#[cfg(test)]
#[path = "uaid_dataspace_restore_tests.rs"]
mod tests;
