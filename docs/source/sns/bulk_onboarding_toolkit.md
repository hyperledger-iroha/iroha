---
title: Atomic Alias Setup Toolkit
summary: Use typed setup intents and the signed planner for bulk alias provisioning.
---

# Atomic Alias Setup Toolkit

`scripts/sns_bulk_onboard.py` is a secret-free orchestration wrapper around the
Rust client and CLI. It validates a grouped alias setup document, asks Torii to
plan the complete dependency graph through a canonical signed request, verifies
the returned plan, and optionally submits its exact frames as one ordinary
transaction.

The tool does not use suffix tables, synthesize settlement proofs, accept raw
tokens or private keys, call retired SNS mutation routes, or split a batch into
per-resource writes.

## Input

The input is JSON with `schema_version: 1` and three dependency groups:

```json
{
  "schema_version": 1,
  "dataspaces": [],
  "domains": [],
  "accounts": [
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

Each entry is the exact JSON shape of `EnsureAlias`. The wrapper concatenates
`dataspaces`, `domains`, and `accounts` in that order into
`AliasSetupPlanRequestV1.intents`. Duplicate resources and unknown fields fail
locally before a request is signed.

## Plan only

```bash
python3 scripts/sns_bulk_onboard.py setup.json \
  --config client.toml \
  --iroha-cli ./target/release/iroha \
  --plan-file setup.plan.json \
  --plan-only
```

The wrapper invokes:

```text
iroha --config client.toml app alias setup plan \
  --intent-file <protected-temporary-file> \
  --plan-file setup.plan.json
```

The CLI canonically signs the planner request, verifies the returned plan hash
and frames, and writes the plan with mode `0600` on Unix. Subprocess output is
not copied into receipts, avoiding accidental exposure of unrelated client
configuration.

## Atomic apply

Omit `--plan-only` to submit the verified plan:

```bash
python3 scripts/sns_bulk_onboard.py setup.json \
  --config client.toml \
  --iroha-cli ./target/release/iroha \
  --plan-file setup.plan.json
```

Apply verifies the plan again and submits the entire framed instruction vector
in one signed transaction. A planner conflict returns no plan and nothing is
submitted. Exact replays yield free no-op resources; repairs emit only the
missing derived state; create resources carry the exact policy quote and cap.

## Secret handling

Intent and plan files must not contain credentials, raw tokens, signatures,
private key fields, payer fields, payment proofs, or lease-expiry setters. The
wrapper has no command-line token/key value option. Transaction signing uses
the normal runtime-only key configuration referenced by `client.toml`.

Sponsored account onboarding uses its separate API-token-authenticated
plan/receipt flow. Pass onboarding tokens in headers or token files only; do
not put them in this setup document.
