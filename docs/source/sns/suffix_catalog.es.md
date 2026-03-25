---
lang: es
direction: ltr
source: docs/source/sns/suffix_catalog.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ea7fafb8e00c9effdf6f09b912d1d0c810cb9de2866b9b01f47003b02afdfd35
source_last_modified: "2026-01-22T07:40:18.736078+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
title: Sora Name Service Suffix Catalog
summary: Canonical allowlist of SNS suffixes, steward assignments, and commercial parameters referenced by SN-1/SN-2.
---

# Sora Name Service Suffix Catalog (SN-1 / SN-2)

Governance tracks every approved suffix in a deterministic catalog so registrars,
DNS operators, wallets, and auditors can prove which namespaces are active and
how they are configured. This document publishes the authoritative allowlist
referenced throughout the roadmap’s Suffix Catalog section and links it to the
machine-readable snapshot consumed by CLI tooling and dashboards.

- **Scope.** `.sora`, `.nexus`, and `.dao` constitute the initial catalog.
  Additional entries follow the same format once the governance vote concludes.
- **Source of truth.** The catalog is exported as Norito-friendly JSON under
  `docs/examples/sns/suffix_catalog_v1.json`; scripts ingesting the catalog must
  hash/sign the JSON when distributing to operators.
- **Consumers.** `iroha sns policy`, `cargo xtask sns-scorecard`, resolver/DNS
  automation, onboarding kits, and KPI dashboards reference the same metadata so
  suffix changes do not require bespoke spreadsheets.

## Catalog schema

| Field | Type | Description |
|-------|------|-------------|
| `suffix` | string | Human-readable suffix with leading dot. |
| `suffix_id` | `u16` | Identifier stored on-ledger in `SuffixPolicyV1::suffix_id`. |
| `status` | enum | `active`, `paused`, or `revoked` describing launch readiness. |
| `steward_account` | string | Account responsible for stewardship (matches registrar policy hooks). |
| `fund_splitter_account` | string | Account that receives payments before routing per `fee_split`. |
| `payment_asset_id` | string | Asset used for settlement (`61CtjvNd9T3THAR65GsMVHr82Bjc` for the initial cohort). |
| `min_term_years` / `max_term_years` | integer | Purchase term bounds from the policy. |
| `grace_period_days` / `redemption_period_days` | integer | Renewal safety windows enforced by Torii. |
| `referral_cap_bps` | integer | Maximum referral carve-out allowed by governance (basis points). |
| `reserved_labels` | array | Governance-protected label objects `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | array | Tier objects with `label_regex`, `base_price`, `auction_kind`, and duration bounds. |
| `fee_split` | object | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` basis-point split. |
| `policy_version` | integer | Monotonic counter incremented every time governance edits the policy. |

> **Tip:** When generating Norito fixtures or CLI drives, load the JSON snapshot
> and emit the `suffix_id`/pricing tuple alongside the registrar payload so the
> review queue can verify the submission without re-scraping docs.

## Current catalog

| Suffix | ID (`hex`) | Steward | Fund splitter | Status | Payment asset | Referral cap (bps) | Term (min – max years) | Grace / Redemption (days) | Pricing tiers (regex → base price / auction) | Reserved labels | Fee split (T/S/R/E bps) | Policy version |
|--------|------------|---------|---------------|--------|---------------|--------------------|--------------------------|---------------------------|----------------------------------------------|-----------------|-------------------------|----------------|
| `.sora` | `0x0001` | `i105...` | `i105...` | Active | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1 – 5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ → 120 XOR (Vickrey)` | `treasury → i105...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `i105...` | `i105...` | Paused | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 1 – 3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ → 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ → 4000 XOR (Dutch floor 500)` | `treasury → i105...`, `guardian → i105...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `i105...` | `i105...` | Revoked | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1 – 2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ → 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

All fields map directly to `SuffixPolicyV1`. The pricing column condenses the
`pricing` array for readability; automation must consume the JSON snapshot to
retrieve the exact regex, tier IDs, and auction parameters.

## JSON snapshot & automation hooks

- **File:** [`docs/examples/sns/suffix_catalog_v1.json`](../../examples/sns/suffix_catalog_v1.json)
- **Versioning:** The `version` field increments every time governance ratifies
  a new suffix or edits an existing one. `generated_at` follows RFC 3339.
- **Validation:** `cargo xtask sns-catalog-verify` validates every
  `suffix_catalog_*.json` snapshot, ensuring suffix ids/tier ids stay unique,
  fields stay within policy bounds, and the accompanying checksum manifest
  (e.g., `docs/examples/sns/suffix_catalog_v1.sha256`) matches the file bytes.
  CI runs this command so drift is detected before publishing scorecards,
  annexes, or registrar docs.

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "i105...",
      "payment_asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc", "amount": 120},
          "auction_kind": "vickrey_commit_reveal",
          "min_duration_years": 1,
          "max_duration_years": 5
        }
      ],
      "...": "see docs/examples/sns/suffix_catalog_v1.json for the full record"
    }
  ]
}
```

## CLI, registrar, and DNS usage

1. **Registrar / CLI.** `iroha sns policy --suffix-id <id>` now references the
   catalog when printing steward, asset, and pricing information so reviewers can
   confirm governance parameters before approving a registration.
2. **DNS / Gateway.** `scripts/sns_zonefile_skeleton.py` loads the catalog to
   stamp explicit reserved labels and suffix IDs into GAR templates.
3. **Dashboards.** `dashboards/grafana/sns_suffix_analytics.json` tags metrics
   with the suffix metadata from this catalog so suffix-specific alerts remain
   in sync with governance-approved values.

When proposing new suffixes, update this catalog (Markdown + JSON) within the
same change as the governance charter addendum so reviewers, registrars, and
automation pick up the change atomically.
