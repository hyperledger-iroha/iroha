# Android Attestation Reference Bundles

This directory contains deterministic mock attestation bundles that exercise the
`android_keystore_attestation` harness with non-Google trust anchors. Each
subdirectory mirrors the documented bundle layout (`chain.pem`, `challenge.hex`,
`alias.txt`, `trust_root_<vendor>.pem`, `notes.md`). The certificates were
minted with `scripts/android_mock_attestation_der.py` so the harness and CI
pipelines can rehearse Huawei/AOSP-style deployments without depending on
physical hardware.

| Directory | Vendor | Notes |
|-----------|--------|-------|
| `mock_huawei/` | Mock Huawei StrongBox attestation | Includes `trust_root_huawei.pem`, `trust_root_bundle_huawei.zip`, and challenge `A1B2…`. |
| `mock_osp/` | Mock OSP/KeyMint attestation | Includes `trust_root_osp.pem`, `trust_root_bundle_osp.zip`, and challenge `F1E2…`. |

Matching ZIP archives of the trust roots live alongside each bundle and are also
mirrored under `fixtures/android/trust_roots/` (named `trust_root_bundle_huawei.zip`
and `trust_root_bundle_osp.zip`). The attestation harness automatically loads
`trust_root_*.pem`/`trust_root_bundle_*.zip` files that are colocated with a bundle,
so the fixtures can be verified by pointing `--bundle-dir` at the directory with no
additional root flags. Directories supplied via `--trust-root-dir` now unpack the
same `trust_root_bundle_*.zip` archives, allowing CI and local rehearsals to reuse
the shared packs without duplicating PEM files.
