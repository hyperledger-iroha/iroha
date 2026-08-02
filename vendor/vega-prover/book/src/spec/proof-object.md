# The proof object

This chapter specifies the byte-exact layout of `VegaMcZkSNARK`, the value that \\(\mathrm{Vega}\_{\mathrm{MC}}\\)'s `verify` procedure consumes. The proof is the [bincode serialization](serialization.md) of this struct, applied recursively until every field resolves to a primitive. All primitive encodings — scalars, group elements, sequences, options, and tuples — live in [Serialization](serialization.md); this chapter specifies structure: which fields exist, in which order, and of which type.

The proof path contains no user-defined enums — so the enum variant-index encoding never appears — and no cached or skipped fields. The only sum types are the `Option<Commitment>` values: `comm_W_shared` at the top level and the two `Option<Commitment>` fields inside each `SplitR1CSInstance`, all of which use the one-byte [Option tag encoding](serialization.md#rust-shaped-values). Every other field contributes bytes unconditionally.

## Single-field struct transparency

Several types in the proof tree are single-field structs. The struct rule in [Serialization](serialization.md) states that a single-field struct serializes exactly as its field, with no extra tag, length prefix, or padding. Treating these as opaque containers and inserting framing bytes produces incorrect output. The affected types are:

| Type | Wire bytes identical to |
| --- | --- |
| `HyraxCommitment` | its `Vec` of group elements |
| `HyraxEvaluationArgument` | its `InnerProductArgumentLinear` |
| `SumcheckProof` | its `Vec<CompressedUniPoly>` |
| `CompressedUniPoly` | its `Vec` of scalars |
| `NovaNIFS` | its `Commitment` |

A `Commitment` is a type alias for `HyraxCommitment`, so every commitment in the proof is transparently a length-prefixed vector of 33-byte compressed group elements. The [commitment example](serialization.md#commitment-example) in the serialization chapter works through the encoding in full.

## Top-level object: `VegaMcZkSNARK`

The proof has eight fields in this declaration order:

| Field | Type | Role |
| --- | --- | --- |
| `comm_W_shared` | `Option<Commitment>` | Shared witness commitment, stored once at the top level; `None` when no witness is shared. |
| `step_instances` | `Vec<SplitR1CSInstance>` | One R1CS instance per step circuit execution. |
| `core_instance` | `SplitR1CSInstance` | R1CS instance for the aggregated core circuit. |
| `eval_arg` | `HyraxEvaluationArgument` | Hyrax batch evaluation argument. |
| `U_verifier` | `SplitMultiRoundR1CSInstance` | R1CS instance for the in-circuit verifier. |
| `nifs` | `NovaNIFS` | Non-interactive folding proof (a single commitment). |
| `random_U` | `RelaxedR1CSInstance` | Fresh random relaxed-R1CS mask, sampled once per proof; the first input (`U1`) to the Nova NIFS fold, not the folded result. |
| `relaxed_snark` | `RelaxedR1CSSpartanProof` | Relaxed Spartan proof over the folded instance. |

```text
VegaMcZkSNARK =
     comm_W_shared
  || step_instances
  || core_instance
  || eval_arg
  || U_verifier
  || nifs
  || random_U
  || relaxed_snark
```

**Shared-witness optimization.** When several step circuits share a common witness prefix, its commitment is identical across every step instance and the core instance. The prover sets the top-level `comm_W_shared` to a clone of the *first* step instance's `comm_W_shared`, then sets the per-instance `comm_W_shared` to `None` in every element of `step_instances` and in `core_instance`. In the serialized proof, each instance's `comm_W_shared` is therefore always the single tag byte `00`; the shared commitment, when present, appears only at the top level. When the circuits declare no shared witness prefix — as in the cubic fixtures, where `num_shared == 0` — the first step instance's `comm_W_shared` is itself `None`, so the top-level field is also `None` (a lone `00` tag byte) and no shared-commitment bytes appear anywhere in the proof. The verifier copies the top-level value back into each instance before checking.

## R1CS instance types

### `SplitR1CSInstance`

Each element of `step_instances` and the `core_instance` field are of this type. Five fields in declaration order:

| Field | Type | Role |
| --- | --- | --- |
| `comm_W_shared` | `Option<Commitment>` | Always `None` in the proof; the shared commitment is hoisted to the top level (see above). |
| `comm_W_precommitted` | `Option<Commitment>` | Pre-committed column commitment; `None` if not present. |
| `comm_W_rest` | `Commitment` | Commitment to the remaining witness columns. |
| `public_values` | `Vec<Scalar>` | Public input scalars. |
| `challenges` | `Vec<Scalar>` | Verifier challenges for this instance. |

```text
SplitR1CSInstance =
     comm_W_shared        (Option<Commitment>)
  || comm_W_precommitted  (Option<Commitment>)
  || comm_W_rest          (Commitment)
  || public_values        (Vec of Scalar)
  || challenges           (Vec of Scalar)
```

Each `Option<Commitment>` follows the [Option encoding](serialization.md#rust-shaped-values): one tag byte (`00` = `None`, `01` = `Some`), then the commitment bytes if the tag is `01`.

### `SplitMultiRoundR1CSInstance`

The `U_verifier` field is of this type. Three fields:

| Field | Type | Role |
| --- | --- | --- |
| `comm_w_per_round` | `Vec<Commitment>` | Witness commitments, one per round. |
| `public_values` | `Vec<Scalar>` | Public input scalars. |
| `challenges_per_round` | `Vec<Vec<Scalar>>` | Verifier challenges grouped by round. |

```text
SplitMultiRoundR1CSInstance =
     comm_w_per_round     (Vec of Commitment)
  || public_values        (Vec of Scalar)
  || challenges_per_round (Vec of Vec of Scalar)
```

`challenges_per_round` is a `Vec` of `Vec`s. The outer `Vec` carries an 8-byte `u64` count; each inner `Vec` carries its own 8-byte count followed by its scalars. There is no extra framing between the inner vectors beyond what the `Vec` rule provides.

### `RelaxedR1CSInstance`

The `random_U` field is of this type. Four fields:

| Field | Type | Role |
| --- | --- | --- |
| `comm_W` | `Commitment` | Witness commitment. |
| `comm_E` | `Commitment` | Error-vector commitment. |
| `X` | `Vec<Scalar>` | Public input vector. |
| `u` | `Scalar` | Relaxation scalar. |

```text
RelaxedR1CSInstance =
     comm_W  (Commitment)
  || comm_E  (Commitment)
  || X       (Vec of Scalar)
  || u       (Scalar, 32 bytes)
```

## Evaluation argument

### `HyraxEvaluationArgument`

`HyraxEvaluationArgument` is a single-field struct whose sole field is `ipa : InnerProductArgumentLinear`. Its wire bytes are those of the `InnerProductArgumentLinear` directly, with no wrapper framing.

### `InnerProductArgumentLinear`

Five fields:

| Field | Type | Role |
| --- | --- | --- |
| `delta` | group element | First blinding commitment; a bare 33-byte compressed point. |
| `beta` | group element | Second blinding commitment; a bare 33-byte compressed point. |
| `z_vec` | `Vec<Scalar>` | Response vector. |
| `z_delta` | `Scalar` | Scalar response for `delta`. |
| `z_beta` | `Scalar` | Scalar response for `beta`. |

```text
InnerProductArgumentLinear =
     delta   (33-byte compressed point, no length prefix)
  || beta    (33-byte compressed point, no length prefix)
  || z_vec   (Vec of Scalar)
  || z_delta (Scalar, 32 bytes)
  || z_beta  (Scalar, 32 bytes)
```

`delta` and `beta` are bare group elements, not `Commitment` structs: each is a single 33-byte compressed point with no preceding `u64` count. The point encoding is `flag || x(BE)` as specified in [Serialization](serialization.md#group-elements).

## Folding proof

### `NovaNIFS`

`NovaNIFS` is a single-field struct whose sole field is `comm_T : Commitment`. Its wire bytes are those of that commitment: an 8-byte `u64` count followed by compressed points.

```text
NovaNIFS = comm_T  (Commitment)
```

## Relaxed Spartan proof

### `RelaxedR1CSSpartanProof`

Seven fields in declaration order:

| Field | Type | Role |
| --- | --- | --- |
| `sc_proof_outer` | `SumcheckProof` | Sum-check proof for the outer reduction. |
| `claims_outer` | `(Scalar, Scalar, Scalar)` | Three claimed evaluations; a fixed tuple, exactly 96 bytes. |
| `sc_proof_inner` | `SumcheckProof` | Sum-check proof for the inner reduction. |
| `v_W` | `Vec<Scalar>` | Evaluation claims for the witness polynomial. |
| `blind_W` | `Scalar` | Blinding scalar for the witness opening. |
| `v_E` | `Vec<Scalar>` | Evaluation claims for the error polynomial. |
| `blind_E` | `Scalar` | Blinding scalar for the error opening. |

```text
RelaxedR1CSSpartanProof =
     sc_proof_outer   (SumcheckProof)
  || claims_outer.0   (Scalar, 32 bytes)
  || claims_outer.1   (Scalar, 32 bytes)
  || claims_outer.2   (Scalar, 32 bytes)
  || sc_proof_inner   (SumcheckProof)
  || v_W              (Vec of Scalar)
  || blind_W          (Scalar, 32 bytes)
  || v_E              (Vec of Scalar)
  || blind_E          (Scalar, 32 bytes)
```

`claims_outer` is a 3-tuple of scalars. Tuples carry no tag, length prefix, or padding (see [Serialization](serialization.md#rust-shaped-values)), so these three scalars occupy exactly 96 consecutive bytes with no intervening count.

### `SumcheckProof`

`SumcheckProof` is a single-field struct whose sole field is `compressed_polys : Vec<CompressedUniPoly>`. Its wire bytes are those of the `Vec` directly. For a sum-check with \\(r\\) rounds:

```text
SumcheckProof =
     u64_le(r)            (8-byte round count)
  || CompressedUniPoly_0
  || CompressedUniPoly_1
  || ...
  || CompressedUniPoly_{r-1}
```

### `CompressedUniPoly`

`CompressedUniPoly` is a single-field struct whose sole field is `coeffs_except_linear_term : Vec<Scalar>`. Its wire bytes are those of the `Vec` directly. A degree-\\(d\\) round polynomial has \\(d + 1\\) coefficients; the single linear (degree-1) coefficient is omitted, so exactly \\(d\\) coefficients are stored — the constant term followed by the degree-2 through degree-\\(d\\) coefficients:

```text
CompressedUniPoly =
     u64_le(d)     (number of stored coefficients = degree of the round polynomial)
  || Scalar_0      (constant term, degree 0)
  || Scalar_1      (degree 2)
  || ...
  || Scalar_{d-1}  (degree d)
```

The linear term (the coefficient of the degree-1 monomial) is absent from the proof. The verifier recovers it from the sum-check invariant for the round; no extra bytes are needed. Every stored coefficient is written as a 32-byte little-endian scalar per the [scalar encoding](serialization.md#scalar-field-elements).

---

The complete type tree established here, together with the primitive rules in [Serialization](serialization.md), fully determines the byte string for any `VegaMcZkSNARK` value. The companion chapter [A simple reference prover](reference-prover.md) shows how these fields are populated, and [Conformance and test vectors](test-vectors.md) provides byte-exact examples that an implementation can check against.
