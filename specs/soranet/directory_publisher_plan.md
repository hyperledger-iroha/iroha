---
title: SoraNet Directory Publisher & Consensus Artifacts
summary: Final specification for SNNet-3 covering directory committee processes, consensus artifacts, client validation, and rollout.
---

# SoraNet Directory Publisher & Consensus Artifacts

## Goals & Scope
- Build the directory publisher toolchain that aggregates relay descriptors, signs consensus artifacts, rotates salts, and publishes to SoraNS/IPFS (SNNet-3).
- Provide library support and operational playbooks for relays, clients, and emergency response.
- Ensure availability and integrity of consensus data across multiple deterministic gateways.

## Architecture Overview
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| Descriptor collectors (`dir_collector`) | Fetch relay microdescriptors from relays (entry/middle/exit). | QUIC connections with authenticated channels. |
| Consensus builder (`dir_builder`) | Aggregates descriptors, computes consensus document, signs with committee keys. | Produces `consensus.car` + metadata. |
| Publisher (`dir_publisher`) | Uploads consensus artifacts to IPFS/SoraNS, updates head pointers, notifies clients. | Works with governance DAG. |
| Client validation library (`soranet_directory`) | Verifies consensus signature, salt schedule, capability lists. | Used by clients/SDKs. |
| Emergency key rotation tool | Manages committee key updates and salt rotation recovery. | Multi-sig operations. |

## Descriptor Format
- Microdescriptor `RelayDescriptorV1` includes:
  - Identity keys (Ed25519), capabilities, guard flags, bandwidth weights.
  - Contact info (optional), policy (allowed ports), region metadata.
  - Salt epoch info (current salt ID, valid range).
- Directory snapshots embed each SRCv2 bundle as `certificate_base64`
  (standard Base64 encoding of `RelayCertificateBundleV2::try_to_cbor()`). Builders
  must ensure the bundle metadata (`relay_id`, `guard_weight`, bandwidth,
  reputation) matches the descriptor fields and that the declared
  `valid_after`/`valid_until` range covers the consensus window. Clients treat
  the certificate as the authoritative source for signed relay metadata and
  handshake suite ordering; expired bundles are ignored in favour of fresh
  descriptors. SRCv2 carries no static relay KEM key or KEM-rotation policy:
  PQ capability comes from the authenticated NK2/NK3 suite list, and each
  handshake encapsulates only to the client's ephemeral KEM share.
- Each relay receives a private `RelayDescriptorManifestV1` (not published in
  consensus) with exactly `version = 1` and an `identity` object containing the
  Ed25519 seed as `identity.ed25519_private_key_hex` (64 lowercase hex
  characters), the raw ML-DSA-65 private key as
  `identity.mldsa65_private_key_hex` (8064 lowercase hex characters). Both
  fields are mandatory, and each derived signing identity must equal the
  corresponding value in the relay's SRCv2 certificate. The runtime consumes
  the exact document through `handshake.descriptor_manifest_path`; aliases,
  extra fields (including static ML-KEM private or public fields), omitted
  keys, and inline configuration secrets are rejected. NK2 and NK3 encapsulate
  only to the public KEM shares carried in the client handshake, so the relay
  manifest contains no persistent KEM key pair.
  Operators inject the owner-private manifest at runtime and never publish or
  commit its private values.
- Relays publish descriptors via `POST /soranet/descriptor` with signatures; descriptors stored in builder DB.

## Consensus Artifact
- `consensus.car` contains canonical document `ConsensusDocumentV1`:
  ```norito
  struct ConsensusDocumentV1 {
      version: u8,
      valid_after: Timestamp,
      fresh_until: Timestamp,
      valid_until: Timestamp,
      relays: Vec<RelayConsensusEntryV1>,
      salts: SaltRotationScheduleV1,
      pow_params: PowParametersV1,
      signatures: Vec<CommitteeSignatureV1>,
  }
  ```
- Each `RelayConsensusEntryV1` references descriptor hash plus consensus weight/capabilities.
- Document hashed (BLAKE3) and stored in CAR file alongside microdescriptor attachments.
- Committee signers (minimum 3 of 5) sign digest with Dilithium3; signature set recorded.
- Consensus also includes blinded CID policy flags, PoW difficulty, guard rotation hints.

## Publication Workflow
1. Collect descriptors hourly; verify signatures and capability compliance.
2. Builder selects latest descriptors, generates consensus with `valid_after = now + 30s` and `fresh_until = valid_after + 1h`.
3. Build CAR file, compute CID, store in IPFS (`ipfs://soranet/consensus/<cycle>.car`).
4. Register CID in SoraNS (`soranet.consensus.current`), including backup pointers.
5. Emit `ConsensusPublishedV1` via Torii containing CID, validity window, previous CID.
6. Governance DAG logs consensus metadata for audit.

## Client Validation
- Library `soranet_directory` provides:
  - `fetch_consensus()` (via HTTP/IPFS) with fallback servers.
  - `validate_consensus(doc)` verifying signatures, validity window, salt schedule.
  - `apply_consensus(doc)` updating local relay list, salt schedule, PoW parameters.
- CLI `soranet directory verify --cid <cid>` for manual validation.
- Failure handling: if consensus signature invalid, client falls back to previous consensus (within `valid_until`), raises alert.
- Cache consensus locally with 24h retention; fetch new when `fresh_until` reached.

## Salt & Emergency Rotation
- Salt schedule embedded in consensus; builder ensures next `k` epochs pre-published (k=3).
- Emergency rotation: `EmergencySaltRotationV1` published if compromise detected; clients fetch from consensus updates + Torii alert.
- Directory publisher includes emergency key rotation playbook: multi-sig process to update `CommitteeKeyV1`, reissue consensus with new keys.

### Witness Key Rotation Checklist
1. Signal rotation intent in governance (`governance/soranet/witness-rotation/<date>.md`) with the planned effective window and affected witness key IDs.
2. Generate replacement Ed25519 witness keypairs inside the hardware vault, export public keys as Norito manifests, and store sealed private material under the custody log entry.
3. Update the publisher manifest (`configs/soranet/directory_publisher.toml`) with the new witness key IDs and validity window; submit for Salt Council review.
4. Rebuild the directory metallib via `soranet-directory build` to ensure the upcoming consensus bundles include the new witness key fingerprint.
5. Run `soranet-directory rotate --snapshot <active-snapshot> --expected-snapshot-digest <independently-approved-digest> --out <rotated-snapshot> --keys-out <private-output-directory>` in staging. Rotation authenticates the exact source bytes and their current validity before issuing replacement material; archive the command evidence at `fixtures/soranet_directory/witness_rotation/<date>/`.
6. Apply the rotation in production during the scheduled window, publish the updated manifest hash via Torii, and confirm clients validate the witness signature chain.
7. Close out the governance ticket with telemetry screenshots (`soranet_consensus_signature_verification_total{result="ok"}`) and attach the verification bundle checksum to the custody log.

#### 2026-03-21 Checklist Update
- Added `governance/soranet/witness-rotation/2026-03-21.md` capturing the Q2 2026 witness update window, affected key IDs, and sign-off from the Salt Council reviewer.
- Generated replacement witness key manifests (`artifacts/soranet_directory/witness_keys/2026-03-21/`) and recorded sealed private material hashes in the custody register.
- Updated `configs/soranet/directory_publisher.toml` with the staged `witness_key_set = "2026-03-21"` entry; review link: `ops/change-requests/CR-2026-03-21-witness.txt`.
- Rebuilt and verified directory artefacts via:
  ```bash
  cargo run -p soranet-directory -- build \
      --config configs/soranet/directory_publisher.toml \
      --out artifacts/soranet_directory/snapshots/consensus-2026-03-21.norito
  cargo run -p soranet-directory -- rotate \
      --snapshot artifacts/soranet_directory/snapshots/consensus-2026-03-21.norito \
      --expected-snapshot-digest <independently-approved-source-digest> \
      --out artifacts/soranet_directory/snapshots/consensus-2026-03-21-rotated.norito \
      --keys-out artifacts/soranet_directory/witness_keys/2026-03-21/
  ```
- Archived verification artefacts in `artifacts/soranet_directory/witness_rotation/2026-03-21/` (snapshot, manifests, checksums) and linked the checksum manifest to governance ticket `GOV-412`.
- Scheduled production activation for 2026-03-24 02:00 UTC with back-out plan documented alongside the ticket; monitoring runbook updated to watch the new witness fingerprint gauge (`soranet_consensus_witness_fingerprint`).

## Availability & Redundancy
- Primary publisher nodes in two regions with cross-region replication.
- Gateways host mirror caches; clients randomize fetch between sources to avoid centralization.
- IPFS cluster ensures CAR availability; fallback HTTP servers available.
- Monitor content availability via `consensus_availability_checks` (periodic fetch/verify from multi-region vantage points).

## Observability & Alerts
- Metrics:
  - `soranet_consensus_publish_latency_seconds`
  - `soranet_consensus_validity_seconds`
  - `soranet_descriptor_ingest_total{result}`
  - `soranet_consensus_fetch_failures_total`
  - `soranet_consensus_signature_verification_total{result}`
- Alerts:
  - Missing consensus publication (no update before `fresh_until`).
  - Invalid descriptor signatures.
  - Consensus availability < 99% (from availability checks).
  - Salt rotation schedule mismatch.

## Testing & Rollout
- Unit tests for descriptor parsing, consensus generation, signature verification.
- Integration tests with simulated committee (5 nodes) verifying consensus distribution and validation.
- Availability tests using IPFS + HTTP fallback.
- Rollout steps:
  1. Build staging committee; publish test consensus documents.
  2. Integrate client validation library; ensure SoraFS clients adopt consensus automatically.
  3. Deploy production committee with multi-sig; run shadow mode for 2 weeks.
  4. Switch clients to live consensus feed; monitor metrics.
  5. Document operator runbooks (`specs/soranet/directory_operator.md`).

## Implementation Checklist
- [x] Define descriptor format and consensus document structure.
- [x] Document builder/publisher workflow and storage.
- [x] Provide client validation library requirements.
- [x] Capture observability, emergency procedures, and rollout steps.

## Tooling

The `soranet-directory` CLI (shipped with the relay tooling) automates snapshot
publication and issuer rotation:

- `soranet-directory build --config snapshots/mainnet.json --out out/guard_snapshot.norito`
  verifies every SRCv2 certificate bundle against the supplied issuer keys,
  enforces directory hash/validity invariants, and emits a Norito-encoded
  `GuardDirectorySnapshotV2`. The JSON config requires both Ed25519 and
  ML-DSA-65 issuer keys and bundle paths. The builder always emits the
  first-release mandatory-dual policy and rejects any certificate missing either
  signature; there is no configuration downgrade. The builder prints the
  domain-separated BLAKE3 digest of the exact snapshot bytes alongside
  fingerprint metadata.
  Governance must distribute that snapshot digest through a channel independent
  of the snapshot host; relay and client runtimes fail closed unless the
  configured digest matches.
  The config now also accepts optional guard-pinning evidence via
  `guard_pinning_proofs` entries pointing at the JSON proofs produced by relays:
  ```json
  {
      "issuers": [ ... ],
      "bundles": [ ... ],
      "guard_pinning_proofs": [
          { "path": "evidence/relay-entry01.json" },
          { "path": "evidence/relay-entry02.json" }
      ]
  }
  ```
  Each proof is verified against the snapshot being built, and the CLI renders a
  summary (relay id, descriptor commit, weights, validity window, and recorded
  timestamp) so publishers can staple the evidence bundle directly into
  governance packets.
- `soranet-directory rotate --snapshot guard_snapshot.norito --expected-snapshot-digest <independently-approved-source-digest> --out rotated.norito --keys-out rotated_keys`
  reissues certificates with freshly generated issuer keys, updating the
  fingerprint, signatures, and issuer signing material stored on disk. The command
  requires a new `--keys-out` directory and durably publishes its complete
  owner-private Ed25519/ML-DSA key bundle before atomically publishing the
  snapshot, so the governance committee can enrol the new issuer in hardware
  security modules before activation. A key-bundle failure leaves the snapshot
  unpublished; a rejected snapshot rename rolls the newly created bundle back.
  Once the snapshot rename succeeds, any subsequent durability error retains
  both artefacts so a visible snapshot is never left without its issuer keys.
  Before generating anything, it authenticates the exact source bytes against
  the supplied digest and enforces `valid_after <= now < valid_until`; an
  explicit ceremony time may be supplied with `--at-unix`.
  Rotation accepts only snapshots whose relay certificates carry valid
  Ed25519 and ML-DSA-65 signatures.
- `soranet-directory inspect --snapshot guard_snapshot.norito` renders snapshot
  metadata (exact snapshot digest, fingerprints, validity windows, relay stats)
  to simplify guard directory audits. This is structural inspection against
  issuer keys embedded in the snapshot, not authentication; compare the printed
  digest with the independently distributed governance value before trusting
  the artefact.
- `soranet-directory verify-proof --proof /var/lib/soranet/relay/guard_pinning_proof.json
  --snapshot snapshots/mainnet.norito` loads the persisted JSON proof emitted by
  a relay, recomputes the directory hash, relay entry fields, weights, and
  validity window inside the supplied snapshot, and fails fast
  whenever a mismatch occurs. Provide `--snapshot` when the publisher stores snapshots in a
  different path than the relay’s recorded `snapshot_path` so auditors can still
  validate remote submissions.
- `soranet-directory collect-proofs --snapshot snapshots/mainnet.norito --proofs-dir evidence/guard-proofs
  --out artifacts/guard_pinning_summaries.json` scans a directory of relay-submitted
  pinning artefacts, verifies each proof against the supplied snapshot, prints a summary
  table, and optionally emits a JSON array of the verified summaries. Directory publishers
  can now ingest `guard_directory.pinning_proof_path` outputs directly rather than rewriting
  ad-hoc scripts for every governance cycle, ensuring that the evidence bundle stapled to the
  guard snapshot always lists the relay id, descriptor commit, weights, and validity
  window recorded at the relay.

The CLI shares the new directory primitives with the orchestrator so guard caches
consume the same Norito structures that the publisher emits. See
`tools/soranet-relay/src/directory.rs` for the builder/rotator implementation.
Relays that set `guard_directory.pinning_proof_path` write a JSON artefact
containing the validated directory hash, relay identifier, descriptor commit,
certificate validity window, and weights every time the snapshot check
succeeds. Relay identifiers use canonical lowercase hex so one relay cannot occupy
multiple evidence slots through alternate encodings. Static KEM metadata is deliberately absent. Directory publishers ingest these files alongside
operator evidence to prove that every guard pinned the committee-issued
descriptor before activation.

With this specification, core protocol teams can implement the directory publisher tooling required for SoraNet operations, ensuring clients receive authenticated, fresh relay information and salt schedules.
