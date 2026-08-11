<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
title: Sora Name Service Governance Playbook
summary: Release-safe approval, execution, and evidence boundaries for SNS and alias operations.
---

# Sora Name Service governance playbook

This playbook describes the governance evidence around alias provisioning. It
does not define a second mutation surface. In the first release the `iroha sns`
command tree is read-only (`registration` and `policy`). Commands such as
`sns governance`, `sns freeze`, `sns unfreeze`, and `sns auction` are not
registered operator APIs and must not appear in procedures, tickets, or
automation as executable actions.

Alias creation and repair use the signed read-only planner followed by one
locally signed ordinary transaction. Renewal and auto-renew changes use the
same plan/verify/apply model. Raw domain registration is reserved for genesis
or internal bootstrap. A governance decision may authorize an action, but it
does not itself mutate world state.

## 1. Roles and boundaries

| Role | Responsibility | Authoritative evidence |
|------|----------------|------------------------|
| Council / policy owner | Approve policy, reserved-name, or dispute decisions through the repository's established governance process. | Signed proposal/vote artifact and the exact policy revision it authorizes. |
| Guardian / incident lead | Coordinate containment and record an emergency decision. | Incident record, affected canonical resources, timestamps, and approvals. |
| Registrar operator | Request a canonical plan, verify it locally, sign one transaction, and submit it. | Intent, canonical plan body/hash, exact frames, and transaction receipt. |
| Onboarding operator | Maintain structurally enabled Torii onboarding config and protected credential/signer sidecars. | Redacted `AliasSetupReportV1`, config-check result, and file-permission audit. |
| Finance | Reconcile planner quotes with committed ledger debits. | Exact quote by asset, cap, payer authority, transaction result, and ledger delta. |
| Resolver / auditor | Perform visibility-authorized reads and verify post-commit state. | Authenticated readiness result and filtered query output. |

The transaction authority is the lease payer. Payer is never a request field;
resource owners remain explicit so an authorized operator may provision for
another account. Client-generated payment proofs and private keys in HTTP bodies
are prohibited.

## 2. Canonical evidence sources

Use these sources together:

- [`registrar_api.md`](./registrar_api.md) defines typed intents, signed
  planning, plan verification, ordinary transaction submission, renewal, and
  auto-renew planning.
- [`registry_schema.md`](./registry_schema.md) describes the actual persisted
  `NameRecordV1` read model.
- [`payment_settlement_plan.md`](./payment_settlement_plan.md) defines quote and
  settlement evidence.
- [`arbitration_toolkit.md`](./arbitration_toolkit.md) defines an off-chain,
  evidence-only dispute packet.
- public operational guidance is maintained at
  [docs.iroha.tech](https://docs.iroha.tech/); repository-local dashboards are
  not canonical execution evidence.

A dashboard or report may summarize canonical evidence, but it cannot prove an
apply. Retain the canonical plan body, plan hash, ordered framed instructions,
transaction hash/result, ledger delta, and post-commit readiness/read result.

## 3. Setup approval and execution

### 3.1 Approve the intent

Review the explicit owners, target account, account-provision mode, alias role,
lease acquisition term, and quote cap before requesting a plan. Textual
dataspaces, domains, and account aliases must resolve to their expected numeric
dataspace IDs. A static/dynamic mapping disagreement is a blocker, not a choice
for the operator.

### 3.2 Plan against live state

Submit the canonical signed request to `POST /v1/aliases/setup/plan`, normally
through the alias CLI or an SDK. Planning is read-only. The returned
`AliasTransactionPlanV1` must include:

- transaction authority, exact genesis-derived `NetworkId`, anchor, expiry,
  and canonical body hash;
- canonical resolved intents and ordered dataspace → domain → account frames;
- per-resource `NoOp`, `Repair`, or `Create` disposition;
- exact quote, payment asset, policy version, cap, and totals by asset; and
- warnings and blockers.

A conflict returns structured HTTP 409 and no partial executable plan. Do not
change ownership, binding, primary status, immutable metadata/controller, or a
text/ID mapping to make the plan pass.

### 3.3 Verify and apply locally

The CLI or SDK must verify the plan hash, decode and re-encode every exact frame,
sign one ordinary transaction locally, and submit it through the existing
transaction endpoint. Never split the resource vector or replace the planned
frames with locally reconstructed instructions.

Consensus revalidates names and live state, recomputes each quote, checks the
guard, and charges the exact calculated amount rather than the cap. The entire
instruction vector commits or rolls back as one transaction.

### 3.4 Verify post-commit state

Fetch authenticated onboarding readiness and the visibility-authorized alias
reads. A `Ready` report plus the committed transaction and ledger debit is the
operational proof. `Pending` is acceptable only for a joining node whose
bootstrap state is not yet available; known drift is `Blocked`, and must not be
auto-reconciled.

## 4. Idempotency and drift decisions

| Classification | Required behavior | Lease charge |
|----------------|-------------------|--------------|
| `NoOp` | Active resource and desired state already match. | None. |
| `Repair` | Only derived state is missing, such as an index, binding, or exact owner capability. | None. |
| `Create` | Resource is absent and every guard passes. | Exact recomputed acquisition amount, once. |
| Conflict | Owner, target, primary role, immutable metadata/controller, non-empty binding, or text/ID mapping differs. | No transaction plan; never overwrite. |

No-op and repair classification precedes lease quote checks. Re-running setup
does not extend a lease or create a second charge. Parent leases may end before
child leases; the planner reports a warning rather than silently changing a
term.

## 5. Lifecycle changes

- **Renewal:** use the renewal planner and `RenewAliasLease`, with
  expected-current-expiry CAS, an absolute target expiry, and the same quote
  guard. Verify and submit the returned frames as one ordinary transaction.
- **Rebind and primary alias:** use the registered CAS-based lifecycle
  instructions. They never accept `lease_expiry_ms`; a mismatch is a conflict.
- **Auto-renew:** plan and submit `ConfigureAliasAutoRenew` with the expected
  revision and an optional configuration. Native deterministic block processing
  performs later renewals. It debits the resource owner, retries insufficient
  funds, and suspends on policy/asset drift or repeated failure.
- **Disable auto-renew:** submit the same instruction with `config: None` and the
  expected revision.

Setup/readiness never enables or reconciles auto-renew implicitly. Subscription
domain/NFT auto-renew plumbing and client-generated payment proofs are not part
of this release.

## 6. Sponsored account onboarding

Sponsored onboarding is the sole server-signing exception:

1. Authenticate `POST /v1/accounts/onboard/plan` with a configured credential
   token supplied in a header from a protected token file.
2. Review the stateless receipt and its allowed ancillary instructions.
3. Submit that receipt to `POST /v1/accounts/onboard`.
4. Torii revalidates it and uses the configured signer to submit one atomic
   transaction containing `EnsureAlias::Account` and only the explicitly
   allowed onboarding instructions.

An exact repeat returns `Unchanged`; missing derived state may be repaired;
drift returns 409. Raw tokens and private keys never cross HTTP bodies and must
not be stored in plan/evidence files.

## 7. Disputes and emergency decisions

The release does not expose `sns governance case`, freeze/unfreeze, transfer, or
auction mutation commands. Record the decision and evidence with the
[arbitration toolkit](./arbitration_toolkit.md). If a registered typed lifecycle
instruction supports the approved remediation, plan or construct that
instruction through its documented API, obtain the required signatures, and
submit a normal transaction separately.

If no registered instruction supports the decision, report the operation as
`Blocked`. Do not emulate it through a stale Torii route, raw domain
registration, a direct storage write, or an undocumented CLI command.

## 8. Read visibility and non-disclosure

- Public-dataspace aliases may resolve without authentication.
- A known restricted dataspace requires canonical request authentication;
  missing or invalid authentication returns 401.
- An authenticated caller without exact `Alias` or applicable
  `Domain`/`Dataspace` resolve permission receives 403 before alias lookup.
- An authorized missing alias returns 404.
- By-account and index queries filter invisible entries before calculating
  totals and cursors.

Operational evidence must not reveal whether a restricted alias exists to an
unauthorized caller.

## 9. Release checklist

- [ ] `iroha3d --check-config` exits cleanly with aggregated diagnostics.
- [ ] Onboarding signer/authority agree; credential IDs and scopes are unique;
      signer/token files are protected and no secret appears inline.
- [ ] The signed plan is unexpired, blocker-free, and anchored to the intended
      chain/live state.
- [ ] The local verifier reproduces the plan hash and exact frame bytes.
- [ ] One ordinary transaction contains the complete ordered vector.
- [ ] The ledger debit equals the exact recomputed quote, not the cap.
- [ ] Replays demonstrate zero-charge no-op/repair behavior.
- [ ] Conflicts return structured 409 without partial plan or state mutation.
- [ ] Post-commit readiness and authorized reads match the approved intent.
- [ ] The evidence bundle is deterministic, redacted, and secret-free.

## 10. Public documentation

Public governance guides and translations are maintained in the sibling
`iroha-docs` repository and published at <https://docs.iroha.tech/>. Keep this
file focused on the implementation-coupled governance contract.
