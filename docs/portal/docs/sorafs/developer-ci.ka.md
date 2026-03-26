---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/developer-ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a4a42040dfcc033d9e4476eb5a93a7806b8812db94a3fe47e91ec38f18e1ba7
source_last_modified: "2026-01-22T16:26:46.521619+00:00"
translation_last_reviewed: 2026-02-07
id: developer-ci
title: SoraFS CI Recipes
sidebar_label: CI Recipes
description: Run the SoraFS CLI inside GitHub and GitLab pipelines with keyless signing.
translator: machine-google-reviewed
---

:::შენიშვნა კანონიკური წყარო
:::

# CI რეცეპტები

SoraFS მილსადენები სარგებლობენ დეტერმინისტული დაშლით, მანიფესტის ხელმოწერით და
მტკიცებულების შემოწმება. `sorafs_cli` ბრძანების ზედაპირი ინარჩუნებს ამ ნაბიჯებს პორტატულად
CI პროვაიდერებს შორის. ეს გვერდი ხაზს უსვამს კანონიკურ რეცეპტებს და მიუთითებს
მზა შაბლონები.

## GitHub მოქმედებები (გასაღების გარეშე)

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

ძირითადი პუნქტები:

- არ არის შენახული სტატიკური ხელმოწერის გასაღებები; OIDC ჟეტონები მოთხოვნილია.
- არტეფაქტები (CAR, მანიფესტი, ნაკრები, მტკიცებულების რეზიუმეები) ატვირთულია განსახილველად.
- სამუშაო ხელახლა იყენებს იგივე Norito სქემებს, რომლებიც გამოიყენება წარმოების გავრცელებაში.

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

- უზრუნველყოს `SIGSTORE_ID_TOKEN` GitLab-ის დატვირთვის იდენტიფიკაციის ფედერაციის ან
  დალუქული საიდუმლო გამოქვეყნების ეტაპის შესრულებამდე.
- ნებისმიერი CLI ნაბიჯის წარუმატებლობა იწვევს მილსადენის გაჩერებას, თანმიმდევრულობის შენარჩუნებას
  არტეფაქტები.

## დამატებითი რესურსები

- ბოლოდან ბოლომდე შაბლონები (მოიცავს Bash დამხმარეებს, ფედერაციულ პირადობის კონფიგურაციას,
  და დასუფთავების ნაბიჯები): `docs/examples/sorafs_ci.md`
- CLI მითითება, რომელიც მოიცავს ყველა ვარიანტს: `docs/source/sorafs_cli.md`
- მმართველობა/სხვა მოთხოვნები წარდგენამდე:
  `docs/source/sorafs/provider_admission_policy.md`