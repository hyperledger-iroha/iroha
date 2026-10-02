//! Closed Native Send credit production, before purpose2 selection.
//!
//! This child of authenticated_ordinary_cash_owner derives from its actual held State, original
//! reserved operation and Native clock. It reuses the maintained typed credit encryption kernel.
//! The Main owner must fsync this private original in its single WAL before exposing output or
//! selecting W2. Decoding this record creates data only; recovery must call recheck_originals
//! with the independently retained receiver holder. This module creates no financial grant.

use super::*;
use crate::kagemusha_v1_crypto::seal_kagemusha_credit_v1_with_rng;
use iroha_data_model::kagemusha::{
    KagemushaCreditOpeningV1, KagemushaOrdinaryCashClockContextV1,
    KagemushaOrdinaryPaymentOutputV1, KagemushaOrdinaryPaymentRequestV1,
    kagemusha_asset_identity_digest_v1, kagemusha_ciphertext_digest_v1,
    kagemusha_ordinary_credit_id_v1, kagemusha_ordinary_send_credit_aad_v1,
    kagemusha_ordinary_transition_nullifier_v1, kagemusha_peer_credit_opening_commitment_v1,
};
use rand::rand_core::{TryCryptoRng, TryRngCore};
use zeroize::{Zeroize as _, Zeroizing};

/// Private WAL data only. Every secret copy and canonical plaintext is cleared on drop.
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinarySendCreditOriginalsV1")]
pub(super) struct SendCreditOriginals {
    operation: DigestV1,
    request_original: Vec<u8>,
    receiver_fi_original: Vec<u8>,
    receiver_credential_original: Vec<u8>,
    receiver_possession_original: Vec<u8>,
    receiver_lease_original: Option<Vec<u8>>,
    receiver_counter_floor: Option<u32>,
    preparation_clock: KagemushaOrdinaryCashClockContextV1,
    output: KagemushaOrdinaryPaymentOutputV1,
    opening: PrivateCreditOpening,
    plaintext_original: PrivateSecretBytes,
    // Exactly 32 ephemeral private bytes then 24 public nonce bytes. This is private Native WAL
    // data needed to reproduce the original AEAD on recovery, never a new entropy request.
    sealing_entropy: PrivateSecretBytes,
    encrypted_credit: Vec<u8>,
}

// Each secret field owns its clearing behavior before the containing DTO has finished
// decoding. A later malformed field cannot leave an already decoded plaintext copy uncleared.
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryPrivateSecretBytesV1")]
struct PrivateSecretBytes(Vec<u8>);
impl Drop for PrivateSecretBytes {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

// Cleared on every error path, including failed metadata checks after AEAD construction.
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryPrivateCreditOpeningV1")]
struct PrivateCreditOpening(KagemushaCreditOpeningV1);
impl Drop for PrivateCreditOpening {
    fn drop(&mut self) {
        self.0.credit_commitment_opening.zeroize();
        self.0.recipient_binding_opening.zeroize();
        self.0.recovery_nonce.zeroize();
    }
}

impl core::fmt::Debug for SendCreditOriginals {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SendCreditOriginals")
            .field("operation", &self.operation)
            .field("output", &self.output)
            .finish_non_exhaustive()
    }
}
impl Drop for SendCreditOriginals {
    fn drop(&mut self) {
        self.plaintext_original.0.zeroize();
        self.sealing_entropy.0.zeroize();
    }
}

impl SendCreditOriginals {
    /// Called only by the exclusive Main owner after Intent, before W2 selection or any OS call.
    /// The returned data must be fsynced and retained in that owner's Pending before any getter
    /// is exposed. The caller must reject a second SendCredit record for the same operation.
    pub(super) fn create(
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        operation: DigestV1,
        successor: &KagemushaStateV1,
        request_original: &[u8],
        receiver: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
        receiver_lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
        independent_receiver_counter_floor: Option<u32>,
    ) -> Result<Self, KagemushaStateErrorV1> {
        owner.require_current_financial_control()?;
        let pending = owner
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.operation != operation || pending.selected.is_some() || pending.fenced {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let request = KagemushaOrdinaryPaymentRequestV1::decode_canonical_exact(request_original)
            .map_err(material)?;
        let clock = pending.preparation_clock;
        require_source(
            &owner.state,
            successor,
            &request,
            receiver,
            receiver_lease,
            independent_receiver_counter_floor,
            &clock,
        )?;
        let mut entropy = Zeroizing::new([0_u8; 152]);
        rand::rngs::OsRng
            .try_fill_bytes(entropy.as_mut())
            .map_err(material)?;
        let output = derive_credit(
            owner.state.state_commitment,
            successor.state_commitment,
            nullifier(&owner.state)?,
            &request,
            &clock,
            &entropy,
        )?;
        let (opening, plaintext_original, sealing_entropy, encrypted_credit, output) = output;
        let this = Self {
            operation,
            request_original: request_original.to_vec(),
            receiver_fi_original: receiver.certificate().canonical_bytes().map_err(material)?,
            receiver_credential_original: receiver.app_credential().original().to_vec(),
            receiver_possession_original: receiver.possession().original().to_vec(),
            receiver_lease_original: receiver_lease.map(|lease| lease.original().to_vec()),
            receiver_counter_floor: independent_receiver_counter_floor,
            preparation_clock: clock,
            output,
            opening,
            plaintext_original: PrivateSecretBytes(plaintext_original.to_vec()),
            sealing_entropy: PrivateSecretBytes(sealing_entropy.to_vec()),
            encrypted_credit,
        };
        this.recheck_originals(owner, operation, successor, receiver, receiver_lease)?;
        // Recheck the real current owner interval after crypto. Original preparation time remains
        // the earlier Native upper; never renew it or accept caller-projected wall time.
        let now = owner
            .publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?;
        now.check_both(|point| {
            if point < this.preparation_clock.lower_at_ms
                || point < request.body.issued_at_ms
                || point >= request.body.expires_at_ms
            {
                return Err(KagemushaStateErrorV1::InvalidTrustedCommitTime);
            }
            match receiver_lease {
                Some(lease) => receiver.recheck_with_integrity_lease(lease, point),
                None => receiver.recheck_at_trusted_time(point),
            }
            .map_err(material)
        })?;
        owner.recheck()?;
        Ok(this)
    }

    /// Reopen the exact original with actual current Main custody and separately held FI/C/PI.
    /// This verifies historical original bindings; it grants neither current receiver time nor
    /// a terminal approval. The Main/Terminal owner separately checks its actual current interval.
    pub(super) fn recheck_originals(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        operation: DigestV1,
        successor: &KagemushaStateV1,
        receiver: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
        receiver_lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        owner.recheck()?;
        self.recheck_original_data(owner, operation, successor, receiver, receiver_lease)?;
        owner.recheck()
    }

    // Data reconstruction at the actual chronological Main replay position. This remains
    // private and requires original publication/control/journal custody. It cannot renew
    // the captured preparation clock or lend a live financial capability.
    pub(super) fn recheck_at_replay_position(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        operation: DigestV1,
        successor: &KagemushaStateV1,
        receiver: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
        receiver_lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        owner.publication.recheck_historical_cash_custody()?;
        owner.journal.check_owned().map_err(storage)?;
        if owner.journal.recovery_prefix().map_err(storage)? != owner.prefix {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let pending = owner
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        owner
            .control
            .recheck_retained_capture_identity(
                owner.publication.cash_financial(),
                pending.financial_control.original_sha256,
                pending.financial_control.lower_ms,
                pending.financial_control.upper_ms,
            )
            .map_err(material)?;
        self.recheck_original_data(owner, operation, successor, receiver, receiver_lease)
    }

    fn recheck_original_data(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        operation: DigestV1,
        successor: &KagemushaStateV1,
        receiver: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
        receiver_lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        let pending = owner
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.operation != operation
            || self.operation != operation
            || self.preparation_clock != pending.preparation_clock
            || self.receiver_fi_original
                != receiver.certificate().canonical_bytes().map_err(material)?
            || self.receiver_credential_original != receiver.app_credential().original()
            || self.receiver_possession_original != receiver.possession().original()
            || self.receiver_lease_original.as_deref()
                != receiver_lease.map(|lease| lease.original())
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let request = self.request()?;
        require_source(
            &owner.state,
            successor,
            &request,
            receiver,
            receiver_lease,
            self.receiver_counter_floor,
            &self.preparation_clock,
        )?;
        let mut entropy = Zeroizing::new([0_u8; 152]);
        entropy[..32].copy_from_slice(&self.opening.0.credit_commitment_opening);
        entropy[32..64].copy_from_slice(&self.opening.0.recipient_binding_opening);
        entropy[64..96].copy_from_slice(&self.opening.0.recovery_nonce);
        if self.sealing_entropy.0.len() != 56 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        entropy[96..].copy_from_slice(&self.sealing_entropy.0);
        let (opening, plaintext, sealing, encrypted, output) = derive_credit(
            owner.state.state_commitment,
            successor.state_commitment,
            nullifier(&owner.state)?,
            &request,
            &self.preparation_clock,
            &entropy,
        )?;
        let identical = opening == self.opening
            && plaintext.as_slice() == self.plaintext_original.0
            && sealing.as_slice() == self.sealing_entropy.0
            && encrypted == self.encrypted_credit
            && output == self.output;
        if !identical {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    pub(super) fn matches_receiver(
        &self,
        receiver: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
    ) -> Result<bool, KagemushaStateErrorV1> {
        Ok(self.receiver_fi_original
            == receiver.certificate().canonical_bytes().map_err(material)?
            && self.receiver_credential_original == receiver.app_credential().original()
            && self.receiver_possession_original == receiver.possession().original())
    }
    pub(super) fn receiver_lease_original(&self) -> Option<&[u8]> {
        self.receiver_lease_original.as_deref()
    }
    pub(super) fn receiver_counter_floor(&self) -> Option<u32> {
        self.receiver_counter_floor
    }
    pub(super) fn request_original(&self) -> &[u8] {
        &self.request_original
    }
    pub(super) fn require_statement(
        &self,
        statement: &TransitionProofStatementV1,
        successor: &KagemushaStateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let request = self.request()?;
        if statement.kind != KagemushaTransitionKindV1::SendSplit
            || statement.amount != request.body.amount
            || statement.predecessor_commitment != self.output.sender_before_commitment
            || statement.successor_commitment != self.output.sender_after_commitment
            || successor.state_commitment != self.output.sender_after_commitment
            || statement.peer_credit_id != self.output.credit_id
            || statement.recipient_encryption_key_binding != request.body.recipient_encryption_key
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    pub(super) fn operation(&self) -> DigestV1 {
        self.operation
    }
    pub(super) fn output(&self) -> &KagemushaOrdinaryPaymentOutputV1 {
        &self.output
    }
    pub(super) fn preparation_clock(&self) -> &KagemushaOrdinaryCashClockContextV1 {
        &self.preparation_clock
    }
    pub(super) fn encrypted_credit(&self) -> &[u8] {
        &self.encrypted_credit
    }
    pub(super) fn request(
        &self,
    ) -> Result<KagemushaOrdinaryPaymentRequestV1, KagemushaStateErrorV1> {
        KagemushaOrdinaryPaymentRequestV1::decode_canonical_exact(&self.request_original)
            .map_err(material)
    }
    // Native-only borrow for the genuine Terminal loan. No managed accessor or raw ctor exists.
    pub(super) fn credit_opening(&self) -> &KagemushaCreditOpeningV1 {
        &self.opening.0
    }
}

fn nullifier(state: &KagemushaStateV1) -> Result<DigestV1, KagemushaStateErrorV1> {
    kagemusha_ordinary_transition_nullifier_v1(
        state.state_commitment,
        state.secure_index,
        state.hardware_epoch.epoch_id,
        *state.lane.network_id.as_bytes(),
        state.lane.device_lane_id,
        state.liability_pool_id,
    )
    .map_err(material)
}

fn require_source(
    before: &KagemushaStateV1,
    successor: &KagemushaStateV1,
    request: &KagemushaOrdinaryPaymentRequestV1,
    receiver: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
    lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    counter_floor: Option<u32>,
    clock: &KagemushaOrdinaryCashClockContextV1,
) -> Result<(), KagemushaStateErrorV1> {
    let body = &request.body;
    let runtime = &receiver.certificate().subject.owner.runtime;
    let balance = before
        .balance
        .checked_sub(body.amount)
        .ok_or(KagemushaStateErrorV1::InsufficientBalance)?;
    let expected = KagemushaStateV1::build(
        before.context(),
        before.liability_pool_id,
        before.lane.clone(),
        balance,
        before
            .logical_sequence
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::SequenceOverflow)?,
        before
            .secure_index
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::SequenceOverflow)?,
        before.hardware_epoch,
        before.device_policy_binding,
        successor.state_nonce_commitment,
        before.consumed_credit_root,
    )?;
    if expected != *successor
        || successor.state_nonce_commitment == before.state_nonce_commitment
        || before.next_one_use_key_reference != [0; 32]
        || body.release_id != before.release_id
        || body.network_id != *before.lane.network_id.as_bytes()
        || body.normalized_asset_id
            != kagemusha_asset_identity_digest_v1(&before.lane.asset).map_err(material)?
        || body.asset_incarnation != *before.asset_incarnation.as_bytes()
        || body.scale != before.lane.scale
        || body.reserve_pool_id != before.liability_pool_id
        || runtime.network_id != before.lane.network_id
        || runtime.asset != before.lane.asset
        || runtime.asset_incarnation != before.asset_incarnation
        || runtime.scale != before.lane.scale
        || receiver.certificate().subject.issuance.release_id != before.release_id
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    clock
        .validate_within_original_window(body.issued_at_ms, body.expires_at_ms)
        .map_err(material)?;
    for now in [clock.lower_at_ms, clock.upper_at_ms] {
        match lease {
            Some(lease) => receiver.recheck_with_integrity_lease(lease, now),
            None => receiver.recheck_at_trusted_time(now),
        }
        .map_err(material)?;
    }
    request
        .authenticate_receiver_signature(receiver.app_credential(), counter_floor)
        .map_err(material)?;
    Ok(())
}

type DerivedCredit = (
    PrivateCreditOpening,
    Zeroizing<Vec<u8>>,
    Zeroizing<Vec<u8>>,
    Vec<u8>,
    KagemushaOrdinaryPaymentOutputV1,
);

// Sole internal deterministic reproduction. Production entropy is selected only by create above;
// private recovery supplies the exact retained original, never replacement randomness.
fn derive_credit(
    before: DigestV1,
    after: DigestV1,
    nullifier: DigestV1,
    request: &KagemushaOrdinaryPaymentRequestV1,
    clock: &KagemushaOrdinaryCashClockContextV1,
    entropy: &[u8; 152],
) -> Result<DerivedCredit, KagemushaStateErrorV1> {
    let request_digest = request.canonical_original_digest().map_err(material)?;
    let opening = PrivateCreditOpening(KagemushaCreditOpeningV1 {
        version: 1,
        credit_id: kagemusha_ordinary_credit_id_v1(nullifier, request_digest),
        amount: request.body.amount,
        credit_commitment_opening: entropy[..32].try_into().map_err(material)?,
        recipient_binding_opening: entropy[32..64].try_into().map_err(material)?,
        recovery_nonce: entropy[64..96].try_into().map_err(material)?,
    });
    let commitment = kagemusha_peer_credit_opening_commitment_v1(
        request_digest,
        request.body.recipient_encryption_key,
        opening.0.amount,
        opening.0.credit_commitment_opening,
        opening.0.recipient_binding_opening,
        opening.0.recovery_nonce,
    )
    .map_err(material)?;
    let aad =
        kagemusha_ordinary_send_credit_aad_v1(request, before, after, nullifier, commitment, clock)
            .map_err(material)?;
    let sealing_entropy = Zeroizing::new(entropy[96..].to_vec());
    let mut rng = RetainedSealingEntropy {
        bytes: &sealing_entropy,
        offset: 0,
    };
    let envelope = seal_kagemusha_credit_v1_with_rng(
        &opening.0,
        &aad,
        request.body.recipient_encryption_key,
        &mut rng,
    )
    .map_err(material)?;
    if rng.offset != 56 {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let encrypted_credit = envelope
        .canonical_bytes_against_recipient_key(request.body.recipient_encryption_key)
        .map_err(material)?;
    let output = KagemushaOrdinaryPaymentOutputV1 {
        version: 1,
        request_digest,
        amount: request.body.amount,
        sender_before_commitment: before,
        sender_after_commitment: after,
        transition_nullifier: nullifier,
        credit_id: opening.0.credit_id,
        ciphertext_commitment: commitment,
        encrypted_credit_digest: kagemusha_ciphertext_digest_v1(&encrypted_credit),
        clock_context_digest: clock.binding_digest().map_err(material)?,
        prepared_at_ms: clock.upper_at_ms,
    };
    output.validate_against_clock(clock).map_err(material)?;
    if output
        .encrypted_credit_aad_against(request, clock)
        .map_err(material)?
        != aad
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let plaintext = Zeroizing::new(opening.0.canonical_bytes().map_err(material)?);
    Ok((
        opening,
        plaintext,
        sealing_entropy,
        encrypted_credit,
        output,
    ))
}

struct RetainedSealingEntropy<'a> {
    bytes: &'a [u8],
    offset: usize,
}
#[derive(Debug)]
struct EntropyExhausted;
impl core::fmt::Display for EntropyExhausted {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("retained Send sealing entropy exhausted")
    }
}
impl TryRngCore for RetainedSealingEntropy<'_> {
    type Error = EntropyExhausted;
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        let mut bytes = [0; 4];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        let mut bytes = [0; 8];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }
    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
        let end = self
            .offset
            .checked_add(destination.len())
            .ok_or(EntropyExhausted)?;
        let original = self.bytes.get(self.offset..end).ok_or(EntropyExhausted)?;
        destination.copy_from_slice(original);
        self.offset = end;
        Ok(())
    }
}
impl TryCryptoRng for RetainedSealingEntropy<'_> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_v1_crypto::open_kagemusha_credit_v1;
    use iroha_data_model::kagemusha::{
        KagemushaAppOperationApprovalEvidenceV1, KagemushaEncryptedCreditEnvelopeV1,
        KagemushaOrdinaryPaymentRequestBodyV1,
    };
    use p256::ecdsa::{SigningKey, signature::Signer as _};

    fn fixture() -> (
        KagemushaOrdinaryPaymentRequestV1,
        KagemushaOrdinaryCashClockContextV1,
        Zeroizing<[u8; 32]>,
    ) {
        // Synthetic component data only. These tests never construct a Native cash/receiver owner.
        let recipient_secret = Zeroizing::new([11; 32]);
        let clock = KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [1; 32],
            signed_observations_original_digest: [2; 32],
            lower_at_ms: 1001,
            upper_at_ms: 1002,
        };
        let body = KagemushaOrdinaryPaymentRequestBodyV1 {
            version: 1,
            release_id: [3; 32],
            network_id: [4; 32],
            normalized_asset_id: [5; 32],
            asset_incarnation: [6; 32],
            scale: 2,
            reserve_pool_id: [7; 32],
            recipient_account_binding: [8; 32],
            amount: 17,
            recipient_encryption_key: iroha_crypto::kagemusha::kagemusha_x25519_public_key_v1(
                &recipient_secret,
            )
            .unwrap(),
            recipient_credential_digest: [9; 32],
            recipient_lane_id: [10; 32],
            request_id: [12; 32],
            clock_context: clock,
            issued_at_ms: 1000,
            expires_at_ms: 2000,
        };
        let signing = SigningKey::from_slice(&[1; 32]).unwrap();
        let signature: p256::ecdsa::Signature =
            signing.sign(&body.canonical_signing_bytes().unwrap());
        let request = KagemushaOrdinaryPaymentRequestV1 {
            body,
            evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: signature.to_der().as_bytes().to_vec(),
            },
        };
        (request, clock, recipient_secret)
    }

    #[test]
    fn ordinary_send_credit_exact_reproduction_and_real_receiver_decryption() {
        let (request, clock, recipient_secret) = fixture();
        let entropy = Zeroizing::new([7; 152]);
        let first =
            derive_credit([13; 32], [14; 32], [15; 32], &request, &clock, &entropy).unwrap();
        let retry =
            derive_credit([13; 32], [14; 32], [15; 32], &request, &clock, &entropy).unwrap();
        assert_eq!(first.0.0, retry.0.0);
        assert_eq!(first.1.as_slice(), retry.1.as_slice());
        assert_eq!(first.3, retry.3);
        assert_eq!(first.4, retry.4);
        assert_eq!(first.1.len(), 200);
        let envelope =
            KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
                &first.3,
                request.body.recipient_encryption_key,
            )
            .unwrap();
        let aad = first
            .4
            .encrypted_credit_aad_against(&request, &clock)
            .unwrap();
        let mut opened = open_kagemusha_credit_v1(
            &envelope,
            &aad,
            request.body.recipient_encryption_key,
            &recipient_secret,
        )
        .unwrap();
        assert_eq!(opened, first.0.0);
        opened.credit_commitment_opening.zeroize();
        opened.recipient_binding_opening.zeroize();
        opened.recovery_nonce.zeroize();
        let mut substituted = aad;
        substituted.context_digest[0] ^= 1;
        assert!(
            open_kagemusha_credit_v1(
                &envelope,
                &substituted,
                request.body.recipient_encryption_key,
                &recipient_secret
            )
            .is_err()
        );
        let mut changed = Zeroizing::new(*entropy);
        changed[0] ^= 1;
        let distinct =
            derive_credit([13; 32], [14; 32], [15; 32], &request, &clock, &changed).unwrap();
        assert_ne!(
            first.4.ciphertext_commitment,
            distinct.4.ciphertext_commitment
        );
        assert_ne!(first.3, distinct.3);
        let mut changed_clock = clock;
        changed_clock.upper_at_ms += 1;
        let changed_time = derive_credit(
            [13; 32],
            [14; 32],
            [15; 32],
            &request,
            &changed_clock,
            &entropy,
        )
        .unwrap();
        assert_ne!(first.3, changed_time.3);
        assert_ne!(
            first.4.binding_digest().unwrap(),
            changed_time.4.binding_digest().unwrap()
        );
    }

    #[test]
    fn ordinary_send_credit_rejects_inert_openings_ephemeral_key_and_expired_clock() {
        let (request, clock, _) = fixture();
        for start in [0, 32, 64, 96] {
            let mut entropy = Zeroizing::new([7; 152]);
            entropy[start..start + 32].fill(0);
            assert!(
                derive_credit([13; 32], [14; 32], [15; 32], &request, &clock, &entropy).is_err()
            );
        }
        let mut expired = clock;
        expired.upper_at_ms = request.body.expires_at_ms;
        assert!(
            derive_credit([13; 32], [14; 32], [15; 32], &request, &expired, &[7; 152]).is_err()
        );
        assert!(derive_credit([13; 32], [13; 32], [15; 32], &request, &clock, &[7; 152]).is_err());
    }
}
