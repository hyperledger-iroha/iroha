# Interim source-file budget diagnostic, 2026-09-24

The read-only `python3 scripts/check_source_file_budget.py` run on the dirty
`optimizations` checkout exited 1. It checked 12,909 files against 166 recorded
exceptions and reported 239 findings: 117 files grew beyond their exact
baselines and 122 files exceed the default production or test limit without an
exception. The findings span Core, SDKs, scripts, and vendored source. The
machine-readable local diagnostic is `target/source_budget_diagnostic.json`;
that generated file is not release evidence.
The checker unit suite, `pytest -q scripts/tests/check_source_file_budget_test.py`,
passes 51/51; the failure is in the checkout source inventory, not a failed
checker unit test.

This is a current-source integration failure, not a measurement of a frozen
release candidate. Owned-file growth needs source reduction or a justified,
reviewed exact exception; unchanged third-party source can receive an audited
no-growth exception. Re-run the checker on the final source. Do not regenerate
the exception baseline merely to erase current growth. The check does not
establish any privacy, SoraFS, or Native runtime behavior, and the release
packaging gate remains open.
