# Contract code readback fixture

`code_readback.ko` is a public, read-only test contract returning the integer 7.
`code_readback.to` is its complete 461-byte IVM artifact, including the execution
header and embedded interface. It contains no keys, account identities or live
state. It is a test input, not deployment authority or an accepted release.

The fixture was produced by the retained development `koto` executable whose
SHA-256 is recorded in `provenance.json`. Source and compiler hashes remained
unchanged through compilation and `--verify`. The compiler's source revision is
not authenticated by that binary hash; the applying owner must run the current
native reproduction/admission regression and may intentionally regenerate it.
`--verify` compares published compiler outputs; it alone is not artifact admission.
A separate standalone probe using the pinned retained SDK reproduced the exact
bytes and passed `ivm::verify_contract_artifact`, ABI 1 and native identity checks.
That is development component evidence, not current native release qualification.

The SDK uses `include_bytes!` with a repository-relative path and pins the native
contract hash. Its tests require neither the compiler nor an ignored output tree.
The existing IVM contract-artifact test harness separately recompiles this source,
compares the exact bytes, performs native artifact admission, and compares the
admitted identity with the shared data-model helper.

Regenerate using the reviewed `koto` built for the source being qualified. From
the repository root, with that executable already present:

```sh
target/debug/koto build --chain-discriminant 369 \
  --target-dir target/contract-code-readback-fixture \
  --out crates/iroha/tests/fixtures/contract_code_readback/code_readback.to \
  --manifest-out target/contract-code-readback-fixture/manifest.json \
  crates/iroha/tests/fixtures/contract_code_readback/code_readback.ko

target/debug/koto build --chain-discriminant 369 \
  --target-dir target/contract-code-readback-fixture \
  --out crates/iroha/tests/fixtures/contract_code_readback/code_readback.to \
  --manifest-out target/contract-code-readback-fixture/manifest.json --verify \
  crates/iroha/tests/fixtures/contract_code_readback/code_readback.ko
```

Intentional regeneration must update the exact fixture hashes and the SDK/IVM
hash goldens together, then rerun the focused regressions. Never replace admission
with an accepted-looking hash or silently update a failing golden.
