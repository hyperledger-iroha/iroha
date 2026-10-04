//! Original sequential checkpoint custody restored into the graph prepared before its claim.

use super::*;

impl PreparedLocalGlobalThresholdBeaconDkgSeatV1 {
    /// Restore original generation owners and signatures from one checked phase-one head.
    ///
    /// The caller retains the original funded canonical publication decoder and
    /// encrypted source. This method authenticates their exact original bytes,
    /// both contextual BLS signatures and the dealer Schnorr proof before any
    /// public row is copied. No RNG, signing, reproof, output admission or owned
    /// decoder is invoked. The first checkpoint bank keeps its exact ciphertext
    /// for the next phase's authenticated previous-checkpoint relation.
    ///
    /// Later phases require sequential restoration of their original inputs and
    /// heads. This method cannot skip a phase or certify activation.
    ///
    /// # Errors
    /// Returns the unchanged prepared owner on source, proof, context or original
    /// decoder refusal. Its same bank and source binding survive retry.
    pub fn restore_generated(
        mut self,
        context: &VerifiedGlobalBeaconDkgCheckpointContextV1,
        publication: &GlobalThresholdBeaconDkgSnapshotV1,
        canonical_publication: &[u8],
        encrypted: &[u8],
        signer: &KeyPair,
        limits: norito::DecodeLimits,
    ) -> Result<LocalGlobalThresholdBeaconDkgSeatV1, (Self, LocalGlobalThresholdBeaconDkgErrorV1)>
    {
        let checked = (|| {
            let binding = context.binding();
            if context.session() != self.session
                || binding.phase != 1
                || binding.seat_index != self.seat_index
                || binding.phase_input_hash != [0; 32]
                || binding.previous_checkpoint_hash != [0; 32]
                || signer.public_key() != self.recipient.record().validator.public_key()
                || publication.session != self.session
                || publication.last_updated_height != self.session.start_height
                || publication.recipient_keys.len() != 1
                || publication.dealer_commitments.len() != 1
                || publication.recipient_keys[0].recipient_index != self.seat_index
                || publication.dealer_commitments[0].dealer_index != self.seat_index
                || !publication.encrypted_shares.is_empty()
                || !publication.share_acceptances.is_empty()
                || canonical_publication.is_empty()
                || binding.public_output_hash != <[u8; 32]>::from(Hash::new(canonical_publication))
            {
                return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
            }
            DkgSnapshotRef::from(publication).validate_with_verifier(&mut self.workspace)?;
            norito::verify_exact_canonical_frame(publication, canonical_publication)
                .map_err(SessionGraphError::from)?;
            let recipient = &publication.recipient_keys[0];
            let dealer = &publication.dealer_commitments[0];
            self.recipient.validate_restoration(recipient)?;
            self.dealer.validate_restoration(dealer)?;
            let parameters = adaptive_beacon_parameters(&self.session)?;
            let original_dealer = super::super::verify_adaptive_dealer(&parameters, dealer)?;
            let original_recipient = HybridPublicKey::from_bytes(
                recipient.x25519_public_key,
                &recipient.mlkem768_public_key,
            )?;
            let bank = &mut self.checkpoints[0];
            if !bank.belongs_to(&self.budget) {
                return Err(GlobalThresholdBeaconSessionError::ForeignReservation.into());
            }
            bank.restore(
                encrypted,
                binding,
                signer,
                &original_recipient,
                std::slice::from_ref(&original_dealer),
                limits,
            )?;
            // The successful original private equations precede every public copy.
            let owners = bank.take_restored_generation_owners()?;
            Ok::<_, LocalGlobalThresholdBeaconDkgErrorV1>(owners)
        })();
        let (encryption, dealer_secret) = match checked {
            Ok(owners) => owners,
            Err(error) => return Err((self, error)),
        };
        self.recipient
            .restore_publication(&publication.recipient_keys[0]);
        self.dealer
            .restore_publication(&publication.dealer_commitments[0]);
        let mut local = LocalGlobalThresholdBeaconDkgSeatV1 {
            session: self.session,
            seat_index: self.seat_index,
            recipient_key: self.recipient.finish_restored(),
            dealer_commitment: self.dealer.finish_restored(),
            encryption,
            dealer_secret: Some(dealer_secret),
            outputs: self.outputs,
            workspace: self.workspace,
            public_frame: self.public_frame,
            checkpoints: self.checkpoints,
            budget: self.budget,
            delivered: false,
            accepted: false,
            extracted: false,
            aborted: false,
            delivery_input: None,
            acceptance_input: None,
        };
        // The exact original canonical output goes into the already funded frame.
        // Every field was validated above and its shape equals the pre-claim plan.
        let output = local
            .publication_frame()
            .unwrap_or_else(|error| panic!("checked original publication frame: {error}"));
        assert!(
            output == canonical_publication,
            "checked original canonical publication changed"
        );
        Ok(local)
    }
}

impl LocalGlobalThresholdBeaconDkgSeatV1 {
    fn original_restore_context(
        &self,
        context: &VerifiedGlobalBeaconDkgCheckpointContextV1,
        phase: u16,
        publication: &GlobalThresholdBeaconDkgSnapshotV1,
        canonical: &[u8],
        signer: &KeyPair,
    ) -> Result<(), LocalGlobalThresholdBeaconDkgErrorV1> {
        let binding = context.binding();
        let previous = self.checkpoints[usize::from(phase - 2)]
            .encrypted_record()
            .ok_or(GlobalThresholdBeaconError::DkgTerminal)?;
        if context.session() != self.session
            || binding.phase != phase
            || binding.seat_index != self.seat_index
            || self.aborted
            || self.extracted
            || publication.session != self.session
            || canonical.is_empty()
            || binding.public_output_hash != <[u8; 32]>::from(Hash::new(canonical))
            || binding.previous_checkpoint_hash != <[u8; 32]>::from(Hash::new(previous))
            || publication.last_updated_height != binding.source.height()
            || signer.public_key() != self.recipient_key.get().validator.public_key()
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
        }
        Ok(())
    }

    /// Restore original dealer deliveries after the authenticated generation head.
    ///
    /// Every complete input, original signed edge, canonical output and chained
    /// private equation is verified before consuming any pending output row.
    /// The original polynomial remains live until the daemon calls
    /// [`Self::retire_durably_published_dealer`] after the original durability barrier.
    /// No capsule, proof, signature, entropy or replacement allocation is produced.
    ///
    /// # Errors
    /// Returns this same owner on source/context/precondition/proof/decode refusal.
    pub fn restore_delivered(
        mut self,
        context: &VerifiedGlobalBeaconDkgCheckpointContextV1,
        commitments: &GlobalThresholdBeaconDkgSnapshotV1,
        publication: &GlobalThresholdBeaconDkgSnapshotV1,
        canonical_publication: &[u8],
        encrypted: &[u8],
        signer: &KeyPair,
        limits: norito::DecodeLimits,
    ) -> Result<Self, (Self, LocalGlobalThresholdBeaconDkgErrorV1)> {
        let checked = (|| {
            self.original_restore_context(context, 2, publication, canonical_publication, signer)?;
            let n = usize::from(self.session.committee_size);
            if self.delivered
                || self.accepted
                || self.dealer_secret.is_none()
                || !self.outputs.outgoing.as_slice().is_empty()
                || !self.outputs.acceptances.as_slice().is_empty()
                || commitments.session != self.session
                || commitments.last_updated_height != self.session.start_height
                || commitments.recipient_keys.len() != n
                || commitments.dealer_commitments.len() != n
                || !commitments.encrypted_shares.is_empty()
                || !commitments.share_acceptances.is_empty()
                || commitments.recipient_keys[usize::from(self.seat_index - 1)]
                    != *self.recipient_key.get()
                || commitments.dealer_commitments[usize::from(self.seat_index - 1)]
                    != *self.dealer_commitment.get()
                || publication.recipient_keys != commitments.recipient_keys
                || publication.dealer_commitments != commitments.dealer_commitments
                || publication.generator_h != commitments.generator_h
                || publication.generator_v != commitments.generator_v
                || publication.encrypted_shares.len() != n
                || !publication.share_acceptances.is_empty()
                || context.binding().phase_input_hash
                    != self.checkpoint_input_hash(2, commitments)?
            {
                return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
            }
            DkgSnapshotRef::from(commitments).validate_with_verifier(&mut self.workspace)?;
            DkgSnapshotRef::from(publication).validate_with_verifier(&mut self.workspace)?;
            norito::verify_exact_canonical_frame(publication, canonical_publication)
                .map_err(SessionGraphError::from)?;
            for (offset, edge) in publication.encrypted_shares.iter().enumerate() {
                if edge.dealer_index != self.seat_index
                    || usize::from(edge.recipient_index) != offset + 1
                    || edge.delivery_height != publication.last_updated_height
                {
                    return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
                }
                self.outputs.pending_edges.as_slice()[offset]
                    .as_ref()
                    .ok_or(GlobalThresholdBeaconError::DkgTerminal)?
                    .validate_restoration(edge)?;
            }
            let parameters = adaptive_beacon_parameters(&self.session)?;
            let mut dealers = arrayvec::ArrayVec::<_, 31>::new();
            for dealer in &commitments.dealer_commitments {
                dealers.push(super::super::verify_adaptive_dealer(&parameters, dealer)?);
            }
            let recipient = self.encryption.public();
            let bank = &mut self.checkpoints[1];
            if !bank.belongs_to(&self.budget) {
                return Err(GlobalThresholdBeaconSessionError::ForeignReservation.into());
            }
            bank.restore(
                encrypted,
                context.binding(),
                signer,
                recipient,
                &dealers,
                limits,
            )?;
            Ok::<_, LocalGlobalThresholdBeaconDkgErrorV1>(bank.take_restored_delivery_owners()?)
        })();
        let (recipient, polynomial) = match checked {
            Ok(owners) => owners,
            Err(error) => return Err((self, error)),
        };
        self.encryption = recipient;
        self.dealer_secret = Some(polynomial);
        for (offset, source) in publication.encrypted_shares.iter().enumerate() {
            let mut pending = self.outputs.pending_edges.as_mut_slice()[offset]
                .take()
                .expect("checked original pending edge");
            pending.restore_publication(source);
            self.outputs
                .outgoing
                .push_reserved(pending.finish_restored());
        }
        self.delivery_input = Some(context.binding().phase_input_hash);
        self.delivered = true;
        #[cfg(all(test, sumeragi_core_mutation = "HC112"))]
        drop(self.dealer_secret.take());
        let output = self
            .delivery_frame(commitments)
            .unwrap_or_else(|error| panic!("checked original delivery frame: {error}"));
        assert!(
            output == canonical_publication,
            "checked original canonical delivery changed"
        );
        Ok(self)
    }

    /// Restore original accepted shares and acknowledgments after durable dealer retirement.
    ///
    /// All `n²` signed capsules and the exact local acknowledgments are checked.
    /// Each checkpoint contribution must also equal the plaintext of its original
    /// authenticated capsule before any copy. The complete original share backing
    /// moves by same-pool empty-buffer exchange; all ciphertext heads remain live.
    /// No signing, RNG, reproof or late output admission occurs.
    ///
    /// # Errors
    /// Returns the unchanged enclosing owner on original source, decoder or custody refusal.
    pub fn restore_accepted(
        mut self,
        context: &VerifiedGlobalBeaconDkgCheckpointContextV1,
        deliveries: &GlobalThresholdBeaconDkgSnapshotV1,
        publication: &GlobalThresholdBeaconDkgSnapshotV1,
        canonical_publication: &[u8],
        encrypted: &[u8],
        signer: &KeyPair,
        limits: norito::DecodeLimits,
    ) -> Result<Self, (Self, LocalGlobalThresholdBeaconDkgErrorV1)> {
        let checked = (|| {
            self.original_restore_context(context, 3, publication, canonical_publication, signer)?;
            let n = usize::from(self.session.committee_size);
            if !self.delivered
                || self.accepted
                || self.dealer_secret.is_some()
                || !self.outputs.shares.as_slice().is_empty()
                || self.outputs.shares.capacity() != n
                || !self.outputs.shares.belongs_to(&self.budget)
                || !self.outputs.acceptances.as_slice().is_empty()
                || deliveries.session != self.session
                || deliveries.last_updated_height != self.session.commitments_end_height
                || deliveries.recipient_keys.len() != n
                || deliveries.dealer_commitments.len() != n
                || deliveries.encrypted_shares.len() != n * n
                || !deliveries.share_acceptances.is_empty()
                || deliveries.recipient_keys[usize::from(self.seat_index - 1)]
                    != *self.recipient_key.get()
                || deliveries.dealer_commitments[usize::from(self.seat_index - 1)]
                    != *self.dealer_commitment.get()
                || deliveries
                    .encrypted_shares
                    .iter()
                    .filter(|edge| edge.dealer_index == self.seat_index)
                    .ne(self
                        .outputs
                        .outgoing
                        .as_slice()
                        .iter()
                        .map(RetainedPayload::get))
                || publication.recipient_keys != deliveries.recipient_keys
                || publication.dealer_commitments != deliveries.dealer_commitments
                || publication.generator_h != deliveries.generator_h
                || publication.generator_v != deliveries.generator_v
                || publication.encrypted_shares != deliveries.encrypted_shares
                || publication.share_acceptances.len() != n
                || context.binding().phase_input_hash
                    != self.checkpoint_input_hash(3, deliveries)?
            {
                return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
            }
            DkgSnapshotRef::from(deliveries).validate_with_verifier(&mut self.workspace)?;
            DkgSnapshotRef::from(publication).validate_with_verifier(&mut self.workspace)?;
            norito::verify_exact_canonical_frame(publication, canonical_publication)
                .map_err(SessionGraphError::from)?;
            for (offset, row) in publication.share_acceptances.iter().enumerate() {
                if row.recipient_index != self.seat_index
                    || usize::from(row.dealer_index) != offset + 1
                    || row.accepted_height != publication.last_updated_height
                {
                    return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
                }
                self.outputs.pending_acceptances.as_slice()[offset]
                    .as_ref()
                    .ok_or(GlobalThresholdBeaconError::DkgTerminal)?
                    .validate_restoration(row)?;
            }
            let parameters = adaptive_beacon_parameters(&self.session)?;
            let mut dealers = arrayvec::ArrayVec::<_, 31>::new();
            for dealer in &deliveries.dealer_commitments {
                dealers.push(super::super::verify_adaptive_dealer(&parameters, dealer)?);
            }
            let bank = &mut self.checkpoints[2];
            if !bank.belongs_to(&self.budget) {
                return Err(GlobalThresholdBeaconSessionError::ForeignReservation.into());
            }
            bank.restore(
                encrypted,
                context.binding(),
                signer,
                self.encryption.public(),
                &dealers,
                limits,
            )?;
            #[cfg(not(all(test, sumeragi_core_mutation = "HC113")))]
            for (offset, dealer) in dealers.iter().enumerate() {
                let edge =
                    &deliveries.encrypted_shares[offset * n + usize::from(self.seat_index - 1)];
                let kem = HybridKemCiphertext::from_parts(
                    edge.ephemeral_x25519_public_key,
                    &edge.mlkem768_ciphertext,
                )?;
                let aad = self
                    .workspace
                    .write(DkgSignaturePreimage::PrivateEdgeAad(&self.session, edge))?;
                let original = open_das_ren_private_share(
                    &parameters,
                    dealer,
                    self.seat_index,
                    self.encryption.secret(),
                    &kem,
                    &edge.encrypted_share,
                    aad,
                )?;
                bank.verify_restored_accepted_contribution(offset, &original)?;
            }
            Ok::<_, LocalGlobalThresholdBeaconDkgErrorV1>(
                bank.take_restored_accepted_owners(&mut self.outputs.shares, &self.budget)?,
            )
        })();
        self.encryption = match checked {
            Ok(recipient) => recipient,
            Err(error) => return Err((self, error)),
        };
        for (offset, source) in publication.share_acceptances.iter().enumerate() {
            let mut pending = self.outputs.pending_acceptances.as_mut_slice()[offset]
                .take()
                .expect("checked original pending acceptance");
            pending.restore_publication(source);
            self.outputs
                .acceptances
                .push_reserved(pending.finish_restored());
        }
        self.acceptance_input = Some(context.binding().phase_input_hash);
        self.accepted = true;
        let output = self
            .acceptance_frame(deliveries)
            .unwrap_or_else(|error| panic!("checked original acceptance frame: {error}"));
        assert!(
            output == canonical_publication,
            "checked original canonical acceptance changed"
        );
        Ok(self)
    }
}

#[cfg(test)]
mod tests;
