//! Restore account-history occurrences only from two validated native images.
//!
//! Canonical source rows stay borrowed and unchanged. The derived occurrence
//! index preserves complete predecessor buckets and redundant source touches.
//! TODO: admit reconstruction scratch through the original snapshot resource
//! owner together with the remaining infallible derived-index reconstruction.

use super::*;
use mv::storage::History;

type Records<'a> = History<'a, AccountAlias, AccountRekeyRecord>;
type Accounts<'a> = History<'a, AccountId, AccountValue>;
type Aliases<'a> = History<'a, AccountAlias, AccountId>;

fn account_exists(accounts: &Accounts<'_>, prior: bool, account: &AccountId) -> bool {
    if prior {
        accounts.get_before_block(account).is_some()
    } else {
        accounts.current().get(account).is_some()
    }
}

fn validate_record<'a>(
    label: &AccountAlias,
    record: &'a AccountRekeyRecord,
    accounts: &Accounts<'_>,
    prior: bool,
    targets: &mut BTreeMap<&'a AccountId, &'a AccountId>,
) -> Result<(), String> {
    if &record.label != label {
        return Err(format!(
            "Account rekey record {label:?} stores mismatched label {:?}",
            record.label
        ));
    }
    if account_label_is_pii(label) {
        return Err(format!(
            "Account rekey record {label:?} looks like raw PII; use UAID/opaque identifiers"
        ));
    }
    if !account_exists(accounts, prior, &record.active_account_id) {
        return Err(format!(
            "Account rekey record {label:?} references missing account {}",
            record.active_account_id
        ));
    }
    let predecessors = record
        .active_account_id_rekey_predecessors()
        .map_err(|error| {
            format!("Account rekey record {label:?} has malformed provenance: {error}")
        })?;
    let mut unique = BTreeSet::new();
    for predecessor in predecessors {
        if predecessor == &record.active_account_id {
            return Err(format!(
                "Account rekey record {label:?} contains an active account-id rekey cycle at {predecessor}"
            ));
        }
        if !unique.insert(predecessor) {
            return Err(format!(
                "Account rekey record {label:?} repeats active account-id rekey predecessor {predecessor}"
            ));
        }
        // Every target is live and every predecessor is absent in this same
        // image, so cross-record chains and cycles cannot have a live middle id.
        if account_exists(accounts, prior, predecessor) {
            return Err(format!(
                "Account-id rekey predecessor {predecessor} remains an independently live account"
            ));
        }
        if let Some(existing) = targets.insert(predecessor, &record.active_account_id)
            && existing != &record.active_account_id
        {
            return Err(format!(
                "Account-id rekey predecessor {predecessor} ambiguously targets {existing} and {}",
                record.active_account_id
            ));
        }
    }
    Ok(())
}

fn validate_alias(
    label: &AccountAlias,
    account: &AccountId,
    records: &Records<'_>,
    accounts: &Accounts<'_>,
    prior: bool,
) -> Result<(), String> {
    if !account_exists(accounts, prior, account) {
        return Err(format!(
            "Account rekey record {label:?} references missing account {account}"
        ));
    }
    let record = if prior {
        records.get_before_block(label)
    } else {
        records.current().get(label)
    }
    .ok_or_else(|| format!("Account alias binding {label:?} is missing its continuity record"))?;
    if &record.active_account_id != account {
        return Err(format!(
            "Account alias binding {label:?} points to {account}, but its continuity record points to {}",
            record.active_account_id
        ));
    }
    Ok(())
}

fn validate(
    records: &Records<'_>,
    accounts: &Accounts<'_>,
    aliases: &Aliases<'_>,
    prior: bool,
) -> Result<(), String> {
    let mut targets = BTreeMap::new();
    if prior {
        for (label, record) in records.iter_before_block() {
            validate_record(label, record, accounts, true, &mut targets)?;
        }
        for (label, account) in aliases.iter_before_block() {
            validate_alias(label, account, records, accounts, true)?;
        }
    } else {
        for (label, record) in records.current().iter() {
            validate_record(label, record, accounts, false, &mut targets)?;
        }
        for (label, account) in aliases.current().iter() {
            validate_alias(label, account, records, accounts, false)?;
        }
    }
    Ok(())
}

pub(super) fn rebuild(world: &mut World) -> Result<(), String> {
    // Retain all source histories through validation and index construction.
    // Neither inspection nor a failure opens or commits a temporary revert.
    let world = &mut world.0;
    let records = world.account_rekey_records.history();
    let accounts = world.accounts.history();
    let aliases = world.account_aliases.history();
    for prior in [false, true] {
        validate(&records, &accounts, &aliases, prior).map_err(|error| {
            let image = if prior { "predecessor" } else { "current" };
            format!("Invalid {image} account continuity: {error}")
        })?;
    }
    let current = account_rekey_occurrence_index(records.current().iter());
    let mut previous = account_rekey_occurrence_index(records.iter_before_block());
    // Preserve redundant source touches as real derived undo entries. Complete
    // predecessor buckets include untouched aliases sharing a changed account.
    let mut touched = BTreeSet::new();
    for (label, prior) in records.revert_map().iter() {
        for record in [prior.as_ref(), records.current().get(label)]
            .into_iter()
            .flatten()
        {
            touched.extend(account_ids_in_rekey_record(record).cloned());
        }
    }
    let undo = touched
        .into_iter()
        .map(|account| {
            let prior = previous.remove(&account);
            (account, prior)
        })
        .collect();
    let index = Storage::from_snapshot_parts(current, undo);
    // Publish only after both complete images and the derived replacement exist.
    world.account_rekey_records_by_account = index;
    Ok(())
}

#[cfg(test)]
mod tests;
