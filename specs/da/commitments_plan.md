# Sora Nexus data availability commitments (DA-3)

A signed block can include a commitment to an application blob together with
the producer's signed pin intent. The commitment binds the blob to its lane,
manifest and retention policy. A provider's custody of the bytes requires a
separate native publication and authenticated completion evidence. The DA
ingest plan describes how to publish and recover a certified blob.
[Ingest and publication](ingest_plan.md)

## Record and wire format

`DaCommitmentRecord` identifies the lane, epoch, sequence, client blob ID,
manifest digest and storage ticket. It includes the Torii acknowledgement
signature, retention class, proof scheme, chunk root and optional proof digest.
`chunk_root` preserves all 32 bytes of the SoraFS BLAKE3 PoR root. The
`proof_digest` field must be present in JSON and can contain `null`. The
first-release lane configuration accepts `sorafs_por_blake3`.
[Record definition](../../crates/iroha_data_model/src/da/commitment.rs)

Commitment and pin-intent bundles travel with the canonical `SignedBlockWire`.
For each nonempty bundle, the signed block header commits its tree version,
leaf count and root. Leaves use each record's canonical Norito encoding. Leaf,
internal-node and tree-descriptor hashes use separate domains for each tree.
An odd last node is promoted unchanged. The header also commits the block's
proof-policy sidecar. [Commitment tree](../../crates/iroha_data_model/src/da/commitment.rs)
and [pin-intent tree](../../crates/iroha_data_model/src/da/pin_intent.rs)

## From ingest to finality

1. Torii persists the manifest, PDP commitment and retained shards. The
   producer verifies the operator-signed receipt and signs the pin scope,
   including the digest of the exact commitment record.
2. Core journals the commitment and pin intent as one candidate. The queue
   holds at most 128 entries and at most 1 MiB of encoded journal data.
   A locally admitted candidate can be relayed to validators in one hop.
   Conflicting ticket, manifest or lane/epoch/sequence identities are rejected.
3. A proposer selects eligible candidates while assembling ordinary paid
   work. Once a lane/epoch cursor exists, later candidates need a contiguous
   sequence. Selection rotates optional lanes across block heights. Validators
   compare the paired records, verify the producer's authorization and apply
   the lane's proof and admission policy.
4. The finalized block authenticates both records. Committed identities and
   cursor advances make consumed or stale entries ineligible after restart.

[Proposal queue and selection](../../crates/iroha_core/src/da/proposal_queue.rs)

## Queries and verification

| Method and route | Request | Result |
| --- | --- | --- |
| `POST /v1/da/commitments` | `DaCommitmentListRequest` | Public, paginated list of located records. |
| `POST /v1/da/commitments/prove` | `DaCommitmentProofRequest` | Membership proof and the referenced block's proof-policy sidecar. |
| `POST /v1/da/commitments/verify` | `DaCommitmentProof` | Verification against the node's stored block. |
| `POST /v1/da/pin-intents` | `DaPinIntentListRequest` | Public, paginated list of located pin intents. |
| `POST /v1/da/pin-intents/prove` | `DaPinIntentQueryRequest` | Pin-intent membership proof. |
| `POST /v1/da/pin-intents/verify` | `DaPinIntentProof` | Verification against the node's stored block. |

The prove and verify routes require canonical account signatures. A client
establishes finality from its trusted Sumeragi checkpoint, then verifies the
returned path against the authenticated block header. Commitment verification
also checks the supplied policy sidecar against the header. A remote
verification result alone supplies no independent finality evidence.
[Route policies](../../crates/iroha_torii_shared/src/route_catalog.rs) and
[proof verification](../../crates/iroha_core/src/da/proofs.rs)

## Qualification

The four-validator integration test submits a two-part blob, queries both
proofs on a different validator, verifies an actual fee debit in the inclusion
block, and publishes the package to three assigned providers. It restarts two
providers and takes the original ingress offline before reading the package
and recovering the original bytes with a data shard withheld. Run the exact
`sorafs_publication::da_lifecycle::four_peer_multipart_da_publication_and_cold_parity_recovery`
test in the `integration_tests` crate's `core_api` target with
`IROHA_TEST_REQUIRE_NETWORK=1`. Set `TEST_NETWORK_BIN_SORAFS_CLI` to the
absolute path of the same-revision provider CLI. To retain the evidence, set
`IROHA_TEST_DA_PUBLICATION_EXPORT` to a new absolute directory name under an
existing parent. The test writes the signed receipt, checkpoint, commitment and
pin proofs, charged transaction identity, provider certificates and both recovered
payloads there before teardown. [Four-peer flow](../../integration_tests/tests/sorafs_publication_da.rs)
