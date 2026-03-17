# Runtime Upgrades (IVM + Host) — No-Downtime, No-Hardfork

This document specifies a deterministic, governance-controlled mechanism to roll out runtime upgrades without stopping the network or hard-forking nodes. Nodes roll out binaries in advance; activation is coordinated on-chain within a bounded height window. Old contracts continue to run unchanged; the host ABI surface stays fixed at v1.

Note (first release): ABI v1 is fixed and no ABI version bumps are planned. Runtime upgrade manifests must set `abi_version = 1`, and `added_syscalls`/`added_pointer_types` must be empty.

Goals
- Deterministic activation at a scheduled height window with idempotent application.
- Preserve ABI v1 stability; runtime upgrades do not change the host ABI surface.
- Admission and execution guardrails so pre‑activation payloads cannot enable new behavior.
- Operator‑friendly rollout with capability visibility and clear failure modes.

Non‑Goals
- Changing `abi_version` away from `1` or expanding syscall/pointer-type surfaces is out of scope for this release.
- Changing existing syscall numbers or pointer-type IDs (forbidden).
- Live patching nodes without deploying updated binaries.

Definitions
- ABI Version: Small integer declared in `ProgramMetadata.abi_version`. In the first release, this is fixed to `1` and always selects the canonical v1 syscall/pointer surface.
- ABI Hash: Deterministic digest of the ABI surface for a given version: syscall list (numbers+shapes), pointer‑type IDs/allowlist, and policy flags; computed by `ivm::syscalls::compute_abi_hash`.
- Syscall Policy: The fixed first-release host allowlist for ABI v1.
- Activation Window: Half‑open block‑height interval `[start, end)` in which activation is valid exactly once at `start`.

State Objects (Data Model)
<!-- BEGIN RUNTIME UPGRADE TYPES -->
- `RuntimeUpgradeId`: Blake2b-256 of the canonical Norito bytes for a manifest.
- `RuntimeUpgradeManifest` fields:
  - `name: String` — human-readable label.
  - `description: String` — short description for operators.
  - `abi_version: u16` — target ABI version to activate (must be 1 in the first release).
  - `abi_hash: [u8; 32]` — canonical ABI hash for the target policy.
  - `added_syscalls: Vec<u16>` — reserved delta list; must be empty in the first release.
  - `added_pointer_types: Vec<u16>` — reserved delta list; must be empty in the first release.
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
  - Invariants: `end_height > start_height`; `abi_version` must be `1`; `abi_hash` must equal `ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1)`; `added_*` must be empty; existing numbers/IDs MUST NOT be removed or renumbered.

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
  - Effects: Flip record to `ActivatedAt(current_height)`; active ABI set remains `{1}` in the first release.
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
- VM Host Policy: During program execution, only `ProgramMetadata.abi_version = 1` is valid, and the host enforces the canonical ABI v1 syscall surface. Unknown syscalls map to `VMError::UnknownSyscall`.
- Pointer‑ABI: The host enforces the canonical ABI v1 pointer-type allowlist; non-v1 ABI annotations fail closed during pointer decode/validation.
- Host Switching: Each block recomputes the active ABI set; in the first release this remains `{1}`, but activation is still recorded and is idempotent (validated by `runtime_upgrade_admission::activation_allows_v1_in_same_block`).
  - Syscall policy binding: `CoreHost` enforces `ivm::syscalls::is_syscall_allowed`/`is_type_allowed_for_policy` against the fixed v1 `SyscallPolicy`. The host reuses the transaction-scoped VM instance, so mid-block activations are safe—later transactions observe the updated active-upgrade record while earlier ones continue with the same v1 policy.

Determinism & Safety Invariants
- Activation occurs only at `start_height` and is idempotent; reorgs below `start_height` deterministically re‑apply once the block re‑lands.
- Active ABI set is fixed to `{1}` in the first release.
- No dynamic negotiation influences consensus or execution order; capability gossip is informational only.

Operator Rollout (No Downtime)
1) Deploy a node binary that includes the new runtime artifact while keeping ABI v1.
2) Observe fleet readiness via telemetry.
3) Submit `ProposeRuntimeUpgrade` with a window sufficiently ahead (e.g., `H+N`).
4) At `start_height`, `ActivateRuntimeUpgrade` executes as part of the included block and records activation; ABI remains v1.

Torii & CLI
- Torii
  - `GET /v1/runtime/abi/active` → `{ abi_version: u16 }` (implemented)
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
  - `FindAbiVersion` returns a Norito-encoded struct `{ abi_version: u16 }`.
  - See sample: `docs/source/samples/find_active_abi_versions.md` (type/fields and JSON example).

Implementation Notes (v1-only)
- iroha_data_model
  - Add `RuntimeUpgradeManifest`, `RuntimeUpgradeRecord`, instruction enums, events, and JSON/Norito codecs with roundtrip tests.
- iroha_core
  - WSV: add `runtime_upgrades` registry with overlap checks and getters.
  - Executors: implement ISI handlers; emit events; enforce admission rules.
  - Admission: gate program manifests by `abi_version` activity and `abi_hash` equality.
  - Syscall policy mapping: thread active ABI set to the VM host constructor; ensure determinism by using block height at execution start.
  - Tests: activation window idempotency, overlap rejections, pre/post admission behavior.
- ivm
  - ABI surface is fixed to v1; syscall lists and ABI hashes are pinned by golden tests.
- iroha_cli / iroha_torii
  - Add endpoints and commands listed above; Norito JSON helpers for manifests; basic integration tests.
- Kotodama compiler
  - Emit `abi_version = 1` and embed the canonical v1 `abi_hash` in `.to` manifests.

Telemetry
- Add `runtime.abi_version` gauge and `runtime.upgrade_events_total{kind}` counter.

Security Considerations
- Only root/sudo may propose/activate/cancel; manifests must be signed appropriately.
- Activation windows prevent front‑running and ensure deterministic application.
- `abi_hash` pins the interface surface to prevent silent drift across binaries.

Acceptance Criteria (Conformance)
- Nodes deterministically reject code with `abi_version != 1` at all times.
- Runtime upgrades do not change ABI policy; existing programs continue to run unchanged with v1.
- Golden tests for ABI hashes and syscall lists pass across x86-64/ARM64.
- Activation is idempotent and safe under reorgs.
