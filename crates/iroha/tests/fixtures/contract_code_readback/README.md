# Contract code readback fixture

`code_readback.ko` is a public, read-only contract returning the integer 7.
`code_readback.to` is its complete V1 table-call artifact, including the execution
header and authenticated interface. It contains no keys, account identities or
live state. The exact identity and local verification evidence are recorded in
`provenance.json`; this fixture does not establish release qualification.

The SDK uses `include_bytes!` with a repository-relative path and pins the native
contract hash. Its tests require neither a compiler nor an ignored output tree.
The IVM contract-artifact suite recompiles the source, compares the exact bytes,
performs native artifact admission, and checks the shared data-model identity.

Regenerate with freshly built `koto` and `ivm_artifact_admit` executables using the canonical
owner in `scripts/regenerate_kotodama_goldens.py`. Its `--write --output-root`
workflow creates a sealed tree under ignored `target` or outside the checkout
after two independent compilations, compiler verification, and native admission
through the same library used by the CLI. Apply the reviewed fixture
from that tree, then run the owner's `--check` mode against the repository.
Compiler and admission-tool staging files remain private (mode `0600`); publication creates
separate public fixture files (mode `0644`) without changing the caller's umask.

Update the provenance and the SDK/IVM hash goldens together. Run the IVM
`contract_artifact` suite and the SDK `contract_code_artifact` route tests on the
same source before recording their results. Admission and exact-byte checks are
required; a plausible hash alone is insufficient.

Build the two canonical producers without a node dependency. The admission tool
is a development binary, so it must be selected with its `dev-tools` feature:

```sh
cargo build --locked -p kotodama_toolchain --bin koto -p ivm_artifact_admission --features ivm_artifact_admission/dev-tools --bin ivm_artifact_admit
```

The owner accepts `--admission-tool <path>` for a nondefault binary location.
An in-checkout publication must be an absent path below `target`, Git-ignored
and contain no tracked files; for example, use an absolute
`target/kotodama-reviewed` output root with `target/kotodama` staging.
The directory parent must already exist. Neither generation mode rewrites
checked-in artifacts, reuses a publication root, or skips native admission.
