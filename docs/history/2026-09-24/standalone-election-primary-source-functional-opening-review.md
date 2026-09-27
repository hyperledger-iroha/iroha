# Standalone election: primary-source aggregate-opening review

This is a read-only F11 literature and current-source assessment on the
`optimizations` checkout on 2026-09-24. It does not select a cryptographic
construction, prove impossibility, qualify a circuit or change the
[standalone election contract](../../../specs/standalone_election_protocol_contract.md).
The [fault matrix](standalone-election-dropout-fault-matrix.md) and
[aggregate-opening review](../2026-09-23/standalone-election-dropout-aggregate-blocker.md)
state the release obligations. The papers below establish useful component
properties, but none establishes the required composition.

## Primary-source comparison

| Construction | Property established by its paper | Gap under this contract |
| --- | --- | --- |
| [Decentralized multi-client functional encryption for inner products, Chotard et al. (ASIACRYPT 2018)](https://www.iacr.org/archive/asiacrypt2018/11272256/11272256.pdf), Definitions 4–5 | No central master secret; decryption evaluates a chosen function over an `n`-vector of same-label ciphertexts after combining the participating clients' function-key shares. | **Inference:** a client absent before issuing its share for the eventual closed-corpus function can block that function. Preissuing usable keys for alternative survivor sets or prefixes would create additional aggregate queries unless a separately proved close restriction prevents their use. The paper does not supply that election-specific restriction or conviction-update relation. |
| [Ad hoc multi-input functional encryption, Agrawal et al. (ITCS 2020)](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ITCS.2020.40) | Sources generate their own keys; the aggregator needs a separate, function-specific key from each selected source. | **Inference:** dynamic selection removes the fixed-arity inconvenience but does not make a dropped accepted source issue a later key. Early keys for multiple selected sets also need a proof against proper-subset outputs. |
| [Verifiable decentralized multi-client functional encryption, Nguyen et al. (2023)](https://eprint.iacr.org/2023/268.pdf), Definition 6 and Section 6.2 | Its decentralized-sum primitive has no private decryption key and reveals the sum when all `n` group ciphertexts under one label are supplied; its all-or-nothing encapsulation addresses incomplete-ciphertext leakage for that fixed group. | **Inference:** disappearance after a participant's final same-label ciphertext is durable need not prevent that fixed sum. A setup participant who never casts leaves a missing component, however, and the paper gives no authoritative latest-update selection or unique-final-corpus opening. Treating a preposted zero as replaceable would allow a baseline-versus-updated aggregate query unless a new restriction is proved. |
| [QV-net, Zhou et al. (CCS 2025)](https://wrap.warwick.ac.uk/194383/2/WRAP-QV-net-decentralized-self-tallying-quadratic-voting-maximal-ballot-secrecy-25.pdf), Section 3 | Registered voters publish keys and then ballots; anyone can tally after all ballots are cast, with its stated maximal-ballot-secrecy game. | **Inference from its pairwise cancellation and two-round protocol:** a setup-only noncaster, changed accepted set or choice-preserving weight update needs a different recovery argument. A voter leaving after a completed ballot is not itself the unsolved case. Its stated game is not the contract's adaptive dropout/update and equal-final-output leakage game. |
| [DRE-ip, Shahandashti and Hao (ESORICS 2016)](https://eprint.iacr.org/2016/670.pdf), Sections 3–4 | A confirmed vote remains counted after its voter leaves; the DRE maintains running tally and randomness sum and posts the final values for public verification without tallying authorities. | The DRE sees and retains a running tally until publication. The paper explicitly permits a partial tally to leak on machine compromise. That privileged machine and leakage do not meet this product's public-worker, anonymous-credential and aggregate-only boundary. |

No row supplies concrete release values for eligible credentials `N`, accepted
cast/update history `A`, updates per credential `U`, options `K`, adversarial
post-accept voter dropouts `d >= 1`, active corruptions `c`, required worker
availability, synchrony or a finite close-to-result deadline. Those values must
be fixed for a proposed construction and its complete proof; they cannot be
borrowed from a different paper's fault model. For a fixed, already complete
same-label ciphertext vector, late voter absence may be harmless. That limited
observation does not cover setup-only noncasting or disappearance immediately
after **each** accepted conviction update in an evolving corpus.

The remaining candidate research seam is a publicly verifiable opening
restricted to the **one** finalized ordered history and its latest accepted
state per credential. It must be derivable after an accepted voter disappears,
without reconstructing that voter's secret, creating a committee/master
decryptor, or exposing a prefix, survivor-set or update-difference tally. This
is a required property, not a selected primitive. The existing
[fresh-mask correction trace](../2026-09-23/election-fresh-mask-correction-rejection.md)
shows why a public linear update correction reveals the hidden choice. A sound
proof of asserted final totals alone does not provide the missing opening
witness or prevent an unauthorized second aggregate query.

## Current implementation seam

[`CreateElection`, `SubmitBallot` and `FinalizeElection`](../../../crates/iroha_data_model/src/isi/zk.rs)
carry an eligibility root, opaque ciphertext and proof, caller-supplied
nullifier, and asserted `Vec<u64>` totals. They have no typed credential,
confidential bond position, cast/update phases or closed-corpus statement.
[`SubmitBallot::execute`](../../../crates/iroha_core/src/smartcontracts/isi/world.rs)
still checks the transaction authority's ballot permission and citizenship;
the contract instead requires a credential proof to authorize the ballot while
a relayer only transports and pays. Its public-input helpers read only a
commitment/root pair, derive a nullifier from the public commitment and read
one `u64` value per option. The separate public PLAIN conviction path uses
checked `u128` smallest-unit weights and aggregates, so the current private
`Vec<u64>` result shape cannot be retained without an explicit proved bound.
[`ElectionState`](../../../crates/iroha_core/src/state.rs) stores nullifiers and
a bounded ciphertext vector, not the latest credential-linked confidential
bond and ballot history. Core's explicit standalone ballot/tally semantic guard
remains fail-closed before proof verification and accepted-state mutation.

F11 remains open. The next reviewable artifact is an explicit phase transcript,
fault/leakage model, complete accepted-corpus opening and soundness/privacy
arguments, followed by implementation and maximum-shape measurements. This
assessment authorizes no key, circuit, compatibility route or production
admission.
