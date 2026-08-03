# Vega reference implementation (`pyvega`)

A small, dependency-light **pure-Python** implementation of the Vega-MC proof
system (the only third-party package is `pycryptodome`, for Keccak). It is the
*authoritative reference* for the protocol: rather than
restating the algorithm in prose, the book points here, and conformance is
established by **mutual acceptance** with the shipped Rust prover/verifier.

> **Status: preliminary** — an executable specification optimized for clarity,
> limited to the fixed cubic circuit, and cross-conformance tested against the
> Rust implementation in both directions. It has not been independently
> security-audited and its internals may change; treat it as a spec and teaching
> aid, not a production prover.

Because the proof is randomized (zero-knowledge), conformance is *not* byte
identity of proofs. Instead it is verifier acceptance in **both directions**:

* the Rust prover's proof is accepted by this Python verifier, and
* this Python prover's proof (and its Python-generated verifier key) is accepted
  by the real Rust verifier.

The worked example throughout is the tiny cubic relation `y = x³ + x + 5`
(`x = 2`, `y = 15`) — the same `CubicCircuit` the Rust test-suite uses — so every
structural feature of the protocol is exercised while staying checkable by hand.

## Layout

```
pyvega/
  params.py            curve/field integer constants
  field.py  polys.py   scalar field arithmetic, MLE / eq / sparse polynomials
  curve.py             base field, curve arithmetic, point (de)compression
  codec.py             bincode reader primitives
  transcript.py        Keccak Fiat-Shamir transcript
  proof.py  vk.py      proof / verifier-key parsers (wire -> structs)
  commitment.py  hyrax.py  instance.py  nifs.py  sumcheck.py  spartan.py
                       verifier building blocks
  verify.py            the top-level verifier (acceptance predicate)

  app_circuit.py       the cubic application R1CS + witness
  verifier_circuit.py  emitter for the fixed in-circuit verifier shape/witness
  prover*.py           the prover: commit, core (NIFS + sum-checks), finish
                       (Nova fold + relaxed Spartan + ZK IPA), serialize
  setup.py             stand-alone key-gen + verifier-key serialization
```

## Running

Every test runs under stock `python3` (the implementation is pure Python).
Install the one dependency once with `python3 -m pip install pycryptodome`.

```sh
# deterministic core (byte-exact vs Rust fixtures)
python3 reference/tests/test_proof_parse.py
python3 reference/tests/test_transcript.py
python3 reference/tests/test_vk_digest.py

# Rust prover -> Python verifier accepts
python3 reference/tests/test_verify.py

# Python prover -> Python verifier accepts (self-check)
python3 reference/tests/test_prove_finish.py

# fully stand-alone: Python setup + prove + verify, then write the fixtures
python3 reference/tests/test_standalone.py
```

The two cross-conformance gates against the real Rust verifier are `#[ignore]`
tests in `tests/reference_conformance.rs`:

```sh
# Python proof against the Rust-exported verifier key
cargo test --test reference_conformance verify_python_proof -- --ignored --nocapture

# Python proof against a Python-generated verifier key (zero Rust setup)
cargo test --test reference_conformance verify_python_standalone -- --ignored --nocapture
```

## Fixtures

`reference/fixtures/cubic/transcript_vector.json` (a frozen Keccak transcript
known-answer vector consumed by `test_transcript.py`) is the only committed
fixture. Every other fixture -- verifier keys, proofs, digests, and the
`meta.json` metadata (circuit info and expected public values) -- is git-ignored
and regenerated on demand, because the keys are large and the proofs are
randomized. Regenerate them with:

| Fixture | Regenerate with |
| --- | --- |
| `cubic/meta.json`, `cubic/proof.bin`, `cubic/vk.bin`, `cubic/vk_digest.bin` | `cargo test --lib export_cubic_fixtures -- --ignored` |
| `cubic/transcript_vector.json` (Keccak transcript known-answer vector) | `cargo test --lib export_transcript_vector -- --ignored` |
| `cubic/python_proof.bin` (reference proof vs the Rust `vk.bin`) | `python3 reference/tests/test_prove_finish.py` |
| `cubic/python_vk.bin`, `cubic/python_standalone_proof.bin` (fully stand-alone) | `python3 reference/tests/test_standalone.py` |

The `verify_python_proof` gate needs `cubic/vk.bin` + `cubic/python_proof.bin`;
the `verify_python_standalone` gate needs `cubic/python_vk.bin` +
`cubic/python_standalone_proof.bin`. Regenerate the relevant fixtures before
running a gate on a fresh checkout.
