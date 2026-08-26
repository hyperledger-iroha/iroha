# Kaigi Privacy & Relay Design

This document captures the privacy-focused evolution toward zero-knowledge
participation proofs and onion-style relays without sacrificing determinism or
ledger auditability. Production `ZkRosterV1` joins are currently unavailable;
the exact fail-closed boundary is described below.

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
4. Keep the fully transparent roster as an explicit supported room mode for deployments that do not require privacy.

# Threat Model Summary

- **Adversaries:** Network observers (ISPs), curious validators, malicious relay operators, and semi-honest hosts.
- **Protected assets:** Participant identity, private per-segment usage/billing details, and network routing metadata. Public roster-summary events intentionally reveal that a roster mutation occurred and publish aggregate counts, so they do not hide mutation timing.
- **Assumptions:** Hosts still learn the true participant set off-chain; ledger peers verify proofs deterministically; overlay relays are untrusted but rate-limited; HPKE and SNARK primitives already exist in the codebase.

# Data Model Changes

All types live in `iroha_data_model::kaigi`.

```rust
/// Commitment to a participant identity (Poseidon hash of account + domain salt).
pub struct KaigiParticipantCommitment {
    pub commitment: FixedBinary<32>,
    pub alias_tag: Option<String>, // reserved on-chain; must be None
}

/// Nullifier unique to each join action, prevents double-use of proofs.
pub struct KaigiParticipantNullifier {
    pub digest: FixedBinary<32>,
    pub issued_at_ms: u64, // reserved on-chain; must be zero
}

/// Relay path description used by clients to set up onion routing.
pub struct KaigiRelayManifest {
    pub hops: Vec<KaigiRelayHop>,
    pub expiry_ms: u64,
}

pub struct KaigiRelayHop {
    pub relay_id: AccountId,
    /// Non-empty encoded HPKE public key bytes for the relay's negotiated suite.
    pub hpke_public_key: Vec<u8>,
    pub weight: u8,
}
```

`KaigiRecord` gains the following fields:

- `roster_commitments: Vec<KaigiParticipantCommitment>` – carries private roster commitments; transparent rooms use the explicit `participants` list instead.
- `nullifier_log: Vec<KaigiParticipantNullifier>` – strictly append-only; native metadata-size admission rejects a mutation before the stored record can exceed the configured bound. V1 does not evict nullifiers, because eviction would reopen proof replay.
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

- Reuses the CLI config (domain `wonderland.universal` + account) unless overridden via `--domain`/`--host`.
- Generates a timestamp-based call name when `--call-name` is omitted and submits `CreateKaigi` against the active Torii endpoint.
- Treats the host as an implicit member of the call; submitting a host
  `JoinKaigi` is invalid.
- Emits a JSON summary containing Torii URL, call identifiers, privacy/room policy, and a ready-to-copy join command. Use `--summary-out path/to/file.json` to persist the blob. V1 does not publish or advertise a token-bearing SoraNet exit spool.

This helper does **not** replace the need for a running `iroha3d --sora` node: privacy intent and relay manifests remain ledger-backed. It simply trims boilerplate when spinning up temporary rooms for external parties; filesystem exit publication remains disabled.

### One-command demo script

For an even faster path there is a companion script: `scripts/kaigi_demo.sh`.
It performs the following for you:

1. Signs the bundled `defaults/nexus/genesis.json` into `target/kaigi-demo/genesis.nrt`.
2. Launches `iroha3d --sora` with the signed block (logs under `target/kaigi-demo/iroha3d.log`) and waits for Torii to expose `http://127.0.0.1:8080/status`.
3. Runs `iroha kaigi quickstart --summary-out target/kaigi-demo/kaigi_summary.json`.
4. Prints the path to the JSON summary so you can share it with external testers. It does not create a SoraNet exit spool.

Environment variables:

- `TORII_URL` — override the Torii endpoint to poll (default `http://127.0.0.1:8080`).
- `RUN_DIR` — override the working directory (default `target/kaigi-demo`).

Stop the demo by pressing `Ctrl+C`; the trap in the script terminates `iroha3d` automatically. The summary remains on disk so you can hand off the non-secret demo metadata after the process exits.

### Signal query admission

`GET /v1/kaigi/calls/{call_id}/signals` treats transaction metadata as an
application projection, not as authority. It admits only successful committed
transactions carrying the canonical `iroha-demo-kaigi-chain-signal/v1` schema,
and the signed transaction authority must share the active account-id rekey
lineage of the host or a current transparent roster participant. Lineage and
roster checks use one generation-consistent state snapshot at its authenticated
committed ledger time; malformed or ambiguous live lineage fails closed.
The projection rejects non-signal and wrong-call transactions before resolving
their authorities, and caps the remaining distinct-authority lineage cache at
the canonical per-query fetch budget so unrelated ledger history cannot grow
request memory without bound.
Only unique, live, explicitly proven account-ID rekey successors of the stored
host or current transparent participants qualify; ordinary alias reassignment
and revoked or expired lineage do not.
Private calls remain host-only while private roster joins are disabled. Because
projecting the signal history scans committed
transactions, the route is classified as expensive compute and requires a
canonical account-signed request; it remains available to ordinary on-ledger
accounts rather than being operator-only. Every request retains general and
heavy-query admission permits for the scan, including callers whose source
network bypasses ordinary rate limiting. Pagination, ordering, and
`after_timestamp_ms` use the canonical carrier block's creation time. The
metadata creation timestamp may not exceed that carrier time, and the carrier
must fall within the stored call lifecycle (inclusive of its creation and end
timestamps), preventing participant-controlled metadata from poisoning cursors
or surfacing signals outside the call's lifetime.
Private responses omit the signed authority and remove the
canonical host/participant identity fields from both the response projection
and its returned metadata. Call reads also reject a stored record whose embedded
identifier does not match the domain metadata key requested by the client.

## `CreateKaigi`

- Validates `privacy_mode` against host permissions.
- Under the fail-safe Initial executor, require the host to own the call's
  domain or hold that domain's exact `CanModifyDomainMetadata` permission, so a
  foreign account cannot squat the protected `kaigi__*` namespace. Installed
  executors are responsible for applying an equivalent or stricter domain
  policy before dispatch.
- Reject an explicitly configured zero participant limit; an omitted limit remains unbounded.
- Require the signed host to be a registered account. An explicit
  `billing_account` must identify that same host; third-party billing remains
  unavailable until a delegated billing authorization is defined.
- If a `relay_manifest` is supplied, enforce ≥3 hops, non-zero weights, HPKE key presence, exact identifier uniqueness, typed account-rekey-lineage uniqueness, and an expiry strictly after the current block time. Every hop must also be allowed by any configured governance allowlist and have a stored descriptor with the exact relay identifier, a non-zero bandwidth class, and the same HPKE key; creation and later manifest replacement use the same admission checks. A retired and successor relay ID therefore cannot fill two route positions.
- Validate `room_policy` input from SDKs/CLI (`public` vs `authenticated`) and retain it as on-chain SoraNet intent. Hosts wire this via `iroha kaigi create --room-policy …`, the JS SDK’s `roomPolicy` field, or by setting `room_policy` when Swift clients assemble the Norito payload prior to submission. V1 disables every token-bearing filesystem exit route until RouteOpen binds a viewer credential and authoritative segment proof and the producer provides durable revocation.
- Starts with an empty participant-commitment log. If the host supplies the
  optional privacy proof at creation, stores the host commitment and records
  its nullifier immediately so it cannot authorize a later action again.

## `JoinKaigi`

Parameters:

- `proof: ZkProof` (Norito bytes wrapper) – candidate Halo2/IPA proof of an
  `(account_id, domain_salt, nullifier_seed)` witness whose domain-separated
  Poseidon hashes equal the supplied `commitment` and `nullifier`. The current
  circuit does not attest that this witness is the signed caller.
- `commitment: FixedBinary<32>`
- `nullifier: FixedBinary<32>`
- `relay_hint: Option<KaigiRelayHop>` – optional per-participant override for the next hop.

Execution steps:

1. If `record.privacy_mode == Transparent`, execute the transparent-room behavior.
2. `ZkRosterV1` currently fails closed before proof dispatch. Its candidate
   Halo2 statement does not bind a public input derived from the signed
   participant, so a copied NIZK could be submitted by another signer.
3. A future versioned roster circuit must bind the canonical transaction
   authority, then check the nullifier and append commitment/nullifier entries.

## `LeaveKaigi`

Transparent mode permits the host or participant's unique successor from an
explicitly typed SNS `AccountIdRekey` lineage to remove the historical roster
entry. Live SNS resolution is preferred, while persisted canonical rekey edges
retain continuity after the supporting lease expires, is revoked, or is later
reassigned. The stored audit identity is not rewritten; leave removes that
exact entry and its participant metadata. `AliasReassignment` edges never
transfer authority, and branches, cycles, registered predecessors, or multiple
registered successors fail closed. An aliasless controller rekey is rejected
while active Kaigi or retained relay state needs that continuity.

Private leave is intentionally unavailable on-chain in the first-release
`ZkRosterV1` profile. A participant disconnects from the local session, or the
host ends the call. A future on-chain leave flow must use a dedicated circuit
that proves Merkle membership in `record.roster_commitments`; the join circuit
must not be reused for that purpose.

## `RecordKaigiUsage`

Extends payload with:

- `usage_commitment: FixedBinary<32>` – commitment to the raw usage tuple (duration, gas, segment ID).
- Optional ZK proof verifying the delta matches encrypted logs provided off-ledger.

Hosts can still submit transparent totals; privacy mode only makes the commitment field mandatory.

# Verification & Circuits

- `iroha_core::smartcontracts::isi::kaigi::privacy` keeps candidate roster
  validation helpers but production join admission is unavailable until the
  signed participant is part of the circuit statement. Usage and host proof
  paths resolve their configured keys from configuration and look up the
  corresponding `VerifyingKeyRef` in WSV (ensuring the record is
  `Active`, backend/circuit identifiers match, commitments align, and the
  encoded proof fits the verifier record's `max_proof_bytes` cap before any
  envelope decode or public-input extraction), charges
  byte accounting, and dispatches to the configured ZK backend.
- The `kaigi_privacy_mocks` feature retains deterministic stubs for in-crate
  unit tests only. Any non-test library build with the feature is a compile-time
  error, including debug builds, so a runnable node cannot ship the stub.
- Operators need to register the host/usage verifier set through governance and
  configure `zk.kaigi_roster_join_vk`, `zk.kaigi_roster_leave_vk`, and
  `zk.kaigi_usage_vk` so those still-supported proof paths can resolve keys at
  runtime. Missing keys reject host proof actions and usage calls
  deterministically. Roster joins reject regardless of key configuration until
  the authority-bound statement is implemented.
- `crates/kaigi_zk` now ships Halo2 circuits for roster joins and usage
  commitments alongside the reusable, domain-separated Poseidon compressors
  (`commitment`, `nullifier`, `usage`). The roster join circuit exposes the
  pre-join Merkle root (four little-endian 64-bit limbs) as additional public
  inputs so the host can crosscheck the proof against the stored roster root
  before verification. Because a join creates a new leaf, this root is a
  state/freshness binding rather than a membership claim. Usage commitments are
  enforced by `KaigiUsageCommitmentCircuit`, which ties `(duration, gas,
  segment)` to the on-ledger hash.
- `Join` circuit private witnesses are `(account_id, domain_salt,
  nullifier_seed)`. Public inputs are `commitment`, `nullifier`, and four limbs
  of the Merkle root for the roster commitment tree (the roster remains
  off-chain, but the root is bound into the transcript). Admission requires
  exactly those six single-row columns and compares the first two values
  byte-for-byte with the instruction artifacts before verification. This
  candidate shape is insufficient for production because it omits the signed
  participant authority.
- Host lifecycle authorization is not delegated to a transferable proof:
  `EndKaigi` always requires the stored host account or its unique live,
  explicitly proven account-ID rekey successor. The historical stored host ID
  remains unchanged.
  If host privacy artifacts were supplied at creation, their nullifier is
  recorded immediately and cannot be reused by a later host action.
- Ledger-visible privacy artifacts carry only cryptographic values: native
  execution requires `KaigiParticipantCommitment.alias_tag = None` and
  `KaigiParticipantNullifier.issued_at_ms = 0`. Clear labels and local timing
  belong in encrypted host session state; nullifier replay protection depends
  only on the digest.
- Kaigi verifier-key carriers use strict ZK1 `IPAK`/`CID1`/`H2VK` metadata in
  that exact order, and outer envelopes use the exact owner-crate schema tag
  plus a nonzero hash of the complete registered key carrier. The public JS
  roster builder rejects while roster join admission is unavailable; its
  internal candidate fixture remains test-only.
- Determinism: we fix Poseidon parameters, circuit versions, and indexes in the
  registry. Any change bumps `KaigiPrivacyMode` to `ZkRosterV2` with matching
  tests/golden files.

# Onion Routing Overlay

## Relay Registration

- Relays self-register as domain metadata entries `kaigi_relay__<account-digest>` including HPKE key material and bandwidth class. Native domain registration and generic metadata ISIs reject attempts to seed, overwrite, or remove these reserved entries; relay descriptors must use `RegisterKaigiRelay`. The signed relay account must be registered and have a live domain-qualified primary alias; that authenticated primary alias, not a global allowlist scan, selects the descriptor's governance domain. Aliasless or domainless-primary relays fail closed. While relay state exists, primary-alias changes may stay within that storage domain but cannot clear or move to another domain; account/domain removal likewise fails while protected Kaigi dependencies remain.
- The `RegisterKaigiRelay` instruction persists the descriptor in domain metadata, emits a `KaigiRelayRegistered` summary (with HPKE fingerprint and bandwidth class), and can be re-invoked to rotate keys deterministically.
- Governance can curate allowlists through domain metadata (`kaigi_relay_allowlist`); when an allowlist is configured, relay registration and manifest updates enforce membership before accepting new paths. Membership follows only a unique, explicitly typed account-ID rekey lineage, including retained canonical `AccountIdRekey` edges after lease expiry or reassignment, so a retired allowlist entry can authorize its valid successor without granting authority to an independent alias assignee. The relay itself still needs a live domain-qualified primary alias, and malformed allowlists in unrelated domains are never consulted.

## Manifest Creation

- Hosts build multi-hop paths (minimum length 3) from available relays. The manifest encodes the sequence of AccountIds and the HPKE public keys required to encrypt the layered envelope.
- `relay_manifest` stored on-chain contains hop descriptors and expiry (Norito-encoded `KaigiRelayManifest`); actual ephemeral keys and per-session offsets are exchanged off-ledger using HPKE.

## Signalling & Media

- SDP/ICE exchange continues via Kaigi metadata but encrypted per hop. Validators only see HPKE ciphertext plus header indexes.
- Media packets travel through relays using QUIC with sealed payloads. Each hop decrypts one layer to learn the next hop address; final recipient gets the media stream after stripping all layers.

## Failover

- Clients monitor relay health via the `ReportKaigiRelayHealth` instruction, which persists signed feedback in domain metadata (`kaigi_relay_feedback__<account-digest>`), broadcasts `KaigiRelayHealthUpdated`, and allows governance/hosts to reason about current availability. Stored feedback must embed the relay identifier selected by its metadata key. The global latest-observation singleton is strictly ordered by `reported_at_ms`: future timestamps beyond the current block time and older reports are rejected, every non-identical equal-timestamp report is rejected across calls, and an exact duplicate is an event-free idempotent no-op. Feedback is diagnostic and does not override relay governance or manifest admission; when a relay fails, the host issues an updated manifest and logs a `KaigiRelayManifestUpdated` event (see below).
- Hosts apply manifest changes on-ledger through the `SetKaigiRelayManifest` instruction, which replaces the stored path or clears it entirely. Clearing emits a summary with `hop_count = 0` so operators can observe the transition back to direct routing.
- Prometheus metrics (`kaigi_relay_registered_total`, `kaigi_relay_registration_bandwidth_class`, `kaigi_relay_manifest_updates_total`, `kaigi_relay_manifest_updates_by_domain_total`, `kaigi_relay_manifest_hop_count`, `kaigi_relay_health_reports_total`, `kaigi_relay_health_reports_by_domain_total`, `kaigi_relay_health_state`, `kaigi_relay_failover_total`, `kaigi_relay_failovers_by_domain_total`, `kaigi_relay_failover_hop_count`) now surface relay churn, health status, and failover cadence for operator dashboards. The domain-only counters back bounded diagnostic snapshots without collecting the dimensioned Prometheus label families.

# Events

Extend `DomainEvent` variants:

- `KaigiRosterSummary` – emitted with anonymised counts and the current roster
  root whenever the roster changes (root is `None` in transparent mode).
- `KaigiRelayRegistered` – emitted whenever a relay registration is created or updated.
- `KaigiRelayManifestUpdated` – emitted when the relay manifest changes.
- `KaigiRelayHealthUpdated` – emitted when hosts submit a relay health report via `ReportKaigiRelayHealth`.
- `KaigiUsageSummary` – emitted after each usage segment, exposing aggregate totals only.

Events serialize with Norito, exposing only commitment hashes and counts.

CLI tooling (`iroha kaigi …`) wraps each ISI so operators can register relay
descriptors, create sessions, submit transparent roster updates, replace relay manifests,
report relay health, and record usage without hand-crafting transactions.
Relay manifests and privacy proofs are loaded from JSON/hex files passed
through the CLI’s normal submission path, making it straightforward to script
contract admission in staging environments.

# Gas Accounting

- New constants in `crates/iroha_core/src/gas.rs`:
  - `BASE_KAIGI_JOIN_ZK`, `BASE_KAIGI_LEAVE_ZK`, and `BASE_KAIGI_USAGE_ZK`
    retain candidate roster calibration and the active usage calibration
    (≈1.6 ms for candidate roster proofs, ≈1.2 ms for usage on Apple M2 Ultra).
    Disabled roster transitions do not dispatch a verifier and retain only a
    governed per-proof-byte payload surcharge.
- Successful host-create proofs charge the governed confidential-verification
  base, proof bytes, six public inputs, one consumed nullifier, and one newly
  stored commitment. Host-end proofs charge the same verifier/public-input and
  nullifier costs but do not charge a new commitment because they reference the
  stored host commitment. `RecordKaigiUsage` proofs charge the verifier base,
  proof bytes, one public input, and one newly stored usage commitment.
- Instruction-batch gas totals use saturating addition, matching every dynamic
  proof component, so extreme governed schedules cannot wrap in release builds
  or panic in debug builds.
- Calibration harness will reuse the confidential asset infrastructure with fixed seeds.

# Testing Strategy

- Unit tests verifying Norito encode/decode for `KaigiParticipantCommitment`, `KaigiRelayManifest`.
- Golden tests for JSON view ensuring canonical ordering.
- Unit and integration tests cover (see
  `crates/iroha_core/tests/kaigi_privacy.rs` and the in-module Kaigi tests):
  - Fail-closed private join admission, strict candidate carriers, host-signature
    enforcement, and create-nullifier replay rejection.
  - Relay manifest updates propagated via metadata events.
- Trybuild UI tests covering host misconfiguration (e.g., missing relay manifest in privacy mode).
- When running unit/integration tests in constrained environments (e.g., the Codex
  sandbox), export `NORITO_SKIP_BINDINGS_SYNC=1` to bypass the Norito binding
  sync check enforced by `crates/norito/build.rs`.

# Release Plan

1. ✅ Ship data model additions behind `KaigiPrivacyMode::Transparent` defaults.
2. ✅ Make `kaigi_privacy_mocks` unit-test-only and fail closed in every runnable
   build.
3. ✅ Introduce the dedicated `kaigi_zk` candidate Halo2 circuits and strict
   carrier validation.
4. ⬜ Version the roster statement with signed-participant public-input binding,
   regenerate the deterministic key/schema/SDK fixtures, and only then enable
   production private joins.
5. ⬜ Decide whether a future ABI should add a private-only roster representation; V1 keeps both room modes explicit.

# Open Questions

- Define the Merkle tree persistence strategy: on-chain vs off-chain (current leaning: off-chain tree with on-chain root commitments). *(Tracked in KPG-201.)*
- Determine whether relay manifests should support multi-path (simultaneous redundant paths). *(Tracked in KPG-202.)*
- Clarify governance for relay reputations—do we need slashing or just soft bans? *(Tracked in KPG-203.)*

These items should be resolved before enabling `KaigiPrivacyMode::ZkRosterV1` in production.
