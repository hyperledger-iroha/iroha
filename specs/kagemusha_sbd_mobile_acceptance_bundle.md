# Kagemusha SBD mobile acceptance bundle

The ignored production bridge test can emit a deterministic, owner-private
cryptographic acceptance bundle for the Android and iOS restart/no-network
suites. Its proof generation, finality verification, native recursion, split,
and acknowledgement execution are genuine. The release authorities and review
artifacts in this deterministic test bundle are self-issued test fixtures,
however; the bundle is not shipping-release provenance, a physical-device
benchmark, or KeyMint/Secure Enclave qualification evidence. The bundle uses
the current Taira chain
`fc56984b-2be7-431d-840e-21514d1883f0` and the exact runtime asset alias
`sbd#cbsi`, whose typed asset definition ID is
`7ZepsJTHCVLKsrFFNZGSRGZgvBhv`. Neither identity may be substituted.

This degree-20 workflow is a production job, not an ordinary Rust test. Never
invoke the ignored test directly: its Eq/Ep Halo2 construction can consume
workstation-scale memory before a late layout failure. The former `--report`
launcher enforced process-tree RSS while treating Darwin physical footprint as
diagnostic; it allowed a 133.653 GiB footprint to exhaust a 128 GiB host. That
launcher now fails closed. The acceptance bundle must not be regenerated until
its generic command path uses the same process-group-scoped 250 ms
`max(RSS, physical footprint)` enforcement as the strict candidate generator.

Production candidate generation remains available only through
`run_kagemusha_v4_generation.py --resource-report ...` with a prebuilt
`kagemusha_recursive_spend_v4_bundle` executable and the exact
`generate-candidate` subcommand. Its supervisor stops and remeasures only its
owned process group before termination, and it refuses evidence publication
after a memory or host-headroom stop.

Do not raise the limit when the guarded workflow stops. Treat that result as a
circuit-layout release blocker, keep the output unpublished, and use the
receipt's last stage and peak-memory fields for the next optimization pass.

The generator installs the authenticated eight-artifact recursive release,
builds a genuine SBD shield proof, authenticates its finalized anchor with the
manifest-selected BLS roster and Commit QC, initializes recursive cash, splits
`1000` atomic units into a `700`-unit receiver payment and `300`-unit sender
change, terminally verifies both branches, and creates and verifies the
receiver P-256 acknowledgement. The output directory and all children are
created with owner-only permissions on Unix. Generation fails when the chosen
directory already contains a file, preventing partial or mixed releases.

## Root manifest

`bundle-v1.json` has schema
`iroha.kagemusha-sbd-mobile-acceptance-bundle` and
`bundle_version: 1`. It records:

- `bridge_abi_version: u32`, `wire_version: 4`, `chain_id` equal to the
  current Taira UUID, `canonical_sbd_asset_definition_id` equal to the typed
  `sbd#cbsi` ID, and `asset_scale: 2`;
- `evaluated_block_height`, `evaluated_block_hash_hex`,
  `acceptance_time_ms`, `sender_account_id`, `recipient_account_id`, and
  `receiver_device_id`;
- `artifact_set`, copied from the validated readiness projection with
  `generation`, the manifest/policy/attestation SHA-256 strings, activation and
  withdrawal heights, proof-size bound, and asset scale;
- `active_transfer_verifier`, copied from the same projection with
  `backend`, `name`, `version`, `circuit_id`, `commitment`,
  `public_inputs_schema_hash`, proof-size bound, and activation/withdrawal
  heights;
- `amounts_atomic`, whose decimal-string fields are `topup`,
  `reference_request`, `fresh_request`, and `sender_change`;
- `requests`, containing both request IDs and digests as lowercase hex plus
  the issued and expiry times;
- `sender_seed` with `wallet_state_version: 9`, `status: "READY"`, and paths
  to the init result, append-local request, and fresh receiver request;
- `receiver_seed` with `wallet_state_version: 9`,
  `status: "REQUEST_PREPARED"`, and paths to the fresh request, receiver note
  opening, and test-only P-256 private scalar; and
- sorted `files` entries with exact fields
  `{kind,use,path,size,sha256,secret}`. `size` is an unsigned byte count,
  `sha256` is lowercase hex, and `use` is either `input` or `assertion`.

`bundle-v1.sha256` authenticates the exact `bundle-v1.json` bytes. A loader
must verify that sidecar first, reject an unknown schema/version, reject a
missing or duplicate kind/path, verify every listed size and SHA-256, and never
use an entry marked `use: "assertion"` to seed wallet state. Files marked
`secret: true` must remain under device test-key protection. The two root
metadata files are the inventory root and therefore are not self-listed.

## Required input inventory

| Kind | Path | Secret |
|---|---|---:|
| `release_manifest_v4` | `release/manifest-v4.norito` | no |
| `release_policy_v1` | `release/policy-v1.norito` | no |
| `release_attestation_v4` | `release/attestation-v4.norito` | no |
| `physical_device_benchmark_evidence` | `release/benchmark-evidence.bin` | no |
| `cryptographic_review_v4` | `release/cryptographic-review-v4.norito` | no |
| `promotion_record_v4` | `release/promotion-record-v4.norito` | no |
| `topup_finality_roster_v2` | `release/topup-finality-roster-v2.norito` | no |
| `offline_readiness_v4` | `release/offline-readiness-v4.norito` | no |
| `recursive_step_eq_params_ipa_v4` | `release/artifacts/<manifest Eq params file_name>` | no |
| `recursive_step_eq_proving_key_v4` | `release/artifacts/<manifest Eq proving-key file_name>` | no |
| `recursive_step_eq_verifying_key_v4` | `release/artifacts/<manifest Eq verifying-key file_name>` | no |
| `recursive_step_eq_bootstrap_witness_v4` | `release/artifacts/<manifest Eq bootstrap file_name>` | no |
| `recursive_step_ep_params_ipa_v4` | `release/artifacts/<manifest Ep params file_name>` | no |
| `recursive_step_ep_proving_key_v4` | `release/artifacts/<manifest Ep proving-key file_name>` | no |
| `recursive_step_ep_verifying_key_v4` | `release/artifacts/<manifest Ep verifying-key file_name>` | no |
| `recursive_step_ep_bootstrap_witness_v4` | `release/artifacts/<manifest Ep bootstrap file_name>` | no |
| `recipient_receive_offer_v2` | `receiver/receive-offer-v2.norito` | no |
| `recipient_reference_request_v2` | `receiver/reference-request-v2.norito` | no |
| `recipient_fresh_request_v2` | `receiver/fresh-request-v2.norito` | no |
| `recipient_registration_lineage_v2` | `receiver/registration-lineage-v2.norito` | no |
| `checkpoint_publisher_envelope_v1` | `receiver/checkpoint-envelope-v1.json` | no |
| `checkpoint_publisher_public_key_ed25519` | `receiver/checkpoint-publisher-public-key-ed25519.bin` | no |
| `trusted_checkpoint_v2` | `receiver/trusted-checkpoint-v2.bin` | no |
| `receiver_device_private_scalar_p256` | `receiver/device-private-scalar-p256.bin` | yes |
| `receiver_device_public_key_sec1_uncompressed` | `receiver/device-public-key-sec1-uncompressed.bin` | no |
| `receiver_fresh_note_opening_v2` | `receiver/fresh-note-opening-v2.norito` | yes |
| `artifact_binding_v4` | `sender/artifact-binding-v4.norito` | no |
| `sender_note_opening_v2` | `sender/topup-note-opening-v2.norito` | yes |
| `topup_zero_frontier_v4` | `sender/topup-zero-frontier-v4.norito` | yes |
| `topup_output_membership_v4` | `sender/topup-output-membership-v4.norito` | yes |
| `topup_unsigned_v4` | `sender/topup-unsigned-v4.norito` | no |
| `topup_anchor_v4` | `sender/topup-anchor-v4.norito` | no |
| `topup_finality_proof_v2` | `sender/topup-finality-proof-v2.norito` | no |
| `recursive_init_local_v4` | `sender/init-local-v4.norito` | yes |
| `recursive_init_result_v4` | `sender/init-result-v4.norito` | no |
| `recursive_init_bundle_v4` | `sender/init-bundle-v4.norito` | no |
| `recursive_init_membership_witness_v2` | `sender/init-membership-witness-v2.norito` | yes |
| `recursive_init_topup_provenance_v4` | `sender/init-topup-provenance-v4.norito` | no |
| `sender_change_opening_v2` | `sender/change-note-opening-v2.norito` | yes |
| `recursive_append_output_membership_v4` | `sender/append-output-membership-v4.norito` | yes |
| `recursive_append_local_v4` | `sender/append-local-v4.norito` | yes |

The host acceptance suites persist the exact init result and branch material,
destroy and recreate the sender and receiver storage/lifecycle objects, reload
wallet-state version 9, and invoke the production append/verify boundaries with
network access trapped. They prove durable encrypted archive recovery and the
complete peer-payment state machine. Actual operating-system process death,
physical Android KeyMint, and physical iOS Secure Enclave behavior remain
separate on-device release qualifications and must not be inferred from this
deterministic host bundle.

## Assertion-only inventory

The following outputs are reference assertions, never state seeds:

| Kind | Path |
|---|---|
| `recursive_verify_local_v4` | `expected/verify-local-v4.norito` |
| `recursive_split_result_v4` | `expected/split-result-v4.norito` |
| `recursive_peer_payment_v4` | `expected/peer-payment-v4.norito` |
| `recursive_verify_result_v4` | `expected/verify-result-v4.norito` |
| `receiver_acknowledgement_payload_v2` | `expected/acknowledgement-payload-v2.norito` |
| `receiver_acknowledgement_signature_raw_p256` | `expected/acknowledgement-signature-raw-p256.bin` |
| `receiver_acknowledgement_v2` | `expected/acknowledgement-v2.norito` |
| `receiver_acknowledgement_verify_result_v2` | `expected/acknowledgement-verify-result-v2.norito` |

The Rust fixture's P-256 acknowledgement signature is deterministic, but a
platform signer is not required to reproduce those signature bytes. Apple
Security and CryptoKit may randomize valid ECDSA signatures. Mobile acceptance
therefore compares the acknowledgement payload and verified semantic bindings,
requires native signature verification, and requires byte-exact replay of the
first locally persisted acknowledgement after restart. It must not inject the
assertion-only signature or acknowledgement as signer output.

The reference request is `625` scale-2 atomic units and expires at
`1900000060000`. The fresh request is independently signed for `700` scale-2
atomic units, has a different request ID and digest, and expires at
`1900000300000`. Tests must assert those inequalities before using the
request-independent receiver lineage.
