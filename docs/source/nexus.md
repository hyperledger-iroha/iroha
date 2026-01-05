#! Iroha 3 – Sora Nexus Ledger: Technical Design Specification

This document proposes the Sora Nexus Ledger architecture for Iroha 3, evolving Iroha 2 toward a single global, logically unified ledger organized around Data Spaces (DS). Data Spaces provide strong privacy domains (“private data spaces”) and open participation (“public data spaces”). The design preserves composability across the global ledger while ensuring strict isolation and confidentiality for private‑DS data, and introduces data‑availability scaling via erasure coding across Kura (block storage) and WSV (World State View).

The same repository builds both Iroha 2 (self-hosted networks) and Iroha 3 (SORA Nexus). Execution is powered by
the shared Iroha Virtual Machine (IVM) and Kotodama toolchain, so contracts and bytecode artifacts remain
portable across self-hosted deployments and the Nexus global ledger.

Goals
- One global logical ledger composed from many cooperating validators and Data Spaces.
- Private Data Spaces for permissioned operation (e.g., CBDCs), with data never leaving the private DS.
- Public Data Spaces with open participation, Ethereum-like permissionless access.
- Composable smart contracts across Data Spaces, subject to explicit permissions for access to private‑DS assets.
- Performance isolation so public activity cannot degrade private‑DS internal transactions.
- Data availability at scale: erasure‑coded Kura and WSV to support effectively unbounded data while keeping private‑DS data private.

Non‑Goals (Initial Phase)
- Defining token economics or validator incentives; scheduling and staking policies are pluggable.
- Introducing a new ABI version; changes target ABI v1 with explicit syscall and pointer‑ABI extensions per IVM policy.

Terminology
- Nexus Ledger: The global logical ledger formed by composing Data Space (DS) blocks into a single, ordered history and state commitment.
- Data Space (DS): A bounded execution and storage domain with its own validators, governance, privacy class, DA policy, quotas, and fee policy. Two classes exist: public DS and private DS.
- Private Data Space: Permissioned validators and access control; transaction data and state never leave the DS. Only commitments/metadata are anchored globally.
- Public Data Space: Permissionless participation; full data and state are publicly available.
- Data Space Manifest (DS Manifest): A Norito-encoded manifest that declares DS parameters (validators/QC keys, privacy class, ISI policy, DA parameters, retention, quotas, ZK policy, fees). The manifest hash is anchored on the nexus chain. Unless overridden, DS quorum certificates use ML‑DSA‑87 (Dilithium5‑class) as the default post‑quantum signature scheme.
- Space Directory: A global on‑chain directory contract that tracks DS manifests, versions, and governance/rotation events for resolvability and audits.
- DSID: A globally unique identifier for a Data Space. Used to namespace all objects and references.
- Anchor: A cryptographic commitment from a DS block/header included into the nexus chain to bind DS history into the global ledger.
- Kura: Iroha block storage. Extended here with erasure‑coded blob storage and commitments.
- WSV: Iroha World State View. Extended here with versioned, snapshot‑capable, erasure‑coded state segments.
- IVM: Iroha Virtual Machine for smart contract execution (Kotodama bytecode `.to`).
 - AIR: Algebraic Intermediate Representation. An algebraic view of computation for STARK‑style proofs, describing execution as field‑based traces with transition and boundary constraints.

Data Spaces Model
- Identity: `DataSpaceId (DSID)` identifies a DS and namespaces everything. DS can be instantiated at two granularities:
  - Domain‑DS: `ds::domain::<domain_name>` — execution and state scoped to a domain.
  - Asset‑DS: `ds::asset::<domain_name>::<asset_name>` — execution and state scoped to a single asset definition.
  Both forms coexist; transactions can touch multiple DSIDs atomically.
- Manifest lifecycle: DS creation, updates (key rotation, policy changes), and retirement are recorded in the Space Directory. Each per‑slot DS artifact references the latest manifest hash.
- Classes: Public DS (open participation, public DA) and Private DS (permissioned, confidential DA). Hybrid policies are possible via manifest flags.
- Policies per DS: ISI permissions, DA parameters `(k,m)`, encryption, retention, quotas (min/max tx share per block), ZK/optimistic proof policy, fees.
- Governance: DS membership and validator rotation defined by the manifest’s governance section (on-chain proposals, multisig, or external governance anchored by nexus transactions and attestations).

Dataspace-aware gossip
- Transaction gossip batches now carry a plane tag (public vs restricted) derived from the lane catalog; restricted batches are unicast to the online peers in the current commit topology (respecting `transaction_gossip_restricted_target_cap`) while public batches use `transaction_gossip_public_target_cap` (set `null` for broadcast). Target selection reshuffles on the per-plane cadence set by `transaction_gossip_public_target_reshuffle_ms` and `transaction_gossip_restricted_target_reshuffle_ms` (default: `transaction_gossip_period_ms`). When no commit-topology peers are online, operators can choose to either refuse or forward restricted payloads onto the public overlay via `transaction_gossip_restricted_public_payload` (default `refuse`); telemetry surfaces fallback attempts, forward/drop counts, and the configured policy alongside per-dataspace target selections.
- Unknown dataspaces are re-queued when `transaction_gossip_drop_unknown_dataspace` is enabled; otherwise they fall back to restricted targeting to avoid leaks.
- Receive-side validation drops batches whose lanes/dataspaces disagree with the local catalog or whose plane tag does not match the derived dataspace visibility.

Capability manifests & UAID
- Universal accounts: Every participant receives a deterministic UAID (`UniversalAccountId` in `crates/iroha_data_model/src/nexus/manifest.rs`) that spans all dataspaces. Capability manifests (`AssetPermissionManifest`) bind a UAID to a specific dataspace, activation/expiry epochs, and an ordered list of allow/deny `ManifestEntry` rules that scope `dataspace`, `program_id`, `method`, `asset`, and optional AMX roles. Deny rules always win; the evaluator emits either `ManifestVerdict::Denied` with an audit reason or an `Allowed` grant with the matching allowance metadata.
- UAID portfolio snapshots are now exposed via `GET /v1/accounts/{uaid}/portfolio` (see `docs/source/torii/portfolio_api.md`), backed by the deterministic aggregator in `iroha_core::nexus::portfolio`.
- Allowances: Each allow entry carries deterministic `AllowanceWindow` buckets (`PerSlot`, `PerMinute`, `PerDay`) plus an optional `max_amount`. Hosts and SDKs consume the same Norito payload, so enforcement remains identical across hardware and SDK implementations.
- Audit telemetry: The Space Directory broadcasts `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) whenever a manifest changes state. The new `SpaceDirectoryEventFilter` surface allows Torii/data-event subscribers to monitor UAID manifest updates, revocations, and deny-wins decisions without custom plumbing.

### UAID manifest operations

Space Directory operations ship in two forms so operators can choose either the
in-binary CLI (for scripted rollouts) or direct Torii submissions (for automated
CI/CD). Both paths enforce the `CanPublishSpaceDirectoryManifest{dataspace}`
permission inside the executor (`crates/iroha_core/src/smartcontracts/isi/space_directory.rs`)
and record lifecycle events in world state (`iroha_core::state::space_directory_manifests`).

#### CLI workflow (`iroha space-directory manifest …`)

1. **Encode manifest JSON** — convert policy drafts into Norito bytes and emit a
   reproducible hash before review:

   ```bash
   iroha space-directory manifest encode \
     --json dataspace/capability.json \
     --out artifacts/capability.manifest.to \
     --hash-out artifacts/capability.manifest.hash
   ```

   The helper accepts either `--json` (raw JSON manifest) or `--manifest` (existing
   `.to` payload) and mirrors the logic in
   `crates/iroha_cli/src/space_directory.rs::ManifestEncodeArgs`.

2. **Publish/replace manifests** — enqueue `PublishSpaceDirectoryManifest`
   instructions from either Norito or JSON sources:

   ```bash
   iroha space-directory manifest publish \
     --manifest artifacts/capability.manifest.to \
     --reason "Retail wave 4 on-boarding"
   ```

   `--reason` backfills `entries[*].notes` for records that omitted operator notes.

3. **Expire** manifests that reached their scheduled end of life or **revoke**
   UAIDs on demand. Both commands accept `--uaid uaid:<hex>` and the numeric
   dataspace id:

   ```bash
   iroha space-directory manifest expire \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --expired-epoch 4600

   iroha space-directory manifest revoke \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --revoked-epoch 9216 \
     --reason "Fraud investigation NX-16-R05"
   ```

4. **Produce audit bundles** — `manifest audit-bundle` writes the manifest JSON,
   `.to` payload, hash, dataspace profile, and machine-readable metadata to an
   output directory so governance reviewers can download a single archive:

   ```bash
   iroha space-directory manifest audit-bundle \
     --manifest-json dataspace/capability.json \
     --profile dataspace/profiles/cbdc_profile.json \
     --out-dir artifacts/capability_bundle
   ```

   The bundle embeds `SpaceDirectoryEvent` hooks from the profile to prove the
   dataspace exposes the mandatory audit webhooks; see `docs/space-directory.md`
   for the field layout and evidence requirements.

#### Torii APIs

Operators and SDKs can perform the same actions over HTTPS. Torii enforces the
same permission checks and signs transactions on behalf of the supplied
authority (private keys travel only in-memory inside Torii’s secure handler):

- `GET /v1/space-directory/uaids/{uaid}` — resolve the current dataspace bindings
  for a UAID (normalized addresses, dataspace ids, program bindings). Add
  `address_format=compressed` for Sora Name Service output.
- `GET /v1/space-directory/uaids/{uaid}/portfolio` —
  Norito-backed aggregator that mirrors `ToriiClient.getUaidPortfolio` so wallets
  can render universal holdings without scraping per-dataspace state.
- `GET /v1/space-directory/uaids/{uaid}/manifests?dataspace={id}` — fetch the
  canonical manifest JSON, lifecycle metadata, and manifest hash for audits.
- `POST /v1/space-directory/manifests` — submit new or replacement manifests
  from JSON (`authority`, `private_key`, `manifest`, optional `reason`). Torii
  returns `202 Accepted` once the transaction is queued.
- `POST /v1/space-directory/manifests/revoke` — enqueue emergency revocations
  with the UAID, dataspace id, effective epoch, and optional reason (mirrors the
  CLI layout).

The JS SDK (`javascript/iroha_js/src/toriiClient.js`) already wraps these read
surfaces via `ToriiClient.getUaidPortfolio`, `.getUaidBindings`, and
`.getUaidManifests`; future Swift/Python releases reuse the same REST payloads.
Reference `docs/source/torii/portfolio_api.md` for complete request/response
schemas and `docs/space-directory.md` for the end-to-end operator playbook.

Recent SDK/AMX updates
- **NX-11 (cross-lane relay verification):** SDK helpers now validate the lane relay
  envelopes exposed by `/v1/sumeragi/status`. The Rust client ships `iroha::nexus`
  helpers for building/verifying relay proofs and rejecting duplicate `(lane_id,
  dataspace_id, height)` tuples, the Python binding exposes
  `verify_lane_relay_envelope_bytes`/`lane_settlement_hash`, and the JS SDK surfaces
  `verifyLaneRelayEnvelope`/`laneRelayEnvelopeSample` so operators can gate cross-lane
  transfer proofs with consistent hashes before forwarding them downstream.【crates/iroha/src/nexus.rs:1】【python/iroha_python/iroha_python_rs/src/lib.rs:666】【crates/iroha_js_host/src/lib.rs:640】【javascript/iroha_js/src/nexus.js:1】
- **NX-17 (AMX budget guardrails):** `ivm::analysis::enforce_amx_budget` estimates
  per-dataspace and group execution cost using the static analysis report and enforces
  the 30 ms / 140 ms budgets captured here. The helper surfaces clear violations for
  per-DS and group budgets and is covered by unit tests, making the AMX slot budget
  deterministic for Nexus schedulers and SDK tooling.【crates/ivm/src/analysis.rs:142】【crates/ivm/src/analysis.rs:241】

High‑Level Architecture
1) Global Composition Layer (Nexus Chain)
- Maintains a single, canonical ordering of 1‑second Nexus Blocks that finalize atomic transactions spanning one or more Data Spaces (DS). Every committed transaction updates the unified global world state (vector of per‑DS roots).
- Contains minimal metadata plus aggregated proofs/QCs to ensure composability, finality, and fraud detection (DSIDs touched, per‑DS state roots before/after, DA commitments, per‑DS validity proofs, and the DS quorum certificate using ML‑DSA‑87). No private data is included.
- Consensus: Single global, pipelined BFT committee of size 22 (3f+1 with f=7), selected from a pool of up to ~200k potential validators by an epochal VRF/stake mechanism. The nexus committee sequences transactions and finalizes the block within 1s.

2) Data Space Layer (Public/Private)
- Executes per‑DS fragments of global transactions, updates DS‑local WSV, and produces per‑block validity artifacts (aggregated per‑DS proofs and DA commitments) that roll up into the 1‑second Nexus Block.
- Private DS encrypt data‑at‑rest and data‑in‑flight among authorized validators; only commitments and PQ validity proofs leave the DS.
- Public DS export full data bodies (via DA) and PQ validity proofs.

3) Atomic Cross‑Data‑Space Transactions (AMX)
- Model: Each user transaction may touch multiple DS (e.g., domain DS and one or more asset DS). It commits atomically in a single Nexus Block or aborts; no partial effects.
- Prepare‑Commit within 1s: For each candidate transaction, touched DS execute in parallel against the same snapshot (start‑of‑slot DS roots) and produce per‑DS PQ validity proofs (FASTPQ‑ISI) and DA commitments. The nexus committee commits the transaction only if all required DS proofs verify and the DA certificates arrive (≤300 ms target); otherwise the transaction is re‑scheduled for the next slot.
- Consistency: Read‑write sets are declared; conflict detection occurs at commit against the start‑of‑slot roots. Lock‑free optimistic execution per DS avoids global stalls; atomicity is enforced by the nexus commit rule (all‑or‑nothing across DS).
- Privacy: Private DS export only proofs/commitments tied to pre/post DS roots. No raw private data leaves the DS.

4) Data Availability (DA) with Erasure Coding
- Kura stores block bodies and WSV snapshots as erasure-coded blobs. Public blobs are widely sharded; private blobs are stored only within private‑DS validators, with encrypted chunks.
- DA Commitments are recorded in both DS artifacts and Nexus Blocks, enabling sampling and recovery guarantees without revealing private contents.

Block and Commit Structure
- Data Space Proof Artifact (per 1s slot, per DS)
  - Fields: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML‑DSA‑87), ds_validity_proof (FASTPQ‑ISI).
  - Private‑DS export artifacts without data bodies; public DS allow body retrieval via DA.

- Nexus Block (1s cadence)
  - Fields: block_number, parent_hash, slot_time, tx_list (atomic cross‑DS transactions with DSIDs touched), ds_artifacts[], nexus_qc.
  - Function: finalizes all atomic transactions whose required DS artifacts verify; updates the global world state vector of DS roots in one step.

Consensus and Scheduling
- Nexus Chain Consensus: Single global, pipelined BFT (Sumeragi-class) with a 22-node committee (3f+1 with f=7) targeting 1s blocks and 1s finality. Committee members are epochally selected via VRF/stake from ~200k candidates; rotation maintains decentralization and censorship resistance.
- Data Space Consensus: Each DS runs its own BFT among its validators to produce per‑slot artifacts (proofs, DA commitments, DS QC). Lane-relay committees are sized at `3f+1` using the dataspace `fault_tolerance` setting and are sampled deterministically per epoch from the dataspace validator pool using the VRF epoch seed bound with `(dataspace_id, lane_id)`. Private DS are permissioned; public DS allow open liveness subject to anti‑Sybil policies. The global nexus committee remains unchanged.
- Transaction Scheduling: Users submit atomic transactions declaring touched DSIDs and read‑write sets. DS execute in parallel within the slot; the nexus committee includes the transaction in the 1s block if all DS artifacts verify and DA certificates are timely (≤300 ms).
- Performance Isolation: Each DS has independent mempools and execution. Per‑DS quotas bound how many transactions touching a given DS can be committed per block to avoid head‑of‑line blocking and protect private DS latency.

Data Model and Namespacing
- DS‑Qualified IDs: All entities (domains, accounts, assets, roles) are qualified by `dsid`. Example: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Global References: A global reference is a tuple `(dsid, object_id, version_hint)` and can be placed on‑chain in the nexus layer or in AMX descriptors for cross‑DS use.
- Norito Serialization: All cross‑DS messages (AMX descriptors, proofs) use Norito codecs. No serde usage in production paths.

Smart Contracts and IVM Extensions
- Execution Context: Add `dsid` to IVM execution context. Kotodama contracts always execute within a specific Data Space.
- Atomic Cross‑DS Primitives:
  - `amx_begin()` / `amx_commit()` demarcate an atomic multi‑DS transaction in the IVM host.
  - `amx_touch(dsid, key)` declares read/write intent for conflict detection against slot snapshot roots.
  - `verify_space_proof(dsid, proof, statement)` → bool
  - `use_asset_handle(handle, op, amount)` → result (operation permitted only if policy allows and handle is valid)
- Asset Handles and Fees:
  - Asset operations are authorized by the DS’s ISI/role policies; fees are paid in the DS’s gas token. Optional capability tokens and richer policy (multi‑approver, rate‑limits, geofencing) can be added later without changing the atomic model.
- Determinism: All new syscalls are pure and deterministic given inputs and declared AMX read/write sets. No hidden time or environment effects.

Post‑Quantum Validity Proofs (Generalized ISIs)
- FASTPQ‑ISI (PQ, no trusted setup): A kernelized, hash‑based argument that generalizes the transfer design to all ISI families while targeting sub‑second proving for 20k‑scale batches on GPU‑class hardware.
  - Operational profile:
    - Production nodes construct the prover through `fastpq_prover::Prover::canonical`, which now always initialises the production backend; the deterministic mock has been removed.【crates/fastpq_prover/src/proof.rs:126】
    - `zk.fastpq.execution_mode` (config) and `irohad --fastpq-execution-mode` allow operators to pin CPU/GPU execution deterministically while the observer hook records requested/resolved/backend triples for fleet audits.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:2192】【crates/iroha_telemetry/src/metrics.rs:8887】
- Arithmetization:
  - KV‑Update AIR: Treat WSV as a typed key‑value map committed via Poseidon2‑SMT. Each ISI expands to a small set of read‑check‑write rows over keys (accounts, assets, roles, domains, metadata, supply).
  - Opcode‑gated constraints: A single AIR table with selector columns enforces per‑ISI rules (conservation, monotonic counters, permissions, range checks, bounded metadata updates).
  - Lookup arguments: Transparent, hash‑committed tables for permissions/roles, asset precisions, and policy parameters avoid heavy bitwise constraints.
- State commitments and updates:
  - Aggregated SMT Proof: All touched keys (pre/post) are proven against `old_root`/`new_root` using a compressed frontier with deduped siblings.
  - Invariants: Global invariants (e.g., total supply per asset) are enforced via multiset equality between effect rows and tracked counters.
- Proof system:
  - FRI‑style polynomial commitments (DEEP‑FRI) with high arity (8/16) and blow‑up 8–16; Poseidon2 hashes; Fiat‑Shamir transcript with SHA‑2/3.
  - Optional recursion: DS‑local recursive aggregation to compress micro‑batches to one proof per slot if needed.
- Scope and examples covered:
  - Assets: transfer, mint, burn, register/unregister asset definitions, set precision (bounded), set metadata.
  - Accounts/Domains: create/remove, set key/threshold, add/remove signatories (state‑only; signature checks are attested by DS validators, not proven inside the AIR).
  - Roles/Permissions (ISI): grant/revoke roles and permissions; enforced by lookup tables and monotonic policy checks.
  - Contracts/AMX: AMX begin/commit markers, capability mint/revoke if enabled; proven as state transitions and policy counters.
- Out‑of‑AIR checks to preserve latency:
  - Signatures and heavy cryptography (e.g., ML‑DSA user signatures) are verified by DS validators and attested in the DS QC; the validity proof covers only state consistency and policy compliance. This keeps proofs PQ and fast.
- Performance targets (illustrative, 32‑core CPU + single modern GPU):
  - 20k mixed ISIs with small key‑touch (≤8 keys/ISI): ~0.4–0.9 s prove, ~150–450 KB proof, ~5–15 ms verify.
  - Heavier ISIs (more keys/rich constraints): micro‑batch (e.g., 10×2k) + recursion to keep per‑slot <1 s.
- DS Manifest configuration:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (signatures verified by DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (default; alternatives must be explicitly declared)
- Fallbacks:
  - Complex/custom ISIs may use a general STARK (`zk.policy = "stark_fri_general"`) with deferred proof and 1 s finality via QC attestation + slashing on invalid proofs.
  - Non‑PQ options (e.g., Plonk with KZG) require a trusted setup and are no longer supported in the default build.

AIR Primer (for Nexus)
- Execution trace: A matrix with width (register columns) and length (steps). Each row is a logical step of ISI processing; columns hold pre/post values, selectors, and flags.
- Constraints:
  - Transition constraints: enforce row‑to‑row relations (e.g., post_balance = pre_balance − amount for a debit row when `sel_transfer = 1`).
  - Boundary constraints: bind public I/O (old_root/new_root, counters) to the first/last rows.
  - Lookups/permutations: ensure membership and multiset equalities against committed tables (permissions, asset params) without bit‑heavy circuits.
- Commitment and verification:
  - Prover commits to traces via hash‑based encodings and constructs low‑degree polynomials that are valid iff constraints hold.
  - Verifier checks low‑degree via FRI (hash‑based, post‑quantum) with a few Merkle openings; cost is logarithmic in steps.
- Example (Transfer): registers include pre_balance, amount, post_balance, nonce, and selectors. Constraints enforce non‑negativity/range, conservation, and nonce monotonicity, while an aggregated SMT multi‑proof links pre/post leaves to old/new roots.

ABI and Syscall Evolution (ABI v1)
- Syscalls to add (illustrative names):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Pointer‑ABI Types to add:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Required updates:
  - Add to `ivm::syscalls::abi_syscall_list()` (keep ordering), gate by policy.
  - Map unknown numbers to `VMError::UnknownSyscall` in hosts.
  - Update tests: syscall list golden, ABI hash, pointer type ID goldens, and policy tests.
  - Docs: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Privacy Model
- Private Data Containment: Transaction bodies, state diffs, and WSV snapshots for private DS never leave the private validator subset.
- Public Exposure: Only headers, DA commitments, and PQ validity proofs are exported.
- Optional ZK Proofs: Private DS may produce ZK proofs (e.g., balance sufficient, policy satisfied) enabling cross‑DS actions without revealing internal state.
- Access Control: Authorization is enforced by ISI/role policies inside the DS. Capability tokens are optional and can be introduced later if needed.

Performance Isolation and QoS
- Separate consensus, mempools, and storage per DS.
- Nexus scheduling quotas per DS to bound anchor inclusion time and avoid head-of-line blocking.
- Contract resource budgets per DS (compute/memory/IO), enforced by IVM host. Public‑DS contention cannot consume private‑DS budgets.
- Asynchronous cross‑DS calls avoid long synchronous waits inside private‑DS execution.

Data Availability and Storage Design
1) Erasure Coding
- Use systematic Reed‑Solomon (e.g., GF(2^16)) for blob‑level erasure coding of Kura blocks and WSV snapshots: parameters `(k, m)` with `n = k + m` shards.
- Default parameters (proposed, public DS): `k=32, m=16` (n=48), enabling recovery from up to 16 shard losses with ~1.5× expansion. For private DS: `k=16, m=8` (n=24) within the permissioned set. Both are configurable per DS Manifest.
- Public Blobs: Shards distributed across many DA nodes/validators with sampling‑based availability checks. DA commitments in headers allow light clients to verify.
- Private Blobs: Shards encrypted and distributed only within private‑DS validators (or designated custodians). Global chain carries only DA commitments (no shard locations or keys).

2) Commitments and Sampling
- For each blob: compute a Merkle root over shards and include it in `*_da_commitment`. Remain PQ by avoiding elliptic‑curve commitments.
- DA Attesters: VRF‑sampled regional attesters (e.g., 64 per region) issue an ML‑DSA‑87 certificate attesting successful shard sampling. Target DA attestation latency ≤300 ms. Nexus committee validates certificates instead of pulling shards.

3) Kura Integration
- Blocks store transaction bodies as erasure-coded blobs with Merkle commitments.
- Headers carry blob commitments; bodies are retrievable via DA network for public DS and via private channels for private DS.

4) WSV Integration
- WSV Snapshotting: Periodically checkpoint DS state into chunked, erasure-coded snapshots with commitments recorded in headers. Between snapshots, maintain change logs. Public snapshots are widely sharded; private snapshots remain within private validators.
- Proof‑Carrying Access: Contracts can provide (or request) state proofs (Merkle/Verkle) anchored by snapshot commitments. Private DS may supply zero‑knowledge attestations instead of raw proofs.

5) Retention and Pruning
- No pruning for public DS: retain all Kura bodies and WSV snapshots via DA (horizontal scaling). Private DS may define internal retention, but exported commitments remain immutable. Nexus layer retains all Nexus Blocks and DS artifact commitments.

Networking and Node Roles
- Global Validators: Participate in nexus consensus, validate Nexus Blocks and DS artifacts, perform DA checks for public DS.
- Data Space Validators: Run DS consensus, execute contracts, manage local Kura/WSV, handle DA for their DS.
- DA Nodes (optional): Store/publicize public blobs, facilitate sampling. For private DS, DA nodes are co-located with validators or trusted custodians.

System‑Level Improvements and Considerations
- Sequencing/mempool decoupling: Adopt a DAG mempool (e.g., Narwhal‑style) feeding a pipelined BFT at the nexus layer to lower latency and improve throughput without changing the logical model.
- DS quotas and fairness: Per‑DS per‑block quotas and weight caps to avoid head‑of‑line blocking and ensure predictable latency for private DS.
- DS attestation (PQ): Default DS quorum certificates use ML‑DSA‑87 (Dilithium5‑class). This is post‑quantum and larger than EC signatures but acceptable at one QC per slot. DS may explicitly opt for ML‑DSA‑65/44 (smaller) or EC signatures if declared in the DS Manifest; public DS are strongly encouraged to keep ML‑DSA‑87.
- DA attesters: For public DS, use VRF‑sampled regional attesters that issue DA certificates. The nexus committee validates certificates instead of raw shard sampling; private DS keep DA attestations internal.
- Recursion and epoch proofs: Optionally aggregate multiple micro‑batches within a DS into one recursive proof per slot/epoch to keep proof sizes and verify time steady under high load.
- Lane scaling (if needed): If a single global committee becomes a bottleneck, introduce K parallel sequencing lanes with a deterministic merge. This preserves a single global order while scaling horizontally.
- Deterministic acceleration: Provide SIMD/CUDA feature‑gated kernels for hashing/FFT with a bit‑exact CPU fallback to preserve cross‑hardware determinism.
- Lane activation thresholds (proposal): Enable 2–4 lanes if either (a) p95 finality exceeds 1.2 s for >3 consecutive minutes, or (b) per‑block occupancy exceeds 85% for >5 minutes, or (c) incoming tx rate would require >1.2× block capacity at sustained levels. Lanes deterministically bucket transactions by DSID hash and merge in the nexus block.

Fees and Economics (Initial Defaults)
- Gas unit: per‑DS gas token with metered compute/IO; fees are paid in the DS’s native gas asset. Conversion across DS is an application concern.
- Inclusion priority: round‑robin across DS with per‑DS quotas to preserve fairness and 1s SLOs; within a DS, fee bidding can break ties.
- Future: optional global fee market or MEV‑minimizing policies can be explored without changing atomicity or PQ proof design.

Cross‑Data‑Space Workflow (Example)
1) A user submits an AMX transaction touching public DS P and private DS S: move asset X from S to beneficiary B whose account is in P.
2) Within the slot, P and S each execute their fragment against the slot snapshot. S verifies authorization and availability, updates its internal state, and produces a PQ validity proof and DA commitment (no private data leaked). P prepares the corresponding state update (e.g., mint/burn/locking in P according to policy) and its proof.
3) The nexus committee verifies both DS proofs and DA certificates; if both verify within the slot, the transaction is committed atomically in the 1s Nexus Block, updating both DS roots in the global world state vector.
4) If any proof or DA certificate is missing/invalid, the transaction aborts (no effects), and the client may resubmit for the next slot. No private data leaves S at any step.

- Security Considerations
- Deterministic Execution: IVM syscalls remain deterministic; cross‑DS outcomes are driven by AMX commit and finality, not wall‑clock or network timing.
- Access Control: ISI permissions in private DS restrict who may submit transactions and what operations are allowed. Capability tokens encode fine‑grained rights for cross‑DS use.
- Confidentiality: End‑to‑end encryption for private‑DS data, erasure‑coded shards stored only among authorized members, optional ZK proofs for external attestations.
- DoS Resistance: Isolation at mempool/consensus/storage layers prevents public congestion from impacting private‑DS progress.

Changes to Iroha Components
- iroha_data_model: Introduce `DataSpaceId`, DS‑qualified identifiers, AMX descriptors (read/write sets), proof/DA commitment types. Norito‑only serialization.
- ivm: Add syscalls and pointer‑ABI types for AMX (`amx_begin`, `amx_commit`, `amx_touch`), and DA proofs; update ABI tests/docs per v1 policy.
- iroha_core: Implement nexus scheduler, Space Directory, AMX routing/validation, DS artifact verification, and policy enforcement for DA sampling and quotas.
- Space Directory & manifest loaders: Thread FMS endpoint metadata (and other common-good service descriptors) through DS manifest parsing so nodes auto-discover local service endpoints when joining a Data Space.
- kura: Blob store with erasure coding, commitments, retrieval APIs respecting private/public policies.
- WSV: Snapshotting, chunking, commitments; proof APIs; integration with AMX conflict detection and verification.
- irohad: Node roles, networking for DA, private‑DS membership/authentication, configuration via `iroha_config` (no env toggles in production paths).

Configuration and Determinism
- All runtime behavior configured via `iroha_config` and threaded through constructors/hosts. No production env toggles.
- Hardware acceleration (SIMD/NEON/METAL/CUDA) is optional and feature-gated; deterministic fallbacks must produce identical results across hardware.
- - Post‑Quantum default: All DS must use PQ validity proofs (STARK/FRI) and ML‑DSA‑87 for DS QCs by default. Alternatives require explicit DS Manifest declaration and policy approval.

### Runtime Lane Lifecycle Control

- **Admin endpoint:** `POST /v1/nexus/lifecycle` (Torii) accepts a Norito/JSON body with `additions` (full `LaneConfig` objects) and `retire` (lane ids) to add or remove lanes without restart. Requests are gated on `nexus.enabled=true` and reuse the same Nexus configuration/state view as the queue.
- **Behaviour:** On success the node applies the lifecycle plan to WSV/Kura metadata, rebuilds queue routing/limits/manifests, and responds with `{ ok: true, lane_count: <u32> }`. Plans that fail validation (unknown retire ids, duplicate aliases/ids, Nexus disabled) return `400 Bad Request` with a `lane_lifecycle_error`.
- **Safety:** The handler uses the shared state view lock to avoid racing with readers while updating catalogs; callers should still serialize lifecycle updates externally to avoid conflicting plans.
- **TODO:** Per-lane scheduler/DA/RBC rebalance, validator-set propagation, and storage teardown/cleanup are pending; keep lifecycle requests infrequent until those hooks land.

Migration Path (Iroha 2 → Iroha 3)
1) Introduce data‑space‑qualified IDs and nexus block/global state composition in data model; add feature flags to keep Iroha 2 compatible modes during transition.
2) Implement Kura/WSV erasure‑coding backends behind feature flags, preserving current backends as defaults during early phases.
3) Add IVM syscalls and pointer types for AMX (atomic multi‑DS) operations; extend tests and docs; keep ABI v1.
4) Deliver minimal nexus chain with a single public DS and 1s blocks; then add first private‑DS pilot exporting proofs/commitments only.
5) Expand to full atomic cross‑DS transactions (AMX) with DS‑local FASTPQ‑ISI proofs and DA attesters; enable ML‑DSA‑87 QCs across DS.

Testing Strategy
- Unit tests for data model types, Norito roundtrips, AMX syscall behaviors, and proof encoding/decoding.
- IVM tests for new syscalls and ABI goldens.
- Integration tests for atomic cross‑DS transactions (positive/negative), DA attester latency targets (≤300 ms), and performance isolation under load.
- Security tests for DS QC verification (ML‑DSA‑87), conflict detection/abort semantics, and confidential shard leakage prevention.

### NX-18 Telemetry & Runbook Assets

- **Grafana board:** `dashboards/grafana/nexus_lanes.json` now exports the “Nexus Lane Finality & Oracles” dashboard requested by NX‑18. Panels cover `histogram_quantile()` on `iroha_slot_duration_ms`, `iroha_da_quorum_ratio`, DA availability warnings (`sumeragi_da_gate_block_total{reason="missing_local_data"}`), oracle price/staleness/TWAP/haircut gauges, and the live `iroha_settlement_buffer_xor` buffer panel so operators can prove the 1 s slot, DA, and treasury SLOs without bespoke queries.
- **CI gate:** `scripts/telemetry/check_slot_duration.py` parses Prometheus snapshots, prints the p50/p95/p99 slot latency, and enforces the NX‑18 thresholds (p95 ≤ 1000 ms, p99 ≤ 1100 ms). The companion harness `scripts/telemetry/nx18_acceptance.py` gates DA quorum, oracle staleness/TWAP/haircuts, settlement buffers, and slot quantiles in one pass (`--json-out` persists evidence), and both run inside `ci/check_nexus_lane_smoke.sh` for RCs.
- **Evidence bundler:** `scripts/telemetry/bundle_slot_artifacts.py` copies the metrics snapshot + JSON summary into `artifacts/nx18/` and emits `slot_bundle_manifest.json` with SHA-256 digests, ensuring every RC uploads the exact artefacts that triggered the NX‑18 gate.
- **Release automation:** `scripts/run_release_pipeline.py` now invokes `ci/check_nexus_lane_smoke.sh` (skip with `--skip-nexus-lane-smoke`) and copies `artifacts/nx18/` into the release output so NX‑18 evidence rides alongside the bundle/image artefacts without a manual step.
- **Runbook:** `docs/source/runbooks/nexus_lane_finality.md` documents the on-call workflow (thresholds, incident steps, evidence capture, chaos drills) that accompanies the dashboard, fulfilling the “publish operator dashboards/runbooks” bullet from NX‑18.
- **Telemetry helpers:** reuse the existing `scripts/telemetry/compare_dashboards.py` to diff exported dashboards (preventing staging/prod drift) and `scripts/telemetry/check_nexus_audit_outcome.py` during routed-trace or chaos rehearsals so every NX‑18 drill archives the matching `nexus.audit.outcome` payload.

Open Questions (Clarification Needed)
1) Transaction signatures: Decision — end users are free to pick any signing algorithm that their target DS advertises (Ed25519, secp256k1, ML‑DSA, etc.). Hosts must enforce multisig/curve capability flags in manifests, provide deterministic fallbacks, and document latency implications when mixing algorithms. Outstanding: finalise capability negotiation flow across Torii/SDKs and update admission tests.
2) Gas economics: Each DS may denominate gas in a local token, while global settlement fees are paid in SORA XOR. Outstanding: define the standard conversion path (public-lane DEX vs. other liquidity sources), ledger accounting hooks, and safeguards for DS that subsidise or zero-price transactions.
3) DA attesters: Target number per region and threshold (e.g., 64 sampled, 43‑of‑64 ML‑DSA‑87 signatures) to meet ≤300 ms while maintaining durability. Any regions we must include from day one?
4) Default DA parameters: We proposed public DS `k=32, m=16` and private DS `k=16, m=8`. Do you want a higher redundancy profile (e.g., `k=30, m=20`) for certain DS classes?
5) DS granularity: Domains and assets can both be DS. Should we support hierarchical DS (domain DS as parent of asset DS) with optional inheritance of policies, or keep them flat for v1?
6) Heavy ISIs: For complex ISIs that cannot produce sub‑second proofs, should we (a) reject them, (b) split into smaller atomic steps across blocks, or (c) allow delayed inclusion with explicit flags?
7) Cross‑DS conflicts: Is client‑declared read/write set sufficient, or should the host infer and expand it automatically for safety (at cost of more conflicts)?

Appendix: Compliance with Repository Policies
- Norito is used for all wire formats and JSON serialization via Norito helpers.
- ABI v1 only; no runtime toggles for ABI policies. Syscall and pointer-type additions follow the documented evolution process with golden tests.
- Determinism preserved across hardware; acceleration is optional and gated.
- No serde in production paths; no environment-based configuration in production.
