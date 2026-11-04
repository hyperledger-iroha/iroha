# Endorsed runtime-upgrade frame capture

`endorsed_runtime_upgrade_identity_frames.json` pins the current complete frames
of `ProposeEndorsedRuntimeUpgradeProposal`, a release addition introduced after
the original Norito identity cutover. These are **current-source fixtures**, not
observations of the pre-cutover codec. The three original frame-owner observation
files and their hashes remain unchanged.

The fixture contains nine frames: scheduling with no endorsement, scheduling with
a genuine deterministic Ed25519 endorsement, cancellation, each corresponding
`Some` frame, `None`, an empty vector, and a vector containing all three proposals.
The seed is public test material. The scheduling value establishes codec coverage;
it does not assert that a root would authorize or activate that change set.

The shared frame checker validates the nominal identity, both directional schema
hashes, header flags, alignment, length, complete frame and bare bytes. It decodes
and re-encodes through canonical, archive and bare paths, compares the complete
value, and rejects wrong-schema, truncated and trailing-byte inputs. A separate
test rejects baseline proposal frames as endorsed proposals and vice versa.

Captured on 2026-11-04 with Rust 1.99.1 (`58cf8f9c3`), default model features, in
the working tree based on `abb574c16c`. The tree contained other uncommitted work;
this is a local regression fixture, not authenticated release or network evidence.

```sh
cargo iroha-fast --stable-local-metadata --incremental -- test --locked \
  -p iroha_data_model --test manual_frame_identity \
  runtime_upgrade::capture_endorsed_runtime_upgrade_frames \
  -- --exact --ignored --nocapture
```

The capture test passed. The single
`ENDORSED_RUNTIME_UPGRADE_IDENTITY_FRAMES=` JSON record was parsed and indented
without changing its values. Rebuilding the regression with the populated
fixture and rerunning its ignored capture reproduced the exact JSON values.
That rebuilt producer's SHA-256 was
`3629c0fdba941edd4b64fa379b2ae28d4df900b687ecdd13788d2531902dd4eb`.

| Input or fixture | SHA-256 at capture |
| --- | --- |
| `tests/manual_frame_identity/runtime_upgrade.rs` | `006f75470faf70f5f814a76e5a11c84563ef760309502d6d6e9cd53807e27377` |
| `tests/manual_frame_identity/support.rs` | `838cbebebb307efd81ee7fe1f89f4b9beb1d41572ce9bd760e867b882b4d9334` |
| `src/isi/governance.rs` | `b330320e99c9b1c1b69ced3264e1274494ee236efad0d8b08e9795046941d3c7` |
| `endorsed_runtime_upgrade_identity_frames.json` | `99d50ea467c806f616850a88d73769dda8dc0e77c63f30eab4e971a1c1ab798d` |

Run the maintained regression with:

```sh
cargo iroha-fast --stable-local-metadata --incremental -- test --locked \
  -p iroha_data_model --test manual_frame_identity runtime_upgrade::
```
