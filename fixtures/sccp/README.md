# SCCP fixtures

Shared SCCP v1 vectors (`specs/sccp.md` revision 3, §11). They are generated
by Rust or by the contracts themselves and consumed by the contract suites
(EDR, Acton), the node and the SDKs.

Conventions of the Rust-generated files: byte strings are `0x`-prefixed
lowercase hex; integers that can reach `2^53` or more (`u128` amounts,
`ton_amount_bound`) are decimal strings and every other integer is a JSON
number; rejected inputs name the `iroha_sccp::v1` error variant that rejects
them (for example `TrailingBytes`, `BadS` or `BadOffset`).

## Rust-generated v1 vectors

`crates/iroha_sccp/tests/v1_vectors.rs` regenerates these files from
`iroha_sccp::v1` and fails when a committed file differs. Each generator also
runs the crate's verifiers over its own vectors (Merkle and history paths,
signature sets, roster rotation, message and control proof bundles), so every
committed vector is accepted or rejected by the Rust implementation exactly as
recorded.

| File | Content |
|---|---|
| `payload_v1.json` | The five profiles (tag, domain, identity word, `network_bytes`, route id, account codec); payloads for all 8 directions with `deadline_ms`, payload hashes, lane bytes and message ids (§3.2, §3.3); `taira_units` conversions and rejected amounts (§0); rejected payload encodings covering every §3.1/§3.2 rule |
| `commitment_tree_v1.json` | 512 mixed transfer/control leaves (`leaf_rule`), worked transfer, control and node preimages, roots and every path for counts 1..=17 and 512 including promoted nodes, and confusion negatives (internal node as leaf, transfer leaf at a control position, flipped pause, count and path mismatches) |
| `control_v1.json` | The two §3.4 example control leaves; control leaves for every target network, two revisions, nonces 1, 2 and 2^32 and both pause values with their 126-byte preimages; rejected control fields; the `SccpControlApplied` topic |
| `history_v1.json` | 40 history leaves (`leaf_rule`), and for sizes 1..=40 the root, the right-bagged peaks and every path (§3.5) |
| `eip712_v1.json` | Domain separator for the fixed `NetworkId`; attestation, rotation attestation and key-PoP struct hashes and digests; four fixed bridge keys with low-S signatures; high-S, `s = HALF_N + 1`, `v ∉ {27, 28}`, `r`/`s` range negatives; a signature checked against another digest; the forced recovery-id ≥ 2 re-sign with fixed extra entropy (§3.6, §3.8) |
| `roster_v1.json` | Roster preimages and digests for n = 4, 7 and 31 with zero slots; `t` for every n in 4..=31; ordering, size, threshold, generation and packing negatives; the §5.1.5 validity bounds at every boundary |
| `evm_calldata_v1.json` | A coherent scenario (rosters g7 and g8, attestations of heights 100, 150 (rotation) and 200, signature sets, block and history roots) and the golden calldata of `finalizeFromTaira`, `finalizeFromTairaHistorical`, `rotateRosters`, `applyControl`, `applyControlHistorical`, `voidExpired`, `voidExpiredHistorical`, `voidFrozen` and `transferToTaira`; every §5.2.2 selector and event topic; the view calls and the `rosterState()` return; `SccpTransferToTaira` and `SccpVoided` logs; non-canonical `transferToTaira` calldata negatives (offset, padding, trailing bytes, truncation, length and amount bounds) |

Common inputs: the Taira `NetworkId` is `0x11…11` (the §3.4 example value),
the base time is `1 800 000 000 000` ms and outbound deadlines are one day
later, and fixture bridge key `i` has secret
`keccak256("SCCP/FIXTURE/KEY/V1" ‖ u8 i)`. Codec-3 (`taira_account`) fields
are opaque byte strings; contracts check only their length (§3.1). The EVM
destination word is `word(0x22…22)`.

Regenerate after a reviewed layout change, then review the diff:

```text
scripts/cargo_fast.sh --target-slot sccp --stable-local-metadata --incremental -- \
  test -p iroha_sccp --test v1_vectors -- --ignored regenerate_v1_vectors
```

`governance_v1.json` and `ton_bodies_v1.json` of the §11 table are added with
the governance payload and TON message-body code that generate them.

## Contract-generated vectors

`ton_stateinit_v1.json` is emitted by the Tolk contracts
(`contracts/ton/sccp/scripts/stateinit-golden.tolk`) and cross-checked by
`scripts/generate_ton_sccp_stateinit_golden.py` (§5.3.1). It pins the
canonical minter initial data, the minter, bucket and wallet addresses and
the code cell hashes and depths. The Rust `iroha_sccp::v1::ton_cell` and
`iroha_sccp::v1::roster` unit tests pin the same values.

## Captured RPC responses

`rpc/` holds captured public-RPC responses; see `rpc/transport/README.md`.

## Retired layouts

`native_transfer_event_v1.json`, `replay_forest_v1.json` and
`ton_stateinit_golden_v1.json` describe the retired SCCP layouts (Groth16
proofs, the replay archive and the previous TON contracts). They are removed
or regenerated together with the code that still reads them (§10, §11).
