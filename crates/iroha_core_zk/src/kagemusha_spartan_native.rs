//! Explicit future-artifact adapter for the canonical stock-phone Advance provider.
//!
//! The provider retains its existing exclusive custody, A0–A12 transaction,
//! redundant frozen copies, Selected/Released markers, hardware signing and
//! exact-byte retry. These frame owners supply the future proof backend without
//! changing current wallet installation selection or decoding another schema as
//! a fallback. This module is admitted only by its authenticated future artifact.

use iroha_data_model::kagemusha::{
    KagemushaDeviceSignatureV1, KagemushaWalletAnchoredTimeV1, KagemushaWalletMarkerStateV1,
    KagemushaWalletMarkerV1, KagemushaWalletMonotonicReadingV1, KagemushaWalletStateCommitmentV1,
    KagemushaWalletValidationErrorV1,
};
use zk_spartan::native::{
    self, AuthenticatedParameters, CompletionRecord, Engine, HardwareAdvanceReceipt,
    RecoveryCapsule,
};

use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletAdvanceCapsuleV1, KagemushaWalletCapacityClassV1,
    KagemushaWalletCompletionFrameV1, KagemushaWalletFrozenFrameV1, KagemushaWalletFsV1,
    KagemushaWalletPlatformV1, KagemushaWalletProviderErrorV1 as ProviderError,
    KagemushaWalletProviderV1, KagemushaWalletTransitionOwnerV1, KagemushaWalletUnavailableV1,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

type TimePair = (
    KagemushaWalletAnchoredTimeV1,
    KagemushaWalletMonotonicReadingV1,
);
#[derive(Default)]
struct TimeChoices {
    anchors: BTreeMap<[u8; 32], KagemushaWalletAnchoredTimeV1>,
    preparations: BTreeMap<([u8; 32], [u8; 32]), TimePair>,
}

/// Protected Native choices backed by the same real platform as Advance.
/// The coordinator supplies its retained signed anchor; callers never supply a
/// clock reading or an acceptance decision. Maps are bounded independently of
/// network activity and no mutex is held while reading the platform clock.
pub struct NativeTimeSource<P> {
    platform: Arc<P>,
    choices: Mutex<TimeChoices>,
}
impl<P: KagemushaWalletPlatformV1> NativeTimeSource<P> {
    /// Use the existing app-owned platform, including its native boot/clock adapter.
    #[must_use]
    pub fn new(platform: Arc<P>) -> Self {
        Self {
            platform,
            choices: Mutex::new(TimeChoices::default()),
        }
    }
    fn now(&self) -> native::Result<KagemushaWalletMonotonicReadingV1> {
        let boot_id = self
            .platform
            .boot_id()
            .map_err(|_| native::Error::Unavailable("native boot identity"))?;
        let monotonic_ms = self
            .platform
            .monotonic_ms()
            .map_err(|_| native::Error::Unavailable("native monotonic clock"))?;
        if boot_id == [0; 32] || self.platform.boot_id().ok() != Some(boot_id) {
            return Err(native::Error::Unavailable("stable native boot identity"));
        }
        Ok(KagemushaWalletMonotonicReadingV1 {
            boot_id,
            monotonic_ms,
        })
    }
    /// Capture choices from the exclusively held source's protected anchor.
    /// The engine subsequently binds its digest to the proven current state.
    ///
    /// # Errors
    /// Rejects foreign bindings, an unavailable clock or exhausted choice capacity.
    pub fn capture(
        &self,
        wallet: [u8; 32],
        operation_id: [u8; 32],
        protected_anchor: KagemushaWalletAnchoredTimeV1,
    ) -> native::Result<KagemushaWalletMonotonicReadingV1> {
        if wallet == [0; 32]
            || operation_id == [0; 32]
            || protected_anchor.anchor.body.wallet_id != wallet
        {
            return Err(native::Error::AuthorityMismatch("protected anchor wallet"));
        }
        let now = self.now()?;
        protected_anchor
            .interval_at(&now, u64::MAX)
            .map_err(|e| native::Error::Rejected(e.to_string()))?;
        let mut choices = self
            .choices
            .lock()
            .map_err(|_| native::Error::Unavailable("native time choices"))?;
        let key = (wallet, operation_id);
        if let Some((anchor, prior)) = choices.preparations.get(&key) {
            if *anchor != protected_anchor {
                return Err(native::Error::AuthorityMismatch(
                    "frozen native anchor choices",
                ));
            }
            return Ok(*prior);
        }
        if choices.preparations.len() >= 256
            || (!choices.anchors.contains_key(&wallet) && choices.anchors.len() >= 256)
        {
            return Err(native::Error::ResourceLimit);
        }
        choices.anchors.insert(wallet, protected_anchor);
        choices.preparations.insert(key, (protected_anchor, now));
        Ok(now)
    }
    /// Release transient choices after the completion is durably retained.
    ///
    /// # Errors
    /// Returns unavailability if the local owner lock was poisoned.
    pub fn release(&self, wallet: [u8; 32], operation_id: [u8; 32]) -> native::Result<()> {
        let mut choices = self
            .choices
            .lock()
            .map_err(|_| native::Error::Unavailable("native time choices"))?;
        choices.preparations.remove(&(wallet, operation_id));
        if !choices.preparations.keys().any(|(w, _)| *w == wallet) {
            choices.anchors.remove(&wallet);
        }
        Ok(())
    }
}
impl<P: KagemushaWalletPlatformV1> native::TimeSource for NativeTimeSource<P> {
    fn prepared(&self, wallet: &[u8; 32], operation_id: &[u8; 32]) -> native::Result<TimePair> {
        self.choices
            .lock()
            .map_err(|_| native::Error::Unavailable("native time choices"))?
            .preparations
            .get(&(*wallet, *operation_id))
            .copied()
            .ok_or(native::Error::Unavailable("retained native time choices"))
    }
    fn observe(&self, wallet: &[u8; 32]) -> native::Result<TimePair> {
        let anchor = self
            .choices
            .lock()
            .map_err(|_| native::Error::Unavailable("native time choices"))?
            .anchors
            .get(wallet)
            .copied()
            .ok_or(native::Error::Unavailable("protected native time anchor"))?;
        Ok((anchor, self.now()?))
    }
}

/// The existing provider with explicit future frame owners. Its journal,
/// hardware signer and marker sequence are unchanged.
pub type FutureProvider<F, P> = KagemushaWalletProviderV1<F, P, RecoveryCapsule, CompletionRecord>;

/// Transfer an already exclusive provider to authenticated future frame types.
/// This changes no persistent file, key, marker or authority. Installation must
/// have authenticated the future artifact before calling this adapter.
#[must_use]
pub fn into_future_frames<F: KagemushaWalletFsV1, P, C, R>(
    provider: KagemushaWalletProviderV1<F, P, C, R>,
    _parameters: &AuthenticatedParameters,
) -> FutureProvider<F, P> {
    provider.into_frame_types()
}

fn invalid_frame(_: native::Error) -> KagemushaWalletValidationErrorV1 {
    KagemushaWalletValidationErrorV1::InvalidField {
        field: "future wallet custody frame",
    }
}
fn provider_error(error: native::Error) -> ProviderError {
    match error {
        native::Error::Unavailable(_) | native::Error::ResourceLimit => {
            ProviderError::Unavailable(KagemushaWalletUnavailableV1::Platform(0))
        }
        _ => ProviderError::Invalid {
            field: "future wallet proof authority",
        },
    }
}
impl KagemushaWalletFrozenFrameV1 for RecoveryCapsule {
    const OBJECT: &'static str = "future recovery capsule";
    const MAX_FRAME_BYTES: usize = native::MAX_CUSTODY_FRAME_BYTES;
    type Binding = [u8; 32];
    fn encode_frame(&self) -> Result<Vec<u8>, KagemushaWalletValidationErrorV1> {
        self.encode().map_err(invalid_frame)
    }
    fn decode_frame(
        bytes: &[u8],
        scheme: &[u8; 32],
    ) -> Result<Self, KagemushaWalletValidationErrorV1> {
        Self::decode(bytes, scheme).map_err(invalid_frame)
    }
    fn frame_digest(bytes: &[u8]) -> [u8; 32] {
        Self::frame_digest(bytes)
    }
}
impl KagemushaWalletAdvanceCapsuleV1 for RecoveryCapsule {
    fn marker_binding(marker: &KagemushaWalletMarkerV1) -> [u8; 32] {
        marker.scheme_id
    }
    fn operation_id(&self) -> [u8; 32] {
        *self.operation_id()
    }
    fn head_marker_state(&self) -> Result<KagemushaWalletMarkerStateV1, ProviderError> {
        let bytes = self.encode().map_err(provider_error)?;
        Ok(KagemushaWalletMarkerStateV1::Head {
            sequence: self.sequence(),
            operation_id: *self.operation_id(),
            head: KagemushaWalletStateCommitmentV1 {
                value: *self.successor(),
            },
            capsule_digest: Self::frame_digest(&bytes),
            predecessor_capsule_digest: *self.predecessor_capsule_digest(),
        })
    }
    fn predecessor_commitment(&self) -> KagemushaWalletStateCommitmentV1 {
        KagemushaWalletStateCommitmentV1 {
            value: *self.predecessor(),
        }
    }
    fn bound_proof_digest(&self) -> Result<[u8; 32], ProviderError> {
        self.encode().map_err(provider_error)?;
        Ok(*self.proof_digest())
    }
    fn bootstrap_enrollment_marker(&self) -> Option<[u8; 32]> {
        self.enrollment_marker()
    }
    fn capacity_class(&self) -> KagemushaWalletCapacityClassV1 {
        KagemushaWalletCapacityClassV1::Growth
    }
    fn require_marker(&self, selected: &KagemushaWalletMarkerV1) -> Result<(), ProviderError> {
        selected.validate().map_err(|_| ProviderError::Invalid {
            field: "future marker",
        })?;
        if selected.scheme_id != *self.scheme()
            || selected.asset_digest != *self.asset()
            || selected.wallet_id != *self.wallet()
            || selected.payment_key.as_ref() != self.payment_key()
            || selected.state != self.head_marker_state()?
        {
            return Err(ProviderError::Invalid {
                field: "future capsule marker",
            });
        }
        Ok(())
    }
}
impl KagemushaWalletFrozenFrameV1 for CompletionRecord {
    const OBJECT: &'static str = "future completion";
    const MAX_FRAME_BYTES: usize = native::MAX_CUSTODY_FRAME_BYTES;
    type Binding = [u8; 32];
    fn encode_frame(&self) -> Result<Vec<u8>, KagemushaWalletValidationErrorV1> {
        self.encode().map_err(invalid_frame)
    }
    fn decode_frame(
        bytes: &[u8],
        wallet: &[u8; 32],
    ) -> Result<Self, KagemushaWalletValidationErrorV1> {
        Self::decode(bytes, wallet).map_err(invalid_frame)
    }
    fn frame_digest(bytes: &[u8]) -> [u8; 32] {
        Self::frame_digest(bytes)
    }
}
impl KagemushaWalletCompletionFrameV1 for CompletionRecord {
    fn marker_binding(marker: &KagemushaWalletMarkerV1) -> [u8; 32] {
        marker.wallet_id
    }
    fn operation_id(&self) -> [u8; 32] {
        *self.operation_id()
    }
    fn capsule_digest(&self) -> [u8; 32] {
        *self.capsule_digest()
    }
}

/// Adapter owning the exact authenticated parameter set and cryptographic
/// engine used both before selection and during restart reconciliation.
pub struct TransitionOwner<'a, E: Engine> {
    engine: &'a E,
    parameters: &'a AuthenticatedParameters,
}
impl<'a, E: Engine> TransitionOwner<'a, E> {
    /// Bind the provider's transition owner to already authenticated parameters.
    #[must_use]
    pub fn new(engine: &'a E, parameters: &'a AuthenticatedParameters) -> Self {
        Self { engine, parameters }
    }
}
impl<E: Engine> KagemushaWalletTransitionOwnerV1<RecoveryCapsule, CompletionRecord>
    for TransitionOwner<'_, E>
{
    fn before_selection(&self, capsule: &RecoveryCapsule) -> Result<(), ProviderError> {
        self.engine
            .before_selection(capsule, self.parameters)
            .map_err(provider_error)
    }
    fn receipt_body(
        &self,
        capsule: &RecoveryCapsule,
        digest: &[u8; 32],
    ) -> Result<Vec<u8>, ProviderError> {
        if RecoveryCapsule::frame_digest(&capsule.encode().map_err(provider_error)?) != *digest {
            return Err(ProviderError::Invalid {
                field: "future private capsule digest",
            });
        }
        let prepared = self
            .engine
            .resume(capsule, self.parameters)
            .map_err(provider_error)?;
        Ok(prepared.receipt_body().to_vec())
    }
    fn require_completion(
        &self,
        capsule: &RecoveryCapsule,
        record: &CompletionRecord,
    ) -> Result<(), ProviderError> {
        self.engine
            .verify_completion(capsule, record, self.parameters)
            .map_err(provider_error)?;
        Ok(())
    }
    fn assemble(
        &self,
        capsule: &RecoveryCapsule,
        digest: &[u8; 32],
        signature: &KagemushaDeviceSignatureV1,
    ) -> Result<CompletionRecord, ProviderError> {
        if RecoveryCapsule::frame_digest(&capsule.encode().map_err(provider_error)?) != *digest {
            return Err(ProviderError::Invalid {
                field: "future private capsule digest",
            });
        }
        let prepared = self
            .engine
            .resume(capsule, self.parameters)
            .map_err(provider_error)?;
        let raw: [u8; 64] = signature
            .as_ref()
            .try_into()
            .map_err(|_| ProviderError::Invalid {
                field: "future receipt signature",
            })?;
        let receipt =
            HardwareAdvanceReceipt::verify_selected(&prepared, prepared.receipt_body(), raw)
                .map_err(provider_error)?;
        let completed = self
            .engine
            .finalize(prepared, &receipt, self.parameters)
            .map_err(provider_error)?;
        if completed.completion().capsule_digest() != digest {
            return Err(ProviderError::Invalid {
                field: "future completed capsule",
            });
        }
        Ok(completed.completion().clone())
    }
}
