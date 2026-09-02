<!--
SPDX-License-Identifier: Apache-2.0
-->

# Bridge finality proofs

This document defines the first-release bridge finality surface. It carries the
exact durable finality evidence produced by Sumeragi v2. The proof envelope has
schema version `2`, while the consensus protocol inside it is version `4`.
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

- `version` must equal `BRIDGE_FINALITY_PROOF_VERSION_V2` (`2`).
- `block_header` is the canonical `BlockHeader` selected by the requested
  height.
- `finality_artifact` is the exact `V2FinalityArtifact` persisted by the
  Sumeragi-v2 apply path for that block. It embeds one durable BLS-normal proof
  of possession for every entry in its height-context roster, in roster order.

The durable artifact is the single source of consensus truth in the proof. It
contains its format and protocol versions, height, complete immutable
`HeightContext`, exact `BlockSubject`, block hash, Commit quorum certificate,
and roster-aligned validator PoPs. The height context freezes the exact
genesis-derived network id,
epoch bounds, consensus mode, parent CommitQC, ordered `ValidatorPower` roster,
canonical `DualQuorum`, Nexus/AMX context commitment, data-availability layout,
and leader seed. At an epoch-ending boundary parent it also embeds the optional
`next_epoch_snapshot`; because that field is part of the context id, the
parent's CommitQC authenticates the snapshot before it can authorize a child
roster. The `FinalizedNextEpochSnapshot` binds its `epoch_end_height` and the
next roster's aligned `validator_set_pops` as well as the next epoch parameters.
The subject binds the parent block hash, block hash, and canonical payload hash.

There are deliberately no duplicate proof-level height, network, block hash,
roster hash, or certificate fields. A malformed sidecar therefore cannot ask a
verifier to choose between competing copies of the same consensus fact.

## Durable production source

The Sumeragi-v2 apply service constructs the artifact from the frozen height
context, exact decided subject, and exact CommitQC and validates it. Before
publishing finality, Kura durably creates an immutable retained-block record
containing the exact canonical header and the block's canonical SCCP outbox
archive in commitment-index order. Version 3 also retains the optional compact
merge-ledger reference extracted from the canonical body for bounded local
historical-sidecar service; unlike the SCCP archive, that field is not exported
as a standalone inclusion proof and requesters revalidate it against their own
carrier. The same retained record must exist before Kura may evict the
historical block body. Kura then stores the validated
artifact in a separate immutable finality record with the same header. Both
writes are idempotent no-clobber operations; a conflicting record at the same
height is rejected. Version 3 is the only accepted retained-record layout.
Pre-release version-2 bytes fail closed during direct reads and startup rather
than being promoted or used as partial finality/SCCP evidence.

Application requires its durable manifest, body frame, deterministic validation
receipt, and execution commitment to match the CommitQC's authenticated
`proposal_round`. The reducer owner tag may name another process generation and
the CommitQC may have a later finality view; neither is allowed to relabel the
block header or application input.

Before accepting or returning finality, Kura validates the complete
canonical-header association (height, hash, predecessor, and immutable
proposal-origin view), then
verifies every roster-aligned PoP, both quorum thresholds, and the CommitQC
aggregate signature. Restart inventory also validates the retained header,
archive, finality record, and durable block-hash association. Recovery can
finish a missing finality record from the retained header without re-executing
an already applied block or restoring its body.

`build_finality_proof` reads the retained canonical header and verified finality
record by height. It never reads a historical block body. Historical
verification reads the PoPs embedded in the artifact; it never substitutes
keys or PoPs from mutable current world state, reconstructs historical
consensus evidence, or projects a retired certificate format. Proof
availability follows the immutable retained-header/finality records and the
durable canonical hash journal, not body-cache residency or a recent in-memory
certificate window. Missing, corrupt, conflicting, or unverifiable records fail
closed.

## Canonical verification

`iroha_data_model::bridge::verify_bridge_finality_proof` performs the stateless
structural and cryptographic checks:

1. Require proof schema version `2`, finality-artifact format version `4`, and
   live Sumeragi protocol version `4` in both the artifact and height context.
2. Validate the height context, its ordered powered roster, canonical dual
   quorum, parent certificate rules, DA layout, and epoch bounds.
3. Require the artifact height, context id, block subject, repeated block hash,
   CommitQC finality round, authenticated proposal round, and Commit phase to
   agree exactly. Prepare evidence requires equal proposal and certification
   rounds; Commit permits only a proposal view at or before its finality view.
   A `next_epoch_snapshot` in the height context is mandatory for every
   representable epoch-ending boundary parent and forbidden elsewhere. The
   terminal `u64::MAX` height carries none because no successor height exists.
4. Require the artifact network id to equal the caller's expected network id.
5. Recompute the block-header height, hash, predecessor, and view-change index
   and require them to match the artifact's height, block hash, subject parent,
   and CommitQC `proposal_round.view` respectively. The CommitQC's own
   `round.view` may be later because it is the finality round, not the block's
   immutable origin.
6. Require the artifact to embed one BLS-normal PoP per roster entry and verify
   every PoP against the corresponding public key.
7. Require strictly increasing, in-range signer indices. The certificate must
   contain exactly `floor(2n/3) + 1` distinct roster members; because revision 4
   is equal-vote, this exact count also has signed voting power strictly greater
   than two thirds of total power. Signer supersets are rejected.
8. Reconstruct the exact Sumeragi-v2 vote preimage and verify the selected-key
   BLS aggregate signature.

The vote preimage is domain-separated by `iroha:sumeragi:v2:vote` and encodes
the following Norito payload:

```text
{
  protocol_version: 4,
  round: { context_id, height, view },
  proposal_round: { context_id, height, view },
  phase: Commit,
  subject: { parent_block_hash, block_hash, payload_hash },
  execution_commitment: {
    parent_state_root,
    post_state_root,
    ordinary_writes_root,
    offline_cash_top_up_root,
    offline_cash_top_up_count,
    native_amx_application_manifest_version,
    native_amx_application_manifest_root,
    native_amx_application_manifest_count,
    merge_carrier,
    executed_block_wire_len,
    executed_block_wire_hash
  }
}
```

For Commit, both rounds have the same context and height and the proposal view
cannot exceed the finality view. Omitting `proposal_round` is not a supported
legacy encoding. The subject hash authenticates the canonical resultless
proposal. The execution commitment separately authenticates the exact
canonical result-bearing block, so replay cannot substitute either proposal
bytes or deterministic execution results while preserving the other binding.
The versioned Native AMX application-manifest root additionally authenticates
the ordered participant-application leaves and their proofs. A zero leaf count
must use the canonical empty root; a nonzero count must not use that root.
The `merge_carrier` option is always present in the execution-commitment wire
layout: it is empty for an ordinary block and otherwise carries the exact V1
merge-ledger entry hash authenticated by finality. It is followed by the
mandatory non-zero `executed_block_wire_len` and then `executed_block_wire_hash`,
binding both the exact byte length and digest of the canonical result-bearing wire.

The signer index and individual signature are not part of the same-message
preimage. The CommitQC's strictly ordered signer list selects the BLS keys and
their aligned PoPs. BLS and PoP verification is mandatory in every production
build; structural validity alone is never finality.

## Trust anchor and successor verification

A standalone proof can establish that its header, artifact, powered roster,
PoPs, and aggregate signature are internally consistent. It cannot establish
that a proof-carried roster is the canonical roster for the intended network.
Callers must supply trust independently.

`BridgeFinalityVerifier` therefore requires an explicitly trusted
`HeightContextId` before accepting its first proof; it never learns trust from
that proof. It also binds every proof to the configured network id. After the
first proof it accepts only the immediate next height and verifies that:

- the child context carries a valid parent CommitQC for the previously accepted
  committed decision;
- that parent certificate verifies under the previous frozen roster and PoPs;
- network, consensus mode, and DA layout obey the v2 transition rules; and
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
protocol version `4`, Taira chain-id hash, checkpoint height and block hash,
checkpoint `HeightContextId`, and a domain-separated hash of the canonical
checkpoint finality artifact. The governed semantic circuit exposes the hash of
this typed anchor as its final public signal. SCCP admits only revision `4`;
revision-3 anchors are invalid even when they were previously signed.

Admission must resolve that anchor from historical governed route state,
authenticate the checkpoint artifact, and establish an immediate-successor
chain from the checkpoint to the message artifact (or compare against the same
trusted local artifacts). Merely accepting a valid aggregate signature under a
roster supplied by the message would not establish Taira finality.

## Compact commitment bundle

`BridgeFinalityBundle` has exactly two fields:

- `commitment`: `{ network_id, height_context_id, block_height, block_hash }`;
- `finality_proof`: the complete proof described above.

The compact commitment duplicates only the exact network, height context, block
height, and block hash needed to reject bundle drift before verifying the
embedded proof. SCCP inclusion uses its own typed message Merkle branch and
governed finality anchor.

## API surface

- `GET /v1/bridge/finality/{height}` returns `BridgeFinalityProof` as Norito by
  default or Norito JSON through `Accept` negotiation.
- `GET /v1/bridge/finality/bundle/{height}` returns `BridgeFinalityBundle`.

Both endpoints fail closed when the retained canonical header or exact durable
v2 artifact is absent or invalid. Historical block-body eviction does not make
an otherwise valid proof unavailable. First-release consumers must reject
unknown fields, unsupported proof/artifact versions, and any retired proof
shape; there is no compatibility fallback.
