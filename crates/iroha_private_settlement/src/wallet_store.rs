//! Bounded software custody for original authorized wallet inputs, without a key generator.
//! Native codecs alone handle witnesses; the index contains only opaque inner ciphertexts.

use super::{
    OneCallPrivateSettlementError, PrivateSettlementBusinessTerms,
    PrivateSettlementRestrictedOpening, PrivateSettlementWallet,
};
use iroha::client::{
    PrivateSettlementRequest, VerifiedPrivateSettlementAbort, VerifiedPrivateSettlementOutcome,
    VerifiedPrivateSettlementReceipt,
};
use iroha::workflow::{Digest32, domain_digest};
use iroha_core_privacy::privacy_engines::atomic_private_settlement::{
    ATOMIC_PRIVATE_SETTLEMENT_RESERVATION_MAX_BYTES_V1,
    AtomicPrivateSettlementAuthorizedWalletLegV1, AtomicPrivateSettlementPreparationErrorV1,
    AtomicPrivateSettlementReservationInspectionV1,
    decode_atomic_private_settlement_reservation_v1,
    encode_atomic_private_settlement_reservation_v1,
};
use iroha_crypto::{
    Hash,
    encryption::{ChaCha20Poly1305, SymmetricEncryptor},
};
use iroha_data_model::nexus::{
    PRIVATE_SETTLEMENT_MAX_AUDIT_MEMO_BYTES_V1, PrivateSettlementAuditPlaintextV1,
    PrivateSettlementAuditPolicyV1, PrivateSettlementPoolGovernanceV1,
    PrivateSettlementProofIntentV1,
};
use iroha_fs::{PrivateDirectory, PublishMode};
use norito::{Decode, Encode};
use std::{collections::BTreeSet, fs::File, path::Path};
use zeroize::Zeroizing;

const STATE_NAME: &str = "authorized-inputs.wallet";
const LOCK_NAME: &str = "authorized-inputs.wallet.lock";
const STATE_AAD: &[u8] = b"iroha:private-settlement:software-wallet-state:v1\0";
pub(crate) const OPENING_MAX_BYTES: usize = 1024 * 1024;
const AEAD_OVERHEAD: usize = 28;

/// Hard plaintext-index bound including pending inner ciphertexts and consumed markers.
/// Physical custody is at most two envelopes of this bound plus 28 AEAD bytes each: one
/// active record and one interrupted/in-flight native staging record, plus an empty lock.
pub const MAX_PRIVATE_SETTLEMENT_WALLET_BYTES: usize = 64 * 1024 * 1024;

/// Original software-custody refusal; none authorizes replacement inputs or releases a note.
#[derive(Debug, thiserror::Error)]
pub enum PrivateSettlementWalletStorageError {
    /// Exact intent, leg, terms, authorization, round or authenticated record differs.
    #[error("private settlement wallet binding mismatch")]
    Binding,
    /// Another original reservation already owns at least one selected input.
    #[error("private settlement wallet input already reserved")]
    InputReserved,
    /// Verified successful execution already consumed these original inputs.
    #[error("private settlement wallet input already consumed")]
    Consumed,
    /// An independently verified Abort permanently closed this exact original intent.
    #[error("private settlement wallet original was aborted")]
    Aborted,
    /// No explicit original authorization has been staged for this intent and leg.
    #[error("private settlement wallet authorization unavailable")]
    Missing,
    /// The original bounded custody allowance is exhausted.
    #[error("private settlement wallet capacity exhausted")]
    Capacity,
    /// Another local owner is publishing the original wallet state.
    #[error("private settlement wallet custody is busy")]
    Busy,
    /// Original filesystem failure, including uncertain publication acknowledgement.
    #[error("private settlement wallet I/O unavailable")]
    Io(#[source] std::io::Error),
    /// Original canonical codec refusal.
    #[error(transparent)]
    Codec(#[from] norito::Error),
    /// Native authorization, secret material or signature validation refused admission.
    #[error(transparent)]
    Authorization(#[from] AtomicPrivateSettlementPreparationErrorV1),
    /// Original secure randomness or encryption refusal before publication.
    #[error("private settlement wallet encryption unavailable")]
    Encryption(#[source] iroha_crypto::encryption::Error),
}
use PrivateSettlementWalletStorageError as Error;

// All three representations deliberately contain no native witness plaintext. The separate
// inner AEAD keeps generic outer Norito decoding and its partial-error paths secret-free.
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::private_settlement::PendingWalletCiphertextsV1")]
struct Pending {
    opening: Vec<u8>,
    authorization: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::private_settlement::WalletCompletionV1")]
enum Completion {
    Consumed(Digest32),
    Aborted(Digest32),
}
impl Completion {
    fn digest(self) -> Digest32 {
        match self {
            Self::Consumed(digest) | Self::Aborted(digest) => digest,
        }
    }
}

#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::private_settlement::WalletInputReservationV1")]
struct Reservation {
    id: Digest32,
    terms: Digest32,
    original: Digest32,
    input_ids: Vec<Hash>,
    round: Option<Digest32>,
    completion: Option<Completion>,
    // None is a permanent minimal terminal marker, never a reopenable authorization.
    pending: Option<Pending>,
}

#[derive(Default, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha::private_settlement::WalletInputReservationsV1")]
struct State {
    // Strictly ordered by intent/leg; input exclusions cover pending and consumed, not aborted.
    reservations: Vec<Reservation>,
}

/// Caller-key encrypted custody of explicitly authorized pending wallet legs.
///
/// The caller supplies real existing wallet inputs, membership witnesses, signed payer/view
/// authorizations, and restricted governed openings. This owner selects no inputs and creates
/// no signing keys. Explicit staging reserves every active input atomically; the first Prepare
/// or Run binds the original workflow round. Restart reopens the same authorization.
/// The one-call adapter supplies one wallet for every leg; stage all its original authorized
/// legs here before Prepare or Run. Reconciliation validates every original leg together.
///
/// [`Self::reconcile_verified`] consumes original inputs after verified success;
/// [`Self::release_verified_abort`] releases them only after a successful original Abort.
/// Minimal terminal markers prevent reopening either completed intent within the same finite
/// allowance. There is no local-expiry, rejected-carrier, cancellation, or unverified release. The caller
/// retains the original encryption key; key loss and filesystem rollback are not repaired here.
/// Methods perform synchronous native validation and filesystem work. Async coordinators run
/// them inside their original admitted native worker and retain the original round lease until
/// that worker exits; cancelling an awaiter does not cancel or release native custody.
pub struct FilePrivateSettlementWallet {
    directory: PrivateDirectory,
    cipher: SymmetricEncryptor<ChaCha20Poly1305>,
    maximum: usize,
}

impl FilePrivateSettlementWallet {
    /// Open or create an owner-private wallet directory with its original caller-retained key.
    /// Opening creates no index or lock file. `maximum_total_bytes` includes all live encrypted
    /// authorization material and consumed replay-protection markers in the plaintext index.
    /// Physical custody reserves two envelopes of `maximum_total_bytes + 28` each (active and
    /// atomic staging), plus one empty lock. Every writer reconciles at most one exact native
    /// interrupted staging file before creating another; unknown or excess files are refused.
    /// # Errors
    /// Zero key, zero/excessive capacity, unsafe native custody or original filesystem refusal.
    pub fn open(
        path: impl AsRef<Path>,
        key: &[u8; 32],
        maximum_total_bytes: usize,
    ) -> Result<Self, Error> {
        if key.iter().all(|byte| *byte == 0) {
            return Err(Error::Binding);
        }
        if !(1..=MAX_PRIVATE_SETTLEMENT_WALLET_BYTES).contains(&maximum_total_bytes) {
            return Err(Error::Capacity);
        }
        Ok(Self {
            directory: PrivateDirectory::open_or_create(path).map_err(Error::Io)?,
            cipher: SymmetricEncryptor::new_with_key(key).map_err(|_| Error::Binding)?,
            maximum: maximum_total_bytes,
        })
    }

    /// Atomically reserve explicitly authorized original inputs before any proof work.
    ///
    /// Repeating exactly the same authorization is idempotent. Another intent, leg, or round
    /// cannot acquire an overlapping active input, including after restart. The native codec
    /// validates the original note bindings and controller signatures before publication.
    /// The supplied secret owner is consumed and wiped by its existing native Drop contract.
    /// # Errors
    /// Invalid original authorization/terms/openings, overlap, completed inputs, busy custody,
    /// original capacity or publication failure. No partial reservation can be published.
    pub fn stage_authorized(
        &self,
        intent: &PrivateSettlementProofIntentV1,
        ordinal: u8,
        terms: &PrivateSettlementBusinessTerms,
        opening: PrivateSettlementRestrictedOpening,
        authorized: AtomicPrivateSettlementAuthorizedWalletLegV1,
    ) -> Result<(), Error> {
        check_terms(terms, authorized.plaintext())?;
        check_opening(intent, ordinal, terms, &opening)?;
        let (material, inspection) =
            encode_atomic_private_settlement_reservation_v1(intent, ordinal, &authorized)?;
        let opening_bytes = Zeroizing::new(norito::encode_canonical(&(
            opening.governance.clone(),
            opening.policy.clone(),
        ))?);
        if opening_bytes.len() > OPENING_MAX_BYTES {
            return Err(Error::Capacity);
        }
        let mut reservation = Reservation {
            id: reservation_id(intent, ordinal)?,
            terms: terms_digest(terms)?,
            original: original_digest(&material, &opening_bytes),
            input_ids: inspection.input_ids,
            round: None,
            completion: None,
            pending: None,
        };
        if reservation.input_ids.is_empty() || reservation.input_ids.len() > 2 {
            return Err(Error::Binding);
        }
        reservation.input_ids.sort();
        if reservation
            .input_ids
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(Error::Binding);
        }
        let lock = self.writer()?;
        let (mut state, exists) = self.load()?;
        let position = state
            .reservations
            .binary_search_by_key(&reservation.id, |r| r.id);
        let position = match position {
            Ok(index) => {
                let original = &state.reservations[index];
                if original.pending.is_none() {
                    return Err(terminal_error(original));
                }
                if original.terms != reservation.terms
                    || original.original != reservation.original
                    || original.input_ids != reservation.input_ids
                {
                    return Err(Error::Binding);
                }
                self.reopen(original, intent, ordinal, terms)?;
                return Ok(());
            }
            Err(index) => index,
        };
        if state.reservations.iter().any(|original| {
            original
                .input_ids
                .iter()
                .any(|id| reservation.input_ids.contains(id))
        }) {
            return Err(Error::InputReserved);
        }
        reservation.pending = Some(Pending {
            opening: self.encrypt(&reservation, "opening", &opening_bytes)?,
            authorization: self.encrypt(&reservation, "authorization", &material)?,
        });
        state.reservations.insert(position, reservation);
        self.publish(&lock, exists, &state)
    }

    /// Atomically erase the original payment's live witnesses after verified successful execution.
    ///
    /// `prepared` must be the caller's original retained prepared request. Its public statement
    /// binds the native authorization's private plaintext/input commitments; its exact deltas
    /// must match the receipt, including the original nullifiers and output commitments. A
    /// public receipt DTO cannot call this API. Repeating the exact completed round is harmless.
    /// Minimal consumed input identities remain durable; no spent input becomes available again.
    /// # Errors
    /// Missing/substituted original, wrong round, mismatched receipt/artifacts, busy custody or
    /// original publication failure. Before publication the original pending record survives.
    pub fn reconcile_verified(
        &self,
        round: Digest32,
        prepared: &PrivateSettlementRequest,
        receipt: &VerifiedPrivateSettlementReceipt,
    ) -> Result<(), Error> {
        let intent = prepared.manifest().proof_intent();
        let actual = receipt.receipt();
        if actual.manifest.proof_intent() != intent || round == [0; 32] {
            return Err(Error::Binding);
        }
        let lock = self.writer()?;
        let (mut state, exists) = self.load()?;
        let mut completed = Vec::with_capacity(prepared.materials().len());
        for (index, original) in prepared.materials().iter().enumerate() {
            let ordinal = u8::try_from(index).map_err(|_| Error::Binding)?;
            let executed = actual.legs.get(index).ok_or(Error::Binding)?;
            if original.statement.leg_ordinal != ordinal || executed.delta != original.delta {
                return Err(Error::Binding);
            }
            let completion = domain_digest(
                b"iroha:private-settlement:wallet-consumed:v1\0",
                &[&norito::encode_canonical(&(
                    receipt.transaction_hash(),
                    receipt.context_id(),
                    original.statement.digest()?,
                    executed.delta.clone(),
                ))?],
            );
            let id = reservation_id(&intent, ordinal)?;
            let position = state
                .reservations
                .binary_search_by_key(&id, |r| r.id)
                .map_err(|_| Error::Missing)?;
            let reservation = &state.reservations[position];
            if reservation.round != Some(round) {
                return Err(Error::Binding);
            }
            if reservation.pending.is_none() {
                if reservation.completion != Some(Completion::Consumed(completion)) {
                    return Err(Error::Binding);
                }
                continue;
            }
            let (authorized, inspection) = self.authorized(reservation, &intent, ordinal)?;
            if plaintext_terms_digest(authorized.plaintext())? != reservation.terms
                || inspection.audit_plaintext_commitment
                    != original.statement.audit_plaintext_commitment
                || inspection.audit_input_commitment != original.statement.audit_input_commitment
                || inspection.nullifiers.as_slice() != original.statement.nullifiers.as_slice()
            {
                return Err(Error::Binding);
            }
            // Drop re-opened native plaintext before changing only its encrypted live record.
            drop(authorized);
            completed.push((position, completion));
        }
        if completed.is_empty() {
            return Ok(());
        }
        // All legs passed before any original ciphertext is removed. One atomic publication
        // turns the entire payment into consumed markers, and an uncertain ack is retryable.
        for (position, completion) in completed {
            let reservation = &mut state.reservations[position];
            reservation.pending = None;
            reservation.completion = Some(Completion::Consumed(completion));
        }
        self.publish(&lock, exists, &state)
    }

    /// Release every original leg atomically after independently authenticated Abort execution.
    ///
    /// The original prepared request binds each reserved authorization and public statement.
    /// A terminal marker forbids reopening the aborted intent; its input exclusion is removed
    /// so another explicitly authorized intent can reserve the inputs. This does not certify
    /// those inputs as currently unspent: the new operation must authenticate its own context.
    /// An exact repeat after restart or an uncertain publication acknowledgement is harmless.
    /// # Errors
    /// Wrong original round, manifest, input commitment or terminal outcome; unavailable custody
    /// or publication failure. No leg is released until all originals have been checked.
    pub fn release_verified_abort(
        &self,
        round: Digest32,
        prepared: &PrivateSettlementRequest,
        abort: &VerifiedPrivateSettlementAbort,
    ) -> Result<(), Error> {
        let intent = prepared.manifest().proof_intent();
        if round == [0; 32] || abort.manifest().proof_intent() != intent {
            return Err(Error::Binding);
        }
        let lock = self.writer()?;
        let (mut state, exists) = self.load()?;
        let mut released = Vec::with_capacity(prepared.materials().len());
        for (index, original) in prepared.materials().iter().enumerate() {
            let ordinal = u8::try_from(index).map_err(|_| Error::Binding)?;
            if original.statement.leg_ordinal != ordinal {
                return Err(Error::Binding);
            }
            let completion = domain_digest(
                b"iroha:private-settlement:wallet-aborted:v1\0",
                &[&norito::encode_canonical(&(
                    abort.transaction_hash(),
                    abort.context_id(),
                    abort.receipt().clone(),
                    original.statement.digest()?,
                ))?],
            );
            let id = reservation_id(&intent, ordinal)?;
            let position = state
                .reservations
                .binary_search_by_key(&id, |r| r.id)
                .map_err(|_| Error::Missing)?;
            let reservation = &state.reservations[position];
            if reservation.round != Some(round) {
                return Err(Error::Binding);
            }
            if reservation.pending.is_none() {
                if reservation.completion != Some(Completion::Aborted(completion)) {
                    return Err(Error::Binding);
                }
                continue;
            }
            let (authorized, inspection) = self.authorized(reservation, &intent, ordinal)?;
            if plaintext_terms_digest(authorized.plaintext())? != reservation.terms
                || inspection.audit_plaintext_commitment
                    != original.statement.audit_plaintext_commitment
                || inspection.audit_input_commitment != original.statement.audit_input_commitment
                || inspection.nullifiers.as_slice() != original.statement.nullifiers.as_slice()
            {
                return Err(Error::Binding);
            }
            drop(authorized);
            released.push((position, completion));
        }
        if released.is_empty() {
            return Ok(());
        }
        for (position, completion) in released {
            let reservation = &mut state.reservations[position];
            reservation.pending = None;
            reservation.input_ids.clear();
            reservation.completion = Some(Completion::Aborted(completion));
        }
        self.publish(&lock, exists, &state)
    }

    fn writer(&self) -> Result<File, Error> {
        let lock = self.directory.open_lock(LOCK_NAME).map_err(Error::Io)?;
        match lock.try_lock() {
            Ok(()) => (),
            Err(std::fs::TryLockError::WouldBlock) => return Err(Error::Busy),
            Err(std::fs::TryLockError::Error(error)) => return Err(Error::Io(error)),
        }
        self.directory
            .require_same_lock(LOCK_NAME, &lock)
            .map_err(Error::Io)?;
        self.inventory()?;
        self.directory
            .require_same_lock(LOCK_NAME, &lock)
            .map_err(Error::Io)?;
        Ok(lock)
    }

    // Called only under the original exclusive wallet lock. The native helper verifies the
    // complete bounded inventory before deleting its single unpublished staging leaf.
    fn inventory(&self) -> Result<(), Error> {
        let maximum_envelope = self
            .maximum
            .checked_add(AEAD_OVERHEAD)
            .ok_or(Error::Capacity)?;
        let names = self.directory.entries(3).map_err(Error::Io)?;
        let mut required = vec![LOCK_NAME];
        for name in names {
            let name = name.to_str().ok_or(Error::Binding)?;
            match name {
                LOCK_NAME => {
                    if self
                        .directory
                        .open_read(name)
                        .map_err(Error::Io)?
                        .metadata()
                        .map_err(Error::Io)?
                        .len()
                        != 0
                    {
                        return Err(Error::Binding);
                    }
                }
                STATE_NAME => {
                    if self
                        .directory
                        .open_read(name)
                        .map_err(Error::Io)?
                        .metadata()
                        .map_err(Error::Io)?
                        .len()
                        > maximum_envelope as u64
                    {
                        return Err(Error::Capacity);
                    }
                    required.push(STATE_NAME);
                }
                candidate if candidate.starts_with(".iroha-fs-") => (),
                _ => return Err(Error::Binding),
            }
        }
        self.directory
            .reconcile_atomic_staging(&required, 2, 1, maximum_envelope)
            .map_err(Error::Io)?;
        Ok(())
    }

    fn load(&self) -> Result<(State, bool), Error> {
        let Some(envelope) = self
            .directory
            .read_optional(
                STATE_NAME,
                self.maximum
                    .checked_add(AEAD_OVERHEAD)
                    .ok_or(Error::Capacity)?,
            )
            .map_err(Error::Io)?
        else {
            return Ok((State::default(), false));
        };
        let plaintext = Zeroizing::new(
            self.cipher
                .decrypt_easy(STATE_AAD, &envelope)
                .map_err(|_| Error::Binding)?,
        );
        if plaintext.len() > self.maximum {
            return Err(Error::Capacity);
        }
        let state: State = norito::decode_canonical_with_limits(
            &plaintext,
            norito::canonical_decode_limits(plaintext.len()),
        )?;
        let mut input_ids = BTreeSet::new();
        for (index, reservation) in state.reservations.iter().enumerate() {
            if reservation.id == [0; 32]
                || reservation.terms == [0; 32]
                || reservation.original == [0; 32]
                || reservation.round == Some([0; 32])
                || (reservation.pending.is_none() && reservation.round.is_none())
                || reservation
                    .completion
                    .is_some_and(|value| value.digest() == [0; 32])
                || reservation.pending.is_none() != reservation.completion.is_some()
                || reservation.input_ids.is_empty()
                    != matches!(reservation.completion, Some(Completion::Aborted(_)))
                || reservation.input_ids.len() > 2
                || reservation
                    .input_ids
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || (index > 0 && state.reservations[index - 1].id >= reservation.id)
            {
                return Err(Error::Binding);
            }
            for id in &reservation.input_ids {
                if id.as_ref().iter().all(|byte| *byte == 0) || !input_ids.insert(*id) {
                    return Err(Error::Binding);
                }
            }
            if let Some(pending) = &reservation.pending {
                if !(AEAD_OVERHEAD..=OPENING_MAX_BYTES + AEAD_OVERHEAD)
                    .contains(&pending.opening.len())
                    || !(AEAD_OVERHEAD
                        ..=ATOMIC_PRIVATE_SETTLEMENT_RESERVATION_MAX_BYTES_V1 + AEAD_OVERHEAD)
                        .contains(&pending.authorization.len())
                {
                    return Err(Error::Capacity);
                }
            }
        }
        Ok((state, true))
    }

    fn publish(&self, lock: &File, exists: bool, state: &State) -> Result<(), Error> {
        if norito::canonical_frame_len(state)? > self.maximum {
            return Err(Error::Capacity);
        }
        let plaintext = Zeroizing::new(norito::encode_canonical(state)?);
        let ciphertext = self
            .cipher
            .encrypt_easy(STATE_AAD, &plaintext)
            .map_err(Error::Encryption)?;
        self.directory
            .require_same_lock(LOCK_NAME, lock)
            .map_err(Error::Io)?;
        self.directory
            .write_atomic(
                STATE_NAME,
                &ciphertext,
                if exists {
                    PublishMode::Replace
                } else {
                    PublishMode::CreateNew
                },
            )
            .map_err(Error::Io)?;
        self.directory
            .require_same_lock(LOCK_NAME, lock)
            .map_err(Error::Io)
    }

    fn encrypt(
        &self,
        reservation: &Reservation,
        purpose: &str,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.cipher
            .encrypt_easy(aad(reservation, purpose)?.as_slice(), plaintext)
            .map_err(Error::Encryption)
    }

    fn decrypt(
        &self,
        reservation: &Reservation,
        purpose: &str,
        ciphertext: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, Error> {
        self.cipher
            .decrypt_easy(aad(reservation, purpose)?.as_slice(), ciphertext)
            .map(Zeroizing::new)
            .map_err(|_| Error::Binding)
    }

    fn opening(
        &self,
        reservation: &Reservation,
    ) -> Result<PrivateSettlementRestrictedOpening, Error> {
        let pending = reservation
            .pending
            .as_ref()
            .ok_or_else(|| terminal_error(reservation))?;
        let bytes = self.decrypt(reservation, "opening", &pending.opening)?;
        let (governance, policy): (
            PrivateSettlementPoolGovernanceV1,
            PrivateSettlementAuditPolicyV1,
        ) = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )?;
        Ok(PrivateSettlementRestrictedOpening { governance, policy })
    }

    fn authorized(
        &self,
        reservation: &Reservation,
        intent: &PrivateSettlementProofIntentV1,
        ordinal: u8,
    ) -> Result<
        (
            AtomicPrivateSettlementAuthorizedWalletLegV1,
            AtomicPrivateSettlementReservationInspectionV1,
        ),
        Error,
    > {
        let pending = reservation
            .pending
            .as_ref()
            .ok_or_else(|| terminal_error(reservation))?;
        let material = self.decrypt(reservation, "authorization", &pending.authorization)?;
        let opening_bytes = self.decrypt(reservation, "opening", &pending.opening)?;
        if original_digest(&material, &opening_bytes) != reservation.original {
            return Err(Error::Binding);
        }
        let (authorized, inspection) =
            decode_atomic_private_settlement_reservation_v1(material, intent, ordinal)?;
        let mut input_ids = inspection.input_ids.clone();
        input_ids.sort();
        if input_ids != reservation.input_ids {
            return Err(Error::Binding);
        }
        Ok((authorized, inspection))
    }

    fn reopen(
        &self,
        reservation: &Reservation,
        intent: &PrivateSettlementProofIntentV1,
        ordinal: u8,
        terms: &PrivateSettlementBusinessTerms,
    ) -> Result<AtomicPrivateSettlementAuthorizedWalletLegV1, Error> {
        if reservation.terms != terms_digest(terms)? {
            return Err(Error::Binding);
        }
        let opening = self.opening(reservation)?;
        check_opening(intent, ordinal, terms, &opening)?;
        let (authorized, _) = self.authorized(reservation, intent, ordinal)?;
        check_terms(terms, authorized.plaintext())?;
        Ok(authorized)
    }

    fn policy(
        &self,
        intent: &PrivateSettlementProofIntentV1,
        ordinal: u8,
        terms: &PrivateSettlementBusinessTerms,
    ) -> Result<PrivateSettlementRestrictedOpening, Error> {
        let id = reservation_id(intent, ordinal)?;
        let _lock = self.writer()?;
        let (state, _) = self.load()?;
        let index = state
            .reservations
            .binary_search_by_key(&id, |r| r.id)
            .map_err(|_| Error::Missing)?;
        let reservation = &state.reservations[index];
        if reservation.terms != terms_digest(terms)? {
            return Err(Error::Binding);
        }
        let opening = self.opening(reservation)?;
        check_opening(intent, ordinal, terms, &opening)?;
        Ok(opening)
    }

    fn reserve(
        &self,
        round: Digest32,
        intent: &PrivateSettlementProofIntentV1,
        ordinal: u8,
        terms: &PrivateSettlementBusinessTerms,
    ) -> Result<AtomicPrivateSettlementAuthorizedWalletLegV1, Error> {
        if round == [0; 32] {
            return Err(Error::Binding);
        }
        let id = reservation_id(intent, ordinal)?;
        let lock = self.writer()?;
        let (mut state, exists) = self.load()?;
        let index = state
            .reservations
            .binary_search_by_key(&id, |r| r.id)
            .map_err(|_| Error::Missing)?;
        let reservation = &mut state.reservations[index];
        if reservation.round.is_some_and(|original| original != round) {
            return Err(Error::Binding);
        }
        let authorized = self.reopen(reservation, intent, ordinal, terms)?;
        if reservation.round.is_none() {
            reservation.round = Some(round);
            self.publish(&lock, exists, &state)?;
        }
        Ok(authorized)
    }
}

fn terminal_error(reservation: &Reservation) -> Error {
    match reservation.completion {
        Some(Completion::Consumed(_)) => Error::Consumed,
        Some(Completion::Aborted(_)) => Error::Aborted,
        None => Error::Binding,
    }
}

impl PrivateSettlementWallet for FilePrivateSettlementWallet {
    fn open_policy(
        &self,
        intent: &PrivateSettlementProofIntentV1,
        leg: u8,
        terms: &PrivateSettlementBusinessTerms,
    ) -> Result<PrivateSettlementRestrictedOpening, OneCallPrivateSettlementError> {
        self.policy(intent, leg, terms)
            .map_err(OneCallPrivateSettlementError::WalletCustody)
    }

    fn reserve_or_reopen(
        &self,
        round: Digest32,
        intent: &PrivateSettlementProofIntentV1,
        leg: u8,
        terms: &PrivateSettlementBusinessTerms,
    ) -> Result<AtomicPrivateSettlementAuthorizedWalletLegV1, OneCallPrivateSettlementError> {
        self.reserve(round, intent, leg, terms)
            .map_err(OneCallPrivateSettlementError::WalletCustody)
    }

    fn reconcile_verified(
        &self,
        round: Digest32,
        prepared: &PrivateSettlementRequest,
        outcome: &VerifiedPrivateSettlementOutcome,
    ) -> Result<(), OneCallPrivateSettlementError> {
        match outcome {
            VerifiedPrivateSettlementOutcome::Settled(receipt) => {
                FilePrivateSettlementWallet::reconcile_verified(self, round, prepared, receipt)
            }
            VerifiedPrivateSettlementOutcome::Aborted(receipt) => {
                self.release_verified_abort(round, prepared, receipt)
            }
        }
        .map_err(OneCallPrivateSettlementError::WalletCustody)
    }
}

fn reservation_id(intent: &PrivateSettlementProofIntentV1, ordinal: u8) -> Result<Digest32, Error> {
    intent.validate().map_err(|_| Error::Binding)?;
    if intent.legs.get(usize::from(ordinal)).is_none() {
        return Err(Error::Binding);
    }
    let bytes = norito::encode_canonical(&(intent.clone(), ordinal))?;
    Ok(domain_digest(
        b"iroha:private-settlement:wallet-reservation:v1\0",
        &[&bytes],
    ))
}

fn aad(reservation: &Reservation, purpose: &str) -> Result<Vec<u8>, Error> {
    Ok(norito::encode_canonical(&(
        "iroha:private-settlement:wallet-inner:v1",
        purpose,
        reservation.id,
        reservation.terms,
        reservation.original,
    ))?)
}

fn original_digest(material: &[u8], opening: &[u8]) -> Digest32 {
    domain_digest(
        b"iroha:private-settlement:wallet-original:v1\0",
        &[material, opening],
    )
}

fn terms_digest(terms: &PrivateSettlementBusinessTerms) -> Result<Digest32, Error> {
    if terms.amount == 0 || terms.memo.len() > PRIVATE_SETTLEMENT_MAX_AUDIT_MEMO_BYTES_V1 {
        return Err(Error::Binding);
    }
    terms_digest_values(
        &terms.asset,
        &terms.payer,
        &terms.recipient,
        terms.amount,
        terms.reimbursement,
        &terms.memo,
    )
}

fn plaintext_terms_digest(
    plaintext: &PrivateSettlementAuditPlaintextV1,
) -> Result<Digest32, Error> {
    if plaintext.amount == 0 || plaintext.memo.len() > PRIVATE_SETTLEMENT_MAX_AUDIT_MEMO_BYTES_V1 {
        return Err(Error::Binding);
    }
    terms_digest_values(
        &plaintext.asset_definition_id,
        &plaintext.payer,
        &plaintext.recipient,
        plaintext.amount,
        plaintext.sponsor_reimbursement_amount,
        &plaintext.memo,
    )
}

fn terms_digest_values(
    asset: &iroha_data_model::asset::AssetDefinitionId,
    payer: &iroha_data_model::account::AccountId,
    recipient: &iroha_data_model::account::AccountId,
    amount: u128,
    reimbursement: u128,
    memo: &[u8],
) -> Result<Digest32, Error> {
    let asset = Zeroizing::new(norito::encode_canonical(asset)?);
    let payer = Zeroizing::new(norito::encode_canonical(payer)?);
    let recipient = Zeroizing::new(norito::encode_canonical(recipient)?);
    let amount = Zeroizing::new(norito::encode_canonical(&amount)?);
    let reimbursement = Zeroizing::new(norito::encode_canonical(&reimbursement)?);
    let memo_bytes = Zeroizing::new(memo.to_vec());
    let memo = Zeroizing::new(norito::encode_canonical(&*memo_bytes)?);
    Ok(domain_digest(
        b"iroha:private-settlement:wallet-terms:v1\0",
        &[&asset, &payer, &recipient, &amount, &reimbursement, &memo],
    ))
}

fn check_terms(
    terms: &PrivateSettlementBusinessTerms,
    plaintext: &PrivateSettlementAuditPlaintextV1,
) -> Result<(), Error> {
    if terms.amount == 0 || terms_digest(terms)? != plaintext_terms_digest(plaintext)? {
        return Err(Error::Binding);
    }
    Ok(())
}

fn check_opening(
    intent: &PrivateSettlementProofIntentV1,
    ordinal: u8,
    terms: &PrivateSettlementBusinessTerms,
    opening: &PrivateSettlementRestrictedOpening,
) -> Result<(), Error> {
    let leg = intent
        .legs
        .get(usize::from(ordinal))
        .ok_or(Error::Binding)?;
    opening
        .governance
        .validate_asset_opening(
            leg.route,
            leg.pool_id,
            &terms.asset,
            opening.governance.body.asset_binding_salt,
        )
        .map_err(|_| Error::Binding)?;
    for height in [intent.authority_context_height, intent.expiry_height] {
        opening
            .governance
            .validate_against_policy_at(&opening.policy, height)
            .map_err(|_| Error::Binding)?;
    }
    if opening.governance.body.asset_binding_commitment != leg.asset_binding_commitment
        || opening.policy.policy_digest != leg.audit_policy_digest
    {
        return Err(Error::Binding);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_fixture;
#[cfg(test)]
mod tests;
