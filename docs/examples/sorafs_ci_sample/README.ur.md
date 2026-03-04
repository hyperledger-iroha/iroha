---
lang: ur
direction: rtl
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-11-02T18:54:59.610441+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/sorafs_ci_sample/README.md کا اردو ترجمہ -->

# SoraFS CI نمونہ Fixtures

یہ ڈائریکٹری `fixtures/sorafs_manifest/ci_sample/` کے sample payload سے بنائے گئے deterministic artefacts پیکج کرتی ہے۔ یہ bundle SoraFS کی end-to-end packaging اور signing pipeline دکھاتا ہے جسے CI workflows چلاتے ہیں۔

## Artefact Inventory

| فائل | وضاحت |
|------|-------|
| `payload.txt` | fixture scripts کے لئے source payload (plain-text sample). |
| `payload.car` | `sorafs_cli car pack` سے نکلا CAR archive. |
| `car_summary.json` | `car pack` کا summary جو chunk digests اور metadata capture کرتا ہے۔ |
| `chunk_plan.json` | fetch-plan JSON جو chunk ranges اور providers expectations بیان کرتا ہے۔ |
| `manifest.to` | `sorafs_cli manifest build` سے بنایا گیا Norito manifest. |
| `manifest.json` | debugging کے لئے human-readable manifest render. |
| `proof.json` | `sorafs_cli proof verify` سے نکلا PoR summary. |
| `manifest.bundle.json` | `sorafs_cli manifest sign` سے generated keyless signature bundle. |
| `manifest.sig` | manifest کی corresponding detached Ed25519 signature. |
| `manifest.sign.summary.json` | signing کے دوران نکلنے والا CLI summary (hashes, bundle metadata). |
| `manifest.verify.summary.json` | `manifest verify-signature` کا CLI summary. |

release notes اور documentation میں درج تمام digests انہی فائلوں سے لئے گئے ہیں۔ `ci/check_sorafs_cli_release.sh` workflow انہی artefacts کو regenerate کر کے committed versions سے diff کرتا ہے۔

## Fixture Regeneration

repository root سے نیچے والے commands چلائیں تاکہ fixture set regenerate ہو۔ یہ `sorafs-cli-fixture` workflow کے steps کو mirror کرتے ہیں:

```bash
sorafs_cli car pack       --input fixtures/sorafs_manifest/ci_sample/payload.txt       --car-out fixtures/sorafs_manifest/ci_sample/payload.car       --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to       --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --car fixtures/sorafs_manifest/ci_sample/payload.car       --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig       --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)"       --issued-at 1700000000       > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b       > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

اگر کسی step سے مختلف hashes نکلیں تو fixtures اپڈیٹ کرنے سے پہلے تحقیق کریں۔ CI workflows deterministic output پر انحصار کرتے ہیں تاکہ regressions پکڑے جا سکیں۔

## Future Coverage

جب اضافی chunker profiles اور proof formats roadmap سے نکلیں گے، تو ان کے canonical fixtures اسی ڈائریکٹری میں شامل کئے جائیں گے (مثلا،
`sorafs.sf2@1.0.0` (دیکھیں `fixtures/sorafs_manifest/ci_sample_sf2/`) یا PDP streaming proofs). ہر نیا profile اسی structure کو follow کرے گا — payload, CAR,
plan, manifest, proofs, اور signature artefacts — تاکہ downstream automation بغیر custom scripting کے releases کو diff کر سکے۔

</div>
