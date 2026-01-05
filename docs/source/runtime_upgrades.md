# Runtime Upgrades (IVM + Host) — No-Downtime, No-Hardfork

This document specifies a deterministic, governance‑controlled mechanism to introduce new IVM/host capabilities (e.g., new syscalls and pointer‑ABI types) without stopping the network or hard‑forking nodes. Nodes roll out binaries in advance; activation is coordinated on‑chain within a bounded height window. Old contracts continue to run unchanged; new capabilities are gated by ABI version and policy.

Note (first release): Only ABI v1 is supported. Runtime upgrade manifests targeting other ABI versions are rejected until a future release introduces a new ABI version.

Goals
- Deterministic activation at a scheduled height window with idempotent application.
- Coexistence of multiple ABI versions; never break existing binaries.
- Admission and execution guardrails so pre‑activation payloads cannot enable new behavior.
- Operator‑friendly rollout with capability visibility and clear failure modes.

Non‑Goals
- Changing existing syscall numbers or pointer‑type IDs (forbidden).
- Live patching nodes without deploying updated binaries.

Definitions
- ABI Version: Small integer declared in `ProgramMetadata.abi_version` that selects a `SyscallPolicy` and pointer‑type allowlist.
- ABI Hash: Deterministic digest of the ABI surface for a given version: syscall list (numbers+shapes), pointer‑type IDs/allowlist, and policy flags; computed by `ivm::syscalls::compute_abi_hash`.
- Syscall Policy: Host mapping that decides whether a syscall number is allowed for a given ABI version and host policy.
- Activation Window: Half‑open block‑height interval `[start, end)` in which activation is valid exactly once at `start`.

State Objects (Data Model)
<!-- BEGIN RUNTIME UPGRADE TYPES -->
- `RuntimeUpgradeId`: Blake2b-256 of the canonical Norito bytes for a manifest.
- `RuntimeUpgradeManifest` fields:
  - `name: String` — human-readable label.
  - `description: String` — short description for operators.
  - `abi_version: u16` — target ABI version to activate.
  - `abi_hash: [u8; 32]` — canonical ABI hash for the target policy.
  - `added_syscalls: Vec<u16>` — syscall numbers that become valid with this version.
  - `added_pointer_types: Vec<u16>` — pointer-type identifiers added by the upgrade.
  - `start_height: u64` — first block height where activation is permitted.
  - `end_height: u64` — exclusive upper bound on the activation window.
  - `sbom_digests: Vec<RuntimeUpgradeSbomDigest>` — SBOM digests for upgrade artefacts.
  - `slsa_attestation: Vec<u8>` — raw SLSA attestation bytes (base64 in JSON).
  - `provenance: Vec<ManifestProvenance>` — signatures over the canonical payload.
- `RuntimeUpgradeRecord` fields:
  - `manifest: RuntimeUpgradeManifest` — canonical proposal payload.
  - `status: RuntimeUpgradeStatus` — proposal lifecycle state.
  - `proposer: AccountId` — authority that submitted the proposal.
  - `created_height: u64` — block height where the proposal entered the ledger.
- `RuntimeUpgradeSbomDigest` fields:
  - `algorithm: String` — digest algorithm identifier.
  - `digest: Vec<u8>` — raw digest bytes (base64 in JSON).
<!-- END RUNTIME UPGRADE TYPES -->
  - Invariants: `end_height > start_height`; `abi_version` is strictly greater than any active version; `abi_hash` must equal `ivm::syscalls::compute_abi_hash(policy_for(abi_version))`; `added_*` must list exactly the additive delta between the new ABI policy and the previously active one; existing numbers/IDs MUST NOT be removed or renumbered.

Storage Layout
- `world.runtime_upgrades`: MVCC map keyed by `RuntimeUpgradeId.0` (raw 32-byte hash) with values encoded as canonical Norito `RuntimeUpgradeRecord` payloads. Entries persist across blocks; commits are idempotent and replay-safe.

Instructions (ISI)
- ProposeRuntimeUpgrade { manifest: RuntimeUpgradeManifest }
  - Effects: Insert `RuntimeUpgradeRecord { status: Proposed }` keyed by `RuntimeUpgradeId` if not present.
  - Reject if window overlaps any other Proposed/Activated record or if invariants fail.
  - Idempotent: re-submitting the same canonical manifest bytes is a no-op.
  - Canonical encoding: manifest bytes must match `RuntimeUpgradeManifest::canonical_bytes()`; non-canonical encodings are rejected.
- ActivateRuntimeUpgrade { id: RuntimeUpgradeId }
  - Preconditions: A matching Proposed record exists; `current_height` must equal `manifest.start_height`; `current_height < manifest.end_height`.
  - Effects: Flip record to `ActivatedAt(current_height)`; append `abi_version` to the active ABI set.
  - Idempotent: Replays at the same height are no‑ops; other heights are rejected deterministically.
- CancelRuntimeUpgrade { id: RuntimeUpgradeId }
  - Preconditions: Status is Proposed and `current_height < manifest.start_height`.
  - Effects: Flip to `Canceled`.

Events (Data Events)
- RuntimeUpgradeEvent::{Proposed { id, manifest }, Activated { id, abi_version, at_height }, Canceled { id }}

Admission Rules
- Contract Admission: Only `ProgramMetadata.abi_version = 1` is accepted in the first release; other values are rejected with `IvmAdmissionError::UnsupportedAbiVersion`.
  - For ABI v1 payloads, recompute `abi_hash(1)` and require equality with payload/manifest when provided; mismatches reject with `IvmAdmissionError::ManifestAbiHashMismatch`.
- Transaction Admission: Instructions `ProposeRuntimeUpgrade`/`ActivateRuntimeUpgrade`/`CancelRuntimeUpgrade` require appropriate permissions (root/sudo); must satisfy window overlap constraints.

Provenance Enforcement
- Runtime-upgrade manifests may carry SBOM digests (`sbom_digests`), SLSA attestation bytes (`slsa_attestation`), and signer metadata (`provenance` signatures). Signatures cover the canonical `RuntimeUpgradeManifestSignaturePayload` (all manifest fields except the `provenance` signatures list).
- Governance configuration controls enforcement under `governance.runtime_upgrade_provenance`:
  - `mode`: `optional` (accept missing provenance, verify if present) or `required` (reject if provenance is absent).
  - `require_sbom`: when `true`, at least one SBOM digest is required.
  - `require_slsa`: when `true`, a non-empty SLSA attestation is required.
  - `trusted_signers`: list of approved signer public keys.
  - `signature_threshold`: minimum number of trusted signatures required.
- Provenance rejections surface stable error codes in instruction failures (prefix `runtime_upgrade_provenance:`):
  - `missing_provenance`, `missing_sbom`, `invalid_sbom_digest`, `missing_slsa_attestation`
  - `missing_signatures`, `invalid_signature`, `untrusted_signer`, `signature_threshold_not_met`
- Telemetry: `runtime_upgrade_provenance_rejections_total{reason}` counts provenance rejection reasons.

Execution Rules
- VM Host Policy: During program execution, derive `SyscallPolicy` from `ProgramMetadata.abi_version`. Unknown syscalls for that version map to `VMError::UnknownSyscall`.
- Pointer‑ABI: Allowlist derived from `ProgramMetadata.abi_version`; types outside the allowlist for that version are rejected during decode/validation.
- Host Switching: Each block recomputes the active ABI set; once an activation transaction commits, subsequent transactions in the same block observe the new policy (validated by `runtime_upgrade_admission::activation_allows_new_abi_in_same_block`).
  - Syscall policy binding: `CoreHost` reads the transaction's declared ABI version and enforces `ivm::syscalls::is_syscall_allowed`/`is_type_allowed_for_policy` against the per-block `SyscallPolicy`. The host reuses the transaction-scoped VM instance, so mid-block activations are safe—later transactions observe the updated policy while earlier ones continue with their original version.

Determinism & Safety Invariants
- Activation occurs only at `start_height` and is idempotent; reorgs below `start_height` deterministically re‑apply once the block re‑lands.
- Existing ABI versions remain active indefinitely; new versions only extend the active set.
- No dynamic negotiation influences consensus or execution order; capability gossip is informational only.

Operator Rollout (No Downtime)
1) Deploy a node binary that supports the new ABI version (`v+1`) but does not activate it.
2) Observe fleet capability via telemetry (percentage of nodes advertising support for `v+1`).
3) Submit `ProposeRuntimeUpgrade` with a window sufficiently ahead (e.g., `H+N`).
4) At `start_height`, `ActivateRuntimeUpgrade` executes automatically as part of the included block and flips the host active set; nodes that didn’t upgrade will continue to function for old contracts but will reject admission/execution of `v+1` programs.
5) After activation, recompile/deploy contracts targeting `v+1`.

Torii & CLI
- Torii
  - `GET /v1/runtime/abi/active` → `{ active_versions: [u16], default_compile_target: u16 }` (implemented)
  - `GET /v1/runtime/abi/hash` → `{ policy: "V1", abi_hash_hex: "<64-hex>" }` (implemented)
  - `GET /v1/runtime/upgrades` → list of records (implemented).
  - `POST /v1/runtime/upgrades/propose` → wraps `ProposeRuntimeUpgrade` (returns instruction skeleton; implemented).
  - `POST /v1/runtime/upgrades/activate/:id` → wraps `ActivateRuntimeUpgrade` (returns instruction skeleton; implemented).
  - `POST /v1/runtime/upgrades/cancel/:id` → wraps `CancelRuntimeUpgrade` (returns instruction skeleton; implemented).
- CLI
  - `iroha runtime abi active` (implemented)
  - `iroha runtime abi hash` (implemented)
  - `iroha runtime upgrade list` (implemented)
  - `iroha runtime upgrade propose --file <manifest.json>` (implemented)
  - `iroha runtime upgrade activate --id <id>` (implemented)
  - `iroha runtime upgrade cancel --id <id>` (implemented)

Core Query API
- Norito singular query (signed):
  - `FindActiveAbiVersions` returns a Norito-encoded struct `{ active_versions: [u16], default_compile_target: u16 }`.
  - See sample: `docs/source/samples/find_active_abi_versions.md` (type/fields and JSON example).

Required Code Changes (by crate)
- iroha_data_model
  - Add `RuntimeUpgradeManifest`, `RuntimeUpgradeRecord`, instruction enums, events, and JSON/Norito codecs with roundtrip tests.
- iroha_core
  - WSV: add `runtime_upgrades` registry with overlap checks and getters.
  - Executors: implement ISI handlers; emit events; enforce admission rules.
  - Admission: gate program manifests by `abi_version` activity and `abi_hash` equality.
  - Syscall policy mapping: thread active ABI set to the VM host constructor; ensure determinism by using block height at execution start.
  - Tests: activation window idempotency, overlap rejections, pre/post admission behavior.
- ivm
  - Define `ABI_V2` (example) with policy: extend `abi_syscall_list()`; `is_syscall_allowed(policy, number)` mapping; pointer‑type policy extension.
  - Recompute and pin golden tests: `abi_syscall_list_golden.rs`, `abi_hash_versions.rs`, `pointer_type_ids_golden.rs`.
- iroha_cli / iroha_torii
  - Add endpoints and commands listed above; Norito JSON helpers for manifests; basic integration tests.
- Kotodama compiler
  - Allow targeting `abi_version = v+1`; embed correct `abi_hash` for selected version into `.to` manifests.

Telemetry
- Add `runtime.active_abi_versions` gauge and `runtime.upgrade_events_total{kind}` counter.

Security Considerations
- Only root/sudo may propose/activate/cancel; manifests must be signed appropriately.
- Activation windows prevent front‑running and ensure deterministic application.
- `abi_hash` pins the interface surface to prevent silent drift across binaries.

Acceptance Criteria (Conformance)
- Pre‑activation, nodes deterministically reject code with `abi_version = v+1`.
- Post‑activation at `start_height`, nodes accept and execute `v+1`; old programs continue to run unchanged.
- Golden tests for ABI hashes and syscall lists pass across x86‑64/ARM64.
- Activation is idempotent and safe under reorgs.
