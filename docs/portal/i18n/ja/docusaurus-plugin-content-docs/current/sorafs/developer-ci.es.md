---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-ci.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-ci
タイトル: Recetas de CI de SoraFS
サイドバーラベル: CI のレセタス
説明: GitHub および GitLab のパイプラインからの SoraFS の CLI の取り出しは、クラベスで行われます。
---

:::メモ フエンテ カノニカ
`docs/source/sorafs/developer/ci.md` のページを参照してください。定期的にバージョンを確認し、ドキュメントを保存し、引退します。
:::

# CI のレセタス

SoraFS のパイプラインは、チャンキング決定の恩恵を受け、マニフェストの会社を決定します
証拠の検証。 `sorafs_cli` の最高コマンド
ポータブル機器は CI の証明に使用されます。アクセスできるページを開き、アクセスできるようになります
plantillas listas para usar。

## GitHub アクション (罪のクラベス)

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

プントス・クラーベ:

- アルマセナンのクラベス・デ・ファーム・エステカスはありません。ロストークン OIDC は、バホ要求を取得します。
- ロス アーティファクト (CAR、マニフェスト、バンドル、プルーフの履歴) の改訂版。
- 製造のロールアウトを再利用する Norito の作業。

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

- GitLab のワークロードの識別情報の連邦中央値 `SIGSTORE_ID_TOKEN` のプロビジョニング
  発行前に公開する秘密。
- パイプラインの安全性を維持するために CLI を実行する必要があります
  アーティファクトは一貫しています。

## アジシオナルス

- Plantillas のエンドツーエンド (ヘルパー Bash、ID 連邦構成を含む)
  パソス デ リンピエザ): `docs/examples/sorafs_ci.md`
- 最後のオプションに関する CLI の参照: `docs/source/sorafs_cli.md`
- Requisitos de gobernanza/別名 antes del envío:
  `docs/source/sorafs/provider_admission_policy.md`