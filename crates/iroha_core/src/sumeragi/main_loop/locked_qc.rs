//! Helpers for enforcing the `HotStuff` `locked-QC` safety rules.

use iroha_crypto::HashOf;
use iroha_data_model::block::BlockHeader;

use crate::sumeragi::consensus::QcHeaderRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LockedQcRejection {
    HeightRegressed {
        locked: u64,
        highest: u64,
    },
    HashMismatch {
        locked: HashOf<BlockHeader>,
        highest: HashOf<BlockHeader>,
    },
}

pub(super) fn ensure_locked_qc_allows(
    locked: Option<QcHeaderRef>,
    highest: QcHeaderRef,
) -> Result<(), LockedQcRejection> {
    let Some(locked) = locked else {
        return Ok(());
    };
    let highest_height = highest.height;
    let locked_height = locked.height;
    if highest_height < locked_height {
        return Err(LockedQcRejection::HeightRegressed {
            locked: locked_height,
            highest: highest_height,
        });
    }
    if highest_height == locked_height && highest.subject_block_hash != locked.subject_block_hash {
        // Same-height lock conflicts can happen during view churn when nodes are temporarily on
        // different branches. Allow a strictly newer view to replace the stale lock so progress
        // can converge on a single branch.
        if highest.view > locked.view {
            return Ok(());
        }
        return Err(LockedQcRejection::HashMismatch {
            locked: locked.subject_block_hash,
            highest: highest.subject_block_hash,
        });
    }
    Ok(())
}

pub(super) fn qc_extends_locked_with_lookup<F>(
    locked: QcHeaderRef,
    highest: QcHeaderRef,
    mut parent_lookup: F,
) -> bool
where
    F: FnMut(HashOf<BlockHeader>, u64) -> Option<HashOf<BlockHeader>>,
{
    if highest.height < locked.height {
        return false;
    }
    if highest.subject_block_hash == locked.subject_block_hash {
        return true;
    }

    let mut current_hash = highest.subject_block_hash;
    let mut current_height = highest.height;

    while current_height > locked.height {
        let Some(parent_hash) = parent_lookup(current_hash, current_height) else {
            return false;
        };
        current_hash = parent_hash;
        current_height = current_height.saturating_sub(1);
        if current_hash == locked.subject_block_hash {
            return true;
        }
    }

    current_height == locked.height && current_hash == locked.subject_block_hash
}

pub(super) fn qc_extends_locked_if_present<F, G>(
    locked: Option<QcHeaderRef>,
    highest: QcHeaderRef,
    parent_lookup: F,
    mut locked_present: G,
) -> bool
where
    F: FnMut(HashOf<BlockHeader>, u64) -> Option<HashOf<BlockHeader>>,
    G: FnMut(HashOf<BlockHeader>) -> bool,
{
    let Some(locked) = locked else {
        return true;
    };
    if !locked_present(locked.subject_block_hash) {
        // Missing locked payload must block extension checks to preserve safety.
        return false;
    }
    qc_extends_locked_with_lookup(locked, highest, parent_lookup)
}

pub(super) fn realign_locked_to_committed_if_extends<F>(
    locked: Option<QcHeaderRef>,
    committed: Option<QcHeaderRef>,
    highest: QcHeaderRef,
    mut parent_lookup: F,
) -> Option<QcHeaderRef>
where
    F: FnMut(HashOf<BlockHeader>, u64) -> Option<HashOf<BlockHeader>>,
{
    let Some(committed_qc) = committed else {
        return locked;
    };
    if qc_extends_locked_with_lookup(committed_qc, highest, &mut parent_lookup) {
        return Some(committed_qc);
    }
    locked
}
