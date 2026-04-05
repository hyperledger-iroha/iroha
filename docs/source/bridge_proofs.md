# Bridge proofs

Bridge proof submissions travel through the standard instruction path (`SubmitBridgeProof`) and land in the proof registry with a verified status. The current surface covers ICS-style Merkle proofs and transparent-ZK payloads with pinned retention and manifest binding.

Torii now exposes three live SCCP bundle families:

- `burn` bundles for the legacy fixed-width burn message path
- `governance` bundles for token add/pause/resume governance actions
- `message` bundles for the generic multi-chain SCCP payload family (`asset_register`, `route_activate`, `transfer`)

## Acceptance rules

- Ranges must be ordered/non-empty and respect `zk.bridge_proof_max_range_len` (0 disables the cap).
- Optional height windows reject stale/future proofs: `zk.bridge_proof_max_past_age_blocks` and `zk.bridge_proof_max_future_drift_blocks` are measured against the block height that ingests the proof (0 disables the guardrails).
- Bridge proofs may not overlap an existing proof for the same backend (pinned proofs are preserved and block overlaps).
- Manifest hashes must be non-zero; payloads are size-capped by `zk.max_proof_size_bytes`.
- ICS payloads honour the configured Merkle depth cap and verify the path using the declared hash function.
- Transparent payloads must declare a non-empty backend label.
- Transparent payloads under the SCCP `sccp/stark-fri-v1/*` family must now decode as a typed SCCP message proof artifact, not an opaque byte blob.
- Typed SCCP message artifacts now validate `proof_bytes` as a real
  Norito-encoded `fastpq_prover::Proof`.
- SCCP transparent-proof verification reconstructs the canonical SCCP statement
  batch from the embedded bundle plus the shared manifest table and replays
  `fastpq_prover::verify(...)` against that batch.
- Legacy 32-byte placeholder digests are no longer accepted.
- Pinned proofs are exempt from retention pruning; unpinned proofs still respect the global `zk.proof_history_cap`/grace/batch settings.

## Torii API surface

- `GET /v1/sccp/capabilities` returns the relayer-facing SCCP capability snapshot:
  - local hub domain/chain identity (`SORA`);
  - legacy burn/governance registry backends;
  - the generic message proof family (`stark-fri-v1`);
  - the typed SCCP message proof-artifact discovery path (`/v1/sccp/artifacts/message/{message_id}`);
  - the normalized SCCP counterparty proof-job discovery path (`/v1/sccp/jobs/message/{message_id}`);
  - the SCCP proof-manifest discovery path (`/v1/sccp/manifests`);
  - supported codec ids/keys; and
  - the per-counterparty generic message backends / registry backends for `eth`,
    `bsc`, `sol`, `ton`, `tron`, `sora2`, `sora-kusama`, and
    `sora-polkadot`.
  - client helpers now exist for this route directly:
    - Rust: `iroha::client::Client::get_sccp_capabilities_json(...)` and `get_sccp_capabilities(...)`;
    - JavaScript: `ToriiClient.getSccpCapabilities(...)`; and
    - Python: `ToriiClient.get_sccp_capabilities()`.
- `GET /v1/sccp/manifests` returns the typed SCCP proof manifests for the same
  counterparty set. Each manifest binds together:
  - the chain key and counterparty domain id;
  - the chain-specific message backend / registry backend pair;
  - the canonical counterparty account codec;
  - the intended verifier target (`EVM`, `Solana`, `TON`, `TRON`, or
    Substrate-style runtime);
  - the finality model label used by the proof worker; and
  - the manifest seed used to derive the bridge proof manifest hash, plus the
    required SCCP public inputs (`message_id`, `payload_hash`, `target_domain`,
    `commitment_root`, `finality_height`, `finality_block_hash`).
  - each manifest now also carries a chain-specific `submission_template`
    describing the expected verifier entrypoint, envelope encoding, submission
    kind, and required argument keys for relayers targeting that chain.
  - client helpers now exist for this route directly:
    - Rust: `iroha::client::Client::get_sccp_proof_manifests_json(...)` and `get_sccp_proof_manifests(...)`;
    - JavaScript: `ToriiClient.getSccpProofManifests(...)`; and
    - Python: `ToriiClient.get_sccp_proof_manifests()`.
- `GET /v1/sccp/proofs/burn/{message_id}`, `GET /v1/sccp/proofs/governance/{message_id}`, and `GET /v1/sccp/proofs/message/{message_id}` return the live SCCP bundle keyed by canonical message id. The generic `message` route remains the raw bundle/debug fetch surface for multi-chain SCCP transfer and registry traffic.
- `GET /v1/sccp/artifacts/message/{message_id}` returns the typed SCCP transparent proof artifact for the same canonical message id. Each artifact now bundles:
  - the chain-specific `message_backend` / `registry_backend`;
  - the finality model and verifier target derived from the shared manifest table;
  - the canonical public inputs (`message_id`, `payload_hash`, `target_domain`, `commitment_root`, `finality_height`, `finality_block_hash`);
  - `proof_bytes` containing a real Norito-encoded FASTPQ STARK-FRI proof over
    the canonical SCCP statement batch derived from the bundle and manifest;
    and
  - the embedded Nexus SCCP message bundle so verifiers can reconstruct the exact statement being proven.
  - `iroha_sccp` now also exposes a normalized counterparty proof-job projection over that artifact:
    - `decode_sccp_normalized_codec_value(...)` decodes codec-bearing SCCP fields into typed EVM / Solana / TON / Tron / logical-text values; and
    - `build_sccp_counterparty_proof_job_from_artifact(...)` / `build_sccp_counterparty_proof_job_from_bundle(...)` produce a prover-oriented job with the normalized payload projection plus the original typed bundle.
  - client helpers now exist for that route directly:
    - Rust: `iroha::client::Client::get_sccp_message_proof_artifact_json(...)` and `get_sccp_message_proof_artifact(...)`;
    - Python: `ToriiClient.get_sccp_message_proof_artifact(...)`; and
    - JavaScript: `ToriiClient.getSccpMessageProofArtifact(...)`.
- `GET /v1/sccp/jobs/message/{message_id}` returns the normalized SCCP counterparty proof job for the same canonical message id. Each job bundles:
  - the chain family, chain key, backend labels, manifest seed, finality model, verifier target, and canonical SCCP public inputs;
  - a normalized payload projection with typed codec values for EVM / Solana / TON / Tron / logical-text surfaces; and
  - the same chain-specific `submission_template` advertised by the manifest, so prover/relayer workers can derive the target verifier entrypoint and argument list without hard-coding per-chain packaging; and
  - the original typed Nexus SCCP message bundle so a prover worker can keep both the normalized view and the canonical committed preimage in one document.
  - client helpers now exist for that route directly:
    - Rust: `iroha::client::Client::get_sccp_message_proof_job_json(...)` and `get_sccp_message_proof_job(...)`;
    - Python: `ToriiClient.get_sccp_message_proof_job(...)`; and
    - JavaScript: `ToriiClient.getSccpMessageProofJob(...)`.
- `GET /v1/sccp/proofs/message/{message_id}` now reconstructs the proof from committed blocks that contain `RecordSccpMessage` instructions and a non-null `sccp_commitment_root` in the finalized block header. The temporary in-memory bundle registry remains only as a fallback/test path.
- Generic SCCP `message` payloads now enforce explicit v1 codec families during structural verification instead of accepting arbitrary nonzero codec ids:
  - `1`: generic UTF-8 logical identifiers;
  - `2`: EVM `0x`-prefixed 20-byte hex addresses;
  - `3`: Solana base58 public keys;
  - `4`: TON raw `workchain:account_hex` addresses; and
  - `5`: Tron base58check account addresses.
- `POST /v1/bridge/proofs/submit` accepts exactly one of `burn_bundle`, `governance_bundle`, or `message_bundle`. `message_bundle` is converted into a typed SCCP transparent proof artifact and then wrapped in a bridge proof with backend label `bridge/sccp/stark-fri-v1/<chain>`.
- `POST /v1/bridge/proofs/submit` now derives chain-specific SCCP transparent backends for generic `message` bundles:
  - outbound `SORA -> ETH` and inbound `ETH -> SORA` messages use `bridge/sccp/stark-fri-v1/eth`;
  - the same pattern applies to `bsc`, `sol`, `ton`, `tron`, `sora2`, `sora-kusama`, and `sora-polkadot`;
  - the bridge proof manifest hash is derived from the same domain suffix, so proof IDs and registry queries split cleanly by counterparty chain instead of collapsing all SCCP traffic into one generic backend bucket.
- `POST /v1/bridge/proofs/submit` and `POST /v1/bridge/messages` now also return normalized SCCP counterparty metadata in the response:
  - `counterparty_domain` is the numeric SCCP domain id; and
  - `counterparty_chain` is the canonical domain key (`eth`, `bsc`, `sol`, `ton`, `tron`, `sora2`, etc.).
- `GET /v1/zk/proof/{backend}/{hash}` and `GET /v1/zk/proofs` now mirror that metadata inside `bridge.payload` for SCCP transparent proofs when the backend matches the chain-split SCCP family.
  - when the stored payload decodes as a typed SCCP artifact, the bridge summary now also exposes `message_id`, `payload_hash`, `target_domain`, `commitment_root`, `finality_height`, `finality_block_hash`, and `proof_artifact_len_bytes`.
  - the bridge summary additionally exposes `inner_chain_family`,
    `inner_payload_kind`, and `inner_statement_hash`, derived from the
    canonical SCCP statement context rather than from an embedded placeholder
    envelope.
- `POST /v1/bridge/messages` accepts an inbound `message_bundle` targeted at SORA, records the corresponding transparent-ZK bridge proof, and emits a typed `BridgeReceipt` for `transfer` payloads.
- `POST /v1/bridge/messages` now also accepts an optional `settlement` object:
  - it resolves a deployed contract target by `contract_address` or `contract_alias`;
  - it appends an ephemeral by-call trigger after proof verification so settlement can happen in the same submitted transaction; and
  - when `payload` is omitted for a parameterized entrypoint, Torii auto-builds the `finalize_inbound(route, message_id, recipient, amount)` payload for `transfer` messages using the SCCP bundle plus the provided local `route`;
  - when the SCCP transfer `route_id` already uses the logical-name codec (`route_id_codec == 1`), the local `route` can be omitted and Torii derives it directly from the bundle.
- Automatic settlement is still opt-in per request. Cross-node policy for always-on contract dispatch remains a higher-level integration choice outside this endpoint.
- The CLI now exposes read-only SCCP discovery helpers under the bridge feature:
  - `iroha ops bridge sccp capabilities`
  - `iroha ops bridge sccp manifests`
  - `iroha ops bridge sccp artifact --message-id <hex>`
  - `iroha ops bridge sccp job --message-id <hex>`
  - text mode prints compact chain/proof summaries, and `artifact` / `job` now also decode the normalized payload projection plus the chain-specific submission template when they are present;
  - JSON mode emits the raw typed payload/JSON route response.

- `GET /v1/zk/proofs` and `GET /v1/zk/proofs/count` accept bridge-aware filters:
  - `bridge_only=true` returns only bridge proofs.
  - `bridge_pinned_only=true` narrows to pinned bridge proofs.
  - `bridge_start_from_height` / `bridge_end_until_height` clamp the bridge range window.
- `GET /v1/zk/proof/{backend}/{hash}` returns bridge metadata (range, manifest hash, payload summary) alongside the proof id/status/VK bindings.
- The full Norito proof record (including payload bytes) remains available via `GET /v1/proofs/{proof_id}` for off-node verifiers.

## Bridge receipt events

Bridge lanes emit typed receipts via the `RecordBridgeReceipt` instruction. Executing this instruction
records a `BridgeReceipt` payload and emits `DataEvent::Bridge(BridgeEvent::Emitted)` on the event
stream, replacing the prior log-only stub. The CLI `iroha bridge emit-receipt` helper submits the
typed instruction so indexers can consume receipts deterministically.

Outbound SCCP traffic is recorded separately through `RecordSccpMessage`. The instruction carries
canonical SCCP payload bytes, is structurally validated during execution, and is used by proposal
assembly to derive the block-level `sccp_commitment_root`.

## External verification sketch (ICS)

```rust
use iroha_data_model::bridge::{BridgeHashFunction, BridgeProofPayload, BridgeProofRecord};
use iroha_crypto::{Hash, HashOf, MerkleTree};

fn verify_ics(record: &BridgeProofRecord) -> bool {
    let BridgeProofPayload::Ics(ics) = &record.proof.payload else {
        return false;
    };
    let leaf = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(ics.leaf_hash));
    let root =
        HashOf::<MerkleTree<[u8; 32]>>::from_untyped_unchecked(Hash::prehashed(ics.state_root));
    match ics.hash_function {
        BridgeHashFunction::Sha256 => ics.proof.clone().verify_sha256(&leaf, &root, ics.proof.audit_path().len()),
        BridgeHashFunction::Blake2b => ics.proof.clone().verify(&leaf, &root, ics.proof.audit_path().len()),
    }
}
```
