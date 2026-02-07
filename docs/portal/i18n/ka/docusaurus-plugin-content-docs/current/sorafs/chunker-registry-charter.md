---
id: chunker-registry-charter
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Charter
sidebar_label: Chunker Registry Charter
description: Governance charter for chunker profile submissions and approvals.
---

:::note Canonical Source
:::

# SoraFS Chunker Registry Governance Charter

> **Ratified:** 2025-10-29 by the Sora Parliament Infrastructure Panel (see
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Any amendments require a
> formal governance vote; implementation teams must treat this document as
> normative until a superseding charter is approved.

This charter defines the process and roles for evolving the SoraFS chunker
registry. It complements the [Chunker Profile Authoring Guide](./chunker-profile-authoring.md) by describing how new

## Scope

The charter applies to every entry in `sorafs_manifest::chunker_registry` and
to any tooling that consumes the registry (manifest CLI, provider-advert CLI,
SDKs). It enforces the alias and handle invariants checked by
`chunker_registry::ensure_charter_compliance()`:

- Profile IDs are positive integers that increase monotonically.
- The canonical handle `namespace.name@semver` **must** appear as the first
- Alias strings are trimmed, unique, and do not collide with canonical handles
  of other entries.

## Roles

- **Author(s)** – prepare the proposal, regenerate fixtures, and collect the
  determinism evidence.
- **Tooling Working Group (TWG)** – validates the proposal using the published
  checklists and ensures the registry invariants hold.
- **Governance Council (GC)** – reviews the TWG report, signs the proposal
  envelope, and approves publication/deprecation timelines.
- **Storage Team** – maintains the registry implementation and publishes
  documentation updates.

## Lifecycle Workflow

1. **Proposal Submission**
   - Author runs the validation checklist from the authoring guide and creates
     a `ChunkerProfileProposalV1` JSON under
     `docs/source/sorafs/proposals/`.
   - Include CLI output from:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Submit a PR containing fixtures, proposal, determinism report, and registry
     updates.

2. **Tooling Review (TWG)**
   - Replay the validation checklist (fixtures, fuzz, manifest/PoR pipeline).
   - Run `cargo test -p sorafs_car --chunker-registry` and ensure
     `ensure_charter_compliance()` passes with the new entry.
   - Verify CLI behaviour (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) reflects the updated aliases and handles.
   - Produce a short report summarising findings and pass/fail status.

3. **Council Approval (GC)**
   - Review TWG report and proposal metadata.
   - Sign the proposal digest (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     and append signatures to the council envelope maintained alongside the
     fixtures.
   - Record the vote outcome in the governance minutes.

4. **Publication**
   - Merge the PR, updating:
     - `sorafs_manifest::chunker_registry_data`.
     - Documentation (`chunker_registry.md`, authoring/conformance guides).
     - Fixtures and determinism reports.
   - Notify operators and SDK teams of the new profile and planned rollout.

5. **Deprecation / Sunset**
   - Proposals that supersede an existing profile must include a dual-publish
     window (grace periods) and upgrade plan.
     in the registry and update the migration ledger.

6. **Emergency Changes**
   - Removal or hotfixes require a council vote with majority approval.
   - TWG must document the risk mitigation steps and update the incident log.

## Tooling Expectations

- `sorafs_manifest_chunk_store` and `sorafs_manifest_stub` expose:
  - `--list-profiles` for registry inspection.
  - `--promote-profile=<handle>` to generate the canonical metadata block used
    when promoting a profile.
  - `--json-out=-` to stream reports to stdout, enabling reproducible review
    logs.
- `ensure_charter_compliance()` is invoked at startup in relevant binaries
  (`manifest_chunk_store`, `provider_advert_stub`). CI tests must fail if new
  entries violate the charter.

## Record Keeping

- Store all determinism reports in `docs/source/sorafs/reports/`.
- Council minutes referencing chunker decisions live under
  `docs/source/sorafs/migration_ledger.md`.
- Update `roadmap.md` and `status.md` after each major registry change.

## References

- Authoring guide: [Chunker Profile Authoring Guide](./chunker-profile-authoring.md)
- Conformance checklist: `docs/source/sorafs/chunker_conformance.md`
- Registry reference: [Chunker Profile Registry](./chunker-registry.md)
