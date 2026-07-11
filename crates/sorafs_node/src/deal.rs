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
        DealStatus, DealTerms, DealUsageReport, GIB_HOURS_PER_MONTH, MAX_DEAL_USAGE_TICKETS,
        TicketId,
    },
    pin_registry::StorageClass,
};
use norito::{
    Error as NoritoError,
    derive::{NoritoDeserialize, NoritoSerialize},
};
use sorafs_manifest::deal::{
    DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1,
    DealSettlementStatusV1, DealSettlementV1, MAX_DEAL_SETTLEMENT_AUDIT_NOTES_BYTES,
};
use thiserror::Error;

const DEAL_ID_DOMAIN: &[u8] = b"sorafs.deal.id.v1";
const MICROPAYMENT_DOMAIN: &[u8] = b"sorafs.ticket.draw.v1";
const BASIS_POINTS_SCALE: u64 = 10_000;
const MAX_DEAL_METADATA_ENCODED_BYTES: usize = 64 * 1024;

fn checked_deal_add(
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
    funding_sequence: u64,
    bond_deposited_nano: u128,
    bond_available_nano: u128,
    bond_locked_nano: u128,
    bond_slashed_nano: u128,
    earnings_nano: u128,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ClientAccount {
    funding_sequence: u64,
    credit_deposited_nano: u128,
    credit_balance_nano: u128,
    credit_debited_nano: u128,
}

#[derive(Debug, Clone)]
struct DealState {
    record: DealRecord,
    terms_digest: [u8; 32],
    initial_bond_nano: u128,
    locked_bond_nano: u128,
    bond_released_nano: u128,
    outstanding_nano: u128,
    micropayment_credit_carry: u128,
    total_expected_charge_nano: u128,
    total_micropayment_generated_nano: u128,
    total_micropayment_credit_nano: u128,
    total_client_debit_nano: u128,
    total_bond_slash_nano: u128,
    window_expected_charge_nano: u128,
    window_micropayment_generated_nano: u128,
    window_micropayment_credit_applied: u128,
    window_storage_gib_hours: u128,
    window_egress_bytes: u128,
    total_storage_gib_hours: u128,
    total_egress_bytes: u128,
    settlement_count: u64,
    last_settlement_epoch: u64,
    last_usage_epoch: Option<u64>,
    settlement_head: Option<DealSettlementV1>,
    seen_tickets: HashMap<[u8; 32], iroha_data_model::sorafs::deal::MicropaymentTicket>,
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
    terms_digest: [u8; 32],
    initial_bond_nano: u128,
    locked_bond_nano: u128,
    bond_released_nano: u128,
    outstanding_nano: u128,
    micropayment_credit_carry: u128,
    total_expected_charge_nano: u128,
    total_micropayment_generated_nano: u128,
    total_micropayment_credit_nano: u128,
    total_client_debit_nano: u128,
    total_bond_slash_nano: u128,
    window_expected_charge_nano: u128,
    window_micropayment_generated_nano: u128,
    window_micropayment_credit_applied: u128,
    window_storage_gib_hours: u128,
    window_egress_bytes: u128,
    total_storage_gib_hours: u128,
    total_egress_bytes: u128,
    settlement_count: u64,
    last_settlement_epoch: u64,
    last_usage_epoch: Option<u64>,
    settlement_head: Option<DealSettlementV1>,
    seen_tickets: Vec<iroha_data_model::sorafs::deal::MicropaymentTicket>,
}

/// Canonical restart snapshot for the embedded deal engine.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct DealRuntimeCheckpointV1 {
    providers: Vec<DealProviderCheckpointV1>,
    clients: Vec<DealClientCheckpointV1>,
    deals: Vec<DealStateCheckpointV1>,
}

impl DealState {
    fn new(
        record: DealRecord,
        terms_digest: [u8; 32],
        locked_bond_nano: u128,
        activation_epoch: u64,
    ) -> Self {
        Self {
            record,
            terms_digest,
            initial_bond_nano: locked_bond_nano,
            locked_bond_nano,
            bond_released_nano: 0,
            outstanding_nano: 0,
            micropayment_credit_carry: 0,
            total_expected_charge_nano: 0,
            total_micropayment_generated_nano: 0,
            total_micropayment_credit_nano: 0,
            total_client_debit_nano: 0,
            total_bond_slash_nano: 0,
            window_expected_charge_nano: 0,
            window_micropayment_generated_nano: 0,
            window_micropayment_credit_applied: 0,
            window_storage_gib_hours: 0,
            window_egress_bytes: 0,
            total_storage_gib_hours: 0,
            total_egress_bytes: 0,
            settlement_count: 0,
            last_settlement_epoch: activation_epoch,
            last_usage_epoch: None,
            settlement_head: None,
            seen_tickets: HashMap::new(),
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
    /// Account arithmetic would overflow its canonical nano-XOR representation.
    #[error("deal engine balance overflow for `{resource}`")]
    BalanceOverflow {
        /// Account field that overflowed.
        resource: &'static str,
    },
    /// A signed funding request was replayed, skipped, or forked.
    #[error(
        "{account_kind} funding sequence {found} does not equal the next expected sequence {expected}"
    )]
    FundingSequenceMismatch {
        /// Stable account-kind label.
        account_kind: &'static str,
        /// Required next sequence.
        expected: u64,
        /// Sequence supplied by the signed request.
        found: u64,
    },
    /// An account has consumed the complete funding sequence space.
    #[error("{account_kind} funding sequence overflow")]
    FundingSequenceOverflow {
        /// Stable account-kind label.
        account_kind: &'static str,
    },
    /// A proposal violates a deterministic first-release admission invariant.
    #[error("invalid deal proposal: {0}")]
    InvalidProposal(String),
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
    /// Usage reports must be strictly ordered within the current settlement window.
    #[error(
        "usage epoch {usage_epoch} is not strictly after prior usage/settlement epoch {previous_epoch} for {deal_id:?}"
    )]
    UsageEpochNotMonotonic {
        /// Deal identifier.
        deal_id: DealId,
        /// Replayed, stale, or out-of-order epoch.
        usage_epoch: u64,
        /// Most recent accepted usage or settlement epoch.
        previous_epoch: u64,
    },
    /// A ticket is not canonically bound to its report and deal.
    #[error("invalid micropayment ticket for {deal_id:?}: {reason}")]
    InvalidTicket {
        /// Deal identifier.
        deal_id: DealId,
        /// Stable validation reason.
        reason: &'static str,
    },
    /// A cancellation would strand accounting or violate canonical finality.
    #[error("deal {deal_id:?} cannot be cancelled: {reason}")]
    UnsafeCancellation {
        /// Deal targeted by the rejected cancellation.
        deal_id: DealId,
        /// Stable rejection reason.
        reason: &'static str,
    },
    /// Operator cancellation rationale is not canonical or exceeds its bound.
    #[error("invalid deal cancellation reason")]
    InvalidCancellationReason,
    /// A ticket identifier was already consumed or duplicated in the same report.
    #[error("micropayment ticket {ticket_id:?} was replayed for {deal_id:?}")]
    TicketReplay {
        /// Deal identifier.
        deal_id: DealId,
        /// Replayed ticket identifier.
        ticket_id: TicketId,
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
    /// Exact settlement epoch arithmetic overflowed.
    #[error("next settlement epoch overflow for {0:?}")]
    SettlementEpochOverflow(DealId),
    /// A bounded allocation failed before authoritative state mutation.
    #[error("deal engine failed to reserve memory for `{resource}`")]
    AllocationFailed {
        /// Bounded collection being allocated.
        resource: &'static str,
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
#[derive(Debug, Clone, Copy)]
pub struct ProviderSnapshot {
    /// Last accepted one-based funding sequence.
    pub funding_sequence: u64,
    /// Total collateral ever deposited into this engine account.
    pub bond_deposited_nano: u128,
    /// Bond not currently locked by deals.
    pub bond_available_nano: u128,
    /// Bond locked against active deals.
    pub bond_locked_nano: u128,
    /// Total collateral irreversibly slashed.
    pub bond_slashed_nano: u128,
    /// Earnings accrued from client settlements and micropayments.
    pub earnings_nano: u128,
}

/// Snapshot describing a client account.
#[derive(Debug, Clone, Copy)]
pub struct ClientSnapshot {
    /// Last accepted one-based funding sequence.
    pub funding_sequence: u64,
    /// Total client credit deposited into this engine account.
    pub credit_deposited_nano: u128,
    /// Credit balance available for settlements.
    pub credit_balance_nano: u128,
    /// Total client credit consumed by completed settlement windows.
    pub credit_debited_nano: u128,
}

/// Snapshot describing deal-level accounting.
#[derive(Debug, Clone)]
pub struct DealSnapshot {
    /// Deal identifier.
    pub deal_id: DealId,
    /// Provider bound by the canonical deal proposal.
    pub provider_id: ProviderId,
    /// Client bound by the canonical deal proposal.
    pub client_id: ClientId,
    /// Current lifecycle status.
    pub status: DealStatus,
    /// Outstanding balance after applying micropayments and settlements.
    pub outstanding_nano: u128,
    /// Micropayment credit held for future windows.
    pub credit_carry_nano: u128,
    /// Bond reserved for the deal.
    pub locked_bond_nano: u128,
    /// Immutable bond amount initially locked for this deal.
    pub initial_bond_nano: u128,
    /// Bond returned to the provider after finalisation.
    pub bond_released_nano: u128,
    /// Completed settlement windows.
    pub settlement_count: u64,
    /// Canonical head of the governance ledger chain.
    pub latest_ledger_snapshot_id: Option<[u8; 32]>,
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
    /// Duplicate counter (always zero because a duplicate rejects the complete report).
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
    #[cfg(test)]
    pub(crate) fn deposit_provider_bond(
        &self,
        provider_id: ProviderId,
        amount_nano: u128,
    ) -> Result<ProviderSnapshot, DealEngineError> {
        self.deposit_provider_bond_inner(provider_id, amount_nano, None)
    }

    /// Apply a provider funding request with an exact durable sequence.
    pub(crate) fn deposit_provider_bond_sequenced(
        &self,
        provider_id: ProviderId,
        amount_nano: u128,
        funding_sequence: u64,
    ) -> Result<ProviderSnapshot, DealEngineError> {
        self.deposit_provider_bond_inner(provider_id, amount_nano, Some(funding_sequence))
    }

    fn deposit_provider_bond_inner(
        &self,
        provider_id: ProviderId,
        amount_nano: u128,
        required_sequence: Option<u64>,
    ) -> Result<ProviderSnapshot, DealEngineError> {
        if amount_nano == 0 {
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
        if !inner.providers.contains_key(&provider_id) {
            inner
                .providers
                .try_reserve(1)
                .map_err(|_| DealEngineError::AllocationFailed {
                    resource: "providers",
                })?;
        }
        let mut account = inner
            .providers
            .get(&provider_id)
            .cloned()
            .unwrap_or_default();
        let next_sequence = account.funding_sequence.checked_add(1).ok_or(
            DealEngineError::FundingSequenceOverflow {
                account_kind: "provider",
            },
        )?;
        if let Some(found) = required_sequence {
            if found != next_sequence {
                return Err(DealEngineError::FundingSequenceMismatch {
                    account_kind: "provider",
                    expected: next_sequence,
                    found,
                });
            }
        }
        account.bond_deposited_nano = account.bond_deposited_nano.checked_add(amount_nano).ok_or(
            DealEngineError::BalanceOverflow {
                resource: "provider_bond_deposited",
            },
        )?;
        account.bond_available_nano = account.bond_available_nano.checked_add(amount_nano).ok_or(
            DealEngineError::BalanceOverflow {
                resource: "provider_bond_available",
            },
        )?;
        account.funding_sequence = next_sequence;
        inner.providers.insert(provider_id, account.clone());
        Ok(ProviderSnapshot {
            funding_sequence: account.funding_sequence,
            bond_deposited_nano: account.bond_deposited_nano,
            bond_available_nano: account.bond_available_nano,
            bond_locked_nano: account.bond_locked_nano,
            bond_slashed_nano: account.bond_slashed_nano,
            earnings_nano: account.earnings_nano,
        })
    }

    /// Register or top up a client credit balance.
    #[cfg(test)]
    pub(crate) fn deposit_client_credit(
        &self,
        client_id: ClientId,
        amount_nano: u128,
    ) -> Result<ClientSnapshot, DealEngineError> {
        self.deposit_client_credit_inner(client_id, amount_nano, None)
    }

    /// Apply a client funding request with an exact durable sequence.
    pub(crate) fn deposit_client_credit_sequenced(
        &self,
        client_id: ClientId,
        amount_nano: u128,
        funding_sequence: u64,
    ) -> Result<ClientSnapshot, DealEngineError> {
        self.deposit_client_credit_inner(client_id, amount_nano, Some(funding_sequence))
    }

    fn deposit_client_credit_inner(
        &self,
        client_id: ClientId,
        amount_nano: u128,
        required_sequence: Option<u64>,
    ) -> Result<ClientSnapshot, DealEngineError> {
        if amount_nano == 0 {
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
        if !inner.clients.contains_key(&client_id) {
            inner
                .clients
                .try_reserve(1)
                .map_err(|_| DealEngineError::AllocationFailed {
                    resource: "clients",
                })?;
        }
        let mut account = inner.clients.get(&client_id).cloned().unwrap_or_default();
        let next_sequence = account.funding_sequence.checked_add(1).ok_or(
            DealEngineError::FundingSequenceOverflow {
                account_kind: "client",
            },
        )?;
        if let Some(found) = required_sequence {
            if found != next_sequence {
                return Err(DealEngineError::FundingSequenceMismatch {
                    account_kind: "client",
                    expected: next_sequence,
                    found,
                });
            }
        }
        account.credit_deposited_nano = account
            .credit_deposited_nano
            .checked_add(amount_nano)
            .ok_or(DealEngineError::BalanceOverflow {
                resource: "client_credit_deposited",
            })?;
        account.credit_balance_nano = account.credit_balance_nano.checked_add(amount_nano).ok_or(
            DealEngineError::BalanceOverflow {
                resource: "client_credit_balance",
            },
        )?;
        account.funding_sequence = next_sequence;
        inner.clients.insert(client_id, account.clone());
        Ok(ClientSnapshot {
            funding_sequence: account.funding_sequence,
            credit_deposited_nano: account.credit_deposited_nano,
            credit_balance_nano: account.credit_balance_nano,
            credit_debited_nano: account.credit_debited_nano,
        })
    }

    /// Open a deal by locking collateral and tracking its lifecycle.
    pub(crate) fn open_deal(
        &self,
        proposal: DealProposal,
        activation_epoch: u64,
    ) -> Result<DealRecord, DealEngineError> {
        validate_deal_proposal(&proposal)?;
        let deal_id = compute_deal_id(&proposal)?;
        let terms_digest = compute_deal_terms_digest(&proposal)?;

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
        deals
            .try_reserve(1)
            .map_err(|_| DealEngineError::AllocationFailed { resource: "deals" })?;

        if !clients.contains_key(&proposal.client_id) {
            return Err(DealEngineError::UnknownClient(proposal.client_id));
        }

        let provider = providers
            .get_mut(&proposal.provider_id)
            .ok_or(DealEngineError::UnknownProvider(proposal.provider_id))?;

        let bond_required = checked_bond_requirement(&proposal.terms, proposal.capacity_gib)?;
        if provider.bond_available_nano < bond_required {
            return Err(DealEngineError::InsufficientBond {
                provider: proposal.provider_id,
                required: bond_required,
                available: provider.bond_available_nano,
            });
        }

        let bond_locked_nano = provider.bond_locked_nano.checked_add(bond_required).ok_or(
            DealEngineError::BalanceOverflow {
                resource: "provider_bond_locked",
            },
        )?;
        provider.bond_available_nano -= bond_required;
        provider.bond_locked_nano = bond_locked_nano;

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
            DealState::new(
                record.clone(),
                terms_digest,
                bond_required,
                activation_epoch,
            ),
        );

        Ok(record)
    }

    /// Record usage for an active deal and evaluate micropayment tickets.
    pub(crate) fn record_usage(
        &self,
        report: DealUsageReport,
    ) -> Result<UsageOutcome, DealEngineError> {
        let ticket_limit = self.entry_limit.min(MAX_DEAL_USAGE_TICKETS);
        if report.tickets.len() > ticket_limit {
            return Err(DealEngineError::ResourceExhausted {
                resource: "tickets_per_usage_report",
                limit: ticket_limit,
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

        let (start, deal_end) = match entry.record.status {
            DealStatus::Active(_) => (entry.record.start_epoch, entry.record.end_epoch),
            _ => return Err(DealEngineError::DealInactive(report.deal_id)),
        };
        let settlement_window_end = entry
            .last_settlement_epoch
            .checked_add(entry.record.terms.settlement_window_epochs)
            .ok_or(DealEngineError::SettlementEpochOverflow(report.deal_id))?;
        let end = deal_end.min(settlement_window_end);

        if report.epoch < start || report.epoch > end {
            return Err(DealEngineError::UsageEpochOutOfRange {
                deal_id: report.deal_id,
                usage_epoch: report.epoch,
                start,
                end,
            });
        }
        let previous_epoch = entry
            .last_usage_epoch
            .unwrap_or(entry.last_settlement_epoch);
        if report.epoch <= previous_epoch {
            return Err(DealEngineError::UsageEpochNotMonotonic {
                deal_id: report.deal_id,
                usage_epoch: report.epoch,
                previous_epoch,
            });
        }
        if report.storage_gib_hours == 0 && report.egress_bytes == 0 && report.tickets.is_empty() {
            return Err(DealEngineError::InvalidTicket {
                deal_id: report.deal_id,
                reason: "usage report is empty",
            });
        }

        let provider_id = entry.record.provider_id;
        let client_id = entry.record.client_id;

        if !inner.clients.contains_key(&client_id) {
            return Err(DealEngineError::UnknownClient(client_id));
        }

        let remaining_tickets = self.entry_limit.saturating_sub(inner.seen_ticket_count);
        let mut novel_tickets = HashSet::new();
        novel_tickets
            .try_reserve(report.tickets.len())
            .map_err(|_| DealEngineError::AllocationFailed {
                resource: "usage_ticket_validation",
            })?;
        let mut ticket_storage_gib_hours = 0_u128;
        let mut ticket_egress_bytes = 0_u128;
        let mut previous_ticket_id = None;
        for ticket in &report.tickets {
            let ticket_bytes = *ticket.ticket_id.as_bytes();
            if ticket.issued_epoch != report.epoch || ticket.is_empty() {
                return Err(DealEngineError::InvalidTicket {
                    deal_id: report.deal_id,
                    reason: "ticket epoch must equal the report epoch and carry accounting weight",
                });
            }
            let expected_ticket_id = derive_micropayment_ticket_id(
                report.deal_id,
                ticket.issued_epoch,
                ticket.storage_gib_hours,
                ticket.egress_bytes,
            );
            if ticket.ticket_id != expected_ticket_id {
                return Err(DealEngineError::InvalidTicket {
                    deal_id: report.deal_id,
                    reason: "ticket id does not bind deal, epoch, storage, and egress",
                });
            }
            if let Some(previous) = previous_ticket_id {
                if previous == ticket.ticket_id {
                    return Err(DealEngineError::TicketReplay {
                        deal_id: report.deal_id,
                        ticket_id: ticket.ticket_id,
                    });
                }
                if previous > ticket.ticket_id {
                    return Err(DealEngineError::InvalidTicket {
                        deal_id: report.deal_id,
                        reason: "tickets must be strictly ordered by canonical ticket id",
                    });
                }
            }
            previous_ticket_id = Some(ticket.ticket_id);
            if entry.seen_tickets.contains_key(&ticket_bytes) || !novel_tickets.insert(ticket_bytes)
            {
                return Err(DealEngineError::TicketReplay {
                    deal_id: report.deal_id,
                    ticket_id: ticket.ticket_id,
                });
            }
            if novel_tickets.len() > remaining_tickets {
                return Err(DealEngineError::ResourceExhausted {
                    resource: "seen_tickets",
                    limit: self.entry_limit,
                });
            }
            ticket_storage_gib_hours = ticket_storage_gib_hours
                .checked_add(u128::from(ticket.storage_gib_hours))
                .ok_or(DealEngineError::BalanceOverflow {
                    resource: "ticket_storage_gib_hours",
                })?;
            ticket_egress_bytes = ticket_egress_bytes
                .checked_add(u128::from(ticket.egress_bytes))
                .ok_or(DealEngineError::BalanceOverflow {
                    resource: "ticket_egress_bytes",
                })?;
        }
        if ticket_storage_gib_hours > u128::from(report.storage_gib_hours)
            || ticket_egress_bytes > u128::from(report.egress_bytes)
        {
            return Err(DealEngineError::InvalidTicket {
                deal_id: report.deal_id,
                reason: "ticket coverage exceeds the signed usage report",
            });
        }
        let new_ticket_count = novel_tickets.len();
        let mut state = entry.clone();

        let sample_charge = checked_deal_add(
            storage_charge(report.storage_gib_hours as u128, &state.record.terms)?,
            egress_charge(report.egress_bytes as u128, &state.record.terms)?,
            "usage_sample_charge",
        )?;

        state.window_storage_gib_hours = checked_deal_add(
            state.window_storage_gib_hours,
            report.storage_gib_hours as u128,
            "window_storage_gib_hours",
        )?;
        state.window_egress_bytes = checked_deal_add(
            state.window_egress_bytes,
            report.egress_bytes as u128,
            "window_egress_bytes",
        )?;
        state.window_expected_charge_nano = checked_deal_add(
            state.window_expected_charge_nano,
            sample_charge,
            "window_expected_charge",
        )?;
        state.total_storage_gib_hours = checked_deal_add(
            state.total_storage_gib_hours,
            report.storage_gib_hours as u128,
            "total_storage_gib_hours",
        )?;
        state.total_egress_bytes = checked_deal_add(
            state.total_egress_bytes,
            report.egress_bytes as u128,
            "total_egress_bytes",
        )?;

        let mut tickets_processed = 0usize;
        let mut tickets_won = 0usize;
        let tickets_duplicate = 0usize;
        let mut new_credit = 0u128;
        let mut generated_credit = 0u128;

        for ticket in &report.tickets {
            tickets_processed += 1;
            let ticket_bytes = *ticket.ticket_id.as_bytes();
            let previous = state.seen_tickets.insert(ticket_bytes, *ticket);
            debug_assert!(
                previous.is_none(),
                "tickets were validated as novel before mutation"
            );

            if evaluate_ticket(
                state.record.deal_id,
                ticket.ticket_id,
                state.record.terms.micropayment_probability_bps,
            ) {
                tickets_won += 1;
                let payout = state.record.terms.micropayment_payout_nano as u128;
                new_credit = checked_deal_add(new_credit, payout, "usage_micropayment_credit")?;
                generated_credit =
                    checked_deal_add(generated_credit, payout, "usage_generated_credit")?;
            }
        }

        let next_window_generated = checked_deal_add(
            state.window_micropayment_generated_nano,
            generated_credit,
            "window_micropayment_generated",
        )?;
        if next_window_generated > state.window_expected_charge_nano {
            return Err(DealEngineError::InvalidTicket {
                deal_id: report.deal_id,
                reason: "winning ticket credit exceeds deterministic charge in the settlement window",
            });
        }

        let mut due_remaining = checked_deal_add(
            state.outstanding_nano,
            sample_charge,
            "deal_outstanding_before_credit",
        )?;
        let provider_credit_total = new_credit;

        let mut credit_applied = 0u128;

        if due_remaining > 0 && state.micropayment_credit_carry > 0 {
            let applied = due_remaining.min(state.micropayment_credit_carry);
            due_remaining -= applied;
            state.micropayment_credit_carry -= applied;
            credit_applied = checked_deal_add(credit_applied, applied, "usage_credit_applied")?;
        }

        if due_remaining > 0 && new_credit > 0 {
            let applied = due_remaining.min(new_credit);
            due_remaining -= applied;
            new_credit -= applied;
            credit_applied = checked_deal_add(credit_applied, applied, "usage_credit_applied")?;
        }

        state.micropayment_credit_carry = checked_deal_add(
            state.micropayment_credit_carry,
            new_credit,
            "micropayment_credit_carry",
        )?;
        state.window_micropayment_credit_applied = checked_deal_add(
            state.window_micropayment_credit_applied,
            credit_applied,
            "window_micropayment_credit_applied",
        )?;
        state.outstanding_nano = due_remaining;
        state.window_micropayment_generated_nano = next_window_generated;
        state.total_micropayment_generated_nano = checked_deal_add(
            state.total_micropayment_generated_nano,
            generated_credit,
            "total_micropayment_generated",
        )?;
        state.last_usage_epoch = Some(report.epoch);

        let provider_earnings = inner
            .providers
            .get(&provider_id)
            .ok_or(DealEngineError::UnknownProvider(provider_id))?
            .earnings_nano;
        let provider_earnings = checked_deal_add(
            provider_earnings,
            provider_credit_total,
            "provider_earnings",
        )?;

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

        inner
            .providers
            .get_mut(&provider_id)
            .expect("provider checked above")
            .earnings_nano = provider_earnings;
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

        let window_epochs = entry.record.terms.settlement_window_epochs;

        if !matches!(entry.record.status, DealStatus::Active(_)) {
            return Err(DealEngineError::DealInactive(deal_id));
        }
        let expected_settlement_epoch = entry
            .last_settlement_epoch
            .checked_add(window_epochs)
            .ok_or(DealEngineError::SettlementEpochOverflow(deal_id))?;
        if settlement_epoch != expected_settlement_epoch {
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
        let next_seen_ticket_count = inner
            .seen_ticket_count
            .checked_sub(state.seen_tickets.len())
            .ok_or_else(|| {
                DealEngineError::InvalidCheckpoint(
                    "global replay-ticket count is below the settling deal window".to_owned(),
                )
            })?;

        let window_start = state.last_settlement_epoch;
        let expected_charge = state.window_expected_charge_nano;
        let credit_applied = state.window_micropayment_credit_applied;
        let mut amount_due = state.outstanding_nano;

        let mut client_debit = 0u128;
        if amount_due > 0 && client.credit_balance_nano > 0 {
            client_debit = amount_due.min(client.credit_balance_nano);
            client.credit_balance_nano -= client_debit;
            client.credit_debited_nano = checked_deal_add(
                client.credit_debited_nano,
                client_debit,
                "client_credit_debited",
            )?;
            amount_due -= client_debit;
        }

        let mut bond_slash = 0u128;
        if client_debit > 0 {
            provider.earnings_nano =
                checked_deal_add(provider.earnings_nano, client_debit, "provider_earnings")?;
        }
        if amount_due > 0 && state.locked_bond_nano > 0 {
            bond_slash = amount_due.min(state.locked_bond_nano);
            state.locked_bond_nano -= bond_slash;
            provider.bond_locked_nano = provider
                .bond_locked_nano
                .checked_sub(bond_slash)
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint(
                        "provider locked bond is below deal bond".to_owned(),
                    )
                })?;
            provider.bond_slashed_nano = checked_deal_add(
                provider.bond_slashed_nano,
                bond_slash,
                "provider_bond_slashed",
            )?;
            amount_due -= bond_slash;
        }

        state.outstanding_nano = amount_due;
        state.settlement_count =
            state
                .settlement_count
                .checked_add(1)
                .ok_or(DealEngineError::BalanceOverflow {
                    resource: "settlement_count",
                })?;
        state.last_settlement_epoch = settlement_epoch;

        state.total_expected_charge_nano = checked_deal_add(
            state.total_expected_charge_nano,
            expected_charge,
            "total_expected_charge",
        )?;
        state.total_micropayment_credit_nano = checked_deal_add(
            state.total_micropayment_credit_nano,
            credit_applied,
            "total_micropayment_credit",
        )?;
        state.total_client_debit_nano = checked_deal_add(
            state.total_client_debit_nano,
            client_debit,
            "total_client_debit",
        )?;
        state.total_bond_slash_nano =
            checked_deal_add(state.total_bond_slash_nano, bond_slash, "total_bond_slash")?;

        let mut bond_released = 0_u128;
        if settlement_epoch < state.record.end_epoch && state.locked_bond_nano == 0 {
            if state.micropayment_credit_carry != 0 || state.total_bond_slash_nano == 0 {
                return Err(DealEngineError::InvalidCheckpoint(
                    "early default must exhaust non-zero collateral without credit carry"
                        .to_owned(),
                ));
            }
            state.record.status = DealStatus::Defaulted(settlement_epoch);
        } else if settlement_epoch >= state.record.end_epoch {
            if state.micropayment_credit_carry != 0 {
                return Err(DealEngineError::InvalidCheckpoint(
                    "terminal deal retains unapplied micropayment credit".to_owned(),
                ));
            }
            let locked_bond = state.locked_bond_nano;
            if state.outstanding_nano == 0 && locked_bond > 0 {
                bond_released = locked_bond;
                provider.bond_available_nano = checked_deal_add(
                    provider.bond_available_nano,
                    locked_bond,
                    "provider_bond_available",
                )?;
                provider.bond_locked_nano = provider
                    .bond_locked_nano
                    .checked_sub(locked_bond)
                    .ok_or_else(|| {
                        DealEngineError::InvalidCheckpoint(
                            "provider locked bond is below released deal bond".to_owned(),
                        )
                    })?;
            }
            if state.outstanding_nano == 0 {
                state.locked_bond_nano = 0;
                state.bond_released_nano = checked_deal_add(
                    state.bond_released_nano,
                    bond_released,
                    "deal_bond_released",
                )?;
                state.record.status = DealStatus::Settled(settlement_epoch);
            } else {
                if state.locked_bond_nano != 0 {
                    return Err(DealEngineError::InvalidCheckpoint(
                        "terminal default retains slashable bond".to_owned(),
                    ));
                }
                state.record.status = DealStatus::Defaulted(settlement_epoch);
            }
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
        let window_micropayment_generated = state.window_micropayment_generated_nano;
        state.window_micropayment_generated_nano = 0;
        state.window_micropayment_credit_applied = 0;
        state.window_storage_gib_hours = 0;
        state.window_egress_bytes = 0;

        let provider_accrual_nano = checked_deal_add(
            state.total_client_debit_nano,
            state.total_micropayment_generated_nano,
            "provider_accrual",
        )?;
        let previous_settlement = state.settlement_head.clone();
        let mut ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            snapshot_id: [0; 32],
            sequence: state.settlement_count,
            previous_snapshot_id: previous_settlement
                .as_ref()
                .map(|settlement| settlement.ledger.snapshot_id),
            deal_id: *deal_id.as_bytes(),
            terms_digest: state.terms_digest,
            provider_id: *state.record.provider_id.as_bytes(),
            client_id: *state.record.client_id.as_bytes(),
            deal_start_epoch: state.record.start_epoch,
            deal_end_epoch: state.record.end_epoch,
            settlement_window_epochs: state.record.terms.settlement_window_epochs,
            window_start_epoch: window_start,
            window_end_epoch: settlement_epoch,
            provider_accrual_nano,
            client_liability_nano: state.total_expected_charge_nano,
            micropayment_credit_generated_nano: state.total_micropayment_generated_nano,
            micropayment_credit_applied_nano: state.total_micropayment_credit_nano,
            micropayment_credit_carry_nano: state.micropayment_credit_carry,
            client_debit_nano: state.total_client_debit_nano,
            outstanding_liability_nano: state.outstanding_nano,
            bond_total_nano: state.initial_bond_nano,
            bond_locked_nano: state.locked_bond_nano,
            bond_slashed_nano: state.total_bond_slash_nano,
            bond_released_nano: state.bond_released_nano,
            window_expected_charge_nano: expected_charge,
            window_micropayment_generated_nano: window_micropayment_generated,
            window_micropayment_applied_nano: credit_applied,
            window_client_debit_nano: client_debit,
            window_bond_slashed_nano: bond_slash,
            window_bond_released_nano: bond_released,
            captured_at: settlement_epoch,
        };
        ledger.snapshot_id = ledger.derive_snapshot_id().map_err(|error| {
            DealEngineError::InvalidCheckpoint(format!(
                "failed to derive canonical deal ledger snapshot: {error}"
            ))
        })?;
        let status = match state.record.status {
            DealStatus::Active(_) => DealSettlementStatusV1::WindowSettled,
            DealStatus::Settled(_) => DealSettlementStatusV1::Completed,
            DealStatus::Cancelled(_) => DealSettlementStatusV1::Cancelled,
            DealStatus::Defaulted(_) => DealSettlementStatusV1::Defaulted,
            DealStatus::Proposed => {
                return Err(DealEngineError::InvalidCheckpoint(
                    "active deal transitioned back to proposed".to_owned(),
                ));
            }
        };
        let audit_notes = if bond_slash > 0 {
            Some(format!(
                "bond slashed {} nano (total {} nano); outstanding {} nano",
                bond_slash, state.total_bond_slash_nano, state.outstanding_nano
            ))
        } else {
            match status {
                DealSettlementStatusV1::Cancelled => {
                    Some("deal cancelled by governance prior to completion".to_string())
                }
                DealSettlementStatusV1::Defaulted => Some(format!(
                    "deal defaulted with {} nano outstanding after bond exhaustion",
                    state.outstanding_nano
                )),
                DealSettlementStatusV1::WindowSettled | DealSettlementStatusV1::Completed => None,
            }
        };
        let mut governance = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            settlement_id: [0; 32],
            deal_id: *deal_id.as_bytes(),
            ledger,
            status,
            settled_at: settlement_epoch,
            audit_notes,
        };
        governance.settlement_id = governance.derive_settlement_id().map_err(|error| {
            DealEngineError::InvalidCheckpoint(format!(
                "failed to derive canonical deal settlement: {error}"
            ))
        })?;
        governance
            .validate_transition(previous_settlement.as_ref())
            .map_err(|error| {
                DealEngineError::InvalidCheckpoint(format!(
                    "generated deal settlement transition is invalid: {error}"
                ))
            })?;
        state.settlement_head = Some(governance.clone());
        state.last_usage_epoch = None;
        state.seen_tickets.clear();

        let outcome = DealSettlementOutcome {
            record: settlement,
            governance,
        };

        inner.clients.insert(client_id, client);
        inner.providers.insert(provider_id, provider);
        inner.seen_ticket_count = next_seen_ticket_count;
        inner.deals.insert(deal_id, state);

        Ok(outcome)
    }

    /// Cancel an idle active deal at its exact next settlement boundary.
    ///
    /// Cancellation is intentionally conservative: any unfinalised usage, credit carry, or
    /// liability must be settled first. The remaining bond is released and a final canonical
    /// governance settlement is produced.
    pub(crate) fn cancel(
        &self,
        deal_id: DealId,
        cancellation_epoch: u64,
        reason: String,
    ) -> Result<DealSettlementOutcome, DealEngineError> {
        if reason.is_empty()
            || reason != reason.trim()
            || reason.len() > MAX_DEAL_SETTLEMENT_AUDIT_NOTES_BYTES
            || reason.chars().any(char::is_control)
        {
            return Err(DealEngineError::InvalidCancellationReason);
        }
        let mut inner = self
            .inner
            .write()
            .map_err(|_| DealEngineError::StateLockPoisoned)?;
        let entry = inner
            .deals
            .get(&deal_id)
            .ok_or(DealEngineError::UnknownDeal(deal_id))?;
        if !matches!(entry.record.status, DealStatus::Active(_)) {
            return Err(DealEngineError::DealInactive(deal_id));
        }
        let expected_epoch = entry
            .last_settlement_epoch
            .checked_add(entry.record.terms.settlement_window_epochs)
            .ok_or(DealEngineError::SettlementEpochOverflow(deal_id))?;
        if cancellation_epoch != expected_epoch {
            return Err(DealEngineError::SettlementWindowMismatch {
                deal_id,
                settlement_epoch: cancellation_epoch,
                window_epochs: entry.record.terms.settlement_window_epochs,
            });
        }
        if cancellation_epoch >= entry.record.end_epoch {
            return Err(DealEngineError::UnsafeCancellation {
                deal_id,
                reason: "terminal deals must use normal settlement",
            });
        }
        if entry.last_usage_epoch.is_some()
            || entry.window_expected_charge_nano != 0
            || entry.window_micropayment_generated_nano != 0
            || entry.window_micropayment_credit_applied != 0
            || entry.window_storage_gib_hours != 0
            || entry.window_egress_bytes != 0
        {
            return Err(DealEngineError::UnsafeCancellation {
                deal_id,
                reason: "the current window contains unsettled usage",
            });
        }
        if entry.outstanding_nano != 0 || entry.micropayment_credit_carry != 0 {
            return Err(DealEngineError::UnsafeCancellation {
                deal_id,
                reason: "liability or micropayment carry remains outstanding",
            });
        }

        let provider_id = entry.record.provider_id;
        let mut provider = inner
            .providers
            .get(&provider_id)
            .cloned()
            .ok_or(DealEngineError::UnknownProvider(provider_id))?;
        let mut state = entry.clone();
        let next_seen_ticket_count = inner
            .seen_ticket_count
            .checked_sub(state.seen_tickets.len())
            .ok_or_else(|| {
                DealEngineError::InvalidCheckpoint(
                    "global replay-ticket count is below the cancelled deal".to_owned(),
                )
            })?;
        let bond_released = state.locked_bond_nano;
        if bond_released == 0 {
            return Err(DealEngineError::UnsafeCancellation {
                deal_id,
                reason: "active deal has no collateral to release",
            });
        }
        provider.bond_locked_nano = provider
            .bond_locked_nano
            .checked_sub(bond_released)
            .ok_or_else(|| {
                DealEngineError::InvalidCheckpoint(
                    "provider locked bond is below cancelled deal bond".to_owned(),
                )
            })?;
        provider.bond_available_nano = checked_deal_add(
            provider.bond_available_nano,
            bond_released,
            "provider_bond_available",
        )?;
        state.locked_bond_nano = 0;
        state.bond_released_nano = checked_deal_add(
            state.bond_released_nano,
            bond_released,
            "deal_bond_released",
        )?;
        state.settlement_count =
            state
                .settlement_count
                .checked_add(1)
                .ok_or(DealEngineError::BalanceOverflow {
                    resource: "settlement_count",
                })?;
        let window_start = state.last_settlement_epoch;
        state.last_settlement_epoch = cancellation_epoch;
        state.last_usage_epoch = None;
        state.record.status = DealStatus::Cancelled(cancellation_epoch);

        let previous_settlement = state.settlement_head.clone();
        let provider_accrual_nano = checked_deal_add(
            state.total_micropayment_generated_nano,
            state.total_client_debit_nano,
            "provider_accrual",
        )?;
        let mut ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            snapshot_id: [0; 32],
            sequence: state.settlement_count,
            previous_snapshot_id: previous_settlement
                .as_ref()
                .map(|settlement| settlement.ledger.snapshot_id),
            deal_id: *deal_id.as_bytes(),
            terms_digest: state.terms_digest,
            provider_id: *state.record.provider_id.as_bytes(),
            client_id: *state.record.client_id.as_bytes(),
            deal_start_epoch: state.record.start_epoch,
            deal_end_epoch: state.record.end_epoch,
            settlement_window_epochs: state.record.terms.settlement_window_epochs,
            window_start_epoch: window_start,
            window_end_epoch: cancellation_epoch,
            provider_accrual_nano,
            client_liability_nano: state.total_expected_charge_nano,
            micropayment_credit_generated_nano: state.total_micropayment_generated_nano,
            micropayment_credit_applied_nano: state.total_micropayment_credit_nano,
            micropayment_credit_carry_nano: 0,
            client_debit_nano: state.total_client_debit_nano,
            outstanding_liability_nano: 0,
            bond_total_nano: state.initial_bond_nano,
            bond_locked_nano: 0,
            bond_slashed_nano: state.total_bond_slash_nano,
            bond_released_nano: state.bond_released_nano,
            window_expected_charge_nano: 0,
            window_micropayment_generated_nano: 0,
            window_micropayment_applied_nano: 0,
            window_client_debit_nano: 0,
            window_bond_slashed_nano: 0,
            window_bond_released_nano: bond_released,
            captured_at: cancellation_epoch,
        };
        ledger.snapshot_id = ledger.derive_snapshot_id().map_err(|error| {
            DealEngineError::InvalidCheckpoint(format!(
                "failed to derive cancellation ledger snapshot: {error}"
            ))
        })?;
        let mut governance = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            settlement_id: [0; 32],
            deal_id: *deal_id.as_bytes(),
            ledger,
            status: DealSettlementStatusV1::Cancelled,
            settled_at: cancellation_epoch,
            audit_notes: Some(reason),
        };
        governance.settlement_id = governance.derive_settlement_id().map_err(|error| {
            DealEngineError::InvalidCheckpoint(format!(
                "failed to derive canonical deal cancellation: {error}"
            ))
        })?;
        governance
            .validate_transition(previous_settlement.as_ref())
            .map_err(|error| {
                DealEngineError::InvalidCheckpoint(format!(
                    "generated deal cancellation transition is invalid: {error}"
                ))
            })?;
        state.settlement_head = Some(governance.clone());
        state.seen_tickets.clear();

        let outcome = DealSettlementOutcome {
            record: DealSettlementRecord {
                provider_id: state.record.provider_id,
                client_id: state.record.client_id,
                deal_id,
                settlement_index: state.settlement_count,
                settled_epoch: cancellation_epoch,
                window_start_epoch: window_start,
                window_end_epoch: cancellation_epoch,
                billed_storage_gib_hours: 0,
                billed_egress_bytes: 0,
                expected_charge_nano: 0,
                micropayment_credit_nano: 0,
                client_credit_debit_nano: 0,
                bond_slash_nano: 0,
                outstanding_nano: 0,
            },
            governance,
        };
        inner.providers.insert(provider_id, provider);
        inner.seen_ticket_count = next_seen_ticket_count;
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
                funding_sequence: account.funding_sequence,
                bond_deposited_nano: account.bond_deposited_nano,
                bond_available_nano: account.bond_available_nano,
                bond_locked_nano: account.bond_locked_nano,
                bond_slashed_nano: account.bond_slashed_nano,
                earnings_nano: account.earnings_nano,
            })
    }

    /// Obtain a snapshot of the client account.
    pub fn client_snapshot(&self, client_id: ClientId) -> Option<ClientSnapshot> {
        let inner = self.inner.read().expect("deal engine poisoned");
        inner.clients.get(&client_id).map(|account| ClientSnapshot {
            funding_sequence: account.funding_sequence,
            credit_deposited_nano: account.credit_deposited_nano,
            credit_balance_nano: account.credit_balance_nano,
            credit_debited_nano: account.credit_debited_nano,
        })
    }

    /// Obtain a snapshot of the current deal state.
    pub fn deal_snapshot(&self, deal_id: DealId) -> Option<DealSnapshot> {
        let inner = self.inner.read().expect("deal engine poisoned");
        inner.deals.get(&deal_id).map(|state| DealSnapshot {
            deal_id,
            provider_id: state.record.provider_id,
            client_id: state.record.client_id,
            status: state.record.status,
            outstanding_nano: state.outstanding_nano,
            credit_carry_nano: state.micropayment_credit_carry,
            locked_bond_nano: state.locked_bond_nano,
            initial_bond_nano: state.initial_bond_nano,
            bond_released_nano: state.bond_released_nano,
            settlement_count: state.settlement_count,
            latest_ledger_snapshot_id: state
                .settlement_head
                .as_ref()
                .map(|settlement| settlement.ledger.snapshot_id),
        })
    }

    /// Export a canonical restart snapshot of all authoritative accounting state.
    pub(crate) fn checkpoint(&self) -> Result<DealRuntimeCheckpointV1, DealEngineError> {
        let inner = self
            .inner
            .read()
            .map_err(|_| DealEngineError::StateLockPoisoned)?;
        let mut providers = Vec::new();
        providers
            .try_reserve_exact(inner.providers.len())
            .map_err(|_| DealEngineError::AllocationFailed {
                resource: "checkpoint_providers",
            })?;
        providers.extend(inner.providers.iter().map(|(provider_id, account)| {
            DealProviderCheckpointV1 {
                provider_id: *provider_id,
                account: account.clone(),
            }
        }));
        providers.sort_by_key(|entry| entry.provider_id);
        let mut clients = Vec::new();
        clients
            .try_reserve_exact(inner.clients.len())
            .map_err(|_| DealEngineError::AllocationFailed {
                resource: "checkpoint_clients",
            })?;
        clients.extend(
            inner
                .clients
                .iter()
                .map(|(client_id, account)| DealClientCheckpointV1 {
                    client_id: *client_id,
                    account: account.clone(),
                }),
        );
        clients.sort_by_key(|entry| entry.client_id);
        let mut deals = Vec::new();
        deals.try_reserve_exact(inner.deals.len()).map_err(|_| {
            DealEngineError::AllocationFailed {
                resource: "checkpoint_deals",
            }
        })?;
        for state in inner.deals.values() {
            let mut seen_tickets = Vec::new();
            seen_tickets
                .try_reserve_exact(state.seen_tickets.len())
                .map_err(|_| DealEngineError::AllocationFailed {
                    resource: "checkpoint_seen_tickets",
                })?;
            seen_tickets.extend(state.seen_tickets.values().copied());
            seen_tickets.sort_unstable_by_key(|ticket| ticket.ticket_id);
            deals.push(DealStateCheckpointV1 {
                record: state.record.clone(),
                terms_digest: state.terms_digest,
                initial_bond_nano: state.initial_bond_nano,
                locked_bond_nano: state.locked_bond_nano,
                bond_released_nano: state.bond_released_nano,
                outstanding_nano: state.outstanding_nano,
                micropayment_credit_carry: state.micropayment_credit_carry,
                total_expected_charge_nano: state.total_expected_charge_nano,
                total_micropayment_generated_nano: state.total_micropayment_generated_nano,
                total_micropayment_credit_nano: state.total_micropayment_credit_nano,
                total_client_debit_nano: state.total_client_debit_nano,
                total_bond_slash_nano: state.total_bond_slash_nano,
                window_expected_charge_nano: state.window_expected_charge_nano,
                window_micropayment_generated_nano: state.window_micropayment_generated_nano,
                window_micropayment_credit_applied: state.window_micropayment_credit_applied,
                window_storage_gib_hours: state.window_storage_gib_hours,
                window_egress_bytes: state.window_egress_bytes,
                total_storage_gib_hours: state.total_storage_gib_hours,
                total_egress_bytes: state.total_egress_bytes,
                settlement_count: state.settlement_count,
                last_settlement_epoch: state.last_settlement_epoch,
                last_usage_epoch: state.last_usage_epoch,
                settlement_head: state.settlement_head.clone(),
                seen_tickets,
            });
        }
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

        let mut providers = HashMap::new();
        providers
            .try_reserve(checkpoint.providers.len())
            .map_err(|_| DealEngineError::AllocationFailed {
                resource: "restore_providers",
            })?;
        let mut previous_provider = None;
        for entry in checkpoint.providers {
            if previous_provider.is_some_and(|previous| previous >= entry.provider_id) {
                return Err(DealEngineError::InvalidCheckpoint(
                    "provider checkpoint keys are not strictly sorted".to_owned(),
                ));
            }
            let accounted = entry
                .account
                .bond_available_nano
                .checked_add(entry.account.bond_locked_nano)
                .and_then(|amount| amount.checked_add(entry.account.bond_slashed_nano))
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint(
                        "provider bond conservation overflow".to_owned(),
                    )
                })?;
            if entry.account.funding_sequence == 0
                || entry.account.bond_deposited_nano == 0
                || accounted != entry.account.bond_deposited_nano
            {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "provider {} does not conserve deposited collateral",
                    hex::encode(entry.provider_id.as_bytes())
                )));
            }
            previous_provider = Some(entry.provider_id);
            providers.insert(entry.provider_id, entry.account);
        }
        let mut clients = HashMap::new();
        clients.try_reserve(checkpoint.clients.len()).map_err(|_| {
            DealEngineError::AllocationFailed {
                resource: "restore_clients",
            }
        })?;
        let mut previous_client = None;
        for entry in checkpoint.clients {
            if previous_client.is_some_and(|previous| previous >= entry.client_id) {
                return Err(DealEngineError::InvalidCheckpoint(
                    "client checkpoint keys are not strictly sorted".to_owned(),
                ));
            }
            let accounted_credit = entry
                .account
                .credit_balance_nano
                .checked_add(entry.account.credit_debited_nano)
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint(
                        "client credit conservation overflow".to_owned(),
                    )
                })?;
            if entry.account.funding_sequence == 0
                || entry.account.credit_deposited_nano == 0
                || accounted_credit != entry.account.credit_deposited_nano
            {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "client {} does not conserve deposited credit",
                    hex::encode(entry.client_id.as_bytes())
                )));
            }
            previous_client = Some(entry.client_id);
            clients.insert(entry.client_id, entry.account);
        }

        let mut deals = HashMap::new();
        deals.try_reserve(checkpoint.deals.len()).map_err(|_| {
            DealEngineError::AllocationFailed {
                resource: "restore_deals",
            }
        })?;
        let mut previous_deal = None;
        let mut seen_ticket_count = 0usize;
        let mut locked_bond_by_provider = HashMap::<ProviderId, u128>::new();
        locked_bond_by_provider
            .try_reserve(providers.len())
            .map_err(|_| DealEngineError::AllocationFailed {
                resource: "restore_provider_bond_index",
            })?;
        let mut slashed_bond_by_provider = HashMap::<ProviderId, u128>::new();
        slashed_bond_by_provider
            .try_reserve(providers.len())
            .map_err(|_| DealEngineError::AllocationFailed {
                resource: "restore_provider_slash_index",
            })?;
        let mut earnings_by_provider = HashMap::<ProviderId, u128>::new();
        earnings_by_provider
            .try_reserve(providers.len())
            .map_err(|_| DealEngineError::AllocationFailed {
                resource: "restore_provider_earnings_index",
            })?;
        let mut debits_by_client = HashMap::<ClientId, u128>::new();
        debits_by_client.try_reserve(clients.len()).map_err(|_| {
            DealEngineError::AllocationFailed {
                resource: "restore_client_debit_index",
            }
        })?;
        for entry in checkpoint.deals {
            let deal_id = entry.record.deal_id;
            if previous_deal.is_some_and(|previous| previous >= deal_id) {
                return Err(DealEngineError::InvalidCheckpoint(
                    "deal checkpoint keys are not strictly sorted".to_owned(),
                ));
            }
            previous_deal = Some(deal_id);
            if !providers.contains_key(&entry.record.provider_id)
                || !clients.contains_key(&entry.record.client_id)
            {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "deal {} references a missing provider or client",
                    hex::encode(deal_id.as_bytes())
                )));
            }
            let proposal = proposal_from_record(&entry.record);
            validate_deal_proposal(&proposal).map_err(|error| {
                DealEngineError::InvalidCheckpoint(format!(
                    "deal {} has invalid immutable terms: {error}",
                    hex::encode(deal_id.as_bytes())
                ))
            })?;
            let expected_deal_id = compute_deal_id(&proposal).map_err(|error| {
                DealEngineError::InvalidCheckpoint(format!(
                    "deal {} identifier derivation failed: {error}",
                    hex::encode(deal_id.as_bytes())
                ))
            })?;
            if deal_id != expected_deal_id {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "deal {} identifier does not bind its canonical proposal",
                    hex::encode(deal_id.as_bytes())
                )));
            }
            let expected_terms_digest = compute_deal_terms_digest(&proposal).map_err(|error| {
                DealEngineError::InvalidCheckpoint(format!(
                    "deal {} terms digest derivation failed: {error}",
                    hex::encode(deal_id.as_bytes())
                ))
            })?;
            let expected_initial_bond =
                checked_bond_requirement(&entry.record.terms, entry.record.capacity_gib).map_err(
                    |error| {
                        DealEngineError::InvalidCheckpoint(format!(
                            "deal {} bond derivation failed: {error}",
                            hex::encode(deal_id.as_bytes())
                        ))
                    },
                )?;
            if entry.terms_digest != expected_terms_digest
                || entry.initial_bond_nano != expected_initial_bond
            {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "deal {} terms digest or initial bond does not bind its proposal",
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
            let all_expected = entry
                .total_expected_charge_nano
                .checked_add(entry.window_expected_charge_nano)
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint(
                        "deal expected-charge checkpoint overflow".to_owned(),
                    )
                })?;
            let all_applied = entry
                .total_micropayment_credit_nano
                .checked_add(entry.window_micropayment_credit_applied)
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint(
                        "deal applied-credit checkpoint overflow".to_owned(),
                    )
                })?;
            let generated_uses = all_applied
                .checked_add(entry.micropayment_credit_carry)
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint(
                        "deal generated-credit checkpoint overflow".to_owned(),
                    )
                })?;
            let liability_uses = all_applied
                .checked_add(entry.total_client_debit_nano)
                .and_then(|amount| amount.checked_add(entry.total_bond_slash_nano))
                .and_then(|amount| amount.checked_add(entry.outstanding_nano))
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint(
                        "deal liability checkpoint overflow".to_owned(),
                    )
                })?;
            let bond_accounted = entry
                .locked_bond_nano
                .checked_add(entry.total_bond_slash_nano)
                .and_then(|amount| amount.checked_add(entry.bond_released_nano))
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint("deal bond checkpoint overflow".to_owned())
                })?;
            if entry.total_micropayment_generated_nano != generated_uses
                || all_expected != liability_uses
                || entry.initial_bond_nano != bond_accounted
                || entry.window_micropayment_generated_nano > entry.window_expected_charge_nano
            {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "deal {} violates credit, liability, or bond conservation",
                    hex::encode(deal_id.as_bytes())
                )));
            }
            match (entry.settlement_count, entry.settlement_head.as_ref()) {
                (0, None) => {}
                (0, Some(_)) | (_, None) => {
                    return Err(DealEngineError::InvalidCheckpoint(format!(
                        "deal {} settlement head does not match its high-water mark",
                        hex::encode(deal_id.as_bytes())
                    )));
                }
                (1, Some(settlement)) => settlement.validate_transition(None).map_err(|error| {
                    DealEngineError::InvalidCheckpoint(format!(
                        "deal {} first settlement head is invalid: {error}",
                        hex::encode(deal_id.as_bytes())
                    ))
                })?,
                (_, Some(settlement)) => settlement.validate().map_err(|error| {
                    DealEngineError::InvalidCheckpoint(format!(
                        "deal {} settlement head is invalid: {error}",
                        hex::encode(deal_id.as_bytes())
                    ))
                })?,
            }
            let activation_epoch = match entry.record.status {
                DealStatus::Active(epoch) => epoch,
                DealStatus::Settled(epoch)
                | DealStatus::Cancelled(epoch)
                | DealStatus::Defaulted(epoch) => epoch,
                DealStatus::Proposed => {
                    return Err(DealEngineError::InvalidCheckpoint(format!(
                        "deal {} cannot be proposed inside the active engine",
                        hex::encode(deal_id.as_bytes())
                    )));
                }
            };
            if entry.settlement_count == 0 {
                if !matches!(entry.record.status, DealStatus::Active(_))
                    || entry.last_settlement_epoch != activation_epoch
                    || entry.total_expected_charge_nano != 0
                    || entry.total_micropayment_credit_nano != 0
                    || entry.total_client_debit_nano != 0
                    || entry.total_bond_slash_nano != 0
                    || entry.bond_released_nano != 0
                {
                    return Err(DealEngineError::InvalidCheckpoint(format!(
                        "deal {} has pre-settlement totals without a settlement chain",
                        hex::encode(deal_id.as_bytes())
                    )));
                }
            } else {
                let last = entry
                    .settlement_head
                    .as_ref()
                    .expect("non-zero settlement count has a validated head");
                let settled_generated = entry
                    .total_micropayment_generated_nano
                    .checked_sub(entry.window_micropayment_generated_nano)
                    .ok_or_else(|| {
                        DealEngineError::InvalidCheckpoint(
                            "window generated credit exceeds cumulative credit".to_owned(),
                        )
                    })?;
                let settled_outstanding = entry
                    .window_micropayment_credit_applied
                    .checked_add(entry.outstanding_nano)
                    .and_then(|uses| uses.checked_sub(entry.window_expected_charge_nano))
                    .ok_or_else(|| {
                        DealEngineError::InvalidCheckpoint(
                            "current window liability cannot derive predecessor outstanding"
                                .to_owned(),
                        )
                    })?;
                let reconstructed_outstanding = settled_outstanding
                    .checked_add(entry.window_expected_charge_nano)
                    .and_then(|sources| {
                        sources.checked_sub(entry.window_micropayment_credit_applied)
                    })
                    .ok_or_else(|| {
                        DealEngineError::InvalidCheckpoint(
                            "current window outstanding reconstruction overflow".to_owned(),
                        )
                    })?;
                if reconstructed_outstanding != entry.outstanding_nano {
                    return Err(DealEngineError::InvalidCheckpoint(
                        "current window outstanding does not reconstruct exactly".to_owned(),
                    ));
                }
                let settled_carry = entry
                    .window_micropayment_credit_applied
                    .checked_add(entry.micropayment_credit_carry)
                    .and_then(|uses| uses.checked_sub(entry.window_micropayment_generated_nano))
                    .ok_or_else(|| {
                        DealEngineError::InvalidCheckpoint(
                            "current window credit cannot derive its predecessor carry".to_owned(),
                        )
                    })?;
                let settled_provider_accrual = settled_generated
                    .checked_add(entry.total_client_debit_nano)
                    .ok_or_else(|| {
                        DealEngineError::InvalidCheckpoint(
                            "settled provider accrual checkpoint overflow".to_owned(),
                        )
                    })?;
                if last.ledger.sequence != entry.settlement_count
                    || last.settled_at != entry.last_settlement_epoch
                    || last.deal_id != *deal_id.as_bytes()
                    || last.ledger.terms_digest != entry.terms_digest
                    || last.ledger.provider_id != *entry.record.provider_id.as_bytes()
                    || last.ledger.client_id != *entry.record.client_id.as_bytes()
                    || last.ledger.deal_start_epoch != entry.record.start_epoch
                    || last.ledger.deal_end_epoch != entry.record.end_epoch
                    || last.ledger.settlement_window_epochs
                        != entry.record.terms.settlement_window_epochs
                    || last.ledger.provider_accrual_nano != settled_provider_accrual
                    || last.ledger.client_liability_nano != entry.total_expected_charge_nano
                    || last.ledger.micropayment_credit_generated_nano != settled_generated
                    || last.ledger.micropayment_credit_applied_nano
                        != entry.total_micropayment_credit_nano
                    || last.ledger.micropayment_credit_carry_nano != settled_carry
                    || last.ledger.client_debit_nano != entry.total_client_debit_nano
                    || last.ledger.outstanding_liability_nano != settled_outstanding
                    || last.ledger.bond_total_nano != entry.initial_bond_nano
                    || last.ledger.bond_slashed_nano != entry.total_bond_slash_nano
                    || last.ledger.bond_released_nano != entry.bond_released_nano
                    || last.ledger.bond_locked_nano != entry.locked_bond_nano
                {
                    return Err(DealEngineError::InvalidCheckpoint(format!(
                        "deal {} runtime totals do not match its settlement-chain head",
                        hex::encode(deal_id.as_bytes())
                    )));
                }
            }
            match entry.record.status {
                DealStatus::Active(_) => {
                    if (entry.settlement_count != 0
                        && entry.last_settlement_epoch >= entry.record.end_epoch)
                        || entry.settlement_head.as_ref().is_some_and(|last| {
                            last.status != DealSettlementStatusV1::WindowSettled
                        })
                    {
                        return Err(DealEngineError::InvalidCheckpoint(format!(
                            "active deal {} has a terminal settlement head",
                            hex::encode(deal_id.as_bytes())
                        )));
                    }
                }
                DealStatus::Settled(epoch) => {
                    if epoch != entry.last_settlement_epoch
                        || entry.locked_bond_nano != 0
                        || entry.outstanding_nano != 0
                        || entry.micropayment_credit_carry != 0
                        || entry.last_usage_epoch.is_some()
                        || entry
                            .settlement_head
                            .as_ref()
                            .is_none_or(|last| last.status != DealSettlementStatusV1::Completed)
                    {
                        return Err(DealEngineError::InvalidCheckpoint(format!(
                            "settled deal {} violates finality",
                            hex::encode(deal_id.as_bytes())
                        )));
                    }
                }
                DealStatus::Defaulted(epoch) => {
                    if epoch != entry.last_settlement_epoch
                        || entry.locked_bond_nano != 0
                        || entry.total_bond_slash_nano == 0
                        || entry.micropayment_credit_carry != 0
                        || entry.last_usage_epoch.is_some()
                        || entry
                            .settlement_head
                            .as_ref()
                            .is_none_or(|last| last.status != DealSettlementStatusV1::Defaulted)
                    {
                        return Err(DealEngineError::InvalidCheckpoint(format!(
                            "defaulted deal {} violates finality",
                            hex::encode(deal_id.as_bytes())
                        )));
                    }
                }
                DealStatus::Cancelled(epoch) => {
                    if epoch < entry.record.start_epoch
                        || epoch >= entry.record.end_epoch
                        || entry.locked_bond_nano != 0
                        || entry.outstanding_nano != 0
                        || entry.micropayment_credit_carry != 0
                        || entry.last_usage_epoch.is_some()
                        || entry
                            .settlement_head
                            .as_ref()
                            .is_none_or(|last| last.status != DealSettlementStatusV1::Cancelled)
                    {
                        return Err(DealEngineError::InvalidCheckpoint(format!(
                            "cancelled deal {} violates finality",
                            hex::encode(deal_id.as_bytes())
                        )));
                    }
                }
                DealStatus::Proposed => unreachable!("proposed rejected above"),
            }
            if let Some(last_usage_epoch) = entry.last_usage_epoch {
                let window_end = entry
                    .last_settlement_epoch
                    .checked_add(entry.record.terms.settlement_window_epochs)
                    .ok_or_else(|| {
                        DealEngineError::InvalidCheckpoint(
                            "deal usage-window epoch overflow".to_owned(),
                        )
                    })?
                    .min(entry.record.end_epoch);
                if !matches!(entry.record.status, DealStatus::Active(_))
                    || last_usage_epoch <= entry.last_settlement_epoch
                    || last_usage_epoch > window_end
                {
                    return Err(DealEngineError::InvalidCheckpoint(format!(
                        "deal {} last usage epoch is stale, future, or final",
                        hex::encode(deal_id.as_bytes())
                    )));
                }
            } else if entry.window_expected_charge_nano != 0
                || entry.window_micropayment_generated_nano != 0
                || entry.window_micropayment_credit_applied != 0
                || entry.window_storage_gib_hours != 0
                || entry.window_egress_bytes != 0
            {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "deal {} has window accounting without a usage high-water mark",
                    hex::encode(deal_id.as_bytes())
                )));
            }
            if !entry
                .seen_tickets
                .windows(2)
                .all(|pair| pair[0].ticket_id < pair[1].ticket_id)
            {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "deal {} ticket ids are duplicated or not canonical",
                    hex::encode(deal_id.as_bytes())
                )));
            }
            let mut ticket_storage_gib_hours = 0_u128;
            let mut ticket_egress_bytes = 0_u128;
            for ticket in &entry.seen_tickets {
                let accepted_epoch_ceiling = entry
                    .last_usage_epoch
                    .unwrap_or(entry.last_settlement_epoch);
                if ticket.is_empty()
                    || entry.last_usage_epoch.is_none()
                    || ticket.issued_epoch <= entry.last_settlement_epoch
                    || ticket.issued_epoch > entry.record.end_epoch
                    || ticket.issued_epoch > accepted_epoch_ceiling
                    || derive_micropayment_ticket_id(
                        deal_id,
                        ticket.issued_epoch,
                        ticket.storage_gib_hours,
                        ticket.egress_bytes,
                    ) != ticket.ticket_id
                {
                    return Err(DealEngineError::InvalidCheckpoint(format!(
                        "deal {} contains a forged or out-of-window ticket",
                        hex::encode(deal_id.as_bytes())
                    )));
                }
                ticket_storage_gib_hours = ticket_storage_gib_hours
                    .checked_add(u128::from(ticket.storage_gib_hours))
                    .ok_or_else(|| {
                        DealEngineError::InvalidCheckpoint(
                            "ticket storage coverage checkpoint overflow".to_owned(),
                        )
                    })?;
                ticket_egress_bytes = ticket_egress_bytes
                    .checked_add(u128::from(ticket.egress_bytes))
                    .ok_or_else(|| {
                        DealEngineError::InvalidCheckpoint(
                            "ticket egress coverage checkpoint overflow".to_owned(),
                        )
                    })?;
            }
            if ticket_storage_gib_hours > entry.window_storage_gib_hours
                || ticket_egress_bytes > entry.window_egress_bytes
            {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "deal {} ticket coverage exceeds retained usage",
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
            *locked_bond = locked_bond
                .checked_add(entry.locked_bond_nano)
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint(
                        "provider locked-bond checkpoint overflow".to_owned(),
                    )
                })?;
            let slashed_bond = slashed_bond_by_provider
                .entry(entry.record.provider_id)
                .or_default();
            *slashed_bond = slashed_bond
                .checked_add(entry.total_bond_slash_nano)
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint(
                        "provider slashed-bond checkpoint overflow".to_owned(),
                    )
                })?;
            let deal_earnings = entry
                .total_micropayment_generated_nano
                .checked_add(entry.total_client_debit_nano)
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint(
                        "provider earnings checkpoint overflow".to_owned(),
                    )
                })?;
            let provider_earnings = earnings_by_provider
                .entry(entry.record.provider_id)
                .or_default();
            *provider_earnings = provider_earnings
                .checked_add(deal_earnings)
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint(
                        "provider aggregate earnings checkpoint overflow".to_owned(),
                    )
                })?;
            let client_debits = debits_by_client.entry(entry.record.client_id).or_default();
            *client_debits = client_debits
                .checked_add(entry.total_client_debit_nano)
                .ok_or_else(|| {
                    DealEngineError::InvalidCheckpoint(
                        "client aggregate debit checkpoint overflow".to_owned(),
                    )
                })?;
            let mut restored_tickets = HashMap::new();
            restored_tickets
                .try_reserve(entry.seen_tickets.len())
                .map_err(|_| DealEngineError::AllocationFailed {
                    resource: "restore_seen_tickets",
                })?;
            for ticket in entry.seen_tickets {
                let ticket_id = *ticket.ticket_id.as_bytes();
                if restored_tickets.insert(ticket_id, ticket).is_some() {
                    return Err(DealEngineError::InvalidCheckpoint(
                        "duplicate restored ticket id".to_owned(),
                    ));
                }
            }
            let state = DealState {
                record: entry.record,
                terms_digest: entry.terms_digest,
                initial_bond_nano: entry.initial_bond_nano,
                locked_bond_nano: entry.locked_bond_nano,
                bond_released_nano: entry.bond_released_nano,
                outstanding_nano: entry.outstanding_nano,
                micropayment_credit_carry: entry.micropayment_credit_carry,
                total_expected_charge_nano: entry.total_expected_charge_nano,
                total_micropayment_generated_nano: entry.total_micropayment_generated_nano,
                total_micropayment_credit_nano: entry.total_micropayment_credit_nano,
                total_client_debit_nano: entry.total_client_debit_nano,
                total_bond_slash_nano: entry.total_bond_slash_nano,
                window_expected_charge_nano: entry.window_expected_charge_nano,
                window_micropayment_generated_nano: entry.window_micropayment_generated_nano,
                window_micropayment_credit_applied: entry.window_micropayment_credit_applied,
                window_storage_gib_hours: entry.window_storage_gib_hours,
                window_egress_bytes: entry.window_egress_bytes,
                total_storage_gib_hours: entry.total_storage_gib_hours,
                total_egress_bytes: entry.total_egress_bytes,
                settlement_count: entry.settlement_count,
                last_settlement_epoch: entry.last_settlement_epoch,
                last_usage_epoch: entry.last_usage_epoch,
                settlement_head: entry.settlement_head,
                seen_tickets: restored_tickets,
            };
            deals.insert(deal_id, state);
        }
        for (provider_id, account) in &providers {
            let expected_locked = locked_bond_by_provider
                .get(provider_id)
                .copied()
                .unwrap_or_default();
            let expected_slashed = slashed_bond_by_provider
                .get(provider_id)
                .copied()
                .unwrap_or_default();
            let expected_earnings = earnings_by_provider
                .get(provider_id)
                .copied()
                .unwrap_or_default();
            if account.bond_locked_nano != expected_locked
                || account.bond_slashed_nano != expected_slashed
                || account.earnings_nano != expected_earnings
            {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "provider {} bond or earnings account disagrees with deal state",
                    hex::encode(provider_id.as_bytes())
                )));
            }
        }
        for (client_id, account) in &clients {
            let expected_debits = debits_by_client.get(client_id).copied().unwrap_or_default();
            if account.credit_debited_nano != expected_debits {
                return Err(DealEngineError::InvalidCheckpoint(format!(
                    "client {} debit account disagrees with deal state",
                    hex::encode(client_id.as_bytes())
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

fn proposal_from_record(record: &DealRecord) -> DealProposal {
    DealProposal {
        provider_id: record.provider_id,
        client_id: record.client_id,
        storage_class: record.storage_class,
        capacity_gib: record.capacity_gib,
        start_epoch: record.start_epoch,
        end_epoch: record.end_epoch,
        terms: record.terms,
        metadata: record.metadata.clone(),
    }
}

fn validate_deal_proposal(proposal: &DealProposal) -> Result<(), DealEngineError> {
    proposal
        .validate()
        .map_err(|error| DealEngineError::InvalidProposal(error.to_string()))?;
    let metadata_len =
        <iroha_data_model::metadata::Metadata as norito::NoritoSerialize>::encoded_len_exact(
            &proposal.metadata,
        )
        .ok_or_else(|| {
            DealEngineError::InvalidProposal("metadata encoded length is not exact".to_owned())
        })?;
    if metadata_len > MAX_DEAL_METADATA_ENCODED_BYTES {
        return Err(DealEngineError::InvalidProposal(format!(
            "metadata is {metadata_len} bytes; maximum is {MAX_DEAL_METADATA_ENCODED_BYTES}"
        )));
    }
    checked_bond_requirement(&proposal.terms, proposal.capacity_gib)?;
    Ok(())
}

fn checked_bond_requirement(terms: &DealTerms, capacity_gib: u64) -> Result<u128, DealEngineError> {
    u128::from(terms.storage_price_nano_per_gib_month)
        .checked_mul(u128::from(capacity_gib))
        .and_then(|monthly| monthly.checked_mul(3))
        .ok_or(DealEngineError::BalanceOverflow {
            resource: "deal_bond_requirement",
        })
}

fn compute_deal_id(proposal: &DealProposal) -> Result<DealId, DealEngineError> {
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
    let metadata_len = u64::try_from(metadata_bytes.len()).map_err(|_| {
        DealEngineError::InvalidProposal("metadata encoded length exceeds u64".to_owned())
    })?;
    hasher.update(&metadata_len.to_le_bytes());
    hasher.update(&metadata_bytes);
    let digest = hasher.finalize();
    Ok(DealId::new(*digest.as_bytes()))
}

fn compute_deal_terms_digest(proposal: &DealProposal) -> Result<[u8; 32], DealEngineError> {
    let deal_id = compute_deal_id(proposal)?;
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs.deal.runtime-terms.v1");
    hasher.update(deal_id.as_bytes());
    Ok(*hasher.finalize().as_bytes())
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

/// Derive the canonical ticket identifier from its immutable deal and usage binding.
#[must_use]
pub fn derive_micropayment_ticket_id(
    deal_id: DealId,
    issued_epoch: u64,
    storage_gib_hours: u64,
    egress_bytes: u64,
) -> TicketId {
    let mut hasher = Hasher::new();
    hasher.update(b"sorafs.deal.ticket.id.v1");
    hasher.update(deal_id.as_bytes());
    hasher.update(&issued_epoch.to_le_bytes());
    hasher.update(&storage_gib_hours.to_le_bytes());
    hasher.update(&egress_bytes.to_le_bytes());
    TicketId(*hasher.finalize().as_bytes())
}

fn storage_charge(gib_hours: u128, terms: &DealTerms) -> Result<u128, DealEngineError> {
    u128::from(terms.storage_price_nano_per_gib_month)
        .checked_mul(gib_hours)
        .map(|amount| amount / GIB_HOURS_PER_MONTH)
        .ok_or(DealEngineError::BalanceOverflow {
            resource: "storage_charge",
        })
}

fn egress_charge(bytes: u128, terms: &DealTerms) -> Result<u128, DealEngineError> {
    u128::from(terms.egress_price_nano_per_gib)
        .checked_mul(bytes)
        .map(|amount| amount / BYTES_PER_GIB)
        .ok_or(DealEngineError::BalanceOverflow {
            resource: "egress_charge",
        })
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

    fn ticket(deal_id: DealId, issued_epoch: u64, weight: u8) -> MicropaymentTicket {
        let storage_gib_hours = u64::from(weight).saturating_add(1);
        let egress_bytes = 0;
        MicropaymentTicket {
            ticket_id: derive_micropayment_ticket_id(
                deal_id,
                issued_epoch,
                storage_gib_hours,
                egress_bytes,
            ),
            issued_epoch,
            storage_gib_hours,
            egress_bytes,
        }
    }

    fn canonical_tickets(mut tickets: Vec<MicropaymentTicket>) -> Vec<MicropaymentTicket> {
        tickets.sort_unstable_by_key(|ticket| ticket.ticket_id);
        tickets
    }

    fn proposal(
        provider_id: ProviderId,
        client_id: ClientId,
        start_epoch: u64,
        end_epoch: u64,
        capacity_gib: u64,
        terms: DealTerms,
    ) -> DealProposal {
        DealProposal {
            provider_id,
            client_id,
            storage_class: StorageClass::Hot,
            capacity_gib,
            start_epoch,
            end_epoch,
            terms,
            metadata: Metadata::default(),
        }
    }

    fn checkpoint_bytes(engine: &DealEngine) -> Vec<u8> {
        norito::to_bytes(&engine.checkpoint().expect("deal checkpoint"))
            .expect("encode deal checkpoint")
    }

    fn assert_checkpoint_rejected_atomically(
        engine: &DealEngine,
        checkpoint: DealRuntimeCheckpointV1,
    ) {
        let before = checkpoint_bytes(engine);
        assert!(matches!(
            engine
                .restore_checkpoint(checkpoint)
                .expect_err("tampered checkpoint must fail"),
            DealEngineError::InvalidCheckpoint(_)
        ));
        assert_eq!(checkpoint_bytes(engine), before);
    }

    #[test]
    fn open_deal_requires_sufficient_bond() {
        let engine = DealEngine::new();
        let provider = provider(1);
        let client = client(2);

        engine
            .deposit_provider_bond(provider, 1_000_000_000)
            .expect("deposit provider bond");
        engine
            .deposit_client_credit(client, 1_000_000_000)
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

        assert!(matches!(
            err,
            DealEngineError::InsufficientBond {
                provider: _,
                required: _,
                available: _
            }
        ));
    }

    #[test]
    fn deal_lifecycle_settlement_releases_bond() {
        let engine = DealEngine::new();
        let provider = provider(1);
        let client = client(2);

        engine
            .deposit_provider_bond(provider, 15_000_000_000)
            .expect("deposit provider bond");
        engine
            .deposit_client_credit(client, 3_000_000_000)
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
            tickets: canonical_tickets(vec![
                ticket(record.deal_id, 12, 10),
                ticket(record.deal_id, 12, 11),
                ticket(record.deal_id, 12, 12),
                ticket(record.deal_id, 12, 13),
                ticket(record.deal_id, 12, 14),
            ]),
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
        assert_eq!(governance.ledger.provider_accrual_nano, 2_550_000_000);

        let provider_snapshot = engine.provider_snapshot(provider).expect("provider");
        assert_eq!(provider_snapshot.bond_available_nano, 15_000_000_000);
        assert_eq!(provider_snapshot.bond_locked_nano, 0);
        assert_eq!(provider_snapshot.earnings_nano, 2_550_000_000);

        let deal_snapshot = engine.deal_snapshot(record.deal_id).expect("deal snapshot");
        assert!(matches!(deal_snapshot.status, DealStatus::Settled(17)));
        assert_eq!(deal_snapshot.outstanding_nano, 0);
        assert_eq!(deal_snapshot.locked_bond_nano, 0);
        assert_eq!(
            provider_snapshot.bond_deposited_nano,
            provider_snapshot.bond_available_nano
                + provider_snapshot.bond_locked_nano
                + provider_snapshot.bond_slashed_nano
        );
        let client_snapshot = engine.client_snapshot(client).expect("client");
        assert_eq!(client_snapshot.credit_deposited_nano, 3_000_000_000);
        assert_eq!(client_snapshot.credit_debited_nano, 2_050_000_000);
        assert_eq!(client_snapshot.credit_balance_nano, 950_000_000);
        governance.validate().expect("canonical settlement");
    }

    #[test]
    fn usage_outcome_reports_micropayment_metrics() {
        let engine = DealEngine::new();
        let provider = provider(3);
        let client = client(4);

        engine
            .deposit_provider_bond(provider, 15_000_000_000)
            .expect("deposit provider bond");
        engine
            .deposit_client_credit(client, 5_000_000_000)
            .expect("deposit client credit");

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
            tickets: canonical_tickets(vec![
                ticket(record.deal_id, 11, 10),
                ticket(record.deal_id, 11, 11),
            ]),
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
    fn duplicate_ticket_fails_before_mutation() {
        let engine = DealEngine::new();
        let provider = provider(7);
        let client = client(8);

        engine
            .deposit_provider_bond(provider, 15_000_000_000)
            .expect("deposit provider bond");
        engine
            .deposit_client_credit(client, 3_000_000_000)
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
            tickets: vec![
                ticket(record.deal_id, 11, 42),
                ticket(record.deal_id, 11, 42),
            ],
        };

        assert!(matches!(
            engine.record_usage(usage).expect_err("duplicate rejected"),
            DealEngineError::TicketReplay { .. }
        ));
        let snapshot = engine.deal_snapshot(record.deal_id).expect("deal state");
        assert_eq!(snapshot.outstanding_nano, 0);
    }

    #[test]
    fn configured_limits_refuse_accounts_deals_and_replay_tickets() {
        let account_limited = DealEngine::with_entry_limit(1);
        account_limited
            .deposit_provider_bond(provider(1), 1)
            .expect("first provider");
        assert!(matches!(
            account_limited
                .deposit_provider_bond(provider(2), 1)
                .expect_err("second provider must be refused"),
            DealEngineError::ResourceExhausted {
                resource: "providers",
                limit: 1
            }
        ));
        account_limited
            .deposit_client_credit(client(1), 1)
            .expect("first client");
        assert!(matches!(
            account_limited
                .deposit_client_credit(client(2), 1)
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
            .deposit_provider_bond(provider_id, 10_000_000_000)
            .expect("provider deposit");
        deal_limited
            .deposit_client_credit(client_id, 1)
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
            .deposit_provider_bond(provider_id, 10_000_000_000)
            .expect("provider deposit");
        ticket_limited
            .deposit_client_credit(client_id, 1)
            .expect("client deposit");
        let mut ticket_terms = sample_terms();
        ticket_terms.micropayment_payout_nano = 1;
        let record = ticket_limited
            .open_deal(
                DealProposal {
                    provider_id,
                    client_id,
                    storage_class: StorageClass::Hot,
                    capacity_gib: 1,
                    start_epoch: 10,
                    end_epoch: 20,
                    terms: ticket_terms,
                    metadata: Metadata::default(),
                },
                10,
            )
            .expect("open deal");
        let report = |epoch, tickets: Vec<MicropaymentTicket>| DealUsageReport {
            deal_id: record.deal_id,
            epoch,
            storage_gib_hours: tickets.iter().map(|ticket| ticket.storage_gib_hours).sum(),
            egress_bytes: 0,
            tickets,
        };
        assert!(matches!(
            ticket_limited
                .record_usage(report(
                    11,
                    vec![
                        ticket(record.deal_id, 11, 1),
                        ticket(record.deal_id, 11, 2),
                        ticket(record.deal_id, 11, 3),
                    ],
                ))
                .expect_err("oversized ticket batch must be refused"),
            DealEngineError::ResourceExhausted {
                resource: "tickets_per_usage_report",
                limit: 2
            }
        ));
        ticket_limited
            .record_usage(report(
                11,
                canonical_tickets(vec![
                    ticket(record.deal_id, 11, 1),
                    ticket(record.deal_id, 11, 2),
                ]),
            ))
            .expect("fill replay set");
        assert!(matches!(
            ticket_limited
                .record_usage(report(12, vec![ticket(record.deal_id, 12, 3)]))
                .expect_err("replay set exhaustion must be refused"),
            DealEngineError::ResourceExhausted {
                resource: "seen_tickets",
                limit: 2
            }
        ));
        ticket_limited
            .settle(record.deal_id, 17)
            .expect("settlement retires prior-window replay tickets");
        ticket_limited
            .record_usage(report(
                18,
                canonical_tickets(vec![
                    ticket(record.deal_id, 18, 3),
                    ticket(record.deal_id, 18, 4),
                ]),
            ))
            .expect("retired ticket capacity is reusable in the next window");
        assert!(matches!(
            ticket_limited
                .record_usage(report(11, vec![ticket(record.deal_id, 11, 1)]))
                .expect_err("retired ticket epoch remains replay protected"),
            DealEngineError::UsageEpochNotMonotonic { .. }
        ));
    }

    #[test]
    fn checkpoint_roundtrip_preserves_replay_protection_and_rejects_forgery() {
        let engine = DealEngine::with_entry_limit(8);
        let provider_id = provider(5);
        let client_id = client(6);
        engine
            .deposit_provider_bond(provider_id, 10_000_000_000)
            .expect("provider deposit");
        engine
            .deposit_client_credit(client_id, 1_000_000_000)
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
                storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
                egress_bytes: 0,
                tickets: vec![ticket(record.deal_id, 11, 7)],
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
                storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
                egress_bytes: 0,
                tickets: vec![ticket(record.deal_id, 11, 7)],
            })
            .expect_err("replayed usage epoch rejected");
        assert!(matches!(
            replay,
            DealEngineError::UsageEpochNotMonotonic { .. }
        ));

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
        forged_bond.providers[0].account.bond_locked_nano = 0;
        assert!(matches!(
            DealEngine::with_entry_limit(8)
                .restore_checkpoint(forged_bond)
                .expect_err("forged provider bond must fail"),
            DealEngineError::InvalidCheckpoint(_)
        ));
    }

    #[test]
    fn proposal_and_deposit_failures_are_atomic() {
        let engine = DealEngine::new();
        let provider_id = provider(0x31);
        let client_id = client(0x32);
        engine
            .deposit_provider_bond(provider_id, 10_000_000_000)
            .expect("provider deposit");
        engine
            .deposit_client_credit(client_id, 5_000_000_000)
            .expect("client deposit");
        let valid = proposal(provider_id, client_id, 10, 20, 1, sample_terms());
        let baseline = checkpoint_bytes(&engine);

        let mut invalid = Vec::new();
        invalid.push(DealProposal {
            provider_id: ProviderId::new([0; 32]),
            ..valid.clone()
        });
        invalid.push(DealProposal {
            client_id: ClientId::new([0; 32]),
            ..valid.clone()
        });
        invalid.push(DealProposal {
            capacity_gib: 0,
            ..valid.clone()
        });
        invalid.push(DealProposal {
            start_epoch: 0,
            ..valid.clone()
        });
        invalid.push(DealProposal {
            start_epoch: 21,
            end_epoch: 20,
            ..valid.clone()
        });
        for mutate in [
            |terms: &mut DealTerms| terms.storage_price_nano_per_gib_month = 0,
            |terms: &mut DealTerms| terms.egress_price_nano_per_gib = 0,
            |terms: &mut DealTerms| terms.settlement_window_epochs = 0,
            |terms: &mut DealTerms| terms.settlement_window_epochs = 12,
            |terms: &mut DealTerms| terms.micropayment_probability_bps = 0,
            |terms: &mut DealTerms| terms.micropayment_probability_bps = 10_001,
            |terms: &mut DealTerms| terms.micropayment_payout_nano = 0,
        ] {
            let mut candidate = valid.clone();
            mutate(&mut candidate.terms);
            invalid.push(candidate);
        }
        for candidate in invalid {
            assert!(matches!(
                engine
                    .open_deal(candidate, 10)
                    .expect_err("invalid proposal must fail"),
                DealEngineError::InvalidProposal(_)
            ));
            assert_eq!(checkpoint_bytes(&engine), baseline);
        }
        for activation_epoch in [9, 21] {
            assert!(matches!(
                engine
                    .open_deal(valid.clone(), activation_epoch)
                    .expect_err("out-of-range activation must fail"),
                DealEngineError::ActivationOutOfRange { .. }
            ));
            assert_eq!(checkpoint_bytes(&engine), baseline);
        }

        let record = engine
            .open_deal(valid.clone(), 10)
            .expect("valid proposal opens");
        let opened = checkpoint_bytes(&engine);
        assert!(matches!(
            engine
                .open_deal(valid, 10)
                .expect_err("duplicate proposal must fail"),
            DealEngineError::DuplicateDeal(id) if id == record.deal_id
        ));
        assert_eq!(checkpoint_bytes(&engine), opened);

        let overflow = DealEngine::new();
        overflow
            .deposit_provider_bond(provider(0x41), u128::MAX)
            .expect("maximum provider deposit");
        let before_provider_overflow = checkpoint_bytes(&overflow);
        assert!(matches!(
            overflow
                .deposit_provider_bond(provider(0x41), 1)
                .expect_err("provider top-up overflow must fail"),
            DealEngineError::BalanceOverflow { .. }
        ));
        assert_eq!(checkpoint_bytes(&overflow), before_provider_overflow);
        overflow
            .deposit_client_credit(client(0x42), u128::MAX)
            .expect("maximum client deposit");
        let before_client_overflow = checkpoint_bytes(&overflow);
        assert!(matches!(
            overflow
                .deposit_client_credit(client(0x42), 1)
                .expect_err("client top-up overflow must fail"),
            DealEngineError::BalanceOverflow { .. }
        ));
        assert_eq!(checkpoint_bytes(&overflow), before_client_overflow);
    }

    #[test]
    fn sequenced_funding_is_restart_safe_and_rejects_replay_fork_and_gap_atomically() {
        let engine = DealEngine::new();
        let provider_id = provider(0x43);
        let client_id = client(0x44);
        let provider_state = engine
            .deposit_provider_bond_sequenced(provider_id, 100, 1)
            .expect("first provider funding");
        assert_eq!(provider_state.funding_sequence, 1);
        let client_state = engine
            .deposit_client_credit_sequenced(client_id, 200, 1)
            .expect("first client funding");
        assert_eq!(client_state.funding_sequence, 1);
        let checkpoint = engine.checkpoint().expect("funding checkpoint");

        let restored = DealEngine::new();
        restored
            .restore_checkpoint(checkpoint)
            .expect("restore funding sequences");
        let baseline = checkpoint_bytes(&restored);
        for sequence in [0, 1, 3] {
            assert!(matches!(
                restored
                    .deposit_provider_bond_sequenced(provider_id, 50, sequence)
                    .expect_err("provider replay/gap must fail"),
                DealEngineError::FundingSequenceMismatch {
                    expected: 2,
                    found,
                    ..
                } if found == sequence
            ));
            assert_eq!(checkpoint_bytes(&restored), baseline);
            assert!(matches!(
                restored
                    .deposit_client_credit_sequenced(client_id, 50, sequence)
                    .expect_err("client replay/gap must fail"),
                DealEngineError::FundingSequenceMismatch {
                    expected: 2,
                    found,
                    ..
                } if found == sequence
            ));
            assert_eq!(checkpoint_bytes(&restored), baseline);
        }
        assert_eq!(
            restored
                .deposit_provider_bond_sequenced(provider_id, 50, 2)
                .expect("next provider funding")
                .bond_deposited_nano,
            150
        );
        assert_eq!(
            restored
                .deposit_client_credit_sequenced(client_id, 50, 2)
                .expect("next client funding")
                .credit_deposited_nano,
            250
        );

        let mut exhausted_checkpoint = restored.checkpoint().expect("exhausted checkpoint");
        exhausted_checkpoint.providers[0].account.funding_sequence = u64::MAX;
        exhausted_checkpoint.clients[0].account.funding_sequence = u64::MAX;
        let exhausted = DealEngine::new();
        exhausted
            .restore_checkpoint(exhausted_checkpoint)
            .expect("maximum sequence checkpoint is valid but final");
        let exhausted_baseline = checkpoint_bytes(&exhausted);
        assert!(matches!(
            exhausted
                .deposit_provider_bond_sequenced(provider_id, 1, u64::MAX)
                .expect_err("provider sequence space is exhausted"),
            DealEngineError::FundingSequenceOverflow {
                account_kind: "provider"
            }
        ));
        assert_eq!(checkpoint_bytes(&exhausted), exhausted_baseline);
        assert!(matches!(
            exhausted
                .deposit_client_credit_sequenced(client_id, 1, u64::MAX)
                .expect_err("client sequence space is exhausted"),
            DealEngineError::FundingSequenceOverflow {
                account_kind: "client"
            }
        ));
        assert_eq!(checkpoint_bytes(&exhausted), exhausted_baseline);
    }

    #[test]
    fn concurrent_funding_sequence_has_exactly_one_winner() {
        use std::{
            sync::{Arc, Barrier},
            thread,
        };

        let engine = DealEngine::new();
        let provider_id = provider(0x45);
        let contenders = 8;
        let barrier = Arc::new(Barrier::new(contenders));
        let handles = (0..contenders)
            .map(|offset| {
                let engine = engine.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    engine.deposit_provider_bond_sequenced(provider_id, 100 + offset as u128, 1)
                })
            })
            .collect::<Vec<_>>();

        let mut accepted = 0;
        let mut conflicts = 0;
        let mut winning_amount = None;
        for handle in handles {
            match handle.join().expect("funding contender did not panic") {
                Ok(snapshot) => {
                    accepted += 1;
                    assert_eq!(snapshot.funding_sequence, 1);
                    winning_amount = Some(snapshot.bond_deposited_nano);
                }
                Err(DealEngineError::FundingSequenceMismatch {
                    expected: 2,
                    found: 1,
                    ..
                }) => conflicts += 1,
                Err(error) => panic!("unexpected concurrent funding error: {error}"),
            }
        }
        assert_eq!(accepted, 1);
        assert_eq!(conflicts, contenders - 1);
        let snapshot = engine
            .provider_snapshot(provider_id)
            .expect("provider state");
        assert_eq!(snapshot.funding_sequence, 1);
        let winning_amount = winning_amount.expect("one funding contender won");
        assert!((100..108).contains(&winning_amount));
        assert_eq!(snapshot.bond_deposited_nano, winning_amount);
        assert_eq!(snapshot.bond_available_nano, winning_amount);
    }

    #[test]
    fn adversarial_usage_reports_fail_before_mutation() {
        let engine = DealEngine::new();
        let provider_id = provider(0x51);
        let client_id = client(0x52);
        engine
            .deposit_provider_bond(provider_id, 10_000_000_000)
            .expect("provider deposit");
        engine
            .deposit_client_credit(client_id, 5_000_000_000)
            .expect("client deposit");
        let record = engine
            .open_deal(
                proposal(provider_id, client_id, 10, 24, 1, sample_terms()),
                10,
            )
            .expect("open deal");
        let baseline = checkpoint_bytes(&engine);

        let weighted = |epoch| DealUsageReport {
            deal_id: record.deal_id,
            epoch,
            storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
            egress_bytes: 0,
            tickets: Vec::new(),
        };
        for report in [weighted(10), weighted(18)] {
            assert!(engine.record_usage(report).is_err());
            assert_eq!(checkpoint_bytes(&engine), baseline);
        }
        assert!(
            engine
                .record_usage(DealUsageReport {
                    deal_id: record.deal_id,
                    epoch: 11,
                    storage_gib_hours: 0,
                    egress_bytes: 0,
                    tickets: Vec::new(),
                })
                .is_err()
        );
        assert_eq!(checkpoint_bytes(&engine), baseline);

        let mut forged_id = ticket(record.deal_id, 11, 0);
        forged_id.ticket_id.0[0] ^= 0x80;
        let wrong_epoch = ticket(record.deal_id, 12, 0);
        let empty = MicropaymentTicket {
            ticket_id: derive_micropayment_ticket_id(record.deal_id, 11, 0, 0),
            issued_epoch: 11,
            storage_gib_hours: 0,
            egress_bytes: 0,
        };
        let coverage = ticket(record.deal_id, 11, 9);
        let foreign = ticket(DealId::new([0xA5; 32]), 11, 0);
        for report in [
            DealUsageReport {
                deal_id: record.deal_id,
                epoch: 11,
                storage_gib_hours: 1,
                egress_bytes: 0,
                tickets: vec![forged_id],
            },
            DealUsageReport {
                deal_id: record.deal_id,
                epoch: 11,
                storage_gib_hours: 1,
                egress_bytes: 0,
                tickets: vec![wrong_epoch],
            },
            DealUsageReport {
                deal_id: record.deal_id,
                epoch: 11,
                storage_gib_hours: 1,
                egress_bytes: 0,
                tickets: vec![empty],
            },
            DealUsageReport {
                deal_id: record.deal_id,
                epoch: 11,
                storage_gib_hours: 1,
                egress_bytes: 0,
                tickets: vec![coverage],
            },
            DealUsageReport {
                deal_id: record.deal_id,
                epoch: 11,
                storage_gib_hours: 1,
                egress_bytes: 0,
                tickets: vec![foreign],
            },
            DealUsageReport {
                deal_id: record.deal_id,
                epoch: 11,
                storage_gib_hours: 1,
                egress_bytes: 0,
                tickets: vec![ticket(record.deal_id, 11, 0)],
            },
        ] {
            assert!(matches!(
                engine
                    .record_usage(report)
                    .expect_err("adversarial usage must fail"),
                DealEngineError::InvalidTicket { .. }
            ));
            assert_eq!(checkpoint_bytes(&engine), baseline);
        }

        let mut unsorted = canonical_tickets(vec![
            ticket(record.deal_id, 11, 1),
            ticket(record.deal_id, 11, 2),
        ]);
        unsorted.reverse();
        let unsorted_storage = unsorted.iter().map(|ticket| ticket.storage_gib_hours).sum();
        assert!(matches!(
            engine
                .record_usage(DealUsageReport {
                    deal_id: record.deal_id,
                    epoch: 11,
                    storage_gib_hours: unsorted_storage,
                    egress_bytes: 0,
                    tickets: unsorted,
                })
                .expect_err("non-canonical ticket order must fail"),
            DealEngineError::InvalidTicket { .. }
        ));
        assert_eq!(checkpoint_bytes(&engine), baseline);

        let flood_ticket = ticket(record.deal_id, 11, 1);
        assert!(matches!(
            engine
                .record_usage(DealUsageReport {
                    deal_id: record.deal_id,
                    epoch: 11,
                    storage_gib_hours: u64::MAX,
                    egress_bytes: 0,
                    tickets: vec![flood_ticket; MAX_DEAL_USAGE_TICKETS + 1],
                })
                .expect_err("protocol ticket cap must fail before allocation-heavy processing"),
            DealEngineError::ResourceExhausted {
                resource: "tickets_per_usage_report",
                limit: MAX_DEAL_USAGE_TICKETS
            }
        ));
        assert_eq!(checkpoint_bytes(&engine), baseline);

        engine.record_usage(weighted(11)).expect("valid usage");
        let accepted = checkpoint_bytes(&engine);
        assert!(matches!(
            engine
                .record_usage(weighted(11))
                .expect_err("same epoch replay must fail"),
            DealEngineError::UsageEpochNotMonotonic { .. }
        ));
        assert_eq!(checkpoint_bytes(&engine), accepted);
    }

    #[test]
    fn settlement_chain_is_exact_final_and_conservative() {
        let engine = DealEngine::new();
        let provider_id = provider(0x61);
        let client_id = client(0x62);
        engine
            .deposit_provider_bond(provider_id, 10_000_000_000)
            .expect("provider deposit");
        engine
            .deposit_client_credit(client_id, 5_000_000_000)
            .expect("client deposit");
        let record = engine
            .open_deal(
                proposal(provider_id, client_id, 10, 24, 1, sample_terms()),
                10,
            )
            .expect("open deal");
        engine
            .record_usage(DealUsageReport {
                deal_id: record.deal_id,
                epoch: 11,
                storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
                egress_bytes: 0,
                tickets: Vec::new(),
            })
            .expect("first usage");
        let before_bad_epoch = checkpoint_bytes(&engine);
        for epoch in [16, 18] {
            assert!(matches!(
                engine
                    .settle(record.deal_id, epoch)
                    .expect_err("settlement gap must fail"),
                DealEngineError::SettlementWindowMismatch { .. }
            ));
            assert_eq!(checkpoint_bytes(&engine), before_bad_epoch);
        }

        let first = engine.settle(record.deal_id, 17).expect("first settlement");
        assert_eq!(
            first.governance.status,
            DealSettlementStatusV1::WindowSettled
        );
        assert_eq!(first.governance.ledger.sequence, 1);
        assert_eq!(first.governance.ledger.previous_snapshot_id, None);
        first
            .governance
            .validate_transition(None)
            .expect("canonical first transition");

        engine
            .record_usage(DealUsageReport {
                deal_id: record.deal_id,
                epoch: 18,
                storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
                egress_bytes: 0,
                tickets: Vec::new(),
            })
            .expect("second usage");
        let second = engine
            .settle(record.deal_id, 24)
            .expect("terminal settlement");
        assert_eq!(second.governance.status, DealSettlementStatusV1::Completed);
        assert_eq!(second.governance.ledger.sequence, 2);
        assert_eq!(
            second.governance.ledger.previous_snapshot_id,
            Some(first.governance.ledger.snapshot_id)
        );
        second
            .governance
            .validate_transition(Some(&first.governance))
            .expect("canonical successor");
        assert_eq!(
            second.governance.ledger.client_liability_nano,
            1_000_000_000
        );
        assert_eq!(second.governance.ledger.client_debit_nano, 1_000_000_000);
        assert_eq!(second.governance.ledger.bond_locked_nano, 0);
        assert_eq!(
            second.governance.ledger.bond_total_nano,
            second.governance.ledger.bond_slashed_nano
                + second.governance.ledger.bond_released_nano
        );

        let provider_state = engine.provider_snapshot(provider_id).expect("provider");
        assert_eq!(
            provider_state.bond_deposited_nano,
            provider_state.bond_available_nano
                + provider_state.bond_locked_nano
                + provider_state.bond_slashed_nano
        );
        let client_state = engine.client_snapshot(client_id).expect("client");
        assert_eq!(
            client_state.credit_deposited_nano,
            client_state.credit_balance_nano + client_state.credit_debited_nano
        );
        let terminal = checkpoint_bytes(&engine);
        assert!(matches!(
            engine
                .settle(record.deal_id, 31)
                .expect_err("terminal deal cannot settle again"),
            DealEngineError::DealInactive(_)
        ));
        assert_eq!(checkpoint_bytes(&engine), terminal);
    }

    #[test]
    fn terminal_default_exhausts_bond_without_creating_provider_earnings() {
        let engine = DealEngine::new();
        let provider_id = provider(0x71);
        let client_id = client(0x72);
        let terms = DealTerms {
            storage_price_nano_per_gib_month: 720,
            egress_price_nano_per_gib: 1,
            settlement_window_epochs: 7,
            micropayment_probability_bps: 1,
            micropayment_payout_nano: 1,
        };
        engine
            .deposit_provider_bond(provider_id, 2_160)
            .expect("exact provider bond");
        engine
            .deposit_client_credit(client_id, 1)
            .expect("minimal client credit");
        let record = engine
            .open_deal(proposal(provider_id, client_id, 10, 16, 1, terms), 10)
            .expect("open deal");
        engine
            .record_usage(DealUsageReport {
                deal_id: record.deal_id,
                epoch: 11,
                storage_gib_hours: 10_000,
                egress_bytes: 0,
                tickets: Vec::new(),
            })
            .expect("record underfunded usage");
        let outcome = engine.settle(record.deal_id, 17).expect("terminal default");
        assert_eq!(outcome.governance.status, DealSettlementStatusV1::Defaulted);
        assert_eq!(outcome.record.expected_charge_nano, 10_000);
        assert_eq!(outcome.record.client_credit_debit_nano, 1);
        assert_eq!(outcome.record.bond_slash_nano, 2_160);
        assert_eq!(outcome.record.outstanding_nano, 7_839);
        assert!(outcome.governance.audit_notes.is_some());
        outcome
            .governance
            .validate()
            .expect("valid default payload");

        let provider_state = engine.provider_snapshot(provider_id).expect("provider");
        assert_eq!(provider_state.bond_deposited_nano, 2_160);
        assert_eq!(provider_state.bond_available_nano, 0);
        assert_eq!(provider_state.bond_locked_nano, 0);
        assert_eq!(provider_state.bond_slashed_nano, 2_160);
        assert_eq!(provider_state.earnings_nano, 1);
        let client_state = engine.client_snapshot(client_id).expect("client");
        assert_eq!(client_state.credit_deposited_nano, 1);
        assert_eq!(client_state.credit_balance_nano, 0);
        assert_eq!(client_state.credit_debited_nano, 1);
    }

    #[test]
    fn collateral_exhaustion_defaults_early_even_when_liability_is_exactly_satisfied() {
        let engine = DealEngine::new();
        let provider_id = provider(0x73);
        let client_id = client(0x74);
        let terms = DealTerms {
            storage_price_nano_per_gib_month: 720,
            egress_price_nano_per_gib: 1,
            settlement_window_epochs: 7,
            micropayment_probability_bps: 1,
            micropayment_payout_nano: 1,
        };
        engine
            .deposit_provider_bond(provider_id, 2_160)
            .expect("exact provider bond");
        engine
            .deposit_client_credit(client_id, 1)
            .expect("minimal client credit");
        let record = engine
            .open_deal(proposal(provider_id, client_id, 10, 30, 1, terms), 10)
            .expect("open deal");
        engine
            .record_usage(DealUsageReport {
                deal_id: record.deal_id,
                epoch: 11,
                storage_gib_hours: 2_161,
                egress_bytes: 0,
                tickets: Vec::new(),
            })
            .expect("record exact exhaustion usage");
        let outcome = engine
            .settle(record.deal_id, 17)
            .expect("collateral exhaustion finalises immediately");
        assert_eq!(outcome.record.outstanding_nano, 0);
        assert_eq!(outcome.record.client_credit_debit_nano, 1);
        assert_eq!(outcome.record.bond_slash_nano, 2_160);
        assert_eq!(outcome.governance.status, DealSettlementStatusV1::Defaulted);
        assert!(outcome.governance.settled_at < outcome.governance.ledger.deal_end_epoch);
        outcome
            .governance
            .validate_transition(None)
            .expect("early default is canonical and final");
        assert!(matches!(
            engine
                .record_usage(DealUsageReport {
                    deal_id: record.deal_id,
                    epoch: 18,
                    storage_gib_hours: 1,
                    egress_bytes: 0,
                    tickets: Vec::new(),
                })
                .expect_err("defaulted deal cannot accept more usage"),
            DealEngineError::DealInactive(_)
        ));
    }

    #[test]
    fn cancellation_is_final_chained_and_refuses_to_strand_window_state() {
        let engine = DealEngine::new();
        let provider_id = provider(0x75);
        let client_id = client(0x76);
        engine
            .deposit_provider_bond(provider_id, 2_000_000_000)
            .expect("provider deposit");
        engine
            .deposit_client_credit(client_id, 1)
            .expect("client deposit");
        let record = engine
            .open_deal(
                proposal(provider_id, client_id, 10, 30, 1, sample_terms()),
                10,
            )
            .expect("open deal");
        let first = engine
            .settle(record.deal_id, 17)
            .expect("settle idle first window");
        assert_eq!(
            first.governance.status,
            DealSettlementStatusV1::WindowSettled
        );
        let maximum_reason = "x".repeat(MAX_DEAL_SETTLEMENT_AUDIT_NOTES_BYTES);
        let cancelled = engine
            .cancel(record.deal_id, 24, maximum_reason.clone())
            .expect("cancel idle second window");
        assert_eq!(
            cancelled.governance.status,
            DealSettlementStatusV1::Cancelled
        );
        assert_eq!(cancelled.governance.ledger.sequence, 2);
        assert_eq!(cancelled.governance.audit_notes, Some(maximum_reason));
        assert_eq!(
            cancelled.governance.ledger.previous_snapshot_id,
            Some(first.governance.ledger.snapshot_id)
        );
        cancelled
            .governance
            .validate_transition(Some(&first.governance))
            .expect("canonical cancellation transition");
        let provider_state = engine.provider_snapshot(provider_id).expect("provider");
        assert_eq!(provider_state.bond_locked_nano, 0);
        assert_eq!(provider_state.bond_available_nano, 2_000_000_000);
        let checkpoint = engine.checkpoint().expect("cancelled checkpoint");
        let mut forged_cancellation = checkpoint.clone();
        forged_cancellation.deals[0]
            .settlement_head
            .as_mut()
            .expect("cancellation head")
            .audit_notes = Some("substituted cancellation rationale".to_owned());
        assert_checkpoint_rejected_atomically(&engine, forged_cancellation);
        DealEngine::new()
            .restore_checkpoint(checkpoint)
            .expect("cancelled finality survives restart");
        assert!(matches!(
            engine
                .cancel(record.deal_id, 31, "replay".to_owned())
                .expect_err("cancel replay must fail"),
            DealEngineError::DealInactive(_)
        ));

        let unsafe_engine = DealEngine::new();
        unsafe_engine
            .deposit_provider_bond(provider_id, 2_000_000_000)
            .expect("provider deposit");
        unsafe_engine
            .deposit_client_credit(client_id, 1_000_000_000)
            .expect("client deposit");
        let unsafe_record = unsafe_engine
            .open_deal(
                proposal(provider_id, client_id, 10, 30, 1, sample_terms()),
                10,
            )
            .expect("open unsafe deal");
        unsafe_engine
            .record_usage(DealUsageReport {
                deal_id: unsafe_record.deal_id,
                epoch: 11,
                storage_gib_hours: 1,
                egress_bytes: 0,
                tickets: Vec::new(),
            })
            .expect("record unsettled usage");
        let baseline = checkpoint_bytes(&unsafe_engine);
        let oversized_reason = "x".repeat(MAX_DEAL_SETTLEMENT_AUDIT_NOTES_BYTES + 1);
        for (epoch, reason) in [
            (16, "valid reason".to_owned()),
            (17, " valid reason".to_owned()),
            (17, String::new()),
            (17, "line\nbreak".to_owned()),
            (17, oversized_reason),
            (17, "valid reason".to_owned()),
        ] {
            let error = unsafe_engine
                .cancel(unsafe_record.deal_id, epoch, reason)
                .expect_err("unsafe cancellation must fail");
            assert!(matches!(
                error,
                DealEngineError::SettlementWindowMismatch { .. }
                    | DealEngineError::InvalidCancellationReason
                    | DealEngineError::UnsafeCancellation { .. }
            ));
            assert_eq!(checkpoint_bytes(&unsafe_engine), baseline);
        }

        let terminal_engine = DealEngine::new();
        terminal_engine
            .deposit_provider_bond(provider_id, 2_000_000_000)
            .expect("terminal provider deposit");
        terminal_engine
            .deposit_client_credit(client_id, 1)
            .expect("terminal client deposit");
        let terminal = terminal_engine
            .open_deal(
                proposal(provider_id, client_id, 10, 17, 1, sample_terms()),
                10,
            )
            .expect("open deal ending at first boundary");
        let terminal_baseline = checkpoint_bytes(&terminal_engine);
        assert!(matches!(
            terminal_engine
                .cancel(terminal.deal_id, 17, "too late to cancel".to_owned())
                .expect_err("terminal boundary must settle normally"),
            DealEngineError::UnsafeCancellation { .. }
        ));
        assert_eq!(checkpoint_bytes(&terminal_engine), terminal_baseline);
    }

    #[test]
    fn concurrent_settlement_and_cancellation_have_one_atomic_winner() {
        use std::{
            sync::{Arc, Barrier},
            thread,
        };

        let engine = DealEngine::new();
        let provider_id = provider(0x77);
        let client_id = client(0x78);
        engine
            .deposit_provider_bond(provider_id, 2_000_000_000)
            .expect("provider deposit");
        engine
            .deposit_client_credit(client_id, 1)
            .expect("client deposit");
        let record = engine
            .open_deal(
                proposal(provider_id, client_id, 10, 30, 1, sample_terms()),
                10,
            )
            .expect("open deal");
        let deal_id = record.deal_id;
        let barrier = Arc::new(Barrier::new(2));
        let settle_engine = engine.clone();
        let settle_barrier = Arc::clone(&barrier);
        let settle = thread::spawn(move || {
            settle_barrier.wait();
            settle_engine.settle(deal_id, 17)
        });
        let cancel_engine = engine.clone();
        let cancel_barrier = Arc::clone(&barrier);
        let cancel = thread::spawn(move || {
            cancel_barrier.wait();
            cancel_engine.cancel(deal_id, 17, "concurrent operator cancellation".to_owned())
        });

        let outcomes = [
            settle.join().expect("settlement contender did not panic"),
            cancel.join().expect("cancellation contender did not panic"),
        ];
        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(outcomes.iter().filter(|result| result.is_err()).count(), 1);
        let snapshot = engine.deal_snapshot(deal_id).expect("deal snapshot");
        assert_eq!(snapshot.settlement_count, 1);
        let provider = engine
            .provider_snapshot(provider_id)
            .expect("provider snapshot");
        assert_eq!(
            provider.bond_deposited_nano,
            provider
                .bond_available_nano
                .checked_add(provider.bond_locked_nano)
                .and_then(|amount| amount.checked_add(provider.bond_slashed_nano))
                .expect("provider bond sum")
        );
        DealEngine::new()
            .restore_checkpoint(engine.checkpoint().expect("winner checkpoint"))
            .expect("concurrent winner state remains restorable");
    }

    #[test]
    fn checkpoint_tampering_is_rejected_without_replacing_live_state() {
        let engine = DealEngine::with_entry_limit(16);
        let provider_id = provider(0x81);
        let client_id = client(0x82);
        engine
            .deposit_provider_bond(provider_id, 10_000_000_000)
            .expect("provider deposit");
        engine
            .deposit_client_credit(client_id, 5_000_000_000)
            .expect("client deposit");
        let record = engine
            .open_deal(
                proposal(provider_id, client_id, 10, 30, 1, sample_terms()),
                10,
            )
            .expect("open deal");
        engine
            .record_usage(DealUsageReport {
                deal_id: record.deal_id,
                epoch: 11,
                storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
                egress_bytes: 0,
                tickets: Vec::new(),
            })
            .expect("first usage");
        engine.settle(record.deal_id, 17).expect("first settlement");
        engine
            .record_usage(DealUsageReport {
                deal_id: record.deal_id,
                epoch: 18,
                storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
                egress_bytes: 0,
                tickets: vec![ticket(record.deal_id, 18, 0)],
            })
            .expect("current-window usage");
        let checkpoint = engine.checkpoint().expect("checkpoint");

        let mut tampered = checkpoint.clone();
        tampered.providers[0].account.bond_available_nano += 1;
        assert_checkpoint_rejected_atomically(&engine, tampered);

        let mut tampered = checkpoint.clone();
        tampered.providers[0].account.funding_sequence = 0;
        assert_checkpoint_rejected_atomically(&engine, tampered);

        let mut tampered = checkpoint.clone();
        tampered.clients[0].account.credit_balance_nano += 1;
        assert_checkpoint_rejected_atomically(&engine, tampered);

        let mut tampered = checkpoint.clone();
        tampered.clients[0].account.funding_sequence = 0;
        assert_checkpoint_rejected_atomically(&engine, tampered);

        let mut tampered = checkpoint.clone();
        tampered.deals[0].terms_digest[0] ^= 1;
        assert_checkpoint_rejected_atomically(&engine, tampered);

        let mut tampered = checkpoint.clone();
        tampered.deals[0].record.provider_id = provider(0xFF);
        assert_checkpoint_rejected_atomically(&engine, tampered);

        let mut tampered = checkpoint.clone();
        tampered.deals[0].total_expected_charge_nano += 1;
        assert_checkpoint_rejected_atomically(&engine, tampered);

        let mut tampered = checkpoint.clone();
        tampered.deals[0]
            .settlement_head
            .as_mut()
            .expect("settlement head")
            .ledger
            .snapshot_id[0] ^= 1;
        assert_checkpoint_rejected_atomically(&engine, tampered);

        let mut tampered = checkpoint.clone();
        tampered.deals[0]
            .settlement_head
            .as_mut()
            .expect("settlement head")
            .settlement_id[0] ^= 1;
        assert_checkpoint_rejected_atomically(&engine, tampered);

        let mut tampered = checkpoint.clone();
        tampered.deals[0].seen_tickets[0].ticket_id.0[0] ^= 1;
        assert_checkpoint_rejected_atomically(&engine, tampered);

        let mut tampered = checkpoint.clone();
        let retained = &mut tampered.deals[0].seen_tickets[0];
        retained.issued_epoch = 11;
        retained.ticket_id = derive_micropayment_ticket_id(
            record.deal_id,
            retained.issued_epoch,
            retained.storage_gib_hours,
            retained.egress_bytes,
        );
        assert_checkpoint_rejected_atomically(&engine, tampered);

        let mut tampered = checkpoint;
        tampered.deals[0].last_usage_epoch = Some(30);
        assert_checkpoint_rejected_atomically(&engine, tampered);
    }

    #[test]
    fn bounded_settlement_head_supports_long_lived_deals_and_terminal_activation_restart() {
        let engine = DealEngine::with_entry_limit(1);
        let provider_id = provider(0x91);
        let client_id = client(0x92);
        engine
            .deposit_provider_bond(provider_id, 2_000_000_000)
            .expect("provider deposit");
        engine
            .deposit_client_credit(client_id, 5_000_000_000)
            .expect("client deposit");
        let record = engine
            .open_deal(
                proposal(provider_id, client_id, 10, 31, 1, sample_terms()),
                10,
            )
            .expect("open deal");
        let mut previous = None;
        for (usage_epoch, settlement_epoch) in [(11, 17), (18, 24), (25, 31)] {
            engine
                .record_usage(DealUsageReport {
                    deal_id: record.deal_id,
                    epoch: usage_epoch,
                    storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
                    egress_bytes: 0,
                    tickets: Vec::new(),
                })
                .expect("usage in exact window");
            let settlement = engine
                .settle(record.deal_id, settlement_epoch)
                .expect("settle exact window");
            if let Some(previous) = previous.as_ref() {
                settlement
                    .governance
                    .validate_transition(Some(previous))
                    .expect("head advances canonically");
            } else {
                settlement
                    .governance
                    .validate_transition(None)
                    .expect("first head is canonical");
            }
            previous = Some(settlement.governance);
        }
        let checkpoint = engine.checkpoint().expect("bounded checkpoint");
        assert_eq!(checkpoint.deals[0].settlement_count, 3);
        assert_eq!(
            checkpoint.deals[0]
                .settlement_head
                .as_ref()
                .expect("head")
                .ledger
                .sequence,
            3
        );
        DealEngine::with_entry_limit(1)
            .restore_checkpoint(checkpoint)
            .expect("restore bounded head");

        let terminal = DealEngine::new();
        let terminal_provider = provider(0xA1);
        let terminal_client = client(0xA2);
        let mut one_epoch_terms = sample_terms();
        one_epoch_terms.settlement_window_epochs = 1;
        terminal
            .deposit_provider_bond(terminal_provider, 2_000_000_000)
            .expect("terminal provider deposit");
        terminal
            .deposit_client_credit(terminal_client, 1)
            .expect("terminal client deposit");
        let terminal_record = terminal
            .open_deal(
                proposal(
                    terminal_provider,
                    terminal_client,
                    10,
                    10,
                    1,
                    one_epoch_terms,
                ),
                10,
            )
            .expect("activate at final usage epoch");
        let checkpoint = terminal.checkpoint().expect("terminal checkpoint");
        let restored = DealEngine::new();
        restored
            .restore_checkpoint(checkpoint)
            .expect("terminal activation restart");
        let outcome = restored
            .settle(terminal_record.deal_id, 11)
            .expect("zero-usage terminal settlement");
        assert_eq!(outcome.governance.status, DealSettlementStatusV1::Completed);
    }
}
