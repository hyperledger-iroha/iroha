---
title: Safe Alias and SNS Provisioning
summary: Declarative planning, atomic local signing, sponsored onboarding, and visibility rules for alias resources.
---

# Safe Alias and SNS Provisioning

Alias setup is a plan-then-sign workflow. Torii never accepts an operator
private key or a client-created settlement proof, and it does not expose an
operator setup/apply endpoint. A client asks Torii to classify one complete
intent against live state, verifies the returned canonical plan locally, then
submits every planned instruction in one ordinary signed transaction.

## Names and dependency order

External names do not depend on a client-side dataspace catalog:

- `merchant@banka.paynet` is label `merchant`, domain `banka`, dataspace
  `paynet`.
- `merchant@paynet` is a dataspace-root account alias.
- Resolved dataspace, domain, and account-alias values carry canonical text and
  the expected numeric `DataSpaceId`.

Resolution combines the static catalog with active SNS records. Matching
static and dynamic mappings are accepted. Unknown mappings fail. Conflicting
mappings fail with `alias.catalog.mapping_conflict`; execution revalidates the
same text/ID pair.

A setup request contains an ordered vector of `EnsureAlias` instructions. Its
dependency order is dataspace, domain, then account alias. Torii may normalize
the order but never splits the vector into separate transactions.

## Planning and apply

`POST /v1/aliases/setup/plan` is read-only and requires canonical request
authentication. The signed JSON body is `AliasSetupPlanRequestV1`:

```json
{
  "schema_version": 1,
  "intents": [
    {
      "intent": {
        "kind": "account_alias",
        "intent": {
          "alias": {
            "canonical_name": {
              "label": "merchant",
              "domain": "banka",
              "dataspace": "paynet"
            },
            "dataspace_id": 7
          },
          "target_account": "<canonical-domainless-account-id>",
          "provision": {"kind": "create", "value": null},
          "role": {"kind": "primary", "value": null}
        }
      },
      "acquisition": {"term_years": 1, "pricing_class_hint": null},
      "quote_guard": {
        "expected_policy_version": 1,
        "expected_payment_asset": "<canonical-asset-definition-id>",
        "max_amount": "1000",
        "valid_until_ms": 1900000000000
      }
    }
  ]
}
```

The transaction authority is the signed request account and is always the
lease payer; payer is not part of an intent. Resource owners and target
accounts remain explicit.

`AliasTransactionPlanV1` binds the authority, exact genesis-derived
`NetworkId`, block anchor, ordered
resolved resources, dispositions, exact quotes, framed Norito instructions,
totals by payment asset, diagnostics, deadline, and a domain-separated hash of
the canonical plan body. The planner returns a structured `409` and no partial
executable plan if any resource conflicts.

Every setup plan has a finite lifetime of at most 60 seconds from its committed
block anchor, including pure no-op and repair plans. A create quote guard with
an earlier deadline shortens the plan lifetime; it never extends it.

If a parent dataspace or domain lease expires before its planned child, the
plan carries the stable `alias.plan.parent_lease_expires_first` warning; setup
remains valid because parent and child terms are intentionally independent.
If the canonical unsigned transaction payload already exceeds Torii's
configured transaction body limit, the plan carries the blocking
`alias.plan.transaction_oversized` diagnostic. Clients reject blocked plans,
and neither Torii nor the CLI silently splits the requested graph.

Use the CLI to verify and apply the exact frames:

```bash
iroha --config client.toml app alias setup plan \
  --intent-file setup.json \
  --plan-file setup.plan.json

iroha --config client.toml app alias setup apply \
  --plan-file setup.plan.json
```

`apply` checks the plan hash, authority, chain, deadline, dependency order,
totals, quote guards, instruction indices, and decode/re-encode equality. It
then signs and submits one normal transaction through the existing transaction
endpoint. Intent and plan files are secret-free.

## Deterministic dispositions

Classification happens before lease quote validation:

- `NoOp`: the active resource and desired state already match. The exact
  `EnsureAlias` frame is retained for apply-time revalidation, but it performs
  no mutation and incurs no lease charge.
- `Repair`: only exact derived state is missing, such as an index, binding,
  primary marker, or owner capability. Repair is emitted without a lease
  charge.
- `Create`: the lease-bearing resource is absent. Consensus recomputes the
  quote and charges the exact amount once, never the cap.
- `Conflict`: ownership, target, primary role, controller, immutable metadata,
  a non-empty binding, lifecycle state, or text/ID mapping differs. Existing
  authoritative state is never overwritten.

Replaying exact setup is therefore free and does not extend a lease. Lease
renewal, account-alias rebind, primary-alias changes, and auto-renew
configuration are separate compare-and-set lifecycle operations.

Renewal and auto-renew use the same plan-then-locally-sign model as setup:

- `POST /v1/aliases/lease/renew/plan` accepts a canonical-request-signed
  `AliasLeaseRenewPlanRequestV1`. It revalidates the text/ID pair, owner or
  exact manage capability, expected-current-expiry CAS, absolute target
  expiry, policy/payment asset, cap, and deadline. The response commits the
  exact quote and framed `RenewAliasLease` instruction.
- `POST /v1/aliases/auto-renew/plan` accepts a canonical-request-signed
  `AliasAutoRenewPlanRequestV1`. It revalidates the exact owner, revision,
  ranges, policy version, and payment asset. Exact clean configuration is a
  zero-charge no-op; a change returns one framed
  `ConfigureAliasAutoRenew` instruction.

The CLI keeps both intent and plan files secret-free and applies each verified
plan through the ordinary transaction endpoint:

```bash
iroha --config client.toml app alias lease renew plan \
  --intent-file renew.json --plan-file renew-plan.json
iroha --config client.toml app alias lease renew apply \
  --plan-file renew-plan.json

iroha --config client.toml app alias auto-renew plan \
  --intent-file auto-renew.json --plan-file auto-renew-plan.json
iroha --config client.toml app alias auto-renew apply \
  --plan-file auto-renew-plan.json
```

## Sponsored onboarding

Sponsored onboarding is the sole server-signing exception. It is enabled only
when `[torii.account_onboarding]` is present. Runtime credentials are supplied
in an HTTP header; configuration stores only BLAKE3 token digests. The
onboarding signer is loaded from `private_key_file`; neither raw API tokens nor
private keys are accepted in configuration values, request bodies, plan files,
or command-line values.

`GET /v1/accounts/onboarding/readiness` is authenticated and returns a sorted,
secret-free `AliasSetupReportV1`. Static configuration can be checked without
opening sockets:

```bash
iroha3d --config peer.toml --check-config
iroha --config client.toml app alias doctor --token-file onboarding.token
```

Absence of local bootstrap state may be reported as `Pending`. Known live
world-state drift reports `Blocked` without panicking or stopping an otherwise
healthy node.

## Read visibility

Alias visibility comes from current dataspace/lane policy, not from an alias
flag:

- aliases in a public dataspace may be resolved unsigned;
- a known restricted dataspace with missing or invalid canonical request
  authentication returns `401`;
- an authenticated caller without exact alias or applicable domain/dataspace
  resolve permission receives `403` before lookup;
- an authorized missing alias returns `404`.

By-account and index results filter invisible entries before totals and cursors
are calculated, so restricted alias existence is not leaked.

## Retired surfaces

The first-release API has no direct `/v1/sns/names` mutation routes, no
multisig-specific onboarding route, no split acquire/bind setup API, and no
key-bearing renewal or auto-renew body. SNS policy and record inspection remain
read-only. Raw domain registration is reserved for genesis and internal
bootstrap.
