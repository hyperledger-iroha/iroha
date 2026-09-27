# SCCP v1 destination contract for Ethereum, BSC and TRON

`SccpTairaXor.sol` is the single SCCP v1 destination for the three EVM-family
profiles of [`specs/sccp.md`](../../../specs/sccp.md) (§5.1, §5.2). It is the
ERC-20/BEP-20/TRC-20 token **Taira XOR** (`tXOR`, 9 decimals, one token unit =
one Taira unit) with the bridge built in. It has no owner, guardian, admin key,
setter, upgrade path, external call or roster-signed mint breaker. The minting
pause changes only through `applyControl` with a Parliament control leaf of an
attested Taira block.

| Profile | Tag | Chain id | Domain | Route id | Compiler |
|---|---|---|---|---|---|
| `ethereum-mainnet` | `0x41` | 1 | 1 | `taira_eth_xor` | solc `0.8.31+commit.fd3a2265` |
| `bsc-mainnet` | `0x42` | 56 | 2 | `taira_bsc_xor` | solc `0.8.31+commit.fd3a2265` |
| `tron-mainnet` | `0x43` | `0x2b6653dc` | 5 | `taira_tron_xor` | tronprotocol `tv_0.8.31` `0.8.31+commit.c2812a3d` |

## Deployment

```solidity
constructor(bytes32 tairaNetworkId, uint8 networkTag, uint32 routeRevision,
            uint256 maxWrappedSupply, RosterV1 initialRoster)
```

The constructor requires `block.chainid` to equal the tag's identity word, a
nonzero `NetworkId` and revision, a cap in `1..2^128-1`, and a §3.7-valid
initial roster (4..=31 members, `t = ⌊2n/3⌋+1`, zero slots first, then
strictly ascending addresses) whose validity satisfies the §5.1.5 bounds. It
stores the immutables `INITIAL_ROSTER_DIGEST`, `INITIAL_GENERATION`,
`TAIRA_NETWORK_ID`, `NETWORK_TAG`, `ROUTE_REVISION`, `MAX_WRAPPED_SUPPLY`,
`DOMAIN_SEPARATOR` and `REQUIRE_DIRECT_CALLER` (TRON only). `controlNonce`,
`opCount` and the previous roster start at zero and `mintingPaused` is false.
The deployed runtime equals the locked template in
`scripts/contract_tooling/artifact-lock.json` with those eight immutables
filled at the recorded `immutable_references`; the EDR suite checks this
byte for byte.

## Entry points

| Selector | Function | Rule |
|---|---|---|
| `0x8056d161` | `finalizeFromTaira` | §5.1.3 direct: roster accepted, `t` signatures, payload, deadline, leaf, nonce bit, cap, mint |
| `0x96925736` | `finalizeFromTairaHistorical` | §5.1.3 through the attested history root |
| `0x909ea456` | `rotateRosters(RotationV1[])` | §5.1.5, 1..=16 sequential rotations signed by the current roster |
| `0x0ce970d6` | `applyControl` | §5.1.6 Parliament control leaf, strictly increasing `controlNonce` (gaps allowed) |
| `0x935a913b` | `applyControlHistorical` | §5.1.6 through the history root |
| `0xebfc6ca8` | `transferToTaira` | §5.1.7 canonical calldata only, per-sender nonce, burn |
| `0xc3de98ad` / `0xbe335b84` | `voidExpired` / `voidExpiredHistorical` | §5.1.8 after the deadline, same nonce bit as mint |
| `0x5b094c00` | `voidFrozen(first, count)` | §5.1.8, 1..=256 nonces once both rosters expired |

Views: `rosterState`, `isConsumed`, `transferNonces`, `tairaNetworkId`,
`routeRevision`, `maxWrappedSupply`, `mintingPaused`, `controlNonce`
(`0x4faac8ca`), `domainSeparator`, `initialRosterDigest`,
`initialRosterGeneration`, `opCount`, `maxRosterValidityMs`, plus the ERC-20
surface with ERC-6093 errors. `opCount` counts finalizations, voids, burns and
applied controls, never rotations. Burns, rotations and voids stay open while
minting is paused; `applyControl` needs an accepted roster, so a frozen
destination (current roster expired) can only burn and, once the previous
roster has also expired, `voidFrozen`.

Verification details: the calldata roster is hashed and checked for `n`, `t`
and ordering on the fly; signatures are positional (`signerBitmap`), low-S,
`v ∈ {27, 28}`, nonzero `r`/`s`, and every `ecrecover` result is masked to 160
bits and compared with a nonzero member. The consumed set is a
`mapping(uint256 => uint256)` bitmap (word `nonce >> 8`, bit `nonce & 255`).
Storage follows §5.2.3 (slots A..D, then the bitmap, transfer nonces and the
ERC-20 state). On TRON `transferToTaira`, `voidExpired*` and `voidFrozen`
require `msg.sender == tx.origin`.

## Build, verify and test

```sh
python3 scripts/contract_artifact_corridor.py build     # target/sccp-contract-artifacts/
python3 scripts/contract_artifact_corridor.py verify
(cd scripts/contract_tooling/evm-runtime && npm ci --ignore-scripts)
node --test contracts/evm/sccp/test/sccp_taira_xor.test.js
bash scripts/sccp_evm_contract_smoke.sh                  # all of the above, fail-closed, in a private directory
```

The toolchain policy (native arm64/x86-64 compilers, no Rosetta or Docker,
cancun legacy pipeline without metadata hash or CBOR) is described in
[`scripts/contract_tooling/README.md`](../../../scripts/contract_tooling/README.md).
The source must not `delete` memory `bytes` elements or declare a custom
storage layout (0.8.31 legacy-pipeline bug patterns); the corridor enforces it.

`test/sccp_taira_xor.test.js` runs every §11 Contracts bullet under chain ids 1,
56 and `0x2b6653dc` on the locked native EDR runtime: roster expiry,
previous-roster grace and its cap, batched sequential rotation (up to 16) and
every validity, skew and `validFromMs` bound; the frozen destination; deadline
edges; `voidExpired` and `voidFrozen` range and revert rules; the supply cap and
bitmap word boundaries; `applyControl` pause and resume, stale and equal
nonces, nonce gaps, historical mode, foreign network, target, destination and
revision leaves, and a transfer leaf offered as a control; non-canonical
`transferToTaira` calldata (offset, padding, trailing bytes); the TRON
direct-caller rule; the initial immutables, `opCount` and `controlNonce() == 0`;
signature malleability (high-S, `v ∉ {27, 28}`, zero or out-of-range `r`/`s`,
duplicate, outsider and zero-slot signers, unordered rosters); and a runtime
`WrongChain` on a foreign chain id. `test/sccp_v1_model.js` is an independent
ethers-based model of the §3 encodings; every typehash, topic and selector is
computed from its canonical string and compared with the spec and the ABI.
`SCCP_GAS_REPORT=<file>` writes the measured gas as JSON.

The TRON profile runs on EDR with the Ethereum-compiler build, which checks the
contract logic under TRON's chain id, codecs and direct-caller rule. The
tronprotocol build itself (with its `CALLTOKENID`/`CALLTOKENVALUE` guards),
TRON energy and the `ecrecover` masking golden are qualified on java-tron (TRE)
separately.

## Measured gas (§5.4)

Measured on the locked EDR `0.12.1` runtime with Osaka rules (EIP-7623
calldata floor included), `n` roster members and `t` signatures. "New balance,
same bitmap word" is the typical mint: a first-time recipient, a warm
consumed-bitmap word and a nonzero supply. The TRON column is the EVM build on
EDR under chain id `0x2b6653dc`, not TRON energy.

| Operation | §5.4 estimate | ETH (1) | BSC (56) | TRON profile |
|---|---|---|---|---|
| `finalizeFromTaira` n=4 t=3, new balance, same word | 75k–110k | 101,304 | 101,420 | 101,559 |
| `finalizeFromTaira` n=4 t=3, existing balance, same word | 75k–110k | 85,721 | 85,861 | 85,976 |
| `finalizeFromTaira` n=4 t=3, first mint (new word, zero supply) | — | 155,691 | 155,831 | 155,946 |
| `finalizeFromTaira` n=31 t=21, new balance, same word | 160k–200k | 199,061 | 199,201 | 199,364 |
| `finalizeFromTaira` n=31, all 31 signatures | — | 246,295 | 246,447 | 246,634 |
| `finalizeFromTairaHistorical` n=4, history size 6 | direct + 8k–20k | 108,146 (+6.8k) | 108,262 | 108,365 |
| `rotateRosters` 1 rotation n=4 (steady state) | 65k–80k | 76,046 | 76,102 | 76,115 |
| `rotateRosters` 1 rotation n=31 (steady state) | 160k–180k | 186,584 | 186,616 | 186,593 |
| `rotateRosters` 1 rotation n=31, first rotation | 160k–180k | 203,648 | 203,680 | 203,669 |
| `rotateRosters` 3 / 16 rotations n=4 | — | 161,060 / 594,155 | 161,092 / 594,415 | 161,093 / 594,620 |
| `voidExpired` n=4 | finalize − 20k | 73,740 | 73,892 | 74,068 |
| `voidExpiredHistorical` n=4 | — | 75,289 | 75,417 | 75,605 |
| `voidFrozen` 1 nonce | — | 37,073 | 37,105 | 37,155 |
| `voidFrozen` 256 nonces, one / two bitmap words | ≈55k + 1.2k per nonce (≈362k) | 449,933 / 472,563 | 449,965 / 472,595 | 450,015 / 472,645 |
| `applyControl` n=4 t=3 | 55k–85k | 65,458 | 65,546 | 65,560 |
| `applyControl` n=31 t=21 | 140k–175k | 161,238 | 161,338 | 161,304 |
| `applyControlHistorical` n=4, history size 6 | — | 68,338 | 68,402 | 68,392 |
| `transferToTaira` (34-byte Taira recipient) | 55k–75k | 68,811 | 68,939 | 69,006 |
| deployment (n=4) | — | 3,012,402 | 3,012,454 | 3,012,475 |

Two measurements exceed the estimates. A steady-state n=31 rotation costs
≈187k (≈204k for the first rotation, which initializes the previous-roster
slots) against 160k–180k: it pays 21 recoveries, two 31-member roster hashes and
≈2.6 KB of calldata. `voidFrozen` costs ≈1.6k per nonce against 1.2k because
every nonce emits its own `SccpVoided(0, nonce)` LOG3 (1.5k alone).
