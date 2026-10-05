# Ledger State Finality

The first-release ledger state endpoints expose only state authenticated by
native Sumeragi finality:

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
World or State root and proves no untouched value. The complete World roots of
the result are `parent_world_state_root` and `world_state_root`
([`sumeragi.md`](sumeragi.md), Appendix E, E51); this response does not return
them. The keyed State root with inclusion, absence and range witnesses is
specified in `sumeragi.md` §16 and not built.

Missing finality returns no successful response. Malformed, forged,
wrong-height, or wrong-block evidence fails closed. The block result Merkle
root and retired QC projections are not state-root or state-proof authorities.
The retired `world.commit_qcs` snapshot field and tiered segment are not part of
the first-release schema; decoders reject either name as unknown rather than
defaulting, redacting, or migrating it.
