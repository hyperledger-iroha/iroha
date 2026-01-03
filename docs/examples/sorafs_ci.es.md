---
lang: es
direction: ltr
source: docs/examples/sorafs_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 61536f7f6dbecb2658cb5e07a15ed99888957bf87b473e0089e38f8431156c18
source_last_modified: "2025-11-08T09:36:10.010765+00:00"
translation_last_reviewed: 2026-01-01
---

---
title: Recetario de CI de SoraFS
summary: Flujo de GitHub Actions que agrupa firmas y verificacion en un solo job, con notas de revision.
---

# Recetario de CI de SoraFS

Este snippet refleja la guia en `docs/source/sorafs_ci_templates.md` y demuestra como integrar
firmas, verificacion y checks de proof en un solo job de GitHub Actions.

```yaml
name: sorafs-cli-release

on:
  push:
    branches: [main]

permissions:
  contents: read
  id-token: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-rust@v1
        with:
          rust-version: 1.92

      - name: Package payload
        run: |
          mkdir -p artifacts
          sorafs_cli car pack             --input payload.bin             --car-out artifacts/payload.car             --plan-out artifacts/chunk_plan.json             --summary-out artifacts/car_summary.json
          sorafs_cli manifest build             --summary artifacts/car_summary.json             --manifest-out artifacts/manifest.to

      - name: Sign manifest bundle
        run: |
          sorafs_cli manifest sign             --manifest artifacts/manifest.to             --chunk-plan artifacts/chunk_plan.json             --bundle-out artifacts/manifest.bundle.json             --signature-out artifacts/manifest.sig             --identity-token-provider=github-actions             --identity-token-audience=sorafs | tee artifacts/manifest.sign.summary.json

      - name: Verify manifest bundle
        run: |
          sorafs_cli manifest verify-signature             --manifest artifacts/manifest.to             --bundle artifacts/manifest.bundle.json             --summary artifacts/car_summary.json

      - name: Proof verification
        run: |
          sorafs_cli proof verify             --manifest artifacts/manifest.to             --car artifacts/payload.car             --summary-out artifacts/proof.json

      - uses: sigstore/cosign-installer@v3
      - name: Verify bundle with cosign
        run: cosign verify-blob --bundle artifacts/manifest.bundle.json artifacts/manifest.to
```

## Notas

- `sorafs_cli` debe estar disponible en el runner (p. ej., `cargo install --path crates/sorafs_car --features cli` antes de estos pasos).
- El workflow debe proporcionar un audience OIDC explicito (aqui `sorafs`); ajusta `--identity-token-audience` para que coincida con tu politica de Fulcio.
- El pipeline de release debe archivar `artifacts/manifest.bundle.json`, `artifacts/manifest.sig` y `artifacts/proof.json` para revision de governance.
- Los artefactos deterministas de ejemplo viven en `fixtures/sorafs_manifest/ci_sample`; copialos en tests cuando necesites manifests, chunk plans o bundle JSON golden sin recomputar el pipeline.

## Verificacion de fixtures

Los artefactos deterministas para este workflow viven bajo
`fixtures/sorafs_manifest/ci_sample`. Los pipelines pueden repetir los pasos anteriores y
comparar sus salidas contra los archivos canonicos, por ejemplo:

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

Diffs vacios confirman que la build produjo manifests, planes y bundles de firma con bytes
identicos. Consulta `fixtures/sorafs_manifest/ci_sample/README.md` para un listado completo
y tips sobre plantillas de notas de release a partir de los summaries capturados.
