# Security audit reconciliation

The private audit ledger is intentionally not stored in this repository. Before
source freeze, place or export the complete archive beneath the checkout and run
the read-only reconciler against its checklist:

```text
python3 scripts/reconcile_security_audit.py \
  --ledger path/to/audit/TODO.md \
  --archive-root path/to/audit \
  --reports-root reports \
  --expected-total 561 \
  --expected-checked 550 \
  --expected-open 11 \
  --expected-reports 303
```

For final closure, set `--expected-checked 561 --expected-open 0`. Do that only
after every report has real terminal evidence. Externally dependent findings
remain unchecked and use the `externally-evidence-blocked` classification until
the required external evidence exists, so the release-audit reconciliation must
continue to fail while any such gate is unavailable.

Every linked report must declare non-placeholder `Classification`, `Status`,
and `Evidence` fields. Classification is closed to:

- `confirmed-source-work`
- `source-fixed-validation-pending`
- `rejected-with-code-path-proof`
- `externally-evidence-blocked`

The reconciler rejects missing or out-of-archive links, count drift, orphaned
reports when `--reports-root` is supplied, checked rows backed by non-terminal
reports, open rows backed by terminal reports, and one report linked from both
checked and open rows. It never edits the archive or changes a checkbox.
