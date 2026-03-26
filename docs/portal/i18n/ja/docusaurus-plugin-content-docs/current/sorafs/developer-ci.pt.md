---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-ci.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-ci
タイトル: Receitas de CI da SoraFS
サイドバーラベル: CI の受信
説明: CLI da SoraFS を実行して、GitHub および GitLab com assinatura sem Chaves のパイプラインを実行します。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/sorafs/developer/ci.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

# CI の受信

SoraFS のパイプラインは、チャンクの確定性、マニフェストの検証、証明の実行に役立ちます。コマンドの特権
`sorafs_cli` は、CI の検証に必要なすべてのポートを管理します。正規のテンプレートとしてページを開き、使用するためのテンプレートを作成します。

## GitHub アクション (sem chaves)

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
            --authority <i105-account-id> \
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

ポントス・チャベ:

- 軍事行動を強化するネンフマ チャベ。トークン OIDC SAO オブティドス すすり泣く要求。
- Artefatos (CAR、マニフェスト、バンドル、証拠の履歴) 改訂版。
- O ジョブは、Norito のロールアウトを再利用しません。

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
    - sorafs_cli manifest submit --manifest artifacts/site.manifest.to --chunk-plan artifacts/site.plan.json --torii-url "$TORII_URL" --authority <i105-account-id> --private-key "$IROHA_PRIVATE_KEY" --summary-out artifacts/site.submit.json
    - sorafs_cli proof verify --manifest artifacts/site.manifest.to --car artifacts/site.car --summary-out artifacts/site.verify.json
  artifacts:
    paths:
      - artifacts/
```

- 連邦政府経由で `SIGSTORE_ID_TOKEN` をプロビジョニングし、ワークロードを特定して GitLab を実行し、公開する前に実行する必要があります。
- CLI でパイプライン パラレルを実行するためのファルハ デ クアルケールは、一貫した保存技術を備えています。

## Recursos アディシオネ

- エンドツーエンドのテンプレート (Bash ヘルパー、ID フェデラーダおよびリンペザの構成を含む): `docs/examples/sorafs_ci.md`
- CLI cobrindo todas を opcoes として参照: `docs/source/sorafs_cli.md`
- Requisitos de Governmenta/alias antes do envio:
  `docs/source/sorafs/provider_admission_policy.md`