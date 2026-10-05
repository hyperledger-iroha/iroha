# Contract-state exact-value proof V1

This is a follow-on capability. The current
`ExecutionCommitment.post_state_root` covers a block's execution witness writes
(or reads for a read-only block), not every current World value. It must never
be used to prove an arbitrary `/v1/contracts/state` value. The existing
`/v1/ledger/state-proof/{height}` response authenticates that execution root
only; its route name does not turn it into an accumulated state proof.

## Commitment

A contract-state value proof is a witness against the one keyed State root of
[`sumeragi.md`](sumeragi.md) §16, over the canonical table
`world.smart_contract_state`. No separate `contract_state_root` field is added
to the execution commitment, and no per-table root is independently
authoritative (§16.5 P1, §16.8). The keyed root is specified and not built:
ZK delivery plan task G.3 builds it, and until then no contract-state value
proof is consensus-authenticated.

- The key is the bare Norito V1 payload of the exact physical `StatePath`; the
  value is the unmodified stored bytes (§16.2). A contract-scoped query proves
  `sc/{address_digest}/` plus its logical path, not the logical path alone.
- An inclusion witness proves the exact stored bytes, an absence witness proves
  that the path has no value, and a complete-range witness over the
  `sc/{address_digest}/` prefix interval proves every value of one contract
  (§16.3). A zero-length stored value is present and provable.
- The root is `keyed_state_root` of the certified result of the proven height,
  or a committed root-history anchor (§16.7). Validator replay recomputes it
  before signing or accepting the CommitQC.

### As-built local map (not a consensus commitment)

`ContractStateMapV1` (`iroha_data_model::contract_state_proof`) commits the
complete current `smart_contract_state` storage with `iroha_crypto::MerkleMap`:

```
key_hash   = H("iroha:contract-state:key:v1\0" || UTF8(StatePath))
value_hash = H("iroha:contract-state:value:v1\0" || stored_bytes)
leaf       = H("iroha:merkle-map:leaf:v1\0" || key_hash || value_hash)
root       = MerkleMap.root()  // includes the exact leaf count
```

`H` is Iroha's Blake2b-256 `Hash::new`, including its low-bit marker. The
compressed MerkleMap path uses MSB-first split bits. The node derives this map
locally for one retail policy query
(`crates/iroha_core/src/state/retail_contract_state_snapshot.rs`). Its root is
not certified, published or admitted, and it has no absence proof. The keyed
State root replaces it: G.3 deletes the map or keeps it only as a test
differential (open defect G1-D7 of
[`state_table_inventory.json`](state_table_inventory.json)).

## Proof response and trust

The first route is `GET /v1/contracts/state?contract_address=...&path=...&proof=true`.
Proof mode accepts one exact `path`, a canonical contract address, raw value
inclusion, and no `paths`, `prefix`, `decode`, alias, or pagination options.
The returned proof includes the canonical physical key, raw bytes, the keyed
State witness bytes, one-based committed height, the exact keyed State root,
block hash/header, and the native finality proof of that height. A missing
value, unavailable committed State version, missing/malformed finality, or a
root/height/hash mismatch fails closed.

The verifier first checks the requested address and exact physical path, then
verifies inclusion against the keyed State root. It requires that root to
equal `keyed_state_root` in the certified result of the proof's CommitQC. It
validates the header, block hash and height against the finality proof, and
verifies finality using an externally trusted height-context anchor and linked
successors, as in `BridgeFinalityVerifier`. The proof's self-described
committee cannot serve as its own trust anchor. A client that only calls
`verifyContractStateValueInclusionV1` must supply a separately authenticated
root; that helper alone does not establish finality.

The admission and resource caps are one key, 1 MiB raw value, the committed
witness byte and work caps (§16.4 K9), and a bounded canonical Norito response.
JSON and Norito encode the same closed fields. Exact-key, value, witness, root,
height, block, network, context, and finality substitution tests are required
before route exposure.

## Completion

The route returns no successful proof until all of these exist together:

1. The keyed State root of `sumeragi.md` §16 in the certified execution
   result, computed by every validator from the actual State and checked on
   replay.
2. Atomic publication and recovery of the commitment with the State view
   (§16.5), including untouched values, and a bounded witness read for the
   committed head.
3. The proof-mode Torii response bound to the same State view and native
   finality proof, with a client trust anchor.

The witness and data-model value verifiers can be developed and tested
independently, but they do not satisfy these conditions by themselves.
