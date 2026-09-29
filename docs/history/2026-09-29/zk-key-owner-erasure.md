# ZK key-owner erasure repair

Date: 2026-09-29. Status: implementation, independent code review and scoped
normal native validation pass. This is an ownership repair, not cryptographic
qualification of the diagnostic BFV profile.

## Defects and ownership boundary

`BfvSecretKey` exposed its coefficient allocation, printed it through derived
`Debug`, and had no clearing destructor. Its coefficients are now private, with
an explicit borrowed `coefficients()` accessor, redacted diagnostics, constant-time
comparison for equal public geometry, and `Zeroize` plus an actual `Drop`.
Cloning creates another clearing owner. The field name, field type and codec
schema identity stay unchanged; there is no retired accessor alias.

`ConfidentialKeyset` implemented the `ZeroizeOnDrop` marker without a destructor.
The marker itself does not erase anything. A real destructor now clears all five
key arrays. Random generation owns its output before calling the RNG; derivation
owns all slots before HKDF writes into them. Error and unwind paths retain that
ownership. No key-derivation label, algorithm or output changes.

The BFV repair also guards the following allocations:

- Secret polynomials from their creation; key-generation noise, products,
  negations and secret squares; selected public-key and key-switch consistency
  residuals and targets.
- Seeded-encryption plaintext, ephemeral polynomials, noise and intermediate
  products; identifier slot values and per-slot seeds.
- Decryption products and scaled coefficients; partial decoded identifier and
  affine output buffers, including failure after a valid prefix.
- Scalar convolution input copies and accumulation; CRT/NTT inputs, residues,
  reconstruction scratch and partially reconstructed/folded output.
- Resolver-private affine weights and biases, held by a private clearing owner
  from construction through validation and evaluation.

The successful caller-owned plaintext, ciphertext and explicit diagnostic exports
still leave the relevant helper through an intentional ownership transfer.

## Validation

Independent review found no change to RNG sampling order, polynomial arithmetic,
CRT residue order, Garner reconstruction, NTT folding or output order. It also
checked that private-field codec reconstruction creates the whole owner and that
existing repository callers do not move the allocation out.

New tests observe live initialized cells after erasure and before deallocation;
they never read freed memory. They cover clones, explicit zeroization, ordinary
drop, validation errors, unwinding, partial RNG failure and codec roundtrips.
The affine late-failure test checks error behavior after a valid prefix; it does
not independently instrument the library's `Zeroizing<Vec<u8>>` destructor.
Existing fixed key-derivation vectors, BFV arithmetic/parity, schema identities,
identifier adversaries and RAM-LFE controls are included in native validation.

The normal default-feature Cargo build passed. The first retained binary passes
273 BFV tests (including scalar/CRT parity, seeded arithmetic, key consistency,
canonical/JSON key-owner roundtrips and schema controls), 13 confidential tests
and three external fixed-vector/discard tests. RAM-LFE passed 63 of 64: a new
affine-output fixture incorrectly requested a three-byte identifier envelope.
The registered first-release profile correctly rejected it before evaluation.
Only that test's literal was replaced with the registered 63-byte constant;
production source remained byte-identical, independently checked.

A fresh normal build then passes all 86 targeted controls: four BFV key-owner,
two BFV schema, 64 RAM-LFE, 13 confidential and three external vector/discard
tests. This overlaps six of the earlier BFV passes; it is not another full BFV
run. The new late-error test reaches both invalid-tail and invalid-byte cases
after a valid output prefix. Both source guards report no drift. Two existing
artifact/vector generators remain intentionally ignored in their respective
suites; neither is an unexecuted behavioral regression test.

| Retained run | Scope / result | Library binary SHA-256 |
| --- | --- | --- |
| `20260929T060732Z` | Build passes; BFV 273, confidential 13 and external 3 pass; RAM 63/64, fixture failure retained; 921.24 s total | `b070bdeaefee52e06e82b0235648d93796a9ad958da45f1ffca15358963843b7` |
| `20260929T062448Z` | Fresh build and repaired selection 86 pass; one pre-existing ignored vector dumper; 153.25 s total | `02a181d86f8a512500a8a6d51a17f109e97847c9daaffe5d031e3b23bb3208b9` |

Evidence lives under `dist/zk-remediation/2026-09-29/bfv-secret-owner/` and
`confidential-keyset-owner/`. The first build command named standalone integration
targets that the repository groups under `iroha_crypto_group_01`; Cargo rejected
the command before compiling. The corrected command uses the actual grouped
target and retains that failed invocation. Its first inventory assertion also
incorrectly expected zero BFV ignores; the pre-existing conformance generator
accounts for the one ignore above. The original failed result is unchanged.
Scoped rustfmt, diff whitespace and the retired-codec guard pass.

## Key-switch extension

A later read-only review found additional retained Galois and sample-extraction
secret polynomials. Their internal helpers now return `Zeroizing<Polynomial>`
and allocate the clearing owner before populating it. This preserves ownership
through generation, consistency checks and any later error. The exact evaluation
bundle's secret square and three derived key-switch seed buffers also clear.
No seed domain, hash input order, arithmetic, public API or codec changed.

Two additional tests enforce the clearing return type, explicit clearing,
negacyclic signs, invalid power/index rejection and preserved source key.
The eight-coefficient Galois answer was also independently reduced as an integer
polynomial modulo `X^8 + 1`. These tests do not independently instrument the
library's `Zeroizing` destructor; the earlier live-cell tests cover the custom
secret-key destructor.

The ordinary default-feature build and all 49 selected key-owner, Galois,
sample-extraction, key-switch and evaluation-bundle controls pass, with zero
ignored tests and no captured-source drift. Build plus selection took 102.39 s.
The retained binary SHA-256 is
`ed6f73017c257d6e18fbbe99c276987f6237f53deb02cf8eedf61d15a17dec1e`.
Evidence, exact predecessor copies and the independent delta review are under
`dist/zk-remediation/2026-09-29/bfv-key-switch-owner/`; the native run is
`20260929T064641Z`. This selection overlaps prior BFV controls and is not a
fresh full-library run. The primitive/diagnostic lifetime exclusions below remain.

## Remaining limits

Caller-created copies and exported serialized key bytes need their own lifetimes.
Norito decoder scratch, compiler-created copies and primitive-internal state are
not covered by these owners. `rand_chacha` does not expose a clearing RNG owner;
the retained seed clears, but its internal state and by-value compiler copies
are not claimed erased. HKDF internal state has the same outstanding boundary.

Some exported BFV diagnostic proof material still carries raw plaintext/noise
vectors. Its complete lifetime and failure cleanup need a separate owner review;
this patch does not establish whole-library secret erasure. Likewise it does
not turn the exact plaintext-multiple BFV evaluator into production-qualified
encryption or admit the unfinished RAM-LFE proof relation.
