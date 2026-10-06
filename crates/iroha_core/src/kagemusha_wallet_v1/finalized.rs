//! Retained committed-State source for voucher release. A live instruction overlay cannot
//! construct this capability; the reader requires the node's committed `StateView`.
use super::{storage, *};
use crate::{
    execution_attempt::ExecutionAttemptError,
    state::{StateView, TransactionsReadOnly as _},
    sumeragi::certified_chain::{CertifiedChain, QcVerification},
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::transaction::TransactionEntrypoint;
use iroha_data_model::{
    query::error::QueryExecutionFail, sumeragi::finality::NativeFinalityLimits,
};

/// Borrowed committed ledger view used to release finalized load bodies to the authorizer.
/// The State view retains the World and transaction-membership cut together. This owner is
/// not constructed from a caller's body, claimed height, ordinary filesystem or boolean verdict.
pub struct FinalizedLedger<'view, 'state> {
    pub(super) view: &'view StateView<'state>,
    pub(super) record_bytes: usize,
    pub(super) decode_limits: norito::DecodeLimits,
}
impl<'view, 'state> FinalizedLedger<'view, 'state> {
    /// Retain a source-verified global finalized cut under finite original-history limits.
    ///
    /// # Errors
    /// Refuses absent/invalid CommitQC custody, a non-global cut, an uncommitted source,
    /// exhausted history capacity, or invalid limits. Membership alone is insufficient.
    pub fn new(view: &'view StateView<'state>, limits: NativeFinalityLimits) -> Result<Self> {
        limits.validate().map_err(|_| Error::Binding)?;
        if crate::sumeragi::lanes::routing::committed_root_scope(view.world())
            != Some(iroha_data_model::block::consensus::SumeragiRootScope::Global)
            || view.height() < 2
        {
            return Err(Error::NotFinalized);
        }
        let height = std::num::NonZeroUsize::new(view.height()).ok_or(Error::NotFinalized)?;
        let mut frames = u64::try_from(limits.block_count).map_err(|_| Error::Overflow)?;
        let mut remaining = u64::try_from(limits.journal_bytes).map_err(|_| Error::Overflow)?;
        let mut admit = |count: u64, bytes: u64| {
            let refusal = || {
                ExecutionAttemptError::<QueryExecutionFail>::Deferred(
                    ivm::error::ExecutionDeferral::CanonicalHistoryCapacity.into(),
                )
            };
            if bytes > limits.block_bytes as u64 {
                return Err(refusal());
            }
            frames = frames.checked_sub(count).ok_or_else(refusal)?;
            remaining = remaining.checked_sub(bytes).ok_or_else(refusal)?;
            Ok(())
        };
        let decode_limits = limits.decode_limits().map_err(|_| Error::Binding)?;
        norito::core::with_decode_limits_scope(decode_limits, || {
            let chain = CertifiedChain::new_with_source_admission(view, &mut admit)
                .map_err(source_error)?;
            let certified = chain
                .certified_from_execution(height, &mut admit)
                .map_err(source_error)?;
            if certified.verification() != QcVerification::Verified {
                return Err(Error::NotFinalized);
            }
            Ok(())
        })?;
        Ok(Self {
            view,
            record_bytes: limits.block_bytes,
            decode_limits,
        })
    }

    /// Read the payer's exact finalized record, refusing over-budget bytes before decoding.
    /// The unsigned body is issuance evidence, not a signed load voucher.
    ///
    /// # Errors
    /// Rejects another payer, absent/unfinalized records, invalid membership or insufficient
    /// caller capacity. It never converts an unavailable source into absence or a voucher.
    pub fn issuance_for(
        &self,
        payer: &AccountId,
        scheme: &Digest,
        wallet: &Digest,
        request: &Digest,
        maximum: usize,
    ) -> Result<Issuance> {
        let issuance = self.read_issuance(scheme, wallet, request, maximum)?;
        if &issuance.payer != payer {
            return Err(Error::Binding);
        }
        Ok(issuance)
    }
    pub(super) fn read_issuance(
        &self,
        scheme: &Digest,
        wallet: &Digest,
        request: &Digest,
        maximum: usize,
    ) -> Result<Issuance> {
        norito::core::with_decode_limits_scope(self.decode_limits, || {
            self.read_issuance_inner(scheme, wallet, request, maximum)
        })
    }
    fn read_issuance_inner(
        &self,
        scheme: &Digest,
        wallet: &Digest,
        request: &Digest,
        maximum: usize,
    ) -> Result<Issuance> {
        let key = storage::issuance_key(*scheme, *wallet, *request);
        let bytes = self
            .view
            .world
            .kagemusha_wallet_ledger
            .get(&key)
            .ok_or(Error::Unavailable)?;
        if bytes.len() > maximum.min(self.record_bytes) {
            return Err(Error::Unavailable);
        }
        validate_row(&key, bytes)?;
        let issuance: Issuance = storage::decode(bytes, bytes.len())?;
        let pending = PendingPublication::from_issuance(&issuance);
        let pending_key = pending.key(*scheme, issuance.body.authorizer_certificate);
        let retained = self
            .view
            .world
            .kagemusha_wallet_ledger
            .get(&pending_key)
            .map(|bytes| {
                validate_row(&pending_key, bytes)?;
                storage::decode::<PendingPublication>(bytes, super::pending::CAP)
            })
            .transpose()?;
        if retained != issuance.voucher.is_none().then_some(pending) {
            return Err(Error::Binding);
        }
        // Resolve both identities from this same verified generation. An old issuance remains
        // bound to its historical signer even if another valid signer is retained later.
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
        let certificate_key = storage::key(
            storage::CERTIFICATE,
            *scheme,
            issuance.body.authorizer_certificate,
        );
        let certificate_bytes = self
            .view
            .world
            .kagemusha_wallet_ledger
            .get(&certificate_key)
            .ok_or(Error::Unavailable)?;
        validate_row(&certificate_key, certificate_bytes)?;
        let certificate: KagemushaWalletSignerCertificateV1 =
            storage::decode(certificate_bytes, KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1)?;
        certificate.verify_role(
            &registration.scheme,
            KagemushaWalletSignerRoleV1::LoadAuthorization,
        )?;
        if let Some(bytes) = &issuance.voucher {
            KagemushaWalletLoadVoucherV1::decode_canonical(bytes, scheme)?
                .verify(&registration.scheme, &certificate)?;
        }
        let transaction = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed(
            issuance.body.transaction_hash,
        ));
        let height = self
            .view
            .transactions
            .get(&transaction)
            .ok_or(Error::NotFinalized)?;
        if u64::try_from(height.get()).map_err(|_| Error::Overflow)? != issuance.body.block_height {
            return Err(Error::Binding);
        }
        Ok(issuance)
    }
}

impl FinalizedSource for FinalizedLedger<'_, '_> {
    fn issuance(&self, scheme: &Digest, wallet: &Digest, request: &Digest) -> Result<Issuance> {
        self.read_issuance(scheme, wallet, request, usize::MAX)
    }
}

fn source_error<T>(error: ExecutionAttemptError<T>) -> Error {
    match error {
        ExecutionAttemptError::Deferred(_) => Error::Unavailable,
        ExecutionAttemptError::Rejected(_) => Error::NotFinalized,
    }
}
