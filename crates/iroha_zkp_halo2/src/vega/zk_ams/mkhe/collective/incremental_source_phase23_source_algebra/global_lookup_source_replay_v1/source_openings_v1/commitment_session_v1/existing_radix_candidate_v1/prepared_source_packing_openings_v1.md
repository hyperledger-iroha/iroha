# Retained source/packing opening preparation

This child prepares the existing source/packing equation's masks inside the
original source/session. It does not construct a native40 governed source, a
qPCS mask, a same-opening proof or a production provider capability.

The sole input is the actual completed `RnsNativeSmallSignedCommitmentsV1`
owner. Its source-facing caller additionally requires the materialized source
cursor9,288, which can be returned only after every preceding prepared plane
emits its32 values and original33rd-slot tail. Emission is not durable storage;
the ordered writer/reopen obligations remain separate.

The producer revalidates the completed source prefix and exact final signed
position, consumes the original session before fallible work, and prepares
exactly1,376 scalars:

```text
for g = 0..344:
    rho_D[g] = sum(h = 0..17, (2^15)^h * rho_Dlow[g,h])
             + (2^15)^17 * rho_bD[g]
for u = 0..1032:
    rho_signed[u] = original rho_x[u]
    u = (record * 3 + role) * 8 + plane, role = r,e0,e1
```

The low-mask array is group-major (`D[17]` then `S[17]`), while physical inventory
is purpose-major. The canonical existing coordinate function connects those
axes: mask index`34*g+h`, inventory index`344+17*g+h`. Top masks use the actual
`bD` entries, never `bS`; signed masks use the first1,032 original signed entries,
never the negative-magnitude half. Original source-order `Csrc` masks cannot
substitute for any derived D mask.

Every referenced point ticket must retain its exact original coordinate and
canonical nonidentity encoding. The single shared point-root traversal in
`rns_native_source_packing_same_opening.rs` reconstructs D points with the same
weights, admits derived identity points as required by the existing equation,
and hashes the same ordered raw and derived points as before. The prepared
owner uses its streaming mode, retaining no second point vector or inventory.
No transcript literal, domain, challenge, source-context axis or proof frame is
added or changed.

The original inventory, entropy, low/top/delta/beta/m/signed/negative masks and
source are retained. The new private material contains a zeroizing1,376-scalar
vector (44,032 payload bytes) and public point root. It has no external
constructor, getter, serialization or detached-provider conversion. Preparation
is one-shot; error, unwind or a repeated call consumes the original owner and
erases its secret vectors. No rho is sampled or replaced. The next physical
inventory position remains`QMaskDigit/0` at27,176.

The scalar vector reserves its exact capacity fallibly. Streaming point hashing
adds no point-vector allocation; all curve/field operations reuse the existing
deterministic T256 implementation. These named payload counts do not measure
allocator/control overhead, peakRSS or whole-proof lifetime. The eventual
whole-session resource owner must charge this retained44,032-byte allocation and
actual derivation/hash work together with every other consumer. Existing
512MiB/16GiB/64GiB limits and all qualification flags are unchanged.

Seven focused tests cover all coordinate mappings, retained original allocation
identity and inventory bytes, independent reverse-Horner mask reconstruction,
exact weighted commitments on the real T256 G/H basis at the first and last D
groups, honest cancelling masks, missing/reordered/malformed tickets, partial-stage refusal, repeated
consumption and the shared scalar-owner erasure counter. Synthetic prior
inventory in these tests is expressly not authenticated production source
coverage; the actual sparse commitment equations do not qualify the full
native40 relation or complete inventories' MSM production.

TODO: connect the prepared material to the genuine native40 authenticated source
context/replay schedule and later direct/membership prover owner, without raw
digest or foreign inventory adapters. `PRODUCTION_DERIVED_MASK_OWNER_AVAILABLE_V1`
and its production seal remain false/uninhabited. The existing38-limb source
lineage is not a40-limb qualification. Governed full40 parameters/keys and source,
Q-mask coefficient sampling/lifecycle, actual ordered stored-opening writer and
reopen, qPCS/composite admission, whole-session resources and independent
security/release evidence remain open.
