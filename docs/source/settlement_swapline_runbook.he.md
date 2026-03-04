---
lang: he
direction: rtl
source: docs/source/settlement_swapline_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 52ca7c593af6292e7a1ddd844e78ee7f866c00f8eb814195e1562e65ceca33e3
source_last_modified: "2026-01-03T18:07:57.418154+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Settlement Swapline Runbook (Repo / Reverse-Repo)

This runbook documents how to bring a treasury-backed swapline online, monitor
it, and unwind it safely. It complements the router spec in
`docs/source/settlement_router.md` and the deterministic metadata emitted via
`LaneSwapMetadata` (haircut tier, epsilon, TWAP window, volatility class).

## Prerequisites

- Swapline configuration checked in via governance:
  - `id`, `limit_xor`, `collateral_haircut_bps`, `fee_rate_bps`,
    `collateral_kind`, `uses_fee_schedule` (see
    `crates/settlement_router/src/swapline.rs`).
- Lane metadata publishes buffer account/asset and capacity so buffer gauges
  resolve (`settlement.buffer_*` keys).
- Telemetry and dashboards are live for:
  - `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`,
    `iroha_settlement_buffer_status`
  - `iroha_settlement_pnl_xor`, `iroha_haircut_bp`,
    `iroha_swapline_utilisation`
- Governance packet ready to archive evidence under `artifacts/settlement/`.

## Bring-up (Repo / Reverse-Repo)

1. **Validate parameters** – Confirm the swapline JSON matches on-ledger policy
   (limits, haircut, fee schedule). Reject if `limit_xor == 0` or haircut >100%.
2. **Load config** – Apply the swapline to the treasury automation. Record the
   hash of the config blob in the run log.
3. **Dry-run health check** – Call `SwapLineConfig::required_collateral` with a
   10% draw to compute required collateral. Post that collateral to the swapline
   account and assert `SwapLineExposure::is_healthy` in the dry-run harness.
4. **Activate** – Allow draws up to the governed `limit_xor`. Capture:
   - `SwapLineExposure` snapshot (outstanding, collateral)
   - `LaneSwapMetadata` sample (epsilon, TWAP window, haircut tier, volatility)
   - Telemetry scrape of the buffer + utilisation gauges
5. **Publish evidence** – Write the activation bundle to
   `artifacts/settlement/swaplines/<id>/activate_<timestamp>/` with:
   - Config blob + hash
   - Dry-run log
   - Telemetry scrape JSON + screenshot
   - `LaneBlockCommitment` excerpt showing attached `LaneSwapMetadata`

## Monitoring

- Alert on `settlement_buffer_status` in Alert/Throttle/XorOnly/Halt states.
- Alert when `iroha_swapline_utilisation` exceeds 80% of `limit_xor`.
- Track `iroha_haircut_bp` and `iroha_settlement_pnl_xor` for variance trends.
- During incident bridges, attach the latest `LaneSwapMetadata` to show the
  exact epsilon/TWAP/liquidity tier applied.

## Emergency unwind

1. **Freeze draws** – Set swapline limit to zero in the treasury automation and
   log the change hash.
2. **Repay outstanding** – Transfer XOR to bring `outstanding_xor` to zero.
   Verify `SwapLineExposure::is_healthy` returns `true` with zero utilisation.
3. **Release collateral** – Return posted collateral and record the tx hash.
4. **Archive evidence** – Store the unwind bundle under
   `artifacts/settlement/swaplines/<id>/unwind_<timestamp>/` with:
   - Pre/post exposure snapshots
   - Telemetry scrapes showing utilisation drop to zero
   - Operator log (who approved, timestamps)
5. **Restore config** – Remove the frozen swapline or reset limits to the
   governed defaults after governance approval. Record the new hash.

## Evidence checklist (attach to status/roadmap updates)

- Config hash + parameters
- Dry-run and activation logs (utilisation, collateral)
- Telemetry scrapes (buffer, utilisation, haircut, P&L)
- Lane commitment excerpt with `LaneSwapMetadata`
- Unwind log (if executed)
