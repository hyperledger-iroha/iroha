# SCCP native compiler policy

`compiler-lock.json` admits separate native compiler owners: Ethereum Solidity
`0.7.6+commit.7338295f` for EVM and TRON Solidity `0.7.6+commit.d1802f25` for TVM.
The corridor checks the complete compiler record against its reviewed constants
before downloading or executing anything. Source pragmas require exact `0.7.6`;
the optimizer, opcode target, source map, ABI, metadata, bytecode and size checks
remain part of artifact admission. `artifact-lock.json` binds the resulting
platform-independent manifest, including all approved platform compiler pins.

The download SHA-256 values are published in the owners' release lists:

- [Ethereum Linux x86-64](https://raw.githubusercontent.com/ethereum/solc-bin/gh-pages/linux-amd64/list.json)
- [Ethereum macOS x86-64](https://raw.githubusercontent.com/ethereum/solc-bin/gh-pages/macosx-amd64/list.json)
- [TRON Linux x86-64](https://raw.githubusercontent.com/tronprotocol/solc-bin/main/linux-amd64/list.json)
- [TRON macOS x86-64](https://raw.githubusercontent.com/tronprotocol/solc-bin/main/macosx-amd64/list.json)

TRON publishes ZIP archives. Each download is authenticated before parsing; the
selected regular executable member is also pinned by its independently measured
SHA-256. Paths, duplicate names, special files, and member sizes are checked.
Only the selected member is read into memory, and no archive path is extracted.
These are reviewed digest pins; the corridor does not claim GPG verification.

Linux requires x86-64. macOS requires x86-64 execution, including Rosetta on
arm64; unsupported hosts fail before execution. The runner reads a stable regular
file without following symlinks, authenticates its bytes and native format, then
creates a private mode-0700 directory containing a mode-0500 verified copy.
Version and standard-JSON compilation use that same retained copy with a clean
environment and bounded execution/output. Inputs contain source text directly,
and output selection is limited to the exact admitted EVM artifact fields.

Rebuild and verify the reviewed artifacts:

```sh
python3 scripts/contract_artifact_corridor.py build --output-dir /tmp/sccp-artifacts
python3 scripts/contract_artifact_corridor.py verify \
  --manifest /tmp/sccp-artifacts/sccp-contract-artifacts-v1.json --check-source-inputs
```

Updating the reviewed artifact lock requires an explicit new output path via
the `lock` command and review of its source, size and manifest changes.
`materialize` and `compile-input` provide the same authenticated native execution
to the Node test harnesses. Compiler/runtime substitutions are not supported.

Native compilation is distinct from deployment qualification. The EVM smoke's
TRON-source diagnostic compilation uses the EVM compiler and must differ from
the governed TVM output. Actual TVM deployment, precompile semantics and receipts
require `contract_tvm_runner.sh` on the pinned real TRE runtime. macOS Rosetta
compiler execution alone supplies no Linux execution or TVM deployment evidence.

The EVM diagnostic runtime uses the pinned native EDR `0.12.1` Node-API engine
directly. Its adapter keeps chain instances isolated, enforces contract and gas
limits, and reports mined transaction failures. The unused Hardhat CLI/compiler
package was removed because its `adm-zip` dependency has an unpatched extraction
advisory, [GHSA-vwc7-r8mq-g2x9](https://github.com/advisories/GHSA-vwc7-r8mq-g2x9).
Both runtime dependency trees still require a clean low-severity npm audit.
