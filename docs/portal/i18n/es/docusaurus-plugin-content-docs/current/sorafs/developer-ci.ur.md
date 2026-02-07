---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-ci.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: desarrollador-ci
título: SoraFS recetas CI
sidebar_label: recetas de CI
descripción: GitHub y GitLab pipelines firma sin llave کے ساتھ SoraFS CLI چلائیں۔
---

:::nota مستند ماخذ
:::

# SoraFS Recetas de CI

SoraFS fragmentación determinista de canalizaciones, firma de manifiesto y verificación de prueba سے فائدہ اٹھاتے ہیں۔
`sorafs_cli` Superficie de comando y pasos کو proveedores de CI کے درمیان portátiles رکھتا ہے۔ یہ صفحہ canónico
recetas destacadas کرتا ہے اور plantillas listas para usar کی طرف اشارہ کرتا ہے۔

## Acciones de GitHub (sin clave)

```yaml
name: sorafs-artifacts

on:
  push:
    branches: [ main ]

jobs:
  build-and-publish:
    runs-on: ubuntu-latest
    permissions:
      id-token: write
      contents: read
    env:
      RUSTFLAGS: "-C target-cpu=native"
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          profile: minimal
          toolchain: stable
      - name: Build CLI
        run: cargo install --path crates/sorafs_car --features cli --bin sorafs_cli --debug
      - name: Pack payload and manifest
        run: |
          sorafs_cli car pack \
            --input fixtures/site.tar.gz \
            --car-out artifacts/site.car \
            --plan-out artifacts/site.plan.json \
            --summary-out artifacts/site.car.json
          sorafs_cli manifest build \
            --summary artifacts/site.car.json \
            --chunk-plan artifacts/site.plan.json \
            --manifest-out artifacts/site.manifest.to
      - name: Sign manifest (Sigstore OIDC)
        run: |
          sorafs_cli manifest sign \
            --manifest artifacts/site.manifest.to \
            --bundle-out artifacts/site.manifest.bundle.json \
            --signature-out artifacts/site.manifest.sig \
            --identity-token-provider=github-actions
      - name: Submit manifest
        env:
          TORII_URL: https://gateway.example/v1
          IROHA_PRIVATE_KEY: ${{ secrets.IROHA_PRIVATE_KEY }}
        run: |
          sorafs_cli manifest submit \
            --manifest artifacts/site.manifest.to \
            --chunk-plan artifacts/site.plan.json \
            --torii-url "$TORII_URL" \
            --authority ih58... \
            --private-key "$IROHA_PRIVATE_KEY" \
            --summary-out artifacts/site.submit.json
      - name: Stream PoR proofs
        env:
          GATEWAY_URL: https://gateway.example/v1/sorafs/proof/stream
          STREAM_TOKEN: ${{ secrets.SORAFS_STREAM_TOKEN }}
        run: |
          sorafs_cli proof stream \
            --manifest artifacts/site.manifest.to \
            --gateway-url "$GATEWAY_URL" \
            --provider-id provider::alpha \
            --samples 64 \
            --stream-token "$STREAM_TOKEN" \
            --summary-out artifacts/site.proof_stream.json
      - uses: actions/upload-artifact@v4
        with:
          name: sorafs-artifacts
          path: artifacts/
```

Puntos clave:

- Tienda de claves de firma estática نہیں ہوتے؛ OIDC recuperación de tokens bajo demanda ہوتے ہیں۔
- Revisión de artefactos (CAR, manifiesto, paquete, resúmenes de pruebas) کے لیے subir ہوتے ہیں۔
- Reutilización de esquemas de trabajo y Norito کرتا ہے جو implementaciones de producción میں استعمال ہوتے ہیں۔

## GitLab CI

```yaml
stages:
  - build
  - publish

variables:
  RUSTFLAGS: "-C target-cpu=native"

sorafs:build:
  stage: build
  image: rust:1.81
  script:
    - cargo install --path crates/sorafs_car --features cli --bin sorafs_cli --debug
    - sorafs_cli car pack --input fixtures/site.tar.gz --car-out artifacts/site.car --plan-out artifacts/site.plan.json --summary-out artifacts/site.car.json
    - sorafs_cli manifest build --summary artifacts/site.car.json --chunk-plan artifacts/site.plan.json --manifest-out artifacts/site.manifest.to
  artifacts:
    paths:
      - artifacts/

sorafs:publish:
  stage: publish
  needs: ["sorafs:build"]
  image: rust:1.81
  script:
    - sorafs_cli manifest sign --manifest artifacts/site.manifest.to --bundle-out artifacts/site.manifest.bundle.json --signature-out artifacts/site.manifest.sig --identity-token-env SIGSTORE_ID_TOKEN
    - sorafs_cli manifest submit --manifest artifacts/site.manifest.to --chunk-plan artifacts/site.plan.json --torii-url "$TORII_URL" --authority ih58... --private-key "$IROHA_PRIVATE_KEY" --summary-out artifacts/site.submit.json
    - sorafs_cli proof verify --manifest artifacts/site.manifest.to --car artifacts/site.car --summary-out artifacts/site.verify.json
  artifacts:
    paths:
      - artifacts/
```

- `SIGSTORE_ID_TOKEN` کو Federación de identidad de carga de trabajo de GitLab یا secreto sellado کے ذریعے provisión کریں، etapa de publicación چلانے سے پہلے۔
- CLI کے کسی بھی paso کی falla en la canalización کو detener کر دیتی ہے، اور artefactos consistentes محفوظ رہتے ہیں۔

## Recursos adicionales

- Plantillas de un extremo a otro (ayudantes de Bash, configuración de identidad federada, pasos de limpieza adicionales): `docs/examples/sorafs_ci.md`
- Referencia CLI جو ہر cubierta de opción کرتا ہے: `docs/source/sorafs_cli.md`
- Requisitos de gobernanza/alias de presentación:
  `docs/source/sorafs/provider_admission_policy.md`