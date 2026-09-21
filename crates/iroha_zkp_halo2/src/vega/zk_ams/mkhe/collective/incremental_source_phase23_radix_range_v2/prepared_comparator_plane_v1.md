# Prepared comparator values and commitments

`Phase23RadixWitnessMaterializedV2::prepare_next_comparator_plane_v1` consumes
its original sealed radix source. Its private cursor selects the next plane.
Source lineage, materialization record, mapping, context, seal, snapshot and
exact retained-session stage are validated before allocation or packed-slot I/O.
The immutable public mapping is cached; source validation and identities are not.

The actual comparator value mapping remains:

| Ordinals (exclusive end) | Logical role | Packed source |
| --- | --- | --- |
| 0–344 | `bD[group]` | lane 0, bit 0 |
| 344–688 | `bS[group]` | lane 0, bit 1 |
| 688–6880 | `beta[group][0..18]` | lane 0 bits 2–7, lane 1 bits 0–7, lane 2 bits 0–3 |
| 6880–7224 | `m[group]` | lane 2, bit 4 |

The first consuming commitment stage covers the 688 top-bit planes.
It starts from the actual completed D/S owner at native inventory 12040: 344 bD
points occupy 12040–12384 and 344 bS occupy 12384–12728. Coordinates come from the
shared native40 inventory. The next physical purpose is 5,848 delta commitments;
preparation at comparator ordinal 688 rejects before I/O until the difference
producer has completed that exact predecessor. It cannot jump directly from
bS to beta at inventory 18576.

Completed D/S opening material, its single append permit and the original
session are moved into the private top-commitment owner. D/S validation retains
its exact completed cursor; it is not weakened to an arbitrary later position.
Before any value chunk is emitted, the same retained original RNG samples rho
through the existing bounded nonzero canonical sampler. The actual prepared
16,384 values feed the existing secret MSM in `G[0..16384]` order plus exactly
one `rho*H`. Only this internally computed point enters the corresponding empty
inventory slot. Rho remains in a zeroizing vector owned by the same session.
No point, blinding, ordinal, entropy or source replacement is accepted from a
caller. The preallocated inventory is reused; 688 retained scalar blindings add
22,016 bytes of named payload, within the existing full-inventory allocation.
This is not a whole-proof memory or hardware qualification.

The compact slot is `3*group+lane` for 344 groups. Its coordinate is already
`v=coefficient*64+block`; expansion performs no second transpose and retains all
16,384 polynomial-coefficient coordinates. A zero coefficient gives `bS=1` and
some nonzero borrow bits. Logical-slot zeros are separately validated after the
packing owner's NTT decode and do not imply zero coefficient tails. All lane-2
reserved high bits must be zero, including the final coefficient coordinate.

The prepared owner retains the source and one existing zeroizing scalar vector,
whose exact capacity is fallibly reserved. It emits 32 ordered crypto-owned chunks
of 512 canonical big-endian T256 scalars each. The scalar payload is 524,288 bytes;
a packed input or returned value chunk adds 16,384 bytes. Callers own returned
chunks; retaining many chunks is outside this owner's one-chunk bound. Wrong,
repeated, skipped or extra chunk requests and incomplete finish consume/drop
all owned state. The original admitted rho/point follows the 32 value chunks as
one canonical 16,384-byte tail: `rho_BE32 || point33 || zero-padding16319`.
Its opaque scalar guard adds 32 bytes plus the public point/ordinal while values
are live; the vector is erased before tail-chunk allocation. Finish advances the
private value cursor only after all 33 slots, including exactly one tail.
Entropy, validation, MSM and adoption errors likewise cannot recover the session.

Emitted chunks do not establish storage, authenticated complete-pair ownership or
an opening proof. The opaque tail comes from the same admitted original
inventory and retained rho; no scalar getter or separately sampled value is
added. The [ordered storage handoff](ordered_storage_handoff_v1.md) now consumes
that complete sequence through the same source ownership chain. When a writer
is attached, emitted-but-unwritten chunks cannot advance this source. Upstream
native40 source/context authority,
all remaining native40 roles and actual proof/release admission remain required.

Tests exercise real compact-bit expansion, sparse public group equations,
matching original-session sampled rho/point, ordered native roles and consuming
failure/drop boundaries. The existing full-shape patterned D/S inventory fixture
is not live authenticated source proof. No positive source authority is fabricated.

The [difference-digit driver](prepared_difference_digit_plane_v1.md) now
consumes this exact completed top stage and produces delta commitments at
12728–18576 using the same retained session and original source. Its completion
returns the original source at beta. The retained comparator continuation now
requires that exact completed delta owner before the sealed-value preparer can
resume at logical ordinal 688. It fills 6,192 beta and 344 m commitments in
physical inventory order 18576–25112 using the same original entropy, actual
prepared-value MSM and retained rho. The 209,152-byte blinding vector retains
all earlier source/D/S/top/delta openings and the original append permit.
Neither an incomplete delta stage nor a skipped/repeated comparator ordinal
can advance. Final m hands off at `SmallSigned` ordinal 25112; the
[signed-source producer](prepared_small_signed_plane_v1.md) continues through
ordinal 27176. No composite proof authority is added. The fixed native descriptor,
9,288-plane storage plan and resource ceilings remain unchanged.

Continuation tests check exact beta/m physical coordinates, incomplete/wrong
stage refusal, original opening custody, actual first/last beta and first/last m
MSMs, final signed-stage refusal, and failure/drop boundaries. Prior inventory
points remain explicitly scoped fixtures, not full-source proof evidence.

TODO: connect the complete source-consuming plane writer to these 33-slot
sequences and the authenticated ordered pair. Full 688 actual
commitment execution, full-source positive tests and whole-proof resource evidence
remain separate qualification work.

The 2026-09-20 original delta pass covered 12 focused tests. The later frozen
delta/beta candidate passed all 17 focused cases and 26 adjacent controls; the
subsequent signed candidate passed all 15 focused cases. The added opening-tail
slice requires its own candidate-specific runtime validation.
Full-source, full-size resource and composite proof qualification remain open.
