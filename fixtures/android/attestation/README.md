# Android Attestation Reference Bundles

This directory contains deterministic mock attestation bundles that exercise the
Kotlin-owned `iroha-attestation` command with non-Google trust anchors. Each
subdirectory mirrors the documented bundle layout (`chain.pem`, `challenge.hex`,
`alias.txt` and `trust_root_<vendor>.pem`). The certificates were
minted with `scripts/android_mock_attestation_der.py` so the harness and CI
pipelines can rehearse Huawei/AOSP-style deployments without depending on
physical hardware.

| Directory | Vendor | Notes |
|-----------|--------|-------|
| `mock_huawei/` | Mock Huawei StrongBox attestation | Includes `trust_root_huawei.pem`, `trust_root_bundle_huawei.zip`, and challenge `A1B2…`. |
| `mock_osp/` | Mock OSP/KeyMint attestation | Includes `trust_root_osp.pem`, `trust_root_bundle_osp.zip`, and challenge `F1E2…`. |

Matching ZIP archives of the trust roots live alongside each bundle and are also
mirrored under `fixtures/android/trust_roots/` (named `trust_root_bundle_huawei.zip`
and `trust_root_bundle_osp.zip`). The command never discovers trusted roots, aliases, challenges or identity
commitments from an untrusted evidence directory. Tests supply the fixture root
through an explicit `--trust-root`, `--trust-root-dir` or `--trust-root-bundle`
argument and pass the known challenge, alias SPKI commitment, governed revocation
snapshot hash and evaluation time separately. `--bundle-dir` selects evidence
only. Trusted root directories can contain nested certificate files and ZIPs;
ZIP members are read with bounds and are never extracted.

Run `cd kotlin && ./gradlew :tools:test --console=plain` for the canonical Java
consumer assertions against the Kotlin command. The repository launcher
`scripts/android_keystore_attestation.sh` builds and executes this same command.
Passing these mock fixtures does not qualify physical StrongBox hardware.
