---
lang: az
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e45957522f3ac3ab0d003af79dc75bee1a2bf3c16d3aa8b6926f4c2b50a524a1
source_last_modified: "2025-12-29T18:16:35.137714+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-fee-model
title: Nexus fee model updates
description: Mirror of `docs/source/nexus_fee_model.md`, documenting the lane settlement receipts and reconciliation surfaces.
---

:::note Canonical Source
This page mirrors `docs/source/nexus_fee_model.md`. Keep both copies aligned while Japanese, Hebrew, Spanish, Portuguese, French, Russian, Arabic, and Urdu translations migrate.
:::

# Nexus Fee Model Updates

The unified settlement router now captures deterministic per-lane receipts so
operators can reconcile gas debits against the Nexus fee model.

- For the full router architecture, buffer policy, telemetry matrix, and rollout
  sequencing see `docs/settlement-router.md`. That guide explains how the
  parameters documented here tie into the NX-3 roadmap deliverable and how SREs
  should monitor the router in production.
- Gas asset configuration (`pipeline.gas.units_per_gas`) includes a
  `twap_local_per_xor` decimal, a `liquidity_profile` (`tier1`, `tier2`,
  or `tier3`), and a `volatility_class` (`stable`, `elevated`, `dislocated`).
  These flags feed the settlement router so the resulting XOR
  quote matches the canonical TWAP and haircut tier for the lane.
- Every transaction that pays gas records a `LaneSettlementReceipt`.  Each
  receipt stores the caller-provided source identifier, the local micro-amount,
  the XOR due immediately, the XOR expected after the haircut, the realised
  variance (`xor_variance_micro`), and the block timestamp in milliseconds.
- Block execution aggregates receipts per lane/dataspace and publishes them
  via `lane_settlement_commitments` in `/v1/sumeragi/status`.  The totals
  expose `total_local_micro`, `total_xor_due_micro`, and
  `total_xor_after_haircut_micro` summed over the block for nightly
  reconciliation exports.
- A new `total_xor_variance_micro` counter tracks how much safety margin was
  consumed (difference between the due XOR and the post-haircut expectation),
  and `swap_metadata` documents the deterministic conversion parameters
  (TWAP, epsilon, liquidity profile, and volatility_class) so auditors can
  verify the quote inputs independent of runtime configuration.

Consumers can watch `lane_settlement_commitments` alongside the existing lane
and dataspace commitment snapshots to verify that fee buffers, haircut tiers,
and swap execution match the configured Nexus fee model.
