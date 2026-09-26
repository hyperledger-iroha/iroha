# Contract-state exact-value proof V1

This is a follow-on protocol release. The current Sumeragi V2
`ExecutionCommitment.post_state_root` covers a block's execution witness writes
(or reads for a read-only block), not every current World value. It must never
be used to prove an arbitrary `/v1/contracts/state` value. The existing
`/v1/ledger/state-proof/{height}` response authenticates that execution root
only; its route name does not turn it into an accumulated state proof.

## Accumulated contract-state commitment

`ContractStateMapV1` commits the complete current `smart_contract_state`
storage, including untouched values, with `iroha_crypto::MerkleMap`. Each leaf
uses the exact physical `StatePath` and unmodified stored bytes:

```
key_hash   = H("iroha:contract-state:key:v1\0" || UTF8(StatePath))
value_hash = H("iroha:contract-state:value:v1\0" || stored_bytes)
leaf       = H("iroha:merkle-map:leaf:v1\0" || key_hash || value_hash)
root       = MerkleMap.root()  // includes the exact leaf count
```

`H` is Iroha's Blake2b-256 `Hash::new`, including its low-bit marker. The
compressed MerkleMap path uses MSB-first split bits. `StatePath` is the
canonical physical key: a contract-scoped query proves `sc/{address_digest}/`
plus its logical path, not the logical path alone. An absent key has no V1
inclusion proof. A zero-length stored value is present and provable.

State owns one retained map version for the exact predecessor World. Cold
startup captures the complete World store. Block preparation applies the
actual `smart_contract_state` MV journal to a clone, preimage-checks every
touched key, and obtains the post-block root. The resulting version must be
published atomically with the committed World and block-hash journal; failed
or replaced candidates cannot advance it. Recovery rechecks the root against
the durable finality artifact. Only the actual post-block map root is placed
in a mandatory, separately named Sumeragi V2 execution-commitment field,
`contract_state_root`. Existing `post_state_root` keeps its execution-witness
meaning. Validator replay recomputes `contract_state_root` from its own World
before signing or accepting the CommitQC. A wire/schema version change is
required; there is no legacy fallback.

## Proof response and trust

The first route is `GET /v1/contracts/state?contract_address=...&path=...&proof=true`.
Proof mode accepts one exact `path`, a canonical contract address, raw value
inclusion, and no `paths`, `prefix`, `decode`, alias, or pagination options.
The returned proof includes the canonical physical key, raw bytes, MerkleMap
leaf count and compressed membership steps, one-based committed height, the
exact `contract_state_root`, block hash/header, and the immutable Sumeragi V2
finality artifact. A missing value, unavailable committed map version,
missing/malformed finality, or a root/height/hash mismatch fails closed.

The verifier first checks the requested address and exact physical path,
then verifies inclusion against `contract_state_root`. It requires that root
to equal the new field in the artifact's CommitQC. It validates the header,
block hash and height against the artifact, and verifies finality using an
externally trusted height-context anchor and linked successors, as in
`BridgeFinalityVerifier`. The artifact's self-described roster cannot serve
as its own trust anchor. A client that only calls
`verifyContractStateValueInclusionV1` must supply a separately authenticated
accumulated root; that helper alone does not establish finality.

The admission and resource caps are one key, 1 MiB raw value, 256 proof steps,
and a bounded canonical Norito response. JSON and Norito encode the same
closed fields. Exact-key, value, count, branch, root, height, block, network,
context, and finality substitution tests are required before route exposure.

## Release gate

The route must not return a successful proof until all of these land together:

1. Consensus-authenticated `contract_state_root` computed by every validator
   from the actual post-block World and checked on replay.
2. Atomic retained-map publication and recovery, including untouched values,
   exact predecessor identity and a bounded proof read for the committed head.
3. The proof-mode Torii response bound to the same State snapshot and Kura V2
   finality artifact, with a client trust anchor.

The MerkleMap membership and data-model value proofs can be developed and
tested independently, but they do not satisfy this release gate by themselves.
