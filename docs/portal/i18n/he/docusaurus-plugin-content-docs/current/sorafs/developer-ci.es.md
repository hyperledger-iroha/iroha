---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-ci.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: developer-ci
כותרת: Recetas de CI de SoraFS
sidebar_label: Recetas de CI
תיאור: Ejecuta el CLI de SoraFS en pipelines de GitHub y GitLab con firma sin claves.
---

:::הערה Fuente canónica
Esta página refleja `docs/source/sorafs/developer/ci.md`. Mantén ambas versiones sincronizadas hasta que los docs heredados se retiren.
:::

# Recetas de CI

Los pipelines de SoraFS se benefician del chunking determinista, la firma de manifests y la
אימות הוכחות. La superficie de comandos de `sorafs_cli` mantiene esos pasos
ניידים entre proveedores de CI. Esta página resalta las recetas canónicas y apunta a
plantillas lists para usar.

## פעולות GitHub (חטאים)

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

- No se almacenan claves de firma estáticas; los tokens OIDC se obtienen bajo demanda.
- Los artefactos (CAR, מניפסט, צרור, resúmenes de proofs) se suben para revisión.
- אל תעסוקה חוזרת ב-Norito בארצות הברית בהפצות ייצור.

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

- Aprovisiona `SIGSTORE_ID_TOKEN` באמצעי הפדרציה לזהות את עומס העבודה של GitLab o un
  secreto sellado antes de ejecutar la etapa de publish.
- El fallo de cualquier paso del CLI que el pipeline se detenga, preservando
  artefactos consistentes.

## אפשרויות נוספות

- Plantillas מקצה לקצה (כולל עוזרים Bash, Configuración de identidad federada y
  pasos de limpieza): `docs/examples/sorafs_ci.md`
- Referencia del CLI לאופציות נוספות: `docs/source/sorafs_cli.md`
- Requisitos de gobernanza/כינוי ante del envío:
  `docs/source/sorafs/provider_admission_policy.md`