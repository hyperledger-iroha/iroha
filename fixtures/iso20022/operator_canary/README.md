# ISO Operator Canary Runbooks

These checked-in runbooks are templates for `scripts/iso_operator_canary.py`.
They are intentionally non-secret examples that use `operator-canary.bank` HTTPS
template endpoints and relative paths resolved from this directory. Relative paths must
stay under the runbook directory; use absolute paths for intentionally external
operator directories. Endpoint URLs must not contain embedded credentials,
params, query strings, or fragments.

Validate a template without contacting Torii or a notary endpoint:

```bash
python3 scripts/iso_operator_canary.py \
  --config fixtures/iso20022/operator_canary/swift_cbpr_plus.preprod.example.json \
  --plan-only
```

Before a live canary, copy the relevant template into an operator runbook
location, replace endpoints and paths, place bearer tokens in runtime-only
files, populate the rail inbox with XML plus `*.xml.json` sidecars, and point
`notary.export_dir` at Torii's configured `iso_bridge.audit_export_dir`.
When running with `--require-explicit-policy`, keep list-valued runbook fields
explicit as arrays, including `verify.receipt_dirs: []` and
`verify.receipts: []` when receipt verification should use only generated stage
receipts. Also set `rail.receipt_dir` and `notary.receipt_dir` explicitly and
keep them separate from `rail.inbox_dir` and `notary.export_dir`; production
policy rejects receipt directories that overlap those source roots.
Receipt directories must also stay separate from configured rail/notary
bearer-token file paths before execution, so runtime-only token files are never
inside generated receipt roots or treated as receipt-root ancestors.
Bearer-token files must also stay outside `rail.inbox_dir` and
`notary.export_dir`; those source roots are for message/audit evidence only.
Direct rail/notary adapter runs additionally reject receipt directories that
overlap explicit rail XML/sidecar source files, rail/notary bearer-token files,
directories containing those token files, or notary anchor/index source files,
and reject bearer-token files under the source roots before source loading or
network delivery.
If `--summary-out` is supplied, the canary runner checks the summary target and
existing ancestors without creating missing parent directories before runbook
JSON parsing; after planning, the same output must still remain separate from
the config input and all planned stage artifacts before any child command is
executed.
Production runbooks must use real operator endpoints. Reserved placeholder
hosts such as `.example`, `example.com`, `example.net`, `example.org`, or
`example.invalid` are rejected before planning or network delivery, and archived
production evidence also rejects the checked-in `operator-canary.bank` template
endpoint suffix.
