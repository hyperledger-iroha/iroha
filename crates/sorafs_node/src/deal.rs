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
        ClientId, DealId, DealProposal, DealRecord, DealSettlementRecord, DealStatus, DealTerms,
        DealUsageReport, TicketId,
    },
    pin_registry::StorageClass,
};
use norito::{
    Error as NoritoError,
    derive::{NoritoDeserialize, NoritoSerialize},
};
use sorafs_manifest::deal::{
    DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1,
    DealSettlementStatusV1, DealSettlementV1, XorQuantity,
};
use thiserror::Error;

const DEAL_ID_DOMAIN: &[u8] = b"sorafs.deal.id.v1";
const MICROPAYMENT_DOMAIN: &[u8] = b"sorafs.ticket.draw.v1";
const BASIS_POINTS_SCALE: u64 = 10_000;
fn checked_deal_add(
    left: &XorQuantity,
    right: &XorQuantity,
    resource: &'static str,
) -> Result<XorQuantity, DealEngineError> {
    left.checked_add(right)
        .map_err(|_| DealEngineError::BalanceOverflow { resource })
}

fn checked_counter_add(
    left: u128,
    right: u128,
    resource: &'static str,
) -> Result<u128, DealEngineError> {
    left.checked_add(right)
        .ok_or(DealEngineError::BalanceOverflow { resource })
}

#[derive(Debug, Default)]
struct Inner {
    providers: HashMap<ProviderId, ProviderAccount>,
    clients: HashMap<ClientId, ClientAccount>,
    deals: HashMap<DealId, DealState>,
    seen_ticket_count: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ProviderAccount {
    bond_available: XorQuantity,
    bond_locked: XorQuantity,
    earnings: XorQuantity,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ClientAccount {
    credit_balance: XorQuantity,
}

#[derive(Debug, Clone)]
struct DealState {
    record: DealRecord,
    locked_bond: XorQuantity,
    outstanding: XorQuantity,
    micropayment_credit_carry: XorQuantity,
    total_expected_charge: XorQuantity,
    total_micropayment_credit: XorQuantity,
    total_client_debit: XorQuantity,
    total_bond_slash: XorQuantity,
    window_expected_charge: XorQuantity,
    window_micropayment_credit_applied: XorQuantity,
    window_storage_gib_hours: u128,
    window_egress_bytes: u128,
    total_storage_gib_hours: u128,
    total_egress_bytes: u128,
    settlement_count: u64,
    last_settlement_epoch: u64,
    seen_tickets: HashSet<[u8; 32]>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct DealProviderCheckpointV1 {
    provider_id: ProviderId,
    account: ProviderAccount,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct DealClientCheckpointV1 {
    client_id: ClientId,
    account: ClientAccount,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct DealStateCheckpointV1 {
    record: DealRecord,
    locked_bond: XorQuantity,
    outstanding: XorQuantity,
    micropayment_credit_carry: XorQuantity,
    total_expected_charge: XorQuantity,
    total_micropayment_credit: XorQuantity,
    total_client_debit: XorQuantity,
    total_bond_slash: XorQuantity,
    window_expected_charge: XorQuantity,
    window_micropayment_credit_applied: XorQuantity,
    window_storage_gib_hours: u128,
    window_egress_bytes: u128,
    total_storage_gib_hours: u128,
    total_egress_bytes: u128,
    settlement_count: u64,
    last_settlement_epoch: u64,
    seen_tickets: Vec<[u8; 32]>,
}

/// Canonical restart snapshot for the embedded deal engine.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct DealRuntimeCheckpointV1 {
    providers: Vec<DealProviderCheckpointV1>,
    clients: Vec<DealClientCheckpointV1>,
    deals: Vec<DealStateCheckpointV1>,
}

impl DealState {
    fn new(record: DealRecord, locked_bond: XorQuantity, activation_epoch: u64) -> Self {
        Self {
            record,
            locked_bond,
            outstanding: XorQuantity::zero(),
            micropayment_credit_carry: XorQuantity::zero(),
            total_expected_charge: XorQuantity::zero(),
            total_micropayment_credit: XorQuantity::zero(),
            total_client_debit: XorQuantity::zero(),
            total_bond_slash: XorQuantity::zero(),
            window_expected_charge: XorQuantity::zero(),
            window_micropayment_credit_applied: XorQuantity::zero(),
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
    /// Zero-value deposits are not admitted because they consume state without funding it.
    #[error("deal engine deposits must be greater than zero")]
    ZeroDeposit,
    /// A configured authoritative-state ceiling was reached.
    #[error("deal engine resource `{resource}` exhausted (limit {limit})")]
    ResourceExhausted {
        /// Bounded resource label.
        resource: &'static str,
        /// Configured entry ceiling.
        limit: usize,
    },
    /// Account arithmetic would exceed the bounded exact quantity domain.
    #[error("deal engine balance overflow for `{resource}`")]
    BalanceOverflow {
        /// Account field that overflowed.
        resource: &'static str,
    },
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
        /// Exact XOR-denominated bond required for the deal.
        required: XorQuantity,
        /// Exact XOR-denominated bond currently available.
        available: XorQuantity,
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
    /// A durable checkpoint failed validation while loading or rolling back.
    #[error("invalid deal runtime checkpoint: {0}")]
    InvalidCheckpoint(String),
    /// A durable checkpoint could not be committed.
    #[error("deal runtime checkpoint failed: {0}")]
    Checkpoint(String),
    /// The in-memory deal engine lock was poisoned.
    #[error("deal engine state lock poisoned")]
    StateLockPoisoned,
}

/// Snapshot describing a provider account.
#[derive(Debug, Clone)]
pub struct ProviderSnapshot {
    /// Bond not currently locked by deals.
    pub bond_available: XorQuantity,
    /// Bond locked against active deals.
    pub bond_locked: XorQuantity,
    /// Earnings accrued from client settlements and micropayments.
    pub earnings: XorQuantity,
}

/// Snapshot describing a client account.
#[derive(Debug, Clone)]
pub struct ClientSnapshot {
    /// Credit balance available for settlements.
    pub credit_balance: XorQuantity,
}

/// Snapshot describing deal-level accounting.
#[derive(Debug, Clone)]
pub struct DealSnapshot {
    /// Deal identifier.
    pub deal_id: DealId,
    /// Current lifecycle status.
    pub status: DealStatus,
    /// Outstanding balance after applying micropayments and settlements.
    pub outstanding: XorQuantity,
    /// Micropayment credit held for future windows.
    pub credit_carry: XorQuantity,
    /// Bond reserved for the deal.
    pub locked_bond: XorQuantity,
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
    pub deterministic_charge: XorQuantity,
    /// Micropayment credit generated during this sample.
    pub micropayment_credit_generated: XorQuantity,
    /// Micropayment credit applied immediately against the charge.
    pub micropayment_credit_applied: XorQuantity,
    /// Micropayment credit carried forward to future windows.
    pub micropayment_credit_carry: XorQuantity,
    /// Outstanding balance after applying credit.
    pub outstanding: XorQuantity,
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
    entry_limit: usize,
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
        Self::with_entry_limit(65_536)
    }

    /// Construct a deal engine with a ceiling for each authoritative index and for the global
    /// replay-protection ticket set.
    #[must_use]
    pub fn with_entry_limit(entry_limit: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner::default())),
            entry_limit: entry_limit.max(1),
        }
    }

    /// Register or top up a provider bond.
    pub(crate) fn deposit_provider_bond(
        &self,
        provider_id: ProviderId,
        amount: XorQuantity,
    ) -> Result<ProviderSnapshot, DealEngineError> {
        if amount.is_zero() {
            return Err(DealEngineError::ZeroDeposit);
        }
        let mut inner = self
            .inner
            .write()
            .map_err(|_| DealEngineError::StateLockPoisoned)?;
        if !inner.providers.contains_key(&provider_id) && inner.providers.len() >= self.entry_limit
        {
            return Err(DealEngineError::ResourceExhausted {
                resource: "providers",
                limit: self.entry_limit,
            });
        }
        let account = inner.providers.entry(provider_id).or_default();
        account.bond_available =
            checked_deal_add(&account.bond_available, &amount, "provider_bond_available")?;
        Ok(ProviderSnapshot {
            bond_available: account.bond_available.clone(),
            bond_locked: account.bond_locked.clone(),
            earnings: account.earnings.clone(),
        })
    }

    /// Register or top up a client credit balance.
    pub(crate) fn deposit_client_credit(
        &self,
        client_id: ClientId,
        amount: XorQuantity,
    ) -> Result<ClientSnapshot, DealEngineError> {
        if amount.is_zero() {
            return Err(DealEngineError::ZeroDeposit);
        }
        let mut inner = self
            .inner
            .write()
            .map_err(|_| DealEngineError::StateLockPoisoned)?;
        if !inner.clients.contains_key(&client_id) && inner.clients.len() >= self.entry_limit {
            return Err(DealEngineError::ResourceExhausted {
                resource: "clients",
                limit: self.entry_limit,
            });
        }
        let account = inner.clients.entry(client_id).or_default();
        account.credit_balance =
            checked_deal_add(&account.credit_balance, &amount, "client_credit_balance")?;
        Ok(ClientSnapshot {
            credit_balance: account.credit_balance.clone(),
        })
    }

    /// Open a deal by locking collateral and tracking its lifecycle.
    pub(crate) fn open_deal(
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

        let mut inner = self
            .inner
            .write()
            .map_err(|_| DealEngineError::StateLockPoisoned)?;
        let inner_ref = &mut *inner;
        let deals = &mut inner_ref.deals;
        let providers = &mut inner_ref.providers;
        let clients = &inner_ref.clients;

        if deals.contains_key(&deal_id) {
            return Err(DealEngineError::DuplicateDeal(deal_id));
        }
        if deals.len() >= self.entry_limit {
            return Err(DealEngineError::ResourceExhausted {
                resource: "deals",
                limit: self.entry_limit,
            });
        }

        if !clients.contains_key(&proposal.client_id) {
            return Err(DealEngineError::UnknownClient(proposal.client_id));
        }

        let provider = providers
            .get_mut(&proposal.provider_id)
            .ok_or(DealEngineError::UnknownProvider(proposal.provider_id))?;

        let bond_required = proposal
            .terms
            .bond_requirement(proposal.capacity_gib)
            .map(XorQuantity::from_quantity)
            .map_err(|_| DealEngineError::BalanceOverflow {
                resource: "deal_bond_requirement",
            })?;
        if provider.bond_available < bond_required {
            return Err(DealEngineError::InsufficientBond {
                provider: proposal.provider_id,
                required: bond_required,
                available: provider.bond_available.clone(),
            });
        }

        let bond_locked = checked_deal_add(
            &provider.bond_locked,
            &bond_required,
            "provider_bond_locked",
        )?;
        provider.bond_available = provider
            .bond_available
            .checked_sub(&bond_required)
            .map_err(|_| {
                DealEngineError::InvalidCheckpoint(
                    "provider available bond fell below the admitted requirement".to_owned(),
                )
            })?;
        provider.bond_locked = bond_locked;

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
    pub(crate) fn record_usage(
        &self,
        report: DealUsageReport,
    ) -> Result<UsageOutcome, DealEngineError> {
        if report.tickets.len() > self.entry_limit {
            return Err(DealEngineError::ResourceExhausted {
                resource: "tickets_per_usage_report",
                limit: self.entry_limit,
            });
        }
        let mut inner = self
            .inner
            .write()
            .map_err(|_| DealEngineError::StateLockPoisoned)?;
        let entry = inner
            .deals
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

        if !inner.clients.contains_key(&client_id) {
            return Err(DealEngineError::UnknownClient(client_id));
        }

        let remaining_tickets = self.entry_limit.saturating_sub(inner.seen_ticket_count);
        let mut novel_tickets = HashSet::with_capacity(remaining_tickets.min(report.tickets.len()));
        for ticket in &report.tickets {
            let ticket_bytes = *ticket.ticket_id.as_bytes();
            if !entry.seen_tickets.contains(&ticket_bytes)
                && novel_tickets.insert(ticket_bytes)
                && novel_tickets.len() > remaining_tickets
            {
                return Err(DealEngineError::ResourceExhausted {
                    resource: "seen_tickets",
                    limit: self.entry_limit,
                });
            }
        }
        let new_ticket_count = novel_tickets.len();
        let mut state = entry.clone();

        let storage_charge = storage_charge(report.storage_gib_hours as u128, &state.record.terms)?;
        let egress_charge = egress_charge(report.egress_bytes as u128, &state.record.terms)?;
        let sample_charge =
            checked_deal_add(&storage_charge, &egress_charge, "usage_sample_charge")?;

        state.window_storage_gib_hours = checked_counter_add(
            state.window_storage_gib_hours,
            report.storage_gib_hours as u128,
            "window_storage_gib_hours",
        )?;
        state.window_egress_bytes = checked_counter_add(
            state.window_egress_bytes,
            report.egress_bytes as u128,
            "window_egress_bytes",
        )?;
        state.window_expected_charge = checked_deal_add(
            &state.window_expected_charge,
            &sample_charge,
            "window_expected_charge",
        )?;
        state.total_storage_gib_hours = checked_counter_add(
            state.total_storage_gib_hours,
            report.storage_gib_hours as u128,
            "total_storage_gib_hours",
        )?;
        state.total_egress_bytes = checked_counter_add(
            state.total_egress_bytes,
            report.egress_bytes as u128,
            "total_egress_bytes",
        )?;

        let mut tickets_processed = 0usize;
        let mut tickets_won = 0usize;
        let mut tickets_duplicate = 0usize;
        let mut new_credit = XorQuantity::zero();
        let mut generated_credit = XorQuantity::zero();

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
                let payout =
                    XorQuantity::from_quantity(state.record.terms.micropayment_payout.clone());
                new_credit = checked_deal_add(&new_credit, &payout, "usage_micropayment_credit")?;
                generated_credit =
                    checked_deal_add(&generated_credit, &payout, "usage_generated_credit")?;
            }
        }

        let mut due_remaining = sample_charge.clone();
        let provider_credit_total = new_credit.clone();

        let mut credit_applied = XorQuantity::zero();

        if !due_remaining.is_zero() && !state.micropayment_credit_carry.is_zero() {
            let applied = XorQuantity::min(&due_remaining, &state.micropayment_credit_carry);
            due_remaining = due_remaining.checked_sub(&applied).map_err(|_| {
                DealEngineError::InvalidCheckpoint(
                    "usage credit exceeded the remaining charge".to_owned(),
                )
            })?;
            state.micropayment_credit_carry = state
                .micropayment_credit_carry
                .checked_sub(&applied)
                .map_err(|_| {
                    DealEngineError::InvalidCheckpoint(
                        "usage credit exceeded the carried credit".to_owned(),
                    )
                })?;
            credit_applied = checked_deal_add(&credit_applied, &applied, "usage_credit_applied")?;
        }

        if !due_remaining.is_zero() && !new_credit.is_zero() {
            let applied = XorQuantity::min(&due_remaining, &new_credit);
            due_remaining = due_remaining.checked_sub(&applied).map_err(|_| {
                DealEngineError::InvalidCheckpoint(
                    "usage credit exceeded the remaining charge".to_owned(),
                )
            })?;
            new_credit = new_credit.checked_sub(&applied).map_err(|_| {
                DealEngineError::InvalidCheckpoint(
                    "usage credit exceeded the generated credit".to_owned(),
                )
            })?;
            credit_applied = checked_deal_add(&credit_applied, &applied, "usage_credit_applied")?;
        }

        state.micropayment_credit_carry = checked_deal_add(
            &state.micropayment_credit_carry,
            &new_credit,
            "micropayment_credit_carry",
        )?;
        state.window_micropayment_credit_applied = checked_deal_add(
            &state.window_micropayment_credit_applied,
            &credit_applied,
            "window_micropayment_credit_applied",
        )?;
        state.outstanding =
            checked_deal_add(&state.outstanding, &due_remaining, "deal_outstanding")?;

        let provider_earnings = inner
            .providers
            .get(&provider_id)
            .ok_or(DealEngineError::UnknownProvider(provider_id))?
            .earnings
            .clone();
        let provider_earnings = checked_deal_add(
            &provider_earnings,
            &provider_credit_total,
            "provider_earnings",
        )?;

        let outcome = UsageOutcome {
            deal_id: report.deal_id,
            provider_id: state.record.provider_id,
            client_id: state.record.client_id,
            deterministic_charge: sample_charge,
            micropayment_credit_generated: generated_credit,
            micropayment_credit_applied: credit_applied,
            micropayment_credit_carry: state.micropayment_credit_carry.clone(),
            outstanding: state.outstanding.clone(),
            tickets_processed,
            tickets_won,
            tickets_duplicate,
        };

        inner
            .providers
            .get_mut(&provider_id)
            .expect("provider checked above")
            .earnings = provider_earnings;
        inner.seen_ticket_count += new_ticket_count;
        inner.deals.insert(report.deal_id, state);

        Ok(outcome)
    }

    /// Settle the current window, withdrawing client credit and applying bond slashing if needed.
    pub(crate) fn settle(
        &self,
        deal_id: DealId,
        settlement_epoch: u64,
    ) -> Result<DealSettlementOutcome, DealEngineError> {
        let mut inner = self
            .inner
            .write()
            .map_err(|_| DealEngineError::StateLockPoisoned)?;
        let entry = inner
            .deals
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

        let mut client = inner
            .clients
            .get(&client_id)
            .cloned()
            .ok_or(DealEngineError::UnknownClient(client_id))?;
        let mut provider = inner
            .providers
            .get(&provider_id)
            .cloned()
            .ok_or(DealEngineError::UnknownProvider(provider_id))?;
        let mut state = entry.clone();

        let window_start = state.last_settlement_epoch;
        let expected_charge = state.window_expected_charge.clone();
        let credit_applied = state.window_micropayment_credit_applied.clone();
        let mut window_outstanding =
            expected_charge.checked_sub(&credit_applied).map_err(|_| {
                DealEngineError::InvalidCheckpoint(
                    "window micropayment credit exceeds expected charge".to_owned(),
                )
            })?;

        let previous_outstanding =
            state
                .outstanding
                .checked_sub(&window_outstanding)
                .map_err(|_| {
                    DealEngineError::InvalidCheckpoint(
                        "deal outstanding is below current window outstanding".to_owned(),
                    )
                })?;

        let mut client_debit = XorQuantity::zero();
        if !window_outstanding.is_zero() && !client.credit_balance.is_zero() {
            client_debit = XorQuantity::min(&window_outstanding, &client.credit_balance);
            client.credit_balance =
                client
                    .credit_balance
                    .checked_sub(&client_debit)
                    .map_err(|_| {
                        DealEngineError::InvalidCheckpoint(
                            "client debit exceeded the available balance".to_owned(),
                        )
                    })?;
            window_outstanding = window_outstanding.checked_sub(&client_debit).map_err(|_| {
                DealEngineError::InvalidCheckpoint(
                    "client debit exceeded the window balance".to_owned(),
                )
            })?;
        }

        let mut bond_slash = XorQuantity::zero();
        if !client_debit.is_zero() {
            provider.earnings =
                checked_deal_add(&provider.earnings, &client_debit, "provider_earnings")?;
        }
        if !window_outstanding.is_zero() && !state.locked_bond.is_zero() {
            bond_slash = XorQuantity::min(&window_outstanding, &state.locked_bond);
            state.locked_bond = state.locked_bond.checked_sub(&bond_slash).map_err(|_| {
                DealEngineError::InvalidCheckpoint("bond slash exceeded the deal bond".to_owned())
            })?;
            provider.bond_locked = provider.bond_locked.checked_sub(&bond_slash).map_err(|_| {
                DealEngineError::InvalidCheckpoint(
                    "provider locked bond is below deal bond".to_owned(),
                )
            })?;
            window_outstanding = window_outstanding.checked_sub(&bond_slash).map_err(|_| {
                DealEngineError::InvalidCheckpoint(
                    "bond slash exceeded the window balance".to_owned(),
                )
            })?;
        }

        state.outstanding = checked_deal_add(
            &previous_outstanding,
            &window_outstanding,
            "deal_outstanding",
        )?;
        state.settlement_count =
            state
                .settlement_count
                .checked_add(1)
                .ok_or(DealEngineError::BalanceOverflow {
                    resource: "settlement_count",
                })?;
        state.last_settlement_epoch = settlement_epoch;

        state.total_expected_charge = checked_deal_add(
            &state.total_expected_charge,
            &expected_charge,
            "total_expected_charge",
        )?;
        state.total_micropayment_credit = checked_deal_add(
            &state.total_micropayment_credit,
            &credit_applied,
            "total_micropayment_credit",
        )?;
        state.total_client_debit = checked_deal_add(
            &state.total_client_debit,
            &client_debit,
            "total_client_debit",
        )?;
        state.total_bond_slash =
            checked_deal_add(&state.total_bond_slash, &bond_slash, "total_bond_slash")?;

        if state.outstanding.is_zero()
            && state.micropayment_credit_carry.is_zero()
            && settlement_epoch >= state.record.end_epoch
        {
            let locked_bond = state.locked_bond.clone();
            if !locked_bond.is_zero() {
                provider.bond_available = checked_deal_add(
                    &provider.bond_available,
                    &locked_bond,
                    "provider_bond_available",
                )?;
                provider.bond_locked =
                    provider
                        .bond_locked
                        .checked_sub(&locked_bond)
                        .map_err(|_| {
                            DealEngineError::InvalidCheckpoint(
                                "provider locked bond is below released deal bond".to_owned(),
                            )
                        })?;
            }
            state.locked_bond = XorQuantity::zero();
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
            expected_charge: expected_charge.clone().into_quantity(),
            micropayment_credit: credit_applied.clone().into_quantity(),
            client_credit_debit: client_debit.clone().into_quantity(),
            bond_slash: bond_slash.clone().into_quantity(),
            outstanding: state.outstanding.clone().into_quantity(),
        };

        state.window_expected_charge = XorQuantity::zero();
        state.window_micropayment_credit_applied = XorQuantity::zero();
        state.window_storage_gib_hours = 0;
        state.window_egress_bytes = 0;

        let provider_accrual = checked_deal_add(
            &checked_deal_add(
                &state.total_client_debit,
                &state.total_micropayment_credit,
                "provider_accrual",
            )?,
            &state.total_bond_slash,
            "provider_accrual",
        )?;
        let ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            deal_id: *deal_id.as_bytes(),
            provider_id: *state.record.provider_id.as_bytes(),
            client_id: *state.record.client_id.as_bytes(),
            provider_accrual,
            client_liability: state.total_expected_charge.clone(),
            bond_locked: state.locked_bond.clone(),
            bond_slashed: state.total_bond_slash.clone(),
            captured_at: settlement_epoch,
        };
        let status = if !bond_slash.is_zero() {
            DealSettlementStatusV1::Slashed
        } else if matches!(state.record.status, DealStatus::Cancelled(_)) {
            DealSettlementStatusV1::Cancelled
        } else {
            DealSettlementStatusV1::Completed
        };
        let audit_notes = match status {
            DealSettlementStatusV1::Slashed => Some(format!(
                "bond slashed {} XOR (total {} XOR); outstanding {} XOR",
                bond_slash, state.total_bond_slash, state.outstanding
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

        inner.clients.insert(client_id, client);
        inner.providers.insert(provider_id, provider);
        inner.deals.insert(deal_id, state);

        Ok(outcome)
    }

    /// Obtain a snapshot of the provider account.
    pub fn provider_snapshot(&self, provider_id: ProviderId) -> Option<ProviderSnapshot> {
        let inner = self.inner.read().expect("deal engine poisoned");
        inner
            .providers
            .get(&provider_id)
            .map(|account| ProviderSnapshot {
                bond_available: account.bond_available.clone(),
                bond_locked: account.bond_locked.clone(),
                earnings: account.earnings.clone(),
            })
    }

    /// Obtain a snapshot of the client account.
    pub fn client_snapshot(&self, client_id: ClientId) -> Option<ClientSnapshot> {
        let inner = self.inner.read().expect("deal engine poisoned");
        inner.clients.get(&client_id).map(|account| ClientSnapshot {
            credit_balance: account.credit_balance.clone(),
        })
    }

    /// Obtain a snapshot of the current deal state.
    pub fn deal_snapshot(&self, deal_id: DealId) -> Option<DealSnapshot> {
        let inner = self.inner.read().expect("deal engine poisoned");
        inner.deals.get(&deal_id).map(|state| DealSnapshot {
            deal_id,
            status: state.record.status,
            outstanding: state.outstanding.clone(),
            credit_carry: state.micropayment_credit_carry.clone(),
            locked_bond: state.locked_bond.clone(),
            settlement_count: state.settlement_count,
        })
    }

    /// Export a canonical restart snapshot of all authoritative accounting state.
    pub(crate) fn checkpoint(&self) -> Result<DealRuntimeCheckpointV1, DealEngineError> {
        let inner = self
            .inner
            .read()
            .map_err(|_| DealEngineError::StateLockPoisoned)?;
        let mut providers = inner
            .providers
            .iter()
            .map(|(provider_id, account)| DealProviderCheckpointV1 {
                provider_id: *provider_id,
                account: account.clone(),
            })
            .collect::<Vec<_>>();
        providers.sort_by_key(|entry| entry.provider_id);
        let mut clients = inner
            .clients
            .iter()
            .map(|(client_id, account)| DealClientCheckpointV1 {
                client_id: *client_id,
                account: account.clone(),
            })
            .collect::<Vec<_>>();
        clients.sort_by_key(|entry| entry.client_id);
        let mut deals = inner
            .deals
            .values()
            .map(|state| {
                let mut seen_tickets = state.seen_tickets.iter().copied().collect::<Vec<_>>();
                seen_tickets.sort_unstable();
                DealStateCheckpointV1 {
                    record: state.record.clone(),
                    locked_bond: state.locked_bond.clone(),
                    outstanding: state.outstanding.clone(),
                    micropayment_credit_carry: state.micropayment_credit_carry.clone(),
                    total_expected_charge: state.total_expected_charge.clone(),
                    total_micropayment_credit: state.total_micropayment_credit.clone(),
                    total_client_debit: state.total_client_debit.clone(),
                    total_bond_slash: state.total_bond_slash.clone(),
                    window_expected_charge: state.window_expected_charge.clone(),
                    window_micropayment_credit_applied: state
                        .window_micropayment_credit_applied
                        .clone(),
                    window_storage_gib_hours: state.window_storage_gib_hours,
                    window_egress_bytes: state.window_egress_bytes,
                    total_storage_gib_hours: state.total_storage_gib_hours,
                    total_egress_bytes: state.total_egress_bytes,
                    settlement_count: state.settlement_count,
                    last_settlement_epoch: state.last_settlement_epoch,
                    seen_tickets,
                }
            })
            .collect::<Vec<_>>();
        deals.sort_by_key(|entry| entry.record.deal_id);
        Ok(DealRuntimeCheckpointV1 {
            providers,
            clients,
            deals,
        })
    }

    /// Restore a canonical checkpoint after validating indexes, replay protection, and bond
    /// accounting invariants.
    pub(crate) fn restore_checkpoint(
        &self,
        checkpoint: DealRuntimeCheckpointV1,
    ) -> Result<(), DealEngineError> {
        for (resource, count) in [
            ("providers", checkpoint.providers.len()),
            ("clients", checkpoint.clients.len()),
            ("deals", checkpoint.deals.len()),
        ] {
            if count > self.entry_limit {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "{resource} count {count} exceeds configured limit {}",
                    self.entry_limit
                )));
            }
        }

        let mut providers = HashMap::with_capacity(checkpoint.providers.len());
        let mut previous_provider = None;
        for entry in checkpoint.providers {
            if previous_provider.is_some_and(|previous| previous >= entry.provider_id) {
                return Err(DealEngineError::InvalidCheckpoint(
                    "provider checkpoint keys are not strictly sorted".to_owned(),
                ));
            }
            previous_provider = Some(entry.provider_id);
            providers.insert(entry.provider_id, entry.account);
        }
        let mut clients = HashMap::with_capacity(checkpoint.clients.len());
        let mut previous_client = None;
        for entry in checkpoint.clients {
            if previous_client.is_some_and(|previous| previous >= entry.client_id) {
                return Err(DealEngineError::InvalidCheckpoint(
                    "client checkpoint keys are not strictly sorted".to_owned(),
                ));
            }
            previous_client = Some(entry.client_id);
            clients.insert(entry.client_id, entry.account);
        }

        let mut deals = HashMap::with_capacity(checkpoint.deals.len());
        let mut previous_deal = None;
        let mut seen_ticket_count = 0usize;
        let mut locked_bond_by_provider = HashMap::<ProviderId, XorQuantity>::new();
        for entry in checkpoint.deals {
            let deal_id = entry.record.deal_id;
            if previous_deal.is_some_and(|previous| previous >= deal_id) {
                return Err(DealEngineError::InvalidCheckpoint(
                    "deal checkpoint keys are not strictly sorted".to_owned(),
                ));
            }
            previous_deal = Some(deal_id);
            if entry.record.start_epoch > entry.record.end_epoch {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "deal {} has an inverted epoch window",
                    hex::encode(deal_id.as_bytes())
                )));
            }
            if !providers.contains_key(&entry.record.provider_id)
                || !clients.contains_key(&entry.record.client_id)
            {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "deal {} references a missing provider or client",
                    hex::encode(deal_id.as_bytes())
                )));
            }
            if let DealStatus::Active(activation_epoch) = entry.record.status
                && (activation_epoch < entry.record.start_epoch
                    || activation_epoch > entry.record.end_epoch)
            {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "deal {} has an invalid activation epoch",
                    hex::encode(deal_id.as_bytes())
                )));
            }
            if matches!(entry.record.status, DealStatus::Settled(_))
                && (!entry.locked_bond.is_zero()
                    || !entry.outstanding.is_zero()
                    || !entry.micropayment_credit_carry.is_zero())
            {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "settled deal {} retains bond, debt, or credit",
                    hex::encode(deal_id.as_bytes())
                )));
            }
            if entry.window_micropayment_credit_applied > entry.window_expected_charge
                || entry.total_micropayment_credit > entry.total_expected_charge
            {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "deal {} applies more micropayment credit than expected charges",
                    hex::encode(deal_id.as_bytes())
                )));
            }
            if !entry.seen_tickets.windows(2).all(|pair| pair[0] < pair[1]) {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "deal {} ticket ids are duplicated or not canonical",
                    hex::encode(deal_id.as_bytes())
                )));
            }
            seen_ticket_count = seen_ticket_count
                .checked_add(entry.seen_tickets.len())
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint(
                        "deal ticket checkpoint count overflow".to_owned(),
                    )
                })?;
            if seen_ticket_count > self.entry_limit {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "seen ticket count {seen_ticket_count} exceeds configured limit {}",
                    self.entry_limit
                )));
            }
            let locked_bond = locked_bond_by_provider
                .entry(entry.record.provider_id)
                .or_default();
            *locked_bond = locked_bond.checked_add(&entry.locked_bond).map_err(|_| {
                DealEngineError::InvalidCheckpoint(
                    "provider locked-bond checkpoint overflow".to_owned(),
                )
            })?;
            let state = DealState {
                record: entry.record,
                locked_bond: entry.locked_bond,
                outstanding: entry.outstanding,
                micropayment_credit_carry: entry.micropayment_credit_carry,
                total_expected_charge: entry.total_expected_charge,
                total_micropayment_credit: entry.total_micropayment_credit,
                total_client_debit: entry.total_client_debit,
                total_bond_slash: entry.total_bond_slash,
                window_expected_charge: entry.window_expected_charge,
                window_micropayment_credit_applied: entry.window_micropayment_credit_applied,
                window_storage_gib_hours: entry.window_storage_gib_hours,
                window_egress_bytes: entry.window_egress_bytes,
                total_storage_gib_hours: entry.total_storage_gib_hours,
                total_egress_bytes: entry.total_egress_bytes,
                settlement_count: entry.settlement_count,
                last_settlement_epoch: entry.last_settlement_epoch,
                seen_tickets: entry.seen_tickets.into_iter().collect(),
            };
            deals.insert(deal_id, state);
        }
        for (provider_id, account) in &providers {
            let expected_locked = locked_bond_by_provider
                .get(provider_id)
                .cloned()
                .unwrap_or_default();
            if account.bond_locked != expected_locked {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "provider {} locked bond disagrees with deal state",
                    hex::encode(provider_id.as_bytes())
                )));
            }
        }

        let mut inner = self
            .inner
            .write()
            .map_err(|_| DealEngineError::StateLockPoisoned)?;
        *inner = Inner {
            providers,
            clients,
            deals,
            seen_ticket_count,
        };
        Ok(())
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
    hasher.update(&norito::to_bytes(&proposal.terms)?);
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

fn storage_charge(gib_hours: u128, terms: &DealTerms) -> Result<XorQuantity, DealEngineError> {
    terms
        .storage_charge(gib_hours)
        .map(XorQuantity::from_quantity)
        .map_err(|_| DealEngineError::BalanceOverflow {
            resource: "storage_charge",
        })
}

fn egress_charge(bytes: u128, terms: &DealTerms) -> Result<XorQuantity, DealEngineError> {
    terms
        .egress_charge(bytes)
        .map(XorQuantity::from_quantity)
        .map_err(|_| DealEngineError::BalanceOverflow {
            resource: "egress_charge",
        })
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{
        metadata::Metadata,
        prelude::{Numeric, Quantity},
        sorafs::deal::MicropaymentTicket,
    };
    use sorafs_manifest::deal::DealSettlementStatusV1;

    use super::*;

    fn sample_terms() -> DealTerms {
        DealTerms {
            storage_price_per_gib_month: quantity("0.5"),
            egress_price_per_gib: quantity("0.05"),
            settlement_window_epochs: 7,
            micropayment_probability_bps: BASIS_POINTS_SCALE as u16,
            micropayment_payout: quantity("0.1"),
        }
    }

    fn xor(value: &str) -> XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }

    fn quantity(value: &str) -> Quantity {
        value.parse().expect("canonical quantity")
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

        engine
            .deposit_provider_bond(provider, xor("1"))
            .expect("deposit provider bond");
        engine
            .deposit_client_credit(client, xor("1"))
            .expect("deposit client credit");

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

        engine
            .deposit_provider_bond(provider, xor("15"))
            .expect("deposit provider bond");
        engine
            .deposit_client_credit(client, xor("3"))
            .expect("deposit client credit");

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

        assert_eq!(settlement.expected_charge, quantity("2.55"));
        assert_eq!(settlement.micropayment_credit, quantity("0.5"));
        assert_eq!(settlement.client_credit_debit, quantity("2.05"));
        assert_eq!(settlement.bond_slash, Quantity::zero());
        assert_eq!(settlement.outstanding, Quantity::zero());

        let governance = &settlement_outcome.governance;
        assert_eq!(governance.status, DealSettlementStatusV1::Completed);
        assert_eq!(
            governance
                .ledger
                .provider_accrual
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            2_550_000
        );

        let provider_snapshot = engine.provider_snapshot(provider).expect("provider");
        assert_eq!(provider_snapshot.bond_available, xor("15"));
        assert_eq!(provider_snapshot.bond_locked, XorQuantity::zero());
        assert_eq!(provider_snapshot.earnings, xor("2.55"));

        let deal_snapshot = engine.deal_snapshot(record.deal_id).expect("deal snapshot");
        matches!(deal_snapshot.status, DealStatus::Settled(17));
        assert_eq!(deal_snapshot.outstanding, XorQuantity::zero());
        assert_eq!(deal_snapshot.locked_bond, XorQuantity::zero());
    }

    #[test]
    fn usage_outcome_reports_micropayment_metrics() {
        let engine = DealEngine::new();
        let provider = provider(3);
        let client = client(4);

        engine
            .deposit_provider_bond(provider, xor("15"))
            .expect("deposit provider bond");
        engine
            .deposit_client_credit(client, xor("5"))
            .expect("deposit client credit");

        let terms = sample_terms();
        let expected_charge = XorQuantity::from_quantity(
            terms
                .storage_price_per_gib_month
                .checked_add(&terms.egress_price_per_gib)
                .expect("fixture charge is representable"),
        );
        let expected_credit = XorQuantity::from_quantity(
            terms
                .micropayment_payout
                .try_mul_decimal(&Numeric::from(2_u32))
                .expect("fixture credit is representable"),
        );
        let expected_outstanding = expected_charge
            .checked_sub(&expected_credit)
            .expect("fixture credit does not exceed its charge");

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
        assert_eq!(outcome.deterministic_charge, expected_charge);
        assert_eq!(outcome.micropayment_credit_generated, expected_credit);
        assert_eq!(outcome.micropayment_credit_applied, expected_credit);
        assert_eq!(outcome.micropayment_credit_carry, XorQuantity::zero());
        assert_eq!(outcome.outstanding, expected_outstanding);
        assert_eq!(outcome.tickets_processed, 2);
        assert_eq!(outcome.tickets_won, 2);
        assert_eq!(outcome.tickets_duplicate, 0);
    }

    #[test]
    fn duplicate_ticket_is_ignored() {
        let engine = DealEngine::new();
        let provider = provider(7);
        let client = client(8);

        engine
            .deposit_provider_bond(provider, xor("15"))
            .expect("deposit provider bond");
        engine
            .deposit_client_credit(client, xor("3"))
            .expect("deposit client credit");

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

    #[test]
    fn configured_limits_refuse_accounts_deals_and_replay_tickets() {
        let account_limited = DealEngine::with_entry_limit(1);
        account_limited
            .deposit_provider_bond(provider(1), xor("0.000000001"))
            .expect("first provider");
        assert!(matches!(
            account_limited
                .deposit_provider_bond(provider(2), xor("0.000000001"))
                .expect_err("second provider must be refused"),
            DealEngineError::ResourceExhausted {
                resource: "providers",
                limit: 1
            }
        ));
        account_limited
            .deposit_client_credit(client(1), xor("0.000000001"))
            .expect("first client");
        assert!(matches!(
            account_limited
                .deposit_client_credit(client(2), xor("0.000000001"))
                .expect_err("second client must be refused"),
            DealEngineError::ResourceExhausted {
                resource: "clients",
                limit: 1
            }
        ));

        let deal_limited = DealEngine::with_entry_limit(1);
        let provider_id = provider(3);
        let client_id = client(4);
        deal_limited
            .deposit_provider_bond(provider_id, xor("10"))
            .expect("provider deposit");
        deal_limited
            .deposit_client_credit(client_id, xor("0.000000001"))
            .expect("client deposit");
        let proposal = DealProposal {
            provider_id,
            client_id,
            storage_class: StorageClass::Hot,
            capacity_gib: 1,
            start_epoch: 10,
            end_epoch: 20,
            terms: sample_terms(),
            metadata: Metadata::default(),
        };
        deal_limited
            .open_deal(proposal.clone(), 10)
            .expect("first deal");
        assert!(matches!(
            deal_limited
                .open_deal(
                    DealProposal {
                        capacity_gib: 2,
                        ..proposal
                    },
                    10,
                )
                .expect_err("second deal must be refused"),
            DealEngineError::ResourceExhausted {
                resource: "deals",
                limit: 1
            }
        ));

        let ticket_limited = DealEngine::with_entry_limit(2);
        ticket_limited
            .deposit_provider_bond(provider_id, xor("10"))
            .expect("provider deposit");
        ticket_limited
            .deposit_client_credit(client_id, xor("0.000000001"))
            .expect("client deposit");
        let record = ticket_limited
            .open_deal(
                DealProposal {
                    provider_id,
                    client_id,
                    storage_class: StorageClass::Hot,
                    capacity_gib: 1,
                    start_epoch: 10,
                    end_epoch: 20,
                    terms: sample_terms(),
                    metadata: Metadata::default(),
                },
                10,
            )
            .expect("open deal");
        let report = |tickets| DealUsageReport {
            deal_id: record.deal_id,
            epoch: 11,
            storage_gib_hours: 0,
            egress_bytes: 0,
            tickets,
        };
        assert!(matches!(
            ticket_limited
                .record_usage(report(vec![ticket(1), ticket(2), ticket(3)]))
                .expect_err("oversized ticket batch must be refused"),
            DealEngineError::ResourceExhausted {
                resource: "tickets_per_usage_report",
                limit: 2
            }
        ));
        ticket_limited
            .record_usage(report(vec![ticket(1), ticket(2)]))
            .expect("fill replay set");
        assert!(matches!(
            ticket_limited
                .record_usage(report(vec![ticket(3)]))
                .expect_err("replay set exhaustion must be refused"),
            DealEngineError::ResourceExhausted {
                resource: "seen_tickets",
                limit: 2
            }
        ));
    }

    #[test]
    fn checkpoint_roundtrip_preserves_replay_protection_and_rejects_forgery() {
        let engine = DealEngine::with_entry_limit(8);
        let provider_id = provider(5);
        let client_id = client(6);
        engine
            .deposit_provider_bond(provider_id, xor("10"))
            .expect("provider deposit");
        engine
            .deposit_client_credit(client_id, xor("1"))
            .expect("client deposit");
        let record = engine
            .open_deal(
                DealProposal {
                    provider_id,
                    client_id,
                    storage_class: StorageClass::Hot,
                    capacity_gib: 1,
                    start_epoch: 10,
                    end_epoch: 20,
                    terms: sample_terms(),
                    metadata: Metadata::default(),
                },
                10,
            )
            .expect("open deal");
        engine
            .record_usage(DealUsageReport {
                deal_id: record.deal_id,
                epoch: 11,
                storage_gib_hours: 0,
                egress_bytes: 0,
                tickets: vec![ticket(7)],
            })
            .expect("record ticket");

        let checkpoint = engine.checkpoint().expect("checkpoint");
        let expected = norito::to_bytes(&checkpoint).expect("encode checkpoint");
        let restored = DealEngine::with_entry_limit(8);
        restored
            .restore_checkpoint(checkpoint.clone())
            .expect("restore checkpoint");
        assert_eq!(
            norito::to_bytes(&restored.checkpoint().expect("restored checkpoint"))
                .expect("encode restored checkpoint"),
            expected
        );
        let replay = restored
            .record_usage(DealUsageReport {
                deal_id: record.deal_id,
                epoch: 11,
                storage_gib_hours: 0,
                egress_bytes: 0,
                tickets: vec![ticket(7)],
            })
            .expect("replay ticket");
        assert_eq!(replay.tickets_duplicate, 1);

        let mut duplicate_ticket = checkpoint.clone();
        let duplicate_id = duplicate_ticket.deals[0].seen_tickets[0];
        duplicate_ticket.deals[0].seen_tickets.push(duplicate_id);
        assert!(matches!(
            DealEngine::with_entry_limit(8)
                .restore_checkpoint(duplicate_ticket)
                .expect_err("duplicate replay id must fail"),
            DealEngineError::InvalidCheckpoint(_)
        ));

        let mut forged_bond = checkpoint;
        forged_bond.providers[0].account.bond_locked = 0;
        assert!(matches!(
            DealEngine::with_entry_limit(8)
                .restore_checkpoint(forged_bond)
                .expect_err("forged provider bond must fail"),
            DealEngineError::InvalidCheckpoint(_)
        ));
    }
}
