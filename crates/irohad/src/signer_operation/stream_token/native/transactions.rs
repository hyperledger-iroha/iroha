//! Configured software transaction custody and bounded same-envelope native submission.
use super::*;
use crate::runtime_credential::load_bounded_runtime_credential_v1;
use iroha_core::{queue::Queue, state::StateReadOnlyWithTransactions, tx::AcceptedTransaction};
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair, PublicKey};
use iroha_data_model::transaction::{
    FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionEntrypoint,
};
use std::time::Instant;

/// The original transaction deadline begins before software signing and never renews.
pub(super) struct NativeSignedTransactionV1 {
    pub(super) transaction: SignedTransaction,
    started: Instant,
}

pub(super) struct NativeTransactionsV1 {
    state: Arc<State>,
    queue: Arc<Queue>,
    operator: AccountId,
    operator_key: KeyPair,
    observer: AccountId,
    observer_key: KeyPair,
    fee_payment: FeePaymentIntent,
    timeout: Duration,
}
impl NativeTransactionsV1 {
    pub(super) fn new(
        state: Arc<State>,
        queue: Arc<Queue>,
        operator: AccountId,
        operator_key: KeyPair,
        observer_key: KeyPair,
        fee_payment: FeePaymentIntent,
        timeout: Duration,
    ) -> Result<Self, SignerOperationErrorV1> {
        if operator.try_signatory() != Some(operator_key.public_key())
            || operator_key.public_key() == observer_key.public_key()
            || operator_key.public_key().algorithm() != Algorithm::Ed25519
            || observer_key.public_key().algorithm() != Algorithm::Ed25519
            || fee_payment.validate().is_err()
            || timeout.is_zero()
            || timeout > Duration::from_secs(60)
        {
            return Err(SignerOperationErrorV1::InvalidOperation);
        }
        Ok(Self {
            state,
            queue,
            operator,
            observer: AccountId::new(observer_key.public_key().clone()),
            operator_key,
            observer_key,
            fee_payment,
            timeout,
        })
    }
    pub(super) fn operator(&self) -> AccountId {
        self.operator.clone()
    }
    pub(super) fn observer(&self) -> AccountId {
        self.observer.clone()
    }
    pub(super) fn sign(
        &self,
        instruction: &MutateSorafsStreamTokenAuthority,
        observer: bool,
    ) -> Result<NativeSignedTransactionV1, SignerOperationErrorV1> {
        let started = Instant::now();
        let is_check = matches!(instruction.request.action, Action::Check(_));
        if is_check != observer
            || instruction.request.network_id != *self.state.network_id_ref().as_bytes()
        {
            return Err(SignerOperationErrorV1::InvalidOperation);
        }
        if let Action::Check(check) = &instruction.request.action {
            if check.expected_observer != self.observer || check.expected_operator != self.operator
            {
                return Err(SignerOperationErrorV1::InvalidOperation);
            }
        }
        let (authority, key) = if observer {
            (&self.observer, &self.observer_key)
        } else {
            (&self.operator, &self.operator_key)
        };
        let mut builder = TransactionBuilder::new(
            *self.state.network_id_ref(),
            authority.clone(),
            self.fee_payment.clone(),
        )
        .with_instructions([instruction.clone()]);
        builder.set_ttl(self.timeout);
        let transaction = builder
            .try_sign(key.private_key())
            .map_err(|_| SignerOperationErrorV1::ProviderUnavailable)?;
        if started.elapsed() >= self.timeout {
            return Err(SignerOperationErrorV1::StateUnavailable);
        }
        Ok(NativeSignedTransactionV1 {
            transaction,
            started,
        })
    }
    pub(super) fn submit_and_wait(
        &self,
        attempt: &NativeSignedTransactionV1,
    ) -> Result<(), SignerOperationErrorV1> {
        let started = attempt.started;
        if started.elapsed() >= self.timeout {
            return Err(SignerOperationErrorV1::StateUnavailable);
        }
        let signed = &attempt.transaction;
        let hash = TransactionEntrypoint::External(signed.clone()).hash();
        let (drift, limits) = self.state.transaction_admission_limits();
        let accepted = AcceptedTransaction::accept(
            signed.clone(),
            self.state.network_id_ref(),
            drift,
            limits,
            self.state.crypto().as_ref(),
        )
        .map_err(|_| SignerOperationErrorV1::StateUnavailable)?;
        let plan = self
            .queue
            .route_plan_with_state(&accepted, &self.state)
            .map_err(|_| SignerOperationErrorV1::StateUnavailable)?;
        if started.elapsed() >= self.timeout {
            return Err(SignerOperationErrorV1::StateUnavailable);
        }
        // A queue failure can be ambiguous; never sign or resubmit a replacement envelope here.
        self.queue
            .push_with_lane_with_state_and_routing_plan(accepted, &self.state, plan)
            .map_err(|_| SignerOperationErrorV1::StateUnavailable)?;
        while started.elapsed() < self.timeout {
            {
                let view = self.state.view();
                if view.has_entrypoint(hash) {
                    let height = view.block_hashes().len() as u64;
                    if let Some(hash) = view.latest_block_hash()
                        && iroha_core::query::signer_finality::verify_signer_finality_v1(
                            &view,
                            height,
                            *hash.as_ref(),
                        )
                        .is_ok()
                    {
                        return Ok(());
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err(SignerOperationErrorV1::StateUnavailable)
    }
    pub(super) fn sign_observation(
        &self,
        body: &sorafs_manifest::signer::stream_token_evidence::SignerStreamTokenStateObservationBodyV1,
    ) -> Result<[u8; 64], SignerOperationErrorV1> {
        let payload = body
            .signing_payload()
            .map_err(|_| SignerOperationErrorV1::InvalidOperation)?;
        Signature::try_new(self.observer_key.private_key(), &payload)
            .map_err(|_| SignerOperationErrorV1::ProviderUnavailable)?
            .payload()
            .try_into()
            .map_err(|_| SignerOperationErrorV1::ProviderUnavailable)
    }
}
pub(super) fn load_key(
    path: &Path,
    expected: &PublicKey,
) -> Result<KeyPair, SignerOperationErrorV1> {
    let bytes = load_bounded_runtime_credential_v1(path, 2, 16 * 1024 + 256)
        .map_err(|_| SignerOperationErrorV1::ProviderUnavailable)?;
    let text = bytes
        .strip_suffix(b"\n")
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .ok_or(SignerOperationErrorV1::ProviderUnavailable)?;
    let private: ExposedPrivateKey = text
        .parse()
        .map_err(|_| SignerOperationErrorV1::ProviderUnavailable)?;
    let canonical = Zeroizing::new(
        private
            .try_to_multihash_string()
            .map_err(|_| SignerOperationErrorV1::ProviderUnavailable)?,
    );
    if canonical.as_str() != text {
        return Err(SignerOperationErrorV1::ProviderUnavailable);
    }
    let key = KeyPair::from_private_key(private.0)
        .map_err(|_| SignerOperationErrorV1::ProviderUnavailable)?;
    if key.public_key() != expected {
        return Err(SignerOperationErrorV1::ProviderUnavailable);
    }
    Ok(key)
}
