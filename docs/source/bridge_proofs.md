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
- ICS payloads honour the configured Merkle depth cap and verify the path using the declared hash function; transparent payloads must declare a non-empty backend label.
- Pinned proofs are exempt from retention pruning; unpinned proofs still respect the global `zk.proof_history_cap`/grace/batch settings.

## Torii API surface

- `GET /v1/sccp/proofs/burn/{message_id}`, `GET /v1/sccp/proofs/governance/{message_id}`, and `GET /v1/sccp/proofs/message/{message_id}` return the live SCCP bundle keyed by canonical message id. The generic `message` route is the canonical fetch surface for multi-chain SCCP transfer and registry traffic.
- `GET /v1/sccp/proofs/message/{message_id}` now reconstructs the proof from committed blocks that contain `RecordSccpMessage` instructions and a non-null `sccp_commitment_root` in the finalized block header. The temporary in-memory bundle registry remains only as a fallback/test path.
- `POST /v1/bridge/proofs/submit` accepts exactly one of `burn_bundle`, `governance_bundle`, or `message_bundle`. `message_bundle` is converted into a transparent-ZK bridge proof with backend label `bridge/sccp/stark-fri-v1`.
- `POST /v1/bridge/messages` accepts an inbound `message_bundle` targeted at SORA, records the corresponding transparent-ZK bridge proof, and emits a typed `BridgeReceipt` for `transfer` payloads. The current settlement path stops at proof persistence plus receipt emission; direct contract dispatch remains a TODO.

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
