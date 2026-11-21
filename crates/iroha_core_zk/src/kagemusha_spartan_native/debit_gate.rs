//! One exclusive wallet's volatile user approval and its short-lived Advance/lookup context.
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletSlotIdV1 as Slot, KagemushaWalletSlotStatusV1 as Status, OutputSourceV1,
};
use native::{DebitApprovalEvidence, DebitAuthorizationSource, DebitBoundary, PreparedOperation};
use zeroize::Zeroizing;

/// Exact originals are bounded by their canonical decoders. The private capsule is zeroized.
pub(super) struct Exact {
    slot: [u8; 32],
    checkpoint: [u8; 32],
    manifest: [u8; 32],
    wallet: [u8; 32],
    operation: [u8; 32],
    frame: Zeroizing<Vec<u8>>,
    review: Vec<u8>,
    original: Vec<u8>,
    signature: DebitApprovalEvidence,
}
impl Exact {
    pub(super) fn new(
        slot: &Slot,
        manifest: [u8; 32],
        checkpoint: [u8; 32],
        prepared: &PreparedOperation,
        original: Vec<u8>,
        signature: DebitApprovalEvidence,
    ) -> Result<Self, ProviderError> {
        let c = prepared.capsule();
        Ok(Self {
            slot: slot.0,
            checkpoint,
            manifest,
            wallet: *c.wallet(),
            operation: *c.operation_id(),
            frame: Zeroizing::new(c.encode().map_err(provider_error)?),
            review: prepared.review_bytes().map_err(provider_error)?,
            original,
            signature,
        })
    }
    fn same(&self, other: &Self) -> bool {
        self.slot == other.slot
            && self.manifest == other.manifest
            && self.wallet == other.wallet
            && self.operation == other.operation
            && self.frame == other.frame
            && self.review == other.review
            && self.original == other.original
    }
    fn matches(&self, binding: &BoundaryOriginals<'_>) -> bool {
        self.manifest == *binding.manifest
            && self.wallet == *binding.wallet
            && self.operation == *binding.operation
            && self.review == binding.review
            && self.frame.as_slice() == binding.frame
    }
}
struct Fresh {
    exact: Exact,
    message: [u8; 32],
}
struct BoundaryOriginals<'a> {
    manifest: &'a [u8; 32],
    wallet: &'a [u8; 32],
    operation: &'a [u8; 32],
    frame: &'a [u8],
    review: &'a [u8],
}
#[derive(Default)]
struct State {
    fresh: Option<Fresh>,
    candidate: Option<Exact>,
    awaiting_marker: Option<Exact>,
    selected: Option<Exact>,
}
#[derive(Default)]
pub(super) struct DebitGate(Mutex<State>);
pub(super) struct DebitWindow(Arc<DebitGate>);
impl Drop for DebitWindow {
    fn drop(&mut self) {
        let mut state = self.0.0.lock().unwrap_or_else(|p| p.into_inner());
        if state
            .candidate
            .as_ref()
            .zip(state.fresh.as_ref())
            .is_some_and(|(c, f)| c.same(&f.exact))
        {
            state.fresh = None;
        }
        state.candidate = None;
        state.awaiting_marker = None;
        state.selected = None;
    }
}
impl DebitGate {
    pub(super) fn accept(&self, exact: Exact, message: [u8; 32]) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).fresh = Some(Fresh { exact, message });
    }
    pub(super) fn retract(&self, slot: &[u8; 32], op: &[u8; 32], message: Option<&[u8; 32]>) {
        let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if state.fresh.as_ref().is_some_and(|f| {
            f.exact.slot == *slot
                && f.exact.operation == *op
                && message.is_none_or(|m| f.message == *m)
        }) {
            state.fresh = None;
        }
    }
    pub(super) fn supersede(&self) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).fresh = None;
    }
    fn fresh_key(
        &self,
        slot: &Slot,
        c: &RecoveryCapsule,
    ) -> Option<iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1> {
        self.0
            .lock()
            .ok()?
            .fresh
            .as_ref()
            .filter(|f| {
                f.exact.slot == slot.0
                    && f.exact.wallet == *c.wallet()
                    && f.exact.operation == *c.operation_id()
            })
            .map(|f| f.exact.signature.key)
    }
    fn open(self: &Arc<Self>, candidate: Option<Exact>, selected: Option<Exact>) -> DebitWindow {
        let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        state.candidate = candidate;
        state.awaiting_marker = selected;
        state.selected = None;
        DebitWindow(self.clone())
    }
}
impl DebitAuthorizationSource for DebitGate {
    fn evidence(
        &self,
        phase: DebitBoundary,
        prepared: &PreparedOperation,
        parameters: &AuthenticatedParameters,
    ) -> native::Result<DebitApprovalEvidence> {
        let capsule = prepared.capsule();
        let frame = Zeroizing::new(capsule.encode()?);
        let review = prepared.review_bytes()?;
        let binding = BoundaryOriginals {
            manifest: parameters.manifest_digest(),
            wallet: capsule.wallet(),
            operation: capsule.operation_id(),
            frame: &frame,
            review: &review,
        };
        self.0
            .lock()
            .map_err(|_| native::Error::Unavailable("debit custody lock"))?
            .evidence(phase, &binding)
    }
}
impl State {
    fn evidence(
        &mut self,
        phase: DebitBoundary,
        binding: &BoundaryOriginals<'_>,
    ) -> native::Result<DebitApprovalEvidence> {
        let missing =
            native::Error::AuthorityMismatch("protected approval for exact debit required");
        match phase {
            DebitBoundary::BeforeSelection => {
                let (Some(fresh), Some(candidate)) = (&self.fresh, &self.candidate) else {
                    return Err(missing);
                };
                if !fresh.exact.same(candidate) || !candidate.matches(binding) {
                    return Err(missing);
                }
                let fresh = self.fresh.take().ok_or(missing)?;
                self.awaiting_marker = self.candidate.take();
                self.selected = None;
                Ok(fresh.exact.signature)
            }
            DebitBoundary::AnchoredSelection(checkpoint) => {
                let found = self.awaiting_marker.as_ref().ok_or(missing)?;
                if checkpoint != found.checkpoint || !found.matches(binding) {
                    return Err(native::Error::AuthorityMismatch(
                        "durable Selected approval mismatch",
                    ));
                }
                let proof = found.signature.clone();
                self.selected = self.awaiting_marker.take();
                Ok(proof)
            }
            DebitBoundary::AfterSelection => {
                let selected = self.selected.as_ref().ok_or(missing)?;
                if !selected.matches(binding) {
                    return Err(native::Error::AuthorityMismatch(
                        "selected debit approval mismatch",
                    ));
                }
                Ok(selected.signature.clone())
            }
        }
    }
}
impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    /// Prepare only the exact independently recovered Selected capsule, and, on Advance, the
    /// currently authorized fresh candidate. The provider is exclusively owned for the full call.
    pub(super) fn debit_window(
        &mut self,
        slot: &Slot,
        requested: Option<&RecoveryCapsule>,
    ) -> Result<DebitWindow, ProviderError> {
        let source = self.provider.output_source(slot)?;
        let pending = matches!(&source, OutputSourceV1::Reconciled(Status::Pending(_)));
        let selected = match source {
            OutputSourceV1::Reconciled(Status::Pending(marker)) => {
                let capsule = self.provider.capsule_at_output_marker(slot, &marker)?;
                capsule.require_marker(marker.marker())?;
                let receipt = self.archived_debit(slot, &capsule, None)?;
                if self
                    .provider
                    .output_source(slot)?
                    .marker()
                    .map(|m| m.marker_file_digest())
                    != Some(marker.marker_file_digest())
                {
                    return Err(ProviderError::Invalid {
                        field: "selected debit marker changed",
                    });
                }
                receipt
            }
            _ => None,
        };
        let requested = requested.filter(|_| !pending);
        let candidate = match requested.and_then(|c| self.debit.fresh_key(slot, c).map(|k| (c, k)))
        {
            Some((c, key)) => self.archived_debit(slot, c, Some(key))?,
            None => None,
        };
        Ok(self.debit.open(candidate, selected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::kagemusha::{KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1};
    use native::DebitBoundary::*;
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    fn proof(n: u8) -> DebitApprovalEvidence {
        let signer = SigningKey::from_slice(&[n; 32]).unwrap();
        let signature: Signature = signer.sign(b"test only");
        DebitApprovalEvidence {
            slot: [1; 32],
            nonce: [n; 32],
            key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
                signer.verifying_key().to_sec1_point(false).as_bytes(),
            )
            .unwrap(),
            signature: KagemushaDeviceSignatureV1::from_der_normalizing_low_s(
                signature.to_der().as_bytes(),
            )
            .unwrap(),
        }
    }
    fn exact() -> Exact {
        Exact {
            manifest: [2; 32],
            checkpoint: [11; 32],
            slot: [1; 32],
            wallet: [3; 32],
            operation: [4; 32],
            frame: Zeroizing::new(vec![5; 32]),
            review: vec![6; 32],
            original: vec![7; 32],
            signature: proof(8),
        }
    }
    fn binding() -> BoundaryOriginals<'static> {
        BoundaryOriginals {
            manifest: &[2; 32],
            wallet: &[3; 32],
            operation: &[4; 32],
            frame: &[5; 32],
            review: &[6; 32],
        }
    }
    #[test]
    fn fresh_is_consumed_once_and_only_anchored_selection_can_finish() {
        let gate = Arc::new(DebitGate::default());
        gate.accept(exact(), [10; 32]);
        let mut wrong = exact();
        wrong.original[31] ^= 1;
        {
            let _window = gate.open(Some(wrong), None);
            assert!(
                gate.0
                    .lock()
                    .unwrap()
                    .evidence(BeforeSelection, &binding())
                    .is_err()
            );
        }
        let window = gate.open(Some(exact()), None);
        let mut state = gate.0.lock().unwrap();
        let mut changed = binding();
        changed.review = &[12; 32];
        assert!(state.evidence(BeforeSelection, &changed).is_err());
        assert_eq!(
            state.evidence(BeforeSelection, &binding()).unwrap().nonce,
            [8; 32]
        );
        assert!(state.evidence(BeforeSelection, &binding()).is_err());
        assert!(state.evidence(AfterSelection, &binding()).is_err());
        assert!(
            state
                .evidence(AnchoredSelection([12; 32]), &binding())
                .is_err()
        );
        changed = binding();
        changed.frame = &[13; 32];
        assert!(
            state
                .evidence(AnchoredSelection([11; 32]), &changed)
                .is_err()
        );
        assert_eq!(
            state
                .evidence(AnchoredSelection([11; 32]), &binding())
                .unwrap()
                .nonce,
            [8; 32]
        );
        assert!(state.evidence(AfterSelection, &changed).is_err());
        assert_eq!(
            state.evidence(AfterSelection, &binding()).unwrap().nonce,
            [8; 32]
        );
        drop(state);
        drop(window);
        assert!(
            gate.0
                .lock()
                .unwrap()
                .evidence(AfterSelection, &binding())
                .is_err()
        );
    }
    #[test]
    fn historical_original_cannot_select_and_retraction_cannot_release() {
        let gate = Arc::new(DebitGate::default());
        gate.accept(exact(), [10; 32]);
        gate.retract(&[1; 32], &[4; 32], Some(&[13; 32]));
        assert!(gate.0.lock().unwrap().fresh.is_some());
        gate.retract(&[1; 32], &[4; 32], Some(&[10; 32]));
        let _window = gate.open(Some(exact()), Some(exact()));
        let mut state = gate.0.lock().unwrap();
        assert!(state.evidence(BeforeSelection, &binding()).is_err());
        assert!(state.evidence(AfterSelection, &binding()).is_err());
        assert_eq!(
            state
                .evidence(AnchoredSelection([11; 32]), &binding())
                .unwrap()
                .nonce,
            [8; 32]
        );
        assert_eq!(
            state.evidence(AfterSelection, &binding()).unwrap().nonce,
            [8; 32]
        );
        assert!(state.evidence(BeforeSelection, &binding()).is_err());
    }
}
