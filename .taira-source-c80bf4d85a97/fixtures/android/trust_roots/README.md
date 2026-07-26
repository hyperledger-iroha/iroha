# Mock Attestation Trust-Root Bundles

ZIP archives that pair with the mock attestation bundles under
`fixtures/android/attestation/`. Each archive contains one or more PEM encoded
roots so developers can test `--trust-root-bundle` handling without depending on
OEM-provided packs.

| File | Contents |
|------|----------|
| `trust_root_bundle_huawei.zip` | `trust_root_huawei.pem` (Mock Huawei StrongBox root). |
| `trust_root_bundle_osp.zip` | `trust_root_osp.pem` (Mock OSP/KeyMint root). |

> These ZIP archives are mirrored next to each attestation bundle under
> `fixtures/android/attestation/<vendor>/` so the harness can auto-detect them.
