# Standalone conviction smallest-unit boundary

This is a focused arithmetic check on the existing public PLAIN referendum
path in `optimizations`, not evidence that the separate anonymous standalone
election protocol is implemented or qualified.

The frozen `PlainConvictionPolicyV1` converts a canonical nonnegative `Quantity`
to the election asset's smallest units by checked base-ten multiplication. It
then computes `floor(sqrt(units))` and a capped integer conviction multiplier.
Core checks replacement corpus capacity before custody movement, adds category
weights and turnout with checked `u128` arithmetic, and rejects a public
replacement whose direction differs from its retained position. The final
approval comparison uses a three-word product, avoiding `u128` overflow in
the threshold multiplication.

The new `smallest_unit_weight_and_u128_boundary_are_exact_at_maximum_asset_scale`
test covers a single `10^-28` unit, rejection of that amount under a frozen
scale of 27, the exact greatest representable `u128` unit amount at scale 28,
rejection of its one-unit successor, maximal conviction weight, and decisive
aggregate overflow. It is a regression guard for the specified integer rule,
not a proof that an encrypted ballot or tally circuit applies it.

Validation: `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_data_model
--lib conviction -- --nocapture` passed 9/9 focused tests, including the new
boundary case. The first-release anonymous route still requires credential
authorization, confidential bond conservation, hidden-choice-preserving
updates, closed-corpus binding, and a reviewed late-dropout aggregate-only tally
protocol. F11 remains open.
