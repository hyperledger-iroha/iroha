# Ledger State Finality

The first-release ledger state endpoints expose only Sumeragi-v2 authenticated
state:

- `GET /v1/ledger/state/{height}`
- `GET /v1/ledger/state-proof/{height}`

Both endpoints return the same closed `StateFinalityResponse` in JSON or
Norito. It contains exactly `height`, `block_hash`, `state_root`,
`block_header`, and `finality_artifact`.

Torii first loads the block through State's committed hash journal, then reads
the exact `V2FinalityArtifact` from Kura. Kura validates the durable artifact,
its canonical header and complete block-wire bindings, the frozen roster proofs
of possession, and CommitQC cryptography. Torii additionally requires the
requested height, State block hash and header, and artifact subject to agree.
The returned `state_root` is always
`finality_artifact.commit_qc.execution_commitment.post_state_root`.

Missing finality returns no successful response. Malformed, forged,
wrong-height, or wrong-block evidence fails closed. The block result Merkle
root and legacy QC projections are not state-root or state-proof authorities.
The retired `world.commit_qcs` snapshot field and tiered segment are not part of
the first-release schema; decoders reject either name as unknown rather than
defaulting, redacting, or migrating it.
