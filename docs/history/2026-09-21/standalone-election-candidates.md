# Standalone election construction review

This is a scoped September 21 literature/requirements comparison, not an
independent audit, impossibility result or complete search. No candidate is
admitted. The required behavior is in the
[standalone protocol contract](../../../specs/standalone_election_protocol_contract.md).

The author manuscript of **QV-net** describes two-round self-tallying quadratic
voting and a tally after all ballots are cast. Its published construction uses
authenticated voter messages and pairwise cancellation from the registered keys
(sections 3.1–3.2). That construction alone does not establish our anonymous
credential authorization, accepted-ballot late-dropout completion or confidential
conviction-update semantics. This is a gap relative to our requirements, not a
claim that its stated security theorem is false. Its measured proof/tally costs
also cannot qualify our different statement and maximum weights.
[Author manuscript, Zhou et al.](https://wrap.warwick.ac.uk/id/eprint/194383/2/WRAP-QV-net-decentralized-self-tallying-quadratic-voting-maximal-ballot-secrecy-25.pdf)

**A Fair and Robust Voting System by Broadcast** adds a recovery round to the
Open Vote construction. Section 3.2 explicitly permits a voter to leave recovery
and have that voter's vote discarded. Consequently, importing that recovery
unchanged would not preserve our complete accepted corpus when an accepted
voter disappears at this point. Our rejection concerns that behavior; it is not
an assertion that every recovery message reconstructs a secret key.
[Khader, Smyth, Ryan and Hao](https://www.dcs.warwick.ac.uk/~fenghao/files/main-openvote.pdf)

**E-cclesia** combines anonymous authorization/broadcast with time-lock ballots.
Its casting procedure gives each ballot a publicly solvable puzzle; tallying
opens individual ballots. Anonymous individual plaintexts disclose more than our
aggregate-only requirement permits. Its voter-identity privacy definition does
not make these plaintexts an acceptable substitute for aggregate-only disclosure.
[Protocol, Arapinis et al.](https://eprint.iacr.org/2020/513.pdf)

**Parallel OV-Net** tolerates some unresponsive voters by running multiple
overlapping sub-elections. Its authors explicitly analyze the extra information
from individually tallyable sub-elections and the statistical loss of accuracy;
when surviving session vectors do not span the full voter vector, its exact
combiner cannot recover the complete sum. Our contract permits one exact final
total over every accepted ballot and no additional subset tallies. Importing
this recovery therefore does not meet either the accuracy or disclosure
requirement. This rejects that construction for this product; it is not a claim
about the paper's stated trade-off theorem.
[Bana et al., sections V–VI](https://eprint.iacr.org/2021/1065.pdf)

A separate algebraic rejection check applies to a tempting update adaptation.
If a binary-choice ballot is `C = M * g^(w*b)` and an update publishes
`C' = M * g^(w'*b)` using the same mask, the public ratio is
`C'/C = g^((w'-w)*b)`. For a known nonzero weight increase smaller than the group
order, comparing the ratio to the identity reveals whether `b` is zero, even
without solving a discrete logarithm. This is our direct derivation for that
specified adaptation, not a claim about an update protocol in those papers.
Hiding the credential or proving choice preservation alone does not fix the
leak. A candidate needs an actual fresh-mask/update construction and its proof.

The next design review must establish a construction meeting all requirements
together, including bounded non-reconstructing recovery and latest-weight
privacy. Familiar proof primitives, small fixture timings and a self-tallying
label do not establish that composition. F11 and release qualification remain
open while the independent implementation workstreams continue.
