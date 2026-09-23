//! One-seat, one-attempt private owner for distributed beacon DKG.

use iroha_crypto::{
    Hash, KeyPair,
    hybrid::HybridKeyPair,
    threshold_bls::{
        AdaptiveThresholdBlsSecretShare, BeaconPurpose, DasRenDealerSecret, DasRenPrivateShare,
    },
};
use iroha_data_model::consensus::{
    GlobalThresholdBeaconDkgConstantProofV1, GlobalThresholdBeaconDkgDealerCommitmentV1,
    GlobalThresholdBeaconDkgEncryptedShareV1, GlobalThresholdBeaconDkgRecipientKeyV1,
    GlobalThresholdBeaconDkgSessionV1, GlobalThresholdBeaconDkgShareAcceptanceV1,
    GlobalThresholdBeaconKeySessionV1,
};
use iroha_model_base::peer::PeerId;
use zeroize::Zeroizing;

use super::{
    AdaptiveGlobalThresholdBeaconDkgCryptoV1, GlobalThresholdBeaconDkgSnapshotV1,
    GlobalThresholdBeaconDkgStateV1, GlobalThresholdBeaconError,
    GlobalThresholdBeaconSessionBindingV1, accept_global_threshold_beacon_dkg_private_edge_v1,
    adaptive_beacon_parameters, global_threshold_beacon_roster_hash_v1,
    seal_global_threshold_beacon_dkg_private_edge_v1,
    sign_global_threshold_beacon_dkg_dealer_commitment_v1,
    sign_global_threshold_beacon_dkg_recipient_key_v1, validate_global_threshold_beacon_session_v1,
};

/// One local validator's non-cloneable and non-serializable DKG custody state.
///
/// The caller must obtain the frozen session and ordered roster through
/// independently verified selection or signed genesis finality. This object
/// owns exactly one dealer polynomial and one hybrid recipient secret; it never
/// reconstructs another seat's contribution.
pub struct LocalGlobalThresholdBeaconDkgSeatV1 {
    session: GlobalThresholdBeaconDkgSessionV1,
    seat_index: u16,
    recipient_key: GlobalThresholdBeaconDkgRecipientKeyV1,
    dealer_commitment: GlobalThresholdBeaconDkgDealerCommitmentV1,
    encryption: HybridKeyPair,
    dealer_secret: Option<DasRenDealerSecret<BeaconPurpose>>,
    outgoing_edges: Option<Vec<GlobalThresholdBeaconDkgEncryptedShareV1>>,
    accepted_shares: Option<Vec<DasRenPrivateShare<BeaconPurpose>>>,
    acceptances: Option<Vec<GlobalThresholdBeaconDkgShareAcceptanceV1>>,
}

impl LocalGlobalThresholdBeaconDkgSeatV1 {
    /// Start one fresh local seat for the authenticated, frozen attempt.
    ///
    /// # Errors
    /// Rejects a mismatched roster/seat or RNG and DKG construction failure.
    pub fn new(
        session: GlobalThresholdBeaconDkgSessionV1,
        ordered_roster: &[PeerId],
        seat_index: u16,
        signer: &KeyPair,
    ) -> Result<Self, GlobalThresholdBeaconError> {
        let seats = usize::from(session.committee_size);
        if seats != ordered_roster.len()
            || seat_index == 0
            || usize::from(seat_index) > seats
            || ordered_roster[usize::from(seat_index - 1)].public_key() != signer.public_key()
            || global_threshold_beacon_roster_hash_v1(ordered_roster) != session.roster_hash
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession);
        }
        // Validate the schedule and cryptographic generators before allocating
        // a one-shot dealer secret. Caller finality authenticates the session.
        let _ = GlobalThresholdBeaconDkgStateV1::new(
            session,
            &AdaptiveGlobalThresholdBeaconDkgCryptoV1,
        )?;
        let encryption = HybridKeyPair::generate(&mut rand::rngs::OsRng)
            .map_err(|_| GlobalThresholdBeaconError::InvalidDkgRecipientKey)?;
        let recipient_key = sign_global_threshold_beacon_dkg_recipient_key_v1(
            &session,
            seat_index,
            signer,
            encryption.public(),
        )?;
        let parameters = adaptive_beacon_parameters(&session)?;
        let (dealer_secret, validated) = DasRenDealerSecret::generate(&parameters, seat_index)?;
        let dealer_commitment = sign_global_threshold_beacon_dkg_dealer_commitment_v1(
            &session,
            &recipient_key,
            signer,
            GlobalThresholdBeaconDkgDealerCommitmentV1 {
                dealer_index: seat_index,
                coefficient_commitments: validated
                    .coefficients()
                    .iter()
                    .map(|coefficient| *coefficient.as_bytes())
                    .collect(),
                constant_term_proof: GlobalThresholdBeaconDkgConstantProofV1 {
                    commitment: *validated.constant_proof().commitment_bytes(),
                    response: *validated.constant_proof().response_bytes(),
                },
                signature: iroha_crypto::Signature::from_bytes(&[]),
            },
        )?;
        Ok(Self {
            session,
            seat_index,
            recipient_key,
            dealer_commitment,
            encryption,
            dealer_secret: Some(dealer_secret),
            outgoing_edges: None,
            accepted_shares: None,
            acceptances: None,
        })
    }

    /// Return this seat's signed public encryption key and dealer commitment.
    #[must_use]
    pub fn publication(
        &self,
    ) -> (
        GlobalThresholdBeaconDkgRecipientKeyV1,
        GlobalThresholdBeaconDkgDealerCommitmentV1,
    ) {
        (self.recipient_key.clone(), self.dealer_commitment.clone())
    }

    /// Emit this dealer's encrypted private edge to every exact recipient.
    ///
    /// The dealer polynomial is consumed exactly once and erased after all
    /// edges are sealed. A failed seal aborts this local attempt.
    ///
    /// # Errors
    /// Rejects a missing, forged, reordered or wrong-roster publication set.
    pub fn deliver(
        &mut self,
        recipient_keys: &[GlobalThresholdBeaconDkgRecipientKeyV1],
        dealer_commitments: &[GlobalThresholdBeaconDkgDealerCommitmentV1],
        delivery_height: u64,
        signer: &KeyPair,
    ) -> Result<Vec<GlobalThresholdBeaconDkgEncryptedShareV1>, GlobalThresholdBeaconError> {
        if self.outgoing_edges.is_some() || self.dealer_secret.is_none() {
            return Err(GlobalThresholdBeaconError::DkgTerminal);
        }
        let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
        let mut public = GlobalThresholdBeaconDkgStateV1::new(self.session, &crypto)?;
        for key in recipient_keys {
            public.record_recipient_key(self.session.start_height, key.clone())?;
        }
        for commitment in dealer_commitments {
            public.record_dealer_commitment(
                self.session.start_height,
                commitment.clone(),
                &crypto,
            )?;
        }
        if recipient_keys.len() != usize::from(self.session.committee_size)
            || dealer_commitments.len() != recipient_keys.len()
            || recipient_keys[usize::from(self.seat_index - 1)] != self.recipient_key
            || dealer_commitments[usize::from(self.seat_index - 1)] != self.dealer_commitment
            || signer.public_key() != self.recipient_key.validator.public_key()
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession);
        }
        let parameters = adaptive_beacon_parameters(&self.session)?;
        let validated = super::verify_adaptive_dealer(&parameters, &self.dealer_commitment)?;
        let secret = self
            .dealer_secret
            .take()
            .ok_or(GlobalThresholdBeaconError::DkgTerminal)?;
        let mut edges = Vec::with_capacity(recipient_keys.len());
        for recipient in recipient_keys {
            let share = secret.private_share(&parameters, &validated, recipient.recipient_index)?;
            edges.push(seal_global_threshold_beacon_dkg_private_edge_v1(
                &self.session,
                &self.recipient_key,
                signer,
                &self.dealer_commitment,
                recipient,
                &share,
                delivery_height,
            )?);
        }
        drop(secret);
        self.outgoing_edges = Some(edges.clone());
        Ok(edges)
    }

    /// Decrypt and acknowledge this recipient's exact inbound edges.
    ///
    /// # Errors
    /// Rejects any incomplete, forged or changed all-edge public snapshot, or
    /// any private share which fails the signed dealer commitment equation.
    pub fn accept(
        &mut self,
        snapshot: &GlobalThresholdBeaconDkgSnapshotV1,
        accepted_height: u64,
        signer: &KeyPair,
    ) -> Result<Vec<GlobalThresholdBeaconDkgShareAcceptanceV1>, GlobalThresholdBeaconError> {
        if self.acceptances.is_some() {
            return Err(GlobalThresholdBeaconError::DkgTerminal);
        }
        let outgoing = self
            .outgoing_edges
            .as_ref()
            .ok_or(GlobalThresholdBeaconError::DkgTerminal)?;
        if snapshot.session != self.session
            || snapshot.recipient_keys.len() != usize::from(self.session.committee_size)
            || snapshot.dealer_commitments.len() != snapshot.recipient_keys.len()
            || snapshot.encrypted_shares.len()
                != snapshot.recipient_keys.len() * snapshot.recipient_keys.len()
            || !snapshot.share_acceptances.is_empty()
            || snapshot.recipient_keys[usize::from(self.seat_index - 1)] != self.recipient_key
            || snapshot.dealer_commitments[usize::from(self.seat_index - 1)]
                != self.dealer_commitment
            || snapshot
                .encrypted_shares
                .iter()
                .filter(|edge| edge.dealer_index == self.seat_index)
                .ne(outgoing.iter())
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession);
        }
        let _ = GlobalThresholdBeaconDkgStateV1::from_snapshot(
            snapshot.clone(),
            &AdaptiveGlobalThresholdBeaconDkgCryptoV1,
        )?;
        let mut shares = Vec::with_capacity(snapshot.recipient_keys.len());
        let mut acceptances = Vec::with_capacity(snapshot.recipient_keys.len());
        for dealer_offset in 0..snapshot.recipient_keys.len() {
            let edge = &snapshot.encrypted_shares
                [dealer_offset * snapshot.recipient_keys.len() + usize::from(self.seat_index - 1)];
            let (share, acceptance) = accept_global_threshold_beacon_dkg_private_edge_v1(
                &self.session,
                &snapshot.recipient_keys[dealer_offset],
                &self.recipient_key,
                signer,
                self.encryption.secret(),
                &snapshot.dealer_commitments[dealer_offset],
                edge,
                accepted_height,
            )?;
            shares.push(share);
            acceptances.push(acceptance);
        }
        self.accepted_shares = Some(shares);
        self.acceptances = Some(acceptances.clone());
        Ok(acceptances)
    }

    /// Aggregate only this seat's private share after exact transcript finality.
    ///
    /// # Errors
    /// Rejects an altered transcript, missing accepted edges or a repeated
    /// credential extraction. This does not certify committee activation.
    pub fn finalize_private_share(
        &mut self,
        public: GlobalThresholdBeaconKeySessionV1,
    ) -> Result<Zeroizing<[[u8; 32]; 3]>, GlobalThresholdBeaconError> {
        let accepted = self
            .acceptances
            .as_ref()
            .ok_or(GlobalThresholdBeaconError::DkgTerminal)?;
        let shares = self
            .accepted_shares
            .as_ref()
            .ok_or(GlobalThresholdBeaconError::DkgTerminal)?;
        let outgoing = self
            .outgoing_edges
            .as_ref()
            .ok_or(GlobalThresholdBeaconError::DkgTerminal)?;
        let binding = GlobalThresholdBeaconSessionBindingV1 {
            network_id: public.network_id,
            session_id: public.session_id,
            roster_hash: public.roster_hash,
            transcript_hash: public.transcript_hash,
        };
        let validated = validate_global_threshold_beacon_session_v1(public, &binding)?;
        let transcript = &validated.record().adaptive_dkg;
        if transcript.session != self.session
            || transcript.recipient_keys[usize::from(self.seat_index - 1)] != self.recipient_key
            || transcript.dealer_commitments[usize::from(self.seat_index - 1)]
                != self.dealer_commitment
            || transcript
                .encrypted_shares
                .iter()
                .filter(|edge| edge.dealer_index == self.seat_index)
                .ne(outgoing.iter())
            || transcript
                .share_acceptances
                .iter()
                .filter(|acceptance| acceptance.recipient_index == self.seat_index)
                .ne(accepted.iter())
        {
            return Err(GlobalThresholdBeaconError::TranscriptMismatch);
        }
        let aggregate =
            AdaptiveThresholdBlsSecretShare::from_dealer_shares(&validated.transcript, shares)?;
        self.accepted_shares = None;
        self.acceptances = None;
        Ok(aggregate.into_components_for_runtime_custody())
    }

    /// Stable attempt identity for an operator's one-shot journal.
    #[must_use]
    pub fn attempt_id(&self) -> [u8; 32] {
        self.session.attempt_id
    }

    /// Exact local seat identity.
    #[must_use]
    pub fn seat_index(&self) -> u16 {
        self.seat_index
    }

    /// Hash of this seat's signed public publication for audit logs.
    #[must_use]
    pub fn publication_hash(&self) -> [u8; 32] {
        Hash::new_from_chunks(&[
            b"iroha.global-beacon.local-seat-publication.v1\0",
            &self.session.attempt_id,
            &super::global_threshold_beacon_dkg_recipient_key_hash_v1(
                &self.session,
                &self.recipient_key,
            ),
            &super::global_threshold_beacon_dkg_dealer_commitment_hash_v1(
                &self.session,
                &self.dealer_commitment,
            ),
        ])
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, HashOf};
    use iroha_data_model::{NetworkId, block::BlockHeader};
    use std::collections::BTreeSet;

    fn signed_session(
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
            let mut local = signers
                .iter()
                .enumerate()
                .map(|(offset, signer)| {
                    LocalGlobalThresholdBeaconDkgSeatV1::new(
                        session,
                        &roster,
                        u16::try_from(offset + 1).expect("seat"),
                        signer,
                    )
                    .expect("one actual dealer owner")
                })
                .collect::<Vec<_>>();
            let publications = local
                .iter()
                .map(LocalGlobalThresholdBeaconDkgSeatV1::publication)
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
            let mut public =
                GlobalThresholdBeaconDkgStateV1::new(session, &crypto).expect("public reducer");
            for key in &keys {
                public
                    .record_recipient_key(1, key.clone())
                    .expect("signed key");
            }
            for commitment in &commitments {
                public
                    .record_dealer_commitment(1, commitment.clone(), &crypto)
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
                assert_eq!(
                    owner.deliver(&keys, &commitments, 2, signer),
                    Err(GlobalThresholdBeaconError::DkgTerminal)
                );
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
            let components = local
                .iter_mut()
                .map(|owner| {
                    owner
                        .finalize_private_share(record.clone())
                        .expect("local share")
                })
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
