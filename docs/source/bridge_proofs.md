# Bridge proofs

Bridge proof submissions travel through the standard instruction path (`SubmitBridgeProof`) and land in the proof registry with a verified status. The current surface covers ICS-style Merkle proofs and transparent-ZK payloads with pinned retention and manifest binding.

Torii now exposes two SCCP bundle families:

- `burn` bundles for the legacy fixed-width burn message path
- `message` bundles for the generic multi-chain SCCP payload family
  (`asset_register`, `route_activate`, `transfer`, `token_add`, `token_pause`,
  `token_resume`)

## Human relay model

SCCP relay to the SORA2 `sccp-bridge` pallet is a manual operator flow. The
production path does not assume an off-chain worker, node-side daemon, or
automated relayer service. A human relay operator uses a bridge web interface to
review a Nexus/Iroha SCCP message, fetch the pallet-ready proof envelope, and
sign the corresponding SORA2 extrinsic through a wallet.

The relay operator is only a courier and transaction fee payer. Authorization
comes from source-chain finality, Nexus commitment binding, and the
cryptographic proof artifacts checked by the destination verifier. Parliament
may govern channel configuration, but it does not approve bridge transactions.
A malformed or unauthorized relay transaction is expected to be rejected
on-chain.

The bridge UI should perform the following checks before preparing a wallet
transaction:

- fetch `/v1/sccp/capabilities` and confirm `runtime_proof_family =
  runtime-scale-v1` and `runtime_verifier_backend = sora-nexus-runtime-v1`;
- fetch the human-readable JSON bundle for the selected message and display the
  payload, message id, commitment root, finality epoch, and finality height;
- fetch the matching runtime SCALE envelope from
  `/v1/sccp/proofs/message/{message_id}/runtime-scale`;
- check that SORA2 already has the required `TrustedNexusFinalityAnchors` and
  destination verifier/trust-anchor configuration; and
- prepare the correct SORA2 call for wallet signing:
  `submit_message_proof`, `submit_token_add_proof`, `submit_token_pause_proof`,
  or `submit_token_resume_proof`.

For the runtime SCALE path, the SORA2 call uses `proof_family =
runtime-scale-v1`, `verifier_backend = sora-nexus-runtime-v1`, and
`bundle_bytes` equal to the raw response body from the `/runtime-scale` endpoint.
`proof_bytes` and `public_inputs` are retained for non-runtime verifier backends
and may be empty for this runtime envelope path.

## Acceptance rules

- Ranges must be ordered/non-empty and respect `zk.bridge_proof_max_range_len` (0 disables the cap).
- Optional height windows reject stale/future proofs: `zk.bridge_proof_max_past_age_blocks` and `zk.bridge_proof_max_future_drift_blocks` are measured against the block height that ingests the proof (0 disables the guardrails).
- Bridge proofs may not overlap an existing proof for the same backend (pinned proofs are preserved and block overlaps).
- Manifest hashes must be non-zero; payloads are size-capped by `zk.max_proof_size_bytes`.
- ICS payloads honour the configured Merkle depth cap and verify the path using the declared hash function.
- Transparent payloads must declare a non-empty backend label.
- Transparent payloads under the SCCP `sccp/stark-fri-v1/*` family must now decode as a typed SCCP message proof artifact, not an opaque byte blob.
- Typed SCCP message artifacts now validate `proof_bytes` as a real
  Norito-encoded `OpenVerifyEnvelope` whose inner payload is the canonical
  SCCP FASTPQ proof and public-input column wrapper.
- SCCP transparent-proof verification reconstructs the canonical SCCP statement
  batch from the embedded bundle plus the shared manifest table, checks the
  `OpenVerifyEnvelope` metadata (`circuit_id`, schema descriptor, verifier
  commitment, and wrapped public inputs), and then replays
  `fastpq_prover::verify(...)` against that batch.
- Legacy 32-byte placeholder digests are no longer accepted.
- Raw SCCP message bundle bytes are not accepted as transparent bridge-proof
  payloads; Torii/core validation requires the typed artifact and replays the
  embedded cryptographic proof.
- Pinned proofs are exempt from retention pruning; unpinned proofs still respect the global `zk.proof_history_cap`/grace/batch settings.

## SCCP message proof formats

`NexusSccpMessageProofV1.finality_proof` is direction-sensitive:

- SORA-origin messages (`SORA -> remote`) carry a Norito-encoded
  `NexusBridgeFinalityProofV1`. The verifier checks the Nexus commit QC,
  commitment root, target-domain commitment, payload hash, message id, and
  Merkle root.
- non-SORA source messages (`ETH/BSC/Solana/TON/TRON/Substrate-family ->
  SORA/Nexus or another supported target`) carry a Norito-encoded
  `SccpSourceChainProofEnvelopeV1`. Raw Nexus finality bytes are rejected for
  these messages, and source-chain envelopes are rejected for SORA-origin
  messages.

`SccpSourceChainProofEnvelopeV1` has this canonical data shape:

- `version = 1`
- `source_domain` and `target_domain` as SCCP numeric domain ids
- `source_chain` as the canonical chain key (`eth`, `bsc`, `sol`, `ton`,
  `tron`, `sora2`, `sora-kusama`, or `sora-polkadot`)
- `source_proof_plan`:
  `EthereumBeaconReceiptProof`, `BscValidatorSetReceiptProof`,
  `SolanaFinalizedTransactionProof`, `TonMasterchainShardProof`,
  `TronDposReceiptProof`, or `SubstrateGrandpaEventProof`
- `finality_model` matching the source domain
- `message_id`, `payload_hash`, and `commitment_root` from the SCCP hub
  commitment
- `source_event_digest =
  blake2b256("sccp:source:event:v1" || 1 || source_domain ||
  target_domain || message_id || payload_hash)`, with integer fields encoded in
  little-endian order
- `finality_height`, `finality_block_hash`, `finalized_header_hash`, and
  `receipt_or_message_root`
- `consensus_proof`, a Norito-encoded `SccpSourceConsensusProofV1`
- `message_inclusion_proof`, a Norito-encoded
  `SccpSourceMessageInclusionProofV1`
- non-empty `inclusion_branch` entries, each exactly 32 bytes

`SccpSourceConsensusProofV1` binds the source domain, chain key, proof plan,
finality model, finality height, finality block hash, receipt/message root,
finalized header hash, a plan-specific `SccpSourceAdapterProofV1`,
`adapter_transcript_hash`, `SccpSourceVerifierEvidenceV1`, and
`SccpSourceAdapterVerificationProofV1`, which wraps a STARK/FastPQ
`OpenVerifyEnvelope` for the canonical source-adapter statement. The finalized
header hash is recomputed as
`blake2b256("sccp:source:header:v1" || 1 || source_domain || finality_model ||
finality_height || finality_block_hash || receipt_or_message_root)`, with
integer fields encoded in little-endian order.

`SccpSourceVerifierEvidenceV1` is the explicit verifier/trust-anchor binding
for the adapter transcript. It carries:

- `version = 1`
- `source_domain`, `source_chain`, `source_proof_plan`, and `finality_model`
- `adapter_proof_hash =
  blake2b256("sccp:source-adapter-proof:v1" || canonical_adapter_proof)`
- `adapter_transcript_hash`
- `adapter_circuit_id = "sccp-source-adapter-v1"`
- `source_trust_anchor_id` and `source_trust_anchor_hash`
- `consensus_verifier_id` and `consensus_verifier_hash`
- `message_inclusion_verifier_id` and `message_inclusion_verifier_hash`
- `finality_policy_id` and `finality_policy_hash`

The evidence record is canonicalized and hashed as
`blake2b256("sccp:source-verifier-evidence:v1" ||
canonical_verifier_evidence)`. The structural verifier recomputes the expected
evidence for the source domain, chain key, proof plan, finality model,
adapter proof hash, adapter transcript, and adapter circuit id. Any zero hash,
empty id, domain replay, circuit replay, stale adapter hash, stale transcript,
or mismatched finality policy is rejected.

The IDs and hashes used by the evidence record come from
`SccpSourceVerifierMaterialV1`. Today the built-in material catalog is
explicitly marked `placeholder_material = true`; those records are accepted
only for diagnostic source-proof artifacts and cannot make a lane
production-ready. A source material record becomes production-ready only when
it is non-placeholder, matches the source domain/chain/proof plan/finality
model/circuit id, and carries non-empty ids plus non-zero hashes for the source
trust anchor, consensus verifier, message-inclusion verifier, and finality
policy. Reusing any built-in placeholder id or digest keeps the record
non-production even if `placeholder_material` is set to `false`. This keeps the
source-adapter statement format stable while leaving the production gate closed
until real light-client anchors and immutable verifier code hashes are installed.
`iroha_sccp` also exposes explicit-material production helpers:
`build_sccp_source_verifier_evidence_with_material(...)`,
`build_sccp_source_adapter_verification_proof_with_material(...)`,
`verify_sccp_source_chain_proof_envelope_production_with_material(...)`, and
`verified_sccp_message_source_chain_proof_envelope_for_production_with_material(...)`.
These helpers let governance/config-sourced material replace the placeholder
catalog without changing the proof envelope. Node configuration now exposes
`zk.sccp_source_verifier_materials`; each entry is part of the ZK consensus
policy hash and is converted into `SccpSourceVerifierMaterialV1` at bridge proof
admission. A configured non-SORA lane opens only when exactly one material
record matches the source domain, all four digest fields are valid 32-byte hex
and non-zero, the record is non-placeholder, and the submitted source proof's
evidence and adapter OpenVerify statement bind to that material. Duplicate,
placeholder, malformed, wrong-domain, built-in-placeholder-reused, or replayed
verifier material fails closed. The default production verifier continues to use
the built-in catalog and therefore remains closed when no explicit material is
configured.

Solana proof generation is a user-side workflow. Web portal and mobile SDKs
construct a canonical local proof request from UI/RPC-collected witness fields
(`finalized_slot`, `blockhash`, `bank_hash`, `transaction_status_root`,
`message_proof_hash`, transaction signature, emitter program id, `message_id`,
`payload_hash`, `commitment_root`, and `source_event_digest`). SDK helpers
derive `message_proof_hash` from
`blake2b256("sccp:solana:message-proof:v1" || 0x01 ||
source_event_digest || transaction_status_root || branch_len_le ||
inclusion_branch[0..n])`, hash the canonical witness, and wrap externally
generated proof bytes, but they do not fabricate proofs or infer the source
event digest. Applications must link the real local prover and submit the
resulting proof artifact on-chain. The Rust source-adapter verifier recomputes
the same Solana message-proof hash and rejects any adapter proof whose hash does
not bind to the source event digest, transaction-status root, and inclusion
branch carried by the envelope.

TON proof generation follows the same user-side model. The SDKs build the
canonical TON public inputs from UI/RPC-collected masterchain and shard witness
fields, derive a stable `query_id`, and package the externally generated proof
as a TON message body BOC. The TON submission template is
`ton_message_body_boc_v1` and has one argument, `message_body_boc`, encoded as
`ton_boc`. The BOC root cell stores the SCCP operation code, schema version,
query id, destination binding hash, statement hash, proof hash, public-input
hash, bundle hash, and three snake-cell references containing proof bytes,
public inputs, and the SCCP bundle. Apps must link the real TON prover and send
the resulting internal message body to the configured verifier contract; nodes
do not synthesize TON proofs or destination messages.

`SccpSourceAdapterProofV1` is an enum whose variant must match
`source_proof_plan` and `source_domain`:

- `EthereumBeaconReceipt`: `source_domain`, `beacon_slot`,
  `execution_block_number`, `execution_block_hash`,
  `execution_receipts_root`, `beacon_finalized_root`,
  `sync_committee_root`, and `receipt_trie_proof_hash`.
- `BscValidatorSetReceipt`: `source_domain`, `validator_epoch`,
  `block_number`, `block_hash`, `receipts_root`, `validator_set_hash`,
  `commit_seal_hash`, and `receipt_trie_proof_hash`.
- `SolanaFinalizedTransaction`: `source_domain`, `finalized_slot`,
  `blockhash`, `bank_hash`, `transaction_status_root`, and
  `message_proof_hash`.
- `TonMasterchainShard`: `source_domain`, `masterchain_seqno`,
  `masterchain_block_hash`, `shard_block_hash`, `shard_state_root`,
  `transaction_root`, and `shard_proof_hash`.
- `TronDposReceipt`: `source_domain`, `solid_block_number`, `block_hash`,
  `witness_schedule_hash`, `receipt_root`, `transaction_root`, and
  `receipt_proof_hash`.
- `SubstrateGrandpaEvent`: `source_domain`, `finalized_block_number`,
  `grandpa_set_id`, `block_hash`, `authority_set_hash`, `events_root`, and
  `storage_proof_hash`.

The generic structural verifier does not claim to validate external consensus
by itself. It does require the adapter variant to match the source plan, the
variant's finality height/block hash/root fields to match the enclosing
envelope, all chain-specific witness hashes to be non-zero, and
`adapter_transcript_hash` to equal
`blake2b256("sccp:source-adapter-transcript:v1" || source_domain ||
target_domain || source_proof_plan || finality_model || finality_height ||
finality_block_hash || receipt_or_message_root || source_event_digest ||
len(canonical_adapter_proof) || canonical_adapter_proof)`. Integer fields and
the adapter-proof length prefix are little-endian. This prevents a generic
self-consistency blob, stale adapter proof, or witness-substituted adapter
proof from being replayed as a different source-chain proof shape while the
real adapters are still disabled.

`SccpSourceAdapterVerificationProofV1` has:

- `version = 1`
- `proof_family = "stark-fri-v1"`
- `circuit_id = "sccp-source-adapter-v1"`
- `proof_bytes`, a Norito-encoded `OpenVerifyEnvelope` whose backend is
  `Stark`

The OpenVerify envelope must use the same circuit id, the canonical FastPQ
parameter set `fastpq-lane-balanced`, an empty auxiliary payload, and public
input columns for `source_domain`, `target_domain`, `message_id`,
`payload_hash`, `source_event_digest`, `finality_height`,
`finality_block_hash`, `receipt_or_message_root`,
`adapter_transcript_hash`, and `source_verifier_evidence_hash`. The embedded
FastPQ proof is verified against a deterministic batch containing the
canonical adapter statement, canonical adapter proof bytes, and adapter context
bytes. The canonical adapter statement includes the verifier-evidence hash, so
stale OpenVerify metadata, wrong public inputs, unanchored verifier evidence,
or tampered FastPQ proof public IO fail structural verification.

Because the source-adapter capsule is embedded inside the SCCP transparent
bridge artifact, production configs must size bridge proof limits for
multi-proof SCCP artifacts. Taira sets `confidential.max_proof_size_bytes =
4194304` and `confidential.max_proof_bytes_block = 16777216`; smaller caps can
reject a valid SCCP artifact before source-adapter readiness is evaluated.

`SccpSourceMessageInclusionProofV1` binds the source/target domains, message
id, payload hash, source event digest, source event leaf hash,
receipt/message root, and leaf index. The source event leaf is recomputed as
`blake2b256("sccp:source:event-leaf:v1" || source_event_digest)`. The verifier
then folds `inclusion_branch` as a binary Merkle path using
`blake2b256("sccp:source:node:v1" || left || right)`, interpreting
`leaf_index` bits from least significant to most significant, and requires the
reconstructed root to equal `receipt_or_message_root`.

The structure gate rejects unsupported domains, source/target equality, a SORA
source in the source-chain envelope, chain-key/proof-plan/finality-model
mismatches, zero hashes or height, malformed typed proof blobs, bad 32-byte
branch shape, Merkle root mismatches, finalized-header/root mismatches, and a
bad `source_event_digest` or wrong plan-specific adapter proof. The bundle
binding gate then requires the envelope's
source domain, target domain, message id, payload hash, and commitment root to
match the embedded SCCP bundle exactly, preventing cross-lane, cross-target,
message-id, payload-hash, and commitment-root replay.

This envelope is the stable typed binding layer for source-chain adapters. The
generic verifier now consumes and cryptographically checks the typed proof blobs
instead of accepting arbitrary non-empty bytes. Production remains blocked until
inbound admission persists the real source-chain proof bytes and the
chain-specific adapters verify external consensus/finality and receipt or
message inclusion according to each source proof plan. The production verifier
has a separate source-adapter readiness gate: a structurally valid
`SccpSourceChainProofEnvelopeV1` is not sufficient for production admission
until the matching adapter is marked ready by the SCCP lane readiness table.
That readiness table now exposes `source_adapter_engine` separately from the
destination rollout. The adapter statement binding and FastPQ/OpenVerify
capsule can be marked ready while the lane still remains disabled until the
external consensus verifier, external receipt/message inclusion verifier, and
source-chain trust anchor are all active for the specific source domain.

On-chain `SubmitBridgeProof` validation is also direction-sensitive for SCCP
message proofs. SORA-origin bundles must expose a locally anchored
`NexusBridgeFinalityProofV1`, while non-SORA-origin bundles must expose a
verified `SccpSourceChainProofEnvelopeV1`; the runtime no longer tries to parse
external-source messages as Nexus finality. The non-SORA path is still gated by
lane production readiness until the source adapters perform real
chain-specific consensus and receipt/message inclusion verification.
Torii's local message-proof builder now refuses to synthesize non-SORA
source-chain envelopes from Iroha finality data; production callers must submit
the source-chain proof envelope produced by the source adapter.

`RecordSccpMessage` is only valid for SORA-origin payloads. Non-SORA source
messages are admitted through `POST /v1/bridge/messages` with their
source-chain proof envelope and bridge proof artifact; they are not reconstructed
as Nexus-origin messages from block-level SCCP records.

## Torii API surface

- `GET /v1/sccp/capabilities` returns the relay-operator-facing SCCP capability snapshot:
  - local hub domain/chain identity (`SORA`);
  - the SCCP burn registry backend;
  - the generic message proof family (`stark-fri-v1`);
  - the SORA2 runtime proof family (`runtime-scale-v1`) and verifier backend
    (`sora-nexus-runtime-v1`);
  - the runtime SCALE envelope paths used by the bridge UI for wallet
    submission:
    - `/v1/sccp/proofs/message/{message_id}/runtime-scale`
  - the typed SCCP message proof-artifact discovery path (`/v1/sccp/artifacts/message/{message_id}`);
  - the normalized SCCP counterparty proof-job discovery path (`/v1/sccp/jobs/message/{message_id}`);
  - the SCCP proof-manifest discovery path (`/v1/sccp/manifests`);
  - supported codec ids/keys; and
  - the per-counterparty generic message backends / registry backends for `eth`,
    `bsc`, `sol`, `ton`, `tron`, `sora2`, `sora-kusama`, and
    `sora-polkadot`.
  - the production launch policy: all advertised SCCP lanes must become ready
    together, proof submission is permissionless, routes are allowlisted by
    deployment-time governance, and per-message human approval is never part of
    verification.
  - every currently advertised lane is marked `production_ready = false` with a
    `disabled_reason` and production-readiness blockers until source-chain
    finality/inclusion verification, source trust anchors, immutable
    destination verifiers, cryptographic anchors, and route allowlists are all
    live. The nested `source_adapter_engine` object shows that the typed adapter
    statement binding and FastPQ/OpenVerify capsule are present, while the real
    external consensus, inclusion, and source-anchor engines still block
    production. The nested `destination_rollout` object is bound to the
    counterparty domain and chain key; production readiness rejects rollout
    records with the wrong domain, wrong chain, wrong verifier plan, missing or
    empty verifier identity, missing anchor id, non-hex/zero verifier code
    hash, or any remaining rollout blocker.
  - client helpers now exist for this route directly:
    - Rust: `iroha::client::Client::get_sccp_capabilities_json(...)` and `get_sccp_capabilities(...)`;
    - JavaScript: `ToriiClient.getSccpCapabilities(...)`; and
    - Python: `ToriiClient.get_sccp_capabilities()`.
- `GET /v1/sccp/manifests` returns the typed SCCP proof manifests for the same
  counterparty set. Each manifest binds together:
  - the chain key and counterparty domain id;
  - the target verifier backend key for that counterparty lane (`evm-groth16-bn254-v1`, `tron-groth16-bn254-v1`, `solana-program-v1`, `ton-contract-v1`, or `substrate-runtime-v1`);
  - the declared SCCP proof security model (`RecursiveZk`) and anchor mode (`CryptographicProof`);
  - a typed destination binding (`version`, `key`, `binding_hash`) that scopes proofs to the intended verifier deployment/runtime context for that lane;
  - the chain-specific message backend / registry backend pair;
  - the canonical counterparty account codec;
  - the intended verifier target (`EVM`, `Solana`, `TON`, `TRON`, or
    Substrate-style runtime);
  - the finality model label used by proof tooling; and
  - the manifest seed used to derive the bridge proof manifest hash, plus the
    required SCCP public inputs (`message_id`, `payload_hash`, `target_domain`,
    `commitment_root`, `finality_height`, `finality_block_hash`).
  - each manifest now also carries a chain-specific `submission_template`
    describing the expected verifier entrypoint, envelope encoding, submission
    kind, and required argument keys for relay tooling targeting that chain.
  - the reference EVM wrapper contracts for that template now live under
    `contracts/evm/sccp` in this repo.
  - ETH and BSC currently share the same reference EVM wrapper entrypoint:
    `submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)`.
  - for ETH/BSC, production manifests target the `evm-groth16-bn254-v1`
    immutable verifier adapter. `contracts/evm/sccp` now includes
    `SccpGroth16Bn254MessageVerifier`, which verifies ABI-encoded Groth16
    proof points against an immutable constructor-supplied BN254 verifying key
    and binds the proof to the six SCCP public-input words, source domain,
    statement hash, and destination binding hash. Constructor G1 points are
    `(x, y)`, constructor G2 points are `(x_0, x_1, y_0, y_1)`, and the
    flattened IC vector must contain exactly ten G1 points: the constant term
    plus one point for each of the nine SCCP public signals. `proof_bytes` for
    this backend must ABI-decode as `(uint256 version, bytes32 message_id,
    uint256 source_domain, bytes32 commitment_root, uint256[2] a, uint256[4]
    b, uint256[2] c)`, with `version = 1`. The nine public signals are
    `uint256(keccak256(abi.encode(label, value))) mod r`, reduced modulo the
    BN254 scalar field, for `message_id`, `payload_hash`, `target_domain`,
    `commitment_root`, `finality_height`, `finality_block_hash`,
    `source_domain`, `statement_hash`, and `destination_binding_hash`, in that
    exact order. The Rust submission-package builder has a signer-free
    `EvmGroth16ContractCall` path for this backend. It accepts only the exact
    ABI tuple above, rejects malformed length, wrong version, source-domain
    overflow, message-id/source-domain/commitment-root replay, zero or
    out-of-field proof points, signer-supplied production packages, and
    destination-binding metadata with the wrong version or zero hash. The
    legacy reference wrapper
    still uses an EVM-native secp256k1 attestation envelope over the native
    SCCP proof hash and canonical fixed-width public inputs. That envelope now
    also commits a `destination_binding_hash` derived from the wrapper address,
    immutable verifier address, verifier backend, proof family, network id, and
    the bound SCCP source/target domains so one attestation cannot be replayed
    across sibling deployments, rebound to a different verifier deployment, or
    reused for a different lane on the same network. The reference wrapper also
    enforces the configured source/target domains before accepting a proof, but
    the overall attestation path is still explicitly non-production and is not
    advertised as the production verifier backend.
  - TRON now advertises the `tron-groth16-bn254-v1` verifier backend and follows
    the same fixed-word BN254 verifier shape on the TVM side, while Solana, TON,
    and Substrate keep their own platform-native instruction/cell/call encodings.
  - client helpers now exist for this route directly:
    - Rust: `iroha::client::Client::get_sccp_proof_manifests_json(...)` and `get_sccp_proof_manifests(...)`;
    - JavaScript: `ToriiClient.getSccpProofManifests(...)`; and
    - Python: `ToriiClient.get_sccp_proof_manifests()`.
- `GET /v1/sccp/proofs/burn/{message_id}` and `GET /v1/sccp/proofs/message/{message_id}` return the live SCCP bundle keyed by canonical message id. The generic `message` route remains the raw bundle/debug fetch surface for multi-chain SCCP transfer, registry, and token-control traffic.
- `GET /v1/sccp/artifacts/message/{message_id}` returns the typed SCCP transparent proof artifact for the same canonical message id. Each artifact now bundles:
  - the target verifier backend metadata for the counterparty lane;
  - the chain-specific `message_backend` / `registry_backend`;
  - the shared SCCP security model / cryptographic anchor mode and the destination binding carried through from the manifest;
  - the finality model and verifier target derived from the shared manifest table;
  - the canonical public inputs (`message_id`, `payload_hash`, `target_domain`, `commitment_root`, `finality_height`, `finality_block_hash`);
  - `proof_bytes` containing a real Norito-encoded `OpenVerifyEnvelope` over
    the canonical SCCP statement batch derived from the bundle and manifest;
  - JSON responses for this route also expose `proof_envelope_summary`, which
    reports the decoded open-verify backend, circuit id, verifier commitment
    hash, schema hash, public-input column/word counts, and wrapper/backend
    proof lengths without changing the underlying Norito wire artifact;
  - a generated `submission_package` carrying the target verifier entrypoint,
    envelope encoding, raw argument blobs, prebuilt relay envelope bytes, and
    a typed `platform_payload` view for that lane:
    - ETH/BSC production backend: `evm_groth16_contract_call` carrying the
      Groth16 ABI proof tuple directly, with no attestor signatures;
    - ETH/BSC reference backend: `evm_contract_call`, available only for the
      non-production secp256k1 wrapper path;
    - TRON: `tron_contract_call`
    - Solana: `solana_program_instruction`
    - TON: `ton_internal_message`, carrying a
      `ton_message_body_boc_v1` `message_body_boc` plus its `query_id`,
      destination binding hash, statement hash, proof bytes, public inputs,
      and SCCP bundle bytes;
    - Substrate-family lanes: `substrate_runtime_call`
  - JSON responses for EVM Groth16 artifacts expose
    `groth16_proof_summary` instead of `proof_envelope_summary`. The summary
    reports `version`, `proof_len_bytes`, `public_input_word_count = 6`,
    `groth16_public_signal_count = 9`, `message_id`, `source_domain`,
    `commitment_root`, `destination_binding_key`, and
    `destination_binding_hash`.
  - the embedded Nexus SCCP message bundle so verifiers can reconstruct the exact statement being proven.
  - `iroha_sccp` now also exposes a normalized counterparty proof-job projection over that artifact:
    - `decode_sccp_normalized_codec_value(...)` decodes codec-bearing SCCP fields into typed EVM / Solana / TON / Tron / logical-text values; and
    - `build_sccp_counterparty_proof_job_from_artifact(...)` /
      `build_sccp_counterparty_proof_job_from_artifact_allow_unready(...)` /
      `build_sccp_counterparty_proof_job_from_bundle(...)` produce a
      prover-oriented job with the normalized payload projection plus the
      original typed bundle. For production EVM/BSC Groth16 lanes, proof tools
      must use
      `build_sccp_counterparty_proof_job_from_bundle_with_evm_groth16_proof_and_destination_binding(...)`
      or its `_allow_unready` diagnostic variant so the actual Groth16 proof
      bytes are supplied explicitly and the signer path remains closed.
  - client helpers now exist for that route directly:
    - Rust: `iroha::client::Client::get_sccp_message_proof_artifact_json(...)` and `get_sccp_message_proof_artifact(...)`;
    - Python: `ToriiClient.get_sccp_message_proof_artifact(...)`; and
    - JavaScript: `ToriiClient.getSccpMessageProofArtifact(...)`.
  - current production behavior: this route rejects all live counterparty lanes
    because the all-lanes-at-once launch policy requires each advertised chain
    to have source-chain finality/inclusion verification, immutable destination
    verifier deployment, active cryptographic anchors, and an anchored route
    allowlist before any lane is enabled.
- `GET /v1/sccp/jobs/message/{message_id}` returns the normalized SCCP counterparty proof job for the same canonical message id. Each job bundles:
  - the chain family, chain key, backend labels, verifier backend, manifest seed, finality model, verifier target, and canonical SCCP public inputs;
  - the same SCCP security model / cryptographic anchor mode and destination binding that the artifact and manifest commit into the canonical statement hash;
  - a normalized payload projection with typed codec values for EVM / Solana / TON / Tron / logical-text surfaces; and
  - the same chain-specific `submission_template` advertised by the manifest, so proof tooling can derive the target verifier entrypoint and argument list without hard-coding per-chain packaging; and
  - the generated `submission_package` for the chain-specific relay/verifier
    lane, including the same typed `platform_payload` projection surfaced on the
    artifact route; and
  - production-ready EVM/BSC lanes require callers to provide
    `network_id_hex`, `verifier_address_hex`, and `bridge_address_hex`; those
    fields bind only the EVM deployment submission package, while the
    transparent proof artifact keeps the manifest destination binding; and
  - JSON responses for OpenVerify-backed jobs expose `proof_envelope_summary`,
    derived from the canonical native SCCP proof for the bundled message so
    operators can inspect the bound circuit/verifier/schema metadata before
    submission. EVM Groth16 jobs expose `groth16_proof_summary` instead, using
    the same fields as the artifact route; and
  - the original typed Nexus SCCP message bundle so proof tooling can keep both the normalized view and the canonical committed preimage in one document.
  - client helpers now exist for that route directly:
    - Rust: `iroha::client::Client::get_sccp_message_proof_job_json(...)` and `get_sccp_message_proof_job(...)`;
    - Python: `ToriiClient.get_sccp_message_proof_job(...)`; and
    - JavaScript: `ToriiClient.getSccpMessageProofJob(...)`.
  - current production behavior: this route rejects all live counterparty lanes
    because the all-lanes-at-once launch policy requires each advertised chain
    to have source-chain finality/inclusion verification, immutable destination
    verifier deployment, active cryptographic anchors, and an anchored route
    allowlist before any lane is enabled.
- `GET /v1/sccp/proofs/message/{message_id}` now reconstructs the proof from committed blocks that contain `RecordSccpMessage` instructions and a non-null `sccp_commitment_root` in the finalized block header. The in-memory bundle registry is retained only for unit tests and never bypasses typed artifact or finality verification.
- Generic SCCP `message` payloads now enforce explicit v1 codec families during structural verification instead of accepting arbitrary nonzero codec ids:
  - `1`: generic UTF-8 logical identifiers;
  - `2`: EVM `0x`-prefixed 20-byte hex addresses;
  - `3`: Solana base58 public keys;
  - `4`: TON raw `workchain:account_hex` addresses; and
  - `5`: Tron base58check account addresses.
- `POST /v1/bridge/proofs/submit` accepts exactly one of `burn_bundle` or `message_bundle`. Token add, pause, and resume operations are submitted as SCCP message bundles; the bridge does not accept parliament certificates as transaction proofs. `message_bundle` is converted into a typed SCCP transparent proof artifact and then wrapped in a bridge proof with backend label `bridge/sccp/stark-fri-v1/<chain>`.
  - current production behavior: generic SCCP `message_bundle` conversion is
    disabled for all live counterparties until every advertised chain satisfies
    the all-lanes production readiness gate. Governance installs verifier
    identities, code hashes, anchors, and route allowlists; after activation,
    message submission remains permissionless and no human approval is part of
    the per-message validity path.
  - SCCP artifact/job discovery, state-changing SCCP submit endpoints, and
    on-chain `SubmitBridgeProof` validation ignore
    `sccp_allow_unready_transparent_proofs`; that flag cannot make disabled
    lanes consumable.
  - non-SORA `message_bundle` submission must carry a verified source-chain
    proof envelope; Torii no longer manufactures synthetic external-chain
    finality from local Nexus/Iroha finality evidence.
- `POST /v1/bridge/proofs/submit` now derives chain-specific SCCP transparent backends for generic `message` bundles:
  - outbound `SORA -> ETH` and inbound `ETH -> SORA` messages use `bridge/sccp/stark-fri-v1/eth`;
  - the same pattern applies to `bsc`, `sol`, `ton`, `tron`, `sora2`, `sora-kusama`, and `sora-polkadot`;
  - the bridge proof manifest hash is derived from the same domain suffix, so proof IDs and registry queries split cleanly by counterparty chain instead of collapsing all SCCP traffic into one generic backend bucket.
- ETH/BSC message-proof building previously depended on Torii's
  `da_receipt_signer` using `secp256k1`, because the EVM submission package was
  a signer-backed attestation envelope over the canonical SCCP proof-envelope hash and
  canonical public inputs. That path is now disabled for production because it
  is not destination-native cryptographic verification.
- `POST /v1/bridge/proofs/submit` and `POST /v1/bridge/messages` now also return normalized SCCP counterparty metadata in the response:
  - `counterparty_domain` is the numeric SCCP domain id; and
  - `counterparty_chain` is the canonical domain key (`eth`, `bsc`, `sol`, `ton`, `tron`, `sora2`, etc.).
- `GET /v1/zk/proof/{backend}/{hash}` and `GET /v1/zk/proofs` now mirror that metadata inside `bridge.payload` for SCCP transparent proofs when the backend matches the chain-split SCCP family.
  - when the stored payload decodes as a typed SCCP artifact, the bridge summary now also exposes `message_id`, `payload_hash`, `target_domain`, `commitment_root`, `finality_height`, `finality_block_hash`, and `proof_artifact_len_bytes`.
  - the bridge summary additionally exposes `verifier_backend`, `inner_verifier_backend`, `inner_chain_family`,
    `inner_payload_kind`, and `inner_statement_hash`, derived from the
    canonical SCCP statement context rather than from an embedded placeholder
    envelope.
- `POST /v1/bridge/messages` accepts an inbound `message_bundle` targeted at SORA, records the corresponding transparent-ZK bridge proof, and emits a typed `BridgeReceipt` for `transfer` payloads.
- `GET /v1/sccp/messages/recent` exposes newest-first committed SCCP message discovery with compact metadata, decoded payload projections when available, and direct links to the existing bundle / artifact / job endpoints.
- `POST /v1/bridge/messages` now also accepts an optional `settlement` object:
  - it resolves a deployed contract target by `contract_address` or `contract_alias`;
  - it appends an ephemeral by-call trigger after proof verification so settlement can happen in the same submitted transaction; and
  - when `payload` is omitted for `finalize_inbound`, Torii auto-builds `finalize_inbound(route, message_id, recipient, amount)` from the `transfer` message bundle and requires the proof-derived `route_id` to decode as a logical `Name`;
  - when `payload` is omitted for `activate_route_governed`, Torii auto-builds `activate_route_governed(message_id, route, asset_key, remote_domain)` from the `route_activate` message bundle and requires both the proof-derived `route_id` and `asset_id` to decode as logical `Name`s;
  - explicit `settlement.payload` is rejected for those proof-managed bridge entrypoints, so callers cannot bypass the proof-derived settlement inputs with raw custom payloads.
- Automatic settlement is still opt-in per request. Cross-node policy for always-on contract dispatch remains a higher-level integration choice outside this endpoint.
- The CLI now exposes read-only SCCP discovery helpers under the bridge feature:
  - `iroha ops bridge sccp capabilities`
  - `iroha ops bridge sccp manifests`
  - `iroha ops bridge sccp artifact --message-id <hex>`
  - `iroha ops bridge sccp job --message-id <hex>`
  - text mode prints compact chain/proof summaries, and `artifact` / `job` now also decode the normalized payload projection, verifier backend, and generated chain-specific submission package when they are present;
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
canonical SORA-origin SCCP payload bytes and remains permissionless for valid bridge flows, but it
is accepted only while applying a verified `Executable::IvmProved` overlay. Bare
`RecordSccpMessage` transactions and non-SORA-origin payloads fail during execution, still follow
the normal rejected-transaction fee path, and do not contribute to the block-level
`sccp_commitment_root`. Proposal assembly derives the root only from proved-overlay records.

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
