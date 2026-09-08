# Contract code readback fixture

`code_readback.ko` is a public, read-only test contract returning the integer 7.
`code_readback.to` is its complete 793-byte IVM artifact, including the execution
header and embedded interface. It contains no keys, account identities or live
state. It is a test input, not deployment authority or an accepted release.

The fixture was regenerated for the sole final V1 ABI using the retained
`koto` executable identified in `provenance.json`. Its hash matches a successful
Cargo build capture, and its compiler source assets match the current source.
Three ABI source files differ through the merged Norito trait split and shared
contract-code-hash helper move. A retained native CLI admitted these exact bytes
and reproduced the compiler manifest byte-for-byte. The compiler's `--verify`
check also passed. The current-source IVM `contract_artifact` suite passed all 58
tests, including exact source reproduction, native admission and the shared native
identity check for this fixture. The native SDK route suite has not been run;
these component results do not establish release qualification.

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
