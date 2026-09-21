# Canonical-reopen dependency audit

Status: the bounded removal/reconciliation is applied; coordinated Rust runtime
validation is pending. It removes an unused local placeholder; it does not
activate a source producer, verifier or release authority.

Paths below are relative to `crates/iroha_zkp_halo2/src/vega/zk_ams/mkhe`.

## What the 344-group placeholder actually does

`collective/incremental_source_phase23_source_algebra/global_lookup_source_replay_v1/source_openings_v1.rs`
defines a private `WeightedOpeningColumnsSinkV1`. For input values `m[g,j]`,
`j=256*b+i`, `k=64*i+b`, its computation is:

```text
source_column[j] = sum_(g=0..343) w[g] * m[g,j]
packing_column[k] = sum_(g=0..343) w[g] * m[g,j]
```

This is the same weighted vector in two coordinate orders. The sink checks
shape/order and retains zeroizing vectors. It does not verify a public opening
equation or bind a Pedersen commitment. Zero fixture weights are accepted.

The child `source_openings_v1/canonical_reopen_v1.rs` rereads the canonical
source and stores the sink output in
`Phase23GlobalLookupSourceReopenedV1.weighted_columns`. Repository searches found
no read of that field, no consumer of either column, and no caller of
`into_canonical_opening_replay_v1`. The production seal has an uninhabited
`post_rho_verifier_weights` field; only tests construct weights.

The two domains ending in `source-opening.canonical-reopen-schedule\0` and
`source-opening.canonical-reopen-record\0` hash a local read schedule and record.
Neither is a weight challenge. `CanonicalReopenRecordV1` does not bind weights,
weight provenance or a verified equation. No exact existing transcript owner
can be assigned to the placeholder. This is an absence in the inspected source
and specification, not a proof that some unpublished construction is impossible.

## Existing native40 obligation retained without changes

`rns_native_source_packing_same_opening.rs` already names the source/packing
statement, canonical source schedule, challenge order and consuming owner.
`collect_commitments_v1` reconstructs each of 344 D-packing commitments:

```text
B = 2^15
C_D[g] = sum_(h=0..16) B^h * C_Dlow[g,h] + B^17 * C_bD[g]
```

Those points plus 1,032 signed-source commitments form 1,376 ordered owners.
`prepare_relation_v1` rereads the exact D/signed vectors and forms:

```text
Q = sum_(o=0..1375) tau^o * (C[o] - <v[o], G>)
z*H = A + c*Q
```

The verifier checks the latter Schnorr equation; it permits identity `Q` and
rejects identity `A`. D coordinates are `64*i+b`; signed coordinates are
`1024*local_block+i`. The existing challenge domains end in:

- `rns-native-source-packing-same-opening.pre-challenge`
- `rns-native-source-packing-same-opening.tau`
- `rns-native-source-packing-same-opening.schnorr`

`verify_rns_native_source_packing_same_opening_owned_v2` consumes a purpose-bound
borrow from the actual `RnsNativeDirectGlobalMembershipReplayV2` predecessor.
Its result retains the predecessor and the actual point/source/replay/equation
bindings; the final composite transition also requires its existing qPCS owner.
The patch changes none of these equations, fields, readers, domains or seals.

The original 344 `Csrc` points and blindings remain distinct and stay in the
original inventory/session. They are in source order and cannot supply the
1,376 derived packing masks. Removing unused weighted columns is therefore
appropriate; claiming formal equivalence to an unspecified older proof, or
deleting the real original source inventory, would not be justified.

## Early-record dependency trace

| Early input | Actual transitive inputs | Current-proof successor edge found? |
| --- | --- | --- |
| `GlobalLookupSourceReplayRecordV1` | Source publication receipt, source-algebra prerequisite, topology/compact mapping, replay spool context, authenticated read schedule, sealed compact snapshot, counters/false gates | None in the inspected record construction |
| `SourceOpeningRecordV1` | Same source receipt/prerequisite, topology/source mapping/basis, local replay context, original commitment root, confidential blinding snapshot root, completed first-pass counters/false gates | None after removing unused future-pass accounting; those counters never constituted proof authority |
| `RadixWitnessMaterializationRecordV2` | Original replay record/source receipt/mapping, radix context, completed authenticated reread schedule, compact radix snapshot and geometry | None; use this materialization record, not a later proof seal |
| Source-algebra prerequisite | Ordered original ciphertext manifests/public-read receipts, source/output lineage, validated original materialized accumulators, preflight and static aggregate-schedule description | No current native proof result; the geometry remains 38 limbs |
| Source publication receipt | Original provider/writer identity/context/mapping, source snapshot identity, actual main/nonce ciphertext digests and lengths, ordered stored-record topology, false completion flags | None; these are local storage identities |

`SourceAlgebraPreflightV2::freeze_v2` creates its prerequisite before replay.
`aggregate_schedule_digest_v2` hashes the fixed formula/order language and
existing lineage; it does not derive `gamma_lk` or `beta_lk`, consume current
relation commitments, or import the current native proof's final aggregation
envelope. `phase23_bundle_digest_from_frames_v1` binds the original profile,
roster, prior share-release transcript, batch/shape/materialized digest, source,
key authority and 43 original ciphertext manifests. These are prior producer
dependencies, not a current-proof successor.

There are two material limitations. First, `SOURCE_ALGEBRA_LIMBS_V2 = 38` and
190 challenge pairs do not satisfy the native40 contract of 40 limbs/200 pairs.
The removal patch does not fix or authorize that lineage. Second,
`phase23_encrypted.rs::materialized_digest` hashes canonical private accumulator
scalars, and the source receipts transitively include locally encrypted
snapshots. These private/local identities must not be published or introduced
as new verifier-visible Fiat--Shamir inputs. An acyclic dependency trace alone
does not establish zero knowledge or transcript soundness.

## Bounded removal and remaining gates

Remove only the unused canonical-reopen child, its private weighted sink/seal,
unconsumed record/hash helpers, its unexecuted future I/O/heap counters, and its
axis in the private plane context. Preserve actual source-opening I/O
350,120,448 bytes, blinding spool accounting, all source commitments/masks and
the entire original ownership/zeroization path. Fixed 512 MiB/16 GiB/64 GiB
ceilings remain unchanged. Removal of an unused pass is not a new full-proof
resource qualification.

Update the source-opening record KAT and private plane-context/ordered-plan KAT;
there is no compatibility decoder. Preserve all retained proof/authority/release
false gates. Replace the removed future-counter mutation cases with actual
first-pass/blinding-counter mutations and check zero/substitution for every
remaining context axis. The deleted sink-only test and deleted child source-size
assertions have no surviving implementation to qualify.

Still required: authenticated native40 source production with the original
governed key/source owner; exact source-to-two-file writer custody; all 33
successful writes before producer advance; actual authenticated role replay and
matching original point/rho equations; a 1,376-slot derived mask owner for the
existing source/packing prover; Q-mask and subsequent original inventory
producers; qPCS/composite admission; full-session shared resource ownership;
maximum-size/resource/adversarial qualification. The ordinal-only plane replay
is not authenticated proof input. `RadixWitnessProofBindingV2::finish_v2` still
discards the comparator cursor while its production seal remains uninhabited;
that transition needs a separate reviewed completion requirement.

## Validation identity

`target/first-release-mkhe-reopen-removal-identity.json` records original and draft
file hashes; the separate applied source manifest records the frozen test candidate. `target/prepare-mkhe-reopen-removal-patch.py` independently encodes
the exact relevant bytes with Python Keccak, first reproduces both original
KATs, and only then derives the new expected bytes. That is a draft consistency
check, not Rust runtime evidence. Coordinated post-application tests must cover
`source_openings_v1::tests`, `source_openings_v1::mapping`,
`incremental_source_phase23_source_algebra::tests`, the vector-plane parent and
ordered-storage tests, and unchanged source/packing same-opening controls.
