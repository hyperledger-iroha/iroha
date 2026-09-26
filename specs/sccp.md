# SCCP v1: SORA Cross-Chain Protocol (first release)

Status: normative design, first release. This document supersedes
`specs/bridge_proofs.md` in full (that document describes only the retired
Groth16/replay-archive SCCP). Generic Sumeragi bridge finality
(`specs/bridge_finality.md`) is unaffected.

SCCP moves Taira XOR between the Taira Iroha network and four external
mainnets. The first release admits exactly these profiles:

| Profile | Tag | SCCP domain | Native identity |
|---|---|---|---|
| `sora-taira` | `0x40` | 0 | runtime `NetworkId` (genesis header hash) |
| `ethereum-mainnet` | `0x41` | 1 | EVM chain id 1 |
| `bsc-mainnet` | `0x42` | 2 | EVM chain id 56 |
| `tron-mainnet` | `0x43` | 5 | TVM chain id `0x2b6653dc` |
| `ton-mainnet` | `0x44` | 4 | TON global id −239 |

There are no testnet profiles, no alias profiles and no compatibility layouts.

## 0. Conventions

- MUST, MUST NOT, SHOULD and MAY are used in the RFC 2119 sense.
- `‖` is byte concatenation. `u8`, `u16`, `u32`, `u64`, `u128` are unsigned
  integers encoded **big-endian** with exactly that width. `bytes32` is 32 raw
  bytes. `word(x)` is the 32-byte big-endian left-zero-padded encoding of an
  unsigned integer or 20-byte address (Solidity `abi.encode` of a static value).
- `keccak256` is Ethereum Keccak-256 (not SHA3-256). `ASCII("…")` is the literal
  byte string without terminator or length prefix.
- `N` is the secp256k1 group order
  `0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141`;
  `HALF_N = 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0`.
- "Taira-internal" structures are Norito types (`norito::{Encode, Decode}`,
  `norito::json`) and are only ever interpreted by Taira nodes and Rust tools.
  "Contract-visible" structures have the explicit byte layouts in §3 and MUST
  NOT depend on Norito, Rust enum discriminants or JSON spelling.
- Heights, epochs and timestamps refer to Taira unless qualified.

## 1. Design summary

### 1.1 Trust model in one paragraph

Taira→external transfers are authorized by **bridge attestations**: for each
committed Taira block that carries SCCP messages, and for each epoch-boundary
block, the validators of that block's roster sign a fixed-layout EIP-712
digest over the block's SCCP commitment root, a history root, and the current
and (at rotations) next roster digest with dedicated secp256k1 **bridge keys**.
Destination contracts keep a roster light client (only a digest on EVM/TRON,
the member list on TON), verify `⌊2n/3⌋+1` signatures with `ecrecover` /
`ECRECOVER`, verify a keccak Merkle path to the message, consume the message
in an on-chain set and mint under an immutable supply cap. There is no trusted
setup, no prover, no relayer and no archive: anyone can submit any step.
External→Taira transfers are verified natively by every Taira node against
permissionless on-chain light clients of the source chain. Safety of Taira→
external equals the Taira consensus assumption (at most `f` of `3f+1`
validators faulty); safety of external→Taira equals each source chain's own
finality assumption plus the Taira consensus assumption.

### 1.2 Settled sub-decisions

| Question | Decision | Why |
|---|---|---|
| Attestation transport | Self-authenticating `SubmitSccpAttestationsV1` instruction in ordinary transactions, auto-submitted by each validator node, fee-exempt behind admission pre-verification (§4.8) | Durable, peer-identical, state-derived bytes that any Torii serves; no Sumeragi wire change while v2 is being redesigned; a stalled signer cannot halt consensus. The digest is transport-independent so a later move to Commit-vote extensions changes no contract (§12). P2P gossip is rejected: not durable, not state-derived. |
| Where the commitment root lives | Post-execution world state (`sccp_block_commitments[h]`), authenticated by the Commit QC's `ExecutionCommitment`; `BlockHeader::sccp_commitment_root` is deleted | The v2 proposer signs a result-less proposal; a header root authored before execution is structurally wrong and breaks on any failed SCCP transaction (§4.5). |
| Bridge-key registry | New map keyed by `PeerId`, set by `SetSccpBridgeKeyV1` carrying a consensus-key consent signature and a secp256k1 proof of possession; permanent never-reused address index; genesis entries for epoch 0 (§4.2) | The consensus-key registry is indexed by the peer's own BLS key and admin-gated; staking registration is lane-scoped and refuses fresh global candidates. |
| Attestation roster | `HeightContext` consensus roster mapped to active bridge addresses, sorted ascending by address (keyless slots as zero, first), `t = ⌊2n/3⌋+1`, grouped into **generations** that change only when the address list changes or the heartbeat age is reached (§4.3) | Deterministic, publicly derivable; destinations rotate only on real change or heartbeat, not every 4 h epoch; ascending order lets contracts enforce signer uniqueness in O(1) per signature. |
| Fees and permissions | Attestation and fault evidence: fee-exempt, self-authorizing, admission-pre-verified. Everything else: ordinary fees paid by the submitter; no permission tokens (§4.18) | Signatures are the authority; fee-exempt roots are safe only when admission rejects invalid payloads before the queue. |
| Storage/pruning | Messages, consumed sets, history leaves, rosters, subjects: permanent. Signatures: pruned after `attestation_retention_blocks` except rotation attestations until the outgoing roster expires (§4.10) | Old messages remain provable through the history root under any newer attestation. |
| Equivocation | `SubmitSccpAttestationFaultV1`: any valid bridge signature over a non-canonical statement; phase 1 records the fault and evicts the key from the next generation; phase 2 slashes through the existing consensus penalty pipeline (§4.11) | Off-chain signatures are otherwise unaccountable. |
| Long-range / weak subjectivity | Every roster generation carries `valid_until_ms`; Taira forces a new generation at least every `roster_max_age_ms` (heartbeat); destinations refuse expired rosters (finalization and rotation) and freeze permanently if not rotated in time; Taira unbonding must exceed `roster_validity_ms` (§5.1.5, §9.8) | Bounds the window in which retired keys can be used against a lagging destination to a window in which they are still slashable. |
| Old messages | Append-only **history accumulator** over SCCP-bearing blocks; every attestation signs its root, so any recent attestation proves any old message (§3.5) | Destinations only need the current roster (plus a 24 h grace for the previous one). |
| Route registration without Parliament | Bridge-roster quorum governance: `ApproveSccpGovernanceV1` accumulates EIP-712 signatures by bridge keys; `t` executes, `f+1` suffices for pause/freeze actions (§4.14) | Parliament cannot be seated on Taira (one citizen, no manager); the same `t` keys already control minting, so this adds no trust assumption. |
| Destination mint breaker | Replaced by a roster-controlled breaker: `f+1` bridge signatures pause minting, `t` resume; burns always open (§5.1.6) | Removes the five privileged guardian keys; still lets an honest minority halt minting on a Taira-side bug. |
| Destination replay protection | Dense per-(network, revision) outbound nonce assigned by Taira; EVM/TRON bitmap `mapping(uint256 ⇒ uint256)`, TON bucket child contracts of 512 flags (§5) | One storage word per 256 messages instead of one slot per message; TON account-state cap (65 536 cells) forbids an in-contract dictionary. |
| Token/bridge shape | One contract per destination: ERC-20/TRC-20 with the bridge built in (EVM/TRON); one Jetton master with the bridge built in plus standard wallets and consumption buckets (TON) | Removes the TRON deployment cycle (token↔route), all cross-contract minting calls and code-hash rechecks. |
| Taira resets | Taira identity is the runtime `NetworkId`; nothing Taira-specific is compiled into Rust or contracts; a reset creates new deployments (§4.17) | The old compiled genesis hash made every reset and devnet unusable. |
| Contract toolchains | Solidity 0.8.31 (`solc` +commit.fd3a2265 for ETH/BSC; tronprotocol `tv_0.8.31` +commit.c2812a3d for TRON), legacy pipeline, `evmVersion: cancun`; Acton 1.2.0 / Tolk 1.4.2 for TON. All ship native macOS arm64 binaries (§5.5) | The pinned 0.7.6 compiler is x86-64 only and needs Rosetta. |

### 1.3 End-to-end shape

```mermaid
sequenceDiagram
    participant U as User wallet (local Rust)
    participant T as Any Taira peer (Torii)
    participant V as Taira validators (attestor)
    participant D as Destination contract
    U->>T: RecordSccpMessage (signed tx)
    Note over T,V: block h commits; post-state holds sccp_block_commitments[h] and subject[h]
    V->>T: SubmitSccpAttestationsV1 (block h+1..h+2)
    U->>T: GET /v1/sccp/messages/{id}/proof
    U->>D: rotateRoster(...) if D lags, then finalizeFromTaira(...)
    D->>D: verify t signatures, Merkle path, nonce bitmap, cap; mint
    U->>D: transferToTaira(recipient, amount, nonce)  (reverse direction)
    U->>T: [AdvanceSccpLightClientV1…, SubmitSccpInboundMessageV1] (signed tx)
    T->>T: native light-client proof check, consumed set, escrow release
```

## 2. Networks, identities and lanes

### 2.1 Identity words

Each profile has a 1-byte tag and a 32-byte identity word:

| Profile | `tag` | `identity_word` |
|---|---|---|
| `sora-taira` | `0x40` | the 32 bytes of Taira's `NetworkId` (hash of the genesis block header), read from world state at runtime |
| `ethereum-mainnet` | `0x41` | `word(1)` |
| `bsc-mainnet` | `0x42` | `word(56)` |
| `tron-mainnet` | `0x43` | `word(0x2b6653dc)` |
| `ton-mainnet` | `0x44` | two's-complement `int256(−239)`, i.e. `0xFF…FF11` |

`network_bytes(p) = tag(p) ‖ identity_word(p)` (33 bytes).

The Taira `ChainId` label, the compiled `SCCP_TAIRA_CHAIN_ID_V1`,
`SCCP_TAIRA_GENESIS_HASH_V1` and `SCCP_TAIRA_FINALITY_NETWORK_ID_V1`
constants are removed. The I105 discriminant is not part of any SCCP hash.

### 2.2 Lanes

A lane is a directed pair with exactly one Taira endpoint:

```
lane_bytes(source, target) = network_bytes(source) ‖ network_bytes(target)   // 66 bytes
```

Taira→X lanes carry outbound messages (minted on X); X→Taira lanes carry
inbound messages (burned on X, released on Taira).

### 2.3 Routes

One route per external profile, identified by `route_id` text:

| Profile | `route_id` | Destination token | Token decimals | `token_units_per_taira_unit` |
|---|---|---|---|---|
| ETH | `taira_eth_xor` | ERC-20 bridge contract | 18 | 10^9 |
| BSC | `taira_bsc_xor` | BEP-20 bridge contract | 18 | 10^9 |
| TRON | `taira_tron_xor` | TRC-20 bridge contract | 18 | 10^9 |
| TON | `taira_ton_xor` | Jetton master (bridge) | 9 | 1 |

Taira XOR has scale 9 (existing Taira genesis asset definition, Global policy).
A route has numbered **revisions** (`u32`, from 1); each revision binds exactly
one deployed destination contract (§4.14). `asset_id` text is `xor`.

## 3. Canonical contract-visible encodings

### 3.1 Account codecs

| Codec | Name | Bytes | Validity |
|---|---|---|---|
| 1 | `canonical_text` | 1..=256 | printable ASCII `0x21..=0x7e`; used for `asset_id` and `route_id` only |
| 2 | `evm_address20` | 20 | nonzero |
| 3 | `taira_account` | 1..=256 | canonical Iroha `AccountAddress` payload bytes (header ‖ controller; the I105 payload without discriminant or checksum). Contracts check only the length; Taira decodes (§4.12.5) |
| 5 | `tron_address21` | 21 | first byte `0x41`, remaining 20 bytes nonzero |
| 7 | `ton_account36` | 36 | `i32` workchain (big-endian) = 0, then a nonzero 32-byte account id |

Codecs 4 and 6 are unassigned. The previous I105-text Taira account encoding
and its on-chain base-105/Ed25519 validation are removed.

### 3.2 Transfer payload

```
payload =
  u8   kind            = 0x02            // Transfer
  u8   version         = 0x01
  u32  source_domain
  u32  dest_domain
  u64  nonce
  u32  route_revision                    // nonzero
  u32  asset_home_domain = 0             // SORA
  u8   asset_id_codec    = 1
  u16  asset_id_len ‖ asset_id           // "xor"
  u128 amount                            // Taira units (scale 9), nonzero
  u8   sender_codec    ‖ u16 sender_len    ‖ sender
  u8   recipient_codec ‖ u16 recipient_len ‖ recipient
  u8   route_id_codec  = 1
  u16  route_id_len ‖ route_id           // e.g. "taira_eth_xor"
```

Rules (decoders MUST enforce all of them and MUST reject trailing bytes):

- Each variable field is 1..=256 bytes. Total payload ≤ 4 096 bytes.
- `source_domain ≠ dest_domain`, exactly one of them is 0.
- Sender/recipient codecs are determined by the domains:

| Direction | `sender_codec` | `recipient_codec` |
|---|---|---|
| Taira → ETH/BSC | 3 | 2 |
| Taira → TRON | 3 | 5 |
| Taira → TON | 3 | 7 |
| ETH/BSC → Taira | 2 | 3 |
| TRON → Taira | 5 | 3 |
| TON → Taira | 7 | 3 |

- `route_id` MUST equal the route of the external endpoint; `route_revision`
  MUST name a registered revision of that route.

The field set is the first-release canonical transfer payload; only the integer
byte order (now big-endian), the length prefix width (now `u16`) and the Taira
account codec changed, so contracts parse without byte swaps.

### 3.3 Payload hash and message id

```
payload_hash = keccak256(ASCII("SCCP/PAYLOAD/V1") ‖ payload)
message_id   = keccak256(ASCII("SCCP/MESSAGE/V1") ‖ lane_bytes(source, target) ‖ payload_hash)
```

`message_id` is the global identity of a message in both directions. It binds
the Taira network identity, so it differs across Taira resets.

### 3.4 Block commitment tree

Leaves are the outbound messages recorded by one Taira block, in
`commitment_index` order (0-based execution order, dense; §4.5):

```
destination_word = word(address20)                  // EVM/TRON: contract address, TRON without 0x41
                 | ton_account_id32                 // TON: account id of the Jetton master (workchain 0)
leaf  = keccak256(ASCII("SCCP/LEAF/V1") ‖ message_id ‖ destination_word)
node(l, r) = keccak256(ASCII("SCCP/NODE/V1") ‖ l ‖ r)
```

Tree rule ("promote-odd"): level 0 is the leaf list. Each next level pairs
elements left to right; an unpaired last element is promoted unchanged. The
root of a single-leaf tree is that leaf. A block has 1..=512 leaves
(`SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1`), so paths have at most 9 siblings.

Verification is positional and binds the leaf index and count:

```
fn merkle_root(leaf, index, count, siblings) -> bytes32:
    require count ≥ 1 and index < count
    h, k = leaf, 0
    while count > 1:
        if index is odd:              h = node(siblings[k], h); k += 1
        elif index + 1 < count:       h = node(h, siblings[k]); k += 1
        # else: promoted, no sibling at this level
        index = index >> 1
        count = (count + 1) >> 1
    require k == len(siblings)
    return h
```

### 3.5 History accumulator

Taira keeps an append-only list of every SCCP-bearing block (message count
> 0), in height order:

```
history_leaf(height, sccp_root, message_count) =
    keccak256(ASCII("SCCP/HISTORY/V1") ‖ u64 height ‖ sccp_root ‖ u32 message_count)
history_root(size) = promote-odd root (§3.4 node rule) over the first `size` history leaves
```

`history_root(0) = 0x00…00`. The promote-odd tree equals a right-bagged Merkle
mountain range: with the perfect-subtree roots `P_a, P_b, …, P_z` of the binary
decomposition of `size` (largest first), `root = node(P_a, node(P_b, … node(P_y, P_z)))`.
Taira therefore maintains it in O(log size) state (the peaks) and verifiers use
the same `merkle_root` function with `(leaf_index, history_size)`. History
paths have at most 32 siblings (`history_size ≤ 2^32` is enforced).

### 3.6 EIP-712 domain and signed structures

Everything a bridge key ever signs is EIP-712 typed data under one domain:

```
EIP712Domain(string name,string version,bytes32 salt)
name    = "SCCP"
version = "1"
salt    = Taira NetworkId (32 bytes)

DOMAIN_TYPEHASH  = keccak256("EIP712Domain(string name,string version,bytes32 salt)")
                 = 0x599a80fcaa47b95e2323ab4d34d34e0cc9feda4b843edafcc30c7bdf60ea15bf
NAME_HASH        = keccak256("SCCP") = 0xd7bacbdfe367013f66397ff7242325c31126ab14c95620817c994f22f1d123ba
VERSION_HASH     = keccak256("1")    = 0xc89efdaa54c0f20c7adf612882df0950f5a951637e0307cdcb4c672f298b8bc6
DOMAIN_SEPARATOR = keccak256(DOMAIN_TYPEHASH ‖ NAME_HASH ‖ VERSION_HASH ‖ salt)
digest(struct)   = keccak256(0x19 ‖ 0x01 ‖ DOMAIN_SEPARATOR ‖ hashStruct(struct))
hashStruct(s)    = keccak256(TYPEHASH(s) ‖ word(field_1) ‖ … ‖ word(field_k))
```

The domain deliberately omits `chainId` and `verifyingContract`: one Taira
attestation is valid on every destination; destination binding lives in the
Merkle leaf (§3.4). All struct fields are static, so `hashStruct` is a fixed
number of 32-byte words. The `0x1901` prefix separates these digests from RLP
transactions and `personal_sign` messages.

| Struct | Type string | TYPEHASH |
|---|---|---|
| Attestation | `SccpAttestation(uint64 height,uint64 epoch,uint64 timestampMs,bytes32 blockHash,bytes32 sccpRoot,uint32 messageCount,bytes32 historyRoot,uint64 historySize,bytes32 rosterDigest,bytes32 nextRosterDigest)` | `0x6ea54d1f320a2e5892d6a362adc4b569967facc991d3a327cbc84b746f520c66` |
| Key proof of possession | `SccpBridgeKey(bytes32 peerKeyHash,address bridgeAddress,uint64 activationEpoch)` | `0x87f74eabbf9693141811f707b2d51c6a18546376727b384e155d2539ed0a1a3a` |
| Governance approval | `SccpGovernance(bytes32 actionHash,bytes32 rosterDigest)` | `0x662400c297c594d721741bfe936e957e1f187ebbcf0b8ca37aefd3bc730376f4` |
| Destination mint control | `SccpMintControl(bytes32 destination,bool paused,uint64 controlNonce)` | `0xddac541688dedbb75aa1f989354fff8f806ce41c1f47bbf5ebb0b239453fb9b3` |

#### 3.6.1 Attestation statement fields

| Field | Meaning |
|---|---|
| `height` | Taira block height `h` |
| `epoch` | NPoS epoch of `h` (`HeightContext(h).epoch`) |
| `timestampMs` | `creation_time_ms` of block `h`'s header |
| `blockHash` | `HashOf<BlockHeader>` of block `h` (32 bytes) |
| `sccpRoot` | root of block `h`'s commitment tree, or zero if `messageCount = 0` |
| `messageCount` | number of outbound messages committed by `h` (0..=512) |
| `historyRoot` | `history_root(historySize)` |
| `historySize` | number of SCCP-bearing blocks with height ≤ `h` (includes `h` if `messageCount > 0`) |
| `rosterDigest` | digest (§3.7) of the roster generation that signs `h` |
| `nextRosterDigest` | digest of the successor generation if `h` is a rotation boundary (§4.3), else zero |

Invariants checked by every verifier: `messageCount = 0 ⇔ sccpRoot = 0`;
`historySize = 0 ⇔ historyRoot = 0`; `messageCount ≤ 512`.

#### 3.6.2 Key PoP fields

`peerKeyHash = keccak256(raw public key bytes of the peer's consensus key)`
(the 48-byte compressed BLS-normal G1 key); `bridgeAddress` is the 20-byte
address of the bridge key; `activationEpoch` as registered.

#### 3.6.3 Governance fields

`actionHash = keccak256(ASCII("SCCP/GOVERNANCE/V1") ‖ norito_bare_encode(SccpGovernanceActionV1))`
(Taira-internal; only Taira verifies it). `rosterDigest` is the current
generation's digest at approval time, so approvals die with the generation.

#### 3.6.4 Mint-control fields

`destination` is the destination word (§3.4) of the target contract; `paused`
the requested state; `controlNonce` the contract's current control nonce.

### 3.7 Roster digest

```
roster_preimage =
  ASCII("SCCP/ROSTER/V1")        // 14 bytes
  ‖ taira_network_id            // 32
  ‖ u64 generation              // ≥ 1
  ‖ u64 valid_from_ms
  ‖ u64 valid_until_ms          // > valid_from_ms
  ‖ u8  n                       // 4..=31
  ‖ u8  t                       // MUST equal ⌊2n/3⌋ + 1
  ‖ member_0 ‖ … ‖ member_{n-1} // 20 bytes each
roster_digest = keccak256(roster_preimage)
```

Member ordering: all zero members (roster peers without an active bridge key)
first, then nonzero members **strictly ascending** as 160-bit unsigned
integers. Every verifier MUST check `n` range, the `t` formula and the ordering
while hashing. Because members are unique and signers are addressed by
position, a single key can never count twice.

### 3.8 Signatures

- A signature is 65 bytes `r ‖ s ‖ v` with `v ∈ {27, 28}`, `1 ≤ r < N`,
  `1 ≤ s ≤ HALF_N` (low-S). Recovery that yields the zero address is invalid.
- Signers use RFC 6979 deterministic nonces over the 32-byte digest (prehash).
  `iroha_crypto::EcdsaSecp256k1Sha256::sign_prehash_recoverable` already
  produces this form (k256 normalizes to low-S and adjusts the recovery id); if
  the recovery id is ≥ 2 the signer MUST re-sign with a fresh extra-entropy
  input (probability ≈ 2^−127).
- A signature set is `(signer_bitmap: u32, signatures)`: bit `i` refers to
  roster member `i`; `signatures` is the concatenation of one 65-byte signature
  per set bit, in ascending bit order; bits ≥ `n` MUST be zero; each set bit's
  member MUST be nonzero and equal the recovered address.
- The address of a key is `keccak256(X ‖ Y)[12..32]` of the uncompressed point.
  It is identical on ETH, BSC, TRON (without the `0x41` prefix) and TON.

### 3.9 Constant summary

| Name | Value |
|---|---|
| Event `SccpTransferToTaira(bytes32,address,uint64,bytes)` topic0 | `0x79ac1cc63262b80bfbf96e55b2d49d96bd82f018ce0192f7002168724c7d264b` |
| Event `SccpFinalized(bytes32,uint64,address,uint256)` topic0 | `0x8ab0eb669cb9abcdf0e37e9ce8443162d317d10c2b059296e40aebbd3fa23748` |
| Event `SccpRosterRotated(uint64,bytes32,uint64)` topic0 | `0x8619e96c15d0af080499408d3d5fb9af0101014daa9acaad772a5aa03ebf315e` |
| Event `SccpMintingPaused(bool,uint64)` topic0 | `0xccb1716935ea7bed42832e37ff26651aae8ec66ce7312f71fa4cebe6ed26c286` |
| `PREVIOUS_ROSTER_GRACE_MS` (contracts) | 86 400 000 (24 h) |
| Max block leaves / block path | 512 / 9 |
| Max history path | 32 |

Implementations MUST pin all constants in golden tests (§11).

## 4. Taira side

### 4.1 On-chain parameters

SCCP consensus parameters are one on-chain value `SccpParametersV1`
(Taira-internal), set in genesis and changed only by the governance action
`SetParameters` (§4.14). `SetParameter` on this value from any other path is
rejected by the executor.

| Field | Taira genesis value | Rule |
|---|---|---|
| `enabled` | `true` | When false, every SCCP instruction fails and no subjects are produced |
| `roster_max_age_ms` | 604 800 000 (7 d) | Heartbeat: force a new generation at the first epoch boundary at or after `valid_from + roster_max_age` |
| `roster_validity_ms` | 2 419 200 000 (28 d) | `valid_until = valid_from + roster_validity`; MUST be ≥ `2 × roster_max_age + 1 d` |
| `attestation_retention_blocks` | 648 000 (≈30 d at 4 s) | Signature pruning horizon (§4.10) |
| `max_attestation_entries_per_instruction` | 256 | Bound for `SubmitSccpAttestationsV1` |
| `governance_approval_ttl_blocks` | 64 800 (≈3 d) | Approval expiry |

Node-local behavior (key file path, submission cadence) lives in
`iroha_config` `[sccp.attestor]` (§4.9). Existing `[zk.sccp]` native verifier
work limits stay in `iroha_config` and remain bound into the consensus policy
hash; their pending-outbound, pairing and BLS-aggregate knobs are removed.

The SCCP gate is "`SccpParametersV1.enabled` in state"; the compiled
chain-id gate (`sccp_local_sora_network_for_chain_id`) is removed.

### 4.2 Bridge keys

#### 4.2.1 State

```
sccp_bridge_keys:       PeerId → SccpBridgeKeyStateV1 {
    active:  Option<SccpBridgeKeyV1>,
    pending: Option<SccpBridgeKeyV1>,          // activates at pending.activation_epoch
    retired: Vec<SccpBridgeKeyV1>,             // tombstones, bounded to 16, oldest dropped from the vec only
}
SccpBridgeKeyV1 { public_key: [u8; 33] /* compressed secp256k1 */, address: [u8; 20],
                  activation_epoch: u64, registered_at_height: u64, faulted: bool }
sccp_bridge_key_owners: [u8; 20] → PeerId       // permanent; an address is never reusable
```

#### 4.2.2 `SetSccpBridgeKeyV1`

```
SetSccpBridgeKeyV1 {
    peer: PeerId,
    public_key: Option<[u8; 33]>,       // None = revoke from activation_epoch
    activation_epoch: u64,
    peer_signature: SignatureOf<SccpBridgeKeyBindingV1>,   // by the peer's consensus key
    key_pop: Option<[u8; 65]>,          // required iff public_key is Some
}
SccpBridgeKeyBindingV1 {
    domain: "iroha.sccp.bridge_key.v1", network_id: NetworkId, peer: PeerId,
    public_key: Option<[u8; 33]>, activation_epoch: u64,
}
```

Validation (all MUST hold):

1. SCCP enabled; `peer` is a registered peer.
2. `peer_signature` verifies under the peer's consensus public key over the
   Norito encoding of the binding (the `PublicLaneCandidateAuthorization`
   pattern).
3. If `public_key` is present: it decompresses to a valid point; `address`
   derived per §3.8; `key_pop` is a valid §3.8 signature over the EIP-712
   `SccpBridgeKey{peerKeyHash, address, activation_epoch}` digest recovering
   `address`; `address ∉ sccp_bridge_key_owners` (never reused, including by the
   same peer).
4. `activation_epoch ≥ current_epoch + 1`, except inside the genesis block where
   `activation_epoch = 0` is REQUIRED.
5. At most one pending change per peer; a new instruction replaces an existing
   pending entry (its address is still burned in `sccp_bridge_key_owners`).

Effect: store as `pending`; insert the owner index. At the start of the first
block of `activation_epoch` (or immediately in genesis), `pending` becomes
`active` and the previous `active` moves to `retired`. Revocation clears
`active` at activation. The authority is any account (the two signatures
authorize); ordinary fees apply. Event `SccpBridgeKeySet`.

Genesis: the Taira reset tooling generates one bridge key per validator as a
runtime secret and places `SetSccpBridgeKeyV1` (epoch 0) for every genesis
validator in the genesis transactions.

### 4.3 Bridge roster generations

#### 4.3.1 State

```
sccp_rosters: u64 generation → SccpBridgeRosterV1 {
    generation, valid_from_ms, valid_until_ms,
    activation_height: u64,               // first height signed by this generation
    members: Vec<SccpRosterMemberV1 { address: [u8; 20], peer: Option<PeerId> }>,  // §3.7 order
    threshold: u8, digest: [u8; 32],
}
sccp_roster_current: u64
```

#### 4.3.2 Derivation

Let `peers(E)` be the consensus roster of epoch `E` (`HeightContext.roster`,
`n = 3f+1`, 4..=31). `members(E)` maps each peer to the address of its bridge
key that is active in epoch `E` and not `faulted`, or the zero address, then
orders per §3.7.

- **Generation 1** is created in the post-execution hook of the genesis block
  from `peers(0)` and the genesis keys: `valid_from_ms` = genesis block
  `creation_time_ms`, `activation_height = 1`.
- **Boundary rule.** In the post-execution hook of each epoch-boundary block
  `h_b` (`h_b = epoch_end_height` of epoch `E`), compute `members(E+1)` from
  `HeightContext(h_b).next_epoch_snapshot.roster` (a deterministic function of
  the pre-state of `h_b`; implementations read it from the same source as
  `threshold_key_lifecycle_successor_roster_v1`) and the keys that will be
  active in `E+1`. Let `g` be the current generation. A new generation `g+1` is
  created iff
  - the ordered member list differs from `g`'s, or
  - `creation_time_ms(h_b) − g.valid_from_ms ≥ roster_max_age_ms`, or
  - a fault (§4.11) evicted a member of `g`.

  `g+1` gets `valid_from_ms = creation_time_ms(h_b)`,
  `valid_until_ms = valid_from_ms + roster_validity_ms`,
  `activation_height = h_b + 1`, `threshold = ⌊2n/3⌋+1`.
- The generation that signs height `h` is the generation with the greatest
  `activation_height ≤ h`. The boundary block `h_b` itself is signed by `g`
  and carries `nextRosterDigest = digest(g+1)`.

A zero-address slot counts in `n` but can never sign; if more than `n − t`
slots are unusable no attestation reaches threshold (liveness failure, not a
safety failure).

### 4.4 Outbound: `RecordSccpMessage`

```
RecordSccpMessage {
    network: SccpNetworkV1,     // external target
    amount: Numeric,            // Taira XOR, scale 9, > 0
    recipient: Vec<u8>,         // bytes for the target's recipient codec (§3.1)
}
```

This is a plain signed instruction (no `IvmProved` executable, no contract, no
replay witness). The authority is the sender. Execution:

1. SCCP enabled. The route for `network` has exactly one revision `r` in state
   `Bidirectional` (§4.14.2).
2. `recipient` is valid for the target codec (§3.1); for EVM/TRON it MUST NOT
   equal the destination contract address.
3. `liability(r) + amount ≤ max_wrapped_supply(r)` (Taira units).
4. The block has fewer than 512 recorded messages.
5. Transfer `amount` XOR from the authority to the route-revision escrow
   account (`sccp_route_escrow_account_id_v1(network_id, route_key, xor)`,
   non-signable); `liability(r) += amount`.
6. `nonce = next_outbound_nonce(r)`; `next_outbound_nonce(r) += 1`. Nonces are
   dense per revision starting at 0.
7. Build the payload (§3.2) with `source_domain 0`, `dest_domain` of `network`,
   `sender_codec 3` = `AccountAddress` bytes of the authority, and compute
   `payload_hash`, `message_id`, `leaf` (with the revision's destination word).
8. Allocate `commitment_index` = number of messages already recorded in this
   block (the existing `next_sccp_outbound_commitment_index` allocator; a failed
   transaction releases its indices by state rollback).
9. Insert `sccp_outbound_messages[message_id] = SccpOutboundMessageRecordV1
   { network, revision, nonce, height, commitment_index, payload, leaf }` and
   `sccp_outbound_index[(height, commitment_index)] = message_id`.
10. Emit `SccpMessageRecorded { message_id, network, revision, nonce, height, commitment_index }`.

Ordinary fees apply. There are no pending caps, no pending-until-delivery state
and no Taira-side destination receipt: Taira cannot observe delivery, and the
destination's consumed set is authoritative (§5).

Contract-originated SCCP sends (Kotodama) are not part of v1; the syscall
`0xA0` operation tag 2 and the `ledger::sccp::record` builtin are removed
(§10). The post-execution root (§4.5) would support them safely later.

### 4.5 Block commitment and history (post-execution)

`BlockHeader::sccp_commitment_root`, `CandidateAttachments.sccp_commitment_root`,
`SccpRootValidation` and `validate_sccp_commitment_root_for_signed_block` are
deleted (a block-header wire change; the header fixtures are regenerated).

In `ValidBlock::finalize_owned_execution_metadata`
(`crates/iroha_core/src/block/post_execution_tail.rs`), after all transactions
of block `h` executed and before the output seal:

1. Read the applied outbox `sccp_outbound_index[(h, 0..m)]`. If `m > 0`,
   compute `root` per §3.4 over their leaves, append
   `history_leaf(h, root, m)` to the accumulator (§3.5), and write
   `sccp_block_commitments[h] = { root, message_count: m, history_index }`.
2. If `h` is an epoch boundary, apply the roster rule (§4.3.2).
3. If `m > 0` or `h` is an epoch boundary, write the attestation subject
   (§4.6).

These writes are part of the post-state covered by the Commit QC's
`ExecutionCommitment` (`ordinary_writes_root`, `post_state_root`), so the root
is QC-authenticated with no vote-wire change. The proposal-time collectors and
the callback-record fail-closed rule become dead code and are removed.

### 4.6 Attestation subjects

```
sccp_attestation_subjects: u64 height → SccpAttestationSubjectV1 {
    height, epoch, timestamp_ms,
    sccp_root: [u8; 32], message_count: u32,
    history_root: [u8; 32], history_size: u64,
    generation: u64, roster_digest: [u8; 32], next_roster_digest: [u8; 32],
}
sccp_attestation_status: u64 height → SccpAttestationStatusV1 {
    signer_bitmap: u32, attested_at_height: Option<u64>,
}
```

The statement of height `h` is the subject plus `blockHash = block_hashes[h]`
(known once `h` is committed; attestation instructions necessarily execute at a
later height). `statement_digest(h)` is the §3.6 attestation digest under the
live `NetworkId`. Event `SccpSubjectCreated { height, generation, message_count, rotation }`.

### 4.7 Attestation transport decision

Chosen: signatures travel as a self-authenticating instruction in ordinary
transactions (§4.8), submitted by each validator node's attestor (§4.9).

| Option | Durable & state-derived | Consensus coupling | Verdict |
|---|---|---|---|
| Instruction in later blocks | yes: stored in world state, identical on every peer | none (ordinary tx path) | **chosen**; latency +1–2 blocks (4–8 s) |
| P2P gossip (new `ConsensusMessageV2`) | no, unless also embedded by the proposer, which reintroduces proposer-authored SCCP data | new message kind + reducer ingress | rejected |
| Sumeragi Commit-vote extension (KAGEMUSHA seal pattern) | yes (in the CommitQC envelope) | changes `HeightContext`, vote and QC wire in the Sumeragi v2 redesign; a validator without a bridge key would stall SCCP blocks | deferred optimization (§12); the digest is unchanged by it |

### 4.8 `SubmitSccpAttestationsV1`

```
SubmitSccpAttestationsV1 {
    entries: Vec<SccpAttestationSignatureV1 { height: u64, signer_index: u8, signature: [u8; 65] }>,
}
```

Validation, per entry (the instruction fails if any entry is invalid):

1. `1 ≤ len(entries) ≤ max_attestation_entries_per_instruction`; entries are
   sorted by `(height, signer_index)` without duplicates.
2. `height < current height` and `sccp_attestation_subjects[height]` exists.
3. `signer_index < n` of the subject's generation; the member at that index is
   nonzero.
4. The signature satisfies §3.8 and recovers that member's address from
   `statement_digest(height)`.

Execution, per entry: if bit `signer_index` of the status bitmap is set, the
entry is a no-op (another validator's transaction already carried it; ECDSA
signatures are not unique, and only the first valid one is stored). Otherwise
store `sccp_attestation_signatures[(height, signer_index)] = signature`, set
the bit and emit `SccpAttestationSigned`. When the bitmap first reaches
`popcount ≥ threshold`, set `attested_at_height` and emit
`SccpBlockAttested { height, generation, signer_bitmap }`. Signatures beyond
threshold are still accepted and stored (up to `n`).

Only canonical signatures can ever be stored because Taira recomputes the
digest from its own state.

**Authority and fees.** The authority is any account; the signatures authorize.
`SubmitSccpAttestationsV1` joins `nexus_protocol_fee_exempt_instruction`
(`crates/iroha_core/src/executor.rs`); a transaction is fee-exempt only if all
its instructions are exempt. Because fee-exempt roots have no chargeable record,
transaction admission (Torii ingress and P2P transaction gossip admission, via
the `ordinary_transaction_ingress` pattern) MUST run checks 1–4 and reject the
transaction if any entry is invalid or if every entry is already recorded. The
transaction is routed to the universal dataspace. The instruction is added to
the wire-id registry, the Initial-executor admitted list, and the
validation-fee classification as "no DS effect".

### 4.9 Node attestor

A new irohad component, modeled on the Soracloud runtime mutation sink:

- On each locally committed block (Commit QC present in Kura), for every height
  with a subject whose generation contains the node's bridge address and whose
  bit is not yet set, compute the statement from **local committed state and
  Kura** and sign it (§3.8). It never signs uncommitted heights and never signs
  anything other than `SccpAttestation` digests derived this way.
- Batch all pending entries into one `SubmitSccpAttestationsV1` transaction per
  block (sorted, ≤ the parameter bound), signed by the node's configured
  runtime transaction signer, pushed through `AcceptedTransaction::accept` into
  the local queue; resubmit entries still unrecorded after
  `resubmit_after_blocks`.
- At startup, verify that the configured key's address equals the peer's
  `active` (or `pending`) bridge key and refuse to sign otherwise.
- Outgoing validators keep signing the heights their generation covers,
  including the boundary block that hands off to the successor.

Configuration (user → actual → defaults, file-only, no environment variables):

```toml
[sccp.attestor]
enabled = true                      # default true; inert if the node is not a roster member
bridge_private_key_file = "…"       # owner-only 0600/0400 file, no symlinks; runtime secret
max_entries_per_transaction = 64    # ≤ on-chain bound
resubmit_after_blocks = 3
```

The Taira launcher passes the bridge key like the other runtime secrets (a new
inherited descriptor next to the mint-finality and beacon credentials).

### 4.10 Storage and pruning

| Map | Retention |
|---|---|
| `sccp_outbound_messages`, `sccp_outbound_index` | permanent (one record per value transfer, ≤ 4 KiB payload) |
| `sccp_block_commitments`, history leaves and peaks | permanent (~48 B per SCCP block) |
| `sccp_attestation_subjects`, `sccp_attestation_status` | permanent (~250 B per SCCP block or epoch boundary) |
| `sccp_attestation_signatures` | pruned at the first block where `height < current − attestation_retention_blocks`, **except** subjects with `next_roster_digest ≠ 0` (rotations), which are kept until the outgoing generation's `valid_until_ms` is older than the current block time |
| `sccp_rosters`, `sccp_bridge_key_owners` | permanent |
| `sccp_inbound_consumed` | permanent (32 B key per message) |
| light-client sets / checkpoints | per-network retention (§4.13.1) |
| governance approvals | deleted on execution, expiry, or generation change |

Pruning runs in the post-execution hook with a per-block work bound of 1 024
deletions. Pruned signatures stay in Kura block history. Old messages remain
finalizable through the history root of any retained attestation (§3.5).

### 4.11 Equivocation evidence and slashing

```
SubmitSccpAttestationFaultV1 {
    statement: SccpAttestationStatementV1,   // all 10 §3.6.1 fields
    signature: [u8; 65],
}
```

Validation: the signature satisfies §3.8 over the EIP-712 digest of `statement`
under Taira's live domain; the recovered address is in `sccp_bridge_key_owners`;
and the statement is **faulty**, meaning one of:

- `statement.height > current height − 1` (honest attestors sign only
  committed heights);
- no subject exists at `statement.height` (the height is committed);
- the subject exists and any field of `statement` differs from the canonical
  statement (subject plus `block_hashes[height]`).

Execution: record `sccp_attestation_faults[(address, height)] =
{ peer, statement_hash, reported_at_height }` (duplicates are no-ops), mark
the key `faulted` (it is excluded from every later generation, which triggers a
new generation at the next boundary, §4.3.2), and emit `SccpAttestationFault`.

Fees and admission: fee-exempt with the same admission pre-verification as
§4.8. Anyone may submit, including evidence copied from destination-chain
calldata.

**Phase 2 (TODO):** apply the existing indexed consensus penalty
(`apply_indexed_consensus_slash_to_validator`, after `slashing_delay_blocks`)
to the validator bonded under the faulty peer, cancellable by the standard
`CancelConsensusEvidencePenalty`. Slashing is only meaningful when the Taira
unbonding delay exceeds `roster_validity_ms + evidence_horizon` (§9.8); on
Taira today it is 0.

### 4.12 Inbound: `SubmitSccpInboundMessageV1`

```
SubmitSccpInboundMessageV1 {
    network: SccpNetworkV1,          // external source
    revision: u32,
    payload: Vec<u8>,                // §3.2
    proof: SccpSourceProofV1,        // per-chain enum, §4.13
}
```

#### 4.12.1 Common validation

1. SCCP enabled; revision `r` of the route exists in state `Bidirectional` or
   `InboundOnly`.
2. The payload decodes (§3.2) with `source_domain` = the network's domain,
   `dest_domain = 0`, `route_revision = r`, `route_id` of the route.
3. `message_id` recomputed (§3.3) is not in `sccp_inbound_consumed`.
4. `proof` verifies against the network's light client (§4.13) and yields a
   normalized source event `{ emitter, message_id, sender, nonce,
   payload_hash }` whose `emitter` equals revision `r`'s deployment address
   and whose fields equal those recomputed from `payload`.
5. Verifier work is reserved through the existing `SccpVerifierWorkV1`
   metering and `[zk.sccp]` limits; at most one inbound message per
   transaction, preceded by any number of `AdvanceSccpLightClientV1`
   instructions within the work limits.

#### 4.12.2 Source event binding per chain

| Source | Evidence of the burn | Normalized fields come from |
|---|---|---|
| ETH, BSC | receipt log with `status = 1`, `address` = deployment, `topics = [0x79ac1cc6…, message_id, word(sender)]`, `data = abi.encode(uint64 nonce, bytes payload)` | log topics and data |
| TRON | the `TriggerSmartContract` transaction with `contractRet = SUCCESS`, `contract_address` = deployment (21 B), `call_value = 0`, `call_token_value = 0`, data = selector `0xebfc6ca8` ‖ `abi.encode(bytes recipient, uint256 tokenAmount, uint64 expectedNonce)` (TRON logs are not header-committed; the existing transaction-inclusion rule over the full `Transaction` protobuf is reused) | `sender = owner_address`; `nonce = expectedNonce`; `amount = tokenAmount / 10^9` (MUST divide exactly); `recipient`; the remaining fields are route constants — Taira rebuilds the payload and requires byte equality |
| TON | a transaction of the deployment (Jetton master) account with compute and action phases successful and an external-out message whose body is `sccp_transfer_to_taira` (§5.3.4) | out-message body |

#### 4.12.3 Settlement

1. Decode `recipient` (codec 3) as `AccountAddress` and derive the `AccountId`.
   If decoding fails, go to §4.12.5 (bounce).
2. The account MUST exist; otherwise the instruction fails with
   `SccpRecipientAccountMissing` and the message stays unconsumed (the recipient
   registers and anyone resubmits). Wallets check existence before burning.
3. `liability(r) ≥ amount`, else fail (it would mean the external side released
   more than Taira locked on this revision).
4. Transfer `amount` XOR from the revision escrow to the recipient;
   `liability(r) −= amount`.
5. Insert `sccp_inbound_consumed[message_id] = { network, revision, height,
   source_locator, outcome: Released }`; emit `SccpInboundReleased`.

#### 4.12.4 Authority and fees

Permissionless: any account may submit and pays the ordinary fee (a
`FeePaymentIntent::Sponsor` lets a fresh recipient claim). The recipient is
fixed by the payload, so relaying cannot redirect funds.

#### 4.12.5 Bounce

If the recipient bytes are not a canonical `AccountAddress`, Taira returns the
value to the source-chain sender on the same revision: it consumes the inbound
message with `outcome: Bounced(bounce_message_id)`, leaves the escrow untouched
and records an outbound message exactly as §4.4 steps 6–10 with
`amount` = the inbound amount, `sender` = the escrow account's `AccountAddress`,
`recipient` = the inbound `sender`, skipping the activation (`InboundOnly` is
allowed) and cap checks; `liability(r)` is decremented and incremented by the
same amount (net zero). Emit `SccpInboundBounced`.

### 4.13 Inbound light clients

Anchors become permissionless light-client state. World state stores
**authenticated validator sets** of each source chain with validity ranges, plus
a bounded FIFO of finalized checkpoints; each inbound proof carries its own
finality evidence verified against a stored set. There are no per-transfer
anchor windows, no anchor preimages and no Parliament-advanced anchors.

#### 4.13.1 State

```
sccp_light_clients: SccpNetworkV1 → SccpLightClientV1 {
    params: SccpLightClientParamsV1,   // chain profile, thresholds, ws_bound_ms, retention, per-ISI bounds
    head: { latest_set_id: u64, latest_finalized: SccpLcPointV1, last_progress_taira_ms: u64 },
    frozen: Option<SccpLcFreezeReasonV1>,
    state_hash: [u8; 32],              // keccak of canonical state bytes; CAS handle for wallets
}
sccp_light_client_sets:        (SccpNetworkV1, u64 set_id) → SccpLcConsensusSetV1
sccp_light_client_checkpoints: (SccpNetworkV1, u64 source_height) → SccpLcCheckpointV1 {
    block_hash, state_root: Option, receipts_or_tx_root, source_time, recorded_at_taira_ms }
```

Sets are retained for `params.retention` (default 180 days); checkpoints are a
FIFO of 16 384 per network, written idempotently as a side effect of every
accepted advance or inbound proof and by explicit backfill.

#### 4.13.2 Instructions

| Instruction | Permission | Effect |
|---|---|---|
| `InitializeSccpLightClient` | governance action only (§4.14.3) | installs params and a bootstrap set (weak-subjectivity checkpoint); also used after a freeze and after every Taira reset |
| `AdvanceSccpLightClientV1 { network, expected_state_hash: Option<[u8;32]>, advance: SccpLcAdvanceV1 }` | anyone; ordinary fee | stores the next set(s)/checkpoints; idempotent (re-proving stored data is `Ok` with no change) so concurrent wallets do not fail |
| `ReportSccpLightClientEquivocationV1 { network, a, b }` | anyone; ordinary fee | two quorum-valid conflicting records (same period/epoch/key block/height, different content) set `frozen`; an advance conflicting with stored data is rejected pointing to this instruction |

Weak subjectivity: an advance or inbound proof is accepted only if the source
time of its signing set is within `params.ws_bound_ms` of the Taira block time.
Old events are proven by ancestry from a fresh finality point or from a
checkpoint recorded while fresh, never with stale keys. A client that falls
outside the bound cannot catch up and needs re-initialization.

#### 4.13.3 Per chain

| Chain | Stored set | Advance | Inbound proof | `ws_bound_ms` |
|---|---|---|---|---|
| Ethereum | sync committee per period (512 pubkeys + aggregate); fork schedule compiled into the profile | ≤16 full `LightClientUpdate`s with finalized period = attested period, ≥342 participants, finality and next-committee branches, fast-aggregate BLS | finality-only update verified against the committee of `period(signature_slot)`; ancestry `SameBlock` \| `HistoryContract` (EIP-2935 account + storage MPT under the finalized state root, `1 ≤ E−B ≤ 8191`) \| `StoredCheckpoint`; event header RLP (`keccak = hash`, fields read by index); receipt MPT | 14 d |
| BSC | Parlia roster per 1000-block epoch checkpoint (address, BLS key) and turn length | ≤64 epoch hops, each a checkpoint header with a fast-finality vote verified by the previous roster (`(2n+2)/3` BLS quorum) | event header + vote `(B, B+1)` (or via ≤256 descendant headers) under the roster covering `B−1` + receipt MPT | 5 d |
| TRON | witness map `account21 → {signer21, last_endorsed}` (≤64) and threshold 19 (⌈0.7·27⌉) | parent-linked header segment (≤128); a header is solid once ≥19 distinct known witnesses produced headers at or after it; solid headers teach new/rotated keys; inactive witnesses pruned after 14 400 blocks | segment in which the event block becomes solid + transaction MPT under `txTrieRoot`; older events via unsigned `raw_data` headers linked to a stored checkpoint | 2 d |
| TON | validator epoch per key block (config 34, 28, 15) | ≤16 key-block hops, each signed by the current subset, with `prev_key_block_seqno` = stored latest key block | any masterchain block signed by the epoch named by its `prev_key_block_seqno` (or `OldMcBlocksInfo` back-link from a fresh block) + shard registration + ≤32 shard-block `prev_ref` walk (both predecessors after merge, `before_split` allowed) + account transaction and out-message | `utime_until + stake_held_for − margin` |

The existing per-chain verifiers in `iroha_sccp` (`ethereum_native.rs`,
`ethereum_source.rs`, `bsc_native.rs`, `tron_native.rs`, `ton_native.rs`) are
kept and refactored to these stateless checks. Removed: `SccpNativeTrustAnchorV1`,
anchor preimage DTOs, the anchor interval cutoff, `sccp_inbound_anchor_high_water`,
BSC full Parlia replay (seal, recents, difficulty, Go `math/rand` backoff),
TRON schedule replay and 27-header window, TON 64-block window and
signature-per-block walk, and every replay-witness field.

Deployment code identity is verified off-chain by approving validators
(§4.14.4); inbound proofs no longer carry per-transfer account proofs.

### 4.14 Route registry and governance

#### 4.14.1 Registry state

```
SccpRouteRevisionV1 {
    key: SccpRouteKeyV1 { lane (external↔Taira), route_id, asset_key: "xor", revision },
    deployment: SccpDeploymentV1,
    activation: SccpRouteActivationV1,
    max_wrapped_supply: u128,          // Taira units; MUST equal the contract cap / units factor
    liability: u128,
    next_outbound_nonce: u64,
    registered_at_height: u64,
}
SccpDeploymentV1 =
  | Evm  { address: [u8; 20], runtime_code_hash: [u8; 32] }      // ETH, BSC
  | Tron { address: [u8; 21], runtime_code_hash: [u8; 32] }      // 0x41-prefixed
  | Ton  { master_account: [u8; 32], code_hash: [u8; 32],
           wallet_code_hash: [u8; 32], bucket_code_hash: [u8; 32] }
```

The Groth16 verifying keys, semantic profiles, finality anchors, outbound proof
policy, IVM execution policy, TON guardian keys and schema hashes are removed
from the registry.

#### 4.14.2 Activation states

`Staged` (nothing flows) → `Bidirectional` (outbound + inbound) ⇄ `Paused`
(nothing flows) → `InboundOnly` (inbound only; the old deployment can still
burn back) → `Retired` (terminal, nothing flows). At most one `Bidirectional`
revision per route. `SwitchRevision` atomically moves the old revision to
`InboundOnly` and a `Staged` successor to `Bidirectional`.

#### 4.14.3 Governance actions and approval

```
SccpGovernanceActionV1 =
  | RegisterRoute     { network, revision, deployment, max_wrapped_supply }   // → Staged; revision = latest + 1
  | SetActivation     { network, revision, expected: SccpRouteActivationV1, next }
  | SwitchRevision    { network, from, to }
  | RemoveStaged      { network, revision }   // never used, liability 0
  | InitializeLightClient { network, expected_state_hash: Option<[u8;32]>, params, bootstrap }
  | FreezeLightClient { network }
  | SetParameters     { expected: SccpParametersV1, next: SccpParametersV1 }

ApproveSccpGovernanceV1 {
    action: SccpGovernanceActionV1,
    approvals: Vec<{ signer_index: u8, signature: [u8; 65] }>,
}
```

Each approval is a §3.8 signature over the EIP-712
`SccpGovernance{actionHash, rosterDigest}` digest by the member at
`signer_index` of the **current** generation. Approvals accumulate in
`sccp_governance_approvals[action_hash] = { roster_digest, signer_bitmap,
first_height }`; the record is discarded if the generation changed or
`first_height + governance_approval_ttl_blocks` passed. The action executes in
the instruction that brings the count to its threshold:

- `f+1 = n − t + 1` for `SetActivation{next: Paused}` and `FreezeLightClient`;
- `t` for everything else.

Execution reuses the existing registry executor (`apply_sccp_route_governance_action`:
compare-and-swap on `expected`, revision monotonicity, resource checks); the
whole-registry Parliament head serialization is dropped. Anyone may submit
approvals (ordinary fee). Validators produce approvals with
`iroha sccp governance sign` using their bridge key file (never automatically).

Removed: `ProposeSccpRouteGovernance`, `ProposalKind::SccpRouteGovernance`,
the Parliament SCCP body list, `ApplySccpRouteGovernance`,
`CanManageSccpGovernance`, `CanProposeSccpRouteGovernance`, the Torii
governance draft route, and the `InitializeTrustAnchor`/`AdvanceTrustAnchor`
actions.

#### 4.14.4 Deployment verification (validator diligence, no server)

Before signing `RegisterRoute`, each validator runs locally
`iroha sccp deployment verify --network <p> --address <a> --rpc <public RPC…>`,
which MUST confirm:

- EVM/TRON: `eth_getCode` / `getcontract` runtime bytes equal the locked
  artifact's runtime bytes with the immutable references (from the compiler's
  `immutableReferences`) filled with the claimed constructor values; views
  return `tairaNetworkId()` = live `NetworkId`, `routeRevision()` = revision,
  `maxWrappedSupply()` = cap × units factor, `rosterState()` digest = a Taira
  generation that is current or newer on the destination, `totalSupply() = 0`.
- TON: the account's code hash equals the locked minter code hash; the data
  decodes to the claimed config; `get_sccp_state` agrees.

Anyone can deploy a destination contract; only the registered deployment
matters.

### 4.15 Liability and escrow invariants

For every route revision `r`:

- `balance(escrow(r)) = liability(r)` after every transaction (tests assert it).
- `liability(r) ≤ max_wrapped_supply(r)` at every `RecordSccpMessage`
  (bounces are net zero).
- On the destination, `totalSupply + pending ≤ liability(r) × units_factor`
  holds under the honest-majority assumption, because every mintable message was
  counted into liability when recorded and burns decrease supply before Taira
  releases. The destination cap (§5.1.4) is defense in depth and never binds
  under honest operation.

### 4.16 Events

`SccpEvent` (Taira data events):
`MessageRecorded`, `BlockCommitted { height, root, count, history_size }`,
`SubjectCreated`, `AttestationSigned`, `BlockAttested`,
`RosterGenerationCreated { generation, digest, activation_height, valid_until_ms }`,
`BridgeKeySet`, `AttestationFault`, `InboundReleased`, `InboundBounced`,
`LightClientAdvanced`, `LightClientFrozen`, `LightClientInitialized`,
`GovernanceApproved { action_hash, signer_index, count }`,
`GovernanceExecuted { action_hash }`.

### 4.17 Taira resets and devnets

- The Taira identity is `NetworkId` read from state; SCCP hashes, the EIP-712
  salt and escrow account derivation all use it. Nothing about Taira is
  compiled into Rust or contracts; `scripts/taira_devnet.py` devnets therefore
  work with the same binaries.
- A reset produces a new `NetworkId`: every existing destination deployment is
  bound to the old identity and becomes permanently unusable for minting and
  for redemption (the new Taira does not accept its burns). Wrapped supply on
  old deployments is stranded; this is accepted for a test network and MUST be
  stated in token metadata and wallet UI ("Taira test XOR; redeemable only on
  Taira network `<NetworkId>`").
- After a reset: genesis carries the validators' new bridge keys; generation 1
  is published by any Torii; anyone deploys fresh destination contracts with
  that roster; validators approve `RegisterRoute` and
  `InitializeLightClient` actions; routes activate.
- Wallets pin deployments per `NetworkId` in `[sccp]` client config and refuse
  a destination whose `tairaNetworkId()` differs from Torii's capabilities.

### 4.18 Permissions and fees

| Instruction | Authority | Fee |
|---|---|---|
| `SetSccpBridgeKeyV1` | any (consent + PoP signatures authorize) | ordinary |
| `SubmitSccpAttestationsV1` | any (signatures authorize) | exempt, admission-verified |
| `SubmitSccpAttestationFaultV1` | any | exempt, admission-verified |
| `RecordSccpMessage` | the sender | ordinary |
| `SubmitSccpInboundMessageV1` | any | ordinary (sponsor allowed) |
| `AdvanceSccpLightClientV1` | any | ordinary |
| `ReportSccpLightClientEquivocationV1` | any | ordinary |
| `ApproveSccpGovernanceV1` | any (bridge-key approvals authorize) | ordinary |

No SCCP permission token exists. The default executor gets allow-visitors for
these instructions; core enforces all SCCP rules.

## 5. Destination contracts

### 5.1 Common semantics (all four chains)

#### 5.1.1 State

- Immutable: `tairaNetworkId`, network tag/domain/identity word, `routeRevision`,
  `maxWrappedSupply` (token units), `DOMAIN_SEPARATOR`, route id.
- Current roster: `digest`, `generation`, `validUntilMs` (TON also stores
  members, `n`, `t`).
- Previous roster: `prevDigest`, `prevValidUntilMs` (TON also members).
- Consumed set over outbound nonces; outbound source nonces; token supply;
  `mintingPaused`, `mintControlNonce`.

#### 5.1.2 Roster acceptance

`now_ms` is the destination block time in milliseconds. An attestation with
`rosterDigest = D` is signed by an **accepted** roster iff
`D = digest ∧ now_ms ≤ validUntilMs` or
`D = prevDigest ∧ now_ms ≤ prevValidUntilMs`. Signatures are verified per
§3.8 against the member list (calldata on EVM/TRON, storage on TON), requiring
`popcount ≥ t`.

#### 5.1.3 `finalizeFromTaira` (direct) and historical mode

Direct mode inputs: attestation `A`, signatures, roster, message proof
`{payload, leafIndex, path}`:

1. Chain identity check (EVM `block.chainid`, TON `GLOBALID = −239`).
2. `mintingPaused = false`.
3. Roster accepted (§5.1.2); signatures over `digest(A)` valid, ≥ `t`.
4. `A.messageCount ≥ 1`; payload valid per §3.2 with `source_domain 0`,
   `dest_domain` = own, `route_revision` = own, `route_id` = own,
   `asset_id = "xor"`, recipient valid for the own codec.
5. `leaf` = §3.4 with own destination word;
   `merkle_root(leaf, leafIndex, A.messageCount, path) = A.sccpRoot`.
6. `nonce` of the payload not consumed; mark consumed.
7. `tokenAmount = amount × units_factor`; `totalSupply (+ pending) + tokenAmount ≤ maxWrappedSupply`.
8. Mint to the recipient; emit `SccpFinalized`.

Historical mode additionally takes `{height, sccpRoot, messageCount, leafIndex, path}`
and in step 5 verifies the message against that `sccpRoot/messageCount`, then
`merkle_root(history_leaf(height, sccpRoot, messageCount), leafIndex, A.historySize, path) = A.historyRoot`.
Any attestation by an accepted roster can therefore finalize any older message.

Finalization is permissionless; the recipient is fixed by the payload.

#### 5.1.4 Supply cap

`maxWrappedSupply` is immutable and set at construction to Taira's
`max_wrapped_supply × units_factor`. Mint reverts if it would be exceeded.

#### 5.1.5 Rotation and weak subjectivity

`rotateRoster(A, current roster, signatures, next roster)`:

1. `A.rosterDigest = digest` (the **current** roster only) and
   `now_ms ≤ validUntilMs`; signatures valid, ≥ `t`.
2. `A.nextRosterDigest ≠ 0` and equals the recomputed digest of `next`
   (§3.7, including ordering and threshold checks).
3. `next.generation = generation + 1`; `next.validUntilMs > now_ms`.
4. `prevDigest = digest`, `prevValidUntilMs = min(validUntilMs, now_ms + PREVIOUS_ROSTER_GRACE_MS)`;
   install `next`; emit `SccpRosterRotated(generation, digest, validUntilMs)`.

Rotations are sequential; a lagging destination replays each generation in
order (Torii `/v1/sccp/rosters/rotations`). If the current roster expires before
anyone rotates, the destination is **frozen for minting forever** (burns stay
open); governance then registers a new revision. Taira's heartbeat guarantees a
new generation at least every `roster_max_age_ms + epoch duration`, so any
party keeping a destination alive has ≥ `roster_validity − roster_max_age`
(21 days by default) to rotate. The minting pause does not block rotation.

#### 5.1.6 Roster-controlled mint breaker

`setMintingPaused(paused, roster, signatures)`: roster MUST be the current
roster (not expired); signatures over EIP-712
`SccpMintControl{destination_word(self), paused, mintControlNonce}`; required
count `n − t + 1` (= `f+1`) to pause, `t` to resume; then set the flag,
`mintControlNonce += 1`, emit `SccpMintingPaused`. Burns and rotations are
never paused. This replaces the five-guardian one-way breaker: it needs no
privileged keys outside the Taira validator set, an honest minority of `f+1`
can still stop minting on a Taira-side bug that honest validators attested,
and it is reversible by a supermajority.

#### 5.1.7 `transferToTaira`

Burns wrapped XOR from the caller and emits the source event:

- recipient: `taira_account` bytes, 1..=256;
- `tokenAmount > 0`, divisible by `units_factor`, `amount ≤ 2^128−1`;
- EVM/TRON: `nonce = transferNonces[msg.sender]` MUST equal the caller-supplied
  `expectedNonce` (per-sender nonces make the payload derivable from calldata,
  which TRON needs because its logs are not header-committed), then increments;
  TRON additionally requires `msg.sender == tx.origin` (internal transactions
  are not provable on TRON);
- TON: `nonce = outbound_nonce++` (the Jetton master serializes all burns);
- payload per §3.2 (`dest_domain 0`, sender = caller, route constants),
  `message_id` per §3.3 with lane `(own, Taira)`.

Burns are allowed while minting is paused and after roster expiry.

### 5.2 EVM and TRON (Solidity)

One source file, `contracts/evm/sccp/SccpTairaXor.sol`, compiled by `solc`
0.8.31 for ETH/BSC and by tronprotocol `tv_0.8.31` for TRON (TRON bytecode MUST
come from the TRON compiler because it inserts `CALLTOKENID`/`CALLTOKENVALUE`
guards). The contract is the ERC-20/BEP-20/TRC-20 token itself; it makes no
external calls (so needs no reentrancy guard), has no owner, no upgrade path and
no setters.

#### 5.2.1 Constructor

```solidity
constructor(
    bytes32 tairaNetworkId,      // nonzero
    uint8   networkTag,          // 0x41 | 0x42 | 0x43
    uint32  routeRevision,       // nonzero
    uint256 maxWrappedSupply,    // token units (18 decimals), nonzero, multiple of 1e9
    RosterV1 memory initialRoster
)
```

Derives domain (1/2/5), identity word, route id text, `REQUIRE_DIRECT_CALLER =
(networkTag == 0x43)`; requires `block.chainid == identity word`; validates the
roster (§3.7) and `validUntilMs > block.timestamp × 1000`; stores its digest,
generation and `validUntilMs`; computes `DOMAIN_SEPARATOR`. Token metadata:
name `Taira XOR`, symbol `tXOR`, decimals 18, initial supply 0.

#### 5.2.2 ABI

```solidity
struct AttestationV1 {
    uint64 height; uint64 epoch; uint64 timestampMs; bytes32 blockHash;
    bytes32 sccpRoot; uint32 messageCount; bytes32 historyRoot; uint64 historySize;
    bytes32 rosterDigest; bytes32 nextRosterDigest;
}
struct RosterV1 {
    uint64 generation; uint64 validFromMs; uint64 validUntilMs;
    uint8 threshold; bytes members;          // n × 20 bytes, §3.7 order
}
struct SignaturesV1 { uint32 signerBitmap; bytes signatures; }   // 65 × popcount
struct MessageProofV1 { bytes payload; uint32 leafIndex; bytes32[] path; }   // path ≤ 9
struct HistoryProofV1 {
    uint64 height; bytes32 sccpRoot; uint32 messageCount;
    uint64 leafIndex; bytes32[] path;        // path ≤ 32
}
```

| Selector | Function | Notes |
|---|---|---|
| `0x8056d161` | `finalizeFromTaira(AttestationV1,RosterV1,SignaturesV1,MessageProofV1) returns (bytes32 messageId)` | §5.1.3 direct |
| `0x96925736` | `finalizeFromTairaHistorical(AttestationV1,RosterV1,SignaturesV1,HistoryProofV1,MessageProofV1) returns (bytes32)` | §5.1.3 historical |
| `0xd581fb85` | `rotateRoster(AttestationV1,RosterV1,SignaturesV1,RosterV1)` | §5.1.5; 2nd arg current, 4th next |
| `0xbc120437` | `setMintingPaused(bool,RosterV1,SignaturesV1)` | §5.1.6 |
| `0xebfc6ca8` | `transferToTaira(bytes tairaRecipient,uint256 tokenAmount,uint64 expectedNonce) returns (bytes32 messageId)` | §5.1.7 |
| `0x85a00f5c` | `rosterState() view returns (bytes32 digest,uint64 generation,uint64 validUntilMs,bytes32 prevDigest,uint64 prevValidUntilMs)` | |
| `0x95b67034` | `isConsumed(uint64 nonce) view returns (bool)` | |
| `0xf6f0e8a6` | `transferNonces(address) view returns (uint64)` | |
| `0x6b91d731` | `tairaNetworkId() view returns (bytes32)` | |
| `0x1818891a` | `routeRevision() view returns (uint32)` | |
| `0xd8bc4dcd` | `maxWrappedSupply() view returns (uint256)` | |
| `0xe1a283d6` | `mintingPaused() view returns (bool)` | |
| `0x41e5c0cb` | `mintControlNonce() view returns (uint64)` | |
| `0xf698da25` | `domainSeparator() view returns (bytes32)` | |

Plus the standard ERC-20 surface (`name`, `symbol`, `decimals`, `totalSupply`,
`balanceOf`, `transfer`, `transferFrom`, `approve`, `allowance`, `Transfer`,
`Approval`). Calldata is standard Solidity ABI encoding of these signatures.

Events:

```solidity
event SccpTransferToTaira(bytes32 indexed messageId, address indexed sender, uint64 nonce, bytes payload);
event SccpFinalized(bytes32 indexed messageId, uint64 indexed nonce, address indexed recipient, uint256 tokenAmount);
event SccpRosterRotated(uint64 indexed generation, bytes32 digest, uint64 validUntilMs);
event SccpMintingPaused(bool paused, uint64 controlNonce);
```

`SccpTransferToTaira` is the inbound source event consumed by Taira (§4.12.2);
`data = abi.encode(uint64 nonce, bytes payload)`, `topics[2] = word(sender)`.
Mint and burn also emit ERC-20 `Transfer` from/to `address(0)`.

Custom errors: `WrongChain()`, `MintingIsPaused()`, `RosterNotAccepted()`,
`BadRoster()`, `BadSignatures()`, `TooFewSignatures()`, `BadProof()`,
`BadPayload()`, `AlreadyConsumed(uint64)`, `SupplyCapExceeded()`,
`BadRotation()`, `BadRecipient()`, `BadAmount()`, `BadNonce(uint64 expected)`,
`DirectCallerRequired()`.

#### 5.2.3 Storage

```
bytes32 rosterDigest;                                  // slot A
uint64  rosterGeneration; uint64 rosterValidUntilMs;   // slot B (packed)
uint64  mintControlNonce; bool mintingPaused;          //   … same slot B
bytes32 prevRosterDigest;                              // slot C
uint64  prevRosterValidUntilMs;                        // slot D
mapping(uint256 => uint256) consumedBitmap;            // word = nonce >> 8, bit = nonce & 255
mapping(address => uint64) transferNonces;
ERC-20: totalSupply, balances, allowances
```

#### 5.2.4 Verification details

- Members come from calldata; the contract recomputes `keccak256` over the
  packed preimage (members are already packed `bytes`) and compares with the
  stored digest, checking ordering, `n`, `t` on the fly.
- Signatures are verified with the `ecrecover` precompile sequentially (≤ 21
  recoveries since `n ≤ 31`). On TRON this is ≈ 0.92 ms of CPU each, ≤ 20 ms
  total against the 80 ms limit, so `BatchValidateSign` (0x09) is not needed.
- Payload parsing reads calldata slices; no memory copies of the payload.
- `transferToTaira` builds the payload in memory and hashes it once for
  `payload_hash` and once for `message_id`.

#### 5.2.5 TRON specifics

- `block.chainid` returns `0x2b6653dc`; `block.timestamp` is seconds.
- Signatures use `v ∈ {27, 28}` (compatible with current rules and TIP-935).
- Energy fee 100 sun; calldata costs bandwidth (≈1 TRX per KB without free or
  staked bandwidth); keep calldata minimal (members 20 B each, signatures
  65 B each).
- One contract means no deployment-address cycle; deployment is one
  `CreateSmartContract` transaction.

### 5.3 TON (Tolk)

Three contracts: `SccpTairaXorMinter` (TEP-74 Jetton master with the bridge
built in), `SccpTairaXorWallet` (TEP-74 wallet), `SccpConsumedBucket`
(replay flags). Workchain 0. The existing bridge/master/wallet, proof verifier
and replay forest are replaced.

#### 5.3.1 Minter storage (TL-B)

```
minter_data#_ total_supply:Coins pending_supply:Coins outbound_nonce:uint64
    mint_control_nonce:uint64 minting_paused:Bool
    config:^MinterConfig roster:^RosterState prev_roster:(Maybe ^RosterState)
    content:^Cell = MinterData;
minter_config#_ taira_network_id:uint256 route_revision:uint32 max_supply:Coins
    wallet_code:^Cell bucket_code:^Cell = MinterConfig;
roster_state#_ digest:uint256 generation:uint64 valid_until_ms:uint64 n:uint8 t:uint8
    members:^MemberChunk = RosterState;
member_chunk#_ addrs:(k × uint160) next:(Maybe ^MemberChunk) = MemberChunk;   // k = min(6, remaining)
```

`prev_roster.valid_until_ms` stores the grace-capped value. `content` is TEP-64
metadata (name `Taira XOR`, symbol `tXOR`, decimals `9`). Initial data is
fixed by the constructor values, so the master address commits to the Taira
identity, revision, cap, initial roster and codes.

#### 5.3.2 Cell formats

```
attestation#_ height:uint64 epoch:uint64 timestamp_ms:uint64 message_count:uint32
    history_size:uint64 block_hash:uint256 sccp_root:uint256
    tail:^AttestationTail = Attestation;                              // 800 bits
attestation_tail#_ history_root:uint256 roster_digest:uint256
    next_roster_digest:uint256 = AttestationTail;                    // 768 bits
signatures#_ signer_bitmap:uint32 first:^SignatureCell = Signatures;
signature_cell#_ r:uint256 s:uint256 v:uint8 next:(Maybe ^SignatureCell) = SignatureCell;
message_proof#_ leaf_index:uint32 path_len:uint8 payload:^Bytes
    path:(Maybe ^HashChunk) = MessageProof;
history_proof#_ height:uint64 sccp_root:uint256 message_count:uint32 leaf_index:uint64
    path_len:uint8 path:(Maybe ^HashChunk) = HistoryProof;
hash_chunk#_ hashes:(k × uint256) next:(Maybe ^HashChunk) = HashChunk;   // k = min(3, remaining)
bytes#_ data:(8·m bits, 1 ≤ m ≤ 127) next:(Maybe ^Bytes) = Bytes;       // snake; no empty chunks
roster_spec#_ generation:uint64 valid_from_ms:uint64 valid_until_ms:uint64
    n:uint8 t:uint8 members:^MemberChunk = RosterSpec;
```

Signatures appear in ascending bitmap order, one per cell (520 bits; two do not
fit in 1 023 bits).

#### 5.3.3 Messages to the minter

| Op | TL-B | Sender |
|---|---|---|
| `0x53434631` | `sccp_finalize query_id:uint64 response:MsgAddressInt attestation:^Attestation signatures:^Signatures message:^MessageProof` | anyone; value ≥ `FINALIZE_MIN_VALUE` (0.15 TON) |
| `0x53434632` | `sccp_finalize_historical query_id:uint64 response:MsgAddressInt attestation:^Attestation signatures:^Signatures history:^HistoryProof message:^MessageProof` | anyone; same value |
| `0x53435231` | `sccp_rotate query_id:uint64 response:MsgAddressInt attestation:^Attestation signatures:^Signatures next:^RosterSpec` | anyone |
| `0x53434d31` | `sccp_mint_control query_id:uint64 response:MsgAddressInt paused:Bool signatures:^Signatures` | anyone |
| `0x53434332` | `sccp_consume_result query_id:uint64 nonce:uint64 amount:uint96 ok:Bool tail:^ConsumeTail` | bucket |
| `0x7bdd97de` | `burn_notification query_id:uint64 amount:Coins sender:MsgAddress response_destination:MsgAddress sccp:^SccpBurnToTaira` | own wallet |
| standard | `provide_wallet_address` / TEP-89 | anyone |

```
consume_tail#_ recipient:MsgAddressInt response:MsgAddressInt message_id:uint256 = ConsumeTail;
sccp_burn_to_taira#53434254 recipient:^Bytes = SccpBurnToTaira;
```

#### 5.3.4 Flows

**Finalize (Taira→TON).**

1. Minter: check `GLOBALID = −239`, not paused, roster accepted (current or
   previous, members from storage), signatures (`ECRECOVER` with `v − 27`,
   low-S checked in-contract, address = low 160 bits of
   `HASHEXT keccak256(x ‖ y)`), the Merkle path(s) and the payload (§5.1.3).
   `amount < 2^96`. Check `total_supply + pending_supply + amount ≤ max_supply`;
   `pending_supply += amount`.
2. Send `sccp_consume#53434331 query_id nonce:uint64 amount:uint96 tail:^ConsumeTail`
   to `bucket(nonce >> 9)` with its StateInit
   (`bucket_data#_ minter:MsgAddressInt index:uint64 bits:uint512`, bits
   initially 0), carrying the remaining value (mode 64). The first 256 body bits
   are exactly `op ‖ query_id ‖ nonce ‖ amount`, so a bounce carries enough to
   release the reservation.
3. Bucket: require sender = minter; on first deployment reserve 0.01 TON for
   storage; `ok = bit (nonce & 511) unset`; set it if `ok`; reply
   `sccp_consume_result` to the minter carrying the remaining value.
4. Minter on result: require sender = `bucket(nonce >> 9)` (recomputed from
   `bucket_code`); `pending_supply −= amount`; if `ok`,
   `total_supply += amount`, send standard `internal_transfer#178d4519` to the
   recipient's wallet (with StateInit) with `response_destination = response`,
   and emit ext-out `sccp_finalized#5343464e message_id:uint256 nonce:uint64`;
   else return the value to `response`. On a bounced `sccp_consume`,
   `pending_supply −= amount` and return the value.

**Burn (TON→Taira).** The owner sends TEP-74
`burn#595f07bc query_id amount response_destination custom_payload` to its
wallet with `custom_payload = sccp_burn_to_taira`. The wallet requires the
custom payload (a plain burn is rejected, so no value can be burned without a
Taira record), debits the balance and sends the extended `burn_notification`.
The minter verifies the wallet address, validates the recipient bytes (1..=256),
`total_supply −= amount`, `nonce = outbound_nonce++`, builds the payload
(`sender_codec 7` = owner), computes `message_id`, emits the ext-out source event

```
sccp_transfer_to_taira#53435454 message_id:uint256 nonce:uint64 sender:MsgAddressInt
    amount:Coins payload:^Bytes
```

and returns excess to `response_destination`. Any failure throws; the standard
bounce of `burn_notification` restores the wallet balance.

**Rotate / mint control.** Per §5.1.5–5.1.6 with members read from storage;
rotation rewrites `roster_state` and moves the old one to `prev_roster`; excess
value returns to `response`.

#### 5.3.5 Get methods

`get_jetton_data`, `get_wallet_address(owner)` (TEP-74);
`get_sccp_state() → (taira_network_id, route_revision, digest, generation,
valid_until_ms, prev_digest, prev_valid_until_ms, outbound_nonce,
minting_paused, mint_control_nonce, total_supply, pending_supply, max_supply)`;
`get_sccp_members() → tuple`; `get_bucket_address(index)`. Consumption of a
nonce is read from the bucket account state (`bits`) or its `get_bits` method;
a non-existent bucket means nothing in it is consumed.

#### 5.3.6 Tolk notes

Tolk 1.4.2 has no ECRECOVER or keccak builtins; declare
`ECRECOVER` (opcode `0xF912`, stack in `hash v r s`, out `h x y -1` or `0`) with a
normalizing asm wrapper (e.g. `asm "ECRECOVER" "NULLSWAPIFNOT2" "NULLSWAPIFNOT"`,
to be confirmed in the Acton emulator) and `HASHEXT` id 3 (keccak256) over
builders/slices. Mainnet global version is 15, so `v` 27/28 would also be
accepted, but contracts pass `v − 27` for independence from that.

### 5.4 Cost estimates

Estimates (not measurements) at current pricing; `n=4` is Taira today,
`n=31` is the protocol maximum. Tests MUST record measured values (§11).

| Operation | ETH/BSC gas | TRON energy (+ bandwidth) | TON |
|---|---|---|---|
| `finalizeFromTaira`, n=4 (t=3) | 75k–110k | 35k–60k (+≈1.3 KB) | ≈15k gas at the minter; attach 0.15 TON, ≈0.02–0.04 consumed incl. bucket and wallet deployment |
| `finalizeFromTaira`, n=31 (t=21) | 160k–200k | 95k–120k (+≈3 KB) | ≈50k gas at the minter; ≈0.03–0.05 TON consumed |
| historical mode | +8k–20k | +3k–8k (+≤1 KB) | +2k–5k gas |
| `rotateRoster`, n=4 / n=31 | 65k–80k / 160k–180k | 40k–60k / 100k–130k | ≈15k / ≈60k gas (≤ 8 cell writes) |
| `setMintingPaused` | 50k–150k | 30k–90k | ≈10k–50k gas |
| `transferToTaira` | 55k–75k | 40k–60k | standard burn + ≈10k gas |

Drivers: `ecrecover` 3 000 (+100 call) per signature; calldata 16 gas per
nonzero byte (EIP-7623 floor 40); new consumed-bitmap word 22k (then 2.9k per
nonce within the word); new balance slot 22k. Under the scheduled Glamsterdam
repricing (EIP-7976 64 gas/calldata byte floor, EIP-8037 new-slot ≈98k)
`finalizeFromTaira` at n=31 becomes floor-bound at ≈215k; the digest-only
roster storage and bitmap consumed set were chosen to stay robust to it.

### 5.5 Toolchains and reproducible artifacts

| Target | Compiler | Settings |
|---|---|---|
| ETH, BSC | solc `0.8.31+commit.fd3a2265` (universal macOS arm64/x86-64, linux-amd64, linux-arm64) | `evmVersion: "cancun"`, `optimizer: {enabled: true, runs: 200}`, `viaIR: false`, `metadata.bytecodeHash: "none"`, `metadata.appendCBOR: false` |
| TRON | tronprotocol `tv_0.8.31` `0.8.31+commit.c2812a3d` (`solc-macos` universal, `solc-static-linux`, `solc-static-linux-arm`) | same settings |
| TON | Acton 1.2.0 bundling Tolk 1.4.2 (`acton-aarch64-apple-darwin`, linux builds) | fixed optimization; code cell hashes locked |

- `scripts/contract_tooling/compiler-lock.json` pins URLs and sha256 of these
  binaries; `contract_artifact_corridor.py` builds, locks and verifies
  artifacts, including each contract's runtime template and
  `immutableReferences`. No Rosetta or Docker is needed on macOS arm64.
- Explicit `evmVersion: cancun` is required because both compilers default to
  `osaka`; the legacy pipeline avoids the via-IR-only 0.8.31 bugs; the source
  MUST NOT `delete` memory `bytes` elements or use custom storage layouts
  (0.8.31 legacy-pipeline bug patterns).
- `Acton.toml` moves to `acton = "1.2.0"` (PascalCase wrapper names,
  `acton simulator` replaces the lightweight localnet).

## 6. Torii read API (served by every Taira peer from state)

All routes are public GETs (`public_sccp_get` descriptors), JSON or Norito by
`Accept`, derived only from committed state, so every honest peer returns
identical bytes and wallets may cross-check peers.

| Route | Path | Content |
|---|---|---|
| `sccp.capabilities` | `GET /v1/sccp/capabilities` | `{network_id, chain_id, profiles, eip712_domain_separator, parameters, latest_attested_height, current_generation, path templates, limits}` |
| `sccp.registry` | `GET /v1/sccp/registry` | route revisions, deployments, activation, liability, cap, `next_outbound_nonce` |
| `sccp.messages.recent` | `GET /v1/sccp/messages/recent?direction&network&after_index&limit≤50` | recent outbound/inbound records |
| `sccp.message.status` | `GET /v1/sccp/messages/{message_id}` | outbound `{recorded|attested{signers,threshold}}` or inbound `{consumed{height, outcome}|unconsumed}` |
| `sccp.message.proof` | `GET /v1/sccp/messages/{message_id}/proof?attestation=own|latest|<height>` | `SccpMessageProofBundleV1`: payload, leaf index, message count, path, attestation statement + digest, roster (members), signature set (≥ t, ascending), optional history proof; 409 `sccp_attestation_pending` until attested; immutable per query (ETag) |
| `sccp.attestation` | `GET /v1/sccp/attestations/{height}` and `/latest?generation=G` | statement, digest, status, signatures |
| `sccp.roster` | `GET /v1/sccp/rosters/{generation}`, `/current` | roster record incl. peers, members, digest |
| `sccp.roster.rotations` | `GET /v1/sccp/rosters/rotations?after_generation=G&limit≤16` | ordered `[{attestation, signatures, current_roster, next_roster}]` catch-up chain |
| `sccp.history.proof` | `GET /v1/sccp/history/{height}?size=S` | history leaf and path within `history_root(S)` |
| `sccp.bridge_keys` | `GET /v1/sccp/bridge-keys` | active/pending keys per peer |
| `sccp.light_clients` | `GET /v1/sccp/light-clients`, `/{network}`, `/{network}/checkpoints?covering=N` | light-client summary, exact canonical state bytes and `state_hash`, nearest retained checkpoint ≥ N |
| `sccp.governance` | `GET /v1/sccp/governance/pending` | pending approvals by action hash |

Writes use the generic `POST /v1/pipeline/transactions`,
`GET /v1/pipeline/transactions/{hash}/status` and `POST /v1/fees/quote`. There
are no SCCP-specific submit endpoints and no destination calldata or BoC
projections in Torii. After the change, regenerate
`specs/sdk_operation_inventory.tsv` and the OpenAPI artifacts.

## 7. Wallet flows (no hosted services)

Wallets run the native Rust library (§8) against any Taira peer's Torii and the
public RPC endpoints listed in the user's `[sccp]` client config (third-party
endpoints; nothing operated by the project). Every flow verifies before paying:
attestation signatures against the destination's on-chain roster digest, Merkle
paths, and inbound proofs through the same `iroha_sccp` verifier Taira runs.

### 7.1 Taira → external

1. `GET /v1/sccp/capabilities`; check `network_id` against the pinned
   deployment set; `GET /v1/sccp/registry` for the `Bidirectional` revision.
2. Destination read: EVM/BSC `eth_call rosterState()`, `tairaNetworkId()`,
   `eth_getCode` hash; TRON `/wallet/triggerconstantcontract` for the same;
   TON `liteServer.runSmcMethod get_sccp_state`. Refuse on mismatch.
3. `POST /v1/fees/quote`, then sign and `POST /v1/pipeline/transactions` with
   `RecordSccpMessage{network, amount, recipient}`; poll status; read
   `message_id` from the `SccpMessageRecorded` event or
   `GET /v1/sccp/messages/recent`.
4. Poll `GET /v1/sccp/messages/{id}` until `attested` (≈ 8–12 s).
5. If the destination's generation is behind the attestation's:
   `GET /v1/sccp/rosters/rotations?after_generation=<dest>` and submit each
   `rotateRoster` / `sccp_rotate` in order.
6. `GET /v1/sccp/messages/{id}/proof?attestation=own` (direct) or `latest`
   (historical when the own attestation's roster is no longer accepted).
   Verify locally, trim signatures to exactly `t`.
7. Submit: EVM `eth_sendRawTransaction` (EIP-1559 type-2) of
   `finalizeFromTaira*`; TRON `/wallet/broadcasthex` of a native-built
   `TriggerSmartContract`; TON `liteServer.sendMessage` of a wallet-v5 external
   message carrying `sccp_finalize` (or hand-off via TON Connect / WalletConnect
   / TronLink with `--emit`).
8. Confirm: `isConsumed(nonce)` / bucket bits; `SccpFinalized` log.

Anyone may perform steps 5–7 for the recipient.

### 7.2 External → Taira

1. Check the Taira recipient account exists (`GET /v1/accounts/{id}`) and
   compute `tairaRecipient = AccountAddress` bytes locally from the I105 literal
   (checksum verified).
2. Burn: EVM/TRON `transferNonces(sender)` then `transferToTaira(recipient,
   amount, expectedNonce)`; TON jetton `burn` with `sccp_burn_to_taira`.
3. Wait for source finality and collect evidence from public RPC:
   - ETH: `eth_getTransactionReceipt`, `eth_getBlockReceipts(B)` (rebuild
     receipt trie), `eth_getBlockByNumber(B)` (re-encode header, check hash),
     beacon `/eth/v1/beacon/light_client/finality_update`,
     `eth_getProof(0x0000F90827F1C53a10cb7A02335B175320002935, [B mod 8191], E)`
     when `B ≠ E`, or a checkpoint path (`/v1/sccp/light-clients/…/checkpoints`).
   - BSC: same receipt/header calls; `eth_getBlockByNumber(B+1..B+4)` for the
     vote attestation in `extraData`; `eth_getFinalizedHeader` for status.
   - TRON: `/wallet/getblockbynum` (full transactions, rebuild `txTrieRoot`),
     `/wallet/getblockbylimitnext` for the solidity segment,
     `/walletsolidity/gettransactioninfobyid` for discovery.
   - TON: liteserver ADNL (`ton.org/global-config.json` peers):
     `getMasterchainInfo`, `lookupBlock`, `getBlockProof`, `getAllShardsInfo`,
     `getBlock`, `getOneTransaction`.
4. `GET /v1/sccp/light-clients/{network}`; if the light client does not cover
   the evidence, build `AdvanceSccpLightClientV1` updates (ETH
   `/eth/v1/beacon/light_client/updates`; BSC epoch hops; TRON segments; TON
   key-block `getBlockProof` links) with `expected_state_hash`.
5. Run the verifier locally, then submit one transaction
   `[AdvanceSccpLightClientV1…, SubmitSccpInboundMessageV1]` (fee paid by
   the submitter or a sponsor) and poll status.

### 7.3 Destination upkeep, deployment and registration

- **Rotation keeper:** anyone runs `iroha sccp roster sync --target <profile>`
  (reads `rosterState`, walks `/v1/sccp/rosters/rotations`, submits rotations).
  At least once per `roster_validity − roster_max_age` per destination.
- **Deployment:** `iroha sccp deploy <evm|tron|ton>` deploys from locked
  artifacts with the deployer's key and the current roster from
  `GET /v1/sccp/rosters/current`, and prints the `RegisterRoute` action.
- **Registration:** each validator runs `iroha sccp deployment verify` (§4.14.4),
  then `iroha sccp governance sign --action <file> --bridge-key-file <path>`;
  anyone submits `ApproveSccpGovernanceV1` with collected approvals, then the
  activation action the same way.

## 8. Off-chain Rust

- `crates/iroha_sccp` (network-free, linked into `iroha_core`): payload codec,
  hashes, Merkle and history functions, EIP-712 digests, roster digest,
  signature verification, per-chain inbound verifiers and light-client logic,
  Torii DTOs (`api.rs`). All compiled Taira constants and Groth16 code removed.
- `crates/iroha_sccp_wallet` (new; std; never in the `irohad` graph, enforced by
  a new `sccp_wallet` layer in `ci/dependency_budget.json`):
  - `pure/`: bundle and rotation verification, EVM ABI encoding and EIP-1559
    signing, TRON `Transaction.raw` protobuf builder and signing, TON cell/BoC
    builders and wallet-v5 messages, inbound proof assembly from fetched raw
    evidence (MPT/RLP/SSZ/BoC builders). FFI-exportable for SDK bridges.
  - `rpc/` (feature `rpc`, default): blocking `reqwest` (rustls, no `json`
    feature; `norito::json` parsing), EVM JSON-RPC, beacon API (SSZ), TRON
    HTTP, TON ADNL-TCP liteclient built from workspace crypto crates.
  - `flows.rs`: resumable flows journaled via `iroha_wallet::operation_journal`
    keyed by `NetworkId`.
  - `config.rs`: client-config `[sccp]` table (file-only): endpoint lists,
    timeouts, pinned deployments per `NetworkId`.
- CLI `iroha sccp {info, routes, recent, send, status, proof, finalize, roster
  sync, burn, claim, light-client status|advance, deploy, deployment verify,
  governance sign|submit, bridge-key register, broadcast}`. Taira keys from
  `account.private_key_file`; external keys only from owner-only key files or
  `iroha_wallet` named wallets; `--emit` for external signers; no keys on argv
  or in environment variables.
- SDKs (phase 2): read models, `RecordSccpMessage` building, bundle
  verification and destination encoding via `connect_norito_bridge` (Swift,
  Kotlin, C#) and `iroha_js_host` (JS); Python verifies only. Java adds nothing
  and its SCCP surface is deleted.

## 9. Security analysis

### 9.1 Trust assumptions

| Direction | Safety holds if | Liveness needs |
|---|---|---|
| Taira → X | fewer than `t = ⌊2n/3⌋+1` members of each generation accepted by the destination sign non-canonical statements (the Taira BFT assumption, applied to bridge keys) | `t` online attestors; one honest submitter on X; a keeper rotating within the validity window |
| X → Taira | the source chain's finality assumption (ETH ≥2/3 sync committee; BSC ≥2/3 Parlia BLS votes; TRON ≥19 of 27 SRs; TON >2/3 validator weight) within the weak-subjectivity bound, **and** Taira BFT | one submitter; a light client kept within `ws_bound` |

Taira today runs 4 validators on one host under one custody owner, so the
effective trust in both directions is that operator. The protocol is
correct for any `n ≤ 31`; decentralization is a deployment property.

### 9.2 Replay

- Destination: each (revision, nonce) mints at most once (bitmap / bucket);
  nonces are dense and unique per revision because Taira assigns them in
  deterministic execution; the leaf binds the destination contract, the
  payload binds the revision, and the message id binds both network identities,
  so a message can neither be minted twice nor on another contract, chain,
  revision or Taira network.
- Taira: `sccp_inbound_consumed[message_id]` is permanent; message ids are
  unique because source nonces are per-sender (EVM/TRON) or serialized
  (TON) and the payload includes sender, nonce and revision.
- Attestation signatures cannot be replayed across Taira networks (EIP-712
  salt) or into other typed messages (distinct typehashes).

### 9.3 Rogue keys and key binding

A bridge key enters a roster only with (a) the peer's consensus-key consent
and (b) a proof of possession of the secp256k1 key over a statement naming the
peer and epoch. Addresses are globally unique and never reused, so one key
cannot occupy two slots, a key cannot be claimed by two peers, and faults are
attributable forever. ECDSA has no aggregation, so rogue-key cancellation does
not apply; PoP prevents registering someone else's address.

### 9.4 Signature malleability and counting

Contracts and Taira enforce low-S, `v ∈ {27,28}`, nonzero `r`, `s`, nonzero
recovered address, position-addressed signers via a bitmap, and strictly
ascending nonzero members. Duplicated or malleated signatures therefore
cannot inflate the count. Consumption is keyed by nonce, never by signature
bytes.

### 9.5 Domain separation

EIP-712 `0x1901` prefix plus per-type typehashes plus the Taira `NetworkId`
salt; ASCII role prefixes for payload, message, leaf, node, history and roster
hashes; leaf and node hashes use distinct prefixes and the verifier computes
leaves itself, so an internal node cannot pass as a leaf; the leaf index and
count are bound (positional proof with authenticated `messageCount` /
`historySize`). The bridge key is dedicated: it never signs Iroha
transactions (which hash with SHA-256) or Ethereum transactions.

### 9.6 Cross-chain / cross-route confusion

The same attestation is valid on all destinations by design; each destination
accepts only leaves with its own destination word, domain, revision and route
id. Deployments on other chains at colliding addresses (EVM CREATE collisions,
TON same-StateInit on testnet) are rejected by the `block.chainid` / `GLOBALID`
check and would mint only unregistered, unredeemable tokens.

### 9.7 Taira reset

New `NetworkId` ⇒ new domain separator, new lane bytes, new roster digests: no
old attestation, message or burn is valid across the reset. Old deployments
freeze naturally (their rosters expire; their burns are not accepted by the new
Taira). Stranded value is the documented cost (§4.17).

### 9.8 Long-range attacks and weak subjectivity

Threat: after validators leave a generation, `t` of its keys are compromised and
used against a destination that still trusts that generation. Mitigations:

1. Destinations accept a generation only until its `valid_until_ms`; Taira
   produces a successor at least every `roster_max_age_ms` (heartbeat), so an
   actively kept destination never trusts keys older than ≈ 7 days plus the
   24 h previous-roster grace.
2. An unkept destination freezes (minting only) when its roster expires; it
   never accepts a rotation signed by an expired roster.
3. Slashability: validators must remain bonded for
   `roster_validity_ms + evidence_horizon` after leaving; any forged statement
   (on any destination's calldata) is attributable via
   `SubmitSccpAttestationFaultV1`. **Taira currently has `unbonding_delay = 0`
   and nominal stake, so this deterrent is inactive on Taira; the time bound
   (1–2) is the operative protection.**
4. Inbound light clients enforce per-chain `ws_bound_ms` against Taira block
   time and freeze on proven equivocation.

### 9.9 Censorship and liveness

- Taira censorship of `RecordSccpMessage`, attestations or inbound claims
  requires control of proposers across leader rotation; any validator node
  resubmits attestations; any wallet can relay stored signatures.
- Destination-side censorship affects only timing; finalization, rotation and
  burns are permissionless and idempotent.
- `f+1` validators can pause a destination's minting and halt Taira itself;
  this is the same liveness assumption as consensus.
- A stalled source-chain light client (e.g. BSC fast finality stalled across an
  epoch switch, TRON losing ≥9 known SRs before learning replacements) needs
  re-initialization by governance; funds are delayed, not lost.

### 9.10 Economic safety

- Destination supply ≤ immutable cap; Taira never records a message that would
  exceed it (§4.15), so no attested message is unmintable because of the cap.
- Taira releases on inbound only up to the revision's liability, so a
  compromised source-chain verification can drain at most that revision's
  escrow, and a compromised Taira attestation can mint at most the destination
  cap minus supply.
- Burns before Taira's recipient checks can only fail recoverably: a missing
  account delays the claim; an undecodable recipient bounces (§4.12.5).

### 9.11 Key compromise

- One bridge key (≤ `f`): no effect on safety; rotate with
  `SetSccpBridgeKeyV1` (next epoch) or report faults to force eviction.
- `≥ t` keys of the current generation: can mint on all destinations up to their
  caps and approve governance actions. Response: `f+1` honest validators pause
  minting on each destination (§5.1.6) and pause routes on Taira; after key
  rotation governance registers new revisions. The same compromise of consensus
  keys would already break Taira itself.
- The node attestor keeps the key hot; it signs only locally committed
  statements, never arbitrary input. Governance and mint-control signatures
  are produced deliberately by the operator CLI.

## 10. Removed from the repository

No alias, shim or fallback decoder survives.

**Rust (`crates/`)**

- `iroha_sccp`: Groth16 BN254/BLS12-381 requests, statements, public signals,
  wrappers and pairing verification; `TairaSccpMessageProofV1` and Taira BLS
  finality-proof verification; destination parse/verify; old
  `finalizeFromTaira` calldata, TON BoC and Solidity replay-witness encoders;
  compiled `SCCP_TAIRA_*` identity constants; `replay_archive.rs`;
  `bin/sccp_release_evidence.rs` with its `[[bin]]` entry and `dev-tools`
  feature; `halo2curves` if unused; forgeable-key Groth16 fixtures; BLAKE2b
  SCCP hashing (`prefixed_blake2b`, `sccp:*` BLAKE2b prefixes).
- `iroha_data_model`: `bridge/sccp_replay.rs` (+ directory),
  `bridge/sccp_ton_breaker.rs`, Groth16 key/profile/anchor/outbound-policy/IVM
  execution-policy/TON-guardian/schema-hash registry types,
  `SccpNativeTrustAnchorV1`, `BridgeSccpDestinationProof*`,
  `BridgeProofPayload::SccpDestination`, `BlockHeader::sccp_commitment_root`
  (and payload/builder/projection sites), `RecordSccpMessage.replay_witness`
  and payload-carrying `RecordSccpMessage` form, SCCP use of
  `SubmitBridgeProof`, `ApplySccpRouteGovernance`,
  `SubmitSccpTonBreakerObservationV1`, `ProposeSccpRouteGovernance`,
  `ProposalKind::SccpRouteGovernance`, pending-outbound records and usage.
- `iroha_core`: SORA finality anchor derivation, destination-proof acceptance
  and receipt projection, replay admission and Kura replay-archive rebuild,
  header-root validation and `SccpRootValidation`, candidate-attachment root,
  proposal-time collectors, `IvmProved` SCCP execution binding
  (`SccpIvmProvedExecutionBindingV1`, overlay plumbing), TON breaker,
  Parliament SCCP bodies, `sccp_replay_forests`, `sccp_ton_breaker_observations`,
  `sccp_outbound_pending_usage`, `sccp_inbound_anchor_high_water`, the compiled
  chain-id gate. SCCP ISIs move out of `smartcontracts/isi/world.rs` into
  `smartcontracts/isi/sccp/`.
- `iroha_torii`: `sccp_replay.rs` (+ directory), readiness gate, bootstrap
  abort and refresh worker; proof-request, outbound-material and replay GETs;
  destination-proof and native-message POST flows and their ingress policy;
  governance draft route; catalog entries for all of these.
- `iroha_config`: `[torii.sccp_replay_archive]`; `[zk.sccp]` pending-outbound,
  pairing and BLS-aggregate knobs; `SCCP_LAUNCH_MODE`.
- Executor: `CanManageSccpGovernance`, `CanProposeSccpRouteGovernance`, their
  grant rules and deny visitors.
- `iroha` client and `iroha_cli`: proof-request/submit methods, compiled
  Taira chain check, `ops bridge sccp` (replaced by `iroha sccp`),
  `gov_instruction record-sccp-transfer | ensure-ivm-execution-vk |
  propose-sccp-route-governance`.
- IVM/Kotodama: `ledger::sccp::record` builtin and every compiler/IR/semantic
  site, syscall `0xA0` operation tag 2 in `ivm_abi`, `mock_wsv` and core host
  handling, fixtures `c085.ko`/`c086.ko`, the tag in `crates/ivm/docs/syscalls.md`
  and `ivm/spec/syscalls.toml`. Syscall numbering is unchanged; the
  `abi_syscall_list` and `abi_hash` goldens MUST still pass unchanged.

**Contracts, circuits, scripts, CI, docs, SDKs**

- `circuits/sccp/` entirely (Go gnark circuits, manifests, KATs).
- `contracts/evm/sccp/{SccpGroth16Bn254MessageVerifier, SccpSha256ReplayForest,
  ISccpMessageVerifier, TairaXorExactEvmSccpBridge, TairaXorEvmToken,
  SccpExactTransferCodec}.sol` and their replay-forest smoke; `contracts/ethereum/sccp/`,
  `contracts/bsc/sccp/` wrappers; `contracts/tron/sccp/*` (TRON uses the shared
  source); TON `proof-verifier.tolk`, `replay-forest.tolk` and the old
  bridge/master/wallet; `artifacts/sccp-bsc/` diagnostic circom.
- `scripts/sccp_release_{bundle,common,fixture,readiness_report}.py`,
  `sccp_verify_release_bundle.py`, `sccp_all_lanes_evidence.py`,
  `sccp_phase_log_runner.py`, `sccp_validator_builder{,_driver}.py`,
  `check_sccp_production_corridor.sh`, `check_sccp_vendor_generated.py`;
  the production corridor of `ton_sccp_builder.py`; the disabled live phase of
  `contract_tvm_smoke.mjs` (rewritten).
- `.github/workflows/sccp_production_corridor.yml`; the matching `pytests/`.
- `fixtures/sccp/release_evidence_v1/`, `fixtures/sccp/replay_forest_v1.json`.
- `docs/source/sccp_validator_release_builder.md`; rewrite
  `docs/source/sccp_ton_release_builder.md`; delete `specs/bridge_proofs.md`
  when this spec is implemented.
- SDKs: replay, Groth16 key and proof-request surfaces in JS, Python, Swift,
  Kotlin and C#; the Java SCCP surface.

## 11. Tests and fixture migration

New shared vectors (`fixtures/sccp/`, Rust-generated, consumed by contract and
SDK tests; every assertion of the retired suites that still applies is ported):

| Fixture | Content |
|---|---|
| `payload_v1.json` | payloads for all 8 directions, payload hashes, message ids, rejected encodings |
| `commitment_tree_v1.json` | leaves, roots and every path for counts 1..=17 and 512, including promoted nodes |
| `history_v1.json` | history leaves, roots and paths for sizes 1..=40; peak-bagging equivalence |
| `eip712_v1.json` | domain separator for a fixed `NetworkId`; attestation, key-PoP, governance and mint-control digests; low-S signatures from fixed keys; high-S and `v ∉ {27,28}` negatives |
| `roster_v1.json` | roster digests for n = 4, 7, 31 with zero slots; ordering and threshold negatives |
| `evm_calldata_v1.json`, `ton_bodies_v1.json` | golden calldata / BoC for finalize, historical, rotate, mint control, transferToTaira, burn |
| `native_transfer_event_v1.json` | regenerated for the keccak/big-endian layout and new event shapes |
| `rpc/{eth,bsc,tron,ton}/` | captured public-RPC responses for real mainnet blocks, driving inbound proof builders and verifiers |

Required tests:

- Rust unit tests for every new function (codecs, digests, Merkle/history,
  roster derivation, signature rules, each ISI's validation branches).
- Core: 4-peer integration test `RecordSccpMessage → commit →
  sccp_block_commitments[h] → attestor signatures → BlockAttested →
  /v1/sccp/messages/{id}/proof` verified by the wallet crate, including a
  failed record in the same block; roster generation change and heartbeat;
  attestation admission rejection of invalid, unknown-subject and all-duplicate
  batches; fault evidence (future height, missing subject, mismatched field);
  escrow/liability invariant; bounce; governance thresholds (`t`, `f+1`,
  expiry, generation change).
- Inbound: per-chain positive proofs from captured mainnet data and negatives
  (equivocation freeze, weak-subjectivity rejection, EIP-2935 window edges,
  TRON unknown-signer learning, TON skipped key block, shard walk across split
  and merge).
- Contracts: EDR (EVM) and Acton (TON) suites for every §5 rule, including
  expired roster, previous-roster grace, sequential rotation, frozen
  destination, cap, bitmap word boundaries, bucket boundaries, bounced consume,
  plain burn rejection, mint pause thresholds; measured gas/energy recorded
  against §5.4. TRON bytecode also runs on a local java-tron (TRE) node.
- ABI goldens: `crates/ivm/tests/abi_syscall_list_golden.rs` and
  `abi_hash_versions.rs` unchanged after removing operation tag 2.
- Regenerate `tests/fixtures/block_signature_identity_frames.json` and schema
  goldens after the header change.

## 12. Phasing and later work

- **Phase 1 (this spec):** everything above except items marked phase 2.
- **Phase 2:** stake slashing for SCCP faults (§4.11); SDK parity (§8); MCP
  `iroha.*` read tools.
- **Later:** carry attestations in Sumeragi Commit votes once v2 settles (same
  digest, no contract change); implicit recipient-account creation if the
  universal-account policy permits it; destination attested-root caching if
  measured traffic justifies it.

## 13. Open questions

1. **Taira decentralization and stake.** Four validators on one host with
   `unbonding_delay = 0` make both slashing and the `t`-of-`n` assumption
   nominal. Setting `nexus.staking.unbonding_delay ≥ roster_validity_ms +
   evidence horizon` and distributing validators are deployment decisions
   outside this spec.
2. **Height-context access.** §4.3.2 assumes the boundary block's execution can
   read `HeightContext(h_b).next_epoch_snapshot.roster`. If it cannot, the
   fallback is to create the rotation subject at `h_b + 1` from the parent's
   finality artifact, signed by the outgoing generation.
3. **Consensus roster never rotates today** (`v2_context.rs` TODO): generations
   change only through key rotation, faults and the heartbeat until NPoS roster
   activation lands.
4. **Fee exemption** for attestation and fault transactions requires extending
   the protocol exemption list; the alternative is paying ordinary fees from a
   genesis fee-sponsor program.
5. **Recipient accounts must pre-exist on Taira**; confirm this versus implicit
   account creation under the universal-account model.
6. **Public RPC coverage** (beacon light-client routes, `eth_getBlockReceipts`
   and `debug_*` availability on BSC, TRON full-block endpoints, liteserver
   reliability) is unverified; the wallet takes endpoint lists and fails over.
7. **Ethereum Glamsterdam/Gloas** changes the beacon light-client layout; the
   Ethereum light client fails closed at the fork until a code release.
8. **TRON contract callers** cannot burn (the `tx.origin` rule) because internal
   transactions are not provable without committed logs.
9. **Parameter tuning:** `roster_max_age_ms` (7 d), `roster_validity_ms` (28 d),
   `PREVIOUS_ROSTER_GRACE_MS` (24 h), `FINALIZE_MIN_VALUE` (0.15 TON) and the
   TON ECRECOVER asm wrapper need confirmation by measurement.
10. **Wrapped test XOR on mainnets** is stranded on every Taira reset; token
    naming and user disclosure need product sign-off.
