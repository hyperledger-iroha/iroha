//! Local wallet projections from protected custody and the verified recursive head.
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletLookupV1 as Lookup, KagemushaWalletMarkerRecordV1 as Marker,
    KagemushaWalletRetainedV1, KagemushaWalletSlotIdV1 as Slot,
    KagemushaWalletSlotStatusV1 as Status,
};
use iroha_data_model::sumeragi_finality::SumeragiCommitCheckpointV1;

pub(super) type ReleasedPair = (
    Arc<native::PreparedOperation>,
    Arc<native::CompletedOperation>,
);

/// A proof result lives only on the owner that authenticated its parameters.
/// Each read still has to obtain the capsule and completion from protected custody.
pub(super) struct VerifiedRelease {
    slot: Slot,
    completion_digest: [u8; 32],
    epoch: Option<SumeragiCommitCheckpointV1>,
    pair: ReleasedPair,
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    pub(super) fn verify_retained_release(
        &mut self,
        slot: &Slot,
        capsule: &RecoveryCapsule,
        retained: &KagemushaWalletRetainedV1<CompletionRecord>,
    ) -> Result<ReleasedPair, ProviderError> {
        // This reads the selected, protected finality checkpoint even for a repeated Load.
        let (engine, epoch) = self.engine_with_epoch_checkpoint(
            slot,
            *capsule.wallet(),
            capsule.load_epoch_selector().map_err(provider_error)?,
        )?;
        if let Some(held) = &self.verified_release
            && held.slot == *slot
            && held.epoch == epoch
            && held.completion_digest == retained.completion_digest
            && held.pair.0.capsule() == capsule
        {
            return Ok(held.pair.clone());
        }
        self.verified_release = None;
        let (prepared, completed) = engine
            .verify_released_operation(capsule, &retained.record, &self.parameters)
            .map_err(provider_error)?;
        let pair = (Arc::new(prepared), Arc::new(completed));
        self.verified_release = Some(VerifiedRelease {
            slot: *slot,
            completion_digest: retained.completion_digest,
            epoch,
            pair: pair.clone(),
        });
        Ok(pair)
    }

    pub(super) fn released_operation(
        &mut self,
        slot: &Slot,
        marker: &Marker,
    ) -> Result<ReleasedPair, ProviderError> {
        let capsule = self.provider.capsule_at_output_marker(slot, marker)?;
        self.require_account_wallet(capsule.wallet())?;
        capsule.require_marker(marker.marker())?;
        let Lookup::Retained(retained) = self.provider.lookup(slot, capsule.operation_id())? else {
            return Err(ProviderError::UnavailableCustodyData {
                object: "released completion",
            });
        };
        let pair = self.verify_retained_release(slot, &capsule, &retained)?;
        self.require_account(slot, Some(pair.0.credential()))?;
        Ok(pair)
    }

    /// Return the next Load ordinal from the wallet's current released state.
    ///
    /// # Errors
    /// Requires a monetary-authorized owner, available custody and the complete matching
    /// release proof and private opening. A pending transition has no committed new ordinal.
    pub fn next_load_ordinal(&mut self, slot: &Slot) -> Result<u128, ProviderError> {
        self.require_account(slot, None)?;
        let marker = match self.provider.status(slot)? {
            Status::Released(marker) => marker,
            Status::Terminal(_) => return Err(ProviderError::Terminal),
            _ => {
                return Err(ProviderError::Invalid {
                    field: "next Load ordinal requires a released wallet",
                });
            }
        };
        let (prepared, released) = self.released_operation(slot, &marker)?;
        released
            .next_load_ordinal(&prepared)
            .map_err(provider_error)
    }

    /// Inspect current offline custody without selecting or signing an operation.
    /// A pending selection carries its retry ID and a released balance includes
    /// the verified lineage's burn adjustment.
    ///
    /// # Errors
    /// Returns the provider's typed refusal for unavailable storage, an unauthorized
    /// account, terminal custody, an invalid proof or inconsistent private opening.
    pub fn balance_snapshot(&mut self, slot: &Slot) -> Result<Vec<u8>, ProviderError> {
        self.require_account(slot, None)?;
        let wallet = self.authorized_wallet_id(slot)?;
        let status = self.provider.status(slot)?;
        let (stage, marker) = match &status {
            Status::Enrollment(marker) => (1, marker),
            Status::Pending(marker) => (2, marker),
            Status::Released(marker) => (3, marker),
            Status::Terminal(_) => return Err(ProviderError::Terminal),
            _ => {
                return Err(ProviderError::Invalid {
                    field: "balance custody marker",
                });
            }
        };
        let current = marker.marker();
        if current.wallet_id != wallet || current.scheme_id != *self.provider.scheme_id() {
            return Err(ProviderError::Invalid {
                field: "balance custody scope",
            });
        }
        let scale = self.account_asset_scale(slot, &current.asset_digest)?;
        let mut out = Vec::with_capacity(196);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&[stage, scale]);
        out.extend_from_slice(&wallet);
        out.extend_from_slice(&current.asset_digest);
        match status {
            Status::Pending(marker) => {
                let capsule = self.provider.capsule_at_output_marker(slot, &marker)?;
                capsule.require_marker(marker.marker())?;
                out.extend_from_slice(capsule.operation_id());
            }
            Status::Released(marker) => {
                let (prepared, completed) = self.released_operation(slot, &marker)?;
                let (balance, burned, spendable) =
                    completed.balance(&prepared).map_err(provider_error)?;
                let capsule = prepared.capsule();
                out.extend_from_slice(capsule.operation_id());
                out.extend_from_slice(capsule.successor());
                for word in [capsule.sequence(), balance, burned, spendable] {
                    out.extend_from_slice(&word.to_le_bytes());
                }
            }
            _ => {}
        }
        Ok(out)
    }
}
