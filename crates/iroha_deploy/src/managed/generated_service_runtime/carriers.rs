//! Bounded exact transaction coordinates retained by the renderer after native child recovery.
//! These structural comparisons neither authenticate a carrier nor construct native evidence.

use super::{ManagedTransactionFinality, Result, invalid};

pub(super) const MAX_REQUIRED_TRANSACTIONS: usize = 32;

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_deploy::managed::generated_service_runtime::RequiredTransaction")]
pub(super) struct RequiredTransaction {
    transaction: [u8; 32],
    height: u64,
    block: [u8; 32],
    block_time_ms: u64,
}

impl RequiredTransaction {
    pub(super) fn from_original(original: &ManagedTransactionFinality) -> Self {
        Self {
            transaction: *original.transaction_hash.as_ref(),
            height: original.height,
            block: *original.block_hash.as_ref(),
            block_time_ms: original.block_time_ms,
        }
    }

    pub(super) fn validate(self) -> Result<()> {
        if self.transaction == [0; 32] || self.height < 2 || self.block == [0; 32] {
            return Err(invalid(
                "generated runtime transaction coordinates are invalid",
            ));
        }
        Ok(())
    }

    fn compatible(self, other: Self) -> bool {
        (self.transaction != other.transaction || self == other)
            && (self.height != other.height
                || (self.block == other.block && self.block_time_ms == other.block_time_ms))
            && (self.block != other.block || self.height == other.height)
    }
}

/// No caller-decoded record constructs this value. The renderer supplies its original recovered
/// histories and still independently rechecks current custody and all owned peers.
pub(super) struct RequiredTransactions {
    originals: Vec<ManagedTransactionFinality>,
    identities: Vec<RequiredTransaction>,
}

impl RequiredTransactions {
    pub(super) fn from_originals(
        originals: impl IntoIterator<Item = ManagedTransactionFinality>,
    ) -> Result<Self> {
        let mut selected = Self {
            originals: Vec::with_capacity(MAX_REQUIRED_TRANSACTIONS),
            identities: Vec::with_capacity(MAX_REQUIRED_TRANSACTIONS),
        };
        // Bound the entire supplied list, including duplicates. Deduplication cannot turn an
        // unbounded iterator into an accepted finite amount of work.
        for (index, original) in originals.into_iter().enumerate() {
            if index >= MAX_REQUIRED_TRANSACTIONS {
                return Err(invalid(
                    "generated runtime transaction inventory exceeds bound",
                ));
            }
            let identity = RequiredTransaction::from_original(&original);
            identity.validate()?;
            if selected
                .identities
                .iter()
                .any(|previous| !identity.compatible(*previous))
            {
                return Err(invalid("generated runtime transaction carriers disagree"));
            }
            if !selected.identities.contains(&identity) {
                selected.identities.push(identity);
                selected.originals.push(original);
            }
        }
        if selected.originals.is_empty() {
            return Err(invalid("generated runtime has no required transaction"));
        }
        Ok(selected)
    }

    pub(super) fn identities(&self) -> &[RequiredTransaction] {
        &self.identities
    }

    pub(super) fn originals(&self) -> &[ManagedTransactionFinality] {
        &self.originals
    }

    /// A lower bound for a fresh native observation only. Every original remains mandatory in
    /// the separate all-peer transaction barrier; selecting this floor discards no receipt.
    pub(super) fn observation_floor(&self) -> Result<ManagedTransactionFinality> {
        self.originals
            .iter()
            .max_by_key(|original| original.height)
            .copied()
            .ok_or_else(|| invalid("generated runtime has no required transaction"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf};

    // Coordinates exercise only structural comparisons; they establish no finalized authority.
    fn coordinates(transaction: u8, height: u64, block: u8) -> ManagedTransactionFinality {
        ManagedTransactionFinality {
            transaction_hash: HashOf::from_untyped_unchecked(Hash::prehashed([transaction; 32])),
            height,
            block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([block; 32])),
            block_time_ms: height * 10,
        }
    }

    #[test]
    fn equal_block_does_not_collapse_distinct_original_transactions() {
        let first = coordinates(1, 2, 3);
        let second = coordinates(2, 2, 3);
        let later = coordinates(3, 4, 5);
        let required = RequiredTransactions::from_originals([first, second, first, later]).unwrap();
        assert_eq!(required.originals(), &[first, second, later]);
        assert_eq!(required.identities().len(), 3);
        assert_eq!(required.observation_floor().unwrap(), later);
        let encoded = norito::to_bytes(&required.identities().to_vec()).unwrap();
        let decoded: Vec<RequiredTransaction> = norito::decode_canonical(&encoded).unwrap();
        assert_eq!(decoded, required.identities());
    }

    #[test]
    fn changed_transaction_or_carrier_identity_refuses_the_whole_selection() {
        let original = coordinates(1, 2, 3);
        let mut changed_time = coordinates(2, 2, 3);
        changed_time.block_time_ms += 1;
        for changed in [
            changed_time,
            coordinates(2, 2, 4),
            coordinates(1, 3, 4),
            coordinates(2, 3, 3),
        ] {
            assert!(RequiredTransactions::from_originals([original, changed]).is_err());
        }
    }

    #[test]
    fn empty_invalid_and_excess_duplicate_inputs_refuse() {
        assert!(RequiredTransactions::from_originals([]).is_err());
        assert!(RequiredTransactions::from_originals([coordinates(1, 1, 3)]).is_err());
        let mut invalid_transaction = RequiredTransaction::from_original(&coordinates(1, 2, 3));
        invalid_transaction.transaction = [0; 32];
        assert!(invalid_transaction.validate().is_err());
        let mut invalid_block = RequiredTransaction::from_original(&coordinates(1, 2, 3));
        invalid_block.block = [0; 32];
        assert!(invalid_block.validate().is_err());
        let original = coordinates(1, 2, 3);
        assert!(
            RequiredTransactions::from_originals([original; MAX_REQUIRED_TRANSACTIONS]).is_ok()
        );
        assert!(
            RequiredTransactions::from_originals([original; MAX_REQUIRED_TRANSACTIONS + 1])
                .is_err()
        );
    }
}
