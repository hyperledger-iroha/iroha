//! Checked original aggregate production, durable retirement and small completed-head restore.

use super::*;
use iroha_crypto::threshold_bls::aggregate_checkpoint::{
    DkgAggregateCheckpointBindingV1, PreparedDkgAggregateCheckpointV1, RetainedDkgAggregateV1,
};

/// Original physical aggregate owner plus the exact immutable native-authenticated public source.
///
/// Neither owned scalar copies nor signing capability escape this owner. Its
/// original96-byte leaf and encrypted checkpoint retain their actual charge
/// while the single canonical credential encoder borrows the source.
pub struct GlobalBeaconAggregateOwnerV1 {
    public: super::super::ValidatedGlobalThresholdBeaconSessionV1,
    secret: RetainedDkgAggregateV1<BeaconPurpose>,
    checkpoint: PreparedDkgAggregateCheckpointV1<BeaconPurpose>,
    binding: DkgAggregateCheckpointBindingV1,
}
impl GlobalBeaconAggregateOwnerV1 {
    /// Borrow the exact original validated public graph without copying it.
    #[must_use]
    pub fn authenticated_session(&self) -> &super::super::ValidatedGlobalThresholdBeaconSessionV1 {
        &self.public
    }
    /// Exact original one-based signer seat.
    #[must_use]
    pub fn signer_index(&self) -> u16 {
        self.secret.seat_index()
    }
    /// Every original physical private/public backing retains this operation pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.public.belongs_to(budget)
            && self.secret.belongs_to(budget)
            && self.checkpoint.belongs_to(budget)
    }
    /// Original completed encrypted checkpoint, retained unchanged through export retry.
    #[must_use]
    pub fn encrypted_checkpoint(&self) -> &[u8] {
        self.checkpoint
            .encrypted_record()
            .expect("checked complete aggregate")
    }
    /// Exact surviving private backing; the shared public graph has an independent lifetime.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    #[must_use]
    pub fn original_backing_bytes(&self) -> usize {
        self.secret.original_backing_bytes() + self.checkpoint.original_backing_bytes()
    }

    /// Borrow the exact authenticated aggregate context, without granting new authority.
    #[must_use]
    pub fn binding(&self) -> &DkgAggregateCheckpointBindingV1 {
        &self.binding
    }
    /// Borrow actual96 backing into the one prepared canonical credential encoder.
    #[must_use]
    pub fn credential_source(
        &self,
    ) -> super::super::credential::GlobalBeaconCredentialSourceV1<'_> {
        super::super::credential::GlobalBeaconCredentialSourceV1::new(
            &self.public,
            self.signer_index(),
            self.secret.components_for_runtime_custody(),
        )
    }
    /// Stream the original fixed raw pending-share bytes into caller-owned private custody.
    ///
    /// The daemon prepares the destination before extraction and enforces its
    /// exact length, privacy, descriptor identity and durability. This borrows
    /// three original slices; it creates no owned copy, RNG or signing proof.
    ///
    /// # Errors
    /// Retains the original writer refusal; this owner survives any partial write.
    pub fn write_pending_share_for_runtime_custody<W: std::io::Write + ?Sized>(
        &self,
        writer: &mut W,
    ) -> std::io::Result<()> {
        for component in self.secret.components_for_runtime_custody() {
            writer.write_all(component)?;
        }
        Ok(())
    }
}

/// Small aggregate-only graph prepared for one complete original encrypted head.
///
/// Completed-head restoration never constructs generation/delivery/acceptance
/// private banks, hybrid keys, polynomials or contribution buffers. Earlier
/// intent/head/source ancestry must be authenticated before this owner is used.
pub struct PreparedGlobalBeaconAggregateRestoreV1 {
    public: super::super::ValidatedGlobalThresholdBeaconSessionV1,
    checkpoint: PreparedDkgAggregateCheckpointV1<BeaconPurpose>,
    budget: AllocationBudget,
}
impl PreparedGlobalBeaconAggregateRestoreV1 {
    /// Exact aggregate ciphertext extent without preparing any private owner.
    ///
    /// # Errors
    /// Preserves original canonical geometry and checked layout overflow.
    pub fn aggregate_checkpoint_bytes() -> Result<usize, LocalGlobalThresholdBeaconDkgErrorV1> {
        Ok(PreparedDkgAggregateCheckpointV1::<BeaconPurpose>::allocation_layouts()?[0].size())
    }

    /// Exact earlier phase checkpoint byte bound without constructing its private owner.
    ///
    /// # Errors
    /// Retains original fixed canonical length and overflow causes.
    pub fn phase_checkpoint_bytes() -> Result<usize, LocalGlobalThresholdBeaconDkgErrorV1> {
        Ok(iroha_crypto::threshold_bls::checkpoint::PreparedDkgSecretsCheckpointV1::<BeaconPurpose>::encrypted_record_bound()?)
    }

    /// Prepare exact public source bounds and the final verifier without a private producer.
    ///
    /// Four original paid public row shapes feed the very same producer geometry
    /// kernel; they are never signed, published or treated as authority. No
    /// hybrid key, dealer polynomial, contributions or phase1..3 private bank is
    /// constructed. The surrounding daemon separately prepays canonical public
    /// input banks and raw source/proof extents before private restoration.
    ///
    /// # Errors
    /// Preserves authenticated geometry, original physical or verifier refusal.
    pub fn prepare_public_source(
        session: GlobalThresholdBeaconDkgSessionV1,
        roster: &[PeerId],
        seat: u16,
        lifecycle: &KeyPair,
        budget: &AllocationBudget,
    ) -> Result<
        (
            [usize; 3],
            super::super::PreparedGlobalThresholdBeaconSessionVerificationV1,
        ),
        LocalGlobalThresholdBeaconDkgErrorV1,
    > {
        super::super::validate_dkg_session(&session)?;
        if usize::from(session.committee_size) != roster.len()
            || seat == 0
            || seat > session.committee_size
            || roster[usize::from(seat - 1)].public_key() != lifecycle.public_key()
            || lifecycle.public_key().algorithm() != iroha_crypto::Algorithm::BlsNormal
            || global_threshold_beacon_roster_hash_v1(roster) != session.roster_hash
        {
            return Err(GlobalThresholdBeaconError::InvalidDkgSession.into());
        }
        let recipient = PendingRow::recipient(seat, lifecycle.public_key(), budget)?;
        let dealer = PendingRow::dealer(seat, session.threshold, budget)?;
        let edge = PendingRow::edge(seat, 1, budget)?;
        let acceptance = PendingRow::acceptance(1, seat, budget)?;
        let max_message = DkgSignaturePreimage::RecipientKey(&session, recipient.record())
            .encoded_len()
            .max(DkgSignaturePreimage::DealerCommitment(&session, dealer.record()).encoded_len())
            .max(DkgSignaturePreimage::EncryptedShare(&session, edge.record()).encoded_len())
            .max(
                DkgSignaturePreimage::ShareAcceptance(&session, acceptance.record()).encoded_len(),
            );
        let (_, bounds) = PublicFrame::measure(
            &session,
            recipient.record(),
            dealer.record(),
            edge.record(),
            acceptance.record(),
        )?;
        drop(recipient);
        drop(dealer);
        drop(edge);
        drop(acceptance);
        let verifier = super::super::PreparedGlobalThresholdBeaconSessionVerificationV1::new(
            session,
            max_message,
            budget,
        )?;
        Ok((bounds, verifier))
    }

    /// Admit only the exact original aggregate graph before any private file read/adoption.
    ///
    /// # Errors
    /// Refuses a foreign public pool, invalid seat or exact original physical cause.
    pub fn new(
        public: &super::super::ValidatedGlobalThresholdBeaconSessionV1,
        seat: u16,
        budget: &AllocationBudget,
    ) -> Result<Self, LocalGlobalThresholdBeaconDkgErrorV1> {
        if !public.belongs_to(budget) {
            return Err(GlobalThresholdBeaconSessionError::ForeignReservation.into());
        }
        let parameters = adaptive_beacon_parameters(&public.record().adaptive_dkg.session)?;
        let checkpoint = PreparedDkgAggregateCheckpointV1::new(&parameters, seat, budget)?;
        Ok(Self {
            public: public.clone(),
            checkpoint,
            budget: budget.clone(),
        })
    }
    /// Exact original encrypted-source extent admitted before reading private bytes.
    #[must_use]
    pub fn encrypted_checkpoint_bytes(&self) -> usize {
        self.checkpoint.encrypted_record_capacity()
    }
    /// Verify every actual surviving backing retains its original operation pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.public.belongs_to(budget)
            && self.checkpoint.belongs_to(budget)
            && self.budget.same_pool(budget)
    }
    /// Restore one complete original aggregate without regeneration or crypto production.
    ///
    /// Original context/source and exact decoder refusal prefixes remain with
    /// the returned preparer on every failure. Successful restore moves the
    /// actual96 leaf once and keeps its immutable ciphertext for export barriers.
    ///
    /// # Errors
    /// Returns this original owner with exact native-context, AEAD, canonical or scalar causes.
    pub fn restore(
        mut self,
        context: &VerifiedGlobalBeaconDkgAggregateContextV1,
        encrypted: &[u8],
        lifecycle: &KeyPair,
        limits: norito::DecodeLimits,
    ) -> Result<GlobalBeaconAggregateOwnerV1, (Self, LocalGlobalThresholdBeaconDkgErrorV1)> {
        let result = (|| {
            context.matches_public(&self.public, lifecycle)?;
            if !self.belongs_to(&self.budget) {
                return Err(GlobalThresholdBeaconSessionError::ForeignReservation.into());
            }
            self.checkpoint.restore(
                encrypted,
                context.binding(),
                lifecycle,
                self.public.transcript(),
                limits,
            )?;
            let secret = self.checkpoint.take_original_aggregate()?;
            Ok(secret)
        })();
        match result {
            Ok(secret) => Ok(GlobalBeaconAggregateOwnerV1 {
                public: self.public,
                secret,
                checkpoint: self.checkpoint,
                binding: *context.binding(),
            }),
            Err(error) => Err((self, error)),
        }
    }
}

impl LocalGlobalThresholdBeaconDkgSeatV1 {
    fn check_aggregate_public_source(
        &self,
        validated: &super::super::ValidatedGlobalThresholdBeaconSessionV1,
    ) -> Result<(), LocalGlobalThresholdBeaconDkgErrorV1> {
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
        Ok(())
    }
    /// Exact side-effect-free local algorithm shared by production and logical-clock fixtures.
    #[cfg(any(test, feature = "iroha-core-tests"))]
    pub(in crate::beacon) fn aggregate_private_share(
        &self,
        validated: &super::super::ValidatedGlobalThresholdBeaconSessionV1,
    ) -> Result<AdaptiveThresholdBlsSecretShare<BeaconPurpose>, LocalGlobalThresholdBeaconDkgErrorV1>
    {
        self.check_aggregate_public_source(validated)?;
        Ok(aggregate_original_private_contributions(
            validated.transcript(),
            self.outputs.shares.as_slice(),
        )?)
    }
    fn check_aggregate_context(
        &self,
        context: &VerifiedGlobalBeaconDkgAggregateContextV1,
        validated: &super::super::ValidatedGlobalThresholdBeaconSessionV1,
        lifecycle: &KeyPair,
    ) -> Result<(), LocalGlobalThresholdBeaconDkgErrorV1> {
        self.check_aggregate_public_source(validated)?;
        context.matches_public(validated, lifecycle)?;
        if context.session() != &self.session || context.binding().seat_index != self.seat_index {
            return Err(DkgCheckpointErrorV1::Binding.into());
        }
        let accepted = self.checkpoints[2]
            .encrypted_record()
            .ok_or(DkgCheckpointErrorV1::Terminal)?;
        if Hash::new(accepted).as_ref() != &context.binding().accepted_checkpoint_hash {
            return Err(DkgCheckpointErrorV1::Binding.into());
        }
        Ok(())
    }
    /// Aggregate once into the original prepaid bank after durable extraction intent.
    ///
    /// Every individual contribution/ack remains until the original aggregate
    /// checkpoint and head cross their exact file/directory durability barriers.
    /// A completed retry borrows identical ciphertext; no second sum or nonce.
    ///
    /// # Errors
    /// Preserves original source/pool/context, equation, encoding and entropy failures.
    pub fn produce_aggregate_checkpoint<'a>(
        &'a mut self,
        context: &VerifiedGlobalBeaconDkgAggregateContextV1,
        validated: &super::super::ValidatedGlobalThresholdBeaconSessionV1,
        lifecycle: &KeyPair,
    ) -> Result<&'a [u8], LocalGlobalThresholdBeaconDkgErrorV1> {
        self.check_aggregate_context(context, validated, lifecycle)?;
        let bank = self
            .aggregate_checkpoint
            .as_mut()
            .ok_or(DkgCheckpointErrorV1::Terminal)?;
        let bytes =
            bank.produce_original(context.binding(), lifecycle, validated.transcript(), || {
                aggregate_original_private_contributions(
                    validated.transcript(),
                    self.outputs.shares.as_slice(),
                )
            })?;
        #[cfg(all(test, sumeragi_core_mutation = "HC115"))]
        {
            // Deliberate mutation: plaintext contributions/acks disappear before
            // original aggregate checkpoint/head file and directory durability.
            self.outputs.shares.truncate(0);
            self.outputs.acceptances.truncate(0);
        }
        Ok(bytes)
    }
    /// Move the original aggregate only after its checkpoint/head durability barriers.
    ///
    /// The sole daemon publisher calls this after verifying/fsyncing original
    /// files and the pinned directory. Core verifies the complete retained
    /// context/ciphertext before moving any owner or retiring contributions; it
    /// cannot itself establish filesystem durability. No arithmetic/RNG repeats.
    ///
    /// # Errors
    /// A refusal retains the same local private owners and aggregate bank for retry.
    pub fn retire_durably_published_aggregate(
        &mut self,
        context: &VerifiedGlobalBeaconDkgAggregateContextV1,
        validated: &super::super::ValidatedGlobalThresholdBeaconSessionV1,
        lifecycle: &KeyPair,
    ) -> Result<GlobalBeaconAggregateOwnerV1, LocalGlobalThresholdBeaconDkgErrorV1> {
        self.check_aggregate_context(context, validated, lifecycle)?;
        let bank = self
            .aggregate_checkpoint
            .as_ref()
            .ok_or(DkgCheckpointErrorV1::Terminal)?;
        bank.encrypted_record_for(context.binding(), lifecycle, validated.transcript())?;
        if !bank.belongs_to(&self.budget) {
            return Err(GlobalThresholdBeaconSessionError::ForeignReservation.into());
        }
        let mut checkpoint = self
            .aggregate_checkpoint
            .take()
            .expect("checked original aggregate bank");
        let secret = match checkpoint.take_original_aggregate() {
            Ok(secret) => secret,
            Err(error) => {
                self.aggregate_checkpoint = Some(checkpoint);
                return Err(error.into());
            }
        };
        self.outputs.shares.truncate(0);
        self.outputs.acceptances.truncate(0);
        self.extracted = true;
        Ok(GlobalBeaconAggregateOwnerV1 {
            public: validated.clone(),
            secret,
            checkpoint,
            binding: *context.binding(),
        })
    }
}

#[cfg(test)]
mod tests;

// Sole side-effect-free aggregation kernel, used by the production callback and
// logical-clock fixture arithmetic. Neither call grants native or durable authority.
fn aggregate_original_private_contributions(
    transcript: &iroha_crypto::threshold_bls::AdaptiveThresholdBlsPublicTranscript<BeaconPurpose>,
    shares: &[DasRenPrivateShare<BeaconPurpose>],
) -> Result<
    AdaptiveThresholdBlsSecretShare<BeaconPurpose>,
    iroha_crypto::threshold_bls::ThresholdBlsError,
> {
    AdaptiveThresholdBlsSecretShare::from_dealer_shares(transcript, shares)
}
