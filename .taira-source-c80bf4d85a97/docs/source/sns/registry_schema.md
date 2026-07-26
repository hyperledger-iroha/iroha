---
title: Sora Name Service Registry Schema
summary: Actual V1 Norito read-model types used by the SNS registry.
---

# Sora Name Service registry schema

This document describes the V1 types currently defined in
`crates/iroha_data_model/src/sns/mod.rs`. It is a persisted/read-model reference,
not an HTTP mutation contract. Alias setup and lifecycle operations use the
signed planner and ordinary transactions described in
[`registrar_api.md`](./registrar_api.md).

The first release does not define `RevenueShareRecordV1`,
`RevenueAccrualEventV1`, `RegistryEventV1`, or `RegistryEventKind`. It also does
not publish storage-map key names or an append-only SNS event layout as a public
wire contract. Reporting must be derived from implemented policy/record reads,
canonical plans, committed transactions, and ledger state.

## 1. Selector and hash

`SuffixId` is a `u16`. The three fixed suffix IDs used for full alias leases are:

| Constant | Value | Resource |
|----------|-------|----------|
| `ACCOUNT_ALIAS_SUFFIX_ID` | `0x1001` | Account alias |
| `DOMAIN_NAME_SUFFIX_ID` | `0x1002` | Domain name |
| `DATASPACE_ALIAS_SUFFIX_ID` | `0x1003` | Dataspace alias |

`NameSelectorV1` contains exactly:

| Field | Type | Meaning |
|-------|------|---------|
| `version` | `u8` | Selector version; currently `1`. |
| `suffix_id` | `SuffixId` | Registered namespace identifier. |
| `label` | `String` | Canonical lowercase, NFC-normalized label. |

`NameSelectorV1::new` rejects an empty label and canonicalizes it with the
shared domain-label rules. `name_hash()` is deterministic BLAKE3 over the
version byte, the big-endian `u16` suffix ID, and the UTF-8 label bytes, in that
order. Consumers must call the shared constructor/hash implementation instead
of reproducing normalization or encoding heuristically.

## 2. `NameRecordV1`

The record contains exactly these fields, in this Norito layout order:

| Field | Type | Meaning |
|-------|------|---------|
| `selector` | `NameSelectorV1` | Canonical registered selector. |
| `name_hash` | `[u8; 32]` | Deterministic selector hash. |
| `owner` | `AccountId` | Current registration owner. |
| `controllers` | `Vec<NameControllerV1>` | Account/resolver/external controller descriptors. |
| `status` | `NameStatus` | Persisted lifecycle status. |
| `pricing_class` | `u8` | Registrar pricing tier identifier. |
| `registered_at_ms` | `u64` | Creation time in milliseconds since Unix epoch. |
| `expires_at_ms` | `u64` | Paid-term expiry. |
| `grace_expires_at_ms` | `u64` | Grace-window expiry. |
| `redemption_expires_at_ms` | `u64` | Redemption-window expiry. |
| `metadata` | `Metadata` | Registrar metadata and resolver hints. |
| `auction` | `Option<NameAuctionStateV1>` | Optional premium/Dutch-reopen state. |

There is no top-level `suffix_id`, `normalized_label`, `display_label`, or
`last_tx_hash` field. Suffix and normalized label are available through
`selector`; transaction evidence remains outside the record.

### 2.1 Status

`NameStatus` has five variants:

- `Active`
- `GracePeriod`
- `Redemption`
- `Frozen(NameFrozenStateV1 { reason: String, until_ms: u64 })`
- `Tombstoned(NameTombstoneStateV1 { reason: String })`

“Available” is derived from absence and is not a stored status.
“PendingAuction” is not a `NameStatus` variant; auction state is represented by
the optional `auction` field.

### 2.2 Controllers

`NameControllerV1` contains:

| Field | Type |
|-------|------|
| `controller_type` | `ControllerType` |
| `account_address` | `Option<AccountAddress>` |
| `resolver_template_id` | `Option<String>` |
| `payload` | `Metadata` |

`ControllerType` variants are `Account`, `Multisig`, `ResolverTemplate`, and
`ExternalLink`.

### 2.3 Auction state

`NameAuctionStateV1` contains:

| Field | Type |
|-------|------|
| `kind` | `AuctionKind` |
| `opened_at_ms` | `u64` |
| `closes_at_ms` | `u64` |
| `floor_price` | `TokenValue` |
| `highest_commitment` | `Option<[u8; 32]>` |
| `settlement_tx` | `Option<Json>` |

`AuctionKind` variants are `VickreyCommitReveal` and `DutchReopen`.
`settlement_tx` is stored audit metadata; it is not a client payment proof and
must not be supplied to alias setup or lifecycle instructions.

## 3. Pricing and suffix policy

`TokenValue` is `{ asset_id: String, amount: Quantity }`. `Quantity` is a
canonical non-negative amount in the asset's native precision; consensus must
not use host floating point.

`SuffixPolicyV1` fields, in layout order, are:

| Field | Type |
|-------|------|
| `suffix_id` | `SuffixId` |
| `suffix` | `String` |
| `steward` | `AccountId` |
| `status` | `SuffixStatus` |
| `min_term_years` | `u8` |
| `max_term_years` | `u8` |
| `grace_period_days` | `u16` |
| `redemption_period_days` | `u16` |
| `referral_cap_bps` | `u16` |
| `reserved_labels` | `Vec<ReservedNameV1>` |
| `payment_asset_id` | `String` |
| `pricing` | `Vec<PriceTierV1>` |
| `fee_split` | `SuffixFeeSplitV1` |
| `fund_splitter_account` | `AccountId` |
| `policy_version` | `u16` |
| `metadata` | `Metadata` |

`SuffixStatus` variants are `Active`, `Paused`, and `Revoked`.

`PriceTierV1` contains `tier_id`, `label_regex`, `base_price`, `auction_kind`,
`dutch_floor`, `min_duration_years`, and `max_duration_years`.
`ReservedNameV1` contains `normalized_label`, `assigned_to`, `release_at_ms`,
and `note`. `SuffixFeeSplitV1` contains `treasury_bps`, `steward_bps`,
`referral_max_bps`, and `escrow_bps`.

## 4. Alias setup relationship

External alias names are catalog-free typed values. The planner resolves static
catalog entries and active SNS records together and returns canonical text plus
the expected numeric `DataSpaceId`. Unknown mappings fail; matching
static/dynamic mappings are accepted; disagreement is
`alias.catalog.mapping_conflict`. Execution revalidates the same pair.

The setup planner orders `EnsureAlias` instructions as dataspace, then domain,
then account alias. Persisted records and every derived binding/index/capability
commit atomically with the transaction. Exact active state is a zero-charge
`NoOp`; missing derived state is a charge-free `Repair`; immutable or ownership
drift is a conflict and never an overwrite.

## 5. Read visibility

Visibility is derived from current dataspace/lane policy, not stored on each
record:

- public-dataspace aliases may resolve unsigned;
- known restricted dataspaces require canonical request authentication (401
  when missing/invalid);
- insufficient exact/applicable resolve permission returns 403 before lookup;
- an authorized missing alias returns 404; and
- list/index results omit invisible entries before totals and cursors are
  calculated.

Consumers should use typed query APIs and must not depend on internal world-state
map keys. Golden Norito/JSON fixtures for alias setup live under
`fixtures/norito_rpc/alias_setup_v1/` and are shared across Rust, Kotlin, Java,
and Swift.
