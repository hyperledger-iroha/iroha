# Prepared D/S low-digit values

`Phase23RadixWitnessMaterializedV2::into_low_digit_preparation_v1` consumes the
original compact witness and source evidence before beginning a fresh strict
canonical-source read. Its entry validates the original replay/source lineage,
materialization record, exact mapping-derived context, seal and snapshot. It
requires comparator cursor zero and the exact source-complete session stage.
The completed source prefix binds the actual source record, original session and
opening contexts, source points and sealed source-blinding snapshot. Its
validation remains mandatory as that same session advances through later stages;
it is not a lower-bound test on a mutable cursor.

The driver itself selects each ordinal. It uses the existing candidate order
`((group * 2 + role) * 17 + digit)`, with 344 groups, D then S, and digits 0–16.
This is 11,696 planes. The existing purpose-major commitment inventory is a
separate owner. Before any value emission, the original retained session samples
the purpose-bound nonzero blinding and computes `C = sum(values[v] * G[v]) + rho * H`
through the existing secret MSM with the full canonical generator prefix. It
adopts only that computed point and retains rho; neither is supplied by a caller.

Each group reads the next 64 authenticated canonical blocks through the same
source cursor used by compact materialization. All 43 × 512 blocks are read once
per complete invocation. One group retains 16,384 canonical T256 scalar values in
source `(block, coefficient)` order. Projection emits the existing
`v = coefficient * 64 + block` order exactly once. For each source value D,
`S = p_T - 1 - D`; digit j is `floor(value / 2^(15*j)) mod 2^15`.
The bit at position 255 is excluded. All polynomial-coefficient coordinates are
emitted: a zero D coefficient has zero D digits but the S digits of `p_T - 1`.
The zero-coefficient test is an arithmetic boundary case. Logical-slot padding
is validated after the original packing owner's NTT decode and does not imply
zero polynomial-coefficient tails. This producer preserves that authenticated
source authority and does not introduce a second padding-validation policy.

The shared private `PreparedRadixValuesV1` owns the exact 16,384-value plane and
emits 32 ordered chunks of 512 canonical big-endian scalars. It is also used by
the comparator producer. Neither producer exposes a scalar-vector getter,
caller-selected values, point, blinding, proof token or tuple split. A wrong,
repeated, skipped or extra chunk poisons the entire retained outer owner. An
incomplete finish consumes it. The driver advances only after all 32 chunks and
drops its retained group after both sets of 17 planes.

After all 11,696 planes, the driver consumes the canonical cursor's complete
read schedule, requires equality with the original materialization's schedule
root, revalidates original source/context ownership, and returns the sole
original compact source together with the internally retained completed D/S
candidate owner. Restarting this preparation is rejected by its exact session
stage. A consumer may discard emitted chunks: no plane storage, canonical 33rd
slot, source-opening proof or full materializer completion is inferred.

Every production scalar vector reserves its final checked capacity before
secret pushes; the exact bounded loops cannot grow it. Named live payloads are
one 524,288-byte source group and one 524,288-byte projected plane. Group loading
also holds one 8,192-byte crypto source chunk; emission holds one 16,384-byte
crypto value chunk instead. Scalar encoding/complement and extracted-bit scratch
reuse the existing radix zeroizing owners. Returned chunks belong to the caller
and zeroize on drop. Source, group and plane remain owned across all errors and
unwinds. These allocation counts are not a process RSS or side-channel claim.
Full replay reads 180,355,072 plaintext bytes plus 352,256 authentication bytes;
full emitted values total 6,132,072,448 bytes. Existing resource caps are unchanged.

The additional D/S candidate owner uses the existing checked 11,696-blinding
allocation and original inventory; the canonical secret MSM owns its 16,385
terms and erases its private scalar/point scratch. This is separate from the two
value buffers above. Error or unwind takes and drops the sole session before
sampling, MSM or adoption; it cannot retry with a replacement session.

The actual materializer retains its original concrete fallible RNG. Authenticated
source replay consumes that same owner into the commitment session; D/S retains
its lifetime, sampling cursor and failure custody. The purpose-specific handoff
accepts a validated source owner, with no detached RNG or caller-selected context.
Deterministic entropy exists only in isolated unit fixtures.

TODO: connect the existing Core health-checked randomness owner and actual MKHE
correspondence producer to this private path. The current Core credential route
is separate; retaining a supplied RNG does not establish that missing authority.
Next owners must commit comparator values, consume the actual delta predecessor,
and store exact canonical tails before the remaining native40 signed planes and
proof can complete. This change introduces no second RNG, detached point
constructor or accepted source fixture.
Tests cover nonzero actual low values and their independently calculated sparse
group equation, stage progress and failure custody. Full source-to-proof positive
qualification remains separate from those bounded tests.
