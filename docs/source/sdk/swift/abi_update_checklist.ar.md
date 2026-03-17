---
lang: ar
direction: rtl
source: docs/source/sdk/swift/abi_update_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f6729246b93fc7633fae39d4416aef070810ddcce960a401664b4637059d54f2
source_last_modified: "2026-01-21T10:30:30.225735+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
title: Swift ABI Update Checklist
summary: Procedures for adopting new IVM ABI releases in the Swift SDK, covering Rust/IVM updates, bridge surfaces, fixtures, and evidence requirements for IOS6.
---

# Swift ABI Update Checklist

This checklist fulfils the IOS6 roadmap requirement to **coordinate ABI update
reviews** before the Swift SDK ships against a new IVM ABI version or pointer
policy. Follow it whenever the Rust/IVM team proposes a core ABI change (not a
runtime upgrade), a new `abi_hash` is published, pointer types are added, or new
syscalls become available. The goal is to keep Swift’s bridge surfaces in lockstep with
`crates/ivm`, Torii, and the other SDKs so parity dashboards remain green.

## Policy note (first release)

ABI v1 is fixed and no v2 is planned. Runtime upgrades must keep
`abi_version = 1` and cannot add syscalls or pointer‑ABI types, so they do not
trigger this checklist. Only run this checklist if a future core release
explicitly introduces an ABI change.

## 1. Trigger Events

Run this checklist only if a core ABI change is explicitly approved (not
planned for v1). Examples:

- A new `ProgramMetadata.abi_version` or `abi_hash` is proposed in `crates/ivm`
  as part of a core release.
- Pointer-ABI types or syscall numbers change (see `crates/ivm_abi/src/pointer_abi.rs`
  and `crates/ivm_abi/src/syscalls.rs`).
- Swift needs to expose ABI capability metadata (e.g., governance manifests,
  Torii `/v1/runtime/*` endpoints, or manifest builders) for Connect, multisig,
  or DA surfaces.
- Governance requests parity evidence for ABI adoption (weekly digest, council
  pre-read, partner readiness packet).

## 2. Inputs & Owners

| Deliverable | Owner | Evidence / Location |
|-------------|-------|---------------------|
| ABI change proposal, syscall list, pointer policy | IVM maintainers / Core Protocol | `crates/ivm/docs/syscalls.md`, `crates/ivm/docs/pointer_abi.md`, linked RFC |
| Updated ABI hash + golden tests | IVM maintainers | `crates/ivm/tests/abi_hash_versions.rs`, `crates/ivm/tests/abi_syscall_list_golden.rs`, `crates/ivm/tests/pointer_type_ids_golden.rs` |
| Torii runtime upgrade schema & endpoints | Torii team | `crates/iroha/src/torii/runtime.rs`, `/v1/runtime/*` OpenAPI entries, fixtures under `integration_tests/tests/runtime_upgrades/` |
| Swift SDK surfaces & tests | Swift lead | `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`, `IrohaSwift/Sources/IrohaSwift/TxBuilder.swift`, `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift` |
| Digest/status updates | Swift Program PM / Docs | `status.md`, `docs/source/status/swift_weekly_digest.md`, exporter automation in `scripts/swift_status_export.py` |

## 3. Checklist

### 3.1 Align Rust/IVM Baseline

1. **Review the ABI proposal.** Confirm the change request includes the
   ABI version, hash, syscall numbers, pointer types, and policy mapping (`abi_version → SyscallPolicy`).
2. **Update and format Rust sources** under `crates/ivm/src/` and rerun
   `cargo fmt --all`.
3. **Keep policy helpers current.**
   - Update `ivm::syscalls::abi_syscall_list()` and `is_syscall_allowed`.
   - Update `ivm::pointer_abi::PointerType` plus `is_type_allowed_for_policy`.
   - Document the change in `crates/ivm/docs/syscalls.md` and `crates/ivm/docs/pointer_abi.md`
     (these files feed `docs/source/ivm_syscalls.md`).
4. **Refresh goldens & tests.** Run:
   ```bash
   cargo test -p ivm abi_syscall_list_golden abi_hash_versions pointer_type_ids_golden
   cargo test --workspace
   ```
   Update the generated tables (`crates/ivm/tests/abi_hash_versions.rs`, etc.)
   if hashes or IDs changed. Regenerate docs via `cargo xtask gen-pointer-types-doc`.
5. **Notify downstream owners.** File/update the roadmap entry and link the
   change bundle (Norito fixtures, RFC, dashboard screenshots).

### 3.2 Update Swift Surfaces & Native Bridge

1. **Runtime upgrade manifest structs.** Ensure
   `ToriiRuntimeUpgradeManifest`, `ToriiRuntimeAbiActive`, and
   `ToriiRuntimeAbiHash` (see `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`)
   mirror any new fields (pointer types list, policy labels, start/end height).
2. **Client APIs.** Verify `ToriiClient` & `TxBuilder` expose the latest `/v1/runtime/*`
   endpoints:
   - `listRuntimeUpgrades`, `getRuntimeAbiActive`, `getRuntimeAbiHash`,
     `proposeRuntimeUpgrade`, `activateRuntimeUpgrade`.
   - Confirm async + completion-handler variants decode and encode the new fields.
3. **Bridge enforcement.** If the ABI introduces new bridge calls or metadata,
   update `IrohaSwift/Sources/IrohaSwift/NativeBridge.swift` and, when relevant,
   follow the instrumentation guide in `docs/source/sdk/swift/native_bridge_instrumentation_checklist.md`
   (bridge versioning, telemetry, XCFramework smoke feeds).
4. **Manifest builders.** Ensure any builders that embed ABI metadata (e.g.,
   transaction manifests, DA/Kotodama wrappers) set the correct `abiVersion`,
   `abiHash`, pointer type IDs, and policy flags pulled from Torii.
5. **Docs.** Keep developer docs pinned to ABI v1 if APIs change
   (`docs/source/sdk/swift/connect_workshop.md`, `docs/source/sdk/swift/support_playbook.md`, etc.).

### 3.3 Tests, Fixtures, & Automation

1. **Swift unit tests.**
   ```bash
   swift test --package-path IrohaSwift --filter ToriiClientTests/testListRuntimeUpgrades
   swift test --package-path IrohaSwift --filter ToriiClientTests/testProposeRuntimeUpgradeAsync
   swift test --package-path IrohaSwift --filter ToriiClientTests/testGetRuntimeAbiHash
   ```
   Add/extend fixtures for any new manifest fields.
2. **Parity fixtures.** Run `make swift-fixtures` (wraps
   `scripts/check_swift_fixtures.py` and `ci/check_swift_fixtures.sh`) so the
   Norito fixture rotation captures the new ABI metadata.
3. **Dashboards & CI.** Execute `ci/check_swift_dashboards.sh` and
   `scripts/check_swift_dashboard_data.py` to ensure the parity/CI feeds
   referenced by `dashboards/mobile_parity.swift` and
   `dashboards/mobile_ci.swift` contain the new ABI information.
4. **Exporter automation.** Run `ci/swift_status_export.sh` so the weekly digest
   picks up the updated ABI rows (e.g., Governance Watchers block showing the
   new hash/version) and artifacts land under `artifacts/swift/`.
5. **Workspace tests.** Per repository policy, close the checklist only after
   `cargo test --workspace` and `swift test` both succeed locally.

### 3.4 Evidence & Reporting

1. Archive artefacts under `artifacts/swift/abi_update/<YYYYMMDD>/`:
   - Torii responses (`runtime/abi_active`, `runtime/abi_hash`).
   - Swift fixture diffs and CI feed snapshots.
   - `scripts/swift_status_export.py` output bundle.
2. Update `status.md` (Latest Updates + Risks) and the weekly digest
   (`docs/source/status/swift_weekly_digest.md`) with:
   - ABI version/hash that shipped.
   - Outstanding pointer/syscall follow-ups.
   - Links to the artefact directory above.
3. Reference this checklist in `roadmap.md` (IOS6 section) and mention the
   evidence location in the PR description, so governance reviewers have a
   deterministic path to the bundle.

### 3.5 Escalation & Contacts

- **Swift Program Lead:** owns ABI readiness across SDK surfaces and parity dashboards.
- **IVM Maintainer:** approves pointer/syscall changes and owns the golden tests.
- **Torii Runtime Owner:** coordinates `/v1/runtime/*` behaviour and staging rollouts.
- **Docs/Support:** keep the digest/status entries aligned and notify partners.

Escalate in `#sdk-parity` (internal) or the governance bridge if any of the
steps above fail or reveal conflicting ABI state across SDKs. Block releases
until the checklist, tests, and evidence bundle are complete.
