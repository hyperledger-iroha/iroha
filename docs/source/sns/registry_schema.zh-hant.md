---
lang: zh-hant
direction: ltr
source: docs/source/sns/registry_schema.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b2aa0f540cfb93f6ab19c6d5203b984194c76450b571bd760f007124f5b2f53
source_last_modified: "2026-01-28T17:11:30.737738+00:00"
translation_last_reviewed: 2026-02-07
title: Sora Name Service Registry Schema
summary: Norito data structures, lifecycle rules, and event contracts for SNS registry smart contracts (SN-2a).
---

# Sora Name Service Registry Schema (SN-2a)

**Status:** Drafted 2026-03-24 -- submitted for SNS program review  
**Roadmap link:** SN-2a “Registry schema & storage layout”  
**Scope:** Define the canonical Norito structures, lifecycle states, and emitted events for the Sora Name Service (SNS) so registry and registrar implementations stay deterministic across contracts, SDKs, and gateways.

This document completes the schema deliverable for SN-2a by specifying:

1. Identifiers and hashing rules (`SuffixId`, `NameHash`, selector derivation).
2. Norito structs/enums for name records, suffix policies, pricing tiers, revenue splits, and registry events.
3. Storage layout and index prefixes for deterministic replay.
4. A state machine covering registration, renewal, grace/redemption, freezes, and tombstones.
5. Canonical events consumed by DNS/gateway automation.

## 1. Identifiers & Hashing

| Identifier | Description | Derivation |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Registry-wide identifier for top-level suffixes (`.sora`, `.nexus`, `.dao`). Aligned with the suffix catalog in `docs/source/sns_suffix_governance_charter.md`. | Assigned by governance vote; stored in `SuffixPolicyV1`. |
| `SuffixSelector` | Canonical string form of the suffix (ASCII, lower-case). | Example: `.sora` → `sora`. |
| `NameSelectorV1` | Binary selector for the registered label. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Label is NFC + lower-case per Norm v1. |
| `NameHash` (`[u8;32]`) | Primary lookup key used by contracts, events, and caches. | `blake3(NameSelectorV1_bytes)`. |

Determinism requirements:

- Labels are normalised via Norm v1 (UTS-46 strict, STD3 ASCII, NFC). Incoming user strings MUST be normalised before hashing.
- Reserved labels (from `SuffixPolicyV1.reserved_labels`) never enter the registry; governance-only overrides emit `ReservedNameAssigned` events.

## 2. Norito Structures

### 2.1 NameRecordV1

| Field | Type | Notes |
|-------|------|-------|
| `suffix_id` | `u16` | References `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Raw selector bytes for audit/debug. |
| `name_hash` | `[u8; 32]` | Key for maps/events. |
| `normalized_label` | `AsciiString` | Human-readable label (post Norm v1). |
| `display_label` | `AsciiString` | Steward-provided casing; optional cosmetics. |
| `owner` | `AccountId` | Controls renewals/transfers. |
| `controllers` | `Vec<NameControllerV1>` | References target account addresses, resolvers, or application metadata. |
| `status` | `NameStatus` | Lifecycle flag (see Section 4). |
| `pricing_class` | `u8` | Index into suffix pricing tiers (standard, premium, reserved). |
| `registered_at` | `Timestamp` | Block timestamp of the initial activation. |
| `expires_at` | `Timestamp` | End of paid term. |
| `grace_expires_at` | `Timestamp` | End of auto-renew grace (default +30 days). |
| `redemption_expires_at` | `Timestamp` | End of redemption window (default +60 days). |
| `auction` | `Option<NameAuctionStateV1>` | Present when Dutch reopen or premium auctions are active. |
| `last_tx_hash` | `Hash` | Deterministic pointer to the transaction that produced this version. |
| `metadata` | `Metadata` | Arbitrary registrar metadata (text records, proofs). |

Supporting structs:

```text
Enum NameStatus {
    Available,          // derived, not stored on-ledger
    PendingAuction,
    Active,
    GracePeriod,
    Redemption,
    Frozen(NameFrozenStateV1),
    Tombstoned(NameTombstoneStateV1)
}

Struct NameFrozenStateV1 {
    reason: String,
    until_ms: u64,
}

Struct NameTombstoneStateV1 {
    reason: String,
}

Struct NameControllerV1 {
    controller_type: ControllerType,   // Account, ResolverTemplate, ExternalLink
    account_address: Option<AccountAddress>,   // Serialized as canonical `0x…` hex in JSON
    resolver_template_id: Option<String>,
    payload: Metadata,                 // Extra selector/value pairs for wallets/gateways
}

Struct TokenValue {
    asset_id: AsciiString,
    amount: u128,
}

Enum ControllerType {
    Account,
    Multisig,
    ResolverTemplate,
    ExternalLink
}

Struct NameAuctionStateV1 {
    kind: AuctionKind,             // Vickrey, DutchReopen
    opened_at_ms: u64,
    closes_at_ms: u64,
    floor_price: TokenValue,
    highest_commitment: Option<Hash>,  // reference to sealed bid
    settlement_tx: Option<Json>,
}

Enum AuctionKind {
    VickreyCommitReveal,
    DutchReopen
}
```

### 2.2 SuffixPolicyV1

| Field | Type | Notes |
|-------|------|-------|
| `suffix_id` | `u16` | Primary key; stable across policy versions. |
| `suffix` | `AsciiString` | e.g., `sora`. |
| `steward` | `AccountId` | Steward defined in the governance charter. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Default settlement asset identifier (e.g., `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | Tiered pricing coefficients and duration rules. |
| `min_term_years` | `u8` | Floor for purchased term regardless of tier overrides. |
| `grace_period_days` | `u16` | Default 30. |
| `redemption_period_days` | `u16` | Default 60. |
| `max_term_years` | `u8` | Maximum upfront renewal length. |
| `referral_cap_bps` | `u16` | <=1000 (10%) per charter. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Governance supplied list with assignment instructions. |
| `fee_split` | `SuffixFeeSplitV1` | Treasury / steward / referral shares (basis points). |
| `fund_splitter_account` | `AccountId` | Account that holds escrow + distributes funds. |
| `policy_version` | `u16` | Incremented on every change. |
| `metadata` | `Metadata` | Extended notes (KPI covenant, compliance doc hashes). |

```text
Struct PriceTierV1 {
    tier_id: u8,
    label_regex: String,       // RE2-syntax pattern describing eligible labels
    base_price: TokenValue,    // Price per one-year term before suffix coefficient
    auction_kind: AuctionKind, // Default auction when the tier triggers
    dutch_floor: Option<TokenValue>,
    min_duration_years: u8,
    max_duration_years: u8,
}

Struct ReservedNameV1 {
    normalized_label: AsciiString,
    assigned_to: Option<AccountId>,
    release_at_ms: Option<u64>,
    note: String,
}

Struct SuffixFeeSplitV1 {
    treasury_bps: u16,     // default 7000 (70%)
    steward_bps: u16,      // default 3000 (30%)
    referral_max_bps: u16, // optional referral carve-out (<= 1000)
    escrow_bps: u16,       // % routed to claw-back escrow
}
```

### 2.3 Revenue & Settlement Records

| Struct | Fields | Purpose |
|--------|--------|---------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Deterministic record of routed payments per settlement epoch (weekly). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Emitted each time a payment posts (registration, renewal, auction). |

All `TokenValue` fields use Norito’s canonical fixed-point encoding with the currency code declared in the associated `SuffixPolicyV1`.

### 2.4 Registry Events

Canonical events provide a replay log for DNS/gateway automation and analytics.

```text
Struct RegistryEventV1 {
    name_hash: [u8; 32],
    suffix_id: u16,
    selector: NameSelectorV1,
    version: u64,               // increments per NameRecord update
    timestamp: Timestamp,
    tx_hash: Hash,
    actor: AccountId,
    event: RegistryEventKind,
}

Enum RegistryEventKind {
    NameRegistered { expires_at: Timestamp, pricing_class: u8 },
    NameRenewed { expires_at: Timestamp, term_years: u8 },
    NameTransferred { previous_owner: AccountId, new_owner: AccountId },
    NameControllersUpdated { controller_count: u16 },
    NameFrozen(NameFrozenStateV1),
    NameUnfrozen,
    NameTombstoned(NameTombstoneStateV1),
    AuctionOpened { kind: AuctionKind },
    AuctionSettled { winning_account: AccountId, clearing_price: TokenValue },
    RevenueSharePosted { epoch_id: u64, treasury_amount: TokenValue, steward_amount: TokenValue },
    SuffixPolicyUpdated { policy_version: u16 },
}
```

Events must be appended to a replayable log (e.g., `RegistryEvents` domain) and mirrored to gateway feeds so DNS caches invalidate within SLA.

## 3. Storage Layout & Indexes

| Key | Description |
|-----|-------------|
| `Names::<name_hash>` | Primary map from `name_hash` to `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Secondary index for wallet UI (pagination friendly). |
| `NamesByLabel::<suffix_id, normalized_label>` | Detect conflicts, power deterministic search. |
| `SuffixPolicies::<suffix_id>` | Latest `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | `RevenueShareRecordV1` history. |
| `RegistryEvents::<u64>` | Append-only log keyed by monotonically increasing sequence. |

All keys serialise using Norito tuples to keep hashing deterministic across hosts. Index updates occur atomically alongside the primary record.

## 4. Lifecycle State Machine

| State | Entry Conditions | Allowed Transitions | Notes |
|-------|-----------------|---------------------|-------|
| Available | Derived when `NameRecord` absent. | `PendingAuction` (premium), `Active` (standard register). | Availability search reads indexes only. |
| PendingAuction | Created when `PriceTierV1.auction_kind` ≠ none. | `Active` (auction settles), `Tombstoned` (no bids). | Auctions emit `AuctionOpened` and `AuctionSettled`. |
| Active | Registration or renewal succeeded. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` drives transition. |
| GracePeriod | Automatically when `now > expires_at`. | `Active` (on-time renewal), `Redemption`, `Tombstoned`. | Default +30 days; still resolves but flagged. |
| Redemption | `now > grace_expires_at` but `< redemption_expires_at`. | `Active` (late renewal), `Tombstoned`. | Commands require penalty fee. |
| Frozen | Governance or guardian freeze. | `Active` (after remediation), `Tombstoned`. | Cannot transfer or update controllers. |
| Tombstoned | Voluntary surrender, permanent dispute outcome, or expired redemption. | `PendingAuction` (Dutch reopen) or remains tombstoned. | Event `NameTombstoned` must include reason. |

State transitions MUST emit the corresponding `RegistryEventKind` so downstream caches stay coherent. Tombstoned names entering Dutch reopen auctions attach an `AuctionKind::DutchReopen` payload.

## 5. Canonical Events & Gateway Sync

Gateways subscribe to `RegistryEventV1` and synchronise to DNS/SoraFS by:

1. Fetching the latest `NameRecordV1` referenced by the event sequence.
2. Regenerating resolver templates (preferred IH58 + second-best compressed (`sora`) addresses, text records).
3. Pinning updated zone data via the SoraDNS workflow described in `docs/source/soradns/soradns_registry_rfc.md`.

Event delivery guarantees:

- Every transaction affecting a `NameRecordV1` *must* append exactly one event with a strictly increasing `version`.
- `RevenueSharePosted` events reference settlements emitted by `RevenueShareRecordV1`.
- Freeze/unfreeze/tombstone events include governance artefact hashes inside `metadata` for audit replay.

## 6. Example Norito Payloads

### 6.1 NameRecord Example

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "ih58...",
    controllers: [
        NameControllerV1 {
            controller_type: Account,
            account_address: Some(AccountAddress("0x020001...")),
            resolver_template_id: None,
            payload: {}
        }
    ],
    status: Active,
    pricing_class: 0,
    registered_at: 1_776_000_000,
    expires_at: 1_807_296_000,
    grace_expires_at: 1_809_888_000,
    redemption_expires_at: 1_815_072_000,
    auction: None,
    last_tx_hash: 0xa3d4...c001,
    metadata: { "resolver": "wallet.default", "notes": "SNS beta cohort" },
}
```

### 6.2 SuffixPolicy Example

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "ih58...",
    status: Active,
    payment_asset_id: "xor#sora",
    pricing: [
        PriceTierV1 { tier_id:0, label_regex:"^[a-z0-9]{3,}$", base_price:"120 XOR", auction_kind:VickreyCommitReveal, dutch_floor:None, min_duration_years:1, max_duration_years:5 },
        PriceTierV1 { tier_id:1, label_regex:"^[a-z]{1,2}$", base_price:"10_000 XOR", auction_kind:DutchReopen, dutch_floor:Some("1_000 XOR"), min_duration_years:1, max_duration_years:3 }
    ],
    min_term_years: 1,
    grace_period_days: 30,
    redemption_period_days: 60,
    max_term_years: 5,
    referral_cap_bps: 500,
    reserved_labels: [
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("ih58..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "ih58...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Next Steps

- **SN-2b (Registrar API & governance hooks):** expose these structs via Torii (Norito and JSON bindings) and wire admission checks to governance artefacts.
- **SN-3 (Auction & registration engine):** reuse `NameAuctionStateV1` to implement commit/reveal and Dutch reopen logic.
- **SN-5 (Payment & settlement):** leverage `RevenueShareRecordV1` for finance reconciliation and reporting automation.

Questions or change requests should be filed alongside the SNS roadmap updates in `roadmap.md` and mirrored in `status.md` when merged.
