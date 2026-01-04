<!--
SPDX-License-Identifier: Apache-2.0
-->

# Bridge finality proofs

This document describes the initial bridge finality proof surface for Iroha.
The goal is to let external chains or light clients verify that an Iroha block
is finalized without off‑chain computation or trusted relays.

## Proof format

`BridgeFinalityProof` (Norito/JSON) contains:

- `height`: block height.
- `chain_id`: Iroha chain identifier to prevent cross-chain replay.
- `block_header`: canonical `BlockHeader`.
- `block_hash`: hash of the header (clients recompute to validate).
- `commit_certificate`: validator set + signatures that finalized the block.

The proof is self‑contained; no external manifests or opaque blobs are required.
Retention: Torii serves finality proofs for the recent commit-certificate window
(bounded by the configured history cap; defaults to 512 entries via
`sumeragi.commit_cert_history_cap` / `SUMERAGI_COMMIT_CERT_HISTORY_CAP`). Clients
should cache or anchor proofs if they need longer horizons.
The canonical tuple is `(block_header, block_hash, commit_certificate)`: the
hash of the header must match the hash inside the commit certificate, and the
chain id binds the proof to a single ledger. Servers reject and log a
`CommitCertificateHashMismatch` when the certificate points to a different block
hash.

## Commitment bundle

`BridgeFinalityBundle` (Norito/JSON) extends the basic proof with an explicit
commitment and justification:

- `commitment`: `{ chain_id, authority_set { id, validator_set, validator_set_hash, validator_set_hash_version }, block_height, block_hash, mmr_root?, mmr_leaf_index?, mmr_peaks?, next_authority_set? }`
- `justification`: signatures from the authority set over the commitment
  payload (reuses the commit-certificate signatures).
- `block_header`, `commit_certificate`: same as the basic proof.

Current placeholder: `mmr_root`/`mmr_peaks` are derived by recomputing a
block-hash MMR in memory; inclusion proofs are not yet returned. Clients can
still verify the same hash via the commitment payload today.

API: `GET /v1/bridge/finality/bundle/{height}` (Norito/JSON).

Verification is analogous to the basic proof: recompute `block_hash` from the
header, verify the commit-certificate signatures, and check the commitment
fields match the certificate and block hash. The bundle adds a commitment/
justification wrapper for bridge protocols that prefer the separation.

## Verification steps

1. Recompute `block_hash` from `block_header`; reject on mismatch.
2. Check `commit_certificate.block_hash` matches the recomputed `block_hash`;
   reject mismatched header/QC pairs.
3. Check `chain_id` matches the expected Iroha chain.
4. Recompute `validator_set_hash` from `commit_certificate.validator_set` and
   check it matches the recorded hash/version.
5. Verify signatures in the commit certificate against the header hash using
   the referenced validator public keys and indices; enforce quorum
   (`2f+1` when `n>3`, else `n`) and reject duplicate/out‑of‑range indices.
6. Optionally bind to a trusted checkpoint by comparing the validator set hash
   to an anchored value (weak‑subjectivity anchor).
7. Optionally bind to an expected epoch anchor so proofs from older/newer
   epochs are rejected until the anchor is rotated intentionally.

`BridgeFinalityVerifier` (in `iroha_data_model::bridge`) applies these checks,
rejecting chain-id/height drift, validator-set hash/version mismatches,
duplicate/out-of-range signers, invalid signatures, and unexpected epochs before
counting quorum so light clients can reuse a single verifier.

## Reference verifier

`BridgeFinalityVerifier` accepts an expected `chain_id` plus optional trusted
validator-set and epoch anchors. It enforces the header/block-hash/
commit-certificate tuple, validates validator-set hash/version, checks
signatures/quorum against the advertised validator roster, and tracks the latest
height to reject stale/skipped proofs. When anchors are supplied it rejects
replays across epochs/rosters with explicit `UnexpectedEpoch`/
`UnexpectedValidatorSet` errors; without anchors it adopts the first proof's
validator-set hash and epoch before continuing to enforce duplicate/out-of-
range/insufficient signatures with deterministic errors.

## API surface

- `GET /v1/bridge/finality/{height}` – returns `BridgeFinalityProof` for the
  requested block height. Content negotiation via `Accept` supports Norito or
  JSON.
- `GET /v1/bridge/finality/bundle/{height}` – returns `BridgeFinalityBundle`
  (commitment + justification + header/certificate) for the requested height.

## Notes and follow‑ups

- Proofs are currently derived from stored commit certificates. The bounded
  history follows the commit certificate retention window; clients should cache
  anchor proofs if they need longer horizons. Requests outside the window return
  `CommitCertificateNotFound(height)`; surface the error and fall back to an
  anchored checkpoint.
- A replayed or forged proof with mismatched `block_hash` (header vs.
  certificate) is rejected with `CommitCertificateHashMismatch`; clients should
  perform the same tuple check before signature verification and discard
  mismatched payloads.
- Future work can add MMR/authority‑set commitment chains to reduce proof size
  the commit certificate inside richer commitment envelopes.
