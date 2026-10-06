//! Bounded recovery-data reads from one committed State generation.
//!
//! A serialized receipt never authenticates funding. Offline consumers must independently
//! verify ordinary transaction finality and the complete Load relation before crediting value.
use super::{storage, *};
use crate::state::{StateReadOnly as _, StateView, TransactionsReadOnly as _};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::transaction::TransactionEntrypoint;

/// Read immutable Load receipt data from one consistent committed State view.
///
/// The source is private and cannot be a live instruction overlay. Reads check account,
/// scope, World row and committed transaction membership; they do not reconstruct a
/// historical certificate or produce a cryptographic finality capability.
pub struct CommittedLoadReceipts<'view, 'state> {
    view: &'view StateView<'state>,
    record_bytes: usize,
    decode_budget: norito::core::DecodeBudgetContext,
}
impl<'view, 'state> CommittedLoadReceipts<'view, 'state> {
    /// Borrow a committed global cut under finite record and cumulative decoding bounds.
    ///
    /// `decode_limits` compose with the request's active scope and remain cumulative
    /// across every read from this owner. Construction performs no history I/O.
    ///
    /// # Errors
    /// Refuses zero/unbounded limits, a non-global source, or an absent/inconsistent
    /// original execution tip. A State row or claimed height alone is insufficient.
    pub fn new(
        view: &'view StateView<'state>,
        record_bytes: usize,
        decode_limits: norito::DecodeLimits,
    ) -> Result<Self> {
        if [
            record_bytes,
            decode_limits.max_sequence_elements(),
            decode_limits.max_field_bytes(),
            decode_limits.max_total_elements(),
            decode_limits.max_total_allocated_bytes(),
            decode_limits.max_nesting_depth(),
        ]
        .into_iter()
        .any(|limit| limit == 0 || limit == usize::MAX)
        {
            return Err(Error::Binding);
        }
        let decode_budget = norito::core::DecodeBudgetContext::new(decode_limits);
        decode_budget.with(|| {
            if crate::sumeragi::lanes::routing::committed_root_scope(view.world())
                != Some(iroha_data_model::block::consensus::SumeragiRootScope::Global)
                || view.height() < 2
            {
                return Err(Error::NotCommitted);
            }
            let tip = view.native_execution_tip().ok_or(Error::NotCommitted)?;
            if tip.height() != u64::try_from(view.height()).map_err(|_| Error::Overflow)?
                || Some(tip.iroha_hash()) != view.latest_block_hash()
            {
                return Err(Error::NotCommitted);
            }
            Ok(())
        })?;
        Ok(Self {
            view,
            record_bytes,
            decode_budget,
        })
    }

    /// Recover the payer's original immutable receipt as transport data.
    ///
    /// # Errors
    /// Refuses another payer/scope, inconsistent or absent records, missing committed
    /// transaction membership and exhausted record/decode capacity. This method never
    /// returns `VerifiedKagemushaWalletLoadV1` or grants offline balance authority.
    pub fn receipt_for(
        &self,
        payer: &AccountId,
        scheme: &Digest,
        wallet: &Digest,
        request: &Digest,
    ) -> Result<KagemushaWalletLoadReceiptV1> {
        self.decode_budget
            .with(|| self.read_receipt(payer, scheme, wallet, request))
    }

    fn read_receipt(
        &self,
        payer: &AccountId,
        scheme: &Digest,
        wallet: &Digest,
        request: &Digest,
    ) -> Result<KagemushaWalletLoadReceiptV1> {
        let key = storage::issuance_key(*scheme, *wallet, *request);
        let bytes = self
            .view
            .world
            .kagemusha_wallet_ledger
            .get(&key)
            .ok_or(Error::Unavailable)?;
        if bytes.len() > self.record_bytes {
            return Err(Error::Unavailable);
        }
        validate_row(&key, bytes)?;
        let issuance: Issuance = storage::decode(bytes, bytes.len())?;
        if &issuance.payer != payer {
            return Err(Error::Binding);
        }
        let registration_key =
            storage::key(storage::REGISTRATION, *scheme, issuance.body.asset_digest);
        let registration_bytes = self
            .view
            .world
            .kagemusha_wallet_ledger
            .get(&registration_key)
            .ok_or(Error::Unavailable)?;
        if registration_bytes.len() > self.record_bytes {
            return Err(Error::Unavailable);
        }
        validate_row(&registration_key, registration_bytes)?;
        let registration: Registration = storage::decode(registration_bytes, self.record_bytes)?;
        registration.require(scheme, &issuance.body.asset_digest)?;
        if &registration.scheme.network_id != self.view.network_id().as_bytes() {
            return Err(Error::Binding);
        }
        let wallet_key = storage::key(storage::WALLET, *scheme, *wallet);
        let wallet_bytes = self
            .view
            .world
            .kagemusha_wallet_ledger
            .get(&wallet_key)
            .ok_or(Error::Unavailable)?;
        if wallet_bytes.len() > self.record_bytes.min(storage::WALLET_CAP) {
            return Err(Error::Unavailable);
        }
        validate_row(&wallet_key, wallet_bytes)?;
        let wallet: WalletRecord = storage::decode(wallet_bytes, storage::WALLET_CAP)?;
        if wallet.asset != issuance.body.asset_digest || wallet.next_load <= issuance.body.ordinal {
            return Err(Error::Binding);
        }
        let transaction = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed(
            issuance.body.transaction_hash,
        ));
        let height = self
            .view
            .transactions
            .get(&transaction)
            .ok_or(Error::NotCommitted)?;
        if u64::try_from(height.get()).map_err(|_| Error::Overflow)? != issuance.body.block_height {
            return Err(Error::Binding);
        }
        if height.get() < 2 || self.view.block_hashes.get(height.get() - 1).is_none() {
            return Err(Error::NotCommitted);
        }
        Ok(issuance.body)
    }
}
