---
id: registry-schema
title: Sora Name Service Registry Schema
description: Actual V1 Norito read-model types used by the SNS registry.
---

:::note Canonical source
This page mirrors `docs/source/sns/registry_schema.md`, which is derived from
`crates/iroha_data_model/src/sns/mod.rs`.
:::

The registry schema is a persisted/read-model reference, not an HTTP mutation
contract. Provisioning and lifecycle changes use the signed planner and ordinary
transactions in the [registrar API guide](./registrar-api.md).

The first release does **not** define `RevenueShareRecordV1`,
`RevenueAccrualEventV1`, `RegistryEventV1`, or `RegistryEventKind`, and it does
not publish internal storage-map keys as a wire contract.

## Selector

`NameSelectorV1` contains `version: u8`, `suffix_id: u16`, and `label: String`.
Version is currently `1`; the shared constructor canonicalizes a non-empty label
with the domain-label rules. Its BLAKE3 hash covers, in order, the version byte,
big-endian suffix ID, and UTF-8 canonical label bytes.

Fixed lease suffix IDs are `0x1001` for account aliases, `0x1002` for domains,
and `0x1003` for dataspaces.

## `NameRecordV1`

The exact layout order is:

| Field | Type |
|-------|------|
| `selector` | `NameSelectorV1` |
| `name_hash` | `[u8; 32]` |
| `owner` | `AccountId` |
| `controllers` | `Vec<NameControllerV1>` |
| `status` | `NameStatus` |
| `pricing_class` | `u8` |
| `registered_at_ms` | `u64` |
| `expires_at_ms` | `u64` |
| `grace_expires_at_ms` | `u64` |
| `redemption_expires_at_ms` | `u64` |
| `metadata` | `Metadata` |
| `auction` | `Option<NameAuctionStateV1>` |

There is no top-level `suffix_id`, `normalized_label`, `display_label`, or
`last_tx_hash`. `NameStatus` variants are `Active`, `GracePeriod`,
`Redemption`, `Frozen(NameFrozenStateV1)`, and
`Tombstoned(NameTombstoneStateV1)`. Availability is derived from absence;
pending auction is represented by `auction`, not a status variant.

`NameControllerV1` contains `controller_type`, optional `account_address`,
optional `resolver_template_id`, and `payload`. Controller types are `Account`,
`Multisig`, `ResolverTemplate`, and `ExternalLink`.

`NameAuctionStateV1` contains `kind`, `opened_at_ms`, `closes_at_ms`,
`floor_price`, `highest_commitment`, and `settlement_tx`. Its auction kinds are
`VickreyCommitReveal` and `DutchReopen`. Stored settlement metadata is not a
client payment proof.

## `SuffixPolicyV1`

The exact layout order is:

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

`SuffixStatus` is `Active`, `Paused`, or `Revoked`. `TokenValue` contains an
asset-holding string and canonical non-negative `Quantity`; consensus uses no
host floating point.

## Alias setup and reads

The planner resolves canonical text with its expected `DataSpaceId`, orders
dataspace → domain → account `EnsureAlias` instructions, and rejects static/SNS
mapping conflicts. Exact state is a zero-charge `NoOp`; missing derived state is
a charge-free `Repair`; owner/binding/immutable drift is a conflict. All primary
and derived writes commit atomically.

Read visibility comes from current dataspace/lane policy: public reads may be
unsigned; restricted reads return 401 for missing/invalid authentication, 403
before lookup for insufficient scope, and 404 only for an authorized miss.
Invisible list entries are removed before totals and cursors.

Use typed queries rather than internal storage keys. Cross-SDK canonical fixtures
live under `fixtures/norito_rpc/alias_setup_v1/`.
