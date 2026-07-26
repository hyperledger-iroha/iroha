---
title: Alias provisioning operational evidence
description: Safe dashboard inputs derived from canonical alias plans, readiness reports, and committed transactions.
---

# Alias Provisioning Operational Evidence

Alias dashboards are read-only projections. They must derive every financial
or lifecycle claim from a canonical `AliasTransactionPlanV1`, a committed
ordinary transaction, or a sorted `AliasSetupReportV1`. Dashboard data never
authorizes a lease, proves payment, or changes world state.

:::warning Legacy dashboard queries
`dashboards/grafana/sns_suffix_analytics.json` still contains retired
bulk-release and fabricated-settlement series. Do not import it or use it as
financial evidence until those queries are regenerated from the sources
below.
:::

## Safe panels

| Panel | Source | Required correlation |
|-------|--------|----------------------|
| Setup dispositions | Planner response and structured conflict report | Plan hash, authority, chain, anchor, resource, and NoOp/Repair/Create/Conflict disposition. |
| Native lease charges | Exact quote plus committed ledger debit | Transaction hash, payment asset, policy version, exact amount, and quote cap. |
| Onboarding readiness | Authenticated `AliasSetupReportV1` snapshots | Overall status, validation phase, stable code, severity, resource, and redacted config path. |
| Renewal and auto-renew | Verified lifecycle plan and transaction result | Expected revision/expiry, target expiry, retry count, suspension reason, and final status. |
| Read authorization | Torii request status grouped by public/restricted dataspace | Authentication outcome without alias-existence leakage. |

Compute counts, totals, and cursors after applying the same visibility policy as
the read API. Never reconstruct hidden aliases from logs or count rejected
lookups as evidence that an alias exists.

## Review checklist

1. Verify each financial sample joins an exact quote to one committed
   transaction and ledger debit.
2. Confirm no dashboard importer accepts a payment proof, settlement bundle,
   private key, raw token, or per-resource mutation receipt.
3. Reconcile replayed setup as a zero-charge no-op and repair as a zero-charge
   derived-state correction.
4. Record policy/asset drift and repeated auto-renew failure as suspension, not
   a silently retried charge under changed terms.
5. Archive only secret-free plan hashes, reports, transaction hashes, and
   aggregate exports.
