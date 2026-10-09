# Non-hiding IPA polynomial and transcript controls

Run the standard-library Python controls from the repository root:

```sh
python3.12 -B -m unittest discover -s formal/kagemusha_fold -p 'test_*.py' -v
```

The original polynomial controls check the algebra in the
[conditional honest-fold abort bound](../../specs/kagemusha_recursion_soundness_v1.md#conditional-honest-fold-abort-bound).
The symbolic recurrence follows `accumulation.rs::create_fold`: combine the
ordered source coefficient vectors with powers of alpha, subtract H(z) from
coefficient zero, form each L/R point, then fold a with inverse u and b/g with u.
Source vectors are generated independently from the bit-product definition.
Logs model an additive cyclic group, including adversarially dependent generators.
No discrete-log independence is assumed by the tested polynomial claim.

For one through four rounds, the tests verify the exact leading z coefficients
after clearing Laurent denominators, the nonzero top coefficient and the total
degree bounds. Finite-characteristic cases use dependent generator logs and
compare the symbolic expressions with a separate scalar recurrence. Short-only
sources make the first L identically zero; removing the evaluation shift changes
R's leading coefficient. These negatives check two essential hypotheses.
The sum of the degree bounds is also checked at native k16: its coefficient is
3,015,170 + 32(m−1), before multiplying by the mapped challenge's maximum atom.
An exhaustive one-round experiment over the field of order17 counts every
zero/identity event for three dependent-generator choices, including tapes
after an earlier exceptional message, and checks the stated union bound.

`source_manifest.json` binds the inspected three native sources and these three
control files. The exact inventory and hashes are checked during the suite.
This is a source-consistency check, not authenticated artifact admission. Nothing
under `target/` or any external Python package is needed.

The all-k mathematical derivation remains in the linked memo; bounded symbolic
examples do not constitute a general theorem prover. These polynomial controls
produce no native proof and do not model RP57 or actual challenge independence.

The separate `transcript.py`/`test_transcript.py` controls execute the unchanged
standard-library RP57 reference arithmetic, with the exact two small native
constant tables checked against the retained KAT constants. They reproduce the
two Rust PIPA-AS prelude KATs, check canonical scalar limbs/maps and trace every
primitive input/output for all nineteen squeeze calls. A separate block scheduler
checks the carried state, padding and counts `52+ceil(35m/2)` for Pallas and
`52+ceil(19m/2)` for Vesta. Empty/even/odd buffers and the logical-history padding
alias are explicit cases. The unmodified reference reader checks final scalar
absorption and the unabsorbed generator suffix without a later squeeze.

These traces use syntactically canonical messages. They do not assert the
non-hiding IPA equation or generate a fold proof, a key or a size65536 generator
vector. Mutated messages demonstrate schedule sensitivity or expected lack of
later absorption; unchanged prior challenges do not make a bad witness valid.
Missing/changed sources and invalid/unbounded inputs refuse. The separate exact
`transcript_source_manifest.json` pins these controls, their reference imports,
native/circuit schedule sources and the Rust KAT test. The original algebra
inventory/assertions remain unchanged except for this README's refreshed hash.

Transcript agreement supplies no fresh-answer law for fixed public RP57. The
conditional ideal-permutation first-hit argument still needs its salt/capacity
interface and shared-oracle assumptions; these controls do not instantiate or
test that security theorem. Neither control group establishes adaptive privacy,
entropy or resource guarantees, recursive continuation, C12, current artifact
validity or release qualification. No external Python dependency or `target/`
input is needed.
