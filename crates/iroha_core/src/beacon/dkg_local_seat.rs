//! One-seat, one-attempt private owner for distributed beacon DKG.

use super::{
    GlobalThresholdBeaconDkgSnapshotV1, GlobalThresholdBeaconError,
    GlobalThresholdBeaconSessionError, adaptive_beacon_parameters,
    global_threshold_beacon_roster_hash_v1,
    session_owner::{DkgMessageWorkspace, PendingRow, SessionGraphError},
    validation::{DkgSignaturePreimage, DkgSnapshotRef},
};
use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedBuffer, RetainedPayload};
use iroha_crypto::{
    Hash, KeyPair,
    hybrid::{HybridKemCiphertext, HybridKeyPair, HybridPublicKey},
    threshold_bls::{
        AdaptiveThresholdBlsSecretShare, BeaconPurpose, DasRenDealerSecret, DasRenPrivateShare,
        open_das_ren_private_share, seal_das_ren_private_share,
    },
};
use iroha_data_model::consensus::{
    GlobalThresholdBeaconDkgDealerCommitmentV1, GlobalThresholdBeaconDkgEncryptedShareV1,
    GlobalThresholdBeaconDkgRecipientKeyV1, GlobalThresholdBeaconDkgSessionV1,
    GlobalThresholdBeaconDkgShareAcceptanceV1,
};
use iroha_model_base::peer::PeerId;
use norito::codec::Encode as _;
use std::alloc::Layout;
use zeroize::Zeroizing;

mod public_frames;
use public_frames::{PublicFrame, public_commitments_hash};

/// Original local resource/entropy causes remain distinct from invalid public DKG messages.
#[derive(Debug, thiserror::Error)]
pub enum LocalGlobalThresholdBeaconDkgErrorV1 {
    /// A signed protocol relation is invalid.
    #[error(transparent)]
    Invalid(#[from] GlobalThresholdBeaconError),
    /// Actual caller-owned allocation or encoding failed without authenticating input.
    #[error(transparent)]
    Session(#[from] GlobalThresholdBeaconSessionError),
    /// The original BLS producer failed before publishing its output.
    #[error(transparent)]
    Signing(#[from] iroha_crypto::PrepaidBlsSignatureError),
    /// The original hybrid producer failed before publishing a key or capsule.
    #[error(transparent)]
    Hybrid(#[from] iroha_crypto::hybrid::HybridError),
    /// The original threshold primitive failed; entropy refusal is never protocol invalidity.
    #[error(transparent)]
    Threshold(#[from] iroha_crypto::threshold_bls::ThresholdBlsError),
}
impl From<AllocationRefusal> for LocalGlobalThresholdBeaconDkgErrorV1 {
    fn from(error: AllocationRefusal) -> Self {
        Self::Session(error.into())
    }
}
impl From<SessionGraphError> for LocalGlobalThresholdBeaconDkgErrorV1 {
    fn from(error: SessionGraphError) -> Self {
        Self::Session(error.into())
    }
}
impl From<super::GlobalThresholdBeaconVerificationError<SessionGraphError>>
    for LocalGlobalThresholdBeaconDkgErrorV1
{
    fn from(error: super::GlobalThresholdBeaconVerificationError<SessionGraphError>) -> Self {
        Self::Session(error.into())
    }
}

/// Complete real backing for one local attempt, created before RNG or durable claim.
///
/// This owner is neither cloneable nor serializable. Preparation has no secret
/// or protocol side effect; a refused constructor leaves the attempt claim free.
pub struct PreparedLocalGlobalThresholdBeaconDkgSeatV1 {
    session: GlobalThresholdBeaconDkgSessionV1,
    seat_index: u16,
    recipient: PendingRow<GlobalThresholdBeaconDkgRecipientKeyV1>,
    dealer: PendingRow<GlobalThresholdBeaconDkgDealerCommitmentV1>,
    outputs: LocalOutputs,
    workspace: DkgMessageWorkspace,
    public_frame: PublicFrame,
    budget: AllocationBudget,
}
struct LocalOutputs {
    pending_edges: ChargedBuffer<Option<PendingRow<GlobalThresholdBeaconDkgEncryptedShareV1>>>,
    pending_acceptances:
        ChargedBuffer<Option<PendingRow<GlobalThresholdBeaconDkgShareAcceptanceV1>>>,
    outgoing: ChargedBuffer<RetainedPayload<GlobalThresholdBeaconDkgEncryptedShareV1>>,
    acceptances: ChargedBuffer<RetainedPayload<GlobalThresholdBeaconDkgShareAcceptanceV1>>,
    shares: ChargedBuffer<DasRenPrivateShare<BeaconPurpose>>,
}
impl PreparedLocalGlobalThresholdBeaconDkgSeatV1 {
    /// Admit and physically construct all publication, edge, acceptance and scratch storage.
    ///
    /// The caller authenticates the frozen schedule/roster independently, then
    /// claims its one-shot attempt only after this method succeeds.
    ///
    /// # Errors
    /// Invalid geometry or exact original admission/allocator errors leave RNG untouched.
    pub fn new(
        session: GlobalThresholdBeaconDkgSessionV1,
        ordered_roster: &[PeerId],
        seat_index: u16,
        signer: &KeyPair,
        budget: &AllocationBudget,
    ) -> Result<Self, LocalGlobalThresholdBeaconDkgErrorV1> {
        super::validate_dkg_session(&session)?;
        let seats = usize::from(session.committee_size);
        if seats != ordered_roster.len()
            || seat_index == 0
            || usize::from(seat_index) > seats
            || ordered_roster[usize::from(seat_index - 1)].public_key() != signer.public_key()
            || signer.public_key().algorithm() != iroha_crypto::Algorithm::BlsNormal
            || global_threshold_beacon_roster_hash_v1(ordered_roster) != session.roster_hash
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
        }
        // Derivation and schedule checks precede both real output allocation and secret production.
        let _ = adaptive_beacon_parameters(&session)?;
        let recipient = PendingRow::recipient(seat_index, signer.public_key(), budget)?;
        let dealer = PendingRow::dealer(seat_index, session.threshold, budget)?;
        let mut reservation = budget
            .try_reserve_layouts([
                Layout::array::<Option<PendingRow<GlobalThresholdBeaconDkgEncryptedShareV1>>>(
                    seats,
                )
                .map_err(|_| AllocationRefusal::DemandOverflow)?,
                Layout::array::<Option<PendingRow<GlobalThresholdBeaconDkgShareAcceptanceV1>>>(
                    seats,
                )
                .map_err(|_| AllocationRefusal::DemandOverflow)?,
                Layout::array::<RetainedPayload<GlobalThresholdBeaconDkgEncryptedShareV1>>(seats)
                    .map_err(|_| AllocationRefusal::DemandOverflow)?,
                Layout::array::<RetainedPayload<GlobalThresholdBeaconDkgShareAcceptanceV1>>(seats)
                    .map_err(|_| AllocationRefusal::DemandOverflow)?,
                Layout::array::<DasRenPrivateShare<BeaconPurpose>>(seats)
                    .map_err(|_| AllocationRefusal::DemandOverflow)?,
            ])
            .map_err(SessionGraphError::from)?;
        let mut pending_edges = ChargedBuffer::from_reservation(seats, &mut reservation)
            .map_err(SessionGraphError::from)?;
        let mut pending_acceptances = ChargedBuffer::from_reservation(seats, &mut reservation)
            .map_err(SessionGraphError::from)?;
        let outgoing = ChargedBuffer::from_reservation(seats, &mut reservation)
            .map_err(SessionGraphError::from)?;
        let acceptances = ChargedBuffer::from_reservation(seats, &mut reservation)
            .map_err(SessionGraphError::from)?;
        let shares = ChargedBuffer::from_reservation(seats, &mut reservation)
            .map_err(SessionGraphError::from)?;
        let mut max_message = DkgSignaturePreimage::RecipientKey(&session, recipient.record())
            .encoded_len()
            .max(DkgSignaturePreimage::DealerCommitment(&session, dealer.record()).encoded_len());
        for index in 1..=session.committee_size {
            let edge = PendingRow::edge(seat_index, index, budget)?;
            let acceptance = PendingRow::acceptance(index, seat_index, budget)?;
            max_message = max_message
                .max(DkgSignaturePreimage::EncryptedShare(&session, edge.record()).encoded_len())
                .max(
                    DkgSignaturePreimage::ShareAcceptance(&session, acceptance.record())
                        .encoded_len(),
                );
            pending_edges.push_reserved(Some(edge));
            pending_acceptances.push_reserved(Some(acceptance));
        }
        let workspace = DkgMessageWorkspace::new(max_message, budget)?;
        let public_frame = PublicFrame::prepare(
            &session,
            recipient.record(),
            dealer.record(),
            pending_edges.as_slice()[0]
                .as_ref()
                .expect("first prepared edge")
                .record(),
            pending_acceptances.as_slice()[0]
                .as_ref()
                .expect("first prepared acceptance")
                .record(),
            budget,
        )?;
        Ok(Self {
            session,
            seat_index,
            recipient,
            dealer,
            outputs: LocalOutputs {
                pending_edges,
                pending_acceptances,
                outgoing,
                acceptances,
                shares,
            },
            workspace,
            public_frame,
            budget: budget.clone(),
        })
    }

    /// Prepare the final input graph's verifier and shared shell before claiming this attempt.
    ///
    /// The exact message bound comes from the canonical output row shapes already
    /// prepared for this same session. This owner will consume the decoded graph,
    /// without admitting or copying another raw session after private work starts.
    ///
    /// # Errors
    /// Returns the original pool or physical refusal without consuming this prepared seat.
    pub fn prepare_final_session_verifier(
        &self,
    ) -> Result<
        super::PreparedGlobalThresholdBeaconSessionVerificationV1,
        GlobalThresholdBeaconSessionError,
    > {
        super::PreparedGlobalThresholdBeaconSessionVerificationV1::new(
            self.session,
            self.workspace.capacity(),
            &self.budget,
        )
    }

    /// Produce this claimed attempt's private state into the already constructed backing.
    ///
    /// # Errors
    /// Rejects a replaced signer or genuine crypto/entropy failure. No local admission remains.
    pub fn generate(
        mut self,
        signer: &KeyPair,
    ) -> Result<LocalGlobalThresholdBeaconDkgSeatV1, LocalGlobalThresholdBeaconDkgErrorV1> {
        if signer.public_key() != self.recipient.record().validator.public_key() {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
        }
        let encryption = HybridKeyPair::generate(&mut rand::rngs::OsRng)?;
        self.recipient.fill_recipient(encryption.public());
        let recipient_key = self
            .recipient
            .sign(signer, &self.session, &mut self.workspace)?;
        let parameters = adaptive_beacon_parameters(&self.session)?;
        let (dealer_secret, validated) =
            DasRenDealerSecret::generate(&parameters, self.seat_index)?;
        self.dealer.fill_dealer(&validated);
        let dealer_commitment = self
            .dealer
            .sign(signer, &self.session, &mut self.workspace)?;
        Ok(LocalGlobalThresholdBeaconDkgSeatV1 {
            session: self.session,
            seat_index: self.seat_index,
            recipient_key,
            dealer_commitment,
            encryption,
            dealer_secret: Some(dealer_secret),
            outputs: self.outputs,
            workspace: self.workspace,
            public_frame: self.public_frame,
            budget: self.budget,
            delivered: false,
            accepted: false,
            extracted: false,
            aborted: false,
            delivery_input: None,
            acceptance_input: None,
        })
    }
}

/// One local validator's non-cloneable, non-serializable, prepaid DKG custody state.
/// All retained public fields and private-share slots belong to the original operation pool.
pub struct LocalGlobalThresholdBeaconDkgSeatV1 {
    session: GlobalThresholdBeaconDkgSessionV1,
    seat_index: u16,
    recipient_key: RetainedPayload<GlobalThresholdBeaconDkgRecipientKeyV1>,
    dealer_commitment: RetainedPayload<GlobalThresholdBeaconDkgDealerCommitmentV1>,
    encryption: HybridKeyPair,
    dealer_secret: Option<DasRenDealerSecret<BeaconPurpose>>,
    outputs: LocalOutputs,
    workspace: DkgMessageWorkspace,
    public_frame: PublicFrame,
    budget: AllocationBudget,
    delivered: bool,
    accepted: bool,
    extracted: bool,
    aborted: bool,
    delivery_input: Option<[u8; 32]>,
    acceptance_input: Option<[u8; 32]>,
}
impl LocalGlobalThresholdBeaconDkgSeatV1 {
    /// Borrow this seat's original signed publication without duplicating its backing.
    pub fn publication(
        &self,
    ) -> (
        &GlobalThresholdBeaconDkgRecipientKeyV1,
        &GlobalThresholdBeaconDkgDealerCommitmentV1,
    ) {
        (self.recipient_key.get(), self.dealer_commitment.get())
    }

    /// Fill and sign every already allocated dealer edge, then erase the polynomial.
    ///
    /// # Errors
    /// Invalid input preserves the original private attempt. Genuine producer failure
    /// after taking the polynomial aborts the attempt; no resource admission occurs there.
    pub fn deliver(
        &mut self,
        recipient_keys: &[GlobalThresholdBeaconDkgRecipientKeyV1],
        dealer_commitments: &[GlobalThresholdBeaconDkgDealerCommitmentV1],
        delivery_height: u64,
        signer: &KeyPair,
    ) -> Result<
        impl ExactSizeIterator<Item = &GlobalThresholdBeaconDkgEncryptedShareV1> + DoubleEndedIterator,
        LocalGlobalThresholdBeaconDkgErrorV1,
    > {
        if self.delivered || self.aborted || self.dealer_secret.is_none() {
            return Err(GlobalThresholdBeaconError::DkgTerminal.into());
        }
        let seats = usize::from(self.session.committee_size);
        if recipient_keys.len() != seats
            || dealer_commitments.len() != seats
            || recipient_keys[usize::from(self.seat_index - 1)] != *self.recipient_key.get()
            || dealer_commitments[usize::from(self.seat_index - 1)] != *self.dealer_commitment.get()
            || signer.public_key() != self.recipient_key.get().validator.public_key()
            || delivery_height < self.session.commitments_end_height
            || delivery_height >= self.session.deliveries_end_height
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
        }
        let parameters = adaptive_beacon_parameters(&self.session)?;
        DkgSnapshotRef::commitments(
            &self.session,
            parameters.h_bytes(),
            parameters.v_bytes(),
            recipient_keys,
            dealer_commitments,
        )
        .validate_with_verifier(&mut self.workspace)?;
        for dealer in dealer_commitments {
            let _ = super::verify_adaptive_dealer(&parameters, dealer)?;
        }
        let validated = super::verify_adaptive_dealer(&parameters, self.dealer_commitment.get())?;
        // Every output allocation is already physically owned. No admission follows secret take.
        let secret = self
            .dealer_secret
            .take()
            .ok_or(GlobalThresholdBeaconError::DkgTerminal)?;
        self.aborted = true;
        for (offset, recipient) in recipient_keys.iter().enumerate() {
            let share = secret.private_share(&parameters, &validated, recipient.recipient_index)?;
            let recipient_crypto = HybridPublicKey::from_bytes(
                recipient.x25519_public_key,
                &recipient.mlkem768_public_key,
            )?;
            let mut pending = self.outputs.pending_edges.as_mut_slice()[offset]
                .take()
                .ok_or(GlobalThresholdBeaconError::DkgTerminal)?;
            pending.bind_edge(
                super::global_threshold_beacon_dkg_dealer_commitment_hash_v1(
                    &self.session,
                    self.dealer_commitment.get(),
                ),
                super::global_threshold_beacon_dkg_recipient_key_hash_v1(&self.session, recipient),
                delivery_height,
            );
            let aad = self.workspace.write(DkgSignaturePreimage::PrivateEdgeAad(
                &self.session,
                pending.record(),
            ))?;
            let (kem, encrypted) = seal_das_ren_private_share(&share, &recipient_crypto, aad)?;
            pending.fill_edge(&kem, &encrypted);
            let edge = pending.sign(signer, &self.session, &mut self.workspace)?;
            self.outputs.outgoing.push_reserved(edge);
        }
        drop(secret);
        self.delivery_input = Some(public_commitments_hash(
            &self.session,
            recipient_keys,
            dealer_commitments,
        )?);
        self.delivered = true;
        self.aborted = false;
        Ok(self
            .outputs
            .outgoing
            .as_slice()
            .iter()
            .map(RetainedPayload::get))
    }

    /// Authenticate all public edges, decrypt only this seat's shares, then sign acknowledgments.
    ///
    /// # Errors
    /// Invalid public/private contributions leave secret custody intact. All local
    /// storage was prepaid before the attempt was claimed; entropy failure aborts.
    pub fn accept(
        &mut self,
        snapshot: &GlobalThresholdBeaconDkgSnapshotV1,
        accepted_height: u64,
        signer: &KeyPair,
    ) -> Result<
        impl ExactSizeIterator<Item = &GlobalThresholdBeaconDkgShareAcceptanceV1> + DoubleEndedIterator,
        LocalGlobalThresholdBeaconDkgErrorV1,
    > {
        if self.accepted || self.aborted || !self.delivered {
            return Err(GlobalThresholdBeaconError::DkgTerminal.into());
        }
        let seats = usize::from(self.session.committee_size);
        if snapshot.session != self.session
            || snapshot.recipient_keys.len() != seats
            || snapshot.dealer_commitments.len() != seats
            || snapshot.encrypted_shares.len() != seats * seats
            || !snapshot.share_acceptances.is_empty()
            || snapshot.recipient_keys[usize::from(self.seat_index - 1)]
                != *self.recipient_key.get()
            || snapshot.dealer_commitments[usize::from(self.seat_index - 1)]
                != *self.dealer_commitment.get()
            || snapshot
                .encrypted_shares
                .iter()
                .filter(|edge| edge.dealer_index == self.seat_index)
                .ne(self
                    .outputs
                    .outgoing
                    .as_slice()
                    .iter()
                    .map(RetainedPayload::get))
            || signer.public_key() != self.recipient_key.get().validator.public_key()
            || accepted_height < self.session.deliveries_end_height
            || accepted_height >= self.session.acceptances_end_height
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
        }
        DkgSnapshotRef::from(snapshot).validate_with_verifier(&mut self.workspace)?;
        let parameters = adaptive_beacon_parameters(&self.session)?;
        for dealer in &snapshot.dealer_commitments {
            let _ = super::verify_adaptive_dealer(&parameters, dealer)?;
        }
        self.outputs.shares.truncate(0);
        for dealer_offset in 0..seats {
            let edge = &snapshot.encrypted_shares
                [dealer_offset * seats + usize::from(self.seat_index - 1)];
            let kem = HybridKemCiphertext::from_parts(
                edge.ephemeral_x25519_public_key,
                &edge.mlkem768_ciphertext,
            )?;
            let dealer = super::verify_adaptive_dealer(
                &parameters,
                &snapshot.dealer_commitments[dealer_offset],
            )?;
            let aad = self
                .workspace
                .write(DkgSignaturePreimage::PrivateEdgeAad(&self.session, edge))?;
            match open_das_ren_private_share(
                &parameters,
                &dealer,
                edge.recipient_index,
                self.encryption.secret(),
                &kem,
                &edge.encrypted_share,
                aad,
            ) {
                Ok(share) => self.outputs.shares.push_reserved(share),
                Err(error) => {
                    self.outputs.shares.truncate(0);
                    return Err(error.into());
                }
            }
        }
        // Only after every private equation succeeds may any prepared acknowledgment be consumed.
        self.aborted = true;
        for dealer_offset in 0..seats {
            let edge = &snapshot.encrypted_shares
                [dealer_offset * seats + usize::from(self.seat_index - 1)];
            let mut pending = self.outputs.pending_acceptances.as_mut_slice()[dealer_offset]
                .take()
                .ok_or(GlobalThresholdBeaconError::DkgTerminal)?;
            pending.bind_acceptance(
                edge.dealer_commitment_hash,
                super::global_threshold_beacon_dkg_encrypted_share_hash_v1(&self.session, edge),
                accepted_height,
            );
            self.outputs.acceptances.push_reserved(pending.sign(
                signer,
                &self.session,
                &mut self.workspace,
            )?);
        }
        self.acceptance_input = Some(*iroha_crypto::HashOf::new(snapshot).as_ref());
        self.accepted = true;
        self.aborted = false;
        Ok(self
            .outputs
            .acceptances
            .as_slice()
            .iter()
            .map(RetainedPayload::get))
    }

    /// Aggregate only this seat's private contributions after exact same-pool transcript finality.
    ///
    /// # Errors
    /// Rejects a changed transcript, foreign owner, missing shares or repeated extraction.
    pub fn finalize_private_share(
        &mut self,
        validated: &super::ValidatedGlobalThresholdBeaconSessionV1,
    ) -> Result<Zeroizing<[[u8; 32]; 3]>, LocalGlobalThresholdBeaconDkgErrorV1> {
        if !self.accepted || self.extracted || self.aborted {
            return Err(GlobalThresholdBeaconError::DkgTerminal.into());
        }
        if !validated.belongs_to(&self.budget) {
            return Err(GlobalThresholdBeaconSessionError::ForeignReservation.into());
        }
        let transcript = &validated.record().adaptive_dkg;
        if transcript.session != self.session
            || transcript.recipient_keys[usize::from(self.seat_index - 1)]
                != *self.recipient_key.get()
            || transcript.dealer_commitments[usize::from(self.seat_index - 1)]
                != *self.dealer_commitment.get()
            || transcript
                .encrypted_shares
                .iter()
                .filter(|edge| edge.dealer_index == self.seat_index)
                .ne(self
                    .outputs
                    .outgoing
                    .as_slice()
                    .iter()
                    .map(RetainedPayload::get))
            || transcript
                .share_acceptances
                .iter()
                .filter(|ack| ack.recipient_index == self.seat_index)
                .ne(self
                    .outputs
                    .acceptances
                    .as_slice()
                    .iter()
                    .map(RetainedPayload::get))
        {
            return Err(GlobalThresholdBeaconError::TranscriptMismatch.into());
        }
        let aggregate = AdaptiveThresholdBlsSecretShare::from_dealer_shares(
            validated.transcript(),
            self.outputs.shares.as_slice(),
        )?;
        self.outputs.shares.truncate(0);
        self.outputs.acceptances.truncate(0);
        self.extracted = true;
        Ok(aggregate.into_components_for_runtime_custody())
    }
    /// Stable authenticated attempt identity for the operator's one-shot journal.
    #[must_use]
    pub fn attempt_id(&self) -> [u8; 32] {
        self.session.attempt_id
    }
    /// Exact local seat identity.
    #[must_use]
    pub fn seat_index(&self) -> u16 {
        self.seat_index
    }
    /// Hash of the original signed publication, without copying its graph.
    #[must_use]
    pub fn publication_hash(&self) -> [u8; 32] {
        Hash::new_from_chunks(&[
            b"iroha.global-beacon.local-seat-publication.v1\0",
            &self.session.attempt_id,
            &super::global_threshold_beacon_dkg_recipient_key_hash_v1(
                &self.session,
                self.recipient_key.get(),
            ),
            &super::global_threshold_beacon_dkg_dealer_commitment_hash_v1(
                &self.session,
                self.dealer_commitment.get(),
            ),
        ])
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beacon::{
        AdaptiveGlobalThresholdBeaconDkgCryptoV1, GlobalThresholdBeaconDkgStateV1,
    };
    use iroha_crypto::{Algorithm, HashOf};
    use iroha_data_model::{NetworkId, block::BlockHeader};
    use std::collections::BTreeSet;

    pub(super) fn signed_session(
        seats: u16,
    ) -> (GlobalThresholdBeaconDkgSessionV1, Vec<KeyPair>, Vec<PeerId>) {
        let mut signers = (1..=seats)
            .map(|index| {
                KeyPair::try_from_seed(
                    vec![u8::try_from(index).expect("test seat fits u8"); 32],
                    Algorithm::BlsNormal,
                )
                .expect("BLS signer")
            })
            .collect::<Vec<_>>();
        signers.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = signers
            .iter()
            .map(|signer| PeerId::new(signer.public_key().clone()))
            .collect::<Vec<_>>();
        let session = GlobalThresholdBeaconDkgSessionV1 {
            version: 1,
            network_id: NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"per-seat-dkg")),
            ),
            session_id: Hash::new_from_chunks(&[b"per-seat-session", &seats.to_be_bytes()]).into(),
            attempt_id: Hash::new_from_chunks(&[b"per-seat-attempt", &seats.to_be_bytes()]).into(),
            authority_generation: 1,
            roster_hash: global_threshold_beacon_roster_hash_v1(&roster),
            committee_size: seats,
            threshold: (seats - 1) / 3 + 1,
            start_height: 1,
            commitments_end_height: 2,
            deliveries_end_height: 3,
            acceptances_end_height: 4,
        };
        (session, signers, roster)
    }

    #[test]
    fn local_seats_complete_only_their_own_signed_edges_at_four_and_seven() {
        for seats in [4, 7] {
            let (session, signers, roster) = signed_session(seats);
            let budget = crate::beacon::fixtures::fixture_budget();
            let mut local = signers
                .iter()
                .enumerate()
                .map(|(offset, signer)| {
                    PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(
                        session,
                        &roster,
                        u16::try_from(offset + 1).expect("seat"),
                        signer,
                        &budget,
                    )
                    .expect("prepared backing")
                    .generate(signer)
                    .expect("one actual dealer owner")
                })
                .collect::<Vec<_>>();
            let publications = local
                .iter()
                .map(|owner| {
                    let (key, commitment) = owner.publication();
                    (key.clone(), commitment.clone())
                })
                .collect::<Vec<_>>();
            let keys = publications
                .iter()
                .map(|pair| pair.0.clone())
                .collect::<Vec<_>>();
            let commitments = publications
                .iter()
                .map(|pair| pair.1.clone())
                .collect::<Vec<_>>();
            let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
            let mut public = GlobalThresholdBeaconDkgStateV1::new(session, &crypto, &budget)
                .expect("public reducer");
            for key in &keys {
                public.record_recipient_key(1, key).expect("signed key");
            }
            for commitment in &commitments {
                public
                    .record_dealer_commitment(1, commitment, &crypto)
                    .expect("signed commitment");
            }
            for (owner, signer) in local.iter_mut().zip(&signers) {
                let edges = owner
                    .deliver(&keys, &commitments, 2, signer)
                    .expect("private edges");
                assert_eq!(edges.len(), usize::from(seats));
                for edge in edges {
                    public.record_encrypted_share(2, edge).expect("signed edge");
                }
                assert!(matches!(
                    owner.deliver(&keys, &commitments, 2, signer),
                    Err(LocalGlobalThresholdBeaconDkgErrorV1::Invalid(
                        GlobalThresholdBeaconError::DkgTerminal
                    ))
                ));
            }
            let snapshot = public
                .public_snapshot()
                .expect("complete encrypted snapshot");
            assert_eq!(snapshot.encrypted_shares.len(), usize::from(seats).pow(2));
            for (owner, signer) in local.iter_mut().zip(&signers) {
                let accepted = owner
                    .accept(&snapshot, 3, signer)
                    .expect("private validation");
                assert_eq!(accepted.len(), usize::from(seats));
                for acceptance in accepted {
                    public
                        .record_share_acceptance(3, acceptance)
                        .expect("signed acceptance");
                }
            }
            let record = public
                .finalize(4, &crypto)
                .expect("all-edge finality")
                .clone();
            let binding = super::super::GlobalThresholdBeaconSessionBindingV1 {
                network_id: record.network_id,
                session_id: record.session_id,
                roster_hash: record.roster_hash,
                transcript_hash: record.transcript_hash,
            };
            let sealed = super::super::validate_global_threshold_beacon_session_v1(
                &record, &binding, &budget,
            )
            .expect("complete authenticated public graph");
            let components = local
                .iter_mut()
                .map(|owner| owner.finalize_private_share(&sealed).expect("local share"))
                .collect::<Vec<_>>();
            assert_eq!(components.len(), usize::from(seats));
            assert_eq!(
                components
                    .iter()
                    .map(|component| {
                        Hash::new(&component.iter().flatten().copied().collect::<Vec<_>>())
                    })
                    .collect::<BTreeSet<_>>()
                    .len(),
                usize::from(seats),
            );
        }
    }
}

#[cfg(test)]
#[path = "dkg_local_seat/ownership_tests.rs"]
mod ownership_tests;
