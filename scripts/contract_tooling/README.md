# SCCP contract toolchain (EVM and TRON)

`specs/sccp.md` §5.5. One Solidity source, `contracts/evm/sccp/SccpTairaXor.sol`,
is compiled by two pinned native compilers:

| Target | Compiler | Platforms pinned in `compiler-lock.json` |
|---|---|---|
| `evm` (ETH, BSC) | Ethereum solc `0.8.31+commit.fd3a2265` | macOS universal (arm64 + x86-64), Linux x86-64, Linux arm64 |
| `tron` | tronprotocol `tv_0.8.31` `0.8.31+commit.c2812a3d` | `solc-macos` universal, `solc-static-linux`, `solc-static-linux-arm` |

Settings (both targets): `evmVersion: "cancun"` (explicit, because both
compilers default to `osaka`), `optimizer: {enabled: true, runs: 200}`,
`viaIR: false` (legacy pipeline), `metadata.bytecodeHash: "none"`,
`metadata.appendCBOR: false`. The source must keep the exact
`pragma solidity 0.8.31;`, declare no imports or custom storage layout, and
never use `delete` (the 0.8.31 legacy-pipeline bug patterns); the corridor
rejects a violating source before compiling it.

## Compiler authentication

The Ethereum SHA-256 pins match `binaries.soliditylang.org/<platform>/list.json`;
the TRON pins match the `tv_0.8.31` release `shasum.txt` and the GitHub asset
digests. `scripts/contract_artifact_corridor.py` checks the complete compiler
record against its reviewed constants before downloading anything, then
authenticates each download (SHA-256, ELF machine or universal Mach-O slices)
and caches it by digest under `target/sccp-contract-tooling/compilers`. Every
execution re-reads a stable regular file without following symlinks,
re-authenticates it, copies it into a private mode-0700 directory as mode 0500,
checks the exact `--version` banner (`solc` vs `solc.tron`) and runs
`--standard-json` with a clean environment and bounded output. Compilers run
natively on macOS arm64/x86-64 and Linux x86-64/arm64; a Python process under
Rosetta translation (or one whose Rosetta probe cannot run) is refused and
Docker is never used. An EVM/TRON compiler alias (shared identity or executable
digest) is refused before anything is downloaded or run. Compiler warnings fail
the build.

## Commands

```sh
python3 scripts/contract_artifact_corridor.py build    # compile, check against artifact-lock.json, publish
python3 scripts/contract_artifact_corridor.py verify   # re-authenticate a manifest against both locks and the sources
python3 scripts/contract_artifact_corridor.py lock     # recompile and rewrite artifact-lock.json (review the diff)
python3 scripts/contract_artifact_corridor.py materialize --target evm --output <new path>
python3 scripts/contract_artifact_corridor.py compile-input --target tron --compiler <path> < input.json
```

`build` writes `target/sccp-contract-artifacts/sccp-contract-artifacts-v1.json`
(override with `--output-dir`); `verify` defaults to that manifest. The
manifest carries, per target, the compiler identity, settings digest, source
inventory, ABI, creation and runtime bytecode with SHA-256 and Keccak-256,
metadata, and the named runtime immutable references. `artifact-lock.json`
records for each contract the creation digest, the complete runtime template,
its `immutable_references` (`name`, AST id, `start`, `length`) and the ABI
digest, plus the digests of the compiler lock and the whole manifest. Any
drift fails closed. The TRON runtime must differ from the EVM runtime, which
proves that the TRON compiler (with its `CALLTOKENID`/`CALLTOKENVALUE`
guards) produced it. `iroha sccp deployment verify` fills the eight §5.2.3
immutables into the locked template and compares it with the deployed code.

`scripts/contract_native_solc.js` exposes `compile-input` to Node harnesses.

## EVM runtime

`evm-runtime/` pins the native EDR `0.12.1` Node-API engine and ethers
`6.16.0` for the EDR suite (`contracts/evm/sccp/test/sccp_taira_xor.test.js`).
Install it with `npm ci --ignore-scripts` in that directory; `node_modules/`
is ignored by git. `edr-provider.js` creates isolated in-memory chains with
explicit chain ids, gas limits and Hardhat-style time control. The
`scripts/sccp_evm_contract_smoke.sh` smoke installs and audits a private copy
(`npm audit --omit=dev --audit-level=low`) before running the suite.
EDR execution is never TVM evidence: TRON deployment, energy and precompile
semantics are qualified on the pinned TRE image (`tvm_runner` in
`compiler-lock.json`).
