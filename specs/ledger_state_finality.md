# Ledger State Finality

The two public ledger finality reads expose state authenticated by native Sumeragi finality:

- `GET /v1/ledger/state/{height}`
- `GET /v1/ledger/state-proof/{height}`

Both endpoints return the same closed `StateFinalityResponse` in JSON or
Norito (`crates/iroha_torii/src/ledger_state_finality.rs`). It contains exactly
`height`, `block_hash`, `witnessed_post_state_root`, `block_header`, and
`finality_proof`. The height must name a certified non-genesis execution
(at least 2): a signed genesis body does not authenticate its own result.

Torii takes one immutable State view, loads the block hash from State's
committed hash journal, and builds the `SumeragiFinalityProof` of that height
with `iroha_core::sumeragi::finality::build_proof`, which verifies the durable
Kura frames, the complete authenticated committee prefix and the application
attestations. Torii additionally requires the requested height, the State block
hash and the proof's header to agree.

The returned `witnessed_post_state_root` is always
`ExecutionCommitment.post_state_root` of the certified result in that proof:
the post-state root of the block's witnessed write set. It is not a complete
World or State root and proves no untouched value. The certified execution
result also carries `parent_world_state_root` and `world_state_root`.
The governed commitment profile determines how to interpret those two raw
digests. A reader of the GET response obtains them from the included finality
proof. Proofs for individual keys use the signed POST below.

Missing finality returns no successful response. Malformed, forged,
wrong-height, or wrong-block evidence fails closed. The block result Merkle
root and retired QC projections are not state-root or state-proof authorities.
The retired `world.commit_qcs` snapshot field and tiered segment are not part of
the first-release schema; decoders reject either name as unknown rather than
defaulting, redacting, or migrating it.

## Selected State rows

`POST /v1/ledger/state-proof/{height}` accepts a network-bound canonical
account signature over the exact request target and JSON body. The height
must equal the current certified non-genesis Global tip. The request names
one to 32 distinct rows in ascending order by registry table name and bare
Norito key bytes. Each key uses lowercase hexadecimal. Cells use the empty key.

```json
{"rows":[{"table":"world.parameters","key_hex":""}]}
```

The reader needs `CanReadAllLedgerData` or a direct `CanReadStateRow`
grant for every requested position. Torii checks current grants before
source capture and again at egress. The complete batch succeeds or fails
together. A permission refusal reveals no row presence or absence. The old
`CanReadCertifiedOriginals` scope alone grants no access to generic row reads.

The portable `KeyedStateRowsV1` response carries the exact height, header
hash, schema digest and ordered positions. Each row carries original value
bytes or a witnessed absence and its own lookup proof. Verify the response
against independently authenticated finality, the original request and a
`SelectedStateSchemaV1` resolved from the selected immutable storage definitions.
Pin its digest independently. The verifier rejects undeclared tables and nonempty
cell keys even when a lookup witness for those bytes would match the root. Only
the governed keyed commitment can supply these witnesses. A root running LtHash,
an unavailable execution version or a stale height cannot serve this evidence.
Existing full-World snapshot projections require the LtHash profile. On a
keyed root, clients read individual rows through the signed POST. The request
body is bounded to 64 KiB, and the
response to the smaller of 32 MiB and the configured Torii response limit.

The lookup path reveals branch positions and sibling summaries. An absence
witness can also carry an adjacent leaf's key and value digests. These
deterministic digests can allow guessing low-entropy entries when the reader
has enough information about a neighboring subtree. The response omits
unrelated raw values and the full list of World hashes.

### HTTP comparison

The local four-validator Ironwood fixture measures the two signed reads needed
for first payment preparation, including request signing, Torii handling,
decoding and verification against an independently verified block. With one
warmup and seven measured iterations, the median was **45.981 ms** for both
reads together and the two responses totaled **5,144 bytes**. The full-World
reader at revision `b4525a5f6c` measured **75.486 ms** and **133,245 bytes**
in the same run. After the old endpoint was removed, the keyed reader measured
**41.915 ms** at the same response size. The fixture uses in-process HTTP on
this Mac. Remote network round-trip latency is additional.

To repeat the keyed measurement, run
`cargo iroha-fast -- test -p iroha_torii --lib measure_ironwood_payment_context_http -- --ignored --nocapture --test-threads=1`.

The paired Core fixture also times complete signed-block execution, certification
and publication, using 20 measured blocks per profile and reversing the run order.
With 1,024 existing 128-byte smart-contract rows, three runs measured median block
times of 334.799, 257.038 and 235.010 ms for LtHash, versus 255.552, 252.292 and
229.366 ms for keyed State, respectively. With no additional contract rows, a separate
comparison measured 286.274 ms and 280.644 ms. Timing varies with host
load. Reproduce the populated run with
`IROHA_STATE_TIMING_ROWS=1024 cargo iroha-fast -- test -p iroha_core --lib certified_ironwood_state_publication_and_read_timing -- --ignored --nocapture --test-threads=1`.
The additional memory retained by the radix index is measured in the
[commitment benchmark](zk_delivery_plan.md#g3-selected-state-commitment-and-publication).
