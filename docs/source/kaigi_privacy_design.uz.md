---
lang: uz
direction: ltr
source: docs/source/kaigi_privacy_design.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b7ffca7e960376a2959357cd865d8dab5afa1dfcb959adbc688b6db60977c8f
source_last_modified: "2026-01-05T09:28:12.022066+00:00"
translation_last_reviewed: 2026-02-07
---

# Kaigi Privacy & Relay Design

This document captures the privacy-focused evolution that introduces zero-knowledge
participation proofs and onion-style relays without sacrificing determinism or
ledger auditability.

# Overview

The design spans three layers:

- **Roster privacy** – hide participant identities on-chain while keeping host permissions and billing consistent.
- **Usage opacity** – allow hosts to log metered usage without disclosing per-segment details publicly.
- **Overlay relays** – route transport packets through multi-hop peers so network observers cannot learn which participants communicate.

All additions remain Norito-first, run under ABI version 1, and must execute deterministically across heterogeneous hardware.

# Goals

1. Admit/evict participants using zero-knowledge proofs so the ledger never exposes raw account IDs.
2. Maintain strong accounting guarantees: every join, leave, and usage event must still reconcile deterministically.
3. Provide optional relay manifests that describe onion routes for control/data channels and can be audited on-chain.
4. Keep the fallback (fully transparent roster) operational for deployments that do not require privacy.

# Threat Model Summary

- **Adversaries:** Network observers (ISPs), curious validators, malicious relay operators, and semi-honest hosts.
- **Protected assets:** Participant identity, participation timing, per-segment usage/billing details, and network routing metadata.
- **Assumptions:** Hosts still learn the true participant set off-chain; ledger peers verify proofs deterministically; overlay relays are untrusted but rate-limited; HPKE and SNARK primitives already exist in the codebase.

# Data Model Changes

All types live in `iroha_data_model::kaigi`.

```rust
/// Commitment to a participant identity (Poseidon hash of account + domain salt).
pub struct KaigiParticipantCommitment {
    pub commitment: FixedBinary<32>,
    pub alias_tag: Option<String>,
}

/// Nullifier unique to each join action, prevents double-use of proofs.
pub struct KaigiParticipantNullifier {
    pub digest: FixedBinary<32>,
    pub issued_at_ms: u64,
}

/// Relay path description used by clients to set up onion routing.
pub struct KaigiRelayManifest {
    pub hops: Vec<KaigiRelayHop>,
    pub expiry_ms: u64,
}

pub struct KaigiRelayHop {
    pub relay_id: AccountId,
    pub hpke_public_key: FixedBinary<32>,
    pub weight: u8,
}
```

`KaigiRecord` gains the following fields:

- `roster_commitments: Vec<KaigiParticipantCommitment>` – replaces the exposed `participants` list once the privacy mode is enabled. Classic deployments can keep both populated during migration.
- `nullifier_log: Vec<KaigiParticipantNullifier>` – strictly append-only, capped by a rolling window to keep metadata bounded.
- `room_policy: KaigiRoomPolicy` – selects the viewer authentication stance for the session (`Public` rooms mirror read-only relays; `Authenticated` rooms require viewer tickets before an exit forwards packets).
- `relay_manifest: Option<KaigiRelayManifest>` – structured manifest encoded with Norito so hops, HPKE keys, and weights stay canonical without JSON shims.
- `privacy_mode: KaigiPrivacyMode` enum (see below).

```rust
pub enum KaigiPrivacyMode {
    Transparent,
    ZkRosterV1,
}
```

`NewKaigi` receives matching optional fields so hosts can opt into privacy at creation time.


- Fields use `#[norito(with = "...")]` helpers to enforce canonical encoding (little-endian for integers, sorted hops by position).
- `KaigiRecord::from_new` seeds the new vectors empty and copies any provided relay manifest.

# Instruction Surface Changes

## Demo quickstart helper

For ad-hoc demos and interoperability tests the CLI now exposes
`iroha kaigi quickstart`. It:

- Reuses the CLI config (domain `wonderland` + account) unless overridden via `--domain`/`--host`.
- Generates a timestamp-based call name when `--call-name` is omitted and submits `CreateKaigi` against the active Torii endpoint.
- Optionally auto-joins the host (`--auto-join-host`) so viewers can connect immediately.
- Emits a JSON summary containing Torii URL, call identifiers, privacy/room policy, a ready-to-copy join command, and the spool path testers should monitor (e.g., `storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/*.norito`). Use `--summary-out path/to/file.json` to persist the blob.

This helper does **not** replace the need for a running `irohad --sora` node: privacy routes, spool files, and relay manifests remain ledger-backed. It simply trims boilerplate when spinning up temporary rooms for external parties.

### One-command demo script

For an even faster path there is a companion script: `scripts/kaigi_demo.sh`.
It performs the following for you:

1. Signs the bundled `defaults/nexus/genesis.json` into `target/kaigi-demo/genesis.nrt`.
2. Launches `irohad --sora` with the signed block (logs under `target/kaigi-demo/irohad.log`) and waits for Torii to expose `http://127.0.0.1:8080/status`.
3. Runs `iroha kaigi quickstart --auto-join-host --summary-out target/kaigi-demo/kaigi_summary.json`.
4. Prints the path to the JSON summary plus the spool directory (`storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/`) so you can share it with external testers.

Environment variables:

- `TORII_URL` — override the Torii endpoint to poll (default `http://127.0.0.1:8080`).
- `RUN_DIR` — override the working directory (default `target/kaigi-demo`).

Stop the demo by pressing `Ctrl+C`; the trap in the script terminates `irohad` automatically. The spool files and summary remain on disk so you can hand off artifacts after the process exits.

## `CreateKaigi`

- Validates `privacy_mode` against host permissions.
- If a `relay_manifest` is supplied, enforce ≥3 hops, non-zero weights, HPKE key presence, and uniqueness so on-chain manifests remain auditable.
- Validate `room_policy` input from SDKs/CLI (`public` vs `authenticated`) and propagate it to SoraNet provisioning so relay caches expose the correct GAR categories (`stream.kaigi.public` vs `stream.kaigi.authenticated`). Hosts wire this via `iroha kaigi create --room-policy …`, the JS SDK’s `roomPolicy` field, or by setting `room_policy` when Swift clients assemble the Norito payload prior to submission.
- Stores empty commitment/nullifier logs.

## `JoinKaigi`

Parameters:

- `proof: ZkProof` (Norito bytes wrapper) – Groth16 proof attesting the caller knows `(account_id, domain_salt)` whose Poseidon hash equals the supplied `commitment`.
- `commitment: FixedBinary<32>`
- `nullifier: FixedBinary<32>`
- `relay_hint: Option<KaigiRelayHop>` – optional per-participant override for the next hop.

Execution steps:

1. If `record.privacy_mode == Transparent`, fallback to current behavior.
2. Verify the Groth16 proof against the circuit registry entry `KAIGI_ROSTER_V1`.
3. Ensure `nullifier` has not appeared in `record.nullifier_log`.
4. Append commitment/nullifier entries; if `relay_hint` is supplied, patch the relay manifest view for this participant (stored only in in-memory session state, not on-chain).

## `LeaveKaigi`

Transparent mode matches current logic.

Private mode requires:

1. Proof that the caller knows a commitment in `record.roster_commitments`.
2. Nullifier update proving single-use leave.
3. Remove commitment/nullifier entries. Auditing preserves tombstones for fixed retention windows to avoid structural leakage.

## `RecordKaigiUsage`

Extends payload with:

- `usage_commitment: FixedBinary<32>` – commitment to the raw usage tuple (duration, gas, segment ID).
- Optional ZK proof verifying the delta matches encrypted logs provided off-ledger.

Hosts can still submit transparent totals; privacy mode only makes the commitment field mandatory.

# Verification & Circuits

- `iroha_core::smartcontracts::isi::kaigi::privacy` now performs full roster
  verification by default. It resolves `zk.kaigi_roster_join_vk` (joins) and
  `zk.kaigi_roster_leave_vk` (leaves) from configuration,
  looks up the corresponding `VerifyingKeyRef` in WSV (ensuring the record is
  `Active`, backend/circuit identifiers match, and commitments align), charges
  byte accounting, and dispatches to the configured ZK backend.
- The `kaigi_privacy_mocks` feature retains the deterministic stub verifier so
  unit/integration tests and constrained CI jobs can run without a Halo2 backend.
  Production builds must keep the feature disabled to enforce real proofs.
- The crate emits a compile-time error if `kaigi_privacy_mocks` is enabled on a
  non-test, non-`debug_assertions` build, preventing accidental release binaries
  from shipping with the stub.
- Operators need to (1) register the roster verifier set through governance, and
  (2) set `zk.kaigi_roster_join_vk`, `zk.kaigi_roster_leave_vk`, and
  `zk.kaigi_usage_vk` in `iroha_config` so hosts can resolve them at runtime.
  Until the keys are present, privacy joins, leaves, and usage calls fail
  deterministically.
- `crates/kaigi_zk` now ships Halo2 circuits for roster joins/leaves and usage
  commitments alongside the reusable compressors (`commitment`, `nullifier`,
  `usage`). The roster circuits expose the Merkle root (four little-endian
  64-bit limbs) as additional public inputs so the host can crosscheck the proof
  against the stored roster root before verification. Usage commitments are
  enforced by `KaigiUsageCommitmentCircuit`, which ties `(duration, gas,
  segment)` to the on-ledger hash.
- `Join` circuit inputs: `(commitment, nullifier, domain_salt)` and private
  `(account_id)`. Public inputs include `commitment`, `nullifier`, and
  four limbs of the Merkle root for the roster commitment tree (the roster
  remains off-chain, but the root is bound into the transcript).
- Determinism: we fix Poseidon parameters, circuit versions, and indexes in the
  registry. Any change bumps `KaigiPrivacyMode` to `ZkRosterV2` with matching
  tests/golden files.

# Onion Routing Overlay

## Relay Registration

- Relays self-register as domain metadata entries `kaigi_relay::<relay_id>` including HPKE key material and bandwidth class.
- The `RegisterKaigiRelay` instruction persists the descriptor in domain metadata, emits a `KaigiRelayRegistered` summary (with HPKE fingerprint and bandwidth class), and can be re-invoked to rotate keys deterministically.
- Governance curates allowlists through domain metadata (`kaigi_relay_allowlist`), and relay registration/manifest updates enforce membership before accepting new paths.

## Manifest Creation

- Hosts build multi-hop paths (minimum length 3) from available relays. The manifest encodes the sequence of AccountIds and the HPKE public keys required to encrypt the layered envelope.
- `relay_manifest` stored on-chain contains hop descriptors and expiry (Norito-encoded `KaigiRelayManifest`); actual ephemeral keys and per-session offsets are exchanged off-ledger using HPKE.

## Signalling & Media

- SDP/ICE exchange continues via Kaigi metadata but encrypted per hop. Validators only see HPKE ciphertext plus header indexes.
- Media packets travel through relays using QUIC with sealed payloads. Each hop decrypts one layer to learn the next hop address; final recipient gets the media stream after stripping all layers.

## Failover

- Clients monitor relay health via the `ReportKaigiRelayHealth` instruction, which persists signed feedback in domain metadata (`kaigi_relay_feedback::<relay_id>`), broadcasts `KaigiRelayHealthUpdated`, and allows governance/hosts to reason about current availability. When a relay fails, the host issues an updated manifest and logs a `KaigiRelayManifestUpdated` event (see below).
- Hosts apply manifest changes on-ledger through the `SetKaigiRelayManifest` instruction, which replaces the stored path or clears it entirely. Clearing emits a summary with `hop_count = 0` so operators can observe the transition back to direct routing.
- Prometheus metrics (`kaigi_relay_registered_total`, `kaigi_relay_registration_bandwidth_class`, `kaigi_relay_manifest_updates_total`, `kaigi_relay_manifest_hop_count`, `kaigi_relay_health_reports_total`, `kaigi_relay_health_state`, `kaigi_relay_failover_total`, `kaigi_relay_failover_hop_count`) now surface relay churn, health status, and failover cadence for operator dashboards.

# Events

Extend `DomainEvent` variants:

- `KaigiRosterSummary` – emitted with anonymised counts and the current roster
  root whenever the roster changes (root is `None` in transparent mode).
- `KaigiRelayRegistered` – emitted whenever a relay registration is created or updated.
- `KaigiRelayManifestUpdated` – emitted when the relay manifest changes.
- `KaigiRelayHealthUpdated` – emitted when hosts submit a relay health report via `ReportKaigiRelayHealth`.
- `KaigiUsageSummary` – emitted after each usage segment, exposing aggregate totals only.

Events serialize with Norito, exposing only commitment hashes and counts.

CLI tooling (`iroha kaigi …`) wraps each ISI so operators can register sessions,
submit roster updates, report relay health, and record usage without hand-crafting transactions.
Relay manifests and privacy proofs are loaded from JSON/hex files passed through
the CLI’s normal submission path, making it straightforward to script contract
admission in staging environments.

# Gas Accounting

- New constants in `crates/iroha_core/src/gas.rs`:
  - `BASE_KAIGI_JOIN_ZK`, `BASE_KAIGI_LEAVE_ZK`, and `BASE_KAIGI_USAGE_ZK`
    calibrated against the Halo2 verification timings (≈1.6 ms for roster
    joins/leaves, ≈1.2 ms for usage on Apple M2 Ultra). Surcharges continue to
    scale with proof byte size via `PER_KAIGI_PROOF_BYTE`.
- `RecordKaigiUsage` commits pay an extra fee based on commitment size and proof verification.
- Calibration harness will reuse the confidential asset infrastructure with fixed seeds.

# Testing Strategy

- Unit tests verifying Norito encode/decode for `KaigiParticipantCommitment`, `KaigiRelayManifest`.
- Golden tests for JSON view ensuring canonical ordering.
- Integration tests spinning up a mini-network with (see
  `crates/iroha_core/tests/kaigi_privacy.rs` for the current coverage):
  - Private join/leave cycles using mock proofs (feature flag `kaigi_privacy_mocks`).
  - Relay manifest updates propagated via metadata events.
- Trybuild UI tests covering host misconfiguration (e.g., missing relay manifest in privacy mode).
- When running unit/integration tests in constrained environments (e.g., the Codex
  sandbox), export `NORITO_SKIP_BINDINGS_SYNC=1` to bypass the Norito binding
  sync check enforced by `crates/norito/build.rs`.

# Migration Plan

1. ✅ Ship data model additions behind `KaigiPrivacyMode::Transparent` defaults.
2. ✅ Wire dual-path verification: production disables `kaigi_privacy_mocks`,
   resolves `zk.kaigi_roster_vk`, and runs real envelope verification; tests can
   still enable the feature for deterministic stubs.
3. ✅ Introduced the dedicated `kaigi_zk` Halo2 crate, calibrated gas, and wired
   integration coverage to run real proofs end-to-end (mocks are now test-only).
4. ⬜ Deprecate the transparent `participants` vector once all consumers understand commitments.

# Open Questions

- Define the Merkle tree persistence strategy: on-chain vs off-chain (current leaning: off-chain tree with on-chain root commitments). *(Tracked in KPG-201.)*
- Determine whether relay manifests should support multi-path (simultaneous redundant paths). *(Tracked in KPG-202.)*
- Clarify governance for relay reputations—do we need slashing or just soft bans? *(Tracked in KPG-203.)*

These items should be resolved before enabling `KaigiPrivacyMode::ZkRosterV1` in production.
