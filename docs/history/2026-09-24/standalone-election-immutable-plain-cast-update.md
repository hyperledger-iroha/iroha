# F11 public ballot cast and conviction update

The first-release public standalone ballot now uses two distinct native instructions.
`CastPlainBallot` creates one owner- and choice-bound lock. A second cast with the
same choice is rejected before tally or escrow mutation; a second cast with a
different choice retains the existing double-vote slash policy. The new
`UpdatePlainConviction` carries no choice field. Core reads the original choice
from the authority-owned retained lock, requires an existing position, and
applies the frozen exact-unit, strict increase, nondecreasing duration/expiry,
aggregate-bound, and actual escrow-delta checks before replacing its weight.

The update has its own canonical four-field Norito instruction layout and wire
ID. The signed transaction must carry that exact instruction as the sole direct
governance ballot entrypoint; a signed update cannot be substituted with a cast.
The Initial executor, native dispatcher, time-sensitive execution classification,
and validation-fee effects inventory include the new instruction. There is no
alternate decoder or cast-as-update path.

Local source checks: scoped Rust formatting and `git diff --check` passed.
Focused Cargo tests are pending a settled shared build slot. The updated tests
cover duplicate casts, missing-position updates, owner mismatch, monotonic
bond/expiry checks, exact escrow deltas, retained choice, slash restitution,
typed wire decoding, and signed-instruction substitution.

This public-account slice does not implement anonymous credential ballots,
confidential bond positions, a sound closed-corpus tally proof, or the mandatory
committee-free late-dropout protocol. Those and SDK/client interface parity
remain F11 release blockers.
