//! Direct signed-time exchange under the exclusive wallet and its selected archive.
use super::custody_archive::storage_error;
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletSlotIdV1 as Slot, KagemushaWalletSlotStatusV1,
};
use crate::kagemusha_wallet_state_v1::{self as state, DirectTimeExchangeV1};
use iroha_data_model::kagemusha::{
    KagemushaWalletSignerCertificateV1, KagemushaWalletTimeAnchorV1, KagemushaWalletTimeIntervalV1,
};

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    fn current_time_policy(
        &mut self,
        slot: &Slot,
    ) -> Result<([u8; 32], u64, [u8; 32]), ProviderError> {
        self.require_account(slot, None)?;
        let _debit_window = self.debit_window(slot, None)?;
        let candidate = self.current_reconciliation_engine(slot)?;
        let owner = TransitionOwner::new(&candidate, &self.parameters);
        let KagemushaWalletSlotStatusV1::Released(record) =
            self.provider.reconcile(slot, &owner)?
        else {
            return Err(ProviderError::Invalid {
                field: "future direct time released head",
            });
        };
        let capsule =
            self.provider
                .current_capsule(slot)?
                .ok_or(ProviderError::UnavailableCustodyData {
                    object: "future time policy capsule",
                })?;
        capsule.require_marker(record.marker())?;
        if capsule.scheme() != &self.scheme.scheme_id() {
            return Err(ProviderError::Invalid {
                field: "future time policy scheme",
            });
        }
        let prepared = self
            .engine_for_capsule(slot, &capsule)?
            .resume(&capsule, &self.parameters)
            .map_err(provider_error)?;
        self.require_account(slot, Some(prepared.credential()))?;
        let (limit, held) = prepared.successor_time_policy().map_err(provider_error)?;
        Ok((*capsule.wallet(), limit, held))
    }

    /// Capture the request nonce and sleep-inclusive monotonic clock in the admitted slot.
    ///
    /// # Errors
    /// Requires a released authenticated head, an available clock and fresh native entropy.
    pub fn begin_direct_time_exchange(
        &mut self,
        slot: &Slot,
    ) -> Result<DirectTimeExchangeV1, ProviderError> {
        let (wallet, maximum, _) = self.current_time_policy(slot)?;
        DirectTimeExchangeV1::start(*self.provider.scheme_id(), wallet, maximum, || {
            self.time
                .now()
                .map_err(|e| state::Error::Provider(provider_error(e)))
        })
        .map_err(storage_error)
    }

    /// Authenticate the single-use response and durably checkpoint its local observations.
    ///
    /// # Errors
    /// Refuses a foreign signer, wrong nonce, reboot, clock regression, late response or storage failure.
    pub fn finish_direct_time_exchange(
        &mut self,
        slot: &Slot,
        exchange: DirectTimeExchangeV1,
        anchor: KagemushaWalletTimeAnchorV1,
        certificate: &KagemushaWalletSignerCertificateV1,
    ) -> Result<(), ProviderError> {
        let received = self.time.now().map_err(provider_error)?;
        let (wallet, maximum, _) = self.current_time_policy(slot)?;
        let scope = (*self.provider.scheme_id(), wallet);
        let anchored = exchange
            .complete_for_wallet(anchor, certificate, &self.scheme, scope, maximum, received)
            .map_err(storage_error)?;
        let (expected, mut checkpoint) = self.custody_checkpoint(slot, wallet)?;
        self.provider
            .with_archive(slot, |archive| checkpoint.retain(archive, &anchored))?;
        self.publish_custody_checkpoint(slot, expected, &checkpoint)
    }

    /// Retire a captured Send interval after abandoning preparation before a review exists.
    ///
    /// # Errors
    /// Requires the authorized wallet slot.
    pub fn abandon_send_capture(
        &self,
        slot: &Slot,
        operation: &[u8; 32],
    ) -> Result<(), ProviderError> {
        self.require_account(slot, None)?;
        let wallet = self.authorized_wallet_id(slot)?;
        self.time.release(&wallet, operation);
        Ok(())
    }

    /// Freeze a time-controlled Send's native interval from its committed anchor and durable observations.
    /// A signed Refresh alone cannot supply a direct exchange. Controls-off Send skips this method.
    ///
    /// # Errors
    /// Missing observations, a changed boot, invalid policy, or exhausted review capacity refuse capture.
    pub fn capture_send_time(
        &mut self,
        slot: &Slot,
        operation: [u8; 32],
    ) -> Result<KagemushaWalletTimeIntervalV1, ProviderError> {
        let (wallet, maximum, held) = self.current_time_policy(slot)?;
        let (_, checkpoint) = self.custody_checkpoint(slot, wallet)?;
        let anchored = self.provider.with_archive(slot, |archive| {
            checkpoint.observation(archive, &held, maximum)
        })?;
        let reading = self
            .time
            .capture(wallet, operation, anchored, maximum)
            .map_err(provider_error)?;
        anchored
            .interval_at(&reading, maximum)
            .map_err(|_| ProviderError::Invalid {
                field: "future captured time interval",
            })
    }
}
