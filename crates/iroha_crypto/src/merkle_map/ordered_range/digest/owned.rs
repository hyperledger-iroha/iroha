//! Final digest-row/key backing tied to its original finite allocation pool.
//!
//! Keys, row slots and padded Merkle levels use exact fixed buffers. The
//! fallible private shared owner retains its exact concrete control allocation
//! charge until the final reference frees the allocation and owned backings.

use super::*;
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError, PrepaidSharedError};
use std::fmt;

#[path = "levels.rs"]
mod levels;

/// One immutable canonical key and digest, with original backing custody.
pub(super) struct DigestEntry {
    /// Exactly allocated canonical key bytes.
    pub(super) key: ChargedBuffer<u8>,
    /// Complete canonical value frame digest.
    pub(super) value_digest: Hash,
}

impl fmt::Debug for DigestEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DigestEntry")
            .field("key", &self.key.as_slice())
            .field("value_digest", &self.value_digest)
            .finish()
    }
}

/// One private strong-only owner shared by every immutable tree clone.
pub(super) struct DigestTreeBacking {
    /// Fixed final row slots and their owned key allocations.
    pub(super) entries: ChargedBuffer<DigestEntry>,
    /// Padded Merkle levels and their exact original charged outer backing.
    pub(super) levels: ChargedBuffer<ChargedBuffer<Hash>>,
    /// Existing canonical ordered commitment.
    pub(super) root: Hash,
}

impl fmt::Debug for DigestTreeBacking {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DigestTreeBacking")
            .field("entries", &self.entries.as_slice())
            .field("level_count", &self.levels.as_slice().len())
            .field("root", &self.root)
            .finish()
    }
}

fn buffer_error(error: ChargedBufferError) -> NoritoKeyRangeError {
    match error {
        ChargedBufferError::Admission(error) => NoritoKeyRangeError::Admission(error),
        ChargedBufferError::Allocator { .. } => NoritoKeyRangeError::Allocation,
    }
}

/// Exact concrete control allocation, including the inline tree backing.
pub(super) fn owner_bytes() -> usize {
    ChargedShared::<DigestTreeBacking>::allocation_layout().size()
}

fn shared_error(error: PrepaidSharedError) -> NoritoKeyRangeError {
    match error {
        PrepaidSharedError::Reservation(error) => NoritoKeyRangeError::PrepaidCapacity(error),
        PrepaidSharedError::Allocator { .. } => NoritoKeyRangeError::Allocation,
    }
}

impl NoritoKeyDigestRangeTreeV1 {
    /// Build a bounded tree from strictly increasing canonical keys and value digests.
    ///
    /// The caller must hash the full canonical value and enumerate every
    /// authoritative row. This layer checks neither condition by itself.
    /// The iterator's exact size admits fixed row backing before allocation;
    /// malformed size claims are rejected without exposing a partial tree.
    /// `budget` is the caller's original finite pool, including when shared
    /// with execution as a local policy. It grants no State or finality authority.
    /// Key copies are admitted while any caller staging buffers remain live.
    /// TODO: fund proof buffers and caller staging through their own owners.
    ///
    /// # Errors
    /// Rejects invalid domains, duplicate or descending keys, resource bounds,
    /// and failed allocations.
    pub fn from_sorted_digests<K: AsRef<[u8]>, I>(
        schema_hash: Hash,
        domain: &[u8],
        entries: I,
        max_retained_bytes: usize,
        budget: &AllocationBudget,
    ) -> Result<Self, NoritoKeyRangeError>
    where
        I: IntoIterator<Item = (K, Hash)>,
        I::IntoIter: ExactSizeIterator,
    {
        validate_domain(domain)?;
        let maximum = max_retained_bytes.min(MAX_NORITO_TREE_PAYLOAD_BYTES);
        let entries = entries.into_iter();
        let count = entries.len();
        if count > MAX_NORITO_TREE_ENTRIES {
            return Err(NoritoKeyRangeError::Capacity);
        }
        let mut owner_credit = budget
            .try_reserve_bytes(owner_bytes())
            .map_err(NoritoKeyRangeError::Admission)?;
        let mut retained =
            ChargedBuffer::<DigestEntry>::new(count, budget).map_err(buffer_error)?;
        let mut bytes = 0_usize;
        for (key, value_digest) in entries {
            if retained.as_slice().len() == count {
                return Err(NoritoKeyRangeError::Capacity);
            }
            let key = key.as_ref();
            if key.len() > MAX_NORITO_KEY_BYTES {
                return Err(NoritoKeyRangeError::Capacity);
            }
            if retained
                .as_slice()
                .last()
                .is_some_and(|previous| previous.key.as_slice() >= key)
            {
                return Err(NoritoKeyRangeError::UnsortedKeys);
            }
            add_size(&mut bytes, key.len(), maximum)?;
            add_size(&mut bytes, Hash::LENGTH, maximum)?;
            let mut owned_key = ChargedBuffer::new(key.len(), budget).map_err(buffer_error)?;
            owned_key
                .append(key)
                .expect("exact key capacity was admitted");
            retained
                .try_push(DigestEntry {
                    key: owned_key,
                    value_digest,
                })
                .map_err(|_| NoritoKeyRangeError::Capacity)?;
        }
        if retained.as_slice().len() != count {
            return Err(NoritoKeyRangeError::Capacity);
        }
        if !retained.as_slice().is_empty() {
            let leaves = retained.as_slice().len().next_power_of_two();
            let nodes = leaves
                .checked_mul(2)
                .and_then(|count| count.checked_sub(1))
                .ok_or(NoritoKeyRangeError::Capacity)?;
            add_size(
                &mut bytes,
                nodes
                    .checked_mul(Hash::LENGTH)
                    .ok_or(NoritoKeyRangeError::Capacity)?,
                maximum,
            )?;
        }
        let levels = levels::build(retained.as_slice(), budget)?;
        let count = u32::try_from(retained.as_slice().len()).expect("bounded entry count fits u32");
        let top = levels
            .as_slice()
            .last()
            .and_then(|nodes| nodes.as_slice().first())
            .copied()
            .unwrap_or_else(empty_hash);
        let backing = DigestTreeBacking {
            root: root_hash(count, &schema_hash, domain, top),
            entries: retained,
            levels,
        };
        let inner = ChargedShared::from_reservation(backing, &mut owner_credit)
            .map_err(|(_backing, error)| shared_error(error))?;
        Ok(Self { inner })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_allocation::InsufficientReservation;

    #[test]
    fn shared_owner_failures_preserve_prepaid_demand_and_local_allocator_error() {
        let shortage = InsufficientReservation {
            requested_bytes: owner_bytes(),
            remaining_bytes: 0,
        };
        assert_eq!(
            shared_error(PrepaidSharedError::Reservation(shortage)),
            NoritoKeyRangeError::PrepaidCapacity(shortage)
        );
        assert_eq!(
            shared_error(PrepaidSharedError::Allocator {
                requested_bytes: owner_bytes(),
            }),
            NoritoKeyRangeError::Allocation
        );
    }
}
