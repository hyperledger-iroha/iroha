---
lang: ru
direction: ltr
source: docs/source/sorafs/chunker_registry_charter.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d0f555294905d5ba7369b4d00e964d6472f20a9ed03d2103090ee00e6f20a87b
source_last_modified: "2026-01-04T10:50:53.654609+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS Chunker Registry Governance Charter

> **Ratified:** 2025-10-29 by the Sora Parliament Infrastructure Panel (see
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Any amendments require a
> formal governance vote; implementation teams must treat this document as
> normative until a superseding charter is approved.

This charter defines the process and roles for evolving the SoraFS chunker
registry. It complements the authoring guide
(`docs/source/sorafs/chunker_profile_authoring.md`) by defining the governance
contract for new entries, aliases, rollout evidence, and deprecation.

## Scope

The charter applies to every entry in `sorafs_manifest::chunker_registry` and to
any tooling that consumes the registry, including manifest CLIs, provider-advert
CLIs, gateways, and SDKs. It tracks the invariants checked by
`chunker_registry::ensure_charter_compliance()`:

- Profile IDs are positive integers that increase monotonically.
- The canonical handle `namespace.name@semver` must appear as the first alias.
- Alias strings are trimmed, unique, and do not collide with canonical handles of
  other entries.

## Roles

- **Author(s)** prepare the proposal, regenerate fixtures, and collect the
  determinism evidence.
- **Tooling Working Group (TWG)** validates proposals using the published
  checklists and confirms the registry invariants hold.
- **Governance Council (GC)** reviews the TWG report, signs the proposal
  envelope, and approves publication or deprecation timelines.
- **Storage Team** maintains the registry implementation and publishes
  documentation updates.

## Lifecycle Workflow

1. **Proposal submission**
   - Authors run the validation checklist from the authoring guide and create a
     `ChunkerProfileProposalV1` JSON under `docs/source/sorafs/proposals/`.
   - Include CLI output from:
     ```bash
     cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_car --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_car --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Submit a PR containing fixtures, proposal, determinism report, and registry
     updates.

2. **Tooling review (TWG)**
   - Replay the validation checklist: fixtures, fuzz, manifest, and PoR pipeline.
   - Run `cargo test -p sorafs_manifest chunker_registry` and ensure
     `ensure_charter_compliance()` passes with the new entry.
   - Verify CLI behavior (`--list-profiles`, `--promote-profile`, and
     `--json-out=-`) reflects the updated aliases and handles.
   - Produce a short report summarizing findings and pass/fail status.

3. **Council approval (GC)**
   - Review the TWG report and proposal metadata.
   - Sign the proposal digest (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     and append signatures to the council envelope maintained alongside the
     fixtures.
   - Record the vote outcome in the governance minutes.

4. **Publication**
   - Merge the PR, updating `crates/sorafs_manifest/src/chunker_registry.rs`,
     documentation, fixtures, and determinism reports.
   - Notify operators and SDK teams of the new profile and planned rollout.

5. **Deprecation / sunset**
   - Proposals that supersede an existing profile must include a dual-publish
     window and upgrade plan.
   - After the migration window, update the registry, migration ledger, and
     operator docs with the deprecation status.

6. **Emergency changes**
   - Removal or hotfixes require a council vote with majority approval.
   - TWG must document risk mitigation steps and update the incident log.

## Tooling Expectations

- `sorafs_manifest_chunk_store` and `sorafs_manifest_stub` expose:
  - `--list-profiles` for registry inspection.
  - `--promote-profile=<handle>` to generate the canonical metadata block used
    when promoting a profile.
  - `--json-out=-` to stream reports to stdout for reproducible review logs.
- `ensure_charter_compliance()` is invoked by registry-aware binaries. CI tests
  must fail if new entries violate the charter.

## Record Keeping

- Store determinism reports in `docs/source/sorafs/reports/`.
- Council minutes referencing chunker decisions live under
  `docs/source/sorafs/migration_ledger.md`.
- Update `roadmap.md` and `status.md` after each major registry change.

## References

- Authoring guide: `docs/source/sorafs/chunker_profile_authoring.md`
- Conformance checklist: `docs/source/sorafs/chunker_conformance.md`
- Registry reference: `docs/source/sorafs/chunker_registry.md`
