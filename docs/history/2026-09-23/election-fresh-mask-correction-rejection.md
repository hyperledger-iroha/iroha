# Standalone election: public update-correction rejection

This is a scoped rejection of one proposed adaptation, not an impossibility
result, a protocol design, or a production qualification. The required
behavior remains in the [standalone election contract](../../../specs/standalone_election_protocol_contract.md).
Self-tallying group ballots with cancellation are useful background (for
example, the [QV-net author manuscript](https://wrap.warwick.ac.uk/id/eprint/194383/2/WRAP-QV-net-decentralized-self-tallying-quadratic-voting-maximal-ballot-secrecy-25.pdf)),
but the update adaptation below is our own derivation and is not attributed to
that paper.

Consider a two-option ballot represented by a pair of elements in a prime-order
group with generator `g`. The accepted masked ballot for hidden option `b`,
weight `w` and mask vector `r` is
`C[k] = g^(w * 1[b = k] + r[k])`, for `k` in `{0, 1}`. Suppose a conviction
update keeps `b`, raises the weight to `w'`, and freshly masks the replacement
as `C'[k] = g^(w' * 1[b = k] + r[k] + d[k])`. To let the public tally cancel
this new mask without another voter acting, the adaptation publishes the
per-update correction `D[k] = g^(-d[k])` alongside the replacement.

An observer computes, coordinatewise,
`C'[k] / C[k] * D[k] = g^((w' - w) * 1[b = k])`. When
`0 < w' - w < order(g)`, exactly one coordinate is the identity, identifying
the hidden option without a discrete logarithm. Concealing the bond and weight
does not help this identity test. A zero-knowledge proof that the update
preserves the choice does not hide these public group elements.

For an equal-final-totals privacy experiment, let voter A's exact smallest-unit
bond increase from 1 to 4 at multiplier 1, giving weights 1 then 2, and let
voter B's bond remain 4, giving weight 2. In one execution A chooses option 0
and B option 1; in the other they exchange choices. Both final totals are
`(2, 2)` and the public schedule and amounts can be identical. The correction
above identifies A's choice, so these executions are distinguishable even
though the allowed final disclosure is the same. If A disappears immediately
after the update is accepted, its already published correction still causes
the leak; if the correction was deferred until A returns, the accepted ballot
cannot be guaranteed to tally after that dropout.

This rejects **public per-update mask compensation**, including a public
correction for a batch containing only one update. It does not reject a
construction that privately couples fresh masks across accepted participants
or otherwise proves an exact closed-corpus tally without releasing the
individual correction. Such a construction still needs a concrete phase
protocol, dropout bound, security argument, resource analysis, implementation
and independent review. F11 remains open.
