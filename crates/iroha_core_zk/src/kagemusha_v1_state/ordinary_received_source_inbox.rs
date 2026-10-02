//! Durable complete received sources under actual Main request-key, FI and proof custody.
//! Intake and cold readmission authenticate sources only; incoming State requires its own CAS.
use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaOrdinaryCashOutgoingOriginalV1, KagemushaVerifiedOrdinaryReceivedCashOutputV1,
    readmit_historical_ordinary_received_cash_output_v1, verify_ordinary_received_cash_output_v1,
};

/// Data retained in Main before an incoming reservation/proof can select this source.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryReceivedSourceOriginalsV1")]
pub(super) struct ReceivedSourceOriginals {
    request_id: DigestV1,
    operation_id: DigestV1,
    credit_id: DigestV1,
    amount: u128,
    receiver_request_original_sha256: DigestV1,
    outgoing_original: Vec<u8>,
    received_assertion_original: Vec<u8>,
    financial_control: CapturedFinancialControlIdentity,
    intake_clock: KagemushaOrdinaryCashClockContextV1,
}

/// Owned genuine mathematical admission retained alongside its exact complete source originals.
/// No decoder, serialized marker or journal checksum constructs the mathematical capability.
pub(super) struct ReceivedSourceAdmission {
    originals: ReceivedSourceOriginals,
    admitted: KagemushaVerifiedOrdinaryReceivedCashOutputV1,
}

/// Proof-only borrow of complete source evidence and the actual Main-owned request key.
/// Constructing it requires current FI custody. Its historical source getters do not create
/// an incoming State, financial approval, replay insertion or global credit-consumption grant.
pub(crate) struct KagemushaAuthenticatedOrdinaryReceivedSourceCustodyV1<'owner> {
    owner: &'owner KagemushaNativeOrdinaryCashOwnerV1,
    request_id: DigestV1,
    prefix: KagemushaRecoveryJournalPrefixV1,
}

impl KagemushaAuthenticatedOrdinaryReceivedSourceCustodyV1<'_> {
    fn source(&self) -> Result<&ReceivedSourceAdmission, KagemushaStateErrorV1> {
        self.owner
            .received_sources
            .get(&self.request_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }

    /// Recheck held complete source/proof/request/C/FI/clock originals and the same Main prefix.
    /// This is historical proof custody; monetary effects require independently fresh authority.
    pub(crate) fn recheck_source_custody(&self) -> Result<(), KagemushaStateErrorV1> {
        if self.owner.recovery_failed
            || self.owner.recovery_catalog.is_some()
            || self.owner.prefix != self.prefix
            || self.owner.journal.recovery_prefix().map_err(storage)? != self.prefix
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.owner.journal.check_owned().map_err(storage)?;
        self.owner.publication.recheck_historical_cash_custody()?;
        self.owner.recheck_lineage_retained_custody()?;
        let source = self.source()?;
        if source.originals.request_id != self.request_id
            || !self
                .owner
                .used_operations
                .contains(&source.originals.operation_id)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        source
            .originals
            .require_admitted(self.owner, &source.admitted)?;
        receiver_request::loan_main_request_historical(self.owner, self.request_id)?
            .recheck_historical_custody()?;
        self.owner.journal.check_owned().map_err(storage)
    }

    /// Actual owned closed Wrapper admission, independently reconstructed on cold Main replay.
    pub(crate) fn received_source(
        &self,
    ) -> Result<&KagemushaVerifiedOrdinaryReceivedCashOutputV1, KagemushaStateErrorV1> {
        self.recheck_source_custody()?;
        Ok(&self.source()?.admitted)
    }

    /// Full immutable receipt envelope, including signature, exact DATA and genuine finality.
    pub(crate) fn received_assertion_original(&self) -> Result<&[u8], KagemushaStateErrorV1> {
        self.recheck_source_custody()?;
        Ok(&self.source()?.originals.received_assertion_original)
    }

    /// Native operation identity already retained before any incoming head selection.
    pub(crate) fn operation_id(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.recheck_source_custody()?;
        Ok(self.source()?.originals.operation_id)
    }

    /// Lend the real AEAD opening only inside a higher-ranked proof-witness construction call.
    /// The key never leaves Main. This source opening is not an incoming financial approval.
    pub(crate) fn with_borrowed_received_credit_opening(
        &self,
        visitor: &mut dyn for<'secret> FnMut(
            &'secret KagemushaCreditOpeningV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_source_custody()?;
        let source = self.source()?;
        let financial = self.owner.publication.cash_financial();
        let identity = source.originals.financial_control;
        let captured = self
            .owner
            .control
            .borrow_captured_proof_decision(
                financial,
                identity.original_sha256,
                identity.lower_ms,
                identity.upper_ms,
            )
            .map_err(material)?;
        let assertion = self
            .owner
            .lineage_cas
            .readmit_received_commit_assertion_for_captured_control(
                financial,
                &captured,
                &source.originals.received_assertion_original,
            )
            .map_err(material)?;
        let request = receiver_request::loan_main_request_historical(self.owner, self.request_id)?;
        let opening = request.open_received(&source.admitted, &assertion)?;
        opening.with_borrowed_credit_opening(visitor)?;
        self.recheck_source_custody()
    }
}

impl ReceivedSourceOriginals {
    fn encoded(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        let raw = norito::encode_canonical(self).map_err(material)?;
        if raw.is_empty() || raw.len() as u64 > FORMAT.maximum_payload_bytes {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(raw)
    }

    fn require_admitted(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        admitted: &KagemushaVerifiedOrdinaryReceivedCashOutputV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let request = owner
            .retained_receiver_requests
            .get(&self.request_id)
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if self.operation_id == [0; 32]
            || self.credit_id == [0; 32]
            || self.amount == 0
            || self.outgoing_original.as_slice() != admitted.outgoing_original()
            || self.received_assertion_original.is_empty()
            || self.credit_id != admitted.credit_id()
            || self.amount != admitted.amount()
            || admitted.request_original() != request.captured.original()
            || self.receiver_request_original_sha256
                != <DigestV1>::from(Sha256::digest(admitted.request_original()))
            || admitted.received_assertion_original_sha256()
                != <DigestV1>::from(Sha256::digest(&self.received_assertion_original))
            || admitted.receiver_credential_digest()
                != owner
                    .publication
                    .cash_financial()
                    .enrollment()
                    .app_credential()
                    .digest()
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let captured = owner
            .control
            .borrow_captured_proof_decision(
                owner.publication.cash_financial(),
                self.financial_control.original_sha256,
                self.financial_control.lower_ms,
                self.financial_control.upper_ms,
            )
            .map_err(material)?;
        captured
            .recheck_financial_owner(owner.publication.cash_financial())
            .map_err(material)?;
        owner
            .publication
            .cash_financial()
            .verified_retained_cash_clock_originals(&self.intake_clock)
            .map_err(material)?;
        if self.intake_clock.lower_at_ms < captured.captured_lower_ms()
            || self.intake_clock.upper_at_ms < captured.captured_upper_ms()
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.require_complete_source_budget(owner)
    }

    fn require_complete_source_budget(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        // The maintained compact carrier already accounts for the full Wrapper, C, signed
        // clock and complete DATA receipt/finality. This WAL adds only bounded fixed selectors.
        let intake_clock_bytes = norito::encode_canonical(&self.intake_clock).map_err(material)?;
        let maximum = u64::from(owner.carrier_budget.compact_receiver_max_bytes())
            .checked_add(u64::try_from(intake_clock_bytes.len()).map_err(material)?)
            .and_then(|v| v.checked_add(4096))
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        if self.encoded()?.len() as u64 > maximum {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        Ok(())
    }

    fn readmit(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
    ) -> Result<KagemushaVerifiedOrdinaryReceivedCashOutputV1, KagemushaStateErrorV1> {
        let financial = owner.publication.cash_financial();
        let captured = owner
            .control
            .borrow_captured_proof_decision(
                financial,
                self.financial_control.original_sha256,
                self.financial_control.lower_ms,
                self.financial_control.upper_ms,
            )
            .map_err(material)?;
        let assertion = owner
            .lineage_cas
            .readmit_received_commit_assertion_for_captured_control(
                financial,
                &captured,
                &self.received_assertion_original,
            )
            .map_err(material)?;
        let outgoing =
            KagemushaOrdinaryCashOutgoingOriginalV1::decode_original(&self.outgoing_original)
                .map_err(material)?;
        let clock = owner
            .lineage_cas
            .authenticate_received_clock_original(
                financial,
                outgoing.admission_clock_signed_original(),
            )
            .map_err(material)?;
        let request = receiver_request::loan_main_request_historical(owner, self.request_id)?;
        let admitted = readmit_historical_ordinary_received_cash_output_v1(
            &owner.verifier,
            &assertion,
            &request,
            &self.outgoing_original,
            &clock,
        )
        .map_err(material)?;
        // Actual AEAD and all same-key/opening commitments are independently checked again.
        request
            .open_received(&admitted, &assertion)?
            .recheck_historical_custody()?;
        self.require_admitted(owner, &admitted)?;
        Ok(admitted)
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    pub(super) fn retained_incoming_received_source(
        &self,
        request_id: DigestV1,
    ) -> Result<(DigestV1, &KagemushaVerifiedOrdinaryReceivedCashOutputV1), KagemushaStateErrorV1>
    {
        self.publication.recheck_historical_cash_custody()?;
        self.journal.check_owned().map_err(storage)?;
        let source = self
            .received_sources
            .get(&request_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        source.originals.require_admitted(self, &source.admitted)?;
        if source.originals.request_id != request_id
            || !self
                .used_operations
                .contains(&source.originals.operation_id)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok((source.originals.operation_id, &source.admitted))
    }
    /// Borrow actual retained source evidence under the same current Native financial holder.
    /// The borrower cannot replace the receipt/proof/private request key or alter State.
    pub(crate) fn received_source_custody(
        &self,
        request_id: DigestV1,
    ) -> Result<KagemushaAuthenticatedOrdinaryReceivedSourceCustodyV1<'_>, KagemushaStateErrorV1>
    {
        self.require_current_financial_control()?;
        let loan = KagemushaAuthenticatedOrdinaryReceivedSourceCustodyV1 {
            owner: self,
            request_id,
            prefix: self.prefix,
        };
        loan.recheck_source_custody()?;
        self.require_current_financial_control()?;
        Ok(loan)
    }

    /// Retain a full immutable source after real Wrapper, Node receipt, clock and AEAD admission.
    /// Exact retry reuses the original operation. This neither consumes a key nor changes funds.
    pub(crate) fn retain_received_source(
        &mut self,
        request_id: DigestV1,
        outgoing_original: &[u8],
        received_assertion_original: &[u8],
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if let Some(existing) = self.received_sources.get(&request_id) {
            if existing.originals.outgoing_original != outgoing_original
                || existing.originals.received_assertion_original != received_assertion_original
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            existing
                .originals
                .require_admitted(self, &existing.admitted)?;
            self.require_current_financial_control()?;
            return Ok(existing.originals.operation_id);
        }
        self.require_received_source_idle()?;
        let captured = self
            .control
            .capture_proof_decision(self.publication.cash_financial())
            .map_err(material)?;
        let identity = CapturedFinancialControlIdentity {
            original_sha256: captured.original_sha256().map_err(material)?,
            lower_ms: captured.captured_lower_ms(),
            upper_ms: captured.captured_upper_ms(),
        };
        let financial = self.publication.cash_financial();
        let current = self.control.loan(financial).map_err(material)?;
        let assertion = self
            .lineage_cas
            .authenticate_received_commit_assertion(
                financial,
                &current,
                received_assertion_original,
            )
            .map_err(material)?;
        let outgoing = KagemushaOrdinaryCashOutgoingOriginalV1::decode_original(outgoing_original)
            .map_err(material)?;
        let clock = self
            .lineage_cas
            .authenticate_received_clock_original(
                financial,
                outgoing.admission_clock_signed_original(),
            )
            .map_err(material)?;
        let request = receiver_request::loan_main_request(self, request_id)?;
        let admitted = verify_ordinary_received_cash_output_v1(
            &self.verifier,
            &assertion,
            &request,
            outgoing_original,
            &clock,
        )
        .map_err(material)?;
        request
            .open_received(&admitted, &assertion)?
            .recheck_current_custody()?;
        let intake_clock = financial.current_cash_clock_context().map_err(material)?;
        let mut entropy = zeroize::Zeroizing::new([0_u8; 32]);
        OsRng.try_fill_bytes(entropy.as_mut()).map_err(material)?;
        let mut h = Sha256::new();
        h.update(b"iroha:kagemusha:v1:ordinary-native-received-source-operation\0");
        h.update(self.prefix.head);
        h.update(self.prefix.sequence.to_le_bytes());
        h.update(request_id);
        h.update(admitted.received_assertion_original_sha256());
        h.update(entropy.as_slice());
        let operation_id: DigestV1 = h.finalize().into();
        let originals = ReceivedSourceOriginals {
            request_id,
            operation_id,
            credit_id: admitted.credit_id(),
            amount: admitted.amount(),
            receiver_request_original_sha256: Sha256::digest(admitted.request_original()).into(),
            outgoing_original: outgoing_original.to_vec(),
            received_assertion_original: received_assertion_original.to_vec(),
            financial_control: identity,
            intake_clock,
        };
        originals.require_admitted(self, &admitted)?;
        self.require_new_received_source(&originals)?;
        self.append_receiver_frame(&Record::ReceivedSource(originals.clone()))?;
        // Follow the actual fsync before a post-fsync live expiry can report failure.
        self.used_operations.insert(operation_id);
        self.received_sources.insert(
            request_id,
            ReceivedSourceAdmission {
                originals,
                admitted,
            },
        );
        self.require_current_financial_control()?;
        Ok(operation_id)
    }

    pub(super) fn replay_received_source(
        &mut self,
        originals: ReceivedSourceOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_new_received_source(&originals)?;
        let admitted = originals.readmit(self)?;
        originals.require_admitted(self, &admitted)?;
        self.used_operations.insert(originals.operation_id);
        self.received_sources.insert(
            originals.request_id,
            ReceivedSourceAdmission {
                originals,
                admitted,
            },
        );
        Ok(())
    }

    fn require_received_source_idle(&self) -> Result<(), KagemushaStateErrorV1> {
        if self.pending.is_some()
            || self.pending_mint.is_some()
            || self.pending_incoming.is_some()
            || self.pending_receiver_request.is_some()
            || self.terminal.as_ref().is_none_or(|t| t.has_pending())
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        Ok(())
    }

    fn require_new_received_source(
        &self,
        next: &ReceivedSourceOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_received_source_idle()?;
        next.require_complete_source_budget(self)?;
        if next.operation_id == [0; 32]
            || self.used_operations.contains(&next.operation_id)
            || self.received_sources.contains_key(&next.request_id)
            || self
                .received_sources
                .values()
                .any(|s| s.originals.credit_id == next.credit_id)
            || !self
                .retained_receiver_requests
                .contains_key(&next.request_id)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let mut used = self.retained_received_source_capacity_charge()?;
        for request in self.retained_receiver_requests.values() {
            used = used
                .checked_add(request.captured.reservation().capacity_charge_bytes()?)
                .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        }
        if let Some(request) = &self.pending_receiver_request {
            used = used
                .checked_add(request.originals.capacity_charge_bytes()?)
                .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        }
        used = used
            .checked_add(next.encoded()?.len() as u64 + 256)
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        if used > self.capacity.inbox_bytes || self.received_sources.len() >= MAX_ROWS as usize {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        Ok(())
    }

    /// Full actual retained source bytes, including framing, used by all same-Main allocators.
    pub(super) fn retained_received_source_capacity_charge(
        &self,
    ) -> Result<u64, KagemushaStateErrorV1> {
        let mut total = 0u64;
        for (request_id, source) in &self.received_sources {
            if *request_id != source.originals.request_id
                || !self
                    .used_operations
                    .contains(&source.originals.operation_id)
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            source.originals.require_admitted(self, &source.admitted)?;
            total = total
                .checked_add(source.originals.encoded()?.len() as u64 + 256)
                .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        }
        Ok(total)
    }
}

// Actual Main-only historical proof custody; no current FI, funds or platform grant.
impl KagemushaNativeOrdinaryCashOwnerV1 {
    pub(super) fn historical_received_source_custody(
        &self,
        request_id: DigestV1,
    ) -> Result<KagemushaAuthenticatedOrdinaryReceivedSourceCustodyV1<'_>, KagemushaStateErrorV1>
    {
        let loan = KagemushaAuthenticatedOrdinaryReceivedSourceCustodyV1 {
            owner: self,
            request_id,
            prefix: self.prefix,
        };
        loan.recheck_source_custody()?;
        Ok(loan)
    }
}
