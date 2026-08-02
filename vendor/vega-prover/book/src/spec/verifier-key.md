# Verifier key and digest

The verifier key is the public parameter set that `verify` consumes. It is produced once by setup from the step and core circuits and the canonical curve, and it is shared unchanged by the prover and the verifier. The verifier key is **not** serialized into the proof. Its role in the byte-exact contract is indirect but decisive: a 32-byte **digest** of the verifier key is the first value absorbed into the Fiat--Shamir transcript, so every later challenge depends on it. A byte-equivalent prover must therefore compute the identical digest, which requires both the identical verifier-key contents and the identical digest algorithm specified here.

## Verifier key contents

The verifier key is a structure with nine serialized fields, in this declaration order:

| Field | Type | Role |
| --- | --- | --- |
| `ck` | commitment key | Hyrax generators used to commit witness columns. |
| `vk_ee` | PCS verifier key | Hyrax evaluation-argument verifier parameters. |
| `S_step` | split R1CS shape | Constraint-system shape of the step circuit. |
| `S_core` | split R1CS shape | Constraint-system shape of the core circuit. |
| `vc_shape` | split multi-round R1CS shape | Shape of the in-circuit verifier circuit. |
| `vc_shape_regular` | R1CS shape | The verifier circuit's shape as a single-round R1CS. |
| `vc_ck` | commitment key | Generators for the verifier circuit's witness. |
| `vc_vk` | inner verifier key | Verifier key of the single-circuit relaxed proof. |
| `num_steps` | `usize` | Number of step instances (at least two). |

A tenth field caches the digest once computed; it is marked non-serialized and never contributes bytes.

The contents of these fields are fixed by setup and fully reproducible: the same step circuit, core circuit, `num_steps`, and canonical curve produce the same verifier key, down to the byte. That determinism is a security property rather than a convenience. The verifier key is the protocol's root of trust — `verify` must be run against the one canonical key for a statement, not an arbitrary key handed over with a proof — and its commitment generators are nothing-up-my-sleeve points obtained by hashing fixed labels to the curve (see [Setup](../mc/setup.md#commitment-material)), so no party knows a discrete-logarithm relation among them. Because the derivation is fully determined, an independent implementation can regenerate this key and check it rather than take it on trust; the reference prover does exactly that. This chapter does not respecify setup; it specifies how the digest is computed from an existing verifier key.

To load an existing key rather than regenerate it, note that the serialized `vk.bin` is the ordinary bincode serialization of these nine fields in declaration order — identical to the digest stream below except that `S_step` and `S_core` appear in their ordinary bincode split-R1CS-shape form instead of the `shape_raw` encoding, and the cached digest field is skipped. The byte-critical requirement for a byte-equivalent prover is the digest specified here, so this chapter pins that stream exactly; an implementation that needs to parse `vk.bin` itself can follow the reference `vk.py`, or simply regenerate the key via [Setup](../mc/setup.md) and check its digest.

## The digest

The digest is the SHA-256 hash of a single byte string \\(D\\) built from the verifier-key fields. The hash input is streamed field by field; there is no domain-separation tag, length prefix, or trailing marker around \\(D\\).

```text
digest = SHA-256(D)          // 32-byte output

D = bincode(ck)
 || bincode(vk_ee)
 || shape_raw(S_step)
 || shape_raw(S_core)
 || bincode(vc_shape)
 || bincode(vc_shape_regular)
 || bincode(vc_ck)
 || bincode(vc_vk)
 || bincode(num_steps)
```

This order is fixed by the verifier key's digest routine, which writes the fields in the declaration order above; a byte-equivalent prover must reproduce this exact sequence rather than rely on any automatic struct serialization. Two encodings appear:

- `bincode(v)` is the little-endian fixint encoding of [Serialization](serialization.md), applied recursively to the value's type. `bincode(num_steps)` is therefore an 8-byte little-endian `u64`. This is the raw number of step instances passed to setup; it is **not** rounded up to a power of two. The power-of-two padding used elsewhere in the protocol affects only the number of folding rounds, never this stored count.
- `shape_raw` is a compact hand-written encoding used only for the two circuit shapes. It is **not** bincode: it exists so the large constraint-system matrices hash quickly, and it is the reason the digest is a mix of two encodings rather than one uniform bincode call.

The nested layouts of the bincode-encoded fields (`ck`, `vk_ee`, `vc_shape`, `vc_shape_regular`, `vc_ck`, `vc_vk`) are pinned field by field in [Bincode object layouts](#bincode-object-layouts) below, using the primitive, sequence, field, and point encodings from [Serialization](serialization.md).

## Shape and matrix raw encoding

A split R1CS shape hashes as ten dimension counts followed by its three constraint matrices:

```text
shape_raw(S) =
     u64_le(S.num_cons)
  || u64_le(S.num_cons_unpadded)
  || u64_le(S.num_shared_unpadded)
  || u64_le(S.num_precommitted_unpadded)
  || u64_le(S.num_rest_unpadded)
  || u64_le(S.num_shared)
  || u64_le(S.num_precommitted)
  || u64_le(S.num_rest)
  || u64_le(S.num_public)
  || u64_le(S.num_challenges)
  || matrix_raw(S.A)
  || matrix_raw(S.B)
  || matrix_raw(S.C)
```

Each matrix is stored in compressed sparse row form: a `data` array of nonzero field values, a parallel `indices` array of column indices, an `indptr` array of row-start offsets, and a `cols` column count. It hashes as three lengths and the column count, followed by the three arrays:

```text
matrix_raw(M) =
     u64_le(len(M.data))
  || u64_le(len(M.indices))
  || u64_le(len(M.indptr))
  || u64_le(M.cols)
  || for d in M.data:    le32(d)      // scalar-field value, 32 little-endian bytes
  || for i in M.indices: u64_le(i)
  || for p in M.indptr:  u64_le(p)
```

The `data` values are scalar-field elements written with the same little-endian 32-byte encoding as a scalar on the wire ([Serialization](serialization.md#scalar-field-elements)); `to_repr` for the scalar field is little-endian. The `indices` and `indptr` entries are machine-word indices written as 8-byte little-endian `u64` values. All four length and count headers are 8-byte little-endian `u64` values, even though the underlying values are register-sized.

## Bincode object layouts

The six bincode-encoded fields of the digest — `ck`, `vk_ee`, `vc_shape`, `vc_shape_regular`, `vc_ck`, and `vc_vk` — expand by the recursive [serialization](serialization.md) rules. This section pins each type's field order so the expansion is unambiguous. Throughout, a `usize` is an 8-byte little-endian `u64`, a `Vec<T>` is an 8-byte little-endian length followed by its elements, a group element is a 33-byte compressed point, and a scalar is 32 little-endian bytes.

### Commitment and verifier keys

`ck` and `vc_ck` are commitment keys; `vk_ee` and `vc_vk` are the matching PCS verifier keys. Both types encode the same three fields, in this order:

| Field | Type | Meaning |
| --- | --- | --- |
| `num_cols` | `usize` | row width |
| `ck` | `Vec<point>` | column generators \\(G\_0, G\_1, \dots\\) |
| `h` | point | hiding base \\(H\\) |

A commitment key additionally holds two precomputed fixed-base tables; both are `#[serde(skip)]` and emit no bytes, so a commitment key and a verifier key of the same width serialize identically. Because `vk_ee` reuses `ck`'s generators and `vc_vk` reuses `vc_ck`'s, `bincode(vk_ee)` equals `bincode(ck)` and `bincode(vc_vk)` equals `bincode(vc_ck)`, byte for byte.

### Verifier-circuit shapes

`vc_shape` is a split multi-round shape; `vc_shape_regular` is the same constraint system viewed as a single-round R1CS shape. Their fields serialize in these orders:

| `vc_shape` field | Type |
| --- | --- |
| `num_cons` | `usize` |
| `num_cons_unpadded` | `usize` |
| `num_rounds` | `usize` |
| `num_vars_per_round_unpadded` | `Vec<usize>` |
| `num_vars_per_round` | `Vec<usize>` |
| `num_challenges_per_round` | `Vec<usize>` |
| `num_public` | `usize` |
| `commitment_width` | `usize` |
| `A`, then `B`, then `C` | `SparseMatrix` |

| `vc_shape_regular` field | Type |
| --- | --- |
| `num_cons` | `usize` |
| `num_vars` | `usize` |
| `num_io` | `usize` |
| `A`, then `B`, then `C` | `SparseMatrix` |

Both types also carry a `#[serde(skip)]` cached digest that emits no bytes.

### Bincode sparse matrix

Inside the two verifier-circuit shapes, each matrix is bincode-encoded, which is **not** the `matrix_raw` layout used for `S_step` and `S_core`. The bincode form length-prefixes each array inline and places `cols` last:

```text
bincode(M) =
     u64_le(len(M.data))    || for d in M.data:    le32(d)
  || u64_le(len(M.indices)) || for i in M.indices: u64_le(i)
  || u64_le(len(M.indptr))  || for p in M.indptr:  u64_le(p)
  || u64_le(M.cols)
```

The `matrix_raw` form above instead groups all four counts first and then the three arrays. Both carry the same numbers; `S_step`/`S_core` use `matrix_raw` in the digest, while `vc_shape`/`vc_shape_regular` use this bincode form.

## The digest in the transcript

The 32-byte SHA-256 output is the digest. It enters the transcript as raw bytes:

```text
transcript.absorb("vk", digest)
```

The digest's transcript encoding is its 32 bytes in order, with no reversal and no field reduction: it is hashed as an opaque byte string, not as a field element. The exact position of this absorb in the schedule is specified in [The transcript schedule](transcript-schedule.md).
