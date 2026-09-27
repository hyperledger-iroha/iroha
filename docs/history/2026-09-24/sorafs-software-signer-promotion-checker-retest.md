# SoraFS software signer and promotion checker local retest — 2026-09-24

On the required `optimizations` checkout, the following focused local tests passed:

```text
python3 -m pytest -q scripts/tests/sorafs_software_signer_launcher_test.py  # 18 passed
python3 -m pytest -q scripts/tests/check_sorafs_production_promotion_bundle_test.py  # 200 passed
```

The launcher supports authenticated software custody without an HSM prerequisite.
The promotion checker still fails closed on the missing purpose-owned native
completed-operation and current-State proofs for foundational, topology,
resilience, and lane-inventory approvals. These tests exercise the local checker
and negative controls; they are not operator signatures, finalized operation
receipts, a frozen candidate, or promotion evidence. The corresponding SoraFS
release gates remain open.
