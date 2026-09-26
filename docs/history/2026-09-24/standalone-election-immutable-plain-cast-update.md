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

Local source checks: scoped Rust formatting and `git diff --check` passed. A
same-source Core library binary passes the signed-update substitution test
(1/1), public ballot selector (4/4), governance registration and direct-entry
selectors (1/1 each), zero-minimum escrow helper (1/1), and slash/restitution
selector (4/4). The fresh DataModel codec selector passes 1/1, including
cast-payload substitution with a direction field. Fresh grouped Core integration
passes 9/9 under `iroha-core-tests`. The updated tests cover duplicate casts,
missing-position updates, owner mismatch, monotonic bond/expiry checks, exact
escrow deltas, retained choice, slash restitution, typed wire decoding, and
signed-instruction substitution.

The JavaScript SDK now exposes a choice-free update builder, one-instruction
signed transaction helper, typed inputs, and the canonical native wire ID.
Its strict Norito boundary rejects an injected direction and duplicate JSON
keys before native dispatch. The public cast also has an exact five-field
boundary that rejects update-like extras and directions outside 0–2. The
isolated strict-boundary and TypeScript checks pass; syntax, scoped ESLint,
distribution build, and built-export checks pass. Native-backed JavaScript
parity tests require a rebuilt same-source
binding: the current binary's source-provenance guard correctly rejects it.
Kotlin and Java-source consumers now compile the exact four-field native
instruction and reject choice fields, noncanonical values, and malformed
frames. Their four focused runtime tests stop at missing same-source ABI-23
account admission before reaching the assertions. Rust fixture parity, the
other SDK/client interfaces, and final fixture regeneration remain open.

The [F12 optional-orderbook lazy cut](javascript-f12-orderbook-lazy-bundle-boundary.md)
restored the JavaScript bundle gate without changing the release limits. The
Torii eager closure is now 811,612 bytes against its unchanged 797 KiB ceiling;
the complete `npm run bundle:check` passes. Native-backed orderbook and ballot
parity still require the rebuilt same-source binding.

This public-account slice does not implement anonymous credential ballots,
confidential bond positions, a sound closed-corpus tally proof, or the mandatory
committee-free late-dropout protocol. Those and SDK/client interface parity
remain F11 release blockers.
