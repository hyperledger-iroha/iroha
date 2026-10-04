//! Exact registry-to-circuit inverse over two original native images.
//!
//! This relation applies to every retained status and activation interval. It grants no
//! verifier admission or proof authority. All comparisons borrow original strings; no
//! canonical tuple, key, decoder, crypto parser or diagnostic allocation is needed.
//! Startup retains its existing infallible work boundary. TODO: admit restore work through
//! the original snapshot owner with the other unfinished restore resource reservations.

use crate::state::authority_registry::original_images::RawStorageImages;
use iroha_data_model::proof::{VerifyingKeyId, VerifyingKeyRecord};
use std::cmp::Ordering;

/// Original logical image being checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Image {
    Current,
    Predecessor,
}

/// A finite local refusal or a completed inverse mismatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    WorkLimit,
    Index { image: Image, missing: bool },
}

/// Local comparison allowance, separate from transaction gas and validity.
pub(super) struct Work(Option<u64>);
impl Work {
    pub(super) fn bounded(limit: u64) -> Self {
        Self(Some(limit))
    }
    pub(super) fn startup() -> Self {
        Self(None)
    }
    fn charge(&mut self, amount: usize) -> Result<(), Error> {
        if let Some(remaining) = &mut self.0 {
            let amount = u64::try_from(amount).map_err(|_| Error::WorkLimit)?;
            *remaining = remaining.checked_sub(amount).ok_or(Error::WorkLimit)?;
        }
        Ok(())
    }
}

// Only the two concrete native key geometries below can enter this internal merge.
trait WorkKey: mv::Key {
    fn prepay(&self, work: &mut Work) -> Result<(), Error>;
}
impl WorkKey for VerifyingKeyId {
    fn prepay(&self, work: &mut Work) -> Result<(), Error> {
        work.charge(self.backend.len())?;
        work.charge(self.name.len())
    }
}
impl WorkKey for (String, u32) {
    fn prepay(&self, work: &mut Work) -> Result<(), Error> {
        work.charge(self.0.len())?;
        work.charge(size_of::<u32>())
    }
}
fn compare<K: WorkKey>(left: &K, right: &K, work: &mut Work) -> Result<Ordering, Error> {
    left.prepay(work)?;
    right.prepay(work)?;
    Ok(left.cmp(right))
}
fn next_physical<'a, K: 'a, V: 'a>(
    rows: &mut impl Iterator<Item = (&'a K, &'a V)>,
    work: &mut Work,
) -> Result<Option<(&'a K, &'a V)>, Error> {
    let row = rows.next();
    if row.is_some() {
        work.charge(1)?;
    }
    Ok(row)
}

/// Merge original maps without hidden variable-key tree lookups or source clones.
fn visit<K: WorkKey, V: mv::Value>(
    rows: &impl RawStorageImages<K, V>,
    image: Image,
    work: &mut Work,
    mut visitor: impl FnMut(&K, &V, &mut Work) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut current = rows.current_entries();
    if image == Image::Current {
        while let Some((key, value)) = next_physical(&mut current, work)? {
            visitor(key, value, work)?;
        }
        return Ok(());
    }
    let mut undo = rows.undo_entries();
    let mut current_row = next_physical(&mut current, work)?;
    let mut undo_row = next_physical(&mut undo, work)?;
    loop {
        match (current_row, undo_row) {
            (Some((key, value)), Some((prior_key, prior))) => {
                match compare(key, prior_key, work)? {
                    Ordering::Less => {
                        visitor(key, value, work)?;
                        current_row = next_physical(&mut current, work)?;
                    }
                    Ordering::Equal => {
                        if let Some(value) = prior {
                            visitor(prior_key, value, work)?;
                        }
                        current_row = next_physical(&mut current, work)?;
                        undo_row = next_physical(&mut undo, work)?;
                    }
                    Ordering::Greater => {
                        if let Some(value) = prior {
                            visitor(prior_key, value, work)?;
                        }
                        undo_row = next_physical(&mut undo, work)?;
                    }
                }
            }
            (Some((key, value)), None) => {
                visitor(key, value, work)?;
                current_row = next_physical(&mut current, work)?;
            }
            (None, Some((key, prior))) => {
                if let Some(value) = prior {
                    visitor(key, value, work)?;
                }
                undo_row = next_physical(&mut undo, work)?;
            }
            (None, None) => return Ok(()),
        }
    }
}
fn count<K: WorkKey, V: mv::Value>(
    rows: &impl RawStorageImages<K, V>,
    image: Image,
    work: &mut Work,
) -> Result<usize, Error> {
    let mut count = 0usize;
    visit(rows, image, work, |_, _, _| {
        count = count.checked_add(1).ok_or(Error::WorkLimit)?;
        Ok(())
    })?;
    Ok(count)
}

/// Exact cardinality and source membership establish the complete bijection in both images.
///
/// Every physical scan (including masked rows/tombstones) and maximum comparison byte
/// geometry is admitted before inspecting it. Repeated borrowed scans may require more
/// local work for a large registry; no arbitrary registry or consensus limit is imposed.
pub(super) fn validate(
    keys: &impl RawStorageImages<VerifyingKeyId, VerifyingKeyRecord>,
    index: &impl RawStorageImages<(String, u32), VerifyingKeyId>,
    work: &mut Work,
) -> Result<(), Error> {
    for image in [Image::Current, Image::Predecessor] {
        let sources = count(keys, image, work)?;
        let entries = count(index, image, work)?;
        if sources != entries {
            return Err(Error::Index {
                image,
                missing: entries < sources,
            });
        }
        visit(keys, image, work, |id, record, work| {
            let mut matched = false;
            visit(index, image, work, |(circuit, version), stored, work| {
                work.charge(record.circuit_id.len())?;
                work.charge(circuit.len())?;
                work.charge(2 * size_of::<u32>())?;
                if record.circuit_id == *circuit && record.version == *version {
                    if compare(id, stored, work)? != Ordering::Equal {
                        return Err(Error::Index {
                            image,
                            missing: true,
                        });
                    }
                    matched = true;
                }
                Ok(())
            })?;
            if !matched {
                return Err(Error::Index {
                    image,
                    missing: true,
                });
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod test_support;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod frozen_tests;
