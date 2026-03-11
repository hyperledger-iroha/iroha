---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-ci.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-ci
titre : Recetas de CI de SoraFS
sidebar_label : Recettes de CI
description : Exécutez la CLI de SoraFS dans les pipelines de GitHub et GitLab sans clés.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/developer/ci.md`. Gardez les versions synchronisées jusqu'à ce que les documents hérités soient retirés.
:::

# Recettes de CI

Les pipelines de SoraFS sont bénéfiques pour le déterministe du chunking, la société de manifestes et la
vérification des preuves. La surface des commandes de `sorafs_cli` conserve ces étapes
portables entre fournisseurs de CI. Cette page affiche les recettes canoniques et les apporte à
plantillas listas para usar.

## Actions GitHub (sans clés)

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
            --authority i105... \
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

Points clés :

- No se almacenan claves de firma estáticas ; les jetons OIDC sont obtenus à basse demande.
- Les artefacts (CAR, manifeste, bundle, resúmenes de proofs) sont soumis à la révision.
- Le travail réutilise les mêmes types Norito utilisés dans les déploiements de production.

## GitLabCI

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
    - sorafs_cli manifest submit --manifest artifacts/site.manifest.to --chunk-plan artifacts/site.plan.json --torii-url "$TORII_URL" --authority i105... --private-key "$IROHA_PRIVATE_KEY" --summary-out artifacts/site.submit.json
    - sorafs_cli proof verify --manifest artifacts/site.manifest.to --car artifacts/site.car --summary-out artifacts/site.verify.json
  artifacts:
    paths:
      - artifacts/
```

- Mise à disposition `SIGSTORE_ID_TOKEN` entre la fédération d'identité de charge de travail de GitLab ou un
  secreto sellado avant d’exécuter l’étape de publication.
- La chute de n'importe quelle étape de la CLI fait que le pipeline se desserre, préserve
  artefactos cohérents.

## Ressources supplémentaires- Plantes de bout en bout (y compris les assistants Bash, la configuration de l'identité fédérale et
  étapes de nettoyage): `docs/examples/sorafs_ci.md`
- Référence de la CLI avec toutes les options : `docs/source/sorafs_cli.md`
- Conditions requises pour l'administration/alias avant l'envoi :
  `docs/source/sorafs/provider_admission_policy.md`