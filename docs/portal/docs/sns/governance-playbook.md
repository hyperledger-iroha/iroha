<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
id: governance-playbook
title: Sora Name Service Governance Playbook
sidebar_label: Governance playbook
description: Release-safe approval, execution, and evidence boundaries for SNS and alias operations.
---

:::note Canonical source
This page mirrors `docs/source/sns/governance_playbook.md`. Regenerate
translations after the source changes; a stale translation is not an
operational runbook.
:::

## Safety boundary

The first-release `iroha sns` tree is read-only (`registration` and `policy`).
There is no supported `sns governance`, freeze/unfreeze, transfer, or auction
mutation command. A governance decision authorizes work; it does not mutate
world state.

Alias setup, renewal, and auto-renew changes use canonical signed planning,
local plan verification, and one locally signed ordinary transaction. Raw
domain registration is reserved for genesis/internal bootstrap. Client payment
proofs, inline private keys, and tokens in HTTP bodies are prohibited.

## Canonical workflow

1. Approve a secret-free typed intent with explicit resource owners and target
   account.
2. Send a canonical signed request to `POST /v1/aliases/setup/plan` through the
   alias CLI or an SDK. Planning is read-only.
3. Review authority, chain/anchor, resolved text/ID pairs, dispositions, exact
   quotes, caps, warnings/blockers, ordered frames, expiry, and plan hash.
4. On conflict, stop: HTTP 409 contains structured details and no partial
   executable plan.
5. Verify the hash and decode/re-encode the exact frames locally.
6. Sign one ordinary transaction locally and submit it through the existing
   transaction endpoint. Never split or rebuild the vector.
7. Retain the transaction result, ledger delta, and authenticated post-commit
   `AliasSetupReportV1`.

Consensus charges the exact recomputed amount rather than the cap. An exact
replay is a zero-charge `NoOp`; missing derived state is a charge-free `Repair`;
setup never extends a lease. Owner, binding, primary, immutable metadata, or
text/ID drift is a conflict and is never overwritten.

## Lifecycle operations

- Renewal uses `RenewAliasLease` with expected-current-expiry CAS, an absolute
  target expiry, and a quote guard.
- Rebind and primary-alias changes use their registered CAS instructions and
  never accept `lease_expiry_ms`.
- `ConfigureAliasAutoRenew` uses expected revision and an optional config;
  `None` disables it. Native block processing performs configured renewals.

Each operation follows the same plan/verify/apply evidence model. Setup and
readiness never enable auto-renew implicitly.

## Sponsored onboarding

Sponsored onboarding is the sole server-signing exception. A protected token
file supplies the authentication header for `POST /v1/accounts/onboard/plan`.
The returned stateless receipt is revalidated by
`POST /v1/accounts/onboard`, whose configured Torii signer submits one atomic,
strictly bounded onboarding transaction. Exact repeat returns `Unchanged`,
repairable derived state is repaired, and drift returns 409.

## Disputes and emergency decisions

Use the canonical source's arbitration toolkit to record an evidence-only case.
If a registered typed instruction supports approved remediation, execute it as
a separately authorized normal transaction through its documented surface. If
no registered instruction exists, report `Blocked`; do not revive a removed
route or invent a CLI command.

## Visibility

Public-dataspace aliases may resolve unsigned. Restricted dataspaces return 401
for missing/invalid canonical authentication, 403 before lookup when an
authenticated caller lacks exact/applicable resolve scope, and 404 only for an
authorized miss. Filter invisible index entries before totals and cursors.

## Evidence checklist

- Approved secret-free intent and canonical plan body/hash
- Exact ordered frame bytes and local verification result
- Ordinary transaction hash and committed/rejected result
- Exact quotes, caps, totals, and committed ledger debit by asset
- Redacted post-commit readiness and authorized reads
- Structured conflict/rejection evidence proving no partial state change

Use the [registrar API](./registrar-api.md),
[registry schema](./registry-schema.md),
[payment settlement plan](./payment-settlement-plan.md), and
[operational evidence guide](./kpi-dashboard.md) for details. A dashboard is a
summary only; it is never apply evidence.
