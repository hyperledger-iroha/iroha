//! Authenticated durable staging for canonical sender payments.

use super::{guard::sealed, send::SendSplitPlanV1, *};
use zeroize::Zeroize as _;

const PAYMENT_OUTBOX_KEY_DOMAIN: &[u8] = b"iroha:offline-cash:v1:payment-outbox-key";
const PAYMENT_OUTBOX_PUBLICATION_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:payment-outbox-publication";

/// Stable failures from the authenticated durable payment outbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthenticatedPaymentOutboxErrorV1 {
    /// Durable storage is currently unavailable.
    Unavailable,
    /// No byte-identical staged record exists for the requested key.
    Missing,
    /// Another staged or published record already owns the deterministic key.
    Conflict,
    /// The durable record failed its authenticated-storage checks.
    Corrupt,
}

/// Deterministic key for the sole payment staged by one prepared send.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PaymentOutboxKeyV1 {
    device_id: Digest,
    hardware_policy_id: Digest,
    wallet_binding: Digest,
    context_digest: Digest,
    request_digest: Digest,
    send_transition_digest: Digest,
    guard_challenge_digest: Digest,
    digest: Digest,
}

impl PaymentOutboxKeyV1 {
    pub(super) fn new(balance: &BalanceOwnerV1, plan: &SendSplitPlanV1) -> Self {
        let digest = digest_framed(
            PAYMENT_OUTBOX_KEY_DOMAIN,
            &[
                &balance.guard_device_id,
                &balance.hardware_policy_id,
                &balance.wallet_binding,
                &balance.context.digest,
                &plan.request_digest,
                &plan.statement.transition_digest,
                &plan.challenge.digest,
            ],
        );
        Self {
            device_id: balance.guard_device_id,
            hardware_policy_id: balance.hardware_policy_id,
            wallet_binding: balance.wallet_binding,
            context_digest: balance.context.digest,
            request_digest: plan.request_digest,
            send_transition_digest: plan.statement.transition_digest,
            guard_challenge_digest: plan.challenge.digest,
            digest,
        }
    }

    pub(crate) const fn device_id(&self) -> Digest {
        self.device_id
    }

    pub(crate) const fn hardware_policy_id(&self) -> Digest {
        self.hardware_policy_id
    }

    pub(crate) const fn wallet_binding(&self) -> Digest {
        self.wallet_binding
    }

    pub(crate) const fn context_digest(&self) -> Digest {
        self.context_digest
    }

    pub(crate) const fn request_digest(&self) -> Digest {
        self.request_digest
    }

    pub(crate) const fn send_transition_digest(&self) -> Digest {
        self.send_transition_digest
    }

    pub(crate) const fn guard_challenge_digest(&self) -> Digest {
        self.guard_challenge_digest
    }

    pub(crate) const fn digest(&self) -> Digest {
        self.digest
    }
}

/// Hardware-authorized transition from staged bytes to publishable bytes.
#[derive(PartialEq, Eq)]
pub(crate) struct PaymentOutboxPublicationV1 {
    key: PaymentOutboxKeyV1,
    payment_digest: Digest,
    intent_epoch: u64,
    intent_digest: Digest,
    authorization_digest: Digest,
}

impl fmt::Debug for PaymentOutboxPublicationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PaymentOutboxPublicationV1")
            .field("key", &self.key.digest)
            .field("payment_digest", &self.payment_digest)
            .field("intent_epoch", &self.intent_epoch)
            .field("intent_digest", &self.intent_digest)
            .field("authorization", &"[REDACTED]")
            .finish()
    }
}

impl Drop for PaymentOutboxPublicationV1 {
    fn drop(&mut self) {
        self.payment_digest.zeroize();
        self.intent_epoch.zeroize();
        self.intent_digest.zeroize();
        self.authorization_digest.zeroize();
    }
}

impl PaymentOutboxPublicationV1 {
    pub(crate) const fn key(&self) -> &PaymentOutboxKeyV1 {
        &self.key
    }

    pub(crate) const fn payment_digest(&self) -> Digest {
        self.payment_digest
    }

    pub(crate) const fn intent_epoch(&self) -> u64 {
        self.intent_epoch
    }

    pub(crate) const fn intent_digest(&self) -> Digest {
        self.intent_digest
    }

    pub(crate) const fn authorization_digest(&self) -> Digest {
        self.authorization_digest
    }
}

/// Authenticated record returned by the durable outbox only to Core.
#[must_use]
pub(crate) struct AuthenticatedPaymentOutboxRecordV1 {
    key: PaymentOutboxKeyV1,
    payment_digest: Digest,
    canonical_payment: Zeroizing<Vec<u8>>,
    publication_digest: Option<Digest>,
}

impl fmt::Debug for AuthenticatedPaymentOutboxRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedPaymentOutboxRecordV1")
            .field("key", &self.key.digest)
            .field("payment_digest", &self.payment_digest)
            .field("canonical_payment", &"[REDACTED]")
            .field("published", &self.publication_digest.is_some())
            .finish()
    }
}

impl AuthenticatedPaymentOutboxRecordV1 {
    /// Construct the exact record returned by a sealed durable adapter.
    pub(crate) fn new(
        key: PaymentOutboxKeyV1,
        payment_digest: Digest,
        canonical_payment: Vec<u8>,
        publication_digest: Option<Digest>,
    ) -> Self {
        Self {
            key,
            payment_digest,
            canonical_payment: Zeroizing::new(canonical_payment),
            publication_digest,
        }
    }

    pub(super) const fn key(&self) -> &PaymentOutboxKeyV1 {
        &self.key
    }

    pub(super) const fn payment_digest(&self) -> Digest {
        self.payment_digest
    }

    pub(super) fn canonical_payment(&self) -> &[u8] {
        &self.canonical_payment
    }

    pub(super) const fn publication_digest(&self) -> Option<Digest> {
        self.publication_digest
    }
}

impl Drop for AuthenticatedPaymentOutboxRecordV1 {
    fn drop(&mut self) {
        self.payment_digest.zeroize();
        if let Some(publication) = self.publication_digest.as_mut() {
            publication.zeroize();
        }
        self.publication_digest = None;
    }
}

/// Sealed authenticated, durable sender-payment outbox.
///
/// Staging must atomically persist and authenticate the exact key, payment
/// digest, and canonical bytes. Repeating a stage is permitted only when all
/// three are byte-identical. `publish_payment_or_recover` must durably bind the
/// supplied hardware authorization before returning the record, and recovery
/// must never return an unpublished record through the published method.
/// Implementations must zeroize transient plaintext byte buffers after durable
/// authenticated storage has consumed them.
pub(crate) trait AuthenticatedPaymentOutboxBackendV1: sealed::Sealed {
    /// Stage or recover one byte-identical unpublished payment.
    fn stage_payment_or_recover(
        &self,
        key: &PaymentOutboxKeyV1,
        payment_digest: Digest,
        canonical_payment: &[u8],
    ) -> Result<(), AuthenticatedPaymentOutboxErrorV1>;

    /// Recover only the authenticated digest of an unpublished payment.
    ///
    /// This method must not materialize, clone, or return canonical payment
    /// bytes. Byte-bearing records are available only from the two published
    /// methods below.
    fn recover_staged_payment_digest(
        &self,
        key: &PaymentOutboxKeyV1,
    ) -> Result<Digest, AuthenticatedPaymentOutboxErrorV1>;

    /// Mark an exact staged record publishable after hardware journal CAS.
    fn publish_payment_or_recover(
        &self,
        authorization: &PaymentOutboxPublicationV1,
    ) -> Result<AuthenticatedPaymentOutboxRecordV1, AuthenticatedPaymentOutboxErrorV1>;

    /// Recover only an already-published, byte-identical record.
    fn recover_published_payment(
        &self,
        key: &PaymentOutboxKeyV1,
    ) -> Result<AuthenticatedPaymentOutboxRecordV1, AuthenticatedPaymentOutboxErrorV1>;
}

pub(super) fn authorize_publication(
    key: PaymentOutboxKeyV1,
    authorization: &HardwareIntentAuthorizationOwnerV1,
    payment_digest: Digest,
) -> Result<PaymentOutboxPublicationV1, StateTransitionErrorV1> {
    if key.device_id != authorization.challenge.device_id
        || key.hardware_policy_id != authorization.challenge.hardware_policy_id
        || key.wallet_binding != authorization.challenge.wallet_binding
        || key.context_digest != authorization.challenge.context_digest
        || key.request_digest == [0; 32]
        || key.guard_challenge_digest != authorization.challenge.intent_digest
        || authorization.challenge.kind != HardwareIntentKindV1::SendPublished
        || !intent_authorization_matches(authorization, &authorization.challenge, payment_digest)
    {
        return Err(StateTransitionErrorV1::HardwareIntentMismatch);
    }
    let epoch_bytes = authorization.epoch.to_le_bytes();
    let authorization_digest = digest_framed(
        PAYMENT_OUTBOX_PUBLICATION_DOMAIN,
        &[
            &key.digest,
            &payment_digest,
            &epoch_bytes,
            &authorization.challenge.digest,
            &authorization.capability[..],
        ],
    );
    Ok(PaymentOutboxPublicationV1 {
        key,
        payment_digest,
        intent_epoch: authorization.epoch,
        intent_digest: authorization.challenge.digest,
        authorization_digest,
    })
}
