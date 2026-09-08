# Touring staged integer range obligations

This is the source-coupled integer argument for `StagedClassedRaceAirV1`. It does
not qualify the commitment scheme, Fiat–Shamir transcript, DEEP/FRI soundness,
proof envelope, equipment entitlement, or a production profile. The stock graph,
shared integer compiler and registered profiles are unchanged.

The former blanket check added 160 ordinary, 78 Boolean and 222/223 radix-four
columns to **every** stage. `PackedIntegerAirV1` takes a separate maximum for
each bank, so even the boundary and one-car rows paid for both full car states.
The equations now check a value at its producer or canonical tick boundary.
No prover-chosen scheduling, unbound carry, or arbitrary input API is introduced.

## Explicit equations

- `ENVIRONMENT` range-checks relative distance in `[-length,length]`, object kind
  in `[0,2]` and object x in `[-7400,7400]` after the exact division and complete
  one-hot lookup equations derive them.
- `CURVE` range-checks the produced movement force in `[-126,126]`.
- Each car's `FINISH` row range-checks all seven outputs: progress in
  `[-12000,3*length]`, x in `[-15300,15300]`, speed in `[0,3300]`, lateral velocity
  in `[-320,320]`, boost energy in `[0,1000]`, finish tick in `[0,5400]` and encoded
  removal tick in `[0,5401]`. These checks also execute for frozen cars and
  disabled/padding ticks. They use the canonical bounds, including finish tick
  at most 5400 even though the unused padding expression `tick+1` can be 5401.
- The global residue function still equates the same 20 ordinary bank inputs
  to the fixed left/right selection of carried state and scratch. A nonexistent
  selected car is exactly zero. Independent Boolean and zero-test equations
  determine alive/unfinished flags; `running=enabled*alive*unfinished` makes each
  selected running input Boolean without another quotient/remainder check.
- Every car and scratch transition, genesis value, checkpoint and final-state
  equality is retained. Only the very last trace row has no successor equation;
  its own state is already bound by its predecessor. Public stage selectors
  continue throughout padding. Boolean/radix-four bank equations remain
  unconditional and the relation degree remains at most four.

The exact range gadget writes `shifted=value-low`, proves unsigned Euclidean
division by `high-low+1`, fixes its quotient to zero, and equates its remainder
to `shifted`. Remainder digits and the strict divisor comparison exclude both
negative representatives and power-of-two slack. Its `Value` endpoint metadata
is normalized only to construct those equations; that normalization is not the
range proof.

## Finite integer induction between checks

Start with the field encoding of the canonical grid and zero scratch, fixed by
the first-row equations. Assume the current carry is this encoding of the
reference state at its public microcycle. The stage's first 20 bank inputs are
then uniquely fixed. Its arithmetic equations force the following values:

1. `BOUNDARY` changes only the removal field, from zero to public `tick+1`, when
   its fixed removal selector and proved alive flag are both one. Replay
   validation bounds event ticks by 5400, yielding encoded removals at most
   5401. All other state is copied.
2. `ENVIRONMENT` receives progress between -12000 and the three-lap finish.
   Adding one track length is nonnegative. Unique Euclidean division and the
   complete 12-cell lookup determine the object scratch. Its explicit range
   checks occur before `GRIP` or `IMPACT` can consume it.
3. `GRIP` uses public Boolean rain and fixed six-bit decoded steering. Its exact
   signed divisions, oil comparisons and Boolean selectors derive the same
   damping as the reference, followed by a clamp to 280 or 320. Thus lateral
   velocity remains within 320. Frozen cars retain the previous bounded value.
4. `DRIVE` compares bounded energy with 25. Boost consumes 25 only when enough
   energy exists; otherwise a minimum with 1000 caps the recharge. Speed is
   clamped between zero and the selected 2640/3300 limit. Both carried outputs
   therefore remain in their intervals.
5. `CURVE` derives curvature from old progress and the public table. Its
   magnitude is at most three. Public wind has magnitude at most 32 and speed
   at most 3300, so the sum of truncated forces is at most
   `floor(3*3300/120)+floor(32*3300/2400)=82+44=126`. The output is explicitly
   range-checked; an inactive car instead produces zero.
6. `MOVE` adds the bounded lateral velocity and force, clamps x to 9000, and
   subtracts an off-road penalty without allowing negative speed. A running
   car had not finished, so adding speed increases progress by at most 3300;
   the intermediate progress bound is `[-12000,finish+3300]`. A frozen car
   remains at its previous canonical progress.
7. `IMPACT` uses the same earlier object geometry and the exact swept-distance
   predicate. Speed either stays fixed or decreases to at least zero. The only
   x replacement is clamped to 9000. No progress is changed.
8. `CONTACT` is visited in the immutable ascending pair order, at most once
   for any pair. A collision needs both proved running flags. Its overlap is
   positive only below separation 1800, so its rounded half-push is at most
   900. Every running car starts this phase at absolute x at most 9000 and
   has at most seven contacts: after each individual pair its absolute x is
   at most `9000+900*visited_contacts <= 15300`. Inactive cars never contact
   and keep their prior bound. Speed only decreases and progress is unchanged.
9. `FINISH` uses the bounded intermediate progress and running flag. A crossing
   clamps progress to the finish and writes the public tick plus one. Enabled
   driving ticks are 0 through 5399, so an actual finish is at most 5400.
   Uncrossed active progress is below the finish; inactive state stays fixed.
   Explicit checks on all seven outputs re-establish canonical state bounds
   before the next tick.

Every intermediate operation has its closed interval below `2^50`, enforced by
the compiler's graph-construction bounds. For a comparison, the Boolean top bit
of the bounded `a-b+2^k` decomposition uniquely selects its sign. For unsigned
division, the quotient digit decomposition permits less than twice its nominal
maximum plus one, but even that actual slack gives a reconstruction below
`2*value.high+2*divisor`, far below the Goldilocks modulus. The remainder is
strictly less than the divisor. Hence the equation cannot choose another
quotient by wrapping modulo the field. Signed division first proves the sign,
divides the exact absolute value and restores the sign. Boolean selections are
therefore selections, not interpolation by unconstrained field elements.

These local uniqueness arguments and the predecessor equality exclude a first
incorrect microcycle. A forged consumer that recomputes all its auxiliaries
still conflicts with the producer transition. Omitting a FINISH check or
restarting at an unproved intermediate state is not an admitted relation.

After replay inputs end, `enabled=0` fixes every running flag to zero. An optional
final removal still follows the public boundary rule. Car state is copied
through the reserved terminal cycle and padding, while geometry remains derived
and force is zero. Genesis, terminal and checkpoint equalities are not replaced
by this induction.

## Required validation

The retained tests cover exact seven-field lower/upper-bound rejection after
recomputing auxiliaries; coherent forged car/scratch consumers; all controls,
weather/object boundaries, contacts and ghosts; all three tracks and rosters;
independent polynomial degree lines; and complete 8-car, 5400-tick reference and
residue comparisons including terminal/padding rows. Geometry remains 85 rows
per tick and 524288 padded rows for a maximum race. A native test run must record
the resulting widths; arithmetic tests do not establish a proof-size or
cryptographic security claim.
