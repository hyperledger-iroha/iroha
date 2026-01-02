---
lang: es
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-11-02T18:54:59.610441+00:00"
translation_last_reviewed: 2026-01-01
---

# Fixtures de ejemplo de CI SoraFS

Este directorio empaqueta artefactos deterministas generados desde el payload de ejemplo en `fixtures/sorafs_manifest/ci_sample/`. El bundle demuestra el pipeline end-to-end de empaquetado y firmado de SoraFS que ejercitan los workflows de CI.

## Inventario de artefactos

| Archivo | Descripcion |
|------|-------------|
| `payload.txt` | Payload fuente usado por los scripts de fixture (muestra de texto plano). |
| `payload.car` | Archivo CAR emitido por `sorafs_cli car pack`. |
| `car_summary.json` | Resumen generado por `car pack` que captura digests de chunks y metadatos. |
| `chunk_plan.json` | JSON de plan de fetch que describe rangos de chunks y expectativas de providers. |
| `manifest.to` | Manifest Norito producido por `sorafs_cli manifest build`. |
| `manifest.json` | Renderizado legible del manifest para debug. |
| `proof.json` | Resumen de PoR emitido por `sorafs_cli proof verify`. |
| `manifest.bundle.json` | Bundle de firma keyless generado por `sorafs_cli manifest sign`. |
| `manifest.sig` | Firma Ed25519 separada correspondiente al manifest. |
| `manifest.sign.summary.json` | Resumen de CLI emitido durante el signing (hashes, metadatos del bundle). |
| `manifest.verify.summary.json` | Resumen de CLI de `manifest verify-signature`. |

Todos los digests referenciados en notas de release y documentacion salen de estos archivos. El workflow `ci/check_sorafs_cli_release.sh` regenera los mismos artefactos y los compara con las versiones committeadas.

## Regeneracion de fixtures

Ejecuta los comandos de abajo desde la raiz del repo para regenerar el set de fixtures. Reflejan los pasos del workflow `sorafs-cli-fixture`:

```bash
sorafs_cli car pack       --input fixtures/sorafs_manifest/ci_sample/payload.txt       --car-out fixtures/sorafs_manifest/ci_sample/payload.car       --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to       --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --car fixtures/sorafs_manifest/ci_sample/payload.car       --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig       --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)"       --issued-at 1700000000       > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b       > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

Si algun paso produce hashes diferentes, investiga antes de actualizar los fixtures. Los workflows de CI dependen de output determinista para detectar regresiones.

## Cobertura futura

A medida que perfiles adicionales de chunker y formatos de proof salgan de la roadmap, sus fixtures canonicos se agregaran en este directorio (por ejemplo,
`sorafs.sf2@1.0.0` (ver `fixtures/sorafs_manifest/ci_sample_sf2/`) o proofs de streaming PDP). Cada perfil seguira la misma estructura - payload, CAR,
plan, manifest, proofs y artefactos de firma - para que la automatizacion downstream pueda comparar releases sin scripting a medida.
