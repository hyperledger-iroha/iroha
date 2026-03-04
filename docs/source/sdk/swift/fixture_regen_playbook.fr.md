---
lang: fr
direction: ltr
source: docs/source/sdk/swift/fixture_regen_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7b38be680b9c11fbdfdc9d65a6cb3964c6ca248172bd66f51a6954bf9f7d883f
source_last_modified: "2026-01-03T18:08:01.457619+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Swift Norito fixture regeneration playbook -->

# Swift Norito Fixture Regeneration & Rollback (IOS2-WB1)

This playbook records the agreed cadence and evidence bundle for Swift Norito
fixture updates. It complements the cross-SDK cadence pre-read and keeps the
Swift-specific steps reproducible.

- **Cadence & owners:** 48 h SLA for governance-driven updates; weekly Wednesday
  17:00 UTC slot alternates Android Foundations (odd weeks) and Swift Lead (even
  weeks). Overrides are flagged via `SWIFT_FIXTURE_EVENT_TRIGGER=1`.
- **Source of truth:** Until the Rust exporter bundles Swift outputs directly,
  Swift mirrors the Android fixture set; `SWIFT_FIXTURE_ARCHIVE` can point at a
  signed archive from the exporter once available.
- **Evidence bundle:** every run must emit the cadence state, provenance
  manifest, and (when CI runs) the parity dashboard feeds so governance can
  audit what changed.

## Regeneration workflow

1. Run `scripts/swift_fixture_regen.sh` (defaults mirror Android resources) or
   point at an exporter archive:
   ```bash
   SWIFT_FIXTURE_ARCHIVE=artifacts/norito-fixtures.tar.gz \
   SWIFT_FIXTURE_APPROVER=swift-lead \
   SWIFT_FIXTURE_TICKET=IOS2-123 \
   SWIFT_FIXTURE_NOTES="governance discriminator bump" \
   ./scripts/swift_fixture_regen.sh
   ```
2. The script writes:
   - Updated fixtures under `IrohaSwift/Fixtures/`.
   - Cadence state at `artifacts/swift_fixture_regen_state.json` (rotation,
     slot window, trigger, roster, optional archive metadata).
   - Provenance manifest at `artifacts/swift_fixture_provenance.json` recording
     fixture/state SHA-256 digests, git revision, approver, change ticket, and
     archive details (see `scripts/swift_fixture_provenance.py`).
3. Run `ci/check_swift_fixtures.sh` or `make swift-dashboards` to enforce parity
   and cadence SLAs before opening a PR; CI wires the same validator.
4. Log the regen in `status.md` and attach the provenance manifest when handing
   off ownership to the next rotation.

## Rollback checklist

If a regen must be reverted:

1. Restore the previous fixture commit (or archive) and re-run
   `scripts/swift_fixture_regen.sh` with `SWIFT_FIXTURE_EVENT_REASON="rollback"`.
2. Attach the prior provenance manifest to the rollback PR and update
   `artifacts/swift_fixture_provenance.json` with a new entry that cites the
   rollback reason/ticket.
3. Re-run `ci/check_swift_fixtures.sh` to confirm parity and push the refreshed
   dashboards; capture the anomaly summary if CI lanes flake.
4. Record the rollback in `status.md` with links to both provenance manifests so
   governance can trace the change/rollback pair without replaying state.
