<!-- Swift Norito fixture regeneration playbook -->

---
title: Swift Norito Fixture Regeneration and Rollback
summary: Canonical cadence, validation, evidence, and rollback steps for Swift fixtures.
---

# Swift Norito Fixture Regeneration & Rollback (IOS2-WB1)

This playbook records the agreed cadence and evidence bundle for Swift Norito
fixture updates. It complements the cross-SDK cadence pre-read and keeps the
Swift-specific steps reproducible.

- **Cadence & owners:** 48 h SLA for governance-driven updates; weekly Wednesday
  17:00 UTC slot alternates Android Foundations (odd weeks) and Swift Lead (even
  weeks). Cadence ownership is operational metadata and never changes the owner
  command or its outputs.
- **Source of truth:** `fixtures/norito_rpc/` is the only fixture owner. Swift
  receives generated copies of `transaction_payloads.json` and
  `transaction_fixtures.manifest.json` under `IrohaSwift/Fixtures/`; it does not
  mirror the canonical `.norito` blobs.
- **Evidence bundle:** every run must retain the canonical and descriptor-mirror
  diffs plus the Swift parity transcript so governance can audit what changed.

## Regeneration workflow

1. Run the canonical owner from the repository root:
   ```bash
   cargo run --locked -p xtask --features dev-tools --bin xtask -- \
     norito-rpc-fixtures --output-root /path/to/first-new-norito-rpc-publication
   cargo run --locked -p xtask --features dev-tools --bin xtask -- \
     norito-rpc-fixtures --output-root /path/to/second-new-norito-rpc-publication
   ```
   The output root is create-only; there are no SDK-specific, archive, or
   compatibility regeneration modes. Before any tracked update, require
   identical exact path sets, entry types, modes, completion manifests, and
   every file byte, then apply the reviewed identity-relative patch.
2. Review the canonical publication and all generated mirrors together. For
   Swift, the only owner-managed outputs are the payload descriptor and manifest;
   existing `swift_*.norito` test assets remain Swift-owned.
3. Run `python3 scripts/check_swift_fixtures.py` and
   `ci/check_swift_fixtures.sh` to enforce descriptor parity and reject copied
   canonical `.norito` blobs. Then run the Swift tests with the matching required
   native bridge.
4. Attach the owner-command diff and validation transcript when handing the
   cadence slot to the next operator.

## Rollback checklist

If a regen must be reverted:

1. Revert the authoritative descriptor change under `fixtures/norito_rpc/`.
   Never restore only `IrohaSwift/Fixtures/`, because it is generated output.
2. Re-run `cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-fixtures --output-root /path/to/first-new-norito-rpc-publication`, repeat at a second absent root, and require identical exact path sets, entry types, modes, completion manifests, and every file byte before applying the reviewed identity-relative patch so the canonical corpus and every managed mirror return to one coherent publication.
3. Re-run `python3 scripts/check_swift_fixtures.py` and
   `ci/check_swift_fixtures.sh`, then capture the rollback diff and validation
   transcript.
