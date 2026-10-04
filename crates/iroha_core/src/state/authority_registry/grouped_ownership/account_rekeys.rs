//! Exact account-history occurrence checks over both retained native images.
//!
//! Alias reassignment breaks controller continuity but retains audit occurrences.
//! This checks stored provenance and indexes, not finalized execution authority.

use super::*;
use crate::state::account_label_is_pii;
use iroha_data_model::account::{AccountAlias, AccountRekeyRecord, AccountValue};

const TABLE: &str = "world.account_rekey_records";
const INDEX: &str = "world.account_rekey_records_by_account";

/// Canonical histories retained with all original reference and occurrence owners.
pub(in super::super) struct CheckedAccountRekeys<'world> {
    world: &'world World,
    rows: CommittedStorageView<'world, AccountAlias, AccountRekeyRecord>,
    accounts: CommittedStorageView<'world, AccountId, AccountValue>,
    aliases: CommittedStorageView<'world, AccountAlias, AccountId>,
    occurrences: CommittedStorageView<'world, AccountId, BTreeSet<AccountAlias>>,
}

impl<'world> CheckedAccountRekeys<'world> {
    /// Retain and check both images without allocating or repairing live storage.
    pub(in super::super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, GroupedOwnershipError> {
        let checked = Self {
            world,
            rows: world
                .account_rekey_records
                .try_committed_view_nonblocking()?,
            accounts: world.accounts.try_committed_view_nonblocking()?,
            aliases: world.account_aliases.try_committed_view_nonblocking()?,
            occurrences: world
                .account_rekey_records_by_account
                .try_committed_view_nonblocking()?,
        };
        let result = checked.validate(&mut Work(max_work));
        if !checked.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(checked)
    }

    fn validate(&self, work: &mut Work) -> Result<(), GroupedOwnershipError> {
        for image in [GroupImage::Current, GroupImage::Predecessor] {
            visit_image(&self.rows, image, work, |label, record, work| {
                // Fund the complete borrowed string scans before comparing the
                // labels or calling the shared phone-like-label predicate.
                for alias in [label, &record.label] {
                    prepay(work, alias.label.as_ref().len())?;
                    if let Some(domain) = &alias.domain {
                        prepay(work, domain.name().as_ref().len())?;
                    }
                }
                if &record.label != label {
                    return Err(source(image, "record label differs from its storage key"));
                }
                if account_label_is_pii(label) {
                    return Err(source(image, "record label looks like raw PII"));
                }
                work.charge()?;
                if get_at(&self.accounts, image, &record.active_account_id).is_none() {
                    return Err(source(image, "active account is absent"));
                }
                let predecessors = funded_predecessors(record, image, work)?;
                for (position, predecessor) in predecessors.iter().enumerate() {
                    work.charge()?;
                    if get_at(&self.accounts, image, predecessor).is_some() {
                        return Err(source(image, "active rekey predecessor remains live"));
                    }
                    for previous in &predecessors[..position] {
                        work.charge()?;
                        if previous == predecessor {
                            return Err(source(image, "active rekey predecessor is repeated"));
                        }
                    }
                }
                // Every active id exists and every active predecessor is absent.
                // Thus an active target cannot be a predecessor of any record:
                // self-cycles and longer cross-record cycles are both impossible.
                for account in core::iter::once(&record.active_account_id)
                    .chain(record.previous_account_ids.iter())
                {
                    work.charge()?;
                    if !get_at(&self.occurrences, image, account)
                        .is_some_and(|aliases| aliases.contains(label))
                    {
                        return Err(corrupt(image, GroupMismatch::MissingMember));
                    }
                }
                Ok(())
            })?;
            visit_image(&self.aliases, image, work, |label, account, work| {
                work.charge()?;
                if get_at(&self.accounts, image, account).is_none() {
                    return Err(source(image, "alias target account is absent"));
                }
                work.charge()?;
                let record = get_at(&self.rows, image, label)
                    .ok_or_else(|| source(image, "alias has no continuity record"))?;
                if &record.active_account_id != account {
                    return Err(source(
                        image,
                        "alias target differs from the active account",
                    ));
                }
                Ok(())
            })?;
            visit_image(&self.occurrences, image, work, |account, aliases, work| {
                if aliases.is_empty() {
                    return Err(corrupt(image, GroupMismatch::EmptyGroup));
                }
                for label in aliases {
                    work.charge()?;
                    let record = get_at(&self.rows, image, label)
                        .ok_or_else(|| corrupt(image, GroupMismatch::ForeignMember))?;
                    if !contains_account(
                        core::iter::once(&record.active_account_id)
                            .chain(record.previous_account_ids.iter()),
                        account,
                        work,
                    )? {
                        return Err(corrupt(image, GroupMismatch::ForeignMember));
                    }
                }
                Ok(())
            })?;
            // Only after both directions establish the exact occurrence index
            // may it bound the records inspected for a shared predecessor.
            visit_image(&self.occurrences, image, work, |account, aliases, work| {
                let mut target = None;
                for label in aliases {
                    work.charge()?;
                    let record = get_at(&self.rows, image, label)
                        .ok_or_else(|| corrupt(image, GroupMismatch::ForeignMember))?;
                    let predecessors = funded_predecessors(record, image, work)?;
                    if contains_account(predecessors.iter(), account, work)? {
                        if target.is_some_and(|previous| previous != &record.active_account_id) {
                            return Err(source(
                                image,
                                "active rekey predecessor has ambiguous targets",
                            ));
                        }
                        target = Some(&record.active_account_id);
                    }
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Borrow the original rows whose histories and occurrences were checked.
    pub(in super::super) fn rows(
        &self,
    ) -> &CommittedStorageView<'world, AccountAlias, AccountRekeyRecord> {
        &self.rows
    }

    /// Recheck all four retained native owners after canonical encoding.
    pub(in super::super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        Ok(self
            .rows
            .try_matches_current(&self.world.account_rekey_records)?
            && self.accounts.try_matches_current(&self.world.accounts)?
            && self
                .aliases
                .try_matches_current(&self.world.account_aliases)?
            && self
                .occurrences
                .try_matches_current(&self.world.account_rekey_records_by_account)?)
    }
}

fn prepay(work: &mut Work, count: usize) -> Result<(), GroupedOwnershipError> {
    let count = u64::try_from(count).map_err(|_| GroupedOwnershipError::WorkLimit)?;
    work.0 = work
        .0
        .checked_sub(count)
        .ok_or(GroupedOwnershipError::WorkLimit)?;
    Ok(())
}

fn funded_predecessors<'a>(
    record: &'a AccountRekeyRecord,
    image: GroupImage,
    work: &mut Work,
) -> Result<&'a [AccountId], GroupedOwnershipError> {
    // The shared method scans provenance backwards. Reserve its maximum scan
    // before entering it, including calls during ambiguity checks.
    work.charge()?;
    prepay(work, record.transition_provenance.len())?;
    record.active_account_id_rekey_predecessors().map_err(|_| {
        source(
            image,
            "transition provenance length differs from account history",
        )
    })
}

fn contains_account<'a, I>(
    accounts: I,
    account: &AccountId,
    work: &mut Work,
) -> Result<bool, GroupedOwnershipError>
where
    I: IntoIterator<Item = &'a AccountId>,
{
    for candidate in accounts {
        work.charge()?;
        if candidate == account {
            return Ok(true);
        }
    }
    Ok(false)
}

fn source(image: GroupImage, reason: &'static str) -> GroupedOwnershipError {
    GroupedOwnershipError::Source {
        table: TABLE,
        image,
        reason,
    }
}

fn corrupt(image: GroupImage, mismatch: GroupMismatch) -> GroupedOwnershipError {
    GroupedOwnershipError::Corrupt {
        index: INDEX,
        image,
        mismatch,
    }
}

#[cfg(test)]
mod tests;
