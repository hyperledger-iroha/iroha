# Atomic Private Settlement V1 Threat Model

## Status and scope

This document models the first-release `AtomicPrivateSettlementV1` path. It is
implementation-coupled security evidence, not an independent review. The
feature remains disabled by default and must not be described as production
ready until the cryptographic, fault, leakage, performance, build, and external
audit gates in `specs/private_settlement.md` have passed.

The model covers:

- the public and principal-authenticated Torii routes in
  `crates/iroha_torii_shared/src/route_catalog/private_settlement.rs`;
- the restricted encrypted sidecar store in
  `crates/iroha_core/src/private_settlement/sidecar_store.rs`;
- capsule encryption and auditor approval in
  `crates/iroha_core/src/private_settlement/{audit,auditor}.rs`;
- four-validator availability, Prepare, and Commit certification in
  `crates/iroha_core/src/private_settlement/{availability,phase,protocol}.rs`;
- proof verification and fixed-shape note transitions in
  `crates/iroha_core/src/privacy_engines/atomic_private_settlement/`;
- the atomic carrier and WSV transition in
  `crates/iroha_core/src/private_settlement/{carrier,global_state}.rs` and
  `crates/iroha_core/src/state.rs`.

Client endpoint security, operating-system hardening, signing and decryption
custody-provider behavior, and legal sufficiency of regulator access are
deployment concerns, but their failure modes are considered where they cross
the protocol boundary. Every provider must meet the same qualification contract.

## Conservative deployment assumptions

Unless an operator establishes stronger controls, assume all Torii routes are
internet reachable; capsule plaintext is regulated, high-sensitivity financial
data; auditor signing and decryption keys are held by independently controlled,
authenticated custody providers that do not persist plaintext private key
material in configuration, repository files, or logs; and validator consensus
keys are never reused as auditor keys. Provider implementation details remain
deployment-owned. The protocol rejects identical signing,
encryption, and consensus keys even when an auditor is also a committee
validator.

The security argument assumes:

- standard cryptographic assumptions hold for SHA-2, Ed25519, X25519,
  ML-KEM-768, XChaCha20-Poly1305, and the pinned IVM private-note STARK profile;
- every participating dataspace has an exact four-validator authority with at
  most one unavailable or Byzantine validator;
- a policy-satisfying set of auditors is available and validates plaintext
  independently before signing;
- global consensus finality and mandatory signed RS16 DA/RBC behave according
  to their existing Iroha 3 assumptions;
- clients generate witnesses locally, verify public bindings, and do not send
  spending secrets to Torii, Python, committee validators, or auditors;
- operator clocks are irrelevant to safety: expiry and policy validity use
  authoritative block heights.

Loss of auditor availability stops liveness but must not weaken privacy or
atomic safety. Compromise of a threshold of authorized auditors reveals the
capsules they can decrypt and may authorize policy-invalid plaintext if they
collude, but it must not let them forge committee QCs, create valid ZK proofs,
or obtain note spending secrets.

## Assets and security objectives

| Asset | Objective |
|---|---|
| Account, asset, amount, memo, business result, note openings, and view data | Confidential to the governed local auditors; absent from public state, ordinary-validator views, errors, logs, events, and telemetry |
| Spending authorities and prover witness | Never included in audit capsules or network payloads; zeroized at terminal wallet boundaries |
| Pool roots, nullifiers, commitments, encrypted outputs, receipts, and replay markers | All-leg atomic integrity and deterministic replay protection |
| Audit policy, key epoch, approval threshold, and authority roster | Governance integrity, exact-context binding, and fail-closed rotation |
| Encrypted sidecars and staged deltas | Durable availability through expiry/finality; access limited to exact principals |
| Auditor, availability, phase, and consensus private keys | Purpose separation and confidentiality/integrity appropriate to each role |
| Consensus and coordinator liveness | Progress with one unavailable validator per exact four-member committee and sufficient auditors/global quorum |

## Trust boundaries and data flow

```text
client wallet
  | encrypted capsule + proof + opaque fixed-shape delta
  v
restricted Torii/DA ---- committee validator (proof only)
  |                              |
  | capsule                      | 3-of-4 Prepare/Commit votes
  v                              v
authorized local auditor    complete-bundle coordinator
  | approval                     |
  +------------------------------+
                                 v
                     sponsor-signed global carrier
                                 |
                                 v
                   one atomic StateTransaction / receipt
```

The public boundary intentionally exposes network and bundle identifiers,
ordered dataspace routes and participant count, authority/expiry/finality
heights, opaque pool identifiers, roots, nullifiers, commitments, ciphertexts,
availability and proof digests, key epochs, QCs, sponsor, public fee intent, and
terminal status. It does not promise to hide timing, traffic volume, stable-pool
activity, participant count, or the fact that a named dataspace participated.

## Attacker model

In scope are unauthenticated remote clients, authenticated malicious clients,
one Byzantine validator in each exact four-member committee, unavailable or
restarting nodes, malicious relayers/sponsors, compromised non-threshold
auditors, storage readers, packet observers, and parties able to delay, replay,
drop, reorder, or substitute network messages. Also in scope are crash and disk
faults at every documented persistence boundary.

No attacker is assumed able to break the cryptographic primitives, control two
validators in an exact four-member committee, compromise a policy-satisfying
auditor threshold while also violating the auditor trust assumption, or violate
global-consensus safety. Those are explicit assumption failures and require
governance suspension, key rotation, incident response, and potentially
capsule rewrapping.

## Principal threats and controls

| ID | Threat and impact | Existing or required control | Residual risk / release evidence |
|---|---|---|---|
| APS-T01 | A public caller fetches proof or capsule material. | Canonical account signatures protect uploads/status; identity-bound signatures authorize exact committee members and governed auditors; the capsule read is a signed read-only `POST` whose strict body carries the complete access policy. Core treats that body as evidence, resolves historical and current policy revisions from one committed `StateView`, reads exactly one content-addressed sidecar, rejects consensus-key reuse, and maps the current signing key to a stable historical auditor identity and wrapped DEK. Denied, missing, and retention-expired reads return the same unavailable class; restricted responses use `no-store`. | Traffic observers still learn endpoint timing and response sizes. Denial, policy-substitution, stable-identity, and packet-capture tests are release gates. |
| APS-T02 | A capsule, wrapped DEK, policy, route, bundle, leg, historical authority, or key epoch is substituted. | XChaCha20-Poly1305 authenticates padded plaintext; X25519/ML-KEM-768 wraps the random DEK independently to each governed auditor. Capsule AAD binds the exact authority digest and `authority_context_height` with every settlement context field; wrap AAD includes that complete capsule AAD plus the exact auditor, recipient hybrid key, and KEM ciphertext. Approval signatures bind statement/proof/capsule/policy digests and expiry. | Cryptographic implementation and AAD completeness require independent review and adversarial vectors. |
| APS-T03 | A validator, sponsor, or ordinary status caller learns plaintext or local audit progress from its protocol view. | Committee APIs return proof plus opaque delta, never capsule plaintext; ordinary leg status omits approval counts and the governed threshold; the carrier contains only the public allowlist; proof statements use salted asset bindings and fixed ciphertext shape. | Stable pool, route, roots, timing, participant count, and single-asset dataspace inference remain public/linkable. Differential leakage results must report this honestly. |
| APS-T04 | Dummy selectors or unused fixed slots leak cardinality or permit value creation. | Exactly two input and three output slots are encoded; inactive slots are domain-separated, zero-value, non-spendable dummies; the relation enforces balance and forbids the directional public-balance bridge. | AIR, selector, nullifier, and three-output constraints require independent cryptographic review plus positive/negative proof tests. |
| APS-T05 | One leg commits without every other leg. | All Prepare QCs precede one sponsor-authorized complete-barrier control carrier. Commit voters require that exact barrier and all of its opaque resource rows in replicated WSV before signing; every Commit QC binds the normalized complete bundle digest containing every certified statement and authority index. Commit QCs do not mutate WSV; one global `StateTransaction` validates every leg and the exact lock set before infallible financial writes and lock removal. | Global-consensus safety is inherited. Byte-identical failure tests, TLC models, crash cuts, and real-process visibility assertions are mandatory. |
| APS-T06 | A stale root, duplicate nullifier/output, old epoch, cross-route alias, or replayed bundle is accepted. | Committee-local staging durably reserves pool heads by `(route, pool_id, epoch, root)`, nullifiers by `(route, pool_id, nullifier)`, and outputs by `(route, pool_id, commitment)`. The complete Prepare registration independently installs consensus-replicated opaque pool-head/nullifier/output/recipient locks, rejects conflicts against active locks and finalized state, and is idempotent only for the exact barrier. Finalization rechecks current WSV roots, epochs, governance, unique exact-route state keys, prior receipts, abort markers, and the exact registered lock set. | Restart/finality reconciliation must rebuild both lock layers and release only exact terminal/expired reservations; disk corruption must fail closed. |
| APS-T07 | A caller-selected, stale, forged, or malformed roster/quorum bypasses a committee. | At `authority_context_height`, every node resolves the exact ordered lane/dataspace roster and active incarnation from consensus state, requires the resolved authority height to match, enforces exactly four validators with `f = 1`, and verifies their BLS proofs of possession. Certificates require exactly three distinct canonical signers over purpose-separated bodies. | Safety assumes at most one Byzantine validator per state-anchored committee. Historical-roster, withheld, delayed, reordered, and healed-vote controls remain release evidence. |
| APS-T08 | An auditor signs plaintext that does not match public commitments or violates local policy. | The auditor decrypts locally, validates canonical padded plaintext, recomputes commitments, checks policy/key validity and availability, and signs a purpose-separated approval; every validator revalidates the approval threshold. | A policy-threshold compromise defeats content-policy assurance and capsule confidentiality for that policy. Separate organizations and independently qualified custody controls are recommended. |
| APS-T09 | Policy/key rotation makes retained capsules undecryptable, invalidates historical receipts, lets a newly added auditor inherit old ciphertext, or lets stale in-flight approvals cross the boundary. | `RotatePrivateSettlementPoolPolicyV1` requires privacy-governance authority, the exact predecessor digest, immutable route/pool/asset binding, the next revision, a newer key epoch, and activation at its inclusion height. It preserves the pool frontier/replay sets and a gap-free public revision lineage, rejects a same-height receipt for the exact route/pool, keeps pre-rotation finalized receipts restart-valid as historical evidence while rejecting exact replay without mutation, and makes old-policy in-flight bundles fail closed. A successor-policy read requires the same stable auditor identity in the historical policy and wrapped-DEK set; approval submission still requires the historical policy itself. The software retained-key provider selects the exact governed historical key instead of trial decrypting. | Operators must retain retired decryption keys or govern and test capsule rewrapping through the retention period. Qualified custody and rewrapping evidence remain deployment-controlled release gates. |
| APS-T10 | A malicious sponsor changes the public fee or reimbursement terms, or gets reimbursed on failure. | The manifest and proof bind the exact public fee intent, sponsor, reimbursement commitment, and designated reimbursement leg; the third output is created only in the atomic global transition. | Public fee and sponsor identity are intentionally visible. Fee-quote freshness and operational sponsorship availability affect liveness. |
| APS-T11 | Oversized proofs, capsules, auditor rosters, participant lists, or carrier records exhaust CPU, memory, disk, or network, or a policy weakens the governed auditor floor. | Configuration and hard protocol bounds cap 2–255 legs, proofs, the whole canonical capsule (AAD, ciphertext, framing, and every wrapped-DEK row), the complete canonical sponsor-signed carrier transaction, padding classes, retention, file count, and total sidecar bytes. Coordinator/WSV preflight includes registered instruction framing; Torii and the core one-shot binding include authority, metadata, fee intent, and signature. Configuration proves each padding class can fit the conservative complete envelope for the governed minimum approval count; admission rejects an actual canonical capsule over the byte limit or a policy threshold below `default_min_auditor_approvals`. | Internet-facing deployments still need connection/rate quotas. Larger auditor rosters consume capsule budget. N=255 codec/wire-size tests and N=2/3/4/8/16 resource measurements are release gates. |
| APS-T12 | Crash/restart loses a staged delta, emits a QC before durability, permits Commit before global registration, or leaves locks forever. | Sidecars use owner-only directories/files, create-new temp files, canonical decode/re-encode checks, fsync-before-rename, directory fsync, a single-writer lease, and restart reservation reconstruction. Prepare QCs are issued only after local durability. The sponsor waits for the exact registration transaction's global state-resolved `Applied` status before Commit fanout; every Commit voter rechecks the exact replicated lock. Finalization/abort removes the full registered set atomically and deterministic block-start expiry sweeps stale sets. | Every persistence cut, including global lock registration, global receipt publication, and startup reconciliation, must be tested on real processes. Storage media/fsync honesty remains an operator assumption. |
| APS-T13 | Logs, metrics, errors, snapshots, Kura artifacts, or SDK exceptions reveal canaries. | Public DTOs are redacted; error codes are stable and non-oracular; the leakage harness scans multiple encodings and compares public shapes and capture-derived traffic counts when only secrets change. It retains and independently replays the unfiltered pcap, exact restricted source bytes, and all-peer atomicity observations instead of trusting digest projections. | Allocation dumps, host compromise, and privileged debugger access are outside protocol confidentiality. Release evidence must include sanitized captures and a documented allowlist. |
| APS-T14 | Governance enables code with the wrong compiled proof profile or too little notice. | Activation is disabled by default; Torii and WSV admission independently require an active compiled IVM private-note profile, fixed slot limits, permitted V1 policy, governed activation height, and minimum notice. | Governance/key compromise can still activate reviewed-but-malicious code. Reproducible build, SBOM, signed baseline, and independent audit evidence are required. |
| APS-T15 | A malicious client or Python integration exfiltrates the witness. | The Rust wallet owns witness bytes in an owner-only one-shot bundle; Python receives only an opaque handle and public result; terminal success or failure consumes and zeroizes secret material. | Host-process compromise can inspect memory. Wallet APIs must document the boundary and avoid debug/serialization traits for secret types. |
| APS-T16 | A coordinator crashes after a committee durably records a Prepare or Commit QC but before the sponsor receives it, then reconstructs a different valid 3-of-4 signer set. | The prepared-bundle digest commits to every signed body and authority index but normalizes quorum-equivalent signer bitmaps and aggregate signatures. Recovery queries three distinct committee endpoints, cryptographically validates returned certificates and bundle bindings, canonically selects an equivalent QC, re-fans it out, and re-queries for the same durable certified body. Equivalent replay after `CommitCertified` is write-free and cannot regress lifecycle. | Concurrent coordinators can choose different certificate encodings, but those encodings certify the same body and produce the same Commit digest. The finalized receipt retains the exact QCs presented in its carrier, while the Commit signature proves the certified statement rather than unique historical signer-subset provenance; deployments needing that provenance must retain the authenticated coordinator transcript. Real-process crash/partition tests must still prove convergence, receipt validity, and no partial visibility with one unavailable or Byzantine validator. |
| APS-T17 | Reusing an output recipient/view key links otherwise private notes across legs or later bundles. | Public statement and delta validation require three distinct non-zero recipient identifiers; the all-leg Prepare barrier and final receipt reject reuse across legs. Committee Prepare checks the globally derived recipient index, and global planning checks it again before any write. Finalization inserts all three recipients atomically into a permanent index; restart deterministically rebuilds that index from canonical encrypted outputs and rejects duplicates or index drift. | A malicious client can still publish linkable rejected material or reuse a key outside this protocol. V1 does not claim to hide network-level pre-publication behavior; wallet guidance, denial tests, and leakage reporting must state that limit. |
| APS-T18 | Snapshot rollback, partial schema migration, or corruption silently drops private-settlement replay or active Prepare-lock state. | Current snapshots require the exact 188-field world schema, including all eight canonical settlement maps. The sole feature-predecessor migration accepts only the exact 180-field `World` emitted by revision `1bdec3b88c348a84776241839fb0e8ad71738b3e`: all five retired SCCP/direct-lane/Parliament stores must decode with their historical types and have empty history/current maps, then all 13 successor fields are synthesized empty. The full-State bridge also requires the exact retired HSM defaults and reinstates them for historical hashing atomically with the 13/5 `World` bridge; partial and intermediate schemas are rejected. Populated roots, nullifiers, outputs, replicated staged locks, receipts, and terminal markers remain committed. The snapshot-skipped recipient index is reconstructed from canonical outputs and cross-checked before startup accepts state. | This is a feature-delta migration, not a promise to accept every unrelated historical world schema. Operators must follow supported upgrade sequencing and retain signed snapshot/Kura recovery evidence. |

## Privacy statement and residual leakage

The protocol provides content confidentiality, not relationship anonymity or
traffic-flow confidentiality. A public observer can correlate stable opaque pool
identifiers and successive roots, see which dataspaces participate, count legs,
observe sponsor and fee data, and measure phase/finality timing. If a dataspace
hosts only one CBDC, its opaque pool activity can support asset inference even
though no literal asset identifier appears. Network-level padding does not hide
the number of protocol phases or participants unless a deployment adds an
independent cover-traffic layer, which V1 does not claim.

Auditors see exact parties, asset, amount, memo, policy references, view data,
and note openings for their local leg. They do not receive spending secrets or
other dataspaces' capsules merely by participating. Ordinary validators can
verify the proof, approvals, current root, fixed nullifier/output shape, expiry,
and durable availability while remaining unable to decrypt the capsule.

## Incident and governance response

- Suspected proof-profile or state-machine defect: suspend the governed privacy
  protocol and private-settlement admission; do not reinterpret existing
  receipts or accept a transparent fallback for private bundles.
- Auditor signing-key compromise: retire the key epoch, raise or replace the
  policy threshold through the governed rotation instruction, reject stale
  approvals and old-policy in-flight bundles, and audit approvals issued during
  the exposure window. Do not place a same-pool finalization at the rotation's
  activation height.
- Auditor decryption-key compromise: treat all capsules wrapped to that key as
  exposed, rotate/retire the key, rewrap retained capsules where policy permits,
  and preserve evidence required by the retention regime.
- Validator/phase-key compromise: replace the committee incarnation through
  governance; old-roster votes cannot be mixed with the new authority.
- Sidecar corruption or ambiguous finality: fail closed, reconstruct from
  authenticated restricted DA and immutable WSV receipts, and never release a
  nonterminal reservation merely because a local file is missing.

## Required closure evidence

This threat model is closed for release only when all of the following are
archived against the exact commit and manifest hashes:

1. independent review of the AIR, dummy selectors, asset/capsule bindings,
   reimbursement relation, cryptography, and cross-dataspace state machine;
2. focused and workspace tests, strict clippy/format, ten randomized seeds,
   two-hour soak, serial privacy-release checks, reproducible build, and SBOM;
3. real four-validator-per-dataspace fault matrices for N=2,3,4,8,16 with
   mandatory DA/RBC, one-validator restarts, authenticated loss/partition
   controls, all persistence cuts, and continuous no-partial-visibility checks;
4. canary and differential leakage analysis across Torii, restricted/public
   P2P, block wire, Kura/merge, snapshots, queries, events, logs, and telemetry;
5. latency/resource distributions and transparent-AMX controls on pinned
   hardware, with raw CSV/JSON, configurations, captures, plots, limitations,
   and signed baselines in a DOI-backed artifact.
