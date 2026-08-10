---
title: Governance App API — Endpoints
---

Status: first-release current contract. Determinism and RBAC policy are
normative constraints; Torii returns unsigned instruction skeletons for
governance transaction-producing flows. Request schemas exclude private
signing material and reject unknown fields before constructing a request
object. Clients sign locally and submit via `/v1/pipeline/transactions`.

Important: we do not ship a standing council or “default” governance roster.
Until governance persists a roster, the current-council endpoint returns the
constant empty state; a read never scans assets or derives a hidden fallback.
A citizen is an account that posted the configured minimum bond; the bond is
an anti-Sybil/collateral floor and does not increase Parliament draw odds or
vote weight above the minimum. There is no baked-in multisig, secret key, or
privileged council account in this repository.

Overview
- Endpoints return JSON except where an endpoint explicitly documents a typed
  Norito proof response. For transaction-producing flows, responses include
  `tx_instructions` — an array of one or more instruction skeletons:
  - `wire_id`: registry identifier for the instruction type
  - `payload_hex`: Norito payload bytes (hex)
- Governance endpoints do not server-sign and their request types have no
  private-key fields. Clients assemble a `SignedTransaction` using their
  authority and exact genesis-derived `NetworkId`, then sign locally and POST to
  `/v1/pipeline/transactions`.
- Runtime/governance routes that read committed state, derive a principal-bound
  draft, or perform bounded proof work require canonical account request
  authentication. The signature commits the exact genesis-derived `NetworkId`,
  HTTP method, origin-form URI (including the query), bounded raw body, timestamp,
  and one-shot nonce. Torii verifies the account against committed state before
  path/query/body extraction or computation, rejects wrong-network signatures
  and nonce replay, and serves successful authenticated responses with
  `Cache-Control: private, no-store` and the canonical-auth `Vary` fields.
  Catalog paths use strict normalization: clients sign and send the declared path
  exactly, without a trailing-slash redirect.
- This boundary covers the ZK roots, Merkle-path and vote-tally reads; active ABI,
  runtime-metrics, node/privacy-capability and projection-checkpoint reads; the
  Ministry draft/read routes; governance proposal, capability, citizen, lock,
  referendum, tally, protected-namespace, unlock, governed-contract, enactment,
  and council reads/drafts; and all typed validation-fee proof/proposal routes.
  The Ministry agenda `authority`, citizenship-draft `owner`, and validation-fee
  PLAIN-ballot-draft `owner` must equal the verified account before state access.
  Operator and protocol-handshake routes retain their stronger dedicated
  boundaries. Only the fixed ABI-v1 hash calculator and the state-independent
  referendum-finalize instruction calculator remain public in this audited
  route family.
- SDK coverage:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` returns `GovernanceProposalResult` (normalising status/kind fields), `ToriiClient.get_governance_referendum_typed` returns `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` returns `GovernanceTally`, and `ToriiClient.get_governance_locks_typed` returns `GovernanceLocksResult`.
- Python lightweight client (`iroha_torii_client`): `ToriiClient.finalize_referendum` and `ToriiClient.enact_proposal` return typed `GovernanceInstructionDraft` bundles (wrapping the Torii skeleton `tx_instructions`), avoiding manual JSON parsing when scripts compose Finalize/Enact flows.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` surfaces typed helpers for
  proposals, referenda, tallies, locks, unlock stats, and the current council
  projection (`getGovernanceCouncilCurrent`).
  `governanceFinalizeReferendumTyped` and `governanceEnactProposalTyped` mirror
  the Python helpers by always returning a structured draft (synthesising the
  empty skeleton when Torii responds with `204 No Content`), which keeps
  automation from branching on `null` before queueing transactions or
  triggers.
- Rust (`iroha::client::Client`):
  `post_validation_fee_plain_ballot_draft` accepts the typed
  `ValidationFeePlainBallotDraftRequestV1`, calls the proposal-bound route, and
  rejects a response unless its canonical framed `CastPlainBallot` exactly
  matches the requested proposal id, owner, and direction together with the
  returned immutable amount and duration.

## SoraFS Governance DAG read authority

The `/v1/sorafs/governance/dag/*` read routes do not trust mutable authority
filenames. Publish-index and CAR-queue handlers consume one canonical typed
publication snapshot from `NodeHandle`; runtime handlers consume one typed
head/index snapshot authenticated by the exact sealed producer checkpoint. The
dashboard, head, block, and node routes consume only the supervised Governance
DAG service's mirror-read capability, which irohad installs exactly once before
the first `NodeHandle` clone is shared. A configured node without that
capability has no mirror authority, and there is no loose-file fallback.

Successful JSON projections identify the authority with `source`,
`source_generation`, and `source_record_blake3`. Mirror and runtime projections
also include `source_checkpoint_generation` and
`source_checkpoint_revision`. They never expose a mutable authority
`source_path` or runtime `head_path`; root-relative immutable artifact paths may
still appear where a response identifies a content-addressed source, block, or
CAR object. Representation ETags commit the typed record identity and, for
mirror/runtime reads, the sealed checkpoint identity before conditional
matching, so changed authentication metadata cannot be hidden by `304 Not
Modified`.

Endpoints

- GET `/v1/gov/capabilities`
  - Exact-network account-authenticated readiness projection. Returns schema
    `iroha.governance.capabilities.v1`, version `1`, one mandatory typed
    `network_id`, current height, ABI/data-model versions, configured PLAIN voting
    and turnout/window parameters, all seven configured Parliament body
    targets, supported proposal kinds, and supported routes.
  - Configured body sizes are targets, not a minimum citizen count. Each
    proposal-time JIT roster is independently capped at the number of eligible
    bonded citizens. A non-empty one-citizen registry therefore produces one
    immutable member in every body and quorum `1`; zero eligible citizens fails
    proposal creation.
  - Every governance integer other than the fixed response `version: 1` is a
    canonical unsigned decimal JSON string. This includes heights, windows,
    thresholds, body targets, and quorum/count fields; clients must parse the
    complete string before applying any bounded UI conversion.

- POST `/v1/gov/citizens/draft`
  - Strict request: `{ "version": 1, "owner": "<i105-account-id>" }`.
  - Returns the exact configured citizenship amount and one
    `RegisterCitizen` instruction skeleton. Unknown fields and unsupported
    versions are rejected; the node never signs the draft.

- POST `/v1/validation-fee/policy/current/proof`
  - Accepts the strict Norito V1 request containing `version` and a non-zero
    `trusted_checkpoint_height`. It returns the complete protected registry
    when configured, its synthetic ordinary-write witness, and a consecutive
    Sumeragi-v2 finality page beginning at that checkpoint.
  - A page contains at most 64 finality proofs and advances at most 63 blocks.
    While `more_available` is true, clients promote `evaluated_context_id` and
    request the next page; an incomplete, skipped, reordered, rollback, or
    equivocal chain is not deployable evidence.
  - Clients verify locally with the immutable chain id, genesis hash,
    policy-chain genesis hash, checkpoint height, and checkpoint context id.
    The resulting verified projection includes, for both the policy and payout
    lifecycle proposals, `plainElectorateRules` and
    `plainElectorateSnapshot.{rosterRoot,memberCount,capturedAtHeight,approvalGateHeight}`.
    Verification also requires PLAIN finalization, matching rules and snapshot
    anchors, and the exact
    `effective_from_height = enacted_at_height + 120,960` relation.
- GET `/v1/validation-fee/proposals`
  - Lists only typed native validation-fee policy and payout-lifecycle
    proposals. Each record carries `plain_electorate_snapshot`: it is `null`
    before referendum opening and the complete frozen citizen roster
    thereafter.
- GET `/v1/validation-fee/proposals/{proposal_id}?account_id=<i105-account-id>`
  - Returns the exact proposal/referendum, proposal-time Parliament snapshot,
    frozen PLAIN electorate snapshot, current height, per-body members,
    alternates, quorum and decision counts for all seven bodies, optional
    current-account decisions, the live or finalized citizen tally, the
    ordered proposal pipeline, and current retained voter locks. The electorate
    snapshot contains the proposal/operator binding, capture and approval-gate
    heights, exact member count, canonical member records, and roster root. All
    integer fields in this projection are canonical unsigned decimal strings.
- POST `/v1/validation-fee/proposals/draft`
  - Builds exactly one native PLAIN validation-fee proposal instruction for
    local signing. The strict request requires `plain_electorate_rules`; those
    exact rules are included in the native proposal fingerprint and retained
    for the voting asset, bond escrow, slash receiver, ballot eligibility,
    amount, duration, conviction, turnout, and approval checks. Legacy
    signed-policy, governance-keyset, detached-signature, and ZK compatibility
    shapes are not accepted.
  - The supplied rules and inclusive referendum span must exactly match active
    governance configuration. Taira fixes the span at 3,600 blocks
    (`h_end = h_start + 3,599`); the draft route rejects any other span or
    rule set.
- POST `/v1/validation-fee/proposals/{proposal_id}/plain-ballot/draft`
  - Strict request:
    `{ "version": 1, "owner": "<i105-account-id>", "direction": "AYE" | "NAY" | "ABSTAIN" }`.
  - Returns exactly one canonical framed `CastPlainBallot` instruction for
    local signing. The response repeats the exact proposal id, owner,
    direction, proposal-bound amount, and proposal-bound duration.
  - The route fails closed unless the referendum is PLAIN and open at the next
    possible inclusion height, all seven retained Parliament bodies still
    satisfy the proposal snapshot, the owner belongs to the electorate frozen
    at `h_start`, and the account has not already cast an effective ballot.
    Membership is never recomputed from the live citizen registry. Callers
    cannot override amount or duration.
  - Accepted locks retain the proposal-bound voting asset, bond escrow, and
    slash receiver. Later governance configuration changes cannot redirect
    locking, release, slashing, or restitution; missing or mismatched custody
    evidence fails closed without deleting the lock.

- POST `/v1/gov/proposals/deploy-contract`
  - Request (JSON):
    {
      "contract_alias": "router::universal"?,
      "contract_address": "irohac1..."?,
      "code_hash": "blake2b32:0x…" | "0x…" | "…64hex",
      "abi_hash": "blake2b32:0x…" | "0x…" | "…64hex",
      "abi_version": "1",
      "window": { "lower": 12345, "upper": 12400 },
      "mode": "Zk" | "Plain",
      "manifest_provenance": { "signer": "ed0120…", "signature": "…" }?
    }
  - Response (JSON):
    { "ok": true, "proposal_id": "…64hex", "tx_instructions": [{ "wire_id": "…", "payload_hex": "…" }] }
  - Validation:
    - exactly one of `contract_address` or `contract_alias` must be provided;
    - aliases resolve to the current active canonical contract address before the proposal id is derived;
    - `code_hash` and `abi_hash` accept only a 64-digit hexadecimal body,
      optionally preceded by the case-insensitive `blake2b32:` scheme and/or
      `0x` prefix, and are canonicalised to unprefixed lowercase hex;
    - only the exact string `abi_version = "1"` is accepted, and `abi_hash`
      must equal the canonical ABI hash for that version
      (`hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`);
    - `window.upper` must be `>= window.lower`; and
    - `mode`, when supplied, must be the exact canonical label `Zk` or `Plain`.
  - Submission model: this endpoint is draft-only. Its strict request schema
    contains neither authority nor private-key material; clients consume
    `tx_instructions`, sign locally, and submit via
    `/v1/pipeline/transactions`.

Contracts API (locally signed deployment)
- Torii does not expose a server-side deployment endpoint and never accepts a
  deployment private key.
- Clients upload/finalize bytecode, register a locally signed manifest, and
  submit `CommitContractDeployment` through the standard transaction pipeline.
- Client tooling records the result as `DeployContractBundleReceiptDto`: its
  `contracts[]` entries preserve the per-contract address, hashes, nonce, and
  outcome instead of flattening a multi-contract deployment into one response.
- The commit instruction atomically checks the expected deployment nonce and
  previous alias target before activation or rotation.
- Related reads:
  - GET `/v1/contracts/code/{code_hash}` → stored manifest
  - GET `/v1/contracts/code-bytes/{code_hash}` → `{ code_b64 }`

Alias Service
- POST `/v1/aliases/resolve`
  - Request: { "alias": "merchant@paynet" }
  - Response: { "alias": "merchant@paynet", "account_id": "<i105-account-id>", "index": 12, "source": "on_chain" }
  - Notes: This is an exact public mapping, not a search endpoint. The request must contain the canonical fully-qualified alias. Torii routes through the alias dataspace, independently rate-limits the route, and accepts unsigned requests. If canonical-signature headers are supplied they must verify; invalid or partial signing headers never downgrade to anonymous access. Returns `404` for an unknown exact alias and `503` when its authoritative route cannot be reached.
- POST `/v1/aliases/resolve-index`
  - Request: { "index": 0 }
  - Response: { "index": 0, "alias": "merchant@paynet", "account_id": "<i105-account-id>", "source": "fanout" }
  - Notes: Canonical request signing is required. Because the index alone does not encode a dataspace, Torii fans this lookup out across the signed caller's visible dataspace routes, dedupes identical results, and returns `source = "fanout"` when the response comes from multi-route merging. Returns `409 route_conflict` if multiple dataspaces return incompatible bindings, `403` for missing/invalid signing or inaccessible routes, `404` when reachable routes miss, and `503` when no route can be reached.
- POST `/v1/aliases/by-account`
  - Request: { "account_id": "<i105-account-id>", "dataspace": "paynet"?, "domain": "merchant"?" }
  - Response: { "account_id": "<i105-account-id>", "total": 2, "items": [{ "alias": "merchant@paynet", "dataspace": "paynet", "domain": null, "is_primary": false }], "source": "fanout" }
  - Notes: This is the exact public reverse mapping for one canonical I105 account, not prefix/index enumeration. Torii queries the target account's routes, merges and deterministically sorts at most 64 deduplicated public alias rows, and recomputes `total`. The route is independently rate limited and accepts unsigned requests; supplied canonical-signature headers must verify. Returns `404` when the exact account has no reachable alias result, `409` for conflicting account roots, and `503` when no route can be reached.

Code Size Cap
- Custom parameter: `max_contract_code_bytes` (JSON u64)
  - Controls the maximum allowed size (in bytes) for on-chain contract code storage.
  - Default: 16 MiB. Nodes reject `RegisterSmartContractBytes` when the `.to` image length exceeds the cap with an invariant violation error.
  - Operators can adjust by submitting `SetParameter(Custom)` with `id = "max_contract_code_bytes"` and a numeric payload.

- POST `/v1/gov/ballots/plain`
  - Request: { "authority": "<i105-account-id>", "network_id": "hash:<64-uppercase-hex>#<CRC16>", "referendum_id": "r1", "owner": "<i105-account-id>", "amount": "1000", "duration_blocks": "6000", "direction": "Aye|Nay|Abstain" }
  - Response: { "ok": true, "accepted": true, "tx_instructions": [{…}] }
  - Notes: Re-votes are extend-only — a new ballot cannot reduce the existing
    lock’s amount or expiry. The `owner` must equal the transaction authority.
    Minimum duration is `conviction_step_blocks`, and the resulting lock must
    remain active through the referendum's inclusive `h_end`.
    Proposal-backed PLAIN voting cannot open a referendum: every required
    proposal-time Parliament body must first reach its exact snapshot quorum,
    after which consensus opens the referendum. Standalone PLAIN referenda
    retain their explicit non-proposal behavior.
    Context identifiers use the canonical first-release governance selector
    grammar: 1–128 RFC 3986 unreserved ASCII bytes without a leading dot.
    `amount` uses the same
    canonical Kotodama V1 `Quantity` grammar as ZK lock hints, while
    `duration_blocks` is a canonical decimal string in `0..=u64::MAX`.
    Torii verifies a canonical exact-network account signature over the bounded
    raw body before JSON decoding, requires its account to equal `authority`,
    and rejects redirects/replays. `chain_id` and `genesis_hash` are retired.

- POST `/v1/gov/finalize`
  - Strict request: { "referendum_id": "…64hex", "proposal_id": "…same 64hex" }
  - Response: { "ok": true, "tx_instructions": [{ "wire_id": "…FinalizeReferendum", "payload_hex": "…" }] }
  - On-chain effect (current scaffold): enacting an approved deploy proposal inserts a minimal `ContractManifest` keyed by `code_hash` with the expected `abi_hash` and marks the proposal Enacted. If a manifest already exists for the `code_hash` with a different `abi_hash`, enactment is rejected.
  - Notes:
    - `referendum_id` and `proposal_id` must be the same exact 64-character
      lowercase hexadecimal proposal fingerprint. Prefixes, uppercase forms,
      whitespace, and distinct selector aliases are rejected before a draft
      instruction is constructed.
    - For ZK elections, contract paths must call `ZK_VOTE_VERIFY_TALLY` prior to executing `FinalizeElection`; hosts enforce a one-shot latch. `FinalizeReferendum` rejects ZK referenda until the election tally is finalized.
    - `h_end` is inclusive. PLAIN referenda close and tally at the start of
      `h_end + 1`, while finalization evidence remains anchored to `h_end`.
      Manual PLAIN finalization at or before `h_end` is rejected. ZK referenda
      still require a finalized election tally.
    - Turnout is `approve + reject + abstain`; abstentions count toward turnout
      and the configured approval denominator.

- POST `/v1/gov/enact`
  - Strict request: `{ "proposal_id": "…64-lowercase-hex" }`.
  - Response:
    `{ "ok": true, "proposal_id": "…", "proposal_kind": {…}, "referendum_window": {"lower": H1, "upper": H2}, "tx_instructions": [{…}] }`.
  - Notes: Torii reads an approved proposal and its exact closed referendum,
    derives the instruction preimage fingerprint and retained window from
    ledger state, and returns a skeleton for local signing. Caller-supplied
    preimages, windows, authorities, private keys, and server-side submission
    are rejected by the strict request shape. On-chain enactment must occur
    after the approved close and rechecks both bindings.

- GET `/v1/gov/proposals/{id}`
  - Path `{id}`: exact lowercase proposal id hex (64 chars); `0x`, uppercase,
    whitespace, and control-character aliases are rejected before lookup.
  - Response: { "found": bool, "proposal": { … }? }

- GET `/v1/gov/locks/{rid}`
  - Path `{rid}`: exact non-empty referendum token; whitespace and control
    characters are rejected rather than trimmed or treated as a cache-miss.
  - Response: { "found": bool, "referendum_id": "rid", "locks": { … }? }

- GET `/v1/gov/referenda/{id}` and GET `/v1/gov/tally/{id}`
  - Path `{id}` follows the same exact non-empty referendum-token grammar as
    the locks endpoint. Noncanonical variants fail before state lookup.

- GET `/v1/gov/council/current`
  - Response: { "epoch": N, "members": [{ "account_id": "…" }, …] }
  - Notes: Returns the latest persisted council through the ordered council
    index. When none exists, returns the constant empty state; it never derives
    a roster by scanning account assets.

- POST `/v1/gov/parliament/ballots`
  - Request: { "authority": "<i105-account-id>", "network_id": "hash:<64-uppercase-hex>#<CRC16>", "proposal_id": "<hex32>", "body": "policy-jury", "decision": "approve|reject|abstain" }
  - `body` uses the exact canonical kebab-case Parliament body label.
  - `decision` uses those exact lowercase spellings; capitalized aliases and
    surrounding whitespace are rejected.
  - Behavior: Builds a `CastParliamentBallot` instruction skeleton. The transaction authority must be the seated body member; alternates cannot vote until promoted into the roster.

### Governance defaults (iroha_config `gov.*`)

Governance execution is parameterised via `iroha_config`; these settings do not
make the current-council read endpoint derive an implicit roster:

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
  voting_asset_id = "61CtjvNd9T3THAR65GsMVHr82Bjc"         # governance bond asset (Sora Nexus default)
  min_bond_amount = "150"              # exact Quantity of voting_asset_id
  bond_escrow_account = "<i105-account-id>"
  slash_receiver_account = "<i105-account-id>"
  slash_double_vote_bps = 0            # percentage (basis points) to slash on double-vote attempts
  slash_invalid_proof_bps = 0          # percentage (basis points) to slash on invalid ballot proofs
  slash_ineligible_proof_bps = 0       # percentage (basis points) to slash on stale/invalid eligibility proofs
  parliament_term_blocks = 43200
  citizenship_asset_id = "79jULkZVMgnbzxBe6NvqeDxVEeEk"
  citizenship_bond_amount = "10000"    # exact Quantity of citizenship_asset_id
```

Governance monetary parameters are canonical non-negative `Quantity` values. TOML
uses their exact decimal string form (for example `"150"` or `"0.5"`), so the
configured asset precision is explicit and no host integer width or implicit
"smallest unit" convention is involved.

Sora Nexus default: ballots lock `min_bond_amount` of `voting_asset_id` into the
configured escrow account. Locks are created or extended when ballots land and
released on expiry; bond lifecycle is emitted via `governance_bond_events_total`
telemetry (lock_created|lock_extended|lock_unlocked|lock_slashed|lock_restituted).

`parliament_term_blocks` defines the epoch length used by explicit governance
selection and persistence workflows (`epoch = floor(height / term_blocks)`).
Bonded-citizen eligibility is consulted only when such a workflow performs a
selection; `GET /v1/gov/council/current` never uses it as an implicit fallback.
Extra bond above `citizenship_bond_amount` does not add draw tickets or vote
weight.

Governance VK verification has no bypass: ballot verification always requires an `Active` verifying key with inline bytes, and environments must not rely on test-only toggles to skip verification.

RBAC
- On-chain execution requires permissions:
  - Proposals: `CanProposeContractDeployment{ contract_address }`
  - Runtime-upgrade proposals: `CanProposeRuntimeUpgrade{ abi_version, abi_hash }`
  - Ballots: `CanSubmitGovernanceBallot{ referendum_id }`
  - Enactment: `CanEnactGovernance`
  - Slashing/appeals: `CanSlashGovernanceLock{ referendum_id }`, `CanRestituteGovernanceLock{ referendum_id }`
  - Citizen service outcomes: `CanRecordCitizenService{ owner }`
  - Council management: `CanManageParliament`
- Scoped governance capabilities are bootstrapped by genesis and thereafter
  delegable only by an existing holder of the exact same scope. In particular,
  direct native ISIs require the exact encoded target (not only the permission
  name), and `CanEnactGovernance` is not a grant root for runtime-upgrade
  proposal scopes.
- Enactment requires the exact unit token `CanEnactGovernance` before any
  proposal lookup or state mutation; a same-name permission with a non-unit
  payload is not equivalent. `FinalizeReferendum` is deliberately
  permissionless because it only derives a deterministic result from existing
  authenticated proposal, referendum, ballot, and Parliament records. It does
  not confer enactment authority.
- The fail-safe Initial executor admits the public native proposal, ballot,
  slashing, restitution, and citizen-service instructions only because Core
  enforces those exact scopes before mutation. The lower-level
  `zk::SubmitBallot` vendor instruction is not part of that signed native
  surface: an IVM host must first consume the one-shot
  `ZK_VOTE_VERIFY_BALLOT` latch before enqueueing it, and Core rechecks its exact
  ballot scope as defense in depth.
- Slashing/appeals:
  - Double-vote/invalid/ineligible ballots apply configured slash percentages against the bond escrow, moving funds into `slash_receiver_account`, updating the slashing ledger, and emitting typed `LockSlashed` events (reason + destination + note).
  - Manual `SlashGovernanceLock`/`RestituteGovernanceLock` instructions support operator-driven penalties and appeals; restitution is capped by recorded slashes, restores funds to the bond escrow, updates the ledger, and emits `LockRestituted` while keeping the lock active until expiry.

Protected Namespaces
- Custom parameter `gov_protected_namespaces` (JSON array of strings) enables admission gating for deploys into listed namespaces.
- Each namespace is an exact non-empty printable-ASCII token (`[!-~]+`). Torii
  rejects whitespace, control characters, non-ASCII text, and unknown request
  fields; it never trims or silently drops an entry.
- Clients must include transaction metadata key `gov_contract_address` for deploys targeting protected namespaces.
- `gov_manifest_approvers`: optional JSON array of <i105-account-id> account IDs. When a lane manifest declares a quorum greater than one, admission requires the transaction authority plus the listed accounts to satisfy the manifest quorum.
- Telemetry exposes holistic admission counters via `governance_manifest_admission_total{result}` so operators can distinguish successful admits from `missing_manifest`, `non_<i105-account-id>_authority`, `quorum_rejected`, `protected_namespace_rejected`, and `runtime_hook_rejected` paths.
- Telemetry surfaces the enforcement path via `governance_manifest_quorum_total{outcome}` (values `satisfied` / `rejected`) so operators can audit missing approvals.
- Lanes enforce the namespace allowlist published in their manifests. Any transaction that sets `gov_contract_address` must resolve into a protected dataspace alias present in the manifest's `protected_namespaces` set. `RegisterSmartContractCode` submissions without this metadata are rejected when protection is enabled.
- Admission enforces that an Enacted governance proposal exists for the tuple `(contract_address, code_hash, abi_hash)`; otherwise validation fails with a NotPermitted error.

Runtime Upgrade Hooks
- Lane manifests may declare `hooks.runtime_upgrade` to gate runtime upgrade instructions (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Hook fields:
  - `allow` (bool, default `true`): when `false`, all runtime-upgrade instructions are rejected.
  - `require_metadata` (bool, default `false`): require the transaction metadata entry specified by `metadata_key`.
  - `metadata_key` (string): metadata name enforced by the hook. Defaults to `gov_upgrade_id` when metadata is required or an allowlist is present.
  - `allowed_ids` (array of strings): optional allowlist of metadata values (after trimming). Rejects when the provided value is not listed.
- When the hook is present, queue admission enforces the metadata policy before the transaction enters the queue. Missing metadata, blank values, or values outside the allowlist produce a deterministic `NotPermitted` error.
- Telemetry tracks enforcement outcomes via `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Transactions satisfying the hook must include metadata `gov_upgrade_id=<value>` (or the manifest-defined key) alongside any <i105-account-id> approvals required by the manifest quorum.

Convenience Endpoint
- POST `/v1/gov/protected-namespaces` — applies `gov_protected_namespaces` directly on the node.
  - Request: { "namespaces": ["apps", "system"] }
  - Response: { "ok": true, "applied": 1 }
  - Notes: The closed request accepts only `namespaces` and optional
    `authority`; it contains no signing secret. Intended for admin/testing and
    requires an API token if configured. For production, prefer submitting a
    signed transaction with `SetParameter(Custom)`.

CLI Helpers
- `iroha --output-format text app gov deploy audit --contract-address irohac1...`
  - Fetches the active binding for the governed contract address and cross-checks that:
    - Torii stores bytecode for the active `code_hash`, and its Blake2b-32 digest matches the `code_hash`.
    - The manifest stored under `/v1/contracts/code/{code_hash}` reports matching `code_hash` and `abi_hash` values.
    - An enacted governance proposal exists for `(contract_address, code_hash, abi_hash)` as derived by the same proposal-id hashing the node uses.
- `iroha app gov deploy meta --contract-address irohac1... [--approver <i105-account-id> --approver <i105-account-id>]`
  - Emits the JSON metadata skeleton used when submitting deployments into protected namespaces, including `gov_contract_address` and optional `gov_manifest_approvers` for satisfying manifest quorum rules.
- `iroha app gov vote --mode zk --referendum-id <id> --backend <tag> --envelope-b64 <b64> [--owner <i105-account-id> --nullifier <32-byte-hex> --amount <Quantity> --duration-blocks <u64> --direction <Aye|Nay|Abstain>]`
  - Submits the canonical flat ZK V1 envelope request. It validates canonical
    I105 account ids, canonicalizes 32-byte nullifier hints, and merges the
    closed optional hint set from `--public <path>` into the request.
  - The nullifier is derived from the proof commitment (public input) plus `domain_tag`, exact `network_id`, and `election_id`; `--nullifier` is validated against the proof when supplied.
  - The one-line summary now surfaces a deterministic `fingerprint=<hex>` derived from the encoded `CastZkBallot` along with any decoded hints (`owner`, `amount`, `duration_blocks`, `direction` when provided).
  - CLI responses annotate `tx_instructions[]` with `payload_fingerprint_hex` plus decoded fields so downstream tooling can verify the skeleton without reimplementing Norito decoding.
  - When any lock hint is provided, ZK ballots must supply `owner`, `amount`, and `duration_blocks`; partial hints are rejected. When `min_bond_amount > 0`, lock hints are required. Direction remains optional and is treated as a hint only.
- `iroha app gov vote --mode plain --referendum-id <id> --owner <i105-account-id> --amount <Quantity> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--owner` accepts canonical I105 literals; Pass domain context through the surrounding scoped interface when required.
  - Summary output mirrors `vote --mode zk` by including the encoded instruction fingerprint and human-readable ballot fields (`owner`, `amount`, `duration_blocks`, `direction`), providing quick confirmation before signing the skeleton.

Governed Contract Lookup
- GET `/v1/gov/contracts/{contract_address}` — returns the active governance binding for a canonical contract address.
  - Response: { "found": bool, "contract_address": "irohac1...", "dataspace": "universal", "code_hash_hex": "…" ? }

Unlock Sweep (Operator/Audit)
- GET `/v1/gov/unlocks/stats`
  - Response: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notes: `height_current` is the committed ledger height captured atomically with the persisted audit cells; `last_sweep_height` is the most recent successful non-empty due-lock sweep, while the bounded count fields are the persisted result of the most recent attempted due-lock sweep (or zero before any attempt), and this endpoint never scans lock history.
- POST `/v1/gov/ballots/zk-v1`
  - Request (v1-style DTO):
    {
      "authority": "<i105-account-id>",
      "network_id": "hash:<64-uppercase-hex>#<CRC16>",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x…64hex?",
      "owner": "i105…",          // canonical AccountId (domainless encoded literal; no @domain suffix)
      "amount": "100?",
      "duration_blocks": 6000?,
      "direction": "Aye|Nay|Abstain?",
      "nullifier": "blake2b32:…64hex?"
    }
  - Response: { "ok": true, "accepted": true, "tx_instructions": [{…}] }
  - Notes:
    - `network_id` is the mandatory typed canonical hash of the genesis header.
      `authority`, `election_id`, and `backend` are exact non-empty tokens;
      whitespace/control variants are rejected rather than trimmed.
      `envelope_b64` must be canonical, non-empty standard base64.
    - The bounded raw request is exact-network account-authenticated before DTO
      decoding; the authenticated account must equal `authority`. Legacy
      `chain_id`/`genesis_hash` keys and label-based signatures are rejected.
    - `amount` is an exact canonical non-negative Kotodama V1 `Quantity`
      string. Fractional values through scale 28 are supported; JSON numbers,
      signed/trimmed spellings, leading zeroes, and redundant fractional zeroes
      are rejected. `duration_blocks` spans the complete `u64` domain.
    - When any lock hint is provided, the ballot must supply `owner`,
      `amount`, and `duration_blocks`; partial hints are rejected. Unknown
      fields and private-key aliases fail before a draft is constructed.
    - ZK re-votes are monotonic: attempts to shrink amount or expiry are
      rejected with `BallotRejected` diagnostics.
    - Contract execution must call `ZK_VOTE_VERIFY_BALLOT` before enqueuing
      `SubmitBallot`; hosts enforce a one-shot latch.

- POST `/v1/gov/ballots/zk-v1/ballot-proof`
  - Accepts a `BallotProof` JSON directly and returns a `CastZkBallot` skeleton.
  - Request:
    {
      "authority": "<i105-account-id>",
      "network_id": "hash:<64-uppercase-hex>#<CRC16>",
      "election_id": "ref-1",
      "ballot": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=",   // base64 of ZK1 or H2* container
        "root_hint": null,                // optional 32-byte hex string (eligibility root)
        "owner": null,                    // optional canonical AccountId (domainless encoded literal; no @domain suffix)
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
    - The strict request has no private-key field; Torii returns only an
      unsigned instruction skeleton for local signing.
    - The server maps optional `root_hint`/`owner`/`amount`/`duration_blocks`/`direction`/`nullifier` from the ballot to `public_inputs_json` for `CastZkBallot`.
    - The envelope bytes are re-encoded as base64 for the instruction payload.
    - This endpoint is part of every V1 app API build.

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

Consensus emits Norito-encoded `Evidence` whenever a <i105-account-id> violates the protocol. Each payload lands in the in-memory `EvidenceStore` and, if unseen, is materialised into the WSV-backed `consensus_evidence` map. Records older than `sumeragi.npos.reconfig.evidence_horizon_blocks` (default `7200` blocks) are rejected so the archive remains bounded, but the rejection is logged for operators. Evidence within the horizon obeys the joint-consensus staging rule (`mode_activation_height requires next_mode to be set in the same block`), the activation delay (`sumeragi.npos.reconfig.activation_lag_blocks`, default `1`), and the slashing delay (`sumeragi.npos.reconfig.slashing_delay_blocks`, default `259200`) so governance can cancel penalties before they apply.

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

- **DoublePrepare/DoubleCommit** — the <i105-account-id> signed conflicting hashes for the same `(phase,height,view,epoch)` tuple.
- **InvalidQc** — an aggregator gossiped a commit QC whose shape fails deterministic checks (e.g., empty signer bitmap).
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
4. **Schedule the follow-up topology** so the offending <i105-account-id> cannot immediately rejoin. Typical flows queue `SetParameter(Sumeragi::NextMode)` and `SetParameter(Sumeragi::ModeActivationHeight)` with the updated roster.
5. **Audit results** via `/v1/sumeragi/evidence` and `/v1/sumeragi/status` to ensure the evidence counter advanced and governance enacted the removal.

### Joint-Consensus Sequencing

Joint consensus guarantees that the outgoing <i105-account-id> set finalises the boundary block before the new set starts proposing. The runtime enforces the rule via paired parameters:

- `SumeragiParameter::NextMode` and `SumeragiParameter::ModeActivationHeight` must be committed in the **same block**. `mode_activation_height` must be strictly greater than the block height that carried the update, providing at least one-block lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (default `1`) is the configuration guard that prevents zero-lag hand-offs:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (default `259200`) delays consensus slashing so governance can cancel penalties before they apply.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- The runtime and CLI expose staged parameters through `/v1/sumeragi/params` and `iroha sumeragi params --summary`, so operators can confirm activation heights and <i105-account-id> rosters.
- Governance automation should always:
  1. Finalise the evidence-backed removal (or reinstatement) decision.
  2. Queue a follow-up reconfiguration with `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitor `/v1/sumeragi/status` until `effective_consensus_mode` flips at the expected height.

Any script that rotates <i105-account-id>s or applies slashing **must not** attempt zero-lag activation or omit the hand-off parameters; such transactions are rejected and leave the network in the previous mode.

## Telemetry surfaces

- Prometheus metrics export governance activity:
  - `governance_proposals_status{status}` (gauge) tracks proposal counts by status.
  - `governance_protected_namespace_total{outcome}` (counter) increments when protected namespace admission allows or rejects a deploy.
  - `governance_manifest_activations_total{event}` (counter) records manifest insertions (`event="manifest_inserted"`) and namespace bindings (`event="instance_bound"`).
- `/status` includes a `governance` object mirroring the proposal counts, reporting protected namespace totals, and listing recent manifest activations (namespace, contract id, code/ABI hash, block height, activation timestamp). Operators can poll this field to confirm that enactments updated manifests and that protected namespace gates are enforced.
- A Grafana template (`specs/grafana_governance_constraints.json`) and the
  telemetry runbook in `telemetry.md` show how to wire alarms for stuck
  proposals, missing manifest activations, or unexpected protected-namespace
  rejections during runtime upgrades.
