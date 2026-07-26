---
title: Safe Alias and SNS Provisioning
sidebar_label: Alias setup API
description: Plan an exact alias dependency graph, sign it locally, and submit it atomically.
---

# Safe Alias and SNS Provisioning

Alias provisioning uses one declarative plan and one ordinary signed
transaction. Torii does not expose an operator apply endpoint and never accepts
a private key or client-generated payment proof.

## Canonical names

`merchant@banka.paynet` is a domain-qualified alias; `merchant@paynet` is a
dataspace-root alias. Resolved names bind canonical text to the expected
numeric dataspace ID. Static catalog and active SNS mappings must agree;
conflicts return `alias.catalog.mapping_conflict` and execution revalidates the
pair.

## Operator workflow

Send a canonically authenticated `AliasSetupPlanRequestV1` to
`POST /v1/aliases/setup/plan`. The request contains version `1` and the full
vector of `EnsureAlias` instructions. Dependencies are ordered dataspace,
domain, then account alias and are never split.

The response binds its authority/lease payer, chain and block anchor, resolved
intents, `NoOp`/`Repair`/`Create` dispositions, exact create quotes, cap guards,
asset totals, framed Norito instructions, deadline, diagnostics, and canonical
plan hash. Drift returns a structured `409` with no executable partial plan.
Every plan, including a pure no-op or repair plan, expires no more than 60
seconds after its committed anchor; an earlier create-quote deadline shortens
that lifetime. It never extends it.
An earlier parent-lease expiry is reported as the non-blocking
`alias.plan.parent_lease_expires_first` warning. A canonical unsigned payload
that already exceeds Torii's configured transaction limit receives the
blocking `alias.plan.transaction_oversized` diagnostic; clients refuse it and
the graph is never silently split.

```bash
iroha --config client.toml app alias setup plan \
  --intent-file setup.json --plan-file setup.plan.json

iroha --config client.toml app alias setup apply \
  --plan-file setup.plan.json
```

The CLI verifies the plan hash and every exact instruction frame, signs one
normal transaction locally, and submits it through the existing transaction
endpoint. Intent and plan files contain no secrets.

Classification precedes quote validation. Exact state is a free no-op; missing
derived state is repaired without a lease charge; an absent resource is
created and charged exactly once; ownership, binding, controller, metadata,
primary-role, lifecycle, or mapping drift is rejected. Replaying setup never
renews a lease.

## Onboarding and readiness

Sponsored account onboarding is the only server-signing exception and is
enabled by the presence of `[torii.account_onboarding]`. The signer comes from a
runtime-only key file. API tokens arrive in headers or token files while peer
configuration contains only BLAKE3 digests.

`GET /v1/accounts/onboarding/readiness` returns an authenticated, sorted,
secret-free setup report. `irohad --config peer.toml --check-config` validates
static configuration and available genesis without binding sockets. Missing
join/bootstrap state may report `Pending`; known state drift reports `Blocked`
without panicking the node.

## Resolution visibility

Public-dataspace aliases resolve unsigned. Restricted dataspaces require
canonical request authentication (`401` when missing or invalid) and an exact
Alias or applicable Domain/Dataspace resolve capability (`403` before lookup
when absent). Authorized missing aliases return `404`. Index and by-account
responses filter invisible entries before calculating totals or cursors.

Lease renewal and native auto-renew configuration are also read-only planning
flows. `POST /v1/aliases/lease/renew/plan` revalidates the absolute-expiry CAS
and exact guarded quote; `POST /v1/aliases/auto-renew/plan` revalidates the
owner, revision, ranges, policy version, and payment asset. Clients verify the
canonical plan hash and exact Norito frame, then locally sign one ordinary
transaction:

```bash
iroha --config client.toml app alias lease renew plan --intent-file renew.json --plan-file renew-plan.json
iroha --config client.toml app alias lease renew apply --plan-file renew-plan.json
iroha --config client.toml app alias auto-renew plan --intent-file auto.json --plan-file auto-plan.json
iroha --config client.toml app alias auto-renew apply --plan-file auto-plan.json
iroha --config client.toml app alias doctor --token-file onboarding.token
```

## Removed APIs

There are no direct SNS mutation routes, key-bearing renew/auto-renew bodies,
split acquire/bind setup calls, or multisig-specific onboarding endpoint. SNS
record and policy inspection remain read-only; raw domain registration is for
genesis/internal bootstrap only.
