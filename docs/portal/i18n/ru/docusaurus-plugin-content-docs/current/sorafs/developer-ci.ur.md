---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-ci.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: разработчик-ci
заголовок: SoraFS Рецепты CI
Sidebar_label: Рецепты CI
описание: GitHub Конвейеры GitLab и подписывание без ключа Поддержка SoraFS CLI چلائیں۔
---

:::примечание
:::

# SoraFS Рецепты CI

SoraFS конвейеры детерминированное разбиение на фрагменты, подписание манифеста и проверка доказательств.
`sorafs_cli` командная поверхность и шаги, необходимые поставщикам CI и портативным устройствам. یہ صفحہ канонический
рецепты выделить کرتا ہے اور готовые шаблоны کی طرف اشارہ کرتا ہے۔

## Действия GitHub (без ключа)

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
            --authority soraカタカナ... \
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

Ключевые моменты:

- Хранилище статических ключей подписи. OIDC токены по требованию, получение ہوتے ہیں۔
- Артефакты (CAR, манифест, пакет, сводка доказательств) просмотрите или загрузите ہوتے ہیں۔
- Job وہی Norito повторное использование схем и развертывание производства.

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
    - sorafs_cli manifest submit --manifest artifacts/site.manifest.to --chunk-plan artifacts/site.plan.json --torii-url "$TORII_URL" --authority soraカタカナ... --private-key "$IROHA_PRIVATE_KEY" --summary-out artifacts/site.submit.json
    - sorafs_cli proof verify --manifest artifacts/site.manifest.to --car artifacts/site.car --summary-out artifacts/site.verify.json
  artifacts:
    paths:
      - artifacts/
```

- `SIGSTORE_ID_TOKEN` для федерации идентификации рабочих нагрузок GitLab, запечатанный секрет, обеспечение для этапа публикации, этап публикации, этап публикации.
- CLI позволяет сделать шаг при сбое конвейера и остановить процесс создания согласованных артефактов.

## Дополнительные ресурсы

- Сквозные шаблоны (помощники Bash, конфигурация федеративной идентификации, дополнительные шаги по очистке): `docs/examples/sorafs_ci.md`.
- Справочник по CLI и обложке опции: `docs/source/sorafs_cli.md`
- Представление требований к управлению/псевдониму:
  `docs/source/sorafs/provider_admission_policy.md`