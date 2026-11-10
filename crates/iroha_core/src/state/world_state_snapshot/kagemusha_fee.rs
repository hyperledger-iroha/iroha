//! Existing immutable fee payout bytes at the original complete certified World cut.

use super::*;
use iroha_data_model::{
    block::consensus::SumeragiRootScope,
    kagemusha::{KagemushaWalletPayoutKeyV1, KagemushaWalletPayoutRecordV1},
};

fn require_fee_original(bytes: &[u8], credit: [u8; 32]) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > 1024 || credit == [0; 32] {
        return Err("fee payout original exceeds its bound or selects a zero credit".into());
    }
    let payout: KagemushaWalletPayoutRecordV1 =
        norito::decode_canonical(bytes).map_err(|error| error.to_string())?;
    if payout.key != KagemushaWalletPayoutKeyV1::Fee(credit)
        || payout.amount == 0
        || payout.source == [0; 32]
        || payout.transaction == [0; 32]
    {
        return Err("fee payout original is not the selected nonzero executed Fee row".into());
    }
    Ok(())
}

fn require_fee_global_root(world: &impl WorldReadOnly) -> Result<(), WorldStateSnapshotError> {
    let root = crate::sumeragi::lanes::routing::read_routing_root_scope(world)
        .map_err(WorldStateSnapshotError::RootScopeRead)?;
    if root != Some(SumeragiRootScope::Global) {
        return Err("fee snapshot requires the authenticated Global root".into());
    }
    Ok(())
}

impl State {
    /// Borrow a complete certified World and the exact existing Fee payout row.
    ///
    /// This read-only owner neither executes a fee nor constructs a payout from caller values.
    /// Full World publication requires the integrating owner to establish all-ledger read
    /// custody; the offline replay owner uses its independently admitted copied node inputs.
    /// Consumers still authenticate the same installed genesis/root, selected finality cut,
    /// native schema and retained wallet Fee claim. The returned bytes confer no custody.
    ///
    /// # Errors
    /// Refuses a foreign/private/stale cut, zero selectors, missing/malformed/oversized row,
    /// post-result row changes, unavailable native replay cut, or original resource refusal.
    pub fn with_native_kagemusha_fee_snapshot_v1<T>(
        &self,
        tip: &CurrentCertifiedCut,
        scheme: [u8; 32],
        credit: [u8; 32],
        budget: &AllocationBudget,
        consume: impl FnOnce(&WorldStateSnapshotV1, &[u8]) -> Result<T, String>,
    ) -> Result<T, WorldStateSnapshotError> {
        if tip.height() < 2
            || tip.root_scope() != SumeragiRootScope::Global
            || scheme == [0; 32]
            || credit == [0; 32]
        {
            return Err(
                "fee snapshot requires a successor Global cut and nonzero selectors".into(),
            );
        }
        self.with_native_world_state_snapshot_cut_v1(tip, None, budget, |snapshot, world| {
            require_fee_global_root(world)?;
            let key = KagemushaWalletPayoutKeyV1::Fee(credit).ledger_key(scheme);
            let bytes = world
                .kagemusha_wallet_ledger()
                .get(&key)
                .ok_or("fee snapshot payout row is absent")?;
            // Decode work remains finite and charged by this caller's retained pool.
            let _row = budget
                .try_reserve_bytes(
                    norito::canonical_decode_limits(1024).max_total_allocated_bytes(),
                )
                .map_err(|error| error.to_string())?;
            require_fee_original(bytes, credit)?;
            require_target(
                snapshot,
                "world.kagemusha_wallet_ledger",
                WorldStateElementKindV1::Table,
                Some(hash_value(&key)?),
                hash_value(bytes)?,
            )?;
            consume(snapshot, bytes).map_err(WorldStateSnapshotError::from)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_root_preserves_original_decoder_refusal_and_accepts_same_world_retry() {
        let global =
            crate::sumeragi::lanes::routing::test_support::world(SumeragiRootScope::Global);
        let view = global.view();
        let zero = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64);
        let error = norito::with_decode_limits_scope(zero, || require_fee_global_root(&view))
            .expect_err("a valid Global root must preserve the original allocation refusal");
        let WorldStateSnapshotError::RootScopeRead(original) = error else {
            panic!("local decoder refusal was converted to a foreign root");
        };
        assert!(matches!(
            original.decode_refusal().expect("actual root decoder refusal").resource_error(),
            norito::core::DecodeResourceError::TotalAllocationExceeded { attempted, limit: 0 }
                if attempted > 0
        ));
        require_fee_global_root(&view)
            .expect("same Global root succeeds once the refusal is removed");
        assert!(matches!(
            require_fee_global_root(&crate::state::World::new().view()),
            Err(WorldStateSnapshotError::Invalid(_))
        ));
    }

    #[test]
    fn fee_original_refuses_wrong_namespace_credit_and_malformed_fields() {
        // Structural control only: this DTO has no World/finality authority.
        let base = KagemushaWalletPayoutRecordV1 {
            key: KagemushaWalletPayoutKeyV1::Fee([1; 32]),
            source: [2; 32],
            amount: 3,
            transaction: [4; 32],
        };
        let bytes = norito::encode_canonical(&base).unwrap();
        require_fee_original(&bytes, [1; 32]).unwrap();
        assert!(require_fee_original(&bytes, [5; 32]).is_err());
        assert!(require_fee_original(&bytes[..bytes.len() - 1], [1; 32]).is_err());
        assert!(require_fee_original(&[0; 1025], [1; 32]).is_err());
        for changed in [
            KagemushaWalletPayoutRecordV1 {
                key: KagemushaWalletPayoutKeyV1::Unload([1; 32]),
                ..base
            },
            KagemushaWalletPayoutRecordV1 { amount: 0, ..base },
            KagemushaWalletPayoutRecordV1 {
                source: [0; 32],
                ..base
            },
            KagemushaWalletPayoutRecordV1 {
                transaction: [0; 32],
                ..base
            },
        ] {
            assert!(
                require_fee_original(&norito::encode_canonical(&changed).unwrap(), [1; 32])
                    .is_err()
            );
        }
    }
}
