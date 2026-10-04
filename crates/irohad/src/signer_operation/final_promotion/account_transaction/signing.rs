//! Signature release requires challenges issued after protected key I/O, not prebuilt observations.

use std::time::Duration;

use iroha_core::{
    query::{
        final_promotion_account_custody::observation::{
            FinalPromotionAccountCheckExpectedV1, FinalPromotionAccountCheckFloorV1,
            PreparedFinalPromotionAccountCheckV1, begin_final_promotion_account_check_v1,
            final_promotion_native_signed_entry_frame_v1,
        },
        final_promotion_authority::observation::{
            FinalPromotionCheckExpectedV1, FinalPromotionCheckFloorV1,
            PreparedFinalPromotionCheckV1, begin_final_promotion_check_v1,
        },
    },
    state::State,
};
use iroha_crypto::Signature;
use iroha_data_model::{
    isi::sorafs::MutateSorafsFinalPromotionAccountCustody,
    sorafs::final_promotion_account_custody::FinalPromotionAccountCustodyActionV1,
    transaction::{SignedTransaction, TransactionBuilder, TransactionEntrypoint},
};

use super::*;

/// Exact immutable account payload exposed only after native receipt and account authority agree.
/// The configured provider must select the independently pinned binding and sign these exact bytes.
/// There is no public constructor and no private key, credential or decoded authority input.
pub struct FinalPromotionAccountKeyRequestV1<'a> {
    prepared: &'a PreparedFinalPromotionAccountTransactionV1,
    message: &'a [u8],
}
impl FinalPromotionAccountKeyRequestV1<'_> {
    /// Independently reviewed complete role-15 software/optional-hardware binding.
    #[must_use]
    pub const fn binding(&self) -> &SignerCustodyBindingV1 {
        &self.prepared.binding
    }
    /// Exact full payload, including native action, fee intent, timing and admission intent.
    #[must_use]
    pub const fn payload(&self) -> &TransactionPayload {
        &self.prepared.payload
    }
    /// The ordinary transaction prehash to sign; this is distinct from the account Check digest.
    #[must_use]
    pub const fn signing_message(&self) -> &[u8] {
        self.message
    }
}

/// Account approval retaining the original receipt phase and immutable completion receipt.
pub struct AuthorizedFinalPromotionAccountTransactionV1 {
    prepared: PreparedFinalPromotionAccountTransactionV1,
    before_account: VerifiedFinalPromotionAccountCheckV1,
}

impl PreparedFinalPromotionAccountTransactionV1 {
    /// Join a fresh executed account Current Check to this exact transaction and receipt observer.
    ///
    /// Both UTC intervals must be independently sampled after floor persistence. Neither this
    /// method nor the returned owner renews either original Check lifetime.
    ///
    /// # Errors
    /// Rejects another account, payload, observer, control binding, original floor or expired phase.
    pub fn authorize(
        self,
        account_check: VerifiedFinalPromotionAccountCheckV1,
        receipt_time: FinalPromotionEligibilityTimeIntervalV1,
        account_time: FinalPromotionAccountEligibilityTimeIntervalV1,
    ) -> Result<AuthorizedFinalPromotionAccountTransactionV1, Error> {
        validate_account_check(&self, &account_check)?;
        let floor = self.receipt_check.applied_floor();
        let actual = account_check.original_floor();
        if (actual.height, actual.block_hash, actual.context_id)
            != (floor.height, floor.block_hash, floor.context_id)
        {
            return Err(Error::Authority);
        }
        self.recheck(receipt_time)?;
        account_check
            .recheck_use_interval(account_time)
            .map_err(|_| Error::Authority)?;
        Ok(AuthorizedFinalPromotionAccountTransactionV1 {
            prepared: self,
            before_account: account_check,
        })
    }
}

impl AuthorizedFinalPromotionAccountTransactionV1 {
    /// Invoke the configured key exactly once, then issue a fresh native account challenge.
    ///
    /// The signature remains private. The returned Check must be signed by the independent
    /// observer, submitted normally and verified against its retained actual State before advancing.
    /// `sample_time` must supply qualified time before and after provider I/O; no clock is inferred.
    /// A failed or ambiguous provider call consumes this owner and is never retried here.
    ///
    /// # Errors
    /// Returns semantic errors for expiry, receipt changes, substituted signatures or failed Check
    /// preparation, and `LocalCapacity` for local canonical-frame refusal. This boundary does not
    /// return a reusable signed continuation after refusal.
    pub fn sign_with(
        self,
        state: Arc<State>,
        max_elapsed: Duration,
        mut sample_time: impl FnMut() -> Result<
            (
                FinalPromotionEligibilityTimeIntervalV1,
                FinalPromotionAccountEligibilityTimeIntervalV1,
            ),
            Error,
        >,
        provider: impl FnOnce(&FinalPromotionAccountKeyRequestV1<'_>) -> Result<Signature, Error>,
    ) -> Result<
        (
            PendingFinalPromotionAccountSignatureV1,
            PreparedFinalPromotionAccountCheckV1,
        ),
        Error,
    > {
        if max_elapsed.is_zero()
            || max_elapsed > Duration::from_secs(60)
            || state.network_id_ref().as_bytes() != &self.prepared.binding.network_id
            || state.chain_id_ref().to_string() != self.prepared.binding.chain_id
        {
            return Err(Error::Binding);
        }
        let (receipt_time, account_time) = sample_time()?;
        self.prepared.recheck(receipt_time)?;
        self.before_account
            .recheck_use_interval(account_time)
            .map_err(|_| Error::Authority)?;
        let builder = TransactionBuilder::from_payload(self.prepared.payload.clone())
            .map_err(|_| Error::Payload)?;
        let message = builder.payload_hash_bytes();
        let signature = provider(&FinalPromotionAccountKeyRequestV1 {
            prepared: &self.prepared,
            message: &message,
        })
        .map_err(|_| Error::Provider)?;
        let (receipt_time, account_time) = sample_time()?;
        self.prepared.recheck(receipt_time)?;
        self.before_account
            .recheck_use_interval(account_time)
            .map_err(|_| Error::Authority)?;
        signature
            .verify(&self.prepared.binding.public_key, &message)
            .map_err(|_| Error::Provider)?;
        let signed = builder.build_with_signature(signature);
        validate_native_frame(&TransactionEntrypoint::External(signed.clone()))?;
        if signed.payload() != &self.prepared.payload {
            return Err(Error::Payload);
        }
        let floor = self.before_account.applied_floor();
        let before = self.before_account.instruction();
        let next = begin_final_promotion_account_check_v1(
            Arc::clone(&state),
            FinalPromotionAccountCheckExpectedV1 {
                binding: self.prepared.binding.clone(),
                observer: self.before_account.observer().clone(),
                expected_account: self.prepared.payload.authority.clone(),
                transaction_payload_digest: self.prepared.payload_digest,
                control_revision: before.expected_control_revision,
                control_digest: before.expected_control_digest,
                floor: FinalPromotionAccountCheckFloorV1 {
                    height: floor.height,
                    block_hash: floor.block_hash,
                    context_id: floor.context_id,
                },
            },
            max_elapsed,
        )
        .map_err(|_| Error::Authority)?;
        let expected_account_check = next.instruction().clone();
        Ok((
            PendingFinalPromotionAccountSignatureV1 {
                authorized: self,
                state,
                signed,
                expected_account_check,
                max_elapsed,
            },
            next,
        ))
    }
}

/// Private signature awaiting the exact account challenge generated after key use.
/// It exposes neither a signature nor a transaction nor a replacement challenge API.
pub struct PendingFinalPromotionAccountSignatureV1 {
    authorized: AuthorizedFinalPromotionAccountTransactionV1,
    state: Arc<State>,
    signed: SignedTransaction,
    expected_account_check: MutateSorafsFinalPromotionAccountCustody,
    max_elapsed: Duration,
}
impl PendingFinalPromotionAccountSignatureV1 {
    /// Verify the post-key account Check and then issue the final fresh receipt-phase Check.
    ///
    /// # Errors
    /// Rejects prebuilt/substituted account challenges, drift, expiry or receipt changes.
    pub fn check_account(
        self,
        after_account: VerifiedFinalPromotionAccountCheckV1,
        receipt_time: FinalPromotionEligibilityTimeIntervalV1,
        account_time: FinalPromotionAccountEligibilityTimeIntervalV1,
    ) -> Result<
        (
            PendingFinalPromotionAccountReleaseV1,
            PreparedFinalPromotionCheckV1,
        ),
        Error,
    > {
        let prepared = &self.authorized.prepared;
        if after_account.instruction() != &self.expected_account_check {
            return Err(Error::Authority);
        }
        validate_account_check(prepared, &after_account)?;
        prepared.recheck(receipt_time)?;
        after_account
            .recheck_use_interval(account_time)
            .map_err(|_| Error::Authority)?;
        self.authorized
            .before_account
            .recheck_use_interval(account_time)
            .map_err(|_| Error::Authority)?;
        let original = prepared.receipt_check.instruction();
        let FinalPromotionAuthorityActionV1::Check(original_check) = &original.action else {
            return Err(Error::Authority);
        };
        let floor = after_account.applied_floor();
        let next = begin_final_promotion_check_v1(
            self.state,
            FinalPromotionCheckExpectedV1 {
                binding: prepared
                    .receipt_check
                    .snapshot()
                    .control
                    .policy
                    .binding
                    .clone(),
                observer: prepared.receipt_check.observer().clone(),
                expected_operator: prepared.receipt_check.expected_operator().clone(),
                request: original_check.request,
                subject: original_check.subject.clone(),
                control_revision: original.expected_control_revision,
                control_digest: original.expected_control_digest,
                floor: FinalPromotionCheckFloorV1 {
                    height: floor.height,
                    block_hash: floor.block_hash,
                    context_id: floor.context_id,
                },
            },
            self.max_elapsed,
        )
        .map_err(|_| Error::Authority)?;
        let expected_receipt_check = next.instruction().clone();
        Ok((
            PendingFinalPromotionAccountReleaseV1 {
                authorized: self.authorized,
                signed: self.signed,
                after_account,
                expected_receipt_check,
            },
            next,
        ))
    }
}

/// Private signature awaiting the exact receipt challenge generated after the post-key account Check.
pub struct PendingFinalPromotionAccountReleaseV1 {
    authorized: AuthorizedFinalPromotionAccountTransactionV1,
    signed: SignedTransaction,
    after_account: VerifiedFinalPromotionAccountCheckV1,
    expected_receipt_check: MutateSorafsFinalPromotionAuthority,
}
impl PendingFinalPromotionAccountReleaseV1 {
    /// Release the exact signature-bound transaction after both fresh authority phases succeed.
    ///
    /// # Errors
    /// Rejects substituted/prebuilt receipt challenges, revoked authority, expiry or receipt drift.
    pub fn release(
        self,
        after_receipt: VerifiedFinalPromotionCheckV1,
        receipt_time: FinalPromotionEligibilityTimeIntervalV1,
        account_time: FinalPromotionAccountEligibilityTimeIntervalV1,
    ) -> Result<SignedFinalPromotionAccountTransactionV1, Error> {
        if after_receipt.instruction() != &self.expected_receipt_check
            || after_receipt.observer() != self.authorized.prepared.receipt_check.observer()
            || after_receipt.expected_operator()
                != self.authorized.prepared.receipt_check.expected_operator()
        {
            return Err(Error::Authority);
        }
        let signed = SignedFinalPromotionAccountTransactionV1 {
            authorized: self.authorized,
            signed: self.signed,
            after_account: self.after_account,
            after_receipt,
        };
        signed.recheck(receipt_time, account_time)?;
        Ok(signed)
    }
}

/// Exact signed envelope retaining all original Checks and the private completion receipt lease.
/// Keep this owner alive through submission and ambiguous-outcome reconciliation; do not rebuild
/// a transaction with new timing, fees, metadata, admission intent, signature or operation identity.
pub struct SignedFinalPromotionAccountTransactionV1 {
    authorized: AuthorizedFinalPromotionAccountTransactionV1,
    signed: SignedTransaction,
    after_account: VerifiedFinalPromotionAccountCheckV1,
    after_receipt: VerifiedFinalPromotionCheckV1,
}
impl SignedFinalPromotionAccountTransactionV1 {
    /// Exact successfully verified Current Check that selected this Reserve, for private
    /// pre-transport journaling only. Its historical signed frame grants no renewed authority.
    pub(in crate::signer_operation::final_promotion) const fn original_current_check(
        &self,
    ) -> &VerifiedFinalPromotionCheckV1 {
        &self.authorized.prepared.receipt_check
    }

    fn recheck(
        &self,
        receipt_time: FinalPromotionEligibilityTimeIntervalV1,
        account_time: FinalPromotionAccountEligibilityTimeIntervalV1,
    ) -> Result<(), Error> {
        self.authorized.prepared.recheck(receipt_time)?;
        self.authorized
            .before_account
            .recheck_use_interval(account_time)
            .map_err(|_| Error::Authority)?;
        self.after_account
            .recheck_use_interval(account_time)
            .map_err(|_| Error::Authority)?;
        self.after_receipt
            .recheck_use_interval(receipt_time)
            .map_err(|_| Error::Authority)
    }

    /// Borrow the exact signed payload for its initial ordinary native submission after immediate checks.
    ///
    /// # Errors
    /// Rejects expired original observations, backward time or any change to the pinned receipt.
    pub fn for_submission(
        &self,
        receipt_time: FinalPromotionEligibilityTimeIntervalV1,
        account_time: FinalPromotionAccountEligibilityTimeIntervalV1,
    ) -> Result<&SignedTransaction, Error> {
        self.recheck(receipt_time, account_time)?;
        Ok(&self.signed)
    }

    /// Exact already-signed envelope for read-only recovery of an ambiguous submission.
    /// Historical bytes grant no new submission, spending, key use or renewed authority.
    #[must_use]
    pub const fn reconciliation_transaction(&self) -> &SignedTransaction {
        &self.signed
    }
}

fn validate_account_check(
    prepared: &PreparedFinalPromotionAccountTransactionV1,
    account: &VerifiedFinalPromotionAccountCheckV1,
) -> Result<(), Error> {
    account.ensure_live().map_err(|_| Error::Authority)?;
    let FinalPromotionAccountCustodyActionV1::Check(check) = &account.instruction().action else {
        return Err(Error::Authority);
    };
    if account.snapshot().control.policy.binding != prepared.binding
        || account.observer() != prepared.receipt_check.observer()
        || check.expected_account != prepared.payload.authority
        || check.transaction_payload_digest != prepared.payload_digest
    {
        return Err(Error::Authority);
    }
    Ok(())
}

// This fixed service boundary distinguishes local refusal, but does not preserve a retry owner.
// TODO: Retain the pre-return signed custody through operational recovery without re-signing.
fn validate_native_frame(entry: &TransactionEntrypoint) -> Result<(), Error> {
    use iroha_core::execution_attempt::ExecutionAttemptError;
    final_promotion_native_signed_entry_frame_v1(entry)
        .map(|_| ())
        .map_err(|error| match error {
            ExecutionAttemptError::Deferred(_) => Error::LocalCapacity,
            ExecutionAttemptError::Rejected(_) => Error::Payload,
        })
}

#[cfg(test)]
mod frame_tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        Level, NetworkId, block::BlockHeader, isi::Log, transaction::FeePaymentIntent,
    };

    #[test]
    fn native_frame_refusal_is_operational_while_forged_and_missing_ttl_are_payload_rejections() {
        let key = KeyPair::try_from_seed(vec![0x35; 32], Algorithm::Ed25519).unwrap();
        let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x49; 32]),
        ));
        let mut builder = TransactionBuilder::new(
            network,
            AccountId::new(key.public_key().clone()),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "canonical frame".to_owned())]);
        builder.set_ttl(Duration::from_secs(60));
        let signed = builder.try_sign(key.private_key()).unwrap();
        let entry = TransactionEntrypoint::External(signed.clone());
        assert_eq!(validate_native_frame(&entry), Ok(()));
        let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 128);
        assert_eq!(
            norito::core::with_decode_limits_scope(limits, || validate_native_frame(&entry)),
            Err(Error::LocalCapacity)
        );
        // A refused attempt neither alters the borrowed input nor releases any signed result.
        assert_eq!(entry, TransactionEntrypoint::External(signed.clone()));
        assert_eq!(validate_native_frame(&entry), Ok(()));
        let forged = TransactionBuilder::from_payload(signed.payload().clone())
            .unwrap()
            .build_with_signature(Signature::from_bytes(&[1; 64]));
        assert_eq!(
            validate_native_frame(&TransactionEntrypoint::External(forged)),
            Err(Error::Payload)
        );
        let mut missing_ttl = TransactionBuilder::from_payload(signed.payload().clone()).unwrap();
        missing_ttl.set_ttl(Duration::ZERO);
        let signature =
            Signature::try_new(key.private_key(), &missing_ttl.payload_hash_bytes()).unwrap();
        let missing_ttl = missing_ttl.build_with_signature(signature);
        missing_ttl.verify_signature().unwrap();
        assert_eq!(
            validate_native_frame(&TransactionEntrypoint::External(missing_ttl)),
            Err(Error::Payload)
        );
    }
}
