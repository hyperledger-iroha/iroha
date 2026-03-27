<!--
SPDX-License-Identifier: Apache-2.0
-->
# Confidential Assets & ZK Transfer Design

## Motivation
- Deliver opt-in shielded asset flows so domains can preserve transactional privacy without altering transparent circulation.
- Provide auditors and operators with lifecycle controls (activation, rotation, revocation) for circuits and cryptographic parameters.

## Threat Model
- Validators are honest-but-curious: they execute consensus faithfully but attempt to inspect ledger/state.
- Network observers see block data and gossiped transactions; no assumption of private gossip channels.
- Out of scope: off-ledger traffic analysis, quantum adversaries (tracked separately under PQ roadmap), ledger availability attacks.

## Design Overview
- Assets may declare a *shielded pool* in addition to existing transparent balances; shielded circulation is represented via cryptographic commitments.
- Notes encapsulate `(asset_id, amount, recipient_view_key, blinding, rho)` with:
  - Commitment: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, independent of note ordering.
  - Encrypted payload: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Transactions transport Norito-encoded `ConfidentialTransfer` payloads containing:
  - Public inputs: Merkle anchor, nullifiers, new commitments, asset id, circuit version.
  - Encrypted payloads for recipients and optional auditors.
  - Zero-knowledge proof attesting value conservation, ownership, and authorization.
- Verifying keys and parameter sets are controlled through on-ledger registries with activation windows; nodes refuse to validate proofs that reference unknown or revoked entries.
- Consensus headers commit to the active confidential feature digest so blocks are only accepted when registry and parameter state matches.
- Proof construction uses a Halo2 (Plonkish) stack without trusted setup; Groth16 or other SNARK variants are intentionally unsupported in v1.

### Deterministic Fixtures

Confidential memo envelopes now ship with a canonical fixture at `fixtures/confidential/encrypted_payload_v1.json`. The dataset captures a positive v1 envelope plus negative malformed samples so SDKs can assert parsing parity. The Rust data-model tests (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) and Swift suite (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) both load the fixture directly, guaranteeing that Norito encoding, error surfaces, and regression coverage stay aligned as the codec evolves.

Swift SDKs can now emit shield instructions without bespoke JSON glue: construct a
`ShieldRequest` with the 32-byte note commitment, encrypted payload, and debit metadata,
then call `IrohaSDK.submit(shield:keypair:)` (or `submitAndWait`) to sign and relay the
transaction over `/v1/pipeline/transactions`. The helper validates commitment lengths,
threads `ConfidentialEncryptedPayload` into the Norito encoder, and mirrors the `zk::Shield`
layout described below so wallets stay in lock-step with Rust.

## Consensus Commitments & Capability Gating
- Block headers expose `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; the digest participates in the consensus hash and must equal the local registry view for block acceptance.
- Governance can stage upgrades by programming `next_conf_features` with a future `activation_height`; until that height, block producers must continue to emit the previous digest.
- Validator nodes MUST operate with `confidential.enabled = true` and `assume_valid = false`. Startup checks refuse to join the validator set if either condition fails or if local `conf_features` diverge.
- P2P handshake metadata now includes `{ enabled, assume_valid, conf_features }`. Peers advertising unsupported features are rejected with `HandshakeConfidentialMismatch` and never enter consensus rotation.
- Non-validator observers may set `assume_valid = true`; they blindly apply confidential deltas but do not influence consensus safety.

## Asset Policies
- Each asset definition carries an `AssetConfidentialPolicy` set by the creator or via governance:
  - `TransparentOnly`: default mode; only transparent instructions (`MintAsset`, `TransferAsset`, etc.) are permitted and shielded operations are rejected.
  - `ShieldedOnly`: all issuance and transfers must use confidential instructions; `RevealConfidential` is forbidden so balances never surface publicly.
  - `Convertible`: holders may move value between transparent and shielded representations using the on/off-ramp instructions below.
- Policies follow a constrained FSM to prevent stranding funds:
  - `TransparentOnly → Convertible` (immediate enablement of shielded pool).
  - `TransparentOnly → ShieldedOnly` (requires pending transition and conversion window).
  - `Convertible → ShieldedOnly` (enforced minimum delay).
  - `ShieldedOnly → Convertible` (migration plan required so shielded notes remain spendable).
  - `ShieldedOnly → TransparentOnly` is disallowed unless the shielded pool is empty or governance encodes a migration that unshields outstanding notes.
- Governance instructions set `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` via the `ScheduleConfidentialPolicyTransition` ISI and may abort scheduled changes with `CancelConfidentialPolicyTransition`. Mempool validation ensures no transaction straddles the transition height and inclusion fails deterministically if a policy check would change mid-block.
- Pending transitions are applied automatically when a new block opens: once the block height enters the conversion window (for `ShieldedOnly` upgrades) or reaches the programmed `effective_height`, the runtime updates `AssetConfidentialPolicy`, refreshes `zk.policy` metadata, and clears the pending entry. If transparent supply remains when a `ShieldedOnly` transition matures, the runtime aborts the change and logs a warning, leaving the previous mode intact.
- Config knobs `policy_transition_delay_blocks` and `policy_transition_window_blocks` enforce minimum notice and grace periods to let wallets convert notes around the switch.
- `pending_transition.transition_id` doubles as an audit handle; governance must quote it when finalising or cancelling transitions so operators can correlate on/off-ramp reports.
- `policy_transition_window_blocks` defaults to 720 (≈12 hours at 60 s block time). Nodes clamp governance requests that attempt shorter notice.
- Genesis manifests and CLI flows surface current and pending policies. Admission logic reads the policy at execution time to confirm each confidential instruction is authorised.
- Migration checklist — see “Migration sequencing” below for the staged upgrade plan that Milestone M0 tracks.

#### Monitoring transitions via Torii

Wallets and auditors poll `GET /v1/confidential/assets/{definition_id}/transitions` to inspect
the active `AssetConfidentialPolicy`. The JSON payload always includes the canonical
asset id, the latest observed block height, the policy’s `current_mode`, the mode that is
effective at that height (conversion windows temporarily report `Convertible`), and the
expected `vk_set_hash`/Poseidon/Pedersen parameter identifiers. Swift SDK consumers can call
`ToriiClient.getConfidentialAssetPolicy` to receive the same data as typed DTOs without
hand-written decoding. When a governance transition is pending the response also embeds:

- `transition_id` — audit handle returned by `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` and the derived `window_open_height` (the block where wallets must
  begin conversion for ShieldedOnly cut-overs).

Example response:

```json
{
  "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

A `404` response indicates no matching asset definition exists. When no transition is
scheduled the `pending_transition` field is `null`.

### Policy state machine

| Current mode       | Next mode        | Prerequisites                                                                 | Effective-height handling                                                                                         | Notes                                                                                     |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| TransparentOnly    | Convertible      | Governance has activated verifier/parameter registry entries. Submit `ScheduleConfidentialPolicyTransition` with `effective_height ≥ current_height + policy_transition_delay_blocks`. | Transition executes exactly at `effective_height`; shielded pool becomes available immediately.                   | Default path for enabling confidentiality while keeping transparent flows.               |
| TransparentOnly    | ShieldedOnly     | Same as above, plus `policy_transition_window_blocks ≥ 1`.                                                         | Runtime auto-enters `Convertible` at `effective_height - policy_transition_window_blocks`; flips to `ShieldedOnly` at `effective_height`. | Provides deterministic conversion window before transparent instructions are disabled.   |
| Convertible        | ShieldedOnly     | Scheduled transition with `effective_height ≥ current_height + policy_transition_delay_blocks`. Governance SHOULD certify (`transparent_supply == 0`) via audit metadata; runtime enforces this at cut-over. | Identical window semantics as above. If the transparent supply is non-zero at `effective_height`, the transition aborts with `PolicyTransitionPrerequisiteFailed`. | Locks the asset into fully confidential circulation.                                     |
| ShieldedOnly       | Convertible      | Scheduled transition; no active emergency withdrawal (`withdraw_height` unset).                                    | State flips at `effective_height`; reveal ramps reopen while shielded notes remain valid.                           | Used for maintenance windows or auditor reviews.                                          |
| ShieldedOnly       | TransparentOnly  | Governance must prove `shielded_supply == 0` or stage a signed `EmergencyUnshield` plan (auditor signatures required). | Runtime opens a `Convertible` window ahead of `effective_height`; at the height, confidential instructions hard-fail and the asset returns to transparent-only mode. | Last-resort exit. Transition auto-cancels if any confidential note spends during the window. |
| Any                | Same as current  | `CancelConfidentialPolicyTransition` clears pending change.                                                        | `pending_transition` removed immediately.                                                                          | Maintains status quo; shown for completeness.                                             |

Transitions not listed above are rejected during governance submission. Runtime checks the prerequisites right before applying a scheduled transition; failing preconditions pushes the asset back to its previous mode and emits `PolicyTransitionPrerequisiteFailed` via telemetry and block events.

### Migration sequencing

2. **Stage the transition:** Submit `ScheduleConfidentialPolicyTransition` with an `effective_height` that respects `policy_transition_delay_blocks`. When moving toward `ShieldedOnly`, specify a conversion window (`window ≥ policy_transition_window_blocks`).
3. **Publish operator guidance:** Record the returned `transition_id` and circulate an on/off-ramp runbook. Wallets and auditors subscribe to `/v1/confidential/assets/{id}/transitions` to learn the window open height.
4. **Window enforcement:** When the window opens, the runtime switches the policy to `Convertible`, emits `PolicyTransitionWindowOpened { transition_id }`, and begins rejecting conflicting governance requests.
5. **Finalize or abort:** At `effective_height`, the runtime verifies the transition prerequisites (zero transparent supply, no emergency withdrawals, etc.). Success flips the policy to the requested mode; failure emits `PolicyTransitionPrerequisiteFailed`, clears the pending transition, and leaves the policy unchanged.
6. **Schema upgrades:** After a successful transition, governance bumps the asset schema version (e.g., `asset_definition.v2`) and CLI tooling requires `confidential_policy` when serialising manifests. Genesis upgrade docs instruct operators to add policy settings and registry fingerprints before restarting validators.

New networks that start with confidentiality enabled encode the desired policy directly in genesis. They still follow the checklist above when changing modes post-launch so that conversion windows remain deterministic and wallets have time to adjust.

### Norito manifest versioning & activation

- Genesis manifests MUST include a `SetParameter` for the custom `confidential_registry_root` key. The payload is Norito JSON matching `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: omit the field (`null`) when no verifier entries are active, otherwise supply a 32-byte hex string (`0x…`) equal to the hash produced by `compute_vk_set_hash` over the verifier instructions shipped in the manifest. Nodes refuse to start if the parameter is missing or the hash disagrees with the encoded registry writes.
- The on-wire `ConfidentialFeatureDigest::conf_rules_version` embeds the manifest layout version. For v1 networks it MUST remain `Some(1)` and equals `iroha_config::parameters::defaults::confidential::RULES_VERSION`. When the ruleset evolves, bump the constant, regenerate manifests, and roll out binaries in lock-step; mixing versions causes validators to reject blocks with `ConfidentialFeatureDigestMismatch`.
- Activation manifests SHOULD bundle registry updates, parameter lifecycle changes, and policy transitions so the digest stays consistent:
  1. Apply the planned registry mutations (`Publish*`, `Set*Lifecycle`) in an offline state view and compute the post-activation digest with `compute_confidential_feature_digest`.
  2. Emit `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` using the computed hash so lagging peers can recover the correct digest even if they miss intermediate registry instructions.
  3. Append the `ScheduleConfidentialPolicyTransition` instructions. Each instruction must quote the governance-issued `transition_id`; manifests that forget it will be rejected by the runtime.
  4. Persist the manifest bytes, a SHA-256 fingerprint, and the digest used in the activation plan. Operators verify all three artefacts before voting the manifest into effect to avoid partitions.
- When rollouts require a deferred cut-over, record the target height in a companion custom parameter (for example `custom.confidential_upgrade_activation_height`). This gives auditors a Norito-encoded proof that validators honoured the notice window before the digest change took effect.

## Verifier & Parameter Lifecycle
### ZK Registry
- Ledger stores `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` where `proving_system` is currently fixed to `Halo2`.
- `(circuit_id, version)` pairs are globally unique; the registry maintains a secondary index for lookups by circuit metadata. Attempts to register a duplicate pair are rejected during admission.
- `circuit_id` must be non-empty and `public_inputs_schema_hash` must be provided (typically a Blake2b-32 hash of the verifier’s canonical public-input encoding). Admission rejects records that omit these fields.
- Governance instructions include:
  - `PUBLISH` to add a `Proposed` entry with metadata only.
  - `ACTIVATE { vk_id, activation_height }` to schedule entry activation at an epoch boundary.
  - `DEPRECATE { vk_id, deprecation_height }` to mark the final height where proofs may reference the entry.
  - `WITHDRAW { vk_id, withdraw_height }` for emergency shutdown; affected assets freeze confidential spending after the withdraw height until new entries activate.
- Genesis manifests auto-emit a `confidential_registry_root` custom parameter whose `vk_set_hash` matches the active entries; validation cross-checks this digest against local registry state before a node can join consensus.
- Registering or updating a verifier requires a `gas_schedule_id`; verification enforces that the registry entry is `Active`, present in the `(circuit_id, version)` index, and that Halo2 proofs provide an `OpenVerifyEnvelope` whose `circuit_id`, `vk_hash`, and `public_inputs_schema_hash` match the registry record.

### Proving Keys
- Proving keys remain off-ledger but are referenced by content-addressed identifiers (`pk_cid`, `pk_hash`, `pk_len`) published alongside verifier metadata.
- Wallet SDKs fetch PK data, verify hashes, and cache locally.

### Pedersen & Poseidon Parameters
- Separate registries (`PedersenParams`, `PoseidonParams`) mirror verifier lifecycle controls, each with `params_id`, hashes of generators/constants, activation, deprecation, and withdraw heights.

## Deterministic Ordering & Nullifiers
- Each asset maintains a `CommitmentTree` with `next_leaf_index`; blocks append commitments in deterministic order: iterate transactions in block order; within each transaction iterate shielded outputs by ascending serialized `output_idx`.
- `note_position` is derived from the tree offsets but **not** part of the nullifier; it only feeds membership paths within the proof witness.
- Nullifier stability under reorgs is guaranteed by the PRF design; the PRF input binds `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, and anchors reference historical Merkle roots limited by `max_anchor_age_blocks`.

## Ledger Flow
1. **MintConfidential { asset_id, amount, recipient_hint }**
   - Requires asset policy `Convertible` or `ShieldedOnly`; admission checks asset authority, retrieves current `params_id`, samples `rho`, emits commitment, updates Merkle tree.
   - Emits `ConfidentialEvent::Shielded` with the new commitment, Merkle root delta, and transaction call hash for audit trails.
2. **TransferConfidential { asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, anchor_root, memo }**
   - VM syscall verifies proof using registry entry; host ensures nullifiers unused, commitments appended deterministically, anchor is recent.
   - Ledger records `NullifierSet` entries, stores encrypted payloads for recipients/auditors, and emits `ConfidentialEvent::Transferred` summarising nullifiers, ordered outputs, proof hash, and Merkle roots.
3. **RevealConfidential { asset_id, proof, circuit_id, version, nullifier, amount, recipient_account, anchor_root }**
   - Available only for `Convertible` assets; proof validates note value equals revealed amount, ledger credits transparent balance, and burns the shielded note by marking the nullifier spent.
   - Emits `ConfidentialEvent::Unshielded` with the public amount, consumed nullifiers, proof identifiers, and transaction call hash.

## Data Model Additions
- `ConfidentialConfig` (new config section) with enablement flag, `assume_valid`, gas/limit knobs, anchor window, verifier backend.
- `ConfidentialNote`, `ConfidentialTransfer`, and `ConfidentialMint` Norito schemas with explicit version byte (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` wraps AEAD memo bytes with `{ version, ephemeral_pubkey, nonce, ciphertext }`, defaulting to `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` for the XChaCha20-Poly1305 layout.
- Canonical key-derivation vectors live in `docs/source/confidential_key_vectors.json`; both the CLI and Torii endpoint regress against these fixtures. Wallet-facing derivatives for the spend/nullifier/viewing ladder are published in `fixtures/confidential/keyset_derivation_v1.json` and exercised by the Rust + Swift SDK tests to guarantee cross-language parity.
- `asset::AssetDefinition` gains `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persists the `(backend, name, commitment)` binding for transfer/unshield verifiers; execution rejects proofs whose referenced or inline verifying key fails to match the registered commitment and verifies transfer/unshield proofs against the resolved backend key before mutating state.
- `CommitmentTree` (per asset with frontier checkpoints), `NullifierSet` keyed by `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` stored in world state.
- Mempool maintains transient `NullifierIndex` and `AnchorIndex` structures for early duplicate detection and anchor age checks.
- Norito schema updates include canonical ordering for public inputs; round-trip tests ensure encoding determinism.
- Encrypted payload roundtrips are locked in via unit tests (`crates/iroha_data_model/src/confidential.rs`), and the wallet key-derivation vectors above anchor the AEAD envelope derivations for auditors. `norito.md` documents the on-wire header for the envelope.

## IVM Integration & Syscall
- Introduce `VERIFY_CONFIDENTIAL_PROOF` syscall accepting:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, and resulting `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - Syscall loads verifier metadata from registry, enforces size/time limits, charges deterministic gas, and only applies delta if proof succeeds.
- Host exposes read-only `ConfidentialLedger` trait for retrieving Merkle root snapshots and nullifier status; Kotodama library provides witness assembly helpers and schema validation.
- Pointer-ABI docs updated to clarify proof buffer layout and registry handles.

## Node Capability Negotiation
- Handshake advertises `feature_bits.confidential` together with a `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. Validator participation requires `confidential.enabled=true`, `assume_valid=false`, identical verifier backend identifiers, and matching digests; mismatches fail the handshake with `HandshakeConfidentialMismatch`.
- Config supports `assume_valid` for observer nodes only: when disabled, encountering confidential instructions yields deterministic `UnsupportedInstruction` without panic; when enabled, observers apply declared state deltas without verifying proofs.
- Mempool rejects confidential transactions if local capability is disabled. Gossip filters avoid sending shielded transactions to peers without matching capability while blind-forwarding unknown verifier IDs within size limits.

### Reveal Pruning & Nullifier Retention Policy

Confidential ledgers must retain enough history to prove note freshness and to
replay governance-driven audits. The default policy, enforced by
`ConfidentialLedger`, is:

- **Nullifier retention:** keep spent nullifiers for *minimum* `730` days (24
  months) after spend height, or the regulator-mandated window if longer.
  Operators may extend the window via `confidential.retention.nullifier_days`.
  Nullifiers younger than the retention window MUST remain queryable via Torii so
  auditors can prove double-spend absence.
- **Reveal pruning:** transparent reveals (`RevealConfidential`) prune the
  associated note commitments immediately after the block finalises, but the
  consumed nullifier remains subject to the retention rule above. Reveal-related
  events (`ConfidentialEvent::Unshielded`) record the public amount, recipient,
  and proof hash so reconstructing historic reveals does not require the pruned
  ciphertext.
- **Frontier checkpoints:** commitment frontiers maintain rolling checkpoints
  covering the larger of `max_anchor_age_blocks` and the retention window. Nodes
  compact older checkpoints only after all nullifiers within the interval expire.
- **Stale digest remediation:** if `HandshakeConfidentialMismatch` is raised due
  to digest drift, operators should (1) verify that nullifier retention windows
  align across the cluster, (2) run `iroha_cli app confidential verify-ledger` to
  regenerate the digest against the retained nullifier set, and (3) redeploy the
  refreshed manifest. Any nullifiers pruned prematurely must be restored from
  cold storage before rejoining the network.

Document local overrides in the operations runbook; governance policies extending
the retention window must update node configuration and archival storage plans in
lockstep.

### Eviction & Recovery Flow

1. During dial, `IrohaNetwork` compares the advertised capabilities. Any mismatch raises `HandshakeConfidentialMismatch`; the connection is closed and the peer remains in the discovery queue without ever being promoted to `Ready`.
2. The failure is surfaced via the network service log (including the remote digest and backend), and Sumeragi never schedules the peer for proposal or voting.
3. Operators remediate by aligning verifier registries and parameter sets (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) or by staging `next_conf_features` with an agreed `activation_height`. Once the digest matches, the next handshake succeeds automatically.
4. If a stale peer manages to broadcast a block (e.g., via archival replay), validators reject it deterministically with `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, keeping ledger state consistent across the network.

### Replay-safe handshake flow

1. Each outbound attempt allocates fresh Noise/X25519 key material. The handshake payload that is signed (`handshake_signature_payload`) concatenates the local and remote ephemeral public keys, the Norito-encoded advertised socket address, and—when compiled with `handshake_chain_id`—the chain identifier. The message is AEAD-encrypted before it leaves the node.
2. The responder recomputes the payload with the peer/local key order reversed and verifies the Ed25519 signature embedded in `HandshakeHelloV1`. Because both ephemeral keys and the advertised address are part of the signature domain, replaying a captured message against another peer or recovering a stale connection fails verification deterministically.
3. Confidential capability flags and the `ConfidentialFeatureDigest` travel inside `HandshakeConfidentialMeta`. The receiver compares the tuple `{ enabled, assume_valid, verifier_backend, digest }` against its locally configured `ConfidentialHandshakeCaps`; any mismatch exits early with `HandshakeConfidentialMismatch` before the transport transitions to `Ready`.
4. Operators MUST recompute the digest (via `compute_confidential_feature_digest`) and restart nodes with the updated registries/policies before reconnecting. Peers advertising old digests continue to fail the handshake, preventing stale state from re-entering the validator set.
5. Handshake successes and failures update the standard `iroha_p2p::peer` counters (`handshake_failure_count`, error taxonomy helpers) and emit structured log entries tagged with the remote peer ID and digest fingerprint. Monitor these indicators to catch replay attempts or misconfigurations during rollout.

## Key Management & Payloads
- Per-account key derivation hierarchy:
  - `sk_spend` → `nk` (nullifier key), `ivk` (incoming viewing key), `ovk` (outgoing viewing key), `fvk`.
- Encrypted note payloads use AEAD with ECDH-derived shared keys; optional auditor view keys may be attached to outputs per asset policy.
- CLI additions: `confidential create-keys`, `confidential send`, `confidential export-view-key`, auditor tooling for decrypting memos, and the `iroha app zk envelope` helper for producing/inspecting Norito memo envelopes offline.

## Gas, Limits & DoS Controls
- Deterministic gas schedule:
  - Halo2 (Plonkish): base `250_000` gas + `2_000` gas per public input.
  - `5` gas per proof byte, plus per-nullifier (`300`) and per-commitment (`500`) charges.
  - Operators may override these constants via the node configuration (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); changes propagate at startup or when the config layer hot-reloads and are applied deterministically across the cluster.
- Hard limits (configurable defaults):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Proofs that exceed `verify_timeout_ms` abort the instruction deterministically (governance ballots emit `proof verification exceeded timeout`, `VerifyProof` returns an error).
- Additional quotas ensure liveness: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, and `max_public_inputs` bound block builders; `reorg_depth_bound` (≥ `max_anchor_age_blocks`) governs frontier checkpoint retention.
- Runtime execution now rejects transactions that exceed these per-transaction or per-block limits, emitting deterministic `InvalidParameter` errors and leaving ledger state unchanged.
- Mempool prefilters confidential transactions by `vk_id`, proof length, and anchor age before invoking the verifier to keep resource usage bounded.
- Verification halts deterministically on timeout or bound violation; transactions fail with explicit errors. SIMD backends are optional but do not alter gas accounting.

### Calibration Baselines & Acceptance Gates
- **Reference platforms.** Calibration runs MUST cover the three hardware profiles below. Runs failing to capture all profiles are rejected during review.

  | Profile | Architecture | CPU / Instance | Compiler flags | Purpose |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) or Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Establish floor values without vector intrinsics; used to tune fallback cost tables. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | default release | Validates AVX2 path; checks that SIMD speedups stay within tolerance of neutral gas. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | default release | Ensures NEON backend remains deterministic and aligned with x86 schedules. |

- **Benchmark harness.** All gas calibration reports MUST be produced with:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` to confirm the deterministic fixture.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` whenever VM opcode costs change.

- **Fixed randomness.** Export `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` before running benches so `iroha_test_samples::gen_account_in` switches to the deterministic `KeyPair::from_seed` path. The harness prints `IROHA_CONF_GAS_SEED_ACTIVE=…` once; if the variable is missing, review MUST fail. Any new calibration utilities must continue honouring this env var when introducing auxiliary randomness.

- **Result capture.**
  - Upload Criterion summaries (`target/criterion/**/raw.csv`) for each profile into the release artefact.
  - Store derived metrics (`ns/op`, `gas/op`, `ns/gas`) in `docs/source/confidential_assets_calibration.md` along with the git commit and compiler version used.
  - Maintain the last two baselines per profile; delete older snapshots once the newest report is validated.

- **Acceptance tolerances.**
  - Gas deltas between `baseline-simd-neutral` and `baseline-avx2` MUST remain ≤ ±1.5%.
  - Gas deltas between `baseline-simd-neutral` and `baseline-neon` MUST remain ≤ ±2.0%.
  - Calibration proposals exceeding these thresholds require either schedule adjustments or an RFC explaining the discrepancy and mitigation.

- **Review checklist.** Submitters are responsible for:
  - Including `uname -a`, `/proc/cpuinfo` excerpts (model, stepping), and `rustc -Vv` in the calibration log.
  - Verifying `IROHA_CONF_GAS_SEED` echoed in the bench output (the benches print the active seed).
  - Ensuring pacemaker and confidential verifier feature flags mirror production (`--features confidential,telemetry` when running benches with Telemetry).

## Config & Operations
- `iroha_config` gains `[confidential]` section:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- Telemetry emits aggregate metrics: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, and `confidential_policy_transitions_total`, never exposing plaintext data.
- RPC surfaces:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## Testing Strategy
- Determinism: randomized transaction shuffling within blocks yields identical Merkle roots and nullifier sets.
- Reorg resilience: simulate multi-block reorgs with anchors; nullifiers remain stable and stale anchors rejected.
- Gas invariants: verify identical gas usage across nodes with and without SIMD acceleration.
- Boundary testing: proofs at size/gas ceilings, max in/out counts, timeout enforcement.
- Lifecycle: governance operations for verifier and parameter activation/deprecation, rotation spend tests.
- Policy FSM: allowed/disallowed transitions, pending transition delays, and mempool rejection around effective heights.
- Registry emergencies: emergency withdrawal freezes affected assets at `withdraw_height` and rejects proofs afterwards.
- Capability gating: validators with mismatched `conf_features` reject blocks; observers with `assume_valid=true` keep up without affecting consensus.
- State equivalence: validator/full/observer nodes produce identical state roots on the canonical chain.
- Negative fuzzing: malformed proofs, oversized payloads, and nullifier collisions reject deterministically.

## Outstanding Work
- Benchmark Halo2 parameter sets (circuit size, lookup strategy) and record the results in the calibration playbook so gas/timeout defaults can be updated alongside the next `confidential_assets_calibration.md` refresh.
- Finalize auditor disclosure policies and associated selective-viewing APIs, wiring the approved workflow into Torii once the governance draft is signed off.
- Extend the witness encryption scheme to cover multi-recipient outputs and batched memos, documenting the envelope format for SDK implementers.
- Commission an external security review of circuits, registries, and parameter-rotation procedures and archive the findings next to the internal audit reports.
- Specify auditor spentness reconciliation APIs and publish view-key scope guidance so wallet vendors can implement the same attestation semantics.

## Implementation Phasing
1. **Phase M0 — Stop-Ship Hardening**
   - ✅ Nullifier derivation now follows the Poseidon PRF design (`nk`, `rho`, `asset_id`, `chain_id`) with deterministic commitment ordering enforced in ledger updates.
   - ✅ Execution enforces proof size caps and per-transaction/per-block confidential quotas, rejecting over-budget transactions with deterministic errors.
   - ✅ P2P handshake advertises `ConfidentialFeatureDigest` (backend digest + registry fingerprints) and fails mismatches deterministically via `HandshakeConfidentialMismatch`.
   - ✅ Remove panics in confidential execution paths and add role gating for nodes without matching capability.
   - ⚪ Enforce verifier timeout budgets and reorg depth bounds for frontier checkpoints.
     - ✅ Verification timeout budgets enforced; proofs exceeding `verify_timeout_ms` now fail deterministically.
     - ✅ Frontier checkpoints now respect `reorg_depth_bound`, pruning checkpoints older than the configured window while keeping deterministic snapshots.
   - Introduce `AssetConfidentialPolicy`, policy FSM, and enforcement gates for mint/transfer/reveal instructions.
   - Commit `conf_features` in block headers and refuse validator participation when registry/parameter digests diverge.
2. **Phase M1 — Registries & Parameters**
   - Land `ZkVerifierEntry`, `PedersenParams`, and `PoseidonParams` registries with governance ops, genesis anchoring, and cache management.
   - Wire syscall to require registry lookups, gas schedule IDs, schema hashing, and size checks.
   - Ship encrypted payload format v1, wallet key derivation vectors, and CLI support for confidential key management.
3. **Phase M2 — Gas & Performance**
   - Implement deterministic gas schedule, per-block counters, and benchmark harnesses with telemetry (verify latency, proof sizes, mempool rejections).
   - Harden CommitmentTree checkpoints, LRU loading, and nullifier indices for multi-asset workloads.
4. **Phase M3 — Rotation & Wallet Tooling**
   - Enable multi-parameter and multi-version proof acceptance; support governance-driven activation/deprecation with transition runbooks.
   - Deliver wallet SDK/CLI migration flows, auditor scanning workflows, and spentness reconciliation tooling.
5. **Phase M4 — Audit & Ops**
   - Provide auditor key workflows, selective disclosure APIs, and operational runbooks.
   - Schedule external cryptography/security review and publish findings in `status.md`.

Each phase updates roadmap milestones and associated tests to maintain deterministic execution guarantees for the blockchain network.

### SDK & Fixture Coverage (Phase M1)

Encrypted payload v1 now ships with canonical fixtures so every SDK produces the
same Norito envelopes and transaction hashes. The golden artefacts live in
`fixtures/confidential/wallet_flows_v1.json` and are exercised directly by the
Rust and Swift suites (`crates/iroha_data_model/tests/confidential_wallet_fixtures.rs`,
`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialWalletFixturesTests.swift`):

```bash
# Rust parity (verifies the signed hex + hash for every case)
cargo test -p iroha_data_model confidential_wallet_fixtures

# Swift parity (builds the same envelopes via TxBuilder/NativeBridge)
cd IrohaSwift && swift test --filter ConfidentialWalletFixturesTests
```

Every fixture records the case identifier, signed transaction hex, and expected
hash. When the Swift encoder cannot yet produce the case—`zk-transfer-basic` is
still gated by the `ZkTransfer` builder—the test suite emits `XCTSkip` so the
roadmap clearly tracks which flows still require bindings. Updating the fixture
file without bumping the format version will fail both suites, keeping the SDKs
and Rust reference implementation in lock-step.

#### Swift builders
`TxBuilder` exposes asynchronous and callback-based helpers for every
confidential request (`IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1183`).
The builders rely on the `connect_norito_bridge` exports
(`crates/connect_norito_bridge/src/lib.rs:3337`,
`IrohaSwift/Sources/IrohaSwift/NativeBridge.swift:1014`) so the generated
payloads match the Rust host encoders byte-for-byte. Example:

```swift
let account = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
let request = RegisterZkAssetRequest(
    chainId: chainId,
    authority: account,
    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    zkParameters: myZkParams,
    ttlMs: 60_000
)
let envelope = try TxBuilder(client: client)
    .buildRegisterZkAsset(request: request, keypair: keypair)
try await TxBuilder(client: client)
    .submit(registerZkAsset: request, keypair: keypair)
```

Shielding/unshielding follow the same pattern (`submit(shield:)`,
`submit(unshield:)`), and the Swift fixture tests re-run the builders with
deterministic key material to guarantee the generated transaction hashes remain
equal to the ones stored in `wallet_flows_v1.json`.

#### JavaScript builders
The JavaScript SDK mirrors the same flows via the transaction helpers exported
from `javascript/iroha_js/src/transaction.js`. Builders such as
`buildRegisterZkAssetTransaction` and `buildRegisterZkAssetInstruction`
(`javascript/iroha_js/src/instructionBuilders.js:1832`) normalise verifying key
identifiers and emit Norito payloads that the Rust host can accept without any
adapters. Example:

```js
import {
  buildRegisterZkAssetTransaction,
  signTransaction,
  ToriiClient,
} from "@hyperledger/iroha";

const unsigned = buildRegisterZkAssetTransaction({
  registration: {
    authority: "<i105-account-id>",
    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    zkParameters: {
      commit_params: "vk_shield",
      reveal_params: "vk_unshield",
    },
    metadata: { displayName: "Rose (Shielded)" },
  },
  chainId: "00000000-0000-0000-0000-000000000000",
});
const signed = signTransaction(unsigned, myKeypair);
await new ToriiClient({ baseUrl: "https://torii" }).submitTransaction(signed);
```

Shield, transfer, and unshield builders follow the same pattern, giving JS
callers the same ergonomics as Swift and Rust. Tests under
`javascript/iroha_js/test/transactionBuilder.test.js` cover the normalisation
logic while the fixtures above keep the signed transaction bytes consistent.

### Telemetry & Monitoring (Phase M2)

Phase M2 now exports CommitmentTree health directly via Prometheus and Grafana:

- `iroha_confidential_tree_commitments`, `iroha_confidential_tree_depth`, `iroha_confidential_root_history_entries`, and `iroha_confidential_frontier_checkpoints` expose the live Merkle frontier per asset while `iroha_confidential_root_evictions_total` / `iroha_confidential_frontier_evictions_total` count the LRU trims enforced by `zk.root_history_cap` and the checkpoint depth window.
- `iroha_confidential_frontier_last_checkpoint_height` and `iroha_confidential_frontier_last_checkpoint_commitments` publish the height + commitment count of the most recent frontier checkpoint so reorg drills and rollbacks can prove that checkpoints advance and retain the expected payload volume.
- The Grafana board (`dashboards/grafana/confidential_assets.json`) includes a depth series, eviction-rate panels, and the existing verifier cache widgets so operators can prove that CommitmentTree depth never collapses even as checkpoints churn.
- Alert `ConfidentialTreeDepthZero` (in `dashboards/alerts/confidential_assets_rules.yml`) trips once commitments are observed but the reported depth sticks at zero for five minutes.

You can verify the metrics locally before wiring Grafana:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="4cuvDVPuLBKJyN6dPbRQhmLh68sU"}'
```

Pair this with `rg 'iroha_confidential_tree_depth'` on the same scrape to confirm that depth grows with new commitments while eviction counters only increase when the history caps trim entries. These values must line up with the Grafana dashboard export you attach to governance evidence bundles.

#### Gas schedule telemetry & alerts

Phase M2 also threads the configurable gas multipliers into the telemetry pipeline so operators can prove that every validator shares the same verification costs before approving a release:

- `iroha_confidential_gas_base_verify` mirrors `confidential.gas.proof_base` (default `250_000`).
- `iroha_confidential_gas_per_public_input`, `iroha_confidential_gas_per_proof_byte`, `iroha_confidential_gas_per_nullifier`, and `iroha_confidential_gas_per_commitment` mirror their respective knobs in `ConfidentialConfig`. Values update at start-up and whenever the config hot-reloads; `irohad` (`crates/irohad/src/main.rs:1591,1642`) pushes the active schedule through `Telemetry::set_confidential_gas_schedule`.

Scrape the gauges alongside the CommitmentTree metrics to confirm the knobs are identical across peers:

```bash
# compare active multipliers across validators
for host in validator-a validator-b validator-c; do
  curl -s "http://$host:8180/metrics" \
    | rg 'iroha_confidential_gas_(base_verify|per_public_input|per_proof_byte|per_nullifier|per_commitment)'
done
```

Grafana dashboard `confidential_assets.json` now includes a “Gas Schedule” panel that renders the five gauges and highlights divergence. Alert rules in `dashboards/alerts/confidential_assets_rules.yml` cover:
- `ConfidentialGasMismatch`: checks the max/min of each multiplier across all scrape targets and pages when any diverge for more than 3 minutes, prompting operators to align `confidential.gas` via hot-reload or redeploy.
- `ConfidentialGasTelemetryMissing`: warns when Prometheus cannot scrape any of the five multipliers for 5 minutes, indicating a missing scrape target or disabled telemetry.

Keep the following PromQL handy for on-call investigations:

```promql
# ensure every multiplier matches across validators (uses the same projection as the alert)
(max without(instance, job) (iroha_confidential_gas_per_public_input)
  - min without(instance, job) (iroha_confidential_gas_per_public_input)) == 0
```

Deviation should remain zero outside of controlled config rollouts. When changing the gas table, capture before/after scrapes, attach them to the change request, and update `docs/source/confidential_assets_calibration.md` with the new multipliers so governance reviewers can link the telemetry evidence to the calibration report.
