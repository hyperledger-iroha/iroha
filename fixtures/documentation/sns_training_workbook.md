# SNS training workbook template

Replace placeholders before distribution. Do not put a raw token, private key,
credential header, or client-generated payment proof in this workbook.

## Session details

- Cohort: `<name>`
- Network: `<staging network>`
- Language: `<language>`
- Facilitator: `<name>`

## Lab 1 — Readiness diagnostics

1. Validate the supplied node configuration with `iroha3d --check-config`.
2. Use the authenticated alias doctor/readiness flow documented in
   `specs/sns/registrar_api.md`.
3. Record the overall status: `Ready | Pending | Blocked`.
4. Record only redacted diagnostic fields: phase, stable code, severity,
   resource, config path, expected/actual values, and remediation.

Diagnostic notes: `________________________________________________________`

## Lab 2 — Read-only setup plan

1. Inspect the secret-free typed intent in `setup.json`.
2. Produce a plan without applying it:

   ```bash
   python3 scripts/sns_bulk_onboard.py setup.json \
     --config client.toml \
     --plan-file setup.plan.json
   ```

3. Record the authority, chain/anchor, ordered resource dispositions, exact
   quote totals, caps, expiry, warnings/blockers, and plan hash.
4. Decode and re-encode the exact framed instructions; verify the hash again.

Plan hash: `_______________________________________________________________`

## Lab 3 — Replay, drift, and read visibility

For each fixture, record the expected result:

| Scenario | Expected result |
|----------|-----------------|
| Exact active state | `NoOp`, zero charge |
| Missing derived index/capability | `Repair`, zero lease charge |
| Missing resource | `Create`, exact calculated charge |
| Owner/binding/primary/text-ID drift | Structured 409, no executable plan |
| Restricted read without valid authentication | 401 |
| Authenticated read without exact/applicable resolve scope | 403 before lookup |
| Authorized missing alias | 404 |

Conflict code and remediation: `___________________________________________`

## Lab 4 — Atomic apply and evidence

1. Apply the exact verified plan only with an explicit mutation flag:

   ```bash
   python3 scripts/sns_bulk_onboard.py setup.json \
     --config client.toml \
     --plan-file setup.plan.json \
     --apply
   ```

   It must locally sign one normal transaction and submit through the existing
   transaction endpoint.
2. Record the transaction hash and committed/rejected result.
3. Compare the exact planner quote with the committed ledger debit. Do not
   report the cap as the charge.
4. Fetch a post-commit readiness report. If rejected, verify that no earlier
   resource, binding, index, permission, or balance write is visible.

Transaction hash: `________________________________________________________`

Evidence packet path: `____________________________________________________`

## Feedback notes

- What was unclear?
- Which diagnostics or plan fields need better explanation?
- Tooling bugs observed?

Return only redacted workbooks and secret-free evidence to the facilitator.
Keep runtime signer/token files in their protected sidecar location.
