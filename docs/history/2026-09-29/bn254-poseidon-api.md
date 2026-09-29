# BN254 Poseidon parameter API correction

Status: implementation and scoped native/consumer validation pass. This is an
algorithm naming correction; the existing permutation and outputs are unchanged.

`iroha_zkp_poseidon` exports `Bn254PoseidonParams` and
`bn254_poseidon_params_width3/6`. The former Poseidon2 parameter names are removed
without aliases. FASTPQ's GPU parameter staging and the AXT fixture generator
use the actual original-Poseidon API. Private two/six-input helpers describe
their arity. IVM opcode numbers, lowering, kernels and arithmetic are unchanged.

Independent source review compares the production body after reversing only
identifier changes and removing comments/whitespace. It also reconstructs the
entire parameter-byte digests from the pre-existing AXT capture, independently of
the parameter generator: width three has 201 field words and SHA-256
`20a6364b21446c75eafb313c00cda37f1e772a3e76f158d6938b40fd52988709`;
width six has 420 words and SHA-256
`443ee9a4a9e5f8425720a184e8ef3fbe1897c9fcae80e4c1827cf0a246889bda`.
The new native test pins both complete byte sequences. Existing output known
answers remain, and separate compile-fail examples reject all three retired names.

## Validation

An isolated candidate at `ce9a01dcdc32bcf9edaf3fcdd182872eb4d4ac51` contains
only the eight reviewed naming amendments. Its 20,720-file source manifest is
`1e853d1c6f471ee272496e9d5179aef3beab0c1e450746599b910693d9e183f9`.
Normal locked/offline Rust 1.93.1 builds use a fresh dedicated target, two Cargo
jobs and the ordinary workspace profiles. Source and retained executable hashes
remain unchanged; every local dependency belongs to the captured candidate and
target, with any reuse checked against the earlier compiler-emitted byte hashes.

- `cargo test -p iroha_zkp_poseidon --lib`: 19 passed, zero failed or ignored.
- `cargo test -p iroha_zkp_poseidon --doc`: one positive and three removed-name
  compile-fail examples passed, zero failed or ignored.
- `cargo check -p fastpq_prover -p iroha_data_model --features
  fastpq_prover/fastpq-gpu,iroha_data_model/dev-tools,iroha_data_model/test-fixtures
  --lib --bin axt_fixtures`: passed for both real consumers.
- Scoped Rust formatting and diff checks passed.

The initial capture script failed before Cargo because of a missing Python
import; its failure record remains. The independently reviewed runner records
test-count mismatches as failures before exiting. The subsequent captured run
passes all steps. Evidence resides under
`dist/zk-remediation/2026-09-29/bn254-poseidon-naming/`, including the exact patch,
independent review and `normal-20260929T105213Z/result.json`.

This evidence does not measure GPU execution, change cryptographic parameters,
or qualify a full proof, network deployment or production release. The distinct
Pasta migration remains governed by the [semantic commitment design](../../../specs/ram_lfe_semantic_commitments.md).
