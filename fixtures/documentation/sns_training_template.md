# SNS training slide template

Adapt this outline for each language cohort. Operational examples must stay on
the signed read-only planner, local verification, ordinary transaction, and
authenticated readiness path.

## Title slide

- Program: “Safe alias onboarding”
- Cohort and date
- Presenters and escalation contacts

## Canonical evidence

- `AliasSetupReportV1`: `Ready`, `Pending`, or `Blocked`, with stable redacted
  diagnostics.
- `AliasTransactionPlanV1`: authority, anchor, dispositions, resolved names,
  exact quotes, caps, ordered frames, expiry, and canonical plan hash.
- Committed transaction: exact submitted frames, transaction hash, and ledger
  debit.
- Explain why a dashboard, cap, client payment proof, or private key is not
  execution evidence.

## Declarative setup lifecycle

- Diagram: intent → signed planner → verify hash/frames → local signature → one
  ordinary transaction → readiness/read check.
- Ordered resources: dataspace, domain, account alias.
- `NoOp` and `Repair` classify before quote checks and never acquire or extend a
  lease.

## Drift and visibility drill

- A conflict returns structured 409 and no partial executable plan.
- Owner, binding, primary role, immutable metadata/controller, and text/ID
  mismatches are conflicts, never overwrites.
- Public reads may be unsigned; restricted reads enforce 401, then 403 before
  lookup, then 404 for an authorized miss.

## Atomic apply and evidence

- Verify and submit the planner's exact frames as one transaction.
- Archive the secret-free intent/plan, verified hash, transaction result, exact
  quote/debit, and post-commit readiness report.
- On rejection, prove no resource, binding, index, permission, or balance write
  escaped the transaction.

## Next steps

- Read `specs/sns/registrar_api.md` and
  `specs/sns/governance_playbook.md`.
- Complete `fixtures/documentation/sns_training_workbook.md`.
- Submit anonymous feedback via
  `fixtures/documentation/sns_training_eval_template.md`.
