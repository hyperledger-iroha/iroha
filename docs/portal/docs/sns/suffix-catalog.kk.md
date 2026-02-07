---
lang: kk
direction: ltr
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffd062b69b97f11e5baa0ae82256c87cb76600982d599e1953573c1944112f51
source_last_modified: "2026-01-22T16:26:46.520176+00:00"
translation_last_reviewed: 2026-02-07
title: Sora Name Service Suffix Catalog
sidebar_label: Suffix catalog
description: Canonical allowlist of SNS suffixes, stewards, and pricing knobs for `.sora`, `.nexus`, and `.dao`.
---

# Sora Name Service Suffix Catalog

The SNS roadmap tracks every approved suffix (SN-1/SN-2). This page mirrors the
source-of-truth catalog so operators running registrars, DNS gateways, or wallet
tooling can load the same parameters without scraping status docs.

- **Snapshot:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **Consumers:** `iroha sns policy`, SNS onboarding kits, KPI dashboards, and
  DNS/Gateway release scripts all read the same JSON bundle.
- **Statuses:** `active` (registrations allowed), `paused` (temporarily gated),
  `revoked` (announced but not currently available).

## Catalog schema

| Field | Type | Description |
|-------|------|-------------|
| `suffix` | string | Human-readable suffix with leading dot. |
| `suffix_id` | `u16` | Identifier stored on-ledger in `SuffixPolicyV1::suffix_id`. |
| `status` | enum | `active`, `paused`, or `revoked` describing launch readiness. |
| `steward_account` | string | Account responsible for stewardship (matches registrar policy hooks). |
| `fund_splitter_account` | string | Account that receives payments before routing per `fee_split`. |
| `payment_asset_id` | string | Asset used for settlement (`xor#sora` for the initial cohort). |
| `min_term_years` / `max_term_years` | integer | Purchase term bounds from the policy. |
| `grace_period_days` / `redemption_period_days` | integer | Renewal safety windows enforced by Torii. |
| `referral_cap_bps` | integer | Maximum referral carve-out allowed by governance (basis points). |
| `reserved_labels` | array | Governance-protected label objects `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | array | Tier objects with `label_regex`, `base_price`, `auction_kind`, and duration bounds. |
| `fee_split` | object | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` basis-point split. |
| `policy_version` | integer | Monotonic counter incremented whenever governance edits the policy. |

## Current catalog

| Suffix | ID (`hex`) | Steward | Fund splitter | Status | Payment asset | Referral cap (bps) | Term (min – max years) | Grace / Redemption (days) | Pricing tiers (regex → base price / auction) | Reserved labels | Fee split (T/S/R/E bps) | Policy version |
|--------|------------|---------|---------------|--------|---------------|--------------------|--------------------------|---------------------------|----------------------------------------------|-----------------|-------------------------|----------------|
| `.sora` | `0x0001` | `ih58...` | `ih58...` | Active | `xor#sora` | 500 | 1 – 5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ → 120 XOR (Vickrey)` | `treasury → ih58...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `ih58...` | `ih58...` | Paused | `xor#sora` | 300 | 1 – 3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ → 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ → 4000 XOR (Dutch floor 500)` | `treasury → ih58...`, `guardian → ih58...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `ih58...` | `ih58...` | Revoked | `xor#sora` | 0 | 1 – 2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ → 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## JSON excerpt

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "ih58...",
      "payment_asset_id": "xor#sora",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "xor#sora", "amount": 120},
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

## Automation notes

1. Load the JSON snapshot and hash/sign it before distributing to operators.
2. Registrar tooling should surface the `suffix_id`, term limits, and pricing
   from the catalog whenever a request hits `/v1/sns/*`.
3. DNS/Gateway helpers read the reserved label metadata when generating GAR
   templates so DNS responses stay aligned with governance controls.
4. KPI annex jobs tag dashboard exports with suffix metadata so alerts match the
   launch state recorded here.
