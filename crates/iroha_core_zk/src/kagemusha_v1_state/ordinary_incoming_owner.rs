//! Main's durable exact incoming head selection, before any online debit or fresh wallet W2.
//! Decoded selectors remain data. Their sources are re-admitted through actual retained owners.
use super::*;
use iroha_data_model::kagemusha::{
    KagemushaOrdinaryFinancialHeadV1, KagemushaOrdinaryIncomingSelectionV1,
    KagemushaOrdinaryIncomingSourceSelectionV1, KagemushaOrdinaryTopUpRequestV1,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryIncomingSourceLocatorV1")]
pub(super) enum SourceLocator {
    Mint,
    Receive { request_id: DigestV1 },
}

/// Complete operands held by Main before the Core caller can request a debit/head reservation.
/// The source operation was already durably selected by Mint or received-source intake.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryIncomingIntentV1")]
pub(super) struct IncomingIntentOriginals {
    pub(super) source: SourceLocator,
    pub(super) selection: KagemushaOrdinaryIncomingSelectionV1,
    pub(super) financial_control: CapturedFinancialControlIdentity,
    pub(super) selection_clock: KagemushaOrdinaryCashClockContextV1,
}

pub(super) struct PendingIncoming {
    pub(super) intent: IncomingIntentOriginals,
    pub(super) approval: Option<super::incoming_preparation::PendingIncomingApproval>,
    pub(super) terminal: Option<super::incoming_terminal::IncomingTerminalPending>,
}

impl IncomingIntentOriginals {
    pub(super) fn recheck_historical(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        owner.publication.recheck_historical_cash_custody()?;
        owner.journal.check_owned().map_err(storage)?;
        owner.recheck_lineage_retained_custody()?;
        self.selection.validate_shape().map_err(material)?;
        self.selection_clock.validate_shape().map_err(material)?;
        owner
            .control
            .recheck_retained_capture_identity(
                owner.publication.cash_financial(),
                self.financial_control.original_sha256,
                self.financial_control.lower_ms,
                self.financial_control.upper_ms,
            )
            .map_err(material)?;
        let clock = owner
            .publication
            .cash_financial()
            .verified_retained_cash_clock_originals(&self.selection_clock)
            .map_err(material)?;
        clock
            .recheck_cash_context(&self.selection_clock)
            .map_err(material)?;
        if self.selection_clock.lower_at_ms < self.financial_control.lower_ms
            || self.selection_clock.upper_at_ms < self.financial_control.upper_ms
            || self.selection.lineage != owner.initial_lineage_anchor.lineage
            || self.selection.predecessor != owner.incoming_current_head()
            || self.selection.recipient_app_credential_digest
                != owner
                    .publication
                    .cash_financial()
                    .enrollment()
                    .app_credential()
                    .digest()
            || !owner.used_operations.contains(&self.selection.operation_id)
            || owner.incoming_consumed.root() != owner.state.consumed_credit_root
            || owner
                .incoming_consumed
                .get(CreditIdV1(self.selection.credit_id))
                .is_some()
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        match self.source {
            SourceLocator::Mint => {
                let original = owner.retained_predebit_request_original()?;
                let admitted = owner.readmit_selected_mint_request(original)?;
                let request = KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
                    admitted.request_original(),
                )
                .map_err(material)?;
                self.selection
                    .validate_against_topup(&request)
                    .map_err(material)?;
            }
            SourceLocator::Receive { request_id } => {
                let (operation, admitted) = owner.retained_incoming_received_source(request_id)?;
                let expected = owner.incoming_received_selection(
                    operation,
                    admitted,
                    self.financial_control,
                    self.selection_clock,
                )?;
                if expected != self.selection {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
            }
        }
        owner.journal.check_owned().map_err(storage)
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    pub(super) fn incoming_current_head(&self) -> KagemushaOrdinaryFinancialHeadV1 {
        KagemushaOrdinaryFinancialHeadV1 {
            state_commitment: self.state.state_commitment,
            logical_sequence: self.state.logical_sequence,
            state_original_sha256: Sha256::digest(&self.public_state_original).into(),
        }
    }

    fn incoming_received_selection(
        &self,
        operation: DigestV1,
        admitted: &crate::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryReceivedCashOutputV1,
        financial_control: CapturedFinancialControlIdentity,
        clock: KagemushaOrdinaryCashClockContextV1,
    ) -> Result<KagemushaOrdinaryIncomingSelectionV1, KagemushaStateErrorV1> {
        let selection = KagemushaOrdinaryIncomingSelectionV1 {
            version: 1,
            lineage: self.initial_lineage_anchor.lineage.clone(),
            operation_id: operation,
            predecessor: self.incoming_current_head(),
            source: KagemushaOrdinaryIncomingSourceSelectionV1::Receive {
                sender_commit_transport_original_sha256: admitted
                    .received_assertion_original_sha256(),
                sender_outgoing_original_sha256: Sha256::digest(admitted.outgoing_original())
                    .into(),
                recipient_request_original_digest: admitted.output().request_digest,
                encrypted_credit_original_sha256: Sha256::digest(admitted.encrypted_credit())
                    .into(),
            },
            credit_id: admitted.credit_id(),
            amount: admitted.amount(),
            scale: self.initial_lineage_anchor.lineage.owner.runtime.scale,
            recipient_app_credential_digest: admitted.receiver_credential_digest(),
            financial_control_original_sha256: financial_control.original_sha256,
            clock_context_digest: clock.binding_digest().map_err(material)?,
        };
        selection.validate_shape().map_err(material)?;
        Ok(selection)
    }

    pub(super) fn capture_incoming_selection_control(
        &mut self,
    ) -> Result<
        (
            CapturedFinancialControlIdentity,
            KagemushaOrdinaryCashClockContextV1,
        ),
        KagemushaStateErrorV1,
    > {
        self.require_current_financial_control()?;
        let captured = self
            .control
            .capture_proof_decision(self.publication.cash_financial())
            .map_err(material)?;
        let identity = CapturedFinancialControlIdentity {
            original_sha256: captured.original_sha256().map_err(material)?,
            lower_ms: captured.captured_lower_ms(),
            upper_ms: captured.captured_upper_ms(),
        };
        let clock = self
            .publication
            .cash_financial()
            .current_cash_clock_context()
            .map_err(material)?;
        if clock.lower_at_ms < identity.lower_ms || clock.upper_at_ms < identity.upper_ms {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        Ok((identity, clock))
    }

    /// Retain the actual complete proven pre-debit source and exact current head before debit.
    /// This is an incoming attempt; no finalized source, wallet W2, balance or DATA result exists.
    pub(crate) fn reserve_incoming_mint(&mut self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if let Some(p) = &self.pending_incoming {
            if p.intent.source != SourceLocator::Mint {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            }
            p.intent.recheck_historical(self)?;
            return p.intent.selection.digest().map_err(material);
        }
        self.require_incoming_intent_idle(true)?;
        let proved = self.verified_retained_mint_request()?;
        let request =
            KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(proved.request_original())
                .map_err(material)?;
        let c = &request.authorization.statement.context;
        let selection = KagemushaOrdinaryIncomingSelectionV1 {
            version: 1,
            lineage: c.lineage.clone(),
            operation_id: c.operation_id,
            predecessor: c.predecessor,
            source: KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
                topup_request_original_sha256: proved.request_original_sha256(),
            },
            credit_id: c.credit_id().map_err(material)?,
            amount: c.amount,
            scale: c.lineage.owner.runtime.scale,
            recipient_app_credential_digest: c.recipient_app_credential_digest,
            financial_control_original_sha256: c.financial_control_original_sha256,
            clock_context_digest: c.clock_context.binding_digest().map_err(material)?,
        };
        selection
            .validate_against_topup(&request)
            .map_err(material)?;
        let (financial_control, selection_clock) = self.capture_incoming_selection_control()?;
        self.persist_incoming_intent(IncomingIntentOriginals {
            source: SourceLocator::Mint,
            selection,
            financial_control,
            selection_clock,
        })
    }

    /// Select only an actually retained complete received source at this exact private head.
    /// The intake operation/source remain immutable; a separate fresh incoming W2 follows.
    pub(crate) fn reserve_incoming_receive(
        &mut self,
        request_id: DigestV1,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if let Some(p) = &self.pending_incoming {
            if p.intent.source != (SourceLocator::Receive { request_id }) {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            }
            p.intent.recheck_historical(self)?;
            return p.intent.selection.digest().map_err(material);
        }
        self.require_incoming_intent_idle(false)?;
        let (financial_control, selection_clock) = self.capture_incoming_selection_control()?;
        let (operation, admitted) = self.retained_incoming_received_source(request_id)?;
        let selection = self.incoming_received_selection(
            operation,
            admitted,
            financial_control,
            selection_clock,
        )?;
        self.persist_incoming_intent(IncomingIntentOriginals {
            source: SourceLocator::Receive { request_id },
            selection,
            financial_control,
            selection_clock,
        })
    }

    fn require_incoming_intent_idle(&self, mint: bool) -> Result<(), KagemushaStateErrorV1> {
        self.require_initial_lineage_anchor_current()?;
        if self.pending.is_some()
            || self.pending_incoming.is_some()
            || self.pending_receiver_request.is_some()
            || self.pending_mint.is_some() != mint
            || self.prepared_commit.is_some()
            || self.prepared_incoming_commit.is_some()
            || self.terminal.as_ref().is_none_or(|t| t.has_pending())
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        Ok(())
    }

    fn persist_incoming_intent(
        &mut self,
        intent: IncomingIntentOriginals,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        intent.recheck_historical(self)?;
        let digest = intent.selection.digest().map_err(material)?;
        self.persist(&Record::IncomingIntent(intent.clone()))?;
        // Keep the actual fsynced operand even when the post-fsync current loan expires.
        self.pending_incoming = Some(PendingIncoming {
            intent,
            approval: None,
            terminal: None,
        });
        self.require_current_financial_control()?;
        Ok(digest)
    }

    pub(super) fn replay_incoming_intent(
        &mut self,
        intent: IncomingIntentOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.pending_incoming.is_some()
            || self.pending.is_some()
            || self.pending_receiver_request.is_some()
            || self.prepared_commit.is_some()
            || self.prepared_incoming_commit.is_some()
            || self.pending_mint.is_some() != matches!(intent.source, SourceLocator::Mint)
            || self.anchor_request_sha256.is_none()
            || self.terminal.as_ref().is_none_or(|t| t.has_pending())
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.require_state_advance_acknowledged()?;
        intent.recheck_historical(self)?;
        self.pending_incoming = Some(PendingIncoming {
            intent,
            approval: None,
            terminal: None,
        });
        Ok(())
    }
}
