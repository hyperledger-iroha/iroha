---
lang: hy
direction: ltr
source: docs/examples/sorafs_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e17a07b8d98725e24b75710496b0b69b6d8878160c7f12884e6e1eef0c0a4af7
source_last_modified: "2026-01-03T19:37:11.140795+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS CI Cookbook
summary: Reference GitHub Actions workflow bundling sign + verify steps with review notes.
translator: machine-google-reviewed
---

# SoraFS CI Խոհարարական գիրք

Այս հատվածը արտացոլում է `docs/source/sorafs_ci_templates.md` ուղեցույցը և
ցույց է տալիս, թե ինչպես կարելի է ինտեգրել ստորագրման, ստուգման և ապացուցման ստուգումները ա
GitHub Actions-ի մեկ աշխատանք:

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
          sorafs_cli car pack \
            --input payload.bin \
            --car-out artifacts/payload.car \
            --plan-out artifacts/chunk_plan.json \
            --summary-out artifacts/car_summary.json
          sorafs_cli manifest build \
            --summary artifacts/car_summary.json \
            --manifest-out artifacts/manifest.to

      - name: Sign manifest bundle
        run: |
          sorafs_cli manifest sign \
            --manifest artifacts/manifest.to \
            --chunk-plan artifacts/chunk_plan.json \
            --bundle-out artifacts/manifest.bundle.json \
            --signature-out artifacts/manifest.sig \
            --identity-token-provider=github-actions \
            --identity-token-audience=sorafs | tee artifacts/manifest.sign.summary.json

      - name: Verify manifest bundle
        run: |
          sorafs_cli manifest verify-signature \
            --manifest artifacts/manifest.to \
            --bundle artifacts/manifest.bundle.json \
            --summary artifacts/car_summary.json

      - name: Proof verification
        run: |
          sorafs_cli proof verify \
            --manifest artifacts/manifest.to \
            --car artifacts/payload.car \
            --summary-out artifacts/proof.json

      - uses: sigstore/cosign-installer@v3
      - name: Verify bundle with cosign
        run: cosign verify-blob --bundle artifacts/manifest.bundle.json artifacts/manifest.to
```

## Նշումներ

- `sorafs_cli`-ը պետք է հասանելի լինի վազորդի վրա (օրինակ՝ `cargo install --path crates/sorafs_car --features cli` նախքան այս քայլերը):
- Աշխատանքային հոսքը պետք է ապահովի հստակ OIDC լսարան (այստեղ `sorafs`); հարմարեցրեք `--identity-token-audience`՝ ձեր Fulcio քաղաքականությանը համապատասխանելու համար:
- Թողարկման խողովակաշարը պետք է արխիվացնի `artifacts/manifest.bundle.json`, `artifacts/manifest.sig` և `artifacts/proof.json` կառավարման վերանայման համար:
- Որոշիչ նմուշային արտեֆակտներ ապրում են `fixtures/sorafs_manifest/ci_sample`-ում; պատճենեք դրանք թեստերի մեջ, երբ ձեզ անհրաժեշտ են ոսկե մանիֆեստներ, կտոր պլաններ կամ փաթեթավորեք JSON՝ առանց խողովակաշարը վերահաշվարկելու:

## Սարքավորումների ստուգում

Այս աշխատանքային հոսքի համար դետերմինիստական արտեֆակտները ապրում են տակ
`fixtures/sorafs_manifest/ci_sample`. Խողովակաշարերը կարող են կրկնել վերը նշված քայլերը և
տարբերել դրանց արդյունքները կանոնական ֆայլերի հետ, օրինակ՝

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

Դատարկ տարբերությունները հաստատում են բայթ-նույնական մանիֆեստները, պլանները և արտադրված կառուցվածքը
ստորագրության փաթեթներ. Ամբողջական տե՛ս `fixtures/sorafs_manifest/ci_sample/README.md`
գրացուցակի ցանկը և խորհուրդներ կաղապարի թողարկման նշումներից գրավվածներից
ամփոփումներ.