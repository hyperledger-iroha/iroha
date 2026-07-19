---
title: SNS Training Collateral
summary: Instructor scripts, localization hooks, and release-safe evidence capture for alias provisioning.
---

# Sora Name Service training collateral

This curriculum teaches the first-release alias workflow: produce one
declarative intent, obtain a read-only plan against live state, verify the
canonical plan locally, sign one ordinary transaction, and confirm readiness.
It must not be adapted to call removed SNS mutation routes or to treat a
dashboard, client-generated payment proof, API token, or private key as
execution evidence.

The canonical references are:

- [`registrar_api.md`](./registrar_api.md) for planner, apply, renewal, and
  auto-renew behavior;
- [`registry_schema.md`](./registry_schema.md) for the persisted SNS read model;
- [`governance_playbook.md`](./governance_playbook.md) for approval and evidence
  boundaries;
- the [portal evidence guide](../../portal/docs/sns/kpi-dashboard.md) for the
  accepted operational evidence sources; and
- `fixtures/norito_rpc/alias_setup_v1/` for cross-SDK canonical fixtures.

## 1. Curriculum overview

### 1.1 Audience tracks

| Track | Objectives | Required pre-reads |
|-------|------------|--------------------|
| Registrar operations | Resolve textual names, review `NoOp`/`Repair`/`Create` dispositions, verify a plan, and submit its exact frames as one transaction. | `registrar_api.md`, `onboarding_kit.md`. |
| Node and gateway operations | Validate onboarding configuration, distinguish `Ready`/`Pending`/`Blocked`, and diagnose visibility without existence leakage. | `registrar_api.md`, portal evidence guide. |
| Governance and compliance | Approve intent, recognize drift conflicts, and retain a replayable evidence packet without secrets. | `governance_playbook.md`, `registry_schema.md`. |
| Finance and analytics | Reconcile exact planner quotes and committed ledger debits; distinguish the charged amount from the caller's cap. | `payment_settlement_plan.md`, portal evidence guide. |

### 1.2 Module sequence

| Module | Duration | Exercise | Exit criteria |
|--------|----------|----------|---------------|
| M1 — Evidence orientation | 30 min | Inspect a redacted `AliasSetupReportV1`, plan body, and committed transaction receipt. | Trainees can identify status, stable diagnostic codes, plan hash, exact quote, cap, and ledger debit. |
| M2 — Setup planning | 45 min | Plan a dataspace → domain → account-alias intent and independently decode/re-encode its ordered instruction frames. | The verified hash matches the planner hash and the plan contains no blocker. |
| M3 — Drift and visibility | 40 min | Classify an exact replay, a repair, a conflicting owner/binding, and restricted read responses. | Trainees expect zero-charge `NoOp`, charge-free `Repair`, structured 409 conflict, and the documented 401/403/404 order. |
| M4 — Atomic apply and evidence | 25 min | Locally sign and submit the exact plan as one normal transaction, then correlate readiness and ledger state. | One transaction receipt plus a post-commit readiness report proves the result; no partial apply is accepted. |

### 1.3 Lab prerequisites

1. Use a staging Torii with the alias planner and authenticated onboarding
   readiness endpoint enabled. Verify static/bootstrap configuration with
   `irohad --check-config` before the session.
2. Give each trainee a runtime-only client configuration whose signer matches
   the transaction authority. If sponsored onboarding is exercised, distribute
   its API token through a protected token file; never place token or key values
   in a workbook, shell history, URL, or HTTP body.
3. Seed secret-free setup intents and expected fixtures under the training
   artifact directory. Do not copy a live signer file into that directory.

## 2. Lab flow

### 2.1 Readiness and diagnostics

Run the alias doctor/readiness flow described by `registrar_api.md`. Record the
overall status, validation phase, stable diagnostic code, affected resource,
expected/actual values, and remediation. Before sharing the report, verify that
it contains no raw token, token digest, private-key material, or credential
header.

### 2.2 Read-only planning

Use a typed, secret-free intent. A representative bulk-tool invocation is:

```bash
python3 scripts/sns_bulk_onboard.py setup.json \
  --config client.toml \
  --plan-file setup.plan.json \
  --plan-only
```

The signed planner request must not mutate state. Review the authority,
chain/anchor, resolved text/ID pairs, dispositions, exact quotes, caps, totals
by asset, warnings/blockers, expiry, ordered framed instructions, and plan hash.
A conflict must return a structured 409 without a partial executable plan.

### 2.3 Local verification and apply

The CLI or SDK must verify the plan hash, decode and re-encode the exact frames,
locally sign one ordinary transaction, and submit it through the existing
transaction endpoint. Do not split a parent/child setup into separate
transactions. Do not substitute locally rebuilt instructions after verification.

### 2.4 Evidence packet

Archive only secret-free material:

- the approved intent and canonical plan body;
- the verified plan hash and ordered frame digests;
- the ordinary transaction hash and committed/rejected result;
- exact quoted and debited amounts by asset; and
- the post-commit `AliasSetupReportV1` plus authorized resolution results.

For a rejection, retain the structured error and prove that no earlier resource,
binding, index, permission, or balance mutation escaped the transaction.

## 3. Localization workflow

Localized handouts live beside this file as
`training_collateral.<lang>.md`. Regenerate or review translations after the
English source changes; a translation carrying an older `source_hash` is not an
operational runbook. Keep command names, route paths, diagnostic codes, and
Norito type names unchanged during translation.

## 4. Instructor checklist

- [ ] Every setup uses the signed read-only planner and one locally signed
      ordinary transaction.
- [ ] Exact replay demonstrates a zero-charge `NoOp`; derived-state repair is
      charge-free.
- [ ] Owner, binding, primary-role, immutable metadata/controller, and text/ID
      drift demonstrate structured conflicts, never overwrite behavior.
- [ ] Restricted reads demonstrate 401 before authentication, 403 before
      lookup for insufficient scope, and 404 only for an authorized miss.
- [ ] Sponsored onboarding is identified as the sole server-signing exception
      and uses a header/file token plus a stateless plan receipt.
- [ ] The shared evidence packet is redacted and contains no payment proof,
      raw token, inline key, or command-line secret.

## 5. Feedback

Use `docs/examples/sns_training_eval_template.md` for anonymous feedback. Store
completed workbooks and redacted evidence beneath the cohort artifact directory;
store runtime credentials separately with restrictive filesystem permissions.
