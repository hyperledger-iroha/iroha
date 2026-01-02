---
lang: fr
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-11-02T18:54:59.610441+00:00"
translation_last_reviewed: 2026-01-01
---

# Fixtures d'echantillon CI SoraFS

Ce repertoire embarque des artefacts deterministes generes depuis le payload d'echantillon sous `fixtures/sorafs_manifest/ci_sample/`. Le bundle demontre le pipeline end-to-end de packaging et de signature SoraFS que les workflows CI executent.

## Inventaire des artefacts

| Fichier | Description |
|------|-------------|
| `payload.txt` | Payload source utilise par les scripts de fixture (echantillon texte brut). |
| `payload.car` | Archive CAR emise par `sorafs_cli car pack`. |
| `car_summary.json` | Resume genere par `car pack` capturant les digests de chunks et les metadonnees. |
| `chunk_plan.json` | JSON de plan de fetch decrivant les plages de chunks et les attentes des providers. |
| `manifest.to` | Manifest Norito produit par `sorafs_cli manifest build`. |
| `manifest.json` | Rendu lisible du manifest pour debug. |
| `proof.json` | Resume PoR emis par `sorafs_cli proof verify`. |
| `manifest.bundle.json` | Bundle de signature keyless genere par `sorafs_cli manifest sign`. |
| `manifest.sig` | Signature Ed25519 detachee correspondant au manifest. |
| `manifest.sign.summary.json` | Resume CLI emis pendant le signing (hashes, metadonnees du bundle). |
| `manifest.verify.summary.json` | Resume CLI de `manifest verify-signature`. |

Tous les digests references dans les release notes et la documentation proviennent de ces fichiers. Le workflow `ci/check_sorafs_cli_release.sh` regenere les memes artefacts et les compare aux versions commitees.

## Regeneration des fixtures

Lancez les commandes ci-dessous depuis la racine du repo pour regenerer le set de fixtures. Elles suivent les etapes du workflow `sorafs-cli-fixture`:

```bash
sorafs_cli car pack       --input fixtures/sorafs_manifest/ci_sample/payload.txt       --car-out fixtures/sorafs_manifest/ci_sample/payload.car       --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to       --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --car fixtures/sorafs_manifest/ci_sample/payload.car       --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig       --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)"       --issued-at 1700000000       > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b       > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

Si une etape produit des hashes differents, investiguez avant de mettre a jour les fixtures. Les workflows CI comptent sur un output deterministe pour detecter les regressions.

## Couverture future

A mesure que des profils chunker supplementaires et des formats de proof quittent la roadmap, leurs fixtures canoniques seront ajoutes sous ce repertoire (par exemple,
`sorafs.sf2@1.0.0` (voir `fixtures/sorafs_manifest/ci_sample_sf2/`) ou proofs streaming PDP). Chaque nouveau profil suivra la meme structure - payload, CAR,
plan, manifest, proofs et artefacts de signature - afin que l'automatisation downstream puisse comparer les releases sans scripting specifique.
