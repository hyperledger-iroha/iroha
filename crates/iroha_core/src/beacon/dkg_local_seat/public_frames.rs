//! Borrowed canonical phase frames in one original preallocated output buffer.

use super::*;
use norito::{
    NoritoSerialize,
    core::{Encoder, PayloadRef, SerializePayload, write_element_sequence},
};

#[derive(Clone)]
struct Sequence<I>(I);
impl<T: SerializePayload, I: ExactSizeIterator<Item = T> + Clone> SerializePayload for Sequence<I> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), norito::Error> {
        write_element_sequence::<T, _>(writer, self.0.clone())
    }
}

// Fixed byte arrays are inline fields so the generated canonical field kernel
// writes raw fixed bytes, exactly as it does for the authoritative snapshot DTO.
#[derive(NoritoSerialize)]
struct SnapshotView<'a> {
    session: PayloadRef<'a, GlobalThresholdBeaconDkgSessionV1>,
    generator_h: [u8; 96],
    generator_v: [u8; 96],
    recipients: PayloadRef<'a, dyn SerializePayload + 'a>,
    dealers: PayloadRef<'a, dyn SerializePayload + 'a>,
    edges: PayloadRef<'a, dyn SerializePayload + 'a>,
    acceptances: PayloadRef<'a, dyn SerializePayload + 'a>,
    height: u64,
}
impl norito::NoritoSchema for SnapshotView<'_> {
    fn nominal_name() -> String {
        <GlobalThresholdBeaconDkgSnapshotV1 as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <GlobalThresholdBeaconDkgSnapshotV1 as norito::NoritoSchema>::frame_name()
    }
    fn static_nominal_name() -> Option<&'static str> {
        <GlobalThresholdBeaconDkgSnapshotV1 as norito::NoritoSchema>::static_nominal_name()
    }
    fn static_frame_name() -> Option<&'static str> {
        <GlobalThresholdBeaconDkgSnapshotV1 as norito::NoritoSchema>::static_frame_name()
    }
}

// Measure only initialized inline placeholders and borrowed original fields
// before RNG. The same derive-generated field kernels serialize the canonical
// row DTOs. These shapes are neither signed records nor alternative decoders.
struct SignatureShape;
impl SerializePayload for SignatureShape {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), norito::Error> {
        // Signature delegates to ConstVec<u8>, whose payload uses the canonical
        // element-sequence kernel (not the raw-byte Vec<u8> specialization).
        write_element_sequence::<u8, _>(
            writer,
            std::iter::repeat_n(
                0u8,
                iroha_crypto::Algorithm::BlsNormal.signature_payload_len(),
            ),
        )
    }
}

#[derive(NoritoSerialize)]
struct RecipientShape<'a> {
    recipient_index: u16,
    validator: PayloadRef<'a, PeerId>,
    x25519_public_key: [u8; 32],
    mlkem768_public_key: PayloadRef<'a, Vec<u8>>,
    signature: SignatureShape,
}
impl<'a> From<&'a GlobalThresholdBeaconDkgRecipientKeyV1> for RecipientShape<'a> {
    fn from(row: &'a GlobalThresholdBeaconDkgRecipientKeyV1) -> Self {
        Self {
            recipient_index: row.recipient_index,
            validator: PayloadRef(&row.validator),
            x25519_public_key: row.x25519_public_key,
            mlkem768_public_key: PayloadRef(&row.mlkem768_public_key),
            signature: SignatureShape,
        }
    }
}

#[derive(NoritoSerialize)]
struct DealerShape<'a> {
    dealer_index: u16,
    coefficient_commitments: PayloadRef<'a, Vec<[u8; 96]>>,
    constant_term_proof:
        PayloadRef<'a, iroha_data_model::consensus::GlobalThresholdBeaconDkgConstantProofV1>,
    signature: SignatureShape,
}
impl<'a> From<&'a GlobalThresholdBeaconDkgDealerCommitmentV1> for DealerShape<'a> {
    fn from(row: &'a GlobalThresholdBeaconDkgDealerCommitmentV1) -> Self {
        Self {
            dealer_index: row.dealer_index,
            coefficient_commitments: PayloadRef(&row.coefficient_commitments),
            constant_term_proof: PayloadRef(&row.constant_term_proof),
            signature: SignatureShape,
        }
    }
}

#[derive(NoritoSerialize)]
struct EdgeShape<'a> {
    dealer_index: u16,
    recipient_index: u16,
    dealer_commitment_hash: [u8; 32],
    recipient_key_hash: [u8; 32],
    delivery_height: u64,
    ephemeral_x25519_public_key: [u8; 32],
    mlkem768_ciphertext: PayloadRef<'a, Vec<u8>>,
    encrypted_share: PayloadRef<'a, Vec<u8>>,
    signature: SignatureShape,
}
impl<'a> From<&'a GlobalThresholdBeaconDkgEncryptedShareV1> for EdgeShape<'a> {
    fn from(row: &'a GlobalThresholdBeaconDkgEncryptedShareV1) -> Self {
        Self {
            dealer_index: row.dealer_index,
            recipient_index: row.recipient_index,
            dealer_commitment_hash: row.dealer_commitment_hash,
            recipient_key_hash: row.recipient_key_hash,
            delivery_height: row.delivery_height,
            ephemeral_x25519_public_key: row.ephemeral_x25519_public_key,
            mlkem768_ciphertext: PayloadRef(&row.mlkem768_ciphertext),
            encrypted_share: PayloadRef(&row.encrypted_share),
            signature: SignatureShape,
        }
    }
}

#[derive(NoritoSerialize)]
struct AcceptanceShape {
    dealer_index: u16,
    recipient_index: u16,
    dealer_commitment_hash: [u8; 32],
    encrypted_share_hash: [u8; 32],
    accepted_height: u64,
    signature: SignatureShape,
}
impl From<&GlobalThresholdBeaconDkgShareAcceptanceV1> for AcceptanceShape {
    fn from(row: &GlobalThresholdBeaconDkgShareAcceptanceV1) -> Self {
        Self {
            dealer_index: row.dealer_index,
            recipient_index: row.recipient_index,
            dealer_commitment_hash: row.dealer_commitment_hash,
            encrypted_share_hash: row.encrypted_share_hash,
            accepted_height: row.accepted_height,
            signature: SignatureShape,
        }
    }
}

// Length-only canonical shapes. No synthetic record is decoded, authenticated or
// published; these views measure the exact already authenticated geometry.
#[derive(NoritoSerialize)]
struct TranscriptShape<'a> {
    session: PayloadRef<'a, GlobalThresholdBeaconDkgSessionV1>,
    generator_h: [u8; 96],
    generator_v: [u8; 96],
    dealers: PayloadRef<'a, dyn SerializePayload + 'a>,
    recipients: PayloadRef<'a, dyn SerializePayload + 'a>,
    edges: PayloadRef<'a, dyn SerializePayload + 'a>,
    acceptances: PayloadRef<'a, dyn SerializePayload + 'a>,
    qualified: PayloadRef<'a, dyn SerializePayload + 'a>,
    event_hash: [u8; 32],
    height: u64,
}
#[derive(NoritoSerialize)]
struct SessionShape<'a> {
    version: u16,
    network_id: iroha_data_model::NetworkId,
    session_id: [u8; 32],
    roster_hash: [u8; 32],
    committee_size: u16,
    threshold: u16,
    group_public_key: [u8; 96],
    shares: PayloadRef<'a, dyn SerializePayload + 'a>,
    transcript: TranscriptShape<'a>,
    dkg_contribution_hash: [u8; 32],
    transcript_hash: [u8; 32],
}
impl norito::NoritoSchema for SessionShape<'_> {
    fn nominal_name() -> String {
        <iroha_data_model::consensus::GlobalThresholdBeaconKeySessionV1 as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <iroha_data_model::consensus::GlobalThresholdBeaconKeySessionV1 as norito::NoritoSchema>::frame_name()
    }
    fn static_nominal_name() -> Option<&'static str> {
        <iroha_data_model::consensus::GlobalThresholdBeaconKeySessionV1 as norito::NoritoSchema>::static_nominal_name()
    }
    fn static_frame_name() -> Option<&'static str> {
        <iroha_data_model::consensus::GlobalThresholdBeaconKeySessionV1 as norito::NoritoSchema>::static_frame_name()
    }
}

pub(super) struct PublicFrame {
    bytes: ChargedBuffer<u8>,
    input_bounds: [usize; 3],
}
impl PublicFrame {
    pub(super) fn prepare(
        session: &GlobalThresholdBeaconDkgSessionV1,
        recipient: &GlobalThresholdBeaconDkgRecipientKeyV1,
        dealer: &GlobalThresholdBeaconDkgDealerCommitmentV1,
        edge: &GlobalThresholdBeaconDkgEncryptedShareV1,
        acceptance: &GlobalThresholdBeaconDkgShareAcceptanceV1,
        budget: &AllocationBudget,
    ) -> Result<Self, SessionGraphError> {
        let parameters =
            adaptive_beacon_parameters(session).map_err(|_| SessionGraphError::PlanChanged)?;
        let n = usize::from(session.committee_size);
        let recipient = RecipientShape::from(recipient);
        let dealer = DealerShape::from(dealer);
        let edge = EdgeShape::from(edge);
        let acceptance = AcceptanceShape::from(acceptance);
        let recipients = Sequence((0..n).map(|_| norito::core::PayloadRef(&recipient)));
        let dealers = Sequence((0..n).map(|_| norito::core::PayloadRef(&dealer)));
        let edges = Sequence((0..n * n).map(|_| norito::core::PayloadRef(&edge)));
        let acceptances = Sequence((0..n * n).map(|_| norito::core::PayloadRef(&acceptance)));
        // A local seat emits only its own n acknowledgments, even though its
        // authenticated acceptance input contains the complete n² deliveries.
        // Complete final-session input bounds below retain all n² acknowledgments.
        let local_acceptances = Sequence((0..n).map(|_| norito::core::PayloadRef(&acceptance)));
        let maximum = SnapshotView {
            session: PayloadRef(session),
            generator_h: *parameters.h_bytes(),
            generator_v: *parameters.v_bytes(),
            recipients: PayloadRef(&recipients),
            dealers: PayloadRef(&dealers),
            edges: PayloadRef(&edges),
            acceptances: PayloadRef(&local_acceptances),
            height: session.acceptances_end_height,
        };
        let size = norito::canonical_frame_len(&maximum)?;
        let empty_edges = Sequence(std::iter::empty::<PayloadRef<'_, EdgeShape<'_>>>());
        let empty_acceptances = Sequence(std::iter::empty::<PayloadRef<'_, AcceptanceShape>>());
        let commitments = SnapshotView {
            session: PayloadRef(session),
            generator_h: *parameters.h_bytes(),
            generator_v: *parameters.v_bytes(),
            recipients: PayloadRef(&recipients),
            dealers: PayloadRef(&dealers),
            edges: PayloadRef(&empty_edges),
            acceptances: PayloadRef(&empty_acceptances),
            height: session.start_height,
        };
        let deliveries = SnapshotView {
            session: PayloadRef(session),
            generator_h: *parameters.h_bytes(),
            generator_v: *parameters.v_bytes(),
            recipients: PayloadRef(&recipients),
            dealers: PayloadRef(&dealers),
            edges: PayloadRef(&edges),
            acceptances: PayloadRef(&empty_acceptances),
            height: session.commitments_end_height,
        };
        let share = iroha_data_model::consensus::GlobalThresholdBeaconPublicShareV1 {
            index: 1,
            participant_seat_binding: [0; 32],
            public_key_share: [0; 96],
        };
        let shares = Sequence((0..n).map(|_| PayloadRef(&share)));
        let qualified = Sequence(1..=session.committee_size);
        let transcript = TranscriptShape {
            session: PayloadRef(session),
            generator_h: *parameters.h_bytes(),
            generator_v: *parameters.v_bytes(),
            dealers: PayloadRef(&dealers),
            recipients: PayloadRef(&recipients),
            edges: PayloadRef(&edges),
            acceptances: PayloadRef(&acceptances),
            qualified: PayloadRef(&qualified),
            event_hash: [0; 32],
            height: session.acceptances_end_height,
        };
        let final_session = SessionShape {
            version: session.version,
            network_id: session.network_id,
            session_id: session.session_id,
            roster_hash: session.roster_hash,
            committee_size: session.committee_size,
            threshold: session.threshold,
            group_public_key: [0; 96],
            shares: PayloadRef(&shares),
            transcript,
            dkg_contribution_hash: [0; 32],
            transcript_hash: [0; 32],
        };
        let input_bounds = [
            norito::canonical_frame_len(&commitments)?,
            norito::canonical_frame_len(&deliveries)?,
            norito::canonical_frame_len(&final_session)?,
        ];
        let mut reservation = budget.try_reserve_bytes(size)?;
        Ok(Self {
            bytes: ChargedBuffer::from_reservation(size, &mut reservation)?,
            input_bounds,
        })
    }
    #[cfg(test)]
    pub(super) fn backing(&self) -> (*const u8, usize) {
        (self.bytes.as_slice().as_ptr(), self.bytes.capacity())
    }
    fn write(&mut self, view: &SnapshotView<'_>) -> Result<&[u8], SessionGraphError> {
        self.bytes.truncate(0);
        // The canonical writer checks its real second-pass length and never grows
        // the fixed destination. A mismatching shape is an invariant failure.
        norito::core::write_canonical_to_writer(
            view,
            &mut super::super::session_owner::ChargedBytesWriter(&mut self.bytes),
        )?;
        Ok(self.bytes.as_slice())
    }
}

impl super::PreparedLocalGlobalThresholdBeaconDkgSeatV1 {
    /// Canonical source lengths for the complete commitments, edges and final session.
    /// These immutable limits derive from this prepared seat's authenticated
    /// roster/threshold and canonical crypto row widths before any private RNG.
    #[must_use]
    pub fn input_frame_bounds(&self) -> [usize; 3] {
        self.public_frame.input_bounds
    }
}

impl LocalGlobalThresholdBeaconDkgSeatV1 {
    /// Borrow the currently encoded original output without reserializing it.
    /// Callers publish only after the corresponding complete phase encoder succeeds.
    #[must_use]
    pub fn encoded_public_frame(&self) -> &[u8] {
        self.public_frame.bytes.as_slice()
    }

    /// Serialize the original local publication into the frame backing admitted before RNG.
    ///
    /// # Errors
    /// Returns a concrete canonical writer error without publishing partial bytes.
    pub fn publication_frame(&mut self) -> Result<&[u8], LocalGlobalThresholdBeaconDkgErrorV1> {
        let parameters = adaptive_beacon_parameters(&self.session)?;
        let recipients =
            Sequence(std::iter::once(self.recipient_key.get()).map(norito::core::PayloadRef));
        let dealers =
            Sequence(std::iter::once(self.dealer_commitment.get()).map(norito::core::PayloadRef));
        let edges = Sequence(std::iter::empty::<
            norito::core::PayloadRef<'_, GlobalThresholdBeaconDkgEncryptedShareV1>,
        >());
        let acceptances = Sequence(std::iter::empty::<
            norito::core::PayloadRef<'_, GlobalThresholdBeaconDkgShareAcceptanceV1>,
        >());
        let view = SnapshotView {
            session: PayloadRef(&self.session),
            generator_h: *parameters.h_bytes(),
            generator_v: *parameters.v_bytes(),
            recipients: PayloadRef(&recipients),
            dealers: PayloadRef(&dealers),
            edges: PayloadRef(&edges),
            acceptances: PayloadRef(&acceptances),
            height: self.session.start_height,
        };
        Ok(self.public_frame.write(&view)?)
    }
    /// Serialize this seat's deliveries with the exact previously authenticated public inputs.
    ///
    /// # Errors
    /// Rejects a replaced source or incomplete phase; no new allocation is attempted.
    pub fn delivery_frame(
        &mut self,
        source: &GlobalThresholdBeaconDkgSnapshotV1,
    ) -> Result<&[u8], LocalGlobalThresholdBeaconDkgErrorV1> {
        if !self.delivered
            || self.aborted
            || source.session != self.session
            || source.last_updated_height != self.session.start_height
            || !source.encrypted_shares.is_empty()
            || !source.share_acceptances.is_empty()
            || Some(public_commitments_hash(
                &self.session,
                &source.recipient_keys,
                &source.dealer_commitments,
            )?) != self.delivery_input
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
        }
        DkgSnapshotRef::from(source).validate_with_verifier(&mut self.workspace)?;
        let recipients = Sequence(source.recipient_keys.iter().map(norito::core::PayloadRef));
        let dealers = Sequence(
            source
                .dealer_commitments
                .iter()
                .map(norito::core::PayloadRef),
        );
        let edges = Sequence(
            self.outputs
                .outgoing
                .as_slice()
                .iter()
                .map(|row| norito::core::PayloadRef(row.get())),
        );
        let acceptances = Sequence(std::iter::empty::<
            norito::core::PayloadRef<'_, GlobalThresholdBeaconDkgShareAcceptanceV1>,
        >());
        let height = self.outputs.outgoing.as_slice()[0].get().delivery_height;
        let view = SnapshotView {
            session: PayloadRef(&self.session),
            generator_h: source.generator_h,
            generator_v: source.generator_v,
            recipients: PayloadRef(&recipients),
            dealers: PayloadRef(&dealers),
            edges: PayloadRef(&edges),
            acceptances: PayloadRef(&acceptances),
            height,
        };
        Ok(self.public_frame.write(&view)?)
    }
    /// Serialize the complete inbound edges and this seat's exact signed acknowledgments.
    ///
    /// # Errors
    /// Rejects a changed authenticated snapshot or incomplete local acceptance.
    pub fn acceptance_frame(
        &mut self,
        source: &GlobalThresholdBeaconDkgSnapshotV1,
    ) -> Result<&[u8], LocalGlobalThresholdBeaconDkgErrorV1> {
        if !self.accepted
            || self.aborted
            || self.extracted
            || self.acceptance_input != Some(*iroha_crypto::HashOf::new(source).as_ref())
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
        }
        DkgSnapshotRef::from(source).validate_with_verifier(&mut self.workspace)?;
        let recipients = Sequence(source.recipient_keys.iter().map(norito::core::PayloadRef));
        let dealers = Sequence(
            source
                .dealer_commitments
                .iter()
                .map(norito::core::PayloadRef),
        );
        let edges = Sequence(source.encrypted_shares.iter().map(norito::core::PayloadRef));
        let acceptances = Sequence(
            self.outputs
                .acceptances
                .as_slice()
                .iter()
                .map(|row| norito::core::PayloadRef(row.get())),
        );
        let height = self.outputs.acceptances.as_slice()[0].get().accepted_height;
        let view = SnapshotView {
            session: PayloadRef(&self.session),
            generator_h: source.generator_h,
            generator_v: source.generator_v,
            recipients: PayloadRef(&recipients),
            dealers: PayloadRef(&dealers),
            edges: PayloadRef(&edges),
            acceptances: PayloadRef(&acceptances),
            height,
        };
        Ok(self.public_frame.write(&view)?)
    }
}

pub(super) fn public_commitments_hash(
    session: &GlobalThresholdBeaconDkgSessionV1,
    keys: &[GlobalThresholdBeaconDkgRecipientKeyV1],
    dealers: &[GlobalThresholdBeaconDkgDealerCommitmentV1],
) -> Result<[u8; 32], LocalGlobalThresholdBeaconDkgErrorV1> {
    let parameters = adaptive_beacon_parameters(session)?;
    Ok(
        *iroha_crypto::HashOf::new(&super::super::validation::DkgEventPreimage {
            session,
            generator_h: parameters.h_bytes(),
            generator_v: parameters.v_bytes(),
            recipient_keys: keys,
            dealer_commitments: dealers,
            encrypted_shares: &[],
            share_acceptances: &[],
            qualified_dealers: &[],
            finalized_at_height: session.start_height,
        })
        .as_ref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_allocations::allocations_during;

    fn assert_shape_matches(
        shape: &dyn SerializePayload,
        canonical: &dyn SerializePayload,
        budget: &AllocationBudget,
    ) {
        // The independently generated original DTO is the oracle. Its deliberately
        // zero signature replaces no protocol verification; only field bytes and
        // pre-RNG output geometry are under test here.
        let mut expected = Vec::new();
        norito::core::serialize_to_buffer(canonical, &mut expected).unwrap();
        assert_eq!(
            norito::core::encoded_payload_len(shape).unwrap(),
            expected.len()
        );
        let mut reservation = budget.try_reserve_bytes(expected.len()).unwrap();
        let mut bytes = ChargedBuffer::from_reservation(expected.len(), &mut reservation).unwrap();
        let original = (bytes.as_slice().as_ptr(), bytes.capacity());
        let before = budget.reserved_bytes();
        let blocker = budget
            .try_reserve_bytes(budget.limit_bytes() - before)
            .unwrap();
        assert_eq!(
            allocations_during(|| {
                norito::core::serialize_to_writer(
                    shape,
                    &mut super::super::super::session_owner::ChargedBytesWriter(&mut bytes),
                )
                .unwrap();
            }),
            0,
        );
        assert_eq!((bytes.as_slice().as_ptr(), bytes.capacity()), original);
        assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
        assert!(
            bytes.as_slice() == expected.as_slice(),
            "canonical row bytes differ"
        );
        drop(bytes);
        drop(blocker);
    }

    #[test]
    fn prepared_row_shapes_use_canonical_fixed_arrays_and_signature_elements_at_four_and_thirty_one()
     {
        for n in [4, 31] {
            let (session, keys, roster) = super::super::tests::signed_session(n);
            let budget = crate::beacon::fixtures::fixture_budget();
            let prepared = PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(
                session, &roster, 1, &keys[0], &budget,
            )
            .unwrap();
            let original = budget.reserved_bytes();
            let signature = || iroha_crypto::Signature::from_bytes(&[0; 96]);
            let mut recipient = prepared.recipient.record().clone();
            recipient.x25519_public_key = [0xA3; 32];
            recipient.signature = signature();
            assert_shape_matches(&RecipientShape::from(&recipient), &recipient, &budget);
            let mut dealer = prepared.dealer.record().clone();
            dealer.constant_term_proof.commitment = [0x6D; 96];
            dealer.constant_term_proof.response = [0x5E; 32];
            dealer.signature = signature();
            assert_shape_matches(&DealerShape::from(&dealer), &dealer, &budget);
            let mut edge = prepared.outputs.pending_edges.as_slice()[0]
                .as_ref()
                .unwrap()
                .record()
                .clone();
            edge.dealer_commitment_hash = [0x81; 32];
            edge.recipient_key_hash = [0x23; 32];
            edge.ephemeral_x25519_public_key = [0x42; 32];
            edge.signature = signature();
            assert_shape_matches(&EdgeShape::from(&edge), &edge, &budget);
            let mut acceptance = prepared.outputs.pending_acceptances.as_slice()[0]
                .as_ref()
                .unwrap()
                .record()
                .clone();
            acceptance.dealer_commitment_hash = [0x17; 32];
            acceptance.encrypted_share_hash = [0xB4; 32];
            acceptance.signature = signature();
            assert_shape_matches(&AcceptanceShape::from(&acceptance), &acceptance, &budget);
            assert_eq!(budget.reserved_bytes(), original);
            drop(prepared);
            assert_eq!(budget.reserved_bytes(), 0);
        }
    }

    #[test]
    fn local_output_keeps_exact_n_acknowledgments_and_complete_n_squared_input_at_four_and_thirty_one()
     {
        use iroha_data_model::consensus::{
            GlobalThresholdBeaconDkgTranscriptV1, GlobalThresholdBeaconKeySessionV1,
            GlobalThresholdBeaconPublicShareV1,
        };

        for n in [4, 31] {
            let (session, keys, roster) = super::super::tests::signed_session(n);
            let source_budget = crate::beacon::fixtures::fixture_budget();
            let prepared = PreparedLocalGlobalThresholdBeaconDkgSeatV1::new(
                session,
                &roster,
                1,
                &keys[0],
                &source_budget,
            )
            .unwrap();
            // These independently materialized canonical DTOs are sizing oracles,
            // not authenticated DKG messages. Signatures use the actual fixed BLS
            // width; no synthetic proof or signature is admitted or published.
            let signature = || iroha_crypto::Signature::from_bytes(&[0; 96]);
            let mut recipient = prepared.recipient.record().clone();
            recipient.signature = signature();
            let mut dealer = prepared.dealer.record().clone();
            dealer.signature = signature();
            let mut edge = prepared.outputs.pending_edges.as_slice()[0]
                .as_ref()
                .unwrap()
                .record()
                .clone();
            edge.signature = signature();
            let mut acceptance = prepared.outputs.pending_acceptances.as_slice()[0]
                .as_ref()
                .unwrap()
                .record()
                .clone();
            acceptance.signature = signature();
            let count = usize::from(n);
            let parameters = adaptive_beacon_parameters(&session).unwrap();
            let output = GlobalThresholdBeaconDkgSnapshotV1 {
                session,
                generator_h: *parameters.h_bytes(),
                generator_v: *parameters.v_bytes(),
                recipient_keys: vec![recipient.clone(); count],
                dealer_commitments: vec![dealer.clone(); count],
                encrypted_shares: vec![edge.clone(); count * count],
                share_acceptances: vec![acceptance.clone(); count],
                last_updated_height: session.acceptances_end_height,
            };
            let encoded_output = norito::encode_canonical(&output).unwrap();
            let mut complete = output.clone();
            complete.share_acceptances = vec![acceptance.clone(); count * count];
            let encoded_complete = norito::encode_canonical(&complete).unwrap();
            assert!(encoded_complete.len() > encoded_output.len());
            let final_session = GlobalThresholdBeaconKeySessionV1 {
                version: session.version,
                network_id: session.network_id,
                session_id: session.session_id,
                roster_hash: session.roster_hash,
                committee_size: n,
                threshold: session.threshold,
                group_public_key: [0; 96],
                public_shares: vec![
                    GlobalThresholdBeaconPublicShareV1 {
                        index: 1,
                        participant_seat_binding: [0; 32],
                        public_key_share: [0; 96],
                    };
                    count
                ],
                adaptive_dkg: GlobalThresholdBeaconDkgTranscriptV1 {
                    session,
                    generator_h: complete.generator_h,
                    generator_v: complete.generator_v,
                    dealer_commitments: complete.dealer_commitments.clone(),
                    recipient_keys: complete.recipient_keys.clone(),
                    encrypted_shares: complete.encrypted_shares.clone(),
                    share_acceptances: complete.share_acceptances.clone(),
                    qualified_dealers: (1..=n).collect(),
                    event_hash: [0; 32],
                    finalized_at_height: session.acceptances_end_height,
                },
                dkg_contribution_hash: [0; 32],
                transcript_hash: [0; 32],
            };
            let encoded_final = norito::encode_canonical(&final_session).unwrap();
            let budget = AllocationBudget::new(encoded_output.len());
            let prepare_frame =
                || PublicFrame::prepare(&session, &recipient, &dealer, &edge, &acceptance, &budget);
            let mut admitted = None;
            assert_eq!(
                allocations_during(|| {
                    admitted = Some(prepare_frame().unwrap());
                }),
                1,
                "one exact original output allocation, no full-committee output copy"
            );
            let mut frame = admitted.unwrap();
            assert_eq!(frame.backing().1, encoded_output.len());
            assert_eq!(
                frame.input_bounds[2],
                encoded_final.len(),
                "all n² input acknowledgments remain independently admitted"
            );
            let mut commitments = complete.clone();
            commitments.encrypted_shares.clear();
            commitments.share_acceptances.clear();
            commitments.last_updated_height = session.start_height;
            assert_eq!(
                frame.input_bounds[0],
                norito::canonical_frame_len(&commitments).unwrap()
            );
            let mut deliveries = complete.clone();
            deliveries.share_acceptances.clear();
            deliveries.last_updated_height = session.commitments_end_height;
            assert_eq!(
                frame.input_bounds[1],
                norito::canonical_frame_len(&deliveries).unwrap()
            );
            let original = frame.backing();
            assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
            let view = SnapshotView {
                session: PayloadRef(&session),
                generator_h: output.generator_h,
                generator_v: output.generator_v,
                recipients: PayloadRef(&output.recipient_keys),
                dealers: PayloadRef(&output.dealer_commitments),
                edges: PayloadRef(&output.encrypted_shares),
                acceptances: PayloadRef(&output.share_acceptances),
                height: output.last_updated_height,
            };
            assert_eq!(
                allocations_during(|| {
                    frame.write(&view).unwrap();
                }),
                0,
                "the exact local maximum writes while its original pool is saturated"
            );
            assert_eq!(frame.bytes.as_slice(), encoded_output);
            assert_eq!(frame.backing(), original);
            let full_view = SnapshotView {
                session: PayloadRef(&session),
                generator_h: complete.generator_h,
                generator_v: complete.generator_v,
                recipients: PayloadRef(&complete.recipient_keys),
                dealers: PayloadRef(&complete.dealer_commitments),
                edges: PayloadRef(&complete.encrypted_shares),
                acceptances: PayloadRef(&complete.share_acceptances),
                height: complete.last_updated_height,
            };
            assert!(
                matches!(frame.write(&full_view), Err(SessionGraphError::Encoding(_))),
                "a full-committee output cannot grow the local destination"
            );
            assert_eq!(frame.backing(), original);
            assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
            frame
                .write(&view)
                .expect("same original backing retries the actual local output");
            assert_eq!(frame.bytes.as_slice(), encoded_output);
            drop(frame);
            assert_eq!(budget.reserved_bytes(), 0);

            let narrow = AllocationBudget::new(encoded_output.len() - 1);
            assert!(matches!(PublicFrame::prepare(
                &session, &recipient, &dealer, &edge, &acceptance, &narrow,
            ), Err(SessionGraphError::Admission(AllocationRefusal::ExceedsLimit { requested_bytes, limit_bytes }))
                if requested_bytes == encoded_output.len() && limit_bytes == encoded_output.len() - 1));
            assert_eq!(narrow.reserved_bytes(), 0);
            let occupied = budget.try_reserve_bytes(1).unwrap();
            assert!(matches!(prepare_frame(),
                Err(SessionGraphError::Admission(AllocationRefusal::Capacity {
                    requested_bytes, reserved_bytes: 1, limit_bytes, ..
                })) if requested_bytes == encoded_output.len() && limit_bytes == encoded_output.len()));
            assert_eq!(budget.reserved_bytes(), 1);
            drop(occupied);
            assert_eq!(budget.reserved_bytes(), 0);
            let (refused, actual) = crate::test_allocations::refuse_one_layout_during(
                std::alloc::Layout::array::<u8>(encoded_output.len()).unwrap(),
                prepare_frame,
            );
            assert!(actual, "the allocator refused this original output layout");
            assert!(matches!(refused, Err(SessionGraphError::Buffer(_))));
            assert_eq!(budget.reserved_bytes(), 0);
            let retry = prepare_frame().expect("same exact geometry after real allocator refusal");
            assert_eq!(retry.backing().1, encoded_output.len());
            drop(retry);
            assert_eq!(budget.reserved_bytes(), 0);
            drop(prepared);
            assert_eq!(source_budget.reserved_bytes(), 0);
        }
    }
}
