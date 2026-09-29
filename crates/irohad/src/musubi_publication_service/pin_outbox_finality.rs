//! Finalized State/Kura read side for the native signed pin-outbox high-water.
//!
//! This authenticates one current State record against its exact successful signed advance in
//! canonical Kura history. It does not open the local outbox or submit pin transactions; the
//! complete local inventory must still be compared before those paths can become operational.
//! A coordinated rollback of both local State and Kura still needs an independently current
//! finalized network checkpoint before effects; a self-consistent old local view is insufficient.
use super::{MusubiPublicationPrivateServiceContextV1, finality::validate_finalized_block_wire};
use iroha_core::state::{
    State, StateQueryView, StateReadOnly as _, WorldReadOnly, WorldStateSnapshot as _,
};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::SignedBlock,
    isi::musubi::AdvanceMusubiPinOutboxV1,
    musubi::MusubiPinOutboxHighWaterV1,
    transaction::{Executable, TransactionEntrypoint},
};
use mv::storage::StorageReadOnly as _;
use std::{num::NonZeroUsize, sync::Arc};

/// Closed result of reading a current finalized pin-outbox high-water.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiPublicationPinOutboxHighWaterReadErrorV1 {
    /// The state record names a height beyond this node's coherent finalized view.
    LocallyAhead,
    /// State, Kura, signed instruction, execution output, or lineage is inconsistent.
    Invalid,
}
impl core::fmt::Display for MusubiPublicationPinOutboxHighWaterReadErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::LocallyAhead => "Musubi pin-outbox high-water is ahead of local finality",
            Self::Invalid => "Musubi pin-outbox high-water finality is invalid",
        })
    }
}
impl std::error::Error for MusubiPublicationPinOutboxHighWaterReadErrorV1 {}

/// A coherent, locally finalized State/Kura tip and its current signer lineage.
///
/// This is a local observation only. An independently current network checkpoint and a
/// deployment-sealed rollback floor must match this tip before stock signing or Queue effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusubiPublicationPinOutboxLocalAnchorV1 {
    /// Exact genesis-derived network identity used to verify this local tip.
    pub network_id: NetworkId,
    /// Height of the exact finalized local tip.
    pub tip_height: u64,
    /// Canonical block-header hash at that height.
    pub tip_block_hash: [u8; 32],
    /// Current signed pin-outbox lineage, if one has been initialized on chain.
    /// Absence does not authorize provisioning a replacement signer or outbox.
    pub high_water: Option<MusubiPinOutboxHighWaterV1>,
}
// TODO: Match this exact local tip to an independently current network checkpoint and
// deployment-sealed monotonic floor before any signer or Queue capability is exposed.

/// Daemon-owned reader for one exact network and its committed State/Kura handles.
#[derive(Clone)]
pub struct MusubiPublicationPinOutboxHighWaterReaderV1 {
    network_id: NetworkId,
    state: Arc<State>,
}
impl core::fmt::Debug for MusubiPublicationPinOutboxHighWaterReaderV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MusubiPublicationPinOutboxHighWaterReaderV1")
            .field("network_id", &self.network_id)
            .finish_non_exhaustive()
    }
}
impl MusubiPublicationPrivateServiceContextV1 {
    /// Bind the high-water reader to this daemon's exact finalized State/Kura owner.
    #[must_use]
    pub fn finalized_pin_outbox_high_water_reader(
        &self,
    ) -> MusubiPublicationPinOutboxHighWaterReaderV1 {
        MusubiPublicationPinOutboxHighWaterReaderV1 {
            network_id: self.network_id(),
            state: self.state(),
        }
    }
}
impl MusubiPublicationPinOutboxHighWaterReaderV1 {
    pub(super) const fn network_id(&self) -> &NetworkId {
        &self.network_id
    }

    /// Bind an explicit finalized State handle to one genesis-derived network.
    ///
    /// # Errors
    /// Rejects a State handle belonging to another network.
    pub fn new(
        network_id: NetworkId,
        state: Arc<State>,
    ) -> Result<Self, MusubiPublicationPinOutboxHighWaterReadErrorV1> {
        if state.network_id_ref() != &network_id {
            return Err(MusubiPublicationPinOutboxHighWaterReadErrorV1::Invalid);
        }
        Ok(Self { network_id, state })
    }

    /// Read the current publisher high-water and authenticate its exact successful advance and
    /// this State/Kura pair's finalized tip.
    ///
    /// Absence is returned only for a State table with no record for this publisher. No caller
    /// may interpret absence as permission to replace a potentially lost signed pin intent.
    ///
    /// # Errors
    /// Rejects a mismatched record, missing finality artifact, failed transaction, or mutation.
    pub fn read_current(
        &self,
        pin_authority: &AccountId,
    ) -> Result<Option<MusubiPinOutboxHighWaterV1>, MusubiPublicationPinOutboxHighWaterReadErrorV1>
    {
        Ok(self.read_current_anchor(pin_authority)?.high_water)
    }

    /// Read the current signer lineage together with an authenticated local finality tip.
    ///
    /// State and Kura must have the same durable height throughout the read. The tip's exact
    /// canonical block and cryptographically verified V2 finality artifact are checked even when
    /// this publisher has no high-water record. This local tip is an input to, not a replacement
    /// for, an independently current network checkpoint.
    ///
    /// # Errors
    /// Rejects missing or inconsistent tip finality, a mismatched State/Kura height, and invalid
    /// signed signer lineage.
    pub fn read_current_anchor(
        &self,
        pin_authority: &AccountId,
    ) -> Result<
        MusubiPublicationPinOutboxLocalAnchorV1,
        MusubiPublicationPinOutboxHighWaterReadErrorV1,
    > {
        self.read_current_anchor_in_view(pin_authority, &self.state.query_view())
    }

    fn read_current_anchor_in_view(
        &self,
        pin_authority: &AccountId,
        view: &StateQueryView<'_>,
    ) -> Result<
        MusubiPublicationPinOutboxLocalAnchorV1,
        MusubiPublicationPinOutboxHighWaterReadErrorV1,
    > {
        use MusubiPublicationPinOutboxHighWaterReadErrorV1::{Invalid, LocallyAhead};
        let tip_height = view.block_hashes().len();
        let tip_number = NonZeroUsize::new(tip_height).ok_or(LocallyAhead)?;
        let tip_height_u64 = u64::try_from(tip_height).map_err(|_| Invalid)?;
        self.require_matching_durable_height(view, tip_height)?;
        let tip_hash = view
            .block_hashes()
            .get(tip_height - 1)
            .copied()
            .ok_or(Invalid)?;
        let tip_block = view.kura().get_block(tip_number).ok_or(Invalid)?;
        if !validate_finalized_block_wire(
            view,
            &self.network_id,
            tip_height_u64,
            tip_hash,
            &tip_block,
        ) {
            return Err(Invalid);
        }
        let high_water = view
            .world()
            .musubi_pin_outbox_high_waters()
            .get(pin_authority)
            .map(|record| {
                record.validate().map_err(|_| Invalid)?;
                if record.network_id != self.network_id || &record.pin_authority != pin_authority {
                    return Err(Invalid);
                }
                let height = usize::try_from(record.recorded_at_height)
                    .ok()
                    .and_then(NonZeroUsize::new)
                    .ok_or(Invalid)?;
                if height.get() > tip_height {
                    return Err(LocallyAhead);
                }
                let canonical_hash = view
                    .block_hashes()
                    .get(height.get() - 1)
                    .copied()
                    .ok_or(Invalid)?;
                let block = view.kura().get_block(height).ok_or(Invalid)?;
                if !validate_finalized_block_wire(
                    view,
                    &self.network_id,
                    record.recorded_at_height,
                    canonical_hash,
                    &block,
                ) || !validate_advance_transaction(record, &block)
                {
                    return Err(Invalid);
                }
                Ok(record.clone())
            })
            .transpose()?;
        self.require_matching_durable_height(view, tip_height)?;
        Ok(MusubiPublicationPinOutboxLocalAnchorV1 {
            network_id: self.network_id,
            tip_height: tip_height_u64,
            tip_block_hash: *tip_hash.as_ref(),
            high_water,
        })
    }

    fn require_matching_durable_height(
        &self,
        view: &StateQueryView<'_>,
        expected_height: usize,
    ) -> Result<(), MusubiPublicationPinOutboxHighWaterReadErrorV1> {
        use MusubiPublicationPinOutboxHighWaterReadErrorV1::{Invalid, LocallyAhead};
        let durable_height = view
            .kura()
            .exact_durable_blocks_count()
            .map_err(|_| Invalid)?;
        if durable_height != expected_height {
            return Err(LocallyAhead);
        }
        Ok(())
    }
}

pub(super) fn validate_advance_transaction(
    record: &MusubiPinOutboxHighWaterV1,
    block: &SignedBlock,
) -> bool {
    if block.header().height().get() != record.recorded_at_height
        || block.validate_output_merkle_cache().is_err()
    {
        return false;
    }
    let mut found = false;
    for (input_index, entrypoint) in block.network_entrypoints().enumerate() {
        let transaction = match entrypoint {
            TransactionEntrypoint::External(transaction) => transaction,
            TransactionEntrypoint::SealedReveal(reveal) => {
                if *reveal.signed_transaction().hash().as_ref() == record.transaction_hash {
                    return false;
                }
                continue;
            }
            TransactionEntrypoint::SealedCommitment(_) => continue,
        };
        if *transaction.hash().as_ref() != record.transaction_hash {
            continue;
        }
        let Some((_, output)) = u32::try_from(input_index)
            .ok()
            .and_then(|index| block.network_output_at(index))
        else {
            return false;
        };
        if found
            || output.result.is_err()
            || transaction.verify_signature().is_err()
            || transaction.network_id() != Some(&record.network_id)
            || transaction.authority() != &record.pin_authority
        {
            return false;
        }
        found = true;
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return false;
        };
        let [instruction] = instructions.as_ref() else {
            return false;
        };
        let Some(advance) = instruction
            .as_any()
            .downcast_ref::<AdvanceMusubiPinOutboxV1>()
        else {
            return false;
        };
        if advance
            .recorded_high_water(record.recorded_at_height, record.transaction_hash)
            .map_or(true, |expected| expected != *record)
        {
            return false;
        }
    }
    found
}
