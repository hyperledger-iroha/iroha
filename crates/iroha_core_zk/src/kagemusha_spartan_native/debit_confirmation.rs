//! Local protected user-presence records for future debit reviews. Records are private
//! custody data, separate from payment-key receipts, public proofs and carrier frames.
use super::custody_archive::MAX_DEBIT_APPROVAL_BYTES;
use super::*;
use crate::kagemusha_wallet_advance_v1::KagemushaWalletSlotIdV1 as Slot;
use iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1;
use rand::rand_core::TryRng as _;
use sha2::{Digest as _, Sha256};

/// Result of retaining user-presence evidence for an exact reviewed capsule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebitConfirmationRetentionV1 {
    /// This call published the first verified original.
    Retained,
    /// A prior call already published an authenticated original for this exact scope.
    AlreadyRetained,
}

fn invalid(field: &'static str) -> ProviderError {
    ProviderError::Invalid { field }
}

/// All fields have fixed width. The review digest covers Native's complete original,
/// including debit amounts, both heads and destinations. The capsule is the provider's
/// own full frame digest, independent of where the authorization record is retained.
#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.spartan.DebitConfirmationScopeV1")]
struct Scope {
    manifest: [u8; 32],
    slot: [u8; 32],
    wallet: [u8; 32],
    operation: [u8; 32],
    capsule: [u8; 32],
    review: [u8; 32],
    payment_key: KagemushaDevicePublicKeyV1,
}

impl Scope {
    fn message(&self, key: &KagemushaDevicePublicKeyV1, nonce: &[u8; 32]) -> [u8; 32] {
        native::DebitApprovalEvidence::message(
            &self.manifest,
            &self.slot,
            &self.wallet,
            &self.operation,
            &self.capsule,
            &self.review,
            &self.payment_key,
            key,
            nonce,
        )
    }
}

/// One outstanding prompt per exclusive wallet owner. Rust retains the key, nonce and scope.
pub(super) struct DebitConfirmationChallengeV1 {
    scope: Scope,
    key: KagemushaDevicePublicKeyV1,
    nonce: [u8; 32],
}

impl DebitConfirmationChallengeV1 {
    fn signing_message(&self) -> [u8; 32] {
        self.scope.message(&self.key, &self.nonce)
    }

    fn cancel_exact(
        pending: &mut Option<Self>,
        slot: &[u8; 32],
        operation: &[u8; 32],
        message: &[u8; 32],
    ) {
        if pending.as_ref().is_some_and(|held| {
            &held.scope.slot == slot
                && &held.scope.operation == operation
                && &held.signing_message() == message
        }) {
            *pending = None;
        }
    }

    fn finish(
        &self,
        scope: &Scope,
        current_key: &KagemushaDevicePublicKeyV1,
        signature_der: &[u8],
    ) -> Result<Record, ProviderError> {
        if &self.scope != scope
            || &self.key != current_key
            || !(8..=72).contains(&signature_der.len())
        {
            return Err(invalid("future debit confirmation scope/key/signature"));
        }
        let signature = KagemushaDeviceSignatureV1::from_der_normalizing_low_s(signature_der)
            .map_err(|_| invalid("future debit confirmation DER"))?;
        let record = Record {
            version: 1,
            scope: self.scope.clone(),
            key: self.key,
            nonce: self.nonce,
            signature,
        };
        record.verify(scope)?;
        Ok(record)
    }
}

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.spartan.DebitConfirmationRecordV1")]
struct Record {
    version: u16,
    scope: Scope,
    key: KagemushaDevicePublicKeyV1,
    nonce: [u8; 32],
    signature: KagemushaDeviceSignatureV1,
}

impl Record {
    fn evidence(&self) -> native::DebitApprovalEvidence {
        native::DebitApprovalEvidence {
            slot: self.scope.slot,
            key: self.key,
            nonce: self.nonce,
            signature: self.signature,
        }
    }
    fn verify(&self, expected: &Scope) -> Result<(), ProviderError> {
        if self.version != 1
            || &self.scope != expected
            || self.nonce == [0; 32]
            || self.key == expected.payment_key
        {
            return Err(invalid("future debit confirmation record scope"));
        }
        self.signature
            .verify(&self.key, &expected.message(&self.key, &self.nonce))
            .map_err(|_| invalid("future debit confirmation signature"))
    }

    fn already_retained(
        original: &[u8],
        scope: &Scope,
    ) -> Result<DebitConfirmationRetentionV1, ProviderError> {
        Self::decode(original, scope)?;
        Ok(DebitConfirmationRetentionV1::AlreadyRetained)
    }

    fn decode(original: &[u8], scope: &Scope) -> Result<Self, ProviderError> {
        if original.is_empty() || original.len() > MAX_DEBIT_APPROVAL_BYTES {
            return Err(invalid("future debit confirmation record length"));
        }
        let record: Self = crate::kagemusha_wallet_advance_v1::decode_resource::read_canonical(
            original,
            MAX_DEBIT_APPROVAL_BYTES,
        )?;
        record.verify(scope)?;
        Ok(record)
    }
}

#[cfg(any(target_os = "ios", test))]
fn prepare_apple_pin(
    pin: &mut Option<([u8; 32], KagemushaDevicePublicKeyV1)>,
    payment_key: KagemushaDevicePublicKeyV1,
    mut hardware: impl FnMut(&[u8; 32], bool) -> Result<KagemushaDevicePublicKeyV1, ProviderError>,
) -> Result<bool, ProviderError> {
    if let Some((tag, key)) = *pin {
        let found = hardware(&tag, false)?;
        if tag == [0; 32] || found != key || key == payment_key {
            return Err(invalid("changed protected Apple debit key"));
        }
        return Ok(false);
    }
    let mut tag = [0; 32];
    rand::rngs::SysRng
        .try_fill_bytes(&mut tag)
        .map_err(|_| ProviderError::Unavailable(KagemushaWalletUnavailableV1::Platform(0)))?;
    if tag == [0; 32] {
        return Err(invalid("Apple debit key tag entropy"));
    }
    let key = hardware(&tag, true)?;
    if key == payment_key || hardware(&tag, false)? != key {
        return Err(invalid(
            "created Apple debit key is not independently retrievable",
        ));
    }
    *pin = Some((tag, key));
    Ok(true)
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    fn confirmation_scope(
        &self,
        slot: &Slot,
        capsule: &RecoveryCapsule,
    ) -> Result<Scope, ProviderError> {
        self.reviewed_debit(slot, capsule).map(|(scope, _)| scope)
    }
    fn reviewed_debit(
        &self,
        slot: &Slot,
        capsule: &RecoveryCapsule,
    ) -> Result<(Scope, native::PreparedOperation), ProviderError> {
        if !matches!(capsule.kind(), 3 | 6) {
            return Err(invalid("future debit confirmation kind"));
        }
        self.require_account_wallet(capsule.wallet())?;
        let prepared = self.engine.resume(capsule, &self.parameters).map_err(provider_error)?;
        self.require_account(slot, Some(prepared.credential()))?;
        let review = prepared.review_bytes().map_err(provider_error)?;
        Ok((
            Scope {
                manifest: *self.parameters.manifest_digest(),
                slot: slot.0,
                wallet: *capsule.wallet(),
                operation: *capsule.operation_id(),
                capsule: RecoveryCapsule::frame_digest(&capsule.encode().map_err(provider_error)?),
                review: Sha256::digest(&review).into(),
                payment_key: KagemushaDevicePublicKeyV1::from_sec1_bytes(capsule.payment_key())
                    .map_err(|_| invalid("future debit payment key"))?,
            },
            prepared,
        ))
    }

    /// The approval UI and durable choice are meaningful only for the actual live predecessor.
    /// Recovery of an already Selected debit never re-prompts or rotates the choice.
    fn require_live_debit_head(
        &mut self,
        slot: &Slot,
        capsule: &RecoveryCapsule,
    ) -> Result<(), ProviderError> {
        use crate::kagemusha_wallet_advance_v1::KagemushaWalletSlotStatusV1 as Status;
        let Status::Released(marker) = self.provider.status(slot)? else {
            return Err(invalid("fresh debit requires a released predecessor"));
        };
        match &marker.marker().state {
            KagemushaWalletMarkerStateV1::Head {
                sequence,
                head,
                capsule_digest,
                ..
            } if sequence.checked_add(1) == Some(capsule.sequence())
                && &head.value == capsule.predecessor()
                && capsule_digest == capsule.predecessor_capsule_digest()
                && marker.marker().wallet_id == *capsule.wallet() =>
            {
                Ok(())
            }
            _ => Err(invalid("changed debit predecessor")),
        }
    }

    fn current_debit_confirmation_key(
        &mut self,
        slot: &Slot,
        _scope: &Scope,
    ) -> Result<KagemushaDevicePublicKeyV1, ProviderError> {
        #[cfg(target_os = "ios")]
        {
            let (_, checkpoint) = self.custody_checkpoint(slot, _scope.wallet)?;
            let (tag, key) = checkpoint
                .apple_key
                .ok_or(invalid("missing protected Apple debit key"))?;
            let found = self
                .platform
                .apple_debit_confirmation_key(slot, &tag, false)
                .map_err(ProviderError::Unavailable)?;
            if found != key {
                return Err(invalid("changed protected Apple debit key"));
            }
            Ok(key)
        }
        #[cfg(not(target_os = "ios"))]
        self.platform
            .debit_confirmation_key(slot)
            .map_err(ProviderError::Unavailable)
    }

    /// Provision a source-created user-presence key and bind its positive public point and random
    /// Keychain tag to the protected wallet checkpoint. Existing pins are verified on every retry.
    /// An interrupted unpinned key cannot be mistaken for the key created on a later attempt.
    ///
    /// # Errors
    /// Refuses missing account authority, changed or unavailable hardware, entropy or custody.
    #[cfg(any(target_os = "ios", test))]
    pub fn prepare_apple_debit_confirmation_key(
        &mut self,
        slot: &Slot,
        capsule: &RecoveryCapsule,
    ) -> Result<(), ProviderError> {
        self.require_live_debit_head(slot, capsule)?;
        let scope = self.confirmation_scope(slot, capsule)?;
        let (expected, mut checkpoint) = self.custody_checkpoint(slot, scope.wallet)?;
        let changed = prepare_apple_pin(
            &mut checkpoint.apple_key,
            scope.payment_key,
            |tag, create| {
                self.platform
                    .apple_debit_confirmation_key(slot, tag, create)
                    .map_err(ProviderError::Unavailable)
            },
        )?;
        if !changed {
            return Ok(());
        }
        self.publish_custody_checkpoint(slot, expected, &checkpoint)
    }

    /// Return the marker-selected key for the exact still-pending Apple system prompt.
    ///
    /// # Errors
    /// Refuses canceled prompts, changed capsule, missing wallet checkpoint or key.
    #[cfg(target_os = "ios")]
    pub fn apple_debit_prompt_key(
        &mut self,
        slot: &Slot,
        capsule: &RecoveryCapsule,
        message: &[u8; 32],
    ) -> Result<([u8; 32], KagemushaDevicePublicKeyV1), ProviderError> {
        self.require_pending_debit_confirmation(slot, capsule, message)?;
        let (_, checkpoint) = self.custody_checkpoint(slot, *capsule.wallet())?;
        let pin = checkpoint
            .apple_key
            .ok_or(invalid("missing protected Apple debit key"))?;
        if !self
            .pending_debit_confirmation
            .as_ref()
            .is_some_and(|pending| pending.key == pin.1)
        {
            return Err(invalid("Apple debit prompt pin changed"));
        }
        Ok(pin)
    }

    /// Prepare fresh user-presence confirmation for one exact authenticated Send or Unload.
    /// This returns no authority to select a debit.
    ///
    /// # Errors
    /// Refuses an absent protected confirmation key, the payment key, invalid custody or entropy loss.
    pub fn begin_debit_confirmation(
        &mut self,
        slot: &Slot,
        capsule: &RecoveryCapsule,
    ) -> Result<[u8; 32], ProviderError> {
        self.require_live_debit_head(slot, capsule)?;
        let scope = self.confirmation_scope(slot, capsule)?;
        let key = self.current_debit_confirmation_key(slot, &scope)?;
        if key == scope.payment_key {
            return Err(invalid("payment key cannot confirm user presence"));
        }
        let mut nonce = [0; 32];
        rand::rngs::SysRng
            .try_fill_bytes(&mut nonce)
            .map_err(|_| ProviderError::Unavailable(KagemushaWalletUnavailableV1::Platform(0)))?;
        if nonce == [0; 32] {
            return Err(invalid("future debit confirmation nonce"));
        }
        let challenge = DebitConfirmationChallengeV1 { scope, key, nonce };
        let message = challenge.signing_message();
        self.debit.supersede();
        self.pending_debit_confirmation = Some(challenge);
        Ok(message)
    }

    /// Check that a platform prompt still names this Native owner's exact frozen review.
    ///
    /// # Errors
    /// A canceled, changed, or foreign review cannot open a platform prompt.
    pub fn require_pending_debit_confirmation(
        &self,
        slot: &Slot,
        capsule: &RecoveryCapsule,
        message: &[u8; 32],
    ) -> Result<(), ProviderError> {
        let digest = RecoveryCapsule::frame_digest(&capsule.encode().map_err(provider_error)?);
        let pending = self
            .pending_debit_confirmation
            .as_ref()
            .filter(|pending| {
                pending.scope.slot == slot.0
                    && pending.scope.operation == *capsule.operation_id()
                    && pending.scope.capsule == digest
                    && pending.signing_message() == *message
            })
            .ok_or(invalid("future debit confirmation is no longer pending"))?;
        self.require_account_wallet(&pending.scope.wallet)
    }

    /// Verify the platform signature against the independently reread key and retain its
    /// canonical original under the wallet's protected checkpoint. A retained approval is
    /// historical evidence; it cannot start a fresh debit.
    ///
    /// # Errors
    /// Rejects a changed review/key, malformed signature, unauthenticated prior record or custody error.
    pub fn retain_signed_debit_confirmation(
        &mut self,
        slot: &Slot,
        capsule: &RecoveryCapsule,
        message: &[u8; 32],
        signature_der: &[u8],
    ) -> Result<DebitConfirmationRetentionV1, ProviderError> {
        self.require_live_debit_head(slot, capsule)?;
        let (scope, prepared) = self.reviewed_debit(slot, capsule)?;
        let key = self.current_debit_confirmation_key(slot, &scope)?;
        let challenge = self
            .pending_debit_confirmation
            .as_ref()
            .filter(|challenge| &challenge.signing_message() == message && challenge.scope == scope)
            .ok_or(invalid("future debit confirmation is no longer pending"))?;
        #[cfg(target_os = "android")]
        self.platform
            .debit_confirmation_origin(slot, message, signature_der)
            .map_err(ProviderError::Unavailable)?;
        let record = challenge.finish(&scope, &key, signature_der)?;
        // The exclusive provider may have committed before a prior response was delivered.
        // Keep its first original, including the old key and nonce. Every new response
        // above must still verify under its own fresh prompt and current protected key.
        let chosen = norito::encode_canonical(&record)
            .map_err(|_| invalid("future debit confirmation record encoding"))?;
        let first = self.retained_debit_authorization(slot, scope.wallet, scope.capsule)?;
        let outcome = match first.as_ref() {
            Some(original) => Record::already_retained(original, &scope)?,
            None => DebitConfirmationRetentionV1::Retained,
        };
        self.choose_debit_authorization(
            slot,
            scope.wallet,
            scope.capsule,
            first.as_deref().unwrap_or(&chosen),
            &chosen,
        )?;
        let Some((root, durable)) =
            self.chosen_debit_authorization(slot, scope.wallet, scope.capsule)?
        else {
            return Err(invalid("missing committed debit choice"));
        };
        if durable != chosen {
            return Err(invalid("changed committed debit choice"));
        }
        let exact = debit_gate::Exact::new(
            slot,
            scope.manifest,
            root,
            &prepared,
            chosen,
            record.evidence(),
        )?;
        self.debit.accept(exact, *message);
        self.pending_debit_confirmation = None;
        Ok(outcome)
    }

    /// Retract a prompt only if this exact slot, operation and challenge are still current.
    pub fn cancel_debit_confirmation(
        &mut self,
        slot: &Slot,
        operation: &[u8; 32],
        message: &[u8; 32],
    ) {
        self.debit.retract(&slot.0, operation, Some(message));
        DebitConfirmationChallengeV1::cancel_exact(
            &mut self.pending_debit_confirmation,
            &slot.0,
            operation,
            message,
        );
    }

    /// Cancel only the pending volatile prompt for this slot and operation.
    pub fn discard_debit_confirmation(&mut self, slot: &Slot, operation: &[u8; 32]) {
        self.debit.retract(&slot.0, operation, None);
        if self
            .pending_debit_confirmation
            .as_ref()
            .is_some_and(|pending| {
                pending.scope.slot == slot.0 && &pending.scope.operation == operation
            })
        {
            self.pending_debit_confirmation = None;
        }
    }

    pub(super) fn archived_debit(
        &mut self,
        slot: &Slot,
        capsule: &RecoveryCapsule,
        fresh_key: Option<KagemushaDevicePublicKeyV1>,
    ) -> Result<Option<debit_gate::Exact>, ProviderError> {
        if !matches!(capsule.kind(), 3 | 6) {
            return Ok(None);
        }
        let (scope, prepared) = self.reviewed_debit(slot, capsule)?;
        if let Some(key) = fresh_key {
            if self.current_debit_confirmation_key(slot, &scope)? != key {
                return Err(invalid("changed live debit confirmation key"));
            }
        }
        let Some((root, original)) =
            self.chosen_debit_authorization(slot, scope.wallet, scope.capsule)?
        else {
            return Ok(None);
        };
        let record = Record::decode(&original, &scope)?;
        debit_gate::Exact::new(
            slot,
            scope.manifest,
            root,
            &prepared,
            original,
            record.evidence(),
        )
        .map(Some)
    }

    /// Reauthenticate the immutable original selected by this wallet's protected checkpoint.
    /// A regenerated local confirmation key does not invalidate the old signed evidence.
    ///
    /// # Errors
    /// Refuses a foreign scope, altered signature or damaged retained record.
    pub fn verified_debit_confirmation_original(
        &mut self,
        slot: &Slot,
        capsule: &RecoveryCapsule,
    ) -> Result<Option<Vec<u8>>, ProviderError> {
        let scope = self.confirmation_scope(slot, capsule)?;
        let original = self.retained_debit_authorization(slot, scope.wallet, scope.capsule)?;
        if let Some(bytes) = &original {
            Record::decode(bytes, &scope)?;
        }
        Ok(original)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    fn key(n: u8) -> (SigningKey, KagemushaDevicePublicKeyV1) {
        let signer = SigningKey::from_slice(&[n; 32]).unwrap();
        let public = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signer.verifying_key().to_sec1_point(false).as_bytes(),
        )
        .unwrap();
        (signer, public)
    }

    fn scope() -> Scope {
        Scope {
            manifest: [1; 32],
            slot: [2; 32],
            wallet: [3; 32],
            operation: [4; 32],
            capsule: [5; 32],
            review: [6; 32],
            payment_key: key(1).1,
        }
    }

    #[test]
    fn replaced_or_canceled_prompt_cannot_consume_a_later_challenge() {
        let (_, key) = key(7);
        let prompt = |nonce: u8| DebitConfirmationChallengeV1 {
            scope: scope(),
            key,
            nonce: [nonce; 32],
        };
        let canceled_message = prompt(9).signing_message();
        let mut pending = Some(prompt(10));
        let current_message = pending.as_ref().unwrap().signing_message();
        let scope = scope();
        DebitConfirmationChallengeV1::cancel_exact(
            &mut pending,
            &scope.slot,
            &scope.operation,
            &canceled_message,
        );
        assert!(pending.is_some());
        DebitConfirmationChallengeV1::cancel_exact(
            &mut pending,
            &[99; 32],
            &scope.operation,
            &current_message,
        );
        assert!(pending.is_some());
        DebitConfirmationChallengeV1::cancel_exact(
            &mut pending,
            &scope.slot,
            &scope.operation,
            &current_message,
        );
        assert!(pending.is_none());
    }

    #[test]
    fn apple_pin_requires_positive_creation_and_readback_and_never_adopts_a_replacement() {
        let (_, payment) = key(2);
        let (_, intended) = key(3);
        let (_, other) = key(4);
        let mut pin = None;
        let mut issued = Vec::new();
        assert!(
            prepare_apple_pin(&mut pin, payment, |tag, create| {
                if create {
                    issued.push(*tag);
                }
                Ok(if create { intended } else { other })
            })
            .is_err()
        );
        assert_eq!(pin, None);
        assert!(
            prepare_apple_pin(&mut pin, payment, |tag, create| {
                if create {
                    issued.push(*tag);
                }
                Ok(intended)
            })
            .unwrap()
        );
        assert_eq!(issued.len(), 2);
        assert_ne!(issued[0], issued[1]);
        assert_eq!(pin, Some((issued[1], intended)));
        for outcome in [Ok(other), Err(invalid("missing key"))] {
            assert!(
                prepare_apple_pin(&mut pin, payment, |tag, create| {
                    assert!(!create);
                    assert_eq!(tag, &issued[1]);
                    outcome
                })
                .is_err()
            );
            assert_eq!(pin, Some((issued[1], intended)));
        }
        assert!(
            !prepare_apple_pin(&mut pin, payment, |_, create| {
                assert!(!create);
                Ok(intended)
            })
            .unwrap()
        );
        let mut no_pin = None;
        assert!(prepare_apple_pin(&mut no_pin, payment, |_, _| Ok(payment)).is_err());
        assert_eq!(no_pin, None);
    }

    #[test]
    fn confirmation_survives_key_rotation_only_as_exact_archived_evidence() {
        let (signer, public) = key(7);
        let (_, replacement) = key(8);
        let fresh = || DebitConfirmationChallengeV1 {
            scope: scope(),
            key: public,
            nonce: [9; 32],
        };
        let message = fresh().signing_message();
        // Derived independently with OpenSSL P-256 SEC1 and SHA-256.
        assert_eq!(
            hex::encode(message),
            "08985473bbf52a0149d7ad8ff3f3261c760527a104b211c130347bc71201d6f7"
        );
        let sign: Signature = signer.sign(&message);
        let der = sign.to_der();
        assert!(
            fresh()
                .finish(&scope(), &replacement, der.as_bytes())
                .is_err()
        );
        assert!(fresh().finish(&scope(), &public, &[0; 72]).is_err());
        let record = fresh().finish(&scope(), &public, der.as_bytes()).unwrap();
        let original = norito::encode_canonical(&record).unwrap();
        assert!(original.len() <= MAX_DEBIT_APPROVAL_BYTES);
        // Recovery opens an authenticated checkpoint, so the original bound key is retained.
        let readback = Record::decode(&original, &scope()).unwrap();
        assert_eq!(norito::encode_canonical(&readback).unwrap(), original);
        assert_eq!(readback.key, public);
        assert_eq!(
            Record::already_retained(&original, &scope()).unwrap(),
            DebitConfirmationRetentionV1::AlreadyRetained
        );

        // A high-S twin is a mathematically valid ECDSA signature, but never a
        // canonical retained original. Replace the fixed-width signature in the frame.
        let normalized = sign.normalize_s();
        let (r, s) = normalized.split_scalars();
        let high = Signature::from_scalars(r.to_bytes(), (-*s.as_ref()).to_bytes()).unwrap();
        assert!(KagemushaDeviceSignatureV1::from_raw_bytes(&high.to_bytes()).is_err());
        let raw = record.signature.as_raw_bytes();
        let mut high_original = original.clone();
        let at = high_original
            .windows(raw.len())
            .position(|window| window == raw)
            .unwrap();
        high_original[at..at + raw.len()].copy_from_slice(&high.to_bytes());
        assert!(Record::already_retained(&high_original, &scope()).is_err());
        assert!(Record::decode(&[&original[..], &[0u8]].concat(), &scope()).is_err());

        for index in 0..6 {
            let mut altered = scope();
            let field = match index {
                0 => &mut altered.manifest,
                1 => &mut altered.slot,
                2 => &mut altered.wallet,
                3 => &mut altered.operation,
                4 => &mut altered.capsule,
                _ => &mut altered.review,
            };
            field[0] ^= 1;
            assert!(fresh().finish(&altered, &public, der.as_bytes()).is_err());
            assert!(Record::decode(&original, &altered).is_err());
        }
        let mut another_nonce = fresh();
        another_nonce.nonce[0] ^= 1;
        assert!(
            another_nonce
                .finish(&scope(), &public, der.as_bytes())
                .is_err()
        );
        // Even a valid signature over this exact scope under its payment key
        // must never be accepted as independent confirmation on archive recovery.
        let (payment_signer, payment_key) = key(1);
        let payment_challenge = DebitConfirmationChallengeV1 {
            scope: scope(),
            key: payment_key,
            nonce: [11; 32],
        };
        let payment_signature: Signature =
            payment_signer.sign(&payment_challenge.signing_message());
        let payment_receipt = Record {
            version: 1,
            scope: scope(),
            key: payment_key,
            nonce: [11; 32],
            signature: KagemushaDeviceSignatureV1::from_der_normalizing_low_s(
                payment_signature.to_der().as_bytes(),
            )
            .unwrap(),
        };
        assert!(
            Record::decode(
                &norito::encode_canonical(&payment_receipt).unwrap(),
                &scope()
            )
            .is_err()
        );
        let mut forged = record;
        forged.key = replacement;
        assert!(Record::decode(&norito::encode_canonical(&forged).unwrap(), &scope()).is_err());
    }
}
