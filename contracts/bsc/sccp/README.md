# SCCP BSC Contracts

This directory contains the BSC/BEP20 deployment entrypoints for the TAIRA XOR
SCCP testnet route.

Files:

- `TairaXOR.sol`: BEP20/ERC20-compatible bridged XOR token. The owner sets one
  bridge, locks it, and only that bridge can mint or burn.
- `SccpBscSourceBridge.sol`: owner-governed source emitter for BSC-origin SCCP
  events. It is fixed to source domain BSC (`2`) and target domain SORA (`0`).
- `TairaXorBscSccpBridge.sol`: route-bound bridge for `taira_bsc_xor`.
  TAIRA-origin proofs mint bridged XOR on BSC; BSC-origin burns emit source
  event digests for TAIRA settlement.

Deployment shape:

1. Deploy `SccpGroth16Bn254MessageVerifier` from `contracts/evm/sccp`.
2. Deploy `SccpBscSourceBridge` with BSC testnet network id `97`, source domain
   `2`, and target domain `0`.
3. Deploy `TairaXOR`.
4. Deploy `TairaXorBscSccpBridge` with the token, raw verifier, source bridge,
   verifier runtime code hash, verifier key hash, backend
   `evm-groth16-bn254-v1`, proof family `stark-fri-v1`, network id `97`,
   source domain `0`, target domain `2`, route hash `keccak256("taira_bsc_xor")`,
   and asset hash `keccak256("xor")`.
5. Call `TairaXOR.setBridge(route_bridge)`, `TairaXOR.lockBridge()`, and
   `SccpBscSourceBridge.transferOwnership(route_bridge)`.

The route bridge computes the EVM destination binding itself using the raw
verifier address plus the route bridge address. Live readback should record:

- `TairaXOR.bridge()` equals the route bridge.
- `TairaXOR.bridgeLocked()` is `true`.
- `SccpBscSourceBridge.owner()` equals the route bridge.
- `TairaXorBscSccpBridge.destinationBindingHash()` equals the manifest binding.
- `TairaXorBscSccpBridge.verifier()`, `verifierCodeHash()`,
  `verifierKeyHash()`, `networkId()`, `expectedSourceDomain()`, and
  `expectedTargetDomain()` match rollout evidence.

The raw verifier does not expose `destinationBindingHash()`. Tooling must not
require that getter on the verifier address for BSC.

Operator helper:

```bash
NODE_PATH=/path/to/node_modules \
  node scripts/sccp_bsc_taira_xor_deploy.mjs compile
SCCP_BSC_DEPLOYER_PRIVATE_KEY=<runtime-only-funded-testnet-key> \
NODE_PATH=/path/to/node_modules \
  node scripts/sccp_bsc_taira_xor_deploy.mjs deploy \
    --verifier artifacts/sccp-bsc/bsc-testnet-verifier-key.json \
    --broadcast true \
    --confirm-testnet taira_bsc_xor \
    --out artifacts/sccp-bsc/taira-bsc-xor-deployment.evidence.json
```

`deploy` refuses to broadcast without `--confirm-testnet taira_bsc_xor`. The
private key is read only from the named environment variable and is never
written to the evidence artifact. The resulting evidence JSON is public
deployment/readback material consumed by the wallet route-manifest helper; it
still needs TAIRA burn-record material plus TAIRA route publication/canary
evidence before a production-ready manifest can be written. Production canary
evidence must include distinct source-event and route-canary transaction ids
plus canonical `https://testnet.bscscan.com/tx/0x...` URLs matching those ids.

Quick verification:

```bash
scripts/sccp_evm_contract_smoke.sh
node --test scripts/sccp_bsc_taira_xor_deploy.test.mjs
tmpdir=$(mktemp -d /tmp/iroha-bsc-smoke-deps.XXXXXX)
npm install --prefix "$tmpdir" --silent solc@0.7.4 ethers@6.16.0 ganache@7.9.2 >/dev/null
NODE_PATH="$tmpdir/node_modules" node scripts/sccp_bsc_taira_xor_deploy_smoke.mjs
rm -rf "$tmpdir"
```

The smoke compiles the shared EVM verifier/wrapper, TRON route contracts, and
BSC route contracts, then runs mint, replay, payload parsing, burn, and source
event assertions on Ganache. The deploy-helper smoke separately runs a local
BSC-testnet-shaped Ganache chain, deploys the verifier/source/token/route
contracts through the operator helper, registers and locks `TairaXOR`, transfers
source-bridge ownership, validates readback evidence, and scans the public
evidence artifact for secret-like material.
