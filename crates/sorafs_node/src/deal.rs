//! Deal engine accounting for SoraFS storage contracts (SF-8).
//!
//! The deal engine maintains provider/client balances, locks collateral,
//! evaluates probabilistic micropayment tickets, and produces settlement
//! records for governance pipelines.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use blake3::Hasher;
use iroha_data_model::sorafs::{
    capacity::ProviderId,
    deal::{
        BYTES_PER_GIB, ClientId, DealId, DealProposal, DealRecord, DealSettlementRecord,
        DealStatus, DealTerms, DealUsageReport, GIB_HOURS_PER_MONTH, TicketId,
    },
    pin_registry::StorageClass,
};
use iroha_telemetry::metrics::{
    MicropaymentCreditSnapshot, MicropaymentTicketCounters, global_sorafs_node_otel,
};
use norito::Error as NoritoError;
use sorafs_manifest::deal::{
    DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1,
    DealSettlementStatusV1, DealSettlementV1, XorAmount,
};
use thiserror::Error;

const DEAL_ID_DOMAIN: &[u8] = b"sorafs.deal.id.v1";
const MICROPAYMENT_DOMAIN: &[u8] = b"sorafs.ticket.draw.v1";
const BASIS_POINTS_SCALE: u64 = 10_000;
const NANO_PER_MICRO: u128 = 1_000;

fn nano_to_xor_amount(nano: u128) -> XorAmount {
    XorAmount::from_micro(nano / NANO_PER_MICRO)
}

#[derive(Debug, Default)]
struct Inner {
    providers: HashMap<ProviderId, ProviderAccount>,
    clients: HashMap<ClientId, ClientAccount>,
    deals: HashMap<DealId, DealState>,
}

#[derive(Debug, Default, Clone)]
struct ProviderAccount {
    bond_available_nano: u128,
    bond_locked_nano: u128,
    earnings_nano: u128,
}

#[derive(Debug, Default, Clone)]
struct ClientAccount {
    credit_balance_nano: u128,
}

#[derive(Debug)]
struct DealState {
    record: DealRecord,
    locked_bond_nano: u128,
    outstanding_nano: u128,
    micropayment_credit_carry: u128,
    total_expected_charge_nano: u128,
    total_micropayment_credit_nano: u128,
    total_client_debit_nano: u128,
    total_bond_slash_nano: u128,
    window_expected_charge_nano: u128,
    window_micropayment_credit_applied: u128,
    window_storage_gib_hours: u128,
    window_egress_bytes: u128,
    total_storage_gib_hours: u128,
    total_egress_bytes: u128,
    settlement_count: u64,
    last_settlement_epoch: u64,
    seen_tickets: HashSet<[u8; 32]>,
}

impl DealState {
    fn new(record: DealRecord, locked_bond_nano: u128, activation_epoch: u64) -> Self {
        Self {
            record,
            locked_bond_nano,
            outstanding_nano: 0,
            micropayment_credit_carry: 0,
            total_expected_charge_nano: 0,
            total_micropayment_credit_nano: 0,
            total_client_debit_nano: 0,
            total_bond_slash_nano: 0,
            window_expected_charge_nano: 0,
            window_micropayment_credit_applied: 0,
            window_storage_gib_hours: 0,
            window_egress_bytes: 0,
            total_storage_gib_hours: 0,
            total_egress_bytes: 0,
            settlement_count: 0,
            last_settlement_epoch: activation_epoch,
            seen_tickets: HashSet::new(),
        }
    }
}

/// Error type returned by the deal engine.
#[derive(Debug, Error)]
pub enum DealEngineError {
    /// Provider must register collateral before opening deals.
    #[error("unknown provider {0:?}")]
    UnknownProvider(ProviderId),
    /// Client must provision credit before participating in deals.
    #[error("unknown client {0:?}")]
    UnknownClient(ClientId),
    /// Provider collateral is insufficient to lock the deal bond.
    #[error(
        "insufficient bond for provider {provider:?}: required {required}, available {available}"
    )]
    InsufficientBond {
        /// Provider identifier.
        provider: ProviderId,
        /// Bond required for the deal (nano-XOR).
        required: u128,
        /// Currently available bond (nano-XOR).
        available: u128,
    },
    /// Deal with the same identifier already exists.
    #[error("deal already exists {0:?}")]
    DuplicateDeal(DealId),
    /// Deal not known to the engine.
    #[error("deal not found {0:?}")]
    UnknownDeal(DealId),
    /// Deal is not active when usage or settlement is attempted.
    #[error("deal not active {0:?}")]
    DealInactive(DealId),
    /// Activation epoch falls outside the negotiated bounds.
    #[error("activation epoch {activation_epoch} outside [{start}, {end}] for {deal_id:?}")]
    ActivationOutOfRange {
        /// Deal identifier.
        deal_id: DealId,
        /// Epoch used for activation.
        activation_epoch: u64,
        /// Expected lower bound.
        start: u64,
        /// Expected upper bound.
        end: u64,
    },
    /// Usage sample targets an epoch outside the deal window.
    #[error("usage epoch {usage_epoch} outside [{start}, {end}] for {deal_id:?}")]
    UsageEpochOutOfRange {
        /// Deal identifier.
        deal_id: DealId,
        /// Epoch supplied by the caller.
        usage_epoch: u64,
        /// Expected lower bound.
        start: u64,
        /// Expected upper bound.
        end: u64,
    },
    /// Settlement attempted before the window duration elapsed.
    #[error(
        "settlement epoch {settlement_epoch} does not satisfy window length {window_epochs} for {deal_id:?}"
    )]
    SettlementWindowMismatch {
        /// Deal identifier.
        deal_id: DealId,
        /// Epoch supplied by the caller.
        settlement_epoch: u64,
        /// Required settlement window in epochs.
        window_epochs: u64,
    },
    /// Metadata encoding failed when deriving the deal identifier.
    #[error("metadata encoding failed: {0}")]
    MetadataEncoding(#[from] NoritoError),
}

/// Snapshot describing a provider account.
#[derive(Debug, Clone, Copy)]
pub struct ProviderSnapshot {
    /// Bond not currently locked by deals.
    pub bond_available_nano: u128,
    /// Bond locked against active deals.
    pub bond_locked_nano: u128,
    /// Earnings accrued from client settlements and micropayments.
    pub earnings_nano: u128,
}

/// Snapshot describing a client account.
#[derive(Debug, Clone, Copy)]
pub struct ClientSnapshot {
    /// Credit balance available for settlements.
    pub credit_balance_nano: u128,
}

/// Snapshot describing deal-level accounting.
#[derive(Debug, Clone)]
pub struct DealSnapshot {
    /// Deal identifier.
    pub deal_id: DealId,
    /// Current lifecycle status.
    pub status: DealStatus,
    /// Outstanding balance after applying micropayments and settlements.
    pub outstanding_nano: u128,
    /// Micropayment credit held for future windows.
    pub credit_carry_nano: u128,
    /// Bond reserved for the deal.
    pub locked_bond_nano: u128,
    /// Completed settlement windows.
    pub settlement_count: u64,
}

/// Outcome returned after recording a usage sample.
#[derive(Debug, Clone)]
pub struct UsageOutcome {
    /// Deal identifier.
    pub deal_id: DealId,
    /// Provider servicing the deal.
    pub provider_id: ProviderId,
    /// Client responsible for the deal.
    pub client_id: ClientId,
    /// Deterministic charge accumulated for the sample.
    pub deterministic_charge_nano: u128,
    /// Micropayment credit generated during this sample.
    pub micropayment_credit_generated_nano: u128,
    /// Micropayment credit applied immediately against the charge.
    pub micropayment_credit_applied_nano: u128,
    /// Micropayment credit carried forward to future windows.
    pub micropayment_credit_carry_nano: u128,
    /// Outstanding balance after applying credit.
    pub outstanding_nano: u128,
    /// Tickets processed in this sample.
    pub tickets_processed: usize,
    /// Tickets that resulted in a payout.
    pub tickets_won: usize,
    /// Tickets ignored due to duplication.
    pub tickets_duplicate: usize,
}

/// Result produced when finalising a settlement window.
#[derive(Debug, Clone)]
pub struct DealSettlementOutcome {
    /// Ledger record for internal consumers (Torii, CLI, etc.).
    pub record: DealSettlementRecord,
    /// Governance payload ready for DAG publication.
    pub governance: DealSettlementV1,
}

/// Deterministic accounting engine for storage deals.
#[derive(Debug, Clone)]
pub struct DealEngine {
    inner: Arc<RwLock<Inner>>,
}

impl Default for DealEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DealEngine {
    /// Construct a new deal engine instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner::default())),
        }
    }

    /// Register or top up a provider bond.
    pub fn deposit_provider_bond(
        &self,
        provider_id: ProviderId,
        amount_nano: u128,
    ) -> ProviderSnapshot {
        let mut inner = self.inner.write().expect("deal engine poisoned");
        let account = inner.providers.entry(provider_id).or_default();
        account.bond_available_nano = account.bond_available_nano.saturating_add(amount_nano);
        ProviderSnapshot {
            bond_available_nano: account.bond_available_nano,
            bond_locked_nano: account.bond_locked_nano,
            earnings_nano: account.earnings_nano,
        }
    }

    /// Register or top up a client credit balance.
    pub fn deposit_client_credit(&self, client_id: ClientId, amount_nano: u128) -> ClientSnapshot {
        let mut inner = self.inner.write().expect("deal engine poisoned");
        let account = inner.clients.entry(client_id).or_default();
        account.credit_balance_nano = account.credit_balance_nano.saturating_add(amount_nano);
        ClientSnapshot {
            credit_balance_nano: account.credit_balance_nano,
        }
    }

    /// Open a deal by locking collateral and tracking its lifecycle.
    pub fn open_deal(
        &self,
        proposal: DealProposal,
        activation_epoch: u64,
    ) -> Result<DealRecord, DealEngineError> {
        let deal_id = compute_deal_id(&proposal)?;

        if activation_epoch < proposal.start_epoch || activation_epoch > proposal.end_epoch {
            return Err(DealEngineError::ActivationOutOfRange {
                deal_id,
                activation_epoch,
                start: proposal.start_epoch,
                end: proposal.end_epoch,
            });
        }

        let mut inner = self.inner.write().expect("deal engine poisoned");
        let inner_ref = &mut *inner;
        let deals = &mut inner_ref.deals;
        let providers = &mut inner_ref.providers;
        let clients = &inner_ref.clients;

        if deals.contains_key(&deal_id) {
            return Err(DealEngineError::DuplicateDeal(deal_id));
        }

        if !clients.contains_key(&proposal.client_id) {
            return Err(DealEngineError::UnknownClient(proposal.client_id));
        }

        let provider = providers
            .get_mut(&proposal.provider_id)
            .ok_or(DealEngineError::UnknownProvider(proposal.provider_id))?;

        let bond_required = proposal.terms.bond_requirement_nano(proposal.capacity_gib);
        if provider.bond_available_nano < bond_required {
            return Err(DealEngineError::InsufficientBond {
                provider: proposal.provider_id,
                required: bond_required,
                available: provider.bond_available_nano,
            });
        }

        provider.bond_available_nano -= bond_required;
        provider.bond_locked_nano = provider.bond_locked_nano.saturating_add(bond_required);

        let record = DealRecord {
            deal_id,
            provider_id: proposal.provider_id,
            client_id: proposal.client_id,
            storage_class: proposal.storage_class,
            capacity_gib: proposal.capacity_gib,
            start_epoch: proposal.start_epoch,
            end_epoch: proposal.end_epoch,
            terms: proposal.terms,
            metadata: proposal.metadata,
            status: DealStatus::Active(activation_epoch),
        };

        deals.insert(
            deal_id,
            DealState::new(record.clone(), bond_required, activation_epoch),
        );

        Ok(record)
    }

    /// Record usage for an active deal and evaluate micropayment tickets.
    pub fn record_usage(&self, report: DealUsageReport) -> Result<UsageOutcome, DealEngineError> {
        let mut inner = self.inner.write().expect("deal engine poisoned");
        let inner_ref = &mut *inner;
        let deals = &mut inner_ref.deals;
        let providers = &mut inner_ref.providers;
        let clients = &inner_ref.clients;

        let entry = deals
            .get(&report.deal_id)
            .ok_or(DealEngineError::UnknownDeal(report.deal_id))?;

        let (start, end) = match entry.record.status {
            DealStatus::Active(_) => (entry.record.start_epoch, entry.record.end_epoch),
            _ => return Err(DealEngineError::DealInactive(report.deal_id)),
        };

        if report.epoch < start || report.epoch > end {
            return Err(DealEngineError::UsageEpochOutOfRange {
                deal_id: report.deal_id,
                usage_epoch: report.epoch,
                start,
                end,
            });
        }

        let provider_id = entry.record.provider_id;
        let client_id = entry.record.client_id;

        if !clients.contains_key(&client_id) {
            return Err(DealEngineError::UnknownClient(client_id));
        }

        let mut state = deals
            .remove(&report.deal_id)
            .expect("deal state removed after presence check");

        let sample_charge =
            storage_charge(report.storage_gib_hours as u128, &state.record.terms).saturating_add(
                egress_charge(report.egress_bytes as u128, &state.record.terms),
            );

        state.window_storage_gib_hours = state
            .window_storage_gib_hours
            .saturating_add(report.storage_gib_hours as u128);
        state.window_egress_bytes = state
            .window_egress_bytes
            .saturating_add(report.egress_bytes as u128);
        state.window_expected_charge_nano = state
            .window_expected_charge_nano
            .saturating_add(sample_charge);
        state.total_storage_gib_hours = state
            .total_storage_gib_hours
            .saturating_add(report.storage_gib_hours as u128);
        state.total_egress_bytes = state
            .total_egress_bytes
            .saturating_add(report.egress_bytes as u128);

        let mut tickets_processed = 0usize;
        let mut tickets_won = 0usize;
        let mut tickets_duplicate = 0usize;
        let mut new_credit = 0u128;
        let mut generated_credit = 0u128;

        for ticket in &report.tickets {
            tickets_processed += 1;
            let ticket_bytes = *ticket.ticket_id.as_bytes();
            if !state.seen_tickets.insert(ticket_bytes) {
                tickets_duplicate += 1;
                continue;
            }

            if evaluate_ticket(
                state.record.deal_id,
                ticket.ticket_id,
                state.record.terms.micropayment_probability_bps,
            ) {
                tickets_won += 1;
                let payout = state.record.terms.micropayment_payout_nano as u128;
                new_credit = new_credit.saturating_add(payout);
                generated_credit = generated_credit.saturating_add(payout);
            }
        }

        let mut due_remaining = sample_charge;
        let provider_credit_total = new_credit;

        let mut credit_applied = 0u128;

        if due_remaining > 0 && state.micropayment_credit_carry > 0 {
            let applied = due_remaining.min(state.micropayment_credit_carry);
            due_remaining -= applied;
            state.micropayment_credit_carry -= applied;
            credit_applied = credit_applied.saturating_add(applied);
        }

        if due_remaining > 0 && new_credit > 0 {
            let applied = due_remaining.min(new_credit);
            due_remaining -= applied;
            new_credit -= applied;
            credit_applied = credit_applied.saturating_add(applied);
        }

        state.micropayment_credit_carry =
            state.micropayment_credit_carry.saturating_add(new_credit);
        state.window_micropayment_credit_applied = state
            .window_micropayment_credit_applied
            .saturating_add(credit_applied);
        state.outstanding_nano = state.outstanding_nano.saturating_add(due_remaining);

        if provider_credit_total > 0 {
            let provider = providers
                .get_mut(&provider_id)
                .expect("provider accounted during deal opening");
            provider.earnings_nano = provider.earnings_nano.saturating_add(provider_credit_total);
        }

        let outcome = UsageOutcome {
            deal_id: report.deal_id,
            provider_id: state.record.provider_id,
            client_id: state.record.client_id,
            deterministic_charge_nano: sample_charge,
            micropayment_credit_generated_nano: generated_credit,
            micropayment_credit_applied_nano: credit_applied,
            micropayment_credit_carry_nano: state.micropayment_credit_carry,
            outstanding_nano: state.outstanding_nano,
            tickets_processed,
            tickets_won,
            tickets_duplicate,
        };

        let provider_hex = hex::encode(state.record.provider_id.as_bytes());
        global_sorafs_node_otel().record_micropayment_sample(
            &provider_hex,
            MicropaymentCreditSnapshot {
                deterministic_charge: sample_charge,
                credit_generated: generated_credit,
                credit_applied,
                credit_carry: state.micropayment_credit_carry,
                outstanding: state.outstanding_nano,
            },
            MicropaymentTicketCounters {
                processed: tickets_processed as u64,
                won: tickets_won as u64,
                duplicate: tickets_duplicate as u64,
            },
        );

        deals.insert(report.deal_id, state);

        Ok(outcome)
    }

    /// Settle the current window, withdrawing client credit and applying bond slashing if needed.
    pub fn settle(
        &self,
        deal_id: DealId,
        settlement_epoch: u64,
    ) -> Result<DealSettlementOutcome, DealEngineError> {
        let mut inner = self.inner.write().expect("deal engine poisoned");
        let inner_ref = &mut *inner;
        let deals = &mut inner_ref.deals;
        let providers = &mut inner_ref.providers;
        let clients = &mut inner_ref.clients;

        let entry = deals
            .get(&deal_id)
            .ok_or(DealEngineError::UnknownDeal(deal_id))?;

        let window_epochs = entry.record.terms.settlement_window_epochs.max(1);

        if !matches!(entry.record.status, DealStatus::Active(_)) {
            return Err(DealEngineError::DealInactive(deal_id));
        }

        if settlement_epoch <= entry.last_settlement_epoch
            || settlement_epoch - entry.last_settlement_epoch < window_epochs
        {
            return Err(DealEngineError::SettlementWindowMismatch {
                deal_id,
                settlement_epoch,
                window_epochs,
            });
        }

        let provider_id = entry.record.provider_id;
        let client_id = entry.record.client_id;

        if !clients.contains_key(&client_id) {
            return Err(DealEngineError::UnknownClient(client_id));
        }

        let mut state = deals
            .remove(&deal_id)
            .expect("deal state removed after presence check");

        let window_start = state.last_settlement_epoch;
        let expected_charge = state.window_expected_charge_nano;
        let credit_applied = state.window_micropayment_credit_applied;
        let mut window_outstanding = expected_charge.saturating_sub(credit_applied);

        let previous_outstanding = state.outstanding_nano.saturating_sub(window_outstanding);

        let mut client_debit = 0u128;
        {
            let client = clients
                .get_mut(&client_id)
                .expect("client accounted during deal opening");
            if window_outstanding > 0 && client.credit_balance_nano > 0 {
                client_debit = window_outstanding.min(client.credit_balance_nano);
                client.credit_balance_nano -= client_debit;
                window_outstanding -= client_debit;
            }
        }

        let mut bond_slash = 0u128;
        {
            let provider = providers
                .get_mut(&provider_id)
                .expect("provider accounted during deal opening");
            if client_debit > 0 {
                provider.earnings_nano = provider.earnings_nano.saturating_add(client_debit);
            }
            if window_outstanding > 0 && state.locked_bond_nano > 0 {
                bond_slash = window_outstanding.min(state.locked_bond_nano);
                state.locked_bond_nano -= bond_slash;
                provider.bond_locked_nano = provider.bond_locked_nano.saturating_sub(bond_slash);
                window_outstanding -= bond_slash;
            }
        }

        state.outstanding_nano = previous_outstanding.saturating_add(window_outstanding);
        state.settlement_count = state.settlement_count.saturating_add(1);
        state.last_settlement_epoch = settlement_epoch;

        state.total_expected_charge_nano = state
            .total_expected_charge_nano
            .saturating_add(expected_charge);
        state.total_micropayment_credit_nano = state
            .total_micropayment_credit_nano
            .saturating_add(credit_applied);
        state.total_client_debit_nano = state.total_client_debit_nano.saturating_add(client_debit);
        state.total_bond_slash_nano = state.total_bond_slash_nano.saturating_add(bond_slash);

        if state.outstanding_nano == 0
            && state.micropayment_credit_carry == 0
            && settlement_epoch >= state.record.end_epoch
        {
            let locked_bond = state.locked_bond_nano;
            if locked_bond > 0 {
                let provider = providers
                    .get_mut(&provider_id)
                    .expect("provider accounted during deal opening");
                provider.bond_available_nano =
                    provider.bond_available_nano.saturating_add(locked_bond);
                provider.bond_locked_nano = provider.bond_locked_nano.saturating_sub(locked_bond);
            }
            state.locked_bond_nano = 0;
            state.record.status = DealStatus::Settled(settlement_epoch);
        }

        let settlement = DealSettlementRecord {
            provider_id: state.record.provider_id,
            client_id: state.record.client_id,
            deal_id,
            settlement_index: state.settlement_count,
            settled_epoch: settlement_epoch,
            window_start_epoch: window_start,
            window_end_epoch: settlement_epoch,
            billed_storage_gib_hours: state.window_storage_gib_hours,
            billed_egress_bytes: state.window_egress_bytes,
            expected_charge_nano: expected_charge,
            micropayment_credit_nano: credit_applied,
            client_credit_debit_nano: client_debit,
            bond_slash_nano: bond_slash,
            outstanding_nano: state.outstanding_nano,
        };

        state.window_expected_charge_nano = 0;
        state.window_micropayment_credit_applied = 0;
        state.window_storage_gib_hours = 0;
        state.window_egress_bytes = 0;

        let provider_accrual_nano = state
            .total_client_debit_nano
            .saturating_add(state.total_micropayment_credit_nano)
            .saturating_add(state.total_bond_slash_nano);
        let ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            deal_id: *deal_id.as_bytes(),
            provider_id: *state.record.provider_id.as_bytes(),
            client_id: *state.record.client_id.as_bytes(),
            provider_accrual: nano_to_xor_amount(provider_accrual_nano),
            client_liability: nano_to_xor_amount(state.total_expected_charge_nano),
            bond_locked: nano_to_xor_amount(state.locked_bond_nano),
            bond_slashed: nano_to_xor_amount(state.total_bond_slash_nano),
            captured_at: settlement_epoch,
        };
        let status = if bond_slash > 0 {
            DealSettlementStatusV1::Slashed
        } else if matches!(state.record.status, DealStatus::Cancelled(_)) {
            DealSettlementStatusV1::Cancelled
        } else {
            DealSettlementStatusV1::Completed
        };
        let provider_label = hex::encode(state.record.provider_id.as_bytes());
        let status_label = match status {
            DealSettlementStatusV1::Completed => "completed",
            DealSettlementStatusV1::Cancelled => "cancelled",
            DealSettlementStatusV1::Slashed => "slashed",
        };
        global_sorafs_node_otel().record_deal_settlement(
            &provider_label,
            status_label,
            expected_charge,
            client_debit,
            bond_slash,
            state.outstanding_nano,
        );
        let audit_notes = match status {
            DealSettlementStatusV1::Slashed => Some(format!(
                "bond slashed {} nano (total {} nano); outstanding {} nano",
                bond_slash, state.total_bond_slash_nano, state.outstanding_nano
            )),
            DealSettlementStatusV1::Cancelled => {
                Some("deal cancelled by governance prior to completion".to_string())
            }
            DealSettlementStatusV1::Completed => None,
        };
        let governance = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            deal_id: *deal_id.as_bytes(),
            ledger,
            status,
            settled_at: settlement_epoch,
            audit_notes,
        };

        let outcome = DealSettlementOutcome {
            record: settlement,
            governance,
        };

        deals.insert(deal_id, state);

        Ok(outcome)
    }

    /// Obtain a snapshot of the provider account.
    pub fn provider_snapshot(&self, provider_id: ProviderId) -> Option<ProviderSnapshot> {
        let inner = self.inner.read().expect("deal engine poisoned");
        inner
            .providers
            .get(&provider_id)
            .map(|account| ProviderSnapshot {
                bond_available_nano: account.bond_available_nano,
                bond_locked_nano: account.bond_locked_nano,
                earnings_nano: account.earnings_nano,
            })
    }

    /// Obtain a snapshot of the client account.
    pub fn client_snapshot(&self, client_id: ClientId) -> Option<ClientSnapshot> {
        let inner = self.inner.read().expect("deal engine poisoned");
        inner.clients.get(&client_id).map(|account| ClientSnapshot {
            credit_balance_nano: account.credit_balance_nano,
        })
    }

    /// Obtain a snapshot of the current deal state.
    pub fn deal_snapshot(&self, deal_id: DealId) -> Option<DealSnapshot> {
        let inner = self.inner.read().expect("deal engine poisoned");
        inner.deals.get(&deal_id).map(|state| DealSnapshot {
            deal_id,
            status: state.record.status,
            outstanding_nano: state.outstanding_nano,
            credit_carry_nano: state.micropayment_credit_carry,
            locked_bond_nano: state.locked_bond_nano,
            settlement_count: state.settlement_count,
        })
    }
}

fn compute_deal_id(proposal: &DealProposal) -> Result<DealId, NoritoError> {
    let mut hasher = Hasher::new();
    hasher.update(DEAL_ID_DOMAIN);
    hasher.update(proposal.provider_id.as_bytes());
    hasher.update(proposal.client_id.as_bytes());
    hasher.update(&proposal.capacity_gib.to_le_bytes());
    hasher.update(&proposal.start_epoch.to_le_bytes());
    hasher.update(&proposal.end_epoch.to_le_bytes());
    hasher.update(
        &proposal
            .terms
            .storage_price_nano_per_gib_month
            .to_le_bytes(),
    );
    hasher.update(&proposal.terms.egress_price_nano_per_gib.to_le_bytes());
    hasher.update(&proposal.terms.settlement_window_epochs.to_le_bytes());
    hasher.update(&proposal.terms.micropayment_probability_bps.to_le_bytes());
    hasher.update(&proposal.terms.micropayment_payout_nano.to_le_bytes());
    hasher.update(&[storage_class_tag(proposal.storage_class)]);
    let metadata_bytes = norito::to_bytes(&proposal.metadata)?;
    hasher.update(&metadata_bytes);
    let digest = hasher.finalize();
    Ok(DealId::new(*digest.as_bytes()))
}

fn storage_class_tag(class: StorageClass) -> u8 {
    match class {
        StorageClass::Hot => 0,
        StorageClass::Warm => 1,
        StorageClass::Cold => 2,
    }
}

fn evaluate_ticket(deal_id: DealId, ticket_id: TicketId, probability_bps: u16) -> bool {
    let probability = probability_bps as u64;
    if probability == 0 {
        return false;
    }
    if probability >= BASIS_POINTS_SCALE {
        return true;
    }

    let mut hasher = Hasher::new();
    hasher.update(MICROPAYMENT_DOMAIN);
    hasher.update(deal_id.as_bytes());
    hasher.update(ticket_id.as_bytes());
    let digest = hasher.finalize();
    let value = u64::from_le_bytes(digest.as_bytes()[..8].try_into().expect("slice length"));
    value % BASIS_POINTS_SCALE < probability
}

fn storage_charge(gib_hours: u128, terms: &DealTerms) -> u128 {
    (terms.storage_price_nano_per_gib_month as u128)
        .saturating_mul(gib_hours)
        .saturating_div(GIB_HOURS_PER_MONTH.max(1))
}

fn egress_charge(bytes: u128, terms: &DealTerms) -> u128 {
    (terms.egress_price_nano_per_gib as u128)
        .saturating_mul(bytes)
        .saturating_div(BYTES_PER_GIB.max(1))
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{metadata::Metadata, sorafs::deal::MicropaymentTicket};
    use sorafs_manifest::deal::DealSettlementStatusV1;

    use super::*;

    fn sample_terms() -> DealTerms {
        DealTerms {
            storage_price_nano_per_gib_month: 500_000_000,
            egress_price_nano_per_gib: 50_000_000,
            settlement_window_epochs: 7,
            micropayment_probability_bps: BASIS_POINTS_SCALE as u16,
            micropayment_payout_nano: 100_000_000,
        }
    }

    fn provider(id_byte: u8) -> ProviderId {
        ProviderId([id_byte; 32])
    }

    fn client(id_byte: u8) -> ClientId {
        ClientId([id_byte; 32])
    }

    fn ticket(id_byte: u8) -> MicropaymentTicket {
        MicropaymentTicket {
            ticket_id: TicketId([id_byte; 32]),
            issued_epoch: 11,
            storage_gib_hours: 0,
            egress_bytes: 0,
        }
    }

    #[test]
    fn open_deal_requires_sufficient_bond() {
        let engine = DealEngine::new();
        let provider = provider(1);
        let client = client(2);

        engine.deposit_provider_bond(provider, 1_000_000_000);
        engine.deposit_client_credit(client, 1_000_000_000);

        let proposal = DealProposal {
            provider_id: provider,
            client_id: client,
            storage_class: StorageClass::Hot,
            capacity_gib: 10,
            start_epoch: 10,
            end_epoch: 20,
            terms: sample_terms(),
            metadata: Metadata::default(),
        };

        let err = engine
            .open_deal(proposal, 10)
            .expect_err("bond should be insufficient");

        matches!(
            err,
            DealEngineError::InsufficientBond {
                provider: _,
                required: _,
                available: _
            }
        );
    }

    #[test]
    fn deal_lifecycle_settlement_releases_bond() {
        let engine = DealEngine::new();
        let provider = provider(1);
        let client = client(2);

        engine.deposit_provider_bond(provider, 15_000_000_000);
        engine.deposit_client_credit(client, 3_000_000_000);

        let proposal = DealProposal {
            provider_id: provider,
            client_id: client,
            storage_class: StorageClass::Hot,
            capacity_gib: 5,
            start_epoch: 10,
            end_epoch: 16,
            terms: sample_terms(),
            metadata: Metadata::default(),
        };

        let record = engine.open_deal(proposal, 10).expect("deal opens");

        let usage = DealUsageReport {
            deal_id: record.deal_id,
            epoch: 12,
            storage_gib_hours: 5 * GIB_HOURS_PER_MONTH as u64,
            egress_bytes: BYTES_PER_GIB as u64,
            tickets: vec![ticket(10), ticket(11), ticket(12), ticket(13), ticket(14)],
        };

        let outcome = engine.record_usage(usage).expect("usage recorded");
        assert_eq!(outcome.tickets_won, 5);

        let settlement_outcome = engine
            .settle(record.deal_id, 17)
            .expect("settlement succeeds");
        let settlement = &settlement_outcome.record;

        assert_eq!(settlement.expected_charge_nano, 2_550_000_000);
        assert_eq!(settlement.micropayment_credit_nano, 500_000_000);
        assert_eq!(settlement.client_credit_debit_nano, 2_050_000_000);
        assert_eq!(settlement.bond_slash_nano, 0);
        assert_eq!(settlement.outstanding_nano, 0);

        let governance = &settlement_outcome.governance;
        assert_eq!(governance.status, DealSettlementStatusV1::Completed);
        assert_eq!(governance.ledger.provider_accrual.as_micro(), 2_550_000);

        let provider_snapshot = engine.provider_snapshot(provider).expect("provider");
        assert_eq!(provider_snapshot.bond_available_nano, 15_000_000_000);
        assert_eq!(provider_snapshot.bond_locked_nano, 0);
        assert_eq!(provider_snapshot.earnings_nano, 2_550_000_000);

        let deal_snapshot = engine.deal_snapshot(record.deal_id).expect("deal snapshot");
        matches!(deal_snapshot.status, DealStatus::Settled(17));
        assert_eq!(deal_snapshot.outstanding_nano, 0);
        assert_eq!(deal_snapshot.locked_bond_nano, 0);
    }

    #[test]
    fn usage_outcome_reports_micropayment_metrics() {
        let engine = DealEngine::new();
        let provider = provider(3);
        let client = client(4);

        engine.deposit_provider_bond(provider, 15_000_000_000);
        engine.deposit_client_credit(client, 5_000_000_000);

        let terms = sample_terms();
        let expected_charge = (terms.storage_price_nano_per_gib_month as u128)
            .saturating_add(terms.egress_price_nano_per_gib as u128);
        let expected_credit = (terms.micropayment_payout_nano as u128).saturating_mul(2);
        let expected_outstanding = expected_charge.saturating_sub(expected_credit);

        let proposal = DealProposal {
            provider_id: provider,
            client_id: client,
            storage_class: StorageClass::Hot,
            capacity_gib: 10,
            start_epoch: 10,
            end_epoch: 20,
            terms,
            metadata: Metadata::default(),
        };

        let record = engine.open_deal(proposal, 10).expect("deal opens");

        let usage = DealUsageReport {
            deal_id: record.deal_id,
            epoch: 11,
            storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
            egress_bytes: BYTES_PER_GIB as u64,
            tickets: vec![ticket(10), ticket(11)],
        };

        let outcome = engine.record_usage(usage).expect("usage recorded");

        assert_eq!(outcome.provider_id, provider);
        assert_eq!(outcome.client_id, client);
        assert_eq!(outcome.deterministic_charge_nano, expected_charge);
        assert_eq!(outcome.micropayment_credit_generated_nano, expected_credit);
        assert_eq!(outcome.micropayment_credit_applied_nano, expected_credit);
        assert_eq!(outcome.micropayment_credit_carry_nano, 0);
        assert_eq!(outcome.outstanding_nano, expected_outstanding);
        assert_eq!(outcome.tickets_processed, 2);
        assert_eq!(outcome.tickets_won, 2);
        assert_eq!(outcome.tickets_duplicate, 0);
    }

    #[test]
    fn duplicate_ticket_is_ignored() {
        let engine = DealEngine::new();
        let provider = provider(7);
        let client = client(8);

        engine.deposit_provider_bond(provider, 15_000_000_000);
        engine.deposit_client_credit(client, 3_000_000_000);

        let proposal = DealProposal {
            provider_id: provider,
            client_id: client,
            storage_class: StorageClass::Hot,
            capacity_gib: 5,
            start_epoch: 10,
            end_epoch: 20,
            terms: sample_terms(),
            metadata: Metadata::default(),
        };

        let record = engine.open_deal(proposal, 10).expect("deal opens");

        let usage = DealUsageReport {
            deal_id: record.deal_id,
            epoch: 11,
            storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
            egress_bytes: BYTES_PER_GIB as u64,
            tickets: vec![ticket(42), ticket(42)],
        };

        let outcome = engine.record_usage(usage).expect("usage recorded");
        assert_eq!(outcome.tickets_processed, 2);
        assert_eq!(outcome.tickets_duplicate, 1);
        assert_eq!(outcome.tickets_won, 1);
    }
}
