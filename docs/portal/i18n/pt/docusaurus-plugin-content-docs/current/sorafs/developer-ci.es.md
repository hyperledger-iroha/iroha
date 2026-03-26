---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-ci.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: desenvolvedor-ci
título: Receitas de CI de SoraFS
sidebar_label: Receitas de CI
description: Execute a CLI de SoraFS em pipelines de GitHub e GitLab com firma sem chaves.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/developer/ci.md`. Mantenha ambas as versões sincronizadas até que os documentos herdados sejam retirados.
:::

#Recetas de CI

Os pipelines de SoraFS são benéficos para o chunking determinista, a firma de manifestos e a
verificação de provas. A superfície de comandos de `sorafs_cli` mantém esses passos
portáteis entre provedores de CI. Esta página realça as receitas canônicas e apunta a
listas de plantas para usar.

## GitHub Actions (sem chaves)

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
          TORII_URL: https://gateway.example/v2
          IROHA_PRIVATE_KEY: ${{ secrets.IROHA_PRIVATE_KEY }}
        run: |
          sorafs_cli manifest submit \
            --manifest artifacts/site.manifest.to \
            --chunk-plan artifacts/site.plan.json \
            --torii-url "$TORII_URL" \
            --authority <katakana-i105-account-id> \
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

Pontos chaves:

- Não se armazenam chaves de firma estáticas; Os tokens OIDC são obtidos em baixa demanda.
- Os artefatos (CAR, manifesto, pacote, resumos de provas) são submetidos para revisão.
- O trabalho reutiliza os mesmos esquemas Norito usados ​​nos lançamentos de produção.

## CI do GitLab

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
    - sorafs_cli manifest submit --manifest artifacts/site.manifest.to --chunk-plan artifacts/site.plan.json --torii-url "$TORII_URL" --authority <katakana-i105-account-id> --private-key "$IROHA_PRIVATE_KEY" --summary-out artifacts/site.submit.json
    - sorafs_cli proof verify --manifest artifacts/site.manifest.to --car artifacts/site.car --summary-out artifacts/site.verify.json
  artifacts:
    paths:
      - artifacts/
```

- Aprovisionamento `SIGSTORE_ID_TOKEN` por meio da federação de identidade de carga de trabalho do GitLab ou de um
  segredo vendido antes de executar a etapa de publicação.
- A falha de qualquer passagem do CLI faz com que o pipeline seja detido, preservando
  artefatos consistentes.

## Recursos adicionais

- Plantas ponta a ponta (incluem ajudantes Bash, configuração de identidade federada e
  passos de limpeza): `docs/examples/sorafs_ci.md`
- Referência da CLI com todas as opções: `docs/source/sorafs_cli.md`
- Requisitos de governança/alias antes do envio:
  `docs/source/sorafs/provider_admission_policy.md`