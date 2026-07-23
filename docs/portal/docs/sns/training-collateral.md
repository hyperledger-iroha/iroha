---
id: training-collateral
title: SNS Training Collateral
description: Curriculum and release-safe evidence capture for declarative alias provisioning.
---

> Mirrors `docs/source/sns/training_collateral.md`. The source guide is the
> canonical instructor runbook.

## Safety boundary

Training must use the canonical alias workflow: signed read-only planning,
local plan verification, one locally signed ordinary transaction, and an
authenticated readiness check. Removed SNS mutation routes, dashboard-derived
claims, client-generated payment proofs, raw API tokens, and private keys are
not valid training inputs or execution evidence.

## Curriculum snapshot

| Module | Exercise | Deliverable |
|--------|----------|-------------|
| Evidence orientation | Read a redacted readiness report, canonical plan, and committed transaction receipt. | Status/codes, plan hash, exact quote, cap, and debit identified. |
| Setup planning | Plan an ordered dataspace → domain → account-alias intent and re-encode its exact instruction frames. | Locally verified plan hash with no blocker. |
| Drift and visibility | Classify `NoOp`, `Repair`, `Create`, conflict, and restricted read responses. | Zero-charge replay/repair and correct 409/401/403/404 expectations. |
| Atomic apply | Sign and submit the exact plan as one normal transaction. | Transaction receipt and post-commit readiness report, with no partial apply. |

Use the [registrar API guide](./registrar-api.md),
[governance playbook](./governance-playbook.md),
[registry schema](./registry-schema.md), and
[operational evidence guide](./kpi-dashboard.md) as the curated references.

## Evidence packet

Archive the secret-free intent and canonical plan body, verified plan hash,
ordered frame digests, transaction result, exact quote/debit totals, and the
post-commit `AliasSetupReportV1`. On rejection, retain the structured error and
verify that resource, binding, index, permission, and balance state did not
partially change.

The plan cap is an upper bound, not the charged amount. Exact replays are
zero-charge `NoOp`s; derived-state repairs do not reacquire or extend a lease.

## Localization workflow

Translations live beside the source guide as
`docs/source/sns/training_collateral.<lang>.md`. Regenerate or review them after
the English source changes. A stale `source_hash` means the translation is not
an operational runbook. Preserve route paths, CLI names, diagnostic codes, and
Norito type names verbatim.

## Training assets

- Slide outline: `docs/examples/sns_training_template.md`.
- Workbook: `docs/examples/sns_training_workbook.md`.
- Invitation: `docs/examples/sns_training_invite_email.md`.
- Evaluation: `docs/examples/sns_training_eval_template.md`.

Keep runtime signer and token files outside shared training artifacts and apply
the generated localnet permissions (bundle directory `0700`, secret files
`0600`).
