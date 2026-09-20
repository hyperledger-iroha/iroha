# Snapshot integrity and restoration

The implementation owners are `iroha_core::snapshot`, `state::deserialize` and
the daemon startup policy in `irohad`. This first-release format has one
canonical representation; missing authoritative state is an error.

## Durable generation

`snapshot.store_dir/current` names an immutable directory under `generations/`.
Each selected generation contains exactly five artifacts:

- `snapshot.data`: canonical Norito JSON State, including authoritative MV
  current values and predecessor records.
- `snapshot.sha256`: the payload digest.
- `snapshot.fast.norito`: the bounded, versioned recovery manifest binding the
  chain/network, height/tip, SCCP policy and bootstrap-lineage presence.
- `snapshot.sig`: the node signature over the domain-separated bundle digest,
  which authenticates both the payload digest and recovery manifest.
- `snapshot.merkle.json`: canonical chunk geometry and SHA-256 leaf commitments.

The writer authenticates and synchronizes a complete generation before replacing
and synchronizing `current`. A reader selects only that pointer and binds the
identities of its directories and regular files. It rechecks the selection and
artifacts around validation. Loose temporary files are not restore authority.
Chunk proofs bind both the root and leaf count, including ragged-tree geometry.

## Strict restoration

The reader checks bounded file sizes, bundle authentication, payload digest,
canonical Merkle metadata, exact chunk geometry and resource policy before typed
State restoration. Fields decode directly from borrowed JSON slices. Static
schema metadata supplies canonical field order without constructing a default
World. Unknown, missing or noncanonical authoritative fields are rejected.

The decoded `Box<State>` retains one heap owner through semantic validation,
initializer callbacks and handoff. Replay preparation also retains a boxed State;
the daemon converts its owned restored State to `Arc<State>`. Nested helpers must
not return whole State values on each stack frame. Ordinary-stack snapshot tests
cover this ownership boundary; no thread-stack override is part of restoration.

Current and actual predecessor records must be validated together. A derived
index must reconstruct both projections from the corresponding authoritative
histories, including deletions and absence preimages. Rebuilding only its current
map loses latest-block replacement semantics. The remaining coverage and
publication requirements are tracked in the
[liveness goals](sumeragi_liveness_redesign_goals.md); this rule is not a claim
that every derived store is already qualified.

Network identity, governed SCCP state, runtime policy and the actual rollback
candidate are checked before snapshot-driven Kura reconciliation. A retained
matching Kura checkpoint also authenticates the canonical snapshot WSV hash.
Digest-pinned bootstrap authorization is explicit and binds its audited boundary.
The daemon owns fallback policy and requires retained Kura geometry sufficient
for reconstruction; it does not accept a failed snapshot as restored state.

## Emergency Fast mode and resource bounds

Emergency Fast startup authenticates the bounded recovery manifest and exact
Kura terminal boundary. It deliberately defers payload/World semantics and
selected journals until Strict restart; it cannot authorize snapshot-bootstrap
lineage and is not full snapshot validation.

`snapshot.max_payload_bytes`, `snapshot.merkle_chunk_size_bytes` and
`snapshot.resources` bound input and structural work. Sidecars have separate
format-derived limits. Heap ownership prevents repeated stack copies; it does
not establish a total allocation or RSS bound. Generation-coherent capture is
finite and reports busy/changed outcomes, while complete capture allocation
admission remains an explicit implementation requirement.
