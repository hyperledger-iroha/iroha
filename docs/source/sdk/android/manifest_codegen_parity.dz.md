---
lang: dz
direction: ltr
source: docs/source/sdk/android/manifest_codegen_parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: daa555598e3c8db151668e9e8d2889e1c9cee0dd7f25af8f82ced8c0c43ca9b6
source_last_modified: "2026-01-30T18:06:03.309939+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Manifest & ISI Codegen Parity Inventory (AND3)

Roadmap item **AND3 — Scope manifest/ISI codegen parity** requires a single
reference describing which instructions already ship through the Android
codegen/export pipeline and what remains to be generated (especially fees,
manifest workflows, and governance payloads). This note captures the current
state of the artifacts under `target-codex/android_codegen/` and threads the
remaining work into concrete generator + fixture tasks so the Kotlin/Java
builders can stay aligned with the Rust data model.

## 1. Inputs & Evidence

- `target-codex/android_codegen/instruction_manifest.json` enumerates **98**
  `InstructionBox` entries with discriminants, schema hashes, enum layouts, and
  Rust type names exported by `norito_codegen_exporter`
  (`tools/norito_codegen_exporter/src/main.rs`). Each entry now carries a
  schema-derived summary when rustdoc text is unavailable, eliminating the
  earlier `TODO` placeholders.
- `target-codex/android_codegen/builder_index.json` lists the generated Kotlin /
  Java builders plus package names, feature gates, and stability annotations
  consumed by `scripts/android_codegen_docs.py` (and surfaced in
  `docs/source/sdk/android/generated/` via `make android-codegen-docs`).
- `target-codex/android_codegen/instruction_examples/` carries one Norito JSON
  example per discriminant. The file names mirror the Rust type paths, which
  lets CI diff fixture drift deterministically.
- `scripts/export_norito_fixtures/` continues to be the canonical exporter for
  signed payloads and will be reused when we promote the Android fixture set
  into `fixtures/norito_rpc/` for SDK parity.

## 2. Instruction Coverage Snapshot

- 98 discriminants span 16 modules (`InstructionBox` families) sourced from
  `crates/iroha_data_model/src/isi/`. All modules have builder entries, and the
  `iroha.revoke`, `iroha.set_key_value`, `iroha.remove_key_value`) still rely on
  the boxed helper enums instead of the fully qualified module paths.
- Every discriminant already has a Norito example file, so fixture generation is
  complete from Rust’s perspective. Android needs to bundle these examples into
  a signed manifest per release (see §4).

| Module | Count | Example discriminants |
|---|---:|---|
| `transparent` | 23 | iroha.custom, iroha.execute_trigger |
| `register` | 16 | iroha.register, iroha.unregister |
| `zk` | 10 | CancelConfidentialPolicyTransition, CreateElection |
| `sorafs` | 9 | ApprovePinManifest, BindManifestAlias |
| `kaigi` | 7 | CreateKaigi, EndKaigi |
| `mint_burn` | 6 | iroha.burn, iroha.mint |
| `transfer` | 6 | iroha.transfer, iroha.transfer_batch |
| `smart_contract_code` | 5 | ActivateContractInstance, DeactivateContractInstance |
| `repo` | 3 | iroha.repo.initiate, iroha.repo.reverse |
| `runtime_upgrade` | 3 | iroha.runtime_upgrade.activate, iroha.runtime_upgrade.cancel |
| `settlement` | 3 | iroha.settlement.dvp, iroha.settlement.pvp |
| `verifying_keys` | 3 | DeprecateVerifyingKey, RegisterVerifyingKey |
| `GrantBox` | 1 | iroha.grant |
| `RemoveKeyValueBox` | 1 | iroha.remove_key_value |
| `RevokeBox` | 1 | iroha.revoke |
| `SetKeyValueBox` | 1 | iroha.set_key_value |

## 3. Fees, Manifest & Governance Payloads

### 3.1 Trading & Fee-Bearing Flows

- **Repo instructions (`repo.rs`).** `RepoIsi`, `ReverseRepoIsi`, and
  `RepoMarginCallIsi` encapsulate haircut governance plus cash/collateral legs
  (`crates/iroha_data_model/src/isi/repo.rs`). Android codegen must emit
  builders that expose every governance knob (`RepoGovernance`) and include
  Norito examples for tri-party repos and unilateral unwind flows. Fixture plan:
  extend `scripts/export_norito_fixtures/src/main.rs` to drop repo-specific
  JSON into `fixtures/norito_rpc/manifest_repo/*.json` so SDK tests can replay
  the full lifecycle.
- **Settlement instructions (`settlement.rs`).** DVP/PVP payloads plus the
  generic `SettlementInstructionBox` are already exported, but Android still
  needs typed builders covering `SettlementPlan`, `SettlementLeg`, and metadata
  (see `crates/iroha_data_model/src/isi/settlement.rs`). Fee coverage relies on
  stitching both legs into a single Norito frame, so the fixture bundler must
  emit dual-leg samples with deterministic metadata.

### 3.2 Manifest / Pinning Surfaces

- **SoraFS pin manifests (`sorafs.rs`).** Instructions such as
  `RegisterPinManifest`, `IssueReplicationOrder`, and
  `RecordCapacityTelemetry` (all in `crates/iroha_data_model/src/isi/sorafs.rs`)
  depend on Norito manifests produced by `sorafs_manifest`. Android builders
  need to expose the same `ManifestV1`, `PinPolicy`, and telemetry fields that
  Rust serializes. Action items:
  1. ✅ `scripts/android_codegen_docs.py` now inlines the manifest field tables,
     so operator SDKs can audit rollout knobs without reading the Rust sources.
     The generator emits dedicated sections for `PinPolicy`, `ChunkerProfileHandle`,
     `ManifestAliasBinding`, and `StorageClass`, and the rendered reference captures
     the same table data that the roadmap called out.【scripts/android_codegen_docs.py:134】【docs/source/sdk/android/generated/instructions.md:903】
  2. ✅ Replay the shared fixture in `fixtures/sorafs_orchestrator/` via
     `scripts/android_codegen_replay_sorafs_fixture.py`. The helper calls
     `sorafs_manifest_stub` with the multi-provider payload, stores the full
     manifest report under
     `target-codex/android_codegen/sorafs_manifest/multi_peer_parity_v1.json`,
     emits the tracked sample at
     `docs/source/sdk/android/generated/fixtures/sorafs_register_pin_manifest_multi_peer_parity_v1.json`,
     and injects a `fixture_example` block into the RegisterPinManifest example
     so the Android builders can hydrate real manifest data. A dedicated pytest
     (`scripts/tests/android_sorafs_fixture_example_test.py`) loads the fixture,
     chunker profile, and generated example to ensure the digests, policy knobs,
     and submitted epoch all match the canonical multi-peer scenario.
  3. ✅ Manifest lifecycle coverage now includes approval, retirement, and alias
     binding: the Android SDK exposes typed builders for
     `ApprovePinManifest`, `RetirePinManifest`, and `BindManifestAlias`
     (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/ApprovePinManifestInstruction.java`,
     `java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/RetirePinManifestInstruction.java`,
     `java/iroha_android/src/main/java/org/hyperledger/iroha/android/model/instructions/BindManifestAliasInstruction.java`).
     Note: transaction encoding now requires wire-framed instruction payloads;
     the argument-map helpers are retained only for parity checks. Round-trip
     tests live under
     `java/iroha_android/src/test/java/org/hyperledger/iroha/android/sorafs/SorafsManifestInstructionBuilderTests.java`
     so CI will fail if the Norito arguments diverge from the Rust schema hashes.

- **Smart-contract uploads (`smart_contract_code.rs`).** The doc generator now
  renders the `ContractManifest`, `AccessSetHints`, and
  `EntrypointDescriptor` tables directly from
  `instruction_manifest.json` so the ABI/code-hash invariants baked into
  `crates/iroha_data_model/src/isi/smart_contract_code.rs` are spelled out
  alongside each instruction. The shared sample hash pair derived from
  `defaults/executor.to` is published under
  `docs/source/sdk/android/generated/fixtures/smart_contract_code_executor_hashes.json`,
  and `scripts/android_codegen_docs.py` links to that fixture so readers can
  verify their `.to` parsing logic without re-deriving the Blake2b/ABI digests.

### 3.3 Governance & Runtime Inputs

- **Kaigi governance (`kaigi.rs`).** `CreateKaigi`, `RecordKaigiUsage`, and
  relay-management instructions (see `crates/iroha_data_model/src/isi/kaigi.rs`)
  must be called out explicitly because they share telemetry proofs across SDKs.
  Builders should surface relay manifest hashes and usage counters so Kaigi
  audits stay deterministic.
- **Runtime upgrades (`runtime_upgrade.rs`).** `ProposeRuntimeUpgrade`,
  `ActivateRuntimeUpgrade`, and `CancelRuntimeUpgrade` discriminate runtime
  payloads with `abi_hash` gating. Android builders must embed the hash
  validation rules described in `docs/source/ivm.md` and reuse the Norito
  fixture used by the Rust admission tests.
- **Governance drafts (`governance.rs`).** ✅ Android now ships typed builders for
  `ProposeDeployContract`, `CastZkBallot`, `CastPlainBallot`, `EnactReferendum`,
  `FinalizeReferendum`, and `PersistCouncilForEpoch`. The generated reference documents their
  schema hashes, while `java/iroha_android/src/test/java/org/hyperledger/iroha/android/governance/GovernanceInstructionBuilderTests.java`
  exercises round-trip coverage so SDK integrations stay aligned with the Rust data model.

## 4. Generator & Fixture Outputs

1. **Doc extraction.** `norito_codegen_exporter` now accepts `--doc-json <path>`
   and uses the supplied rustdoc JSON to populate the `documentation` field for
   every instruction. Generate the JSON via
   `cargo +nightly rustdoc -p iroha_data_model -- -Zunstable-options --output-format json`
   (the file lands under `target/doc/iroha_data_model.json`) and pass that path
   when invoking the exporter so `scripts/android_codegen_docs.py` can drop the
   `TODO` placeholders automatically.
2. **Manifest-aware builder docs.** Update the generator to include enum layout
   tables (tags/payloads) for `sorafs`, `repo`, `settlement`, and governance
   instructions. This keeps the Kotlin/Java builders self-explanatory without
   reading the Rust source.
3. **Fixture bundler.** `scripts/android_codegen_fixtures.py` now zips the
   contents of `target-codex/android_codegen/instruction_examples/` into
   `artifacts/android/codegen_fixtures/<timestamp>/instruction_examples.zip`,
   writes the SHA-256 sidecar, and records `metadata.json`. CI can call this
   helper before publishing SDK parity fixtures.
4. **CI gate.** The `android-codegen-parity` workflow runs
   `make android-codegen-verify` in CI so pull requests fail if
   `instruction_manifest.json`, `builder_index.json`, or the generated docs fall
   out of sync; the run uploads `artifacts/android/codegen_parity_summary.json`
   for auditing.
5. **Recorded metadata.** `docs/source/sdk/android/generated/codegen_manifest_metadata.json`
   now stores the blessed SHA-256 digests and entry counts for the instruction
   manifest and builder index. The helper `scripts/check_android_codegen_parity.py`
   ingests those values and emits a parity summary (see `make android-codegen-verify`),
   failing the build when regenerated manifests drift from the recorded hashes.

## 5. Next Steps & Owners

Closed: the generator, fixtures, documentation, and CI gate are all in place.
Future schema changes should follow the change-log workflow
(`docs/source/sdk/android/norito_instruction_changes.md`) and rerun
`make android-codegen-verify` to refresh the recorded hashes and regenerated
docs.
