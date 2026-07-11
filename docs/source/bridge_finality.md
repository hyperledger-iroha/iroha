<!--
SPDX-License-Identifier: Apache-2.0
-->

# Bridge finality proofs

This document defines the first-release bridge finality surface. It carries the
exact durable finality evidence produced by Sumeragi v2. The proof envelope has
schema version `1`, while the consensus protocol inside it is version `2`.
There is no Sumeragi-v1 certificate projection, decoder, or fallback path.

## Exact proof format

`BridgeFinalityProof` is encoded with Norito or Norito JSON and has exactly three
fields:

```text
{
  version,
  block_header,
  finality_artifact
}
```

- `version` must equal `BRIDGE_FINALITY_PROOF_VERSION_V1` (`1`).
- `block_header` is the canonical `BlockHeader` selected by the requested
  height.
- `finality_artifact` is the exact `V2FinalityArtifact` persisted by the
  Sumeragi-v2 apply path for that block. It embeds one durable BLS-normal proof
  of possession for every entry in its height-context roster, in roster order.

The durable artifact is the single source of consensus truth in the proof. It
contains its format and protocol versions, height, complete immutable
`HeightContext`, exact `BlockSubject`, block hash, Commit quorum certificate,
and roster-aligned validator PoPs. The height context freezes the chain id,
epoch bounds, consensus mode, parent CommitQC, ordered `ValidatorPower` roster,
canonical `DualQuorum`, Nexus/AMX context commitment, data-availability layout,
and leader seed. At an epoch-ending boundary parent it also embeds the optional
`next_epoch_snapshot`; because that field is part of the context id, the
parent's CommitQC authenticates the snapshot before it can authorize a child
roster. The `FinalizedNextEpochSnapshot` binds its `epoch_end_height` and the
next roster's aligned `validator_set_pops` as well as the next epoch parameters.
The subject binds the parent block hash, block hash, and canonical payload hash.

There are deliberately no duplicate proof-level height, chain, block hash,
roster hash, or certificate fields. A malformed sidecar therefore cannot ask a
verifier to choose between competing copies of the same consensus fact.

## Durable production source

The Sumeragi-v2 apply service constructs the artifact from the frozen height
context, exact decided subject, and exact CommitQC, validates it, and stores it
as an immutable Kura sidecar after applying the block. The write is idempotent;
Kura rejects a conflicting artifact at the same height. Restart recovery can
finish a missing sidecar without re-executing an already applied block.

`build_finality_proof` reads the canonical block and its sidecar by height,
checks their height/hash/chain association, and runs the same cryptographic
verifier used by consumers. Historical verification reads the PoPs embedded in
the sidecar; it never substitutes keys or PoPs from mutable current world state,
reconstructs historical consensus evidence, or projects a retired certificate
format. Proof availability follows the durable block and sidecar; it is not a
recent in-memory certificate window. Missing, corrupt, conflicting, or
unverifiable sidecars fail closed.

## Canonical verification

`iroha_data_model::bridge::verify_bridge_finality_proof` performs the stateless
structural and cryptographic checks:

1. Require proof schema version `1`, artifact format version `1`, and Sumeragi
   protocol version `2` in both the artifact and height context.
2. Validate the height context, its ordered powered roster, canonical dual
   quorum, parent certificate rules, DA layout, and epoch bounds.
3. Require the artifact height, context id, block subject, repeated block hash,
   CommitQC round, and Commit phase to agree exactly. A `next_epoch_snapshot`
   in the height context is mandatory for an epoch-ending boundary parent and
   forbidden elsewhere.
4. Require the artifact chain id to equal the caller's expected chain id.
5. Recompute the block-header height and hash and require both to match the
   artifact.
6. Require the artifact to embed one BLS-normal PoP per roster entry and verify
   every PoP against the corresponding public key.
7. Require strictly increasing, in-range signer indices. The certificate must
   satisfy both quorum thresholds: at least `floor(2n/3) + 1` distinct roster
   members and signed voting power strictly greater than two thirds of total
   power.
8. Reconstruct the exact Sumeragi-v2 vote preimage and verify the selected-key
   BLS aggregate signature.

The vote preimage is domain-separated by `iroha:sumeragi:v2:vote` and encodes
the following Norito payload:

```text
{
  protocol_version: 2,
  round: { context_id, height, view },
  phase: Commit,
  subject: { parent_block_hash, block_hash, payload_hash }
}
```

The signer index and individual signature are not part of the same-message
preimage. The CommitQC's strictly ordered signer list selects the BLS keys and
their aligned PoPs. BLS and PoP verification is mandatory in every production
build; structural validity alone is never finality.

## Trust anchor and successor verification

A standalone proof can establish that its header, artifact, powered roster,
PoPs, and aggregate signature are internally consistent. It cannot establish
that a proof-carried roster is the canonical roster for the intended chain.
Callers must supply trust independently.

`BridgeFinalityVerifier` therefore requires an explicitly trusted
`HeightContextId` before accepting its first proof; it never learns trust from
that proof. It also binds every proof to the configured chain id. After the
first proof it accepts only the immediate next height and verifies that:

- the child context carries a valid parent CommitQC for the previously accepted
  committed decision;
- that parent certificate verifies under the previous frozen roster and PoPs;
- chain, consensus mode, and DA layout obey the v2 transition rules; and
- within an epoch, the child copies the previous artifact's roster-aligned PoPs;
  at an epoch boundary, its epoch, roster, dual quorum, leader seed, and PoPs
  match the previous height context's CommitQC-authenticated
  `next_epoch_snapshot`, including its authenticated `epoch_end_height`.

Stale and skipped heights, unlinked parents, and unauthorized context
transitions are rejected. Applications that start from a later checkpoint must
pin that checkpoint's context id through governance or another authenticated
channel, then verify every immediate successor.

## SCCP trust boundary

`TairaSccpMessageProofV1.finality_proof` is the canonical Norito encoding of the
same `BridgeFinalityProof`; SCCP does not maintain a second consensus transcript
or quorum implementation. Structural message checks bind the selected SCCP
commitment and Merkle path to the commitment root in the finalized block
header. Cryptographic verification then establishes self-consistency under the
artifact's frozen roster.

Self-consistency is not the SCCP trust decision. Each governed outbound route
pins an `SccpSoraFinalityAnchorV1` containing the exact Taira source network,
protocol version `2`, Taira chain-id hash, checkpoint height and block hash,
checkpoint `HeightContextId`, and a domain-separated hash of the canonical
checkpoint finality artifact. The governed semantic circuit exposes the hash of
this typed anchor as its final public signal.

Admission must resolve that anchor from historical governed route state,
authenticate the checkpoint artifact, and establish an immediate-successor
chain from the checkpoint to the message artifact (or compare against the same
trusted local artifacts). Merely accepting a valid aggregate signature under a
roster supplied by the message would not establish Taira finality.

## Commitment bundle and MMR

`BridgeFinalityBundle` has exactly two fields:

- `commitment`: `{ chain_id, height_context_id, block_height, block_hash,
  mmr_root?, mmr_leaf_index?, mmr_peaks? }`;
- `finality_proof`: the complete proof described above.

The optional MMR fields are commitments only: they are a root-checkpoint aid,
not a finality substitute or an inclusion proof.
The endpoint recomputes the block-hash MMR and returns its peaks but does not
return a membership path. Peaks are ordered left to right and bagged from right
to left: `root = H(p_n, H(p_{n-1}, ... H(p_1, p_0)))`. SCCP uses its own typed
message Merkle branch and governed finality anchor instead of this optional MMR
surface.

## API surface

- `GET /v1/bridge/finality/{height}` returns `BridgeFinalityProof` as Norito by
  default or Norito JSON through `Accept` negotiation.
- `GET /v1/bridge/finality/bundle/{height}` returns `BridgeFinalityBundle`.

Both endpoints fail closed when the block or exact durable v2 artifact is
absent or invalid. First-release consumers must reject unknown fields,
unsupported proof/artifact versions, and any retired proof shape; there is no
compatibility fallback.
