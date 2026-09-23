# BFV RNS source bounds before target-limb scale-round

The two-product scale-round entry points accept typed RNS product polynomials.
Previously, the target-limb path checked the centered bound only after converting
the sum into the evaluator chain. When that chain has a narrower product, a
caller-supplied source coefficient outside the permitted centered product bound
can reduce to a small target residue. The direct path checked only the sum, so
two individually out-of-bound inputs could cancel before its final bound check.

Both entry points now reconstruct the source-chain representatives and check
each input against the one-product centered bound and their sum against the
two-product bound before scale-rounding. The target-limb path does this before
the lossy conversion. The new regression uses the degree-two RNS fixture with a
source product larger than the target product and tests positive, negative,
right-hand, and cancelling source residues. Its target-limb fixture maps an
out-of-bound positive source residue to the target residue one.

For a source product `Q` and one-product absolute bound `B`, the centered
reduction accepts residues in `[0, B]` or `[Q-B, Q)` and rejects the gap
between them. The two-product chain preflight requires `Q >= 2(2B)+1`, so the
sum of two admitted inputs in `[-B, B]` has a unique signed representative in
`[-2B, 2B]`; modular wrap cannot turn it into a different in-bound sum. The
new check streams each coefficient through fixed eight-limb stack scratch.
The existing polynomial reconstruction now also uses stack limb scratch while
retaining its one bounded output vector. Other BFV execution and basis-extension
vectors remain uncharged allocations and require candidate-wide resource
qualification; this edit does not close that admission gate.

`rustfmt --edition 2024 --check` on the two edited Rust files and scoped
`git diff --check` passed. The focused locked `iroha_crypto` Cargo test passed
the new adversarial regression, and the same freshly built binary passed the
existing positive `rns_centered_target_limb_scale_round_bridge_matches_scalar_boundary`
control. No proof, parameter, or resource ceiling changed.

This bound check does not prove that arbitrary RNS product polynomials were
computed from the claimed operands. The full BFV-RNS relation, hidden-trace
soundness, eight-party qualification, and independently audited parameter,
lattice-security, noise, and qROM evidence remain release blockers. The
production qualification function remains fail-closed.
