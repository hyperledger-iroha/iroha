---
lang: my
direction: ltr
source: docs/portal/docs/governance/api.md
status: needs-update
generator: docs/portal/scripts/sync-i18n.mjs
title: Governance App API — Endpoints (Draft)
translator: machine-google-reviewed
translation_last_reviewed: 2026-03-20
---

> Translation sync note (2026-03-20): this locale temporarily mirrors the updated English canonical text so the self-describing contract artifact and deploy API docs stay accurate while a refreshed translation is pending.

# Governance App API — Endpoints (Draft)

Status: draft/sketch to accompany the governance implementation tasks. Shapes may change during implementation. Determinism and RBAC policy are normative constraints; Torii can sign/submit transactions when `authority` and `private_key` are provided, otherwise clients build and submit to `/transaction`.

Overview
- All endpoints return JSON. For transaction-producing flows, responses include `tx_instructions` — an array of one or more instruction skeletons:
  - `wire_id`: registry identifier for the instruction type
  - `payload_hex`: Norito payload bytes (hex)
- If `authority` and `private_key` are provided (or `private_key` on ballot DTOs), Torii signs and submits the transaction and still returns `tx_instructions`.
- Otherwise, clients assemble a SignedTransaction using their authority and chain_id, then sign and POST to `/transaction`.
- SDK coverage:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` returns `GovernanceProposalResult` (normalising status/kind fields), `ToriiClient.get_governance_referendum_typed` returns `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` returns `GovernanceTally`, `ToriiClient.get_governance_locks_typed` returns `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` returns `GovernanceUnlockStats`, and `ToriiClient.list_governance_instances_typed` returns `GovernanceInstancesPage`, enforcing typed access across the governance surface with README usage examples.
- Python lightweight client (`iroha_torii_client`): `ToriiClient.finalize_referendum` and `ToriiClient.enact_proposal` return typed `GovernanceInstructionDraft` bundles (wrapping the Torii skeleton `tx_instructions`), avoiding manual JSON parsing when scripts compose Finalize/Enact flows.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` surfaces typed helpers for proposals, referenda, tallies, locks, unlock stats, and now `listGovernanceInstances(namespace, options)` plus the council endpoints (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) so Node.js clients can paginate `/v1/gov/instances/{ns}` and drive VRF-backed workflows alongside the existing contract-instance listing.

Endpoints

- POST `/v1/gov/proposals/deploy-contract`
  - Request (JSON):
    {
      "namespace": "apps",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:…" | "…64hex",
      "abi_hash": "blake2b32:…" | "…64hex",
      "abi_version": "1",
      "window": { "lower": 12345, "upper": 12400 },
      "authority": "i105…?",
      "private_key": "…?"
    }
  - Response (JSON):
    { "ok": true, "proposal_id": "…64hex", "tx_instructions": [{ "wire_id": "…", "payload_hex": "…" }] }
  - Validation: nodes canonicalise `abi_hash` for the provided `abi_version` and reject mismatches. For `abi_version = "v1"`, the expected value is `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

Contracts API (deploy)
- POST `/v1/contracts/deploy`
  - Request: { "authority": "i105...", "private_key": "…", "code_b64": "…" }
  - Behavior: Verifies the embedded `CNTR` contract interface, derives the canonical manifest from the artifact, computes `code_hash` from the full artifact body after the fixed IVM header and `abi_hash` from the enforced ABI policy, then submits `RegisterSmartContractCode` (derived manifest) and `RegisterSmartContractBytes` (full `.to` bytes) on behalf of `authority`.
  - Response: { "ok": true, "code_hash_hex": "…", "abi_hash_hex": "…" }
  - Related:
    - GET `/v1/contracts/code/{code_hash}` → returns stored manifest
    - GET `/v1/contracts/code-bytes/{code_hash}` → returns `{ code_b64 }`
- POST `/v1/contracts/instance`
  - Request: { "authority": "i105...", "private_key": "…", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "…" }
  - Behavior: Deploys the supplied bytecode and immediately activates the `(namespace, contract_id)` mapping via `ActivateContractInstance`.
  - Response: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "…", "abi_hash_hex": "…" }

Alias Service
- POST `/v1/aliases/voprf/evaluate`
  - Request: { "blinded_element_hex": "…" }
  - Response: { "evaluated_element_hex": "…128hex", "backend": "blake2b512-mock" }
    - `backend` reflects the evaluator implementation. Current value: `blake2b512-mock`.
  - Notes: Deterministic mock evaluator that applies Blake2b512 with domain separation `iroha.alias.voprf.mock.v1`. Meant for test tooling until the production VOPRF pipeline is wired through Iroha.
  - Errors: HTTP `400` on malformed hex input. Torii returns a Norito `ValidationFail::QueryFailed::Conversion` envelope with the decoder error message.
- POST `/v1/aliases/resolve`
  - Request: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Response: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Notes: Requires the ISO bridge runtime staging (`[iso_bridge.account_aliases]` in `iroha_config`). Torii normalises aliases by stripping whitespace and upper-casing before lookup. Returns 404 when the alias is absent and 503 when the ISO bridge runtime is disabled.
- POST `/v1/aliases/resolve_index`
  - Request: { "index": 0 }
  - Response: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Notes: Alias indices are assigned deterministically from configuration order (0-based). Clients can cache responses offline to build audit trails for alias attestation events.

Code Size Cap
- Custom parameter: `max_contract_code_bytes` (JSON u64)
  - Controls the maximum allowed size (in bytes) for on-chain contract code storage.
  - Default: 16 MiB. Nodes reject `RegisterSmartContractBytes` when the `.to` image length exceeds the cap with an invariant violation error.
  - Operators can adjust by submitting `SetParameter(Custom)` with `id = "max_contract_code_bytes"` and a numeric payload.

- POST `/v1/gov/ballots/zk`
  - Request: { "authority": "i105...", "private_key": "…?", "chain_id": "…", "election_id": "e1", "proof_b64": "…", "public": {…} }
  - Response: { "ok": true, "accepted": true, "tx_instructions": [{…}] }
  - Notes:
    - When the circuit’s public inputs include `owner`, `amount`, and `duration_blocks`, and the proof verifies against the configured VK, the node creates or extends a governance lock for `election_id` with that `owner`. Direction remains hidden (`unknown`); only amount/expiry are updated. Re-votes are monotonic: amount and expiry only increase (the node applies max(amount, prev.amount) and max(expiry, prev.expiry)).
    - ZK re-votes that attempt to shrink amount or expiry are rejected server-side with `BallotRejected` diagnostics.
    - Contract execution must call `ZK_VOTE_VERIFY_BALLOT` prior to enqueuing `SubmitBallot`; hosts enforce a one-shot latch.

- POST `/v1/gov/ballots/plain`
  - Request: { "authority": "i105...", "private_key": "…?", "chain_id": "…", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - Response: { "ok": true, "accepted": true, "tx_instructions": [{…}] }
  - Notes: Re-votes are extend-only — a new ballot cannot reduce the existing lock’s amount or expiry. The `owner` must equal the transaction authority. Minimum duration is `conviction_step_blocks`.

- POST `/v1/gov/finalize`
  - Request: { "referendum_id": "r1", "proposal_id": "…64hex", "authority": "i105…?", "private_key": "…?" }
  - Response: { "ok": true, "tx_instructions": [{ "wire_id": "…FinalizeReferendum", "payload_hex": "…" }] }
  - On-chain effect (current scaffold): enacting an approved deploy proposal inserts a minimal `ContractManifest` keyed by `code_hash` with the expected `abi_hash` and marks the proposal Enacted. If a manifest already exists for the `code_hash` with a different `abi_hash`, enactment is rejected.
  - Notes:
    - For ZK elections, contract paths must call `ZK_VOTE_VERIFY_TALLY` prior to executing `FinalizeElection`; hosts enforce a one-shot latch. `FinalizeReferendum` rejects ZK referenda until the election tally is finalized.
    - Auto-close at `h_end` emits Approved/Rejected only for Plain referenda; ZK referenda remain closed until a finalized tally is submitted and `FinalizeReferendum` is executed.
    - Turnout checks use approve+reject only; abstain does not count toward turnout.

- POST `/v1/gov/enact`
  - Request: { "proposal_id": "…64hex", "preimage_hash": "…64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "…?" }
  - Response: { "ok": true, "tx_instructions": [{ "wire_id": "…EnactReferendum", "payload_hex": "…" }] }
  - Notes: Torii submits the signed transaction when `authority`/`private_key` are provided; otherwise it returns a skeleton for clients to sign and submit. The preimage is optional and currently informational.

- GET `/v1/gov/proposals/{id}`
  - Path `{id}`: proposal id hex (64 chars)
  - Response: { "found": bool, "proposal": { … }? }

- GET `/v1/gov/locks/{rid}`
  - Path `{rid}`: referendum id string
  - Response: { "found": bool, "referendum_id": "rid", "locks": { … }? }

- GET `/v1/gov/council/current`
  - Response: { "epoch": N, "members": [{ "account_id": "…" }, …] }
  - Notes: Returns the persisted council when present; otherwise derives a deterministic fallback using the configured stake asset and thresholds (mirrors the VRF spec until live VRF proofs are persisted on chain).

- POST `/v1/gov/council/derive-vrf` (feature: gov_vrf)
  - Request: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "…", "variant": "Normal|Small", "pk_b64": "…", "proof_b64": "…" }, …] }
  - Behavior: Verifies each candidate’s VRF proof against the canonical input derived from `chain_id`, `epoch`, and the latest block hash beacon; sorts by output bytes desc with tiebreakers; returns the top `committee_size` members. Does not persist.
  - Response: { "epoch": N, "members": [{ "account_id": "…" } …], "total_candidates": M, "verified": K }
  - Notes: Normal = pk in G1, proof in G2 (96 bytes). Small = pk in G2, proof in G1 (48 bytes). Inputs are domain-separated and include `chain_id`.

### Governance defaults (iroha_config `gov.*`)

The council fallback used by Torii when no persisted roster exists is parameterised via `iroha_config`:

```toml
[gov]
  vk_ballot.backend = "halo2/ipa"
  vk_ballot.name    = "ballot_v1"
  vk_tally.backend  = "halo2/ipa"
  vk_tally.name     = "tally_v1"
  plain_voting_enabled = false
  conviction_step_blocks = 100
  max_conviction = 6
  approval_q_num = 1
  approval_q_den = 2
  min_turnout = 0
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

Equivalent environment overrides:

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

`parliament_committee_size` caps the number of fallback members returned when no council has been persisted, `parliament_term_blocks` defines the epoch length used for seed derivation (`epoch = floor(height / term_blocks)`), `parliament_min_stake` enforces the minimum stake (in smallest units) on the eligibility asset, and `parliament_eligibility_asset_id` selects which asset balance is scanned when building the candidate set.

Governance VK verification has no bypass: ballot verification always requires an `Active` verifying key with inline bytes, and environments must not rely on test-only toggles to skip verification.

RBAC
- On-chain execution requires permissions:
  - Proposals: `CanProposeContractDeployment{ contract_id }`
  - Ballots: `CanSubmitGovernanceBallot{ referendum_id }`
  - Enactment: `CanEnactGovernance`
  - Council management (future): `CanManageParliament`

Protected Namespaces
- Custom parameter `gov_protected_namespaces` (JSON array of strings) enables admission gating for deploys into listed namespaces.
- Clients must include transaction metadata keys for deploys targeting protected namespaces:
  - `gov_namespace`: the target namespace (e.g., `"apps"`)
  - `gov_contract_id`: the logical contract id within the namespace
- `gov_manifest_approvers`: optional JSON array of validator account IDs. When a lane manifest declares a quorum greater than one, admission requires the transaction authority plus the listed accounts to satisfy the manifest quorum.
- Telemetry exposes holistic admission counters via `governance_manifest_admission_total{result}` so operators can distinguish successful admits from `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected`, and `runtime_hook_rejected` paths.
- Telemetry surfaces the enforcement path via `governance_manifest_quorum_total{outcome}` (values `satisfied` / `rejected`) so operators can audit missing approvals.
- Lanes enforce the namespace allowlist published in their manifests. Any transaction that sets `gov_namespace` must provide `gov_contract_id`, and the namespace must appear in the manifest's `protected_namespaces` set. `RegisterSmartContractCode` submissions without this metadata are rejected when protection is enabled.
- Admission enforces that an Enacted governance proposal exists for the tuple `(namespace, contract_id, code_hash, abi_hash)`; otherwise validation fails with a NotPermitted error.

Runtime Upgrade Hooks
- Lane manifests may declare `hooks.runtime_upgrade` to gate runtime upgrade instructions (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Hook fields:
  - `allow` (bool, default `true`): when `false`, all runtime-upgrade instructions are rejected.
  - `require_metadata` (bool, default `false`): require the transaction metadata entry specified by `metadata_key`.
  - `metadata_key` (string): metadata name enforced by the hook. Defaults to `gov_upgrade_id` when metadata is required or an allowlist is present.
  - `allowed_ids` (array of strings): optional allowlist of metadata values (after trimming). Rejects when the provided value is not listed.
- When the hook is present, queue admission enforces the metadata policy before the transaction enters the queue. Missing metadata, blank values, or values outside the allowlist produce a deterministic `NotPermitted` error.
- Telemetry tracks enforcement outcomes via `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Transactions satisfying the hook must include metadata `gov_upgrade_id=<value>` (or the manifest-defined key) alongside any validator approvals required by the manifest quorum.

Convenience Endpoint
- POST `/v1/gov/protected-namespaces` — applies `gov_protected_namespaces` directly on the node.
  - Request: { "namespaces": ["apps", "system"] }
  - Response: { "ok": true, "applied": 1 }
  - Notes: Intended for admin/testing; requires API token if configured. For production, prefer submitting a signed transaction with `SetParameter(Custom)`.

CLI Helpers
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Fetches contract instances for the namespace and cross-checks that:
    - Torii stores bytecode for each `code_hash`, and its Blake2b-32 digest matches the `code_hash`.
    - The manifest stored under `/v1/contracts/code/{code_hash}` reports matching `code_hash` and `abi_hash` values.
    - An enacted governance proposal exists for `(namespace, contract_id, code_hash, abi_hash)` as derived by the same proposal-id hashing the node uses.
  - Outputs a JSON report with `results[]` per contract (issues, manifest/code/proposal summaries) plus a one-line summary unless suppressed (`--no-summary`).
  - Useful for auditing protected namespaces or verifying governance-controlled deploy workflows.
- `iroha app gov deploy meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Emits the JSON metadata skeleton used when submitting deployments into protected namespaces, including optional `gov_manifest_approvers` for satisfying manifest quorum rules.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — lock hints are required when `min_bond_amount > 0`, and any provided hint set must include `owner`, `amount`, and `duration_blocks`.
  - Validates canonical account ids, canonicalizes 32-byte nullifier hints, and merges the hints into `public_inputs_json` (with `--public <path>` for additional overrides).
  - The nullifier is derived from the proof commitment (public input) plus `domain_tag`, `chain_id`, and `election_id`; `--nullifier` is validated against the proof when supplied.
  - The one-line summary now surfaces a deterministic `fingerprint=<hex>` derived from the encoded `CastZkBallot` along with any decoded hints (`owner`, `amount`, `duration_blocks`, `direction` when provided).
  - CLI responses annotate `tx_instructions[]` with `payload_fingerprint_hex` plus decoded fields so downstream tooling can verify the skeleton without reimplementing Norito decoding.
  - Supplying the lock hints allows the node to emit `LockCreated`/`LockExtended` events for ZK ballots once the circuit exposes the same values.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--owner` accepts canonical I105 literals; optional `@<domain>` suffixes are routing hints only.
  - Aliases `--lock-amount`/`--lock-duration-blocks` mirror the ZK flag names for scripting parity.
  - Summary output mirrors `vote --mode zk` by including the encoded instruction fingerprint and human-readable ballot fields (`owner`, `amount`, `duration_blocks`, `direction`), providing quick confirmation before signing the skeleton.

Instances Listing
- GET `/v1/gov/instances/{ns}` — lists active contract instances for a namespace.
  - Query params:
    - `contains`: filter by substring of `contract_id` (case-sensitive)
    - `hash_prefix`: filter by hex prefix of `code_hash_hex` (lowercase)
    - `offset` (default 0), `limit` (default 100, max 10_000)
    - `order`: one of `cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`
  - Response: { "namespace": "ns", "instances": [{ "contract_id": "…", "code_hash_hex": "…" }, …], "total": N, "offset": n, "limit": m }
  - SDK helper: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) or `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Unlock Sweep (Operator/Audit)
- GET `/v1/gov/unlocks/stats`
  - Response: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notes: `last_sweep_height` reflects the most recent block height where expired locks were swept and persisted. `expired_locks_now` is computed by scanning lock records with `expiry_height <= height_current`.
- POST `/v1/gov/ballots/zk-v1`
  - Request (v1-style DTO):
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "…?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x…64hex?",
      "owner": "i105…?",          // canonical AccountId (I105 literal)
      "amount": "100?",
      "duration_blocks": 6000?,
      "direction": "Aye|Nay|Abstain?",
      "nullifier": "blake2b32:…64hex?"
    }
  - Response: { "ok": true, "accepted": true, "tx_instructions": [{…}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (feature: `zk-ballot`)
  - Accepts a `BallotProof` JSON directly and returns a `CastZkBallot` skeleton.
  - Request:
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "…?",
      "election_id": "ref-1",
      "ballot": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=",   // base64 of ZK1 or H2* container
        "root_hint": null,                // optional 32-byte hex string (eligibility root)
        "owner": null,                    // optional canonical AccountId (I105 literal)
        "nullifier": null,                // optional 32-byte hex string (nullifier hint)
        "amount": "100",                  // optional lock amount hint (decimal string)
        "duration_blocks": 6000,          // optional lock duration hint
        "direction": "Aye"                // optional direction hint
      }
    }
  - Response:
    {
      "ok": true,
      "accepted": true,
      "reason": "build transaction skeleton",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "…" }
      ]
    }
  - Notes:
    - The server maps optional `root_hint`/`owner`/`amount`/`duration_blocks`/`direction`/`nullifier` from the ballot to `public_inputs_json` for `CastZkBallot`.
    - The envelope bytes are re-encoded as base64 for the instruction payload.
    - The response `reason` changes to `submitted transaction` when Torii submits the ballot.
    - This endpoint is only available when the `zk-ballot` feature is enabled.

CastZkBallot Verification Path
- `CastZkBallot` decodes the supplied base64 proof and rejects empty or malformed payloads (`BallotRejected` with `invalid or empty proof`).
- If `public_inputs_json` is supplied, it must be a JSON object; non-object payloads are rejected.
- The host resolves the ballot verifying key from the referendum (`vk_ballot`) or governance defaults and requires the record to exist, be `Active`, and carry inline bytes.
- Stored verifying-key bytes are re-hashed with `hash_vk`; any commitment mismatch aborts execution before verification to guard against tampered registry entries (`BallotRejected` with `verifying key commitment mismatch`).
- Proof bytes are dispatched to the registered backend via `zk::verify_backend`; invalid transcripts surface as `BallotRejected` with `invalid proof` and the instruction fails deterministically.
- The proof must expose a ballot commitment and eligibility root as public inputs; the root must match the election’s `eligible_root`, and the derived nullifier must match any provided hint.
- Successful proofs emit `BallotAccepted`; duplicate nullifiers, stale eligibility roots, or lock regressions continue to produce the existing rejection reasons described earlier in this document.

## Validator Misbehaviour & Joint Consensus

### Slashing and Jailing Workflow

Consensus emits Norito-encoded `Evidence` whenever a validator violates the protocol. Each payload lands in the in-memory `EvidenceStore` and, if unseen, is materialised into the WSV-backed `consensus_evidence` map. Records older than `sumeragi.npos.reconfig.evidence_horizon_blocks` (default `7 200` blocks) are rejected so the archive remains bounded, but the rejection is logged for operators. Evidence within the horizon obeys the joint-consensus staging rule (`mode_activation_height requires next_mode to be set in the same block`), the activation delay (`sumeragi.npos.reconfig.activation_lag_blocks`, default `1`), and the slashing delay (`sumeragi.npos.reconfig.slashing_delay_blocks`, default `259200`) so governance can cancel penalties before they apply.

Recognised offences map one-to-one to `EvidenceKind`; the discriminants are stable and enforced by the data model:

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** — the validator signed conflicting hashes for the same `(phase,height,view,epoch)` tuple.
- **InvalidQc** — an aggregator gossiped a commit certificate whose shape fails deterministic checks (e.g., empty signer bitmap).
- **InvalidProposal** — a leader proposed a block that fails structural validation (e.g., breaks the locked-chain rule).
- **Censorship** — signed submission receipts show a transaction that was never proposed/committed.

VRF penalties are enforced automatically after `activation_lag_blocks` (offenders are jailed). Consensus slashing is applied only after the `slashing_delay_blocks` window unless governance cancels the penalty.

Operators and tooling can inspect and re-broadcast payloads through:

- Torii: `GET /v1/sumeragi/evidence` and `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `… count`, and `… submit --evidence-hex <payload>`.

Governance must treat the evidence bytes as canonical proof:

1. **Collect the payload** before it ages out. Archive the raw Norito bytes alongside height/view metadata.
2. **Cancel if needed** by submitting `CancelConsensusEvidencePenalty` with the evidence payload before `slashing_delay_blocks` elapses; the record is marked `penalty_cancelled` and `penalty_cancelled_at_height`, and no slashing applies.
3. **Stage the penalty** by embedding the payload in a referendum or sudo instruction (e.g., `Unregister::peer`). Execution re-validates the payload; malformed nor stale evidence is rejected deterministically.
4. **Schedule the follow-up topology** so the offending validator cannot immediately rejoin. Typical flows queue `SetParameter(Sumeragi::NextMode)` and `SetParameter(Sumeragi::ModeActivationHeight)` with the updated roster.
5. **Audit results** via `/v1/sumeragi/evidence` and `/v1/sumeragi/status` to ensure the evidence counter advanced and governance enacted the removal.

### Joint-Consensus Sequencing

Joint consensus guarantees that the outgoing validator set finalises the boundary block before the new set starts proposing. The runtime enforces the rule via paired parameters:

- `SumeragiParameter::NextMode` and `SumeragiParameter::ModeActivationHeight` must be committed in the **same block**. `mode_activation_height` must be strictly greater than the block height that carried the update, providing at least one-block lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (default `1`) is the configuration guard that prevents zero-lag hand-offs:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (default `259200`) delays consensus slashing so governance can cancel penalties before they apply.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- The runtime and CLI expose staged parameters through `/v1/sumeragi/params` and `iroha --output-format text ops sumeragi params`, so operators can confirm activation heights and validator rosters.
- Governance automation should always:
  1. Finalise the evidence-backed removal (or reinstatement) decision.
  2. Queue a follow-up reconfiguration with `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitor `/v1/sumeragi/status` until `effective_consensus_mode` flips at the expected height.

Any script that rotates validators or applies slashing **must not** attempt zero-lag activation or omit the hand-off parameters; such transactions are rejected and leave the network in the previous mode.

## Telemetry surfaces

- Prometheus metrics export governance activity:
  - `governance_proposals_status{status}` (gauge) tracks proposal counts by status.
  - `governance_protected_namespace_total{outcome}` (counter) increments when protected namespace admission allows or rejects a deploy.
  - `governance_manifest_activations_total{event}` (counter) records manifest insertions (`event="manifest_inserted"`) and namespace bindings (`event="instance_bound"`).
- `/status` includes a `governance` object mirroring the proposal counts, reporting protected namespace totals, and listing recent manifest activations (namespace, contract id, code/ABI hash, block height, activation timestamp). Operators can poll this field to confirm that enactments updated manifests and that protected namespace gates are enforced.
- A Grafana template (`docs/source/grafana_governance_constraints.json`) and the
  telemetry runbook in `telemetry.md` show how to wire alarms for stuck
  proposals, missing manifest activations, or unexpected protected-namespace
  rejections during runtime upgrades.
