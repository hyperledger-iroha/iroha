# Execution proof security qualification record

The release target is at least **128 bits of classical computational soundness**. The compiled execution profile is not qualified. Genuine full-computation proofs, arithmetic parity, parameter calculations and codec tests establish useful implementation facts; they do not substitute for an independent cryptographer reviewing this exact construction. No post-quantum Fiat–Shamir claim is made.

## First-release construction

The sole execution suite uses one published Poseidon2 permutation with width 16, rate 8, capacity 8 and six output coordinates. Its pinned parameters and cryptanalytic assumptions are in [POSEIDON2_PROVENANCE.md](POSEIDON2_PROVENANCE.md). RaceV1 uses eligibility-first awards; a forfeited input key cannot earn a prize from an earlier finish. Obsolete draft suites are absent from the runtime catalog. Privacy proof formats and their implementation remain separate.

## Exact FRI arithmetic

For native trace logarithm `n` in 13 through 19, the LDE logarithm is `d=n+3` and binary folding performs exactly `r=d-10`, hence six through twelve rounds. The verifier constructs the complete arity vector `[2; r]` from that actual schedule and rejects missing, extra, nonbinary or stale three-fold metadata. The initial affine batching uses independent extension-field coefficients; it is separate from the binary fold arities.

The profile uses `m=3`, effective rate at most `rho=1/7`, 136 distinct unbiased query indices, and terminal degree at most 143 on 1,024 points. The masked native polynomial has degree at most `N+975`, hence **N+976 coefficients**. At the smallest native domain that is 9,168 coefficients; it fits the accepted FRI input coefficient cap 9,216, and `9,216/65,536 = 144/1,024 < 1/7`. This explicit coefficient/degree distinction avoids an off-by-one inference.

Under the applicable affine-batched FRI correlated-agreement theorem, the query term is exactly bounded by

`(sqrt(1/7) * (1 + 1/6))^136 = (7/36)^68 < 2^-160`.

Sampling distinct indices without replacement can only reduce the probability of all queries hitting a fixed bad set: each factor `(b-i)/(D-i)` is at most `b/D`. The verifier enforces canonical unique indices and deterministic rejection sampling; the adversary does not supply that set or its randomness.

Using the conservative extension-field lower bound `|Fp4| > 2^252`, the two commitment terms are bounded by `2^(2d+20-252)` and `2^(d+10-252)`. For the second, the complete arity sum is at most 24, and `7*sqrt(7)*24 < 504 < 512`; also `D+1 < 2D`. The first coefficient is conservatively bounded by `7^7 < 2^20`. At `d=22`, the commitment-term sum is below `2^-187`; at `d=16`, below `2^-199`. The existing coarse numerical ceiling was large enough for twelve binary folds, but obsolete draft certificate did not express this correspondence correctly. The current verifier enforces the complete fold schedule.

The theorem mapping must be reviewed against [Haböck's FRI/DEEP summary](https://eprint.iacr.org/2022/1216) and the underlying [Proximity Gaps paper](https://eccc.weizmann.ac.il/report/2020/083/revision/3/download). The integer calculator proves the stated inequalities and rejects malformed geometry. It does not prove the implementation satisfies every correlated-agreement premise. In particular, the reviewer must verify the exact affine batching, degree correction, DEEP openings and sequence of commitments against the theorem, rather than treating the calculator name as a security certificate.

## Whole-profile obligations still open

| Obligation | Existing evidence | Required independent qualification |
| --- | --- | --- |
| Integer semantics | Separate native reference; complete microcycle residual tests for all tracks; arbitrary-field degree-four audit; browser parity | Prove every manual interval, division/comparison branch and selector enforces a unique bounded integer transition; audit collision order, disabled rows, padding and final/checkpoint boundaries |
| AIR-to-polynomial linkage | Independent Fp4 constraint weights sampled after auxiliary roots; exact quotient/opening checks; four degree-bounded composition chunks | Derive all cancellation and correlated-agreement error terms for this exact AIR and chunking, including boundary constraints and zero denominators |
| DEEP binding | One admissible Fp4 point; current/next base and auxiliary openings; all chunks committed before sampling; independent mixes after DEEP values | Establish the exact DEEP/ALI round-by-round bound and accepted-domain exclusions; do not replace it with a generic `degree/field_size` guess |
| Copy machinery | RaceV1's eight shared copy cells are fixed inactive and forced zero; gameplay state transitions use direct AIR equations | Confirm no racing semantic obligation relies on a probabilistic shared byte permutation; separately audit the generic permutation before a different application uses it |
| Commitment primitive | Published wide Poseidon2 parameters; upstream KAT; regenerated Grain constants; exact modulo and framing tests | Review the exact sponge mode and current algebraic attacks; a parameter label or output length is insufficient |
| Fiat–Shamir | Statement/profile before roots, base roots before copy challenges, auxiliary roots before constraint weights, composition/mask roots before DEEP point, openings before mixes, each FRI root before its beta, terminal before grinding/query indices | Map this multiround transcript to an applicable random-oracle reduction with explicit adversary-query and work accounting; include all restart/grinding choices |
| Ledger statement | Session/network/manifest/roster/transcript/history/outcome binding; exact compiled profile; canonical public replay | Four-validator settlement, dispute/restart and state-root agreement, authenticated ledger inclusion, fee/resource admission and custody adversaries |

The measured FRI terms together are below `2^-159`; that number is not the overall proof security. The remaining algebraic linkage and DEEP errors need an explicit union bound, followed by a justified Fiat–Shamir reduction and commitment assumptions. Twenty-bit grinding is not added to the security exponent. [Concrete noninteractive FRI analysis](https://eprint.iacr.org/2024/1161) documents why conjectured IOP estimates and noninteractive security can materially differ; this implementation must not inherit a claim merely by copying its query count.

The execution driver includes a conditional classical-ROM work-accounting helper. Supplying a round-by-round exponent to that helper is an assumption, not evidence establishing it. Its result must not activate the profile until the exact round-by-round bound and oracle model above have independent review. The final signed qualification report must state the model, attack work measure, all assumptions, every loss, the reviewed source/profile hashes and the final combined target.

## CPU, fallback and accelerated parity

The current production qualification path is portable scalar Rust arithmetic with deterministic Rayon orchestration and the single compiled wide Poseidon2 execution suite. Historical draft proof measurements do not qualify the current environment relation.

Before any accelerated path is selected: check primitive/permutation vectors and full framed digests, FFT/IFFT and fixed-query evaluations, Merkle roots/frontiers, FRI folds/terminal degrees and rejection behavior against scalar Rust; test thread widths 1, 2 and the machine default. Deterministic injected masking entropy is required for byte-for-byte proof comparison. Separately verify independently generated proofs in both directions, since real proving uses fresh masks and need not emit identical bytes. Unsupported hardware, acceleration errors and exhausted resource budgets must return a bounded failure or use the exact same scalar suite. GPU availability never changes profile selection, parameters, accepted statements or settlement authority.

No GPU or cross-architecture proof benchmark has passed yet. Native target builds, complete validator resource measurements and a four-validator integration run remain required. Current full-proof and prototype evidence is retained separately under `sora-cars/output/qualification/native-proof/`; its warm development dependencies and concurrent-machine-work caveats still apply.

## Profile source and ledger commitments

The catalog accepts only exact compiled descriptors, and the first release has one corrected stock relation. The profile binds the native reference, outcome derivation, whole-tick and staged arithmetic, environment catalogs, Poseidon2 parameters and proof driver. Registry and export code and separate proof tests are outside that commitment; changing them alone cannot replace a verifier. Native model schemas and canonical Norito decoding are also part of the deployment trust context and require shared browser fixtures. There is no compatibility fallback for obsolete draft formats.

The rules descriptor explicitly includes all environment tables and branch parameters; its identity binds browser replay interpretation. The complete verifier/source profile is independently bound into every manifest and proof statement. Any source or specification change before release must regenerate the current profile, browser exports and qualification evidence.

Iroha `Hash::prehashed` forces one output marker bit; `Hash` therefore carries at most 255 unconstrained bits. The idealized generic birthday exponent is 127.5, while fixed-target substitution has a different attack game. Do not infer a literal 128-bit whole-settlement collision bound from its 32-byte encoding. Review profile, network, manifest, roster, transcript, dispute, outcome, inclusion and finality commitments under their actual attacker control and trust assumptions. Wide Poseidon2's 48-byte digest does not repair or automatically justify ledger-hash security. This accounting and the complete Fiat–Shamir/DEEP/ALI union bound remain open; the profile stays unqualified.
