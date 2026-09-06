# Canonical Sumeragi Norito fixtures

`sumeragi_v4_commit_vote_signature_payload.hex` is the exact
`VoteSignaturePayload::encode()` portion extracted from the Rust-generated
`message/commit_vote_reproposal` row in
`fixtures/sumeragi_v2/wire_v2.tsv`. That upstream fixture is emitted by
`iroha_data_model::block::consensus_v2` using `Encode::encode` with bare Norito
V1 `COMPACT_LEN`; it is not reconstructed by the Go codec under test.

`sumeragi_v4_bls_normal_qc.json` is emitted from
`iroha_sccp::sccp_exact_outbound_test_fixture_v1`. It contains the exact Rust
`Vote::signature_preimage()`, ordered BLS-normal roster, roster-aligned proofs
of possession, signer indices, and aggregate CommitQC signature. The Go test
uses these Rust-owned values directly for in-circuit hash-to-curve, PoP, and
aggregate-pairing verification; no Go key generator participates in that KAT.

The exact Rust-emitted `BlockHeaderConsensusProjectionV1` differential fixture
is not yet checked in. The Go circuit currently reconstructs the authoritative
Rust field order and bare-Norito option shapes, but its native encoder is not
independent evidence. Production therefore remains fail-closed on
`rust_generated_block_header_consensus_projection_differential_fixture` until
the Core encoder is stable and emits the byte/hash fixture consumed here.
