---
lang: kk
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

# SoraFS CI аспаздық кітабы

Бұл үзінді `docs/source/sorafs_ci_templates.md` нұсқаулығын көрсетеді және
қол қою, тексеру және дәлелдеу тексерулерін а
жалғыз GitHub әрекеттері жұмысы.

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

## Ескертпелер

- `sorafs_cli` жүгіргіште қолжетімді болуы керек (мысалы, `cargo install --path crates/sorafs_car --features cli` осы қадамдарға дейін).
- Жұмыс процесі анық OIDC аудиториясын қамтамасыз етуі керек (мұнда `sorafs`); Fulcio саясатыңызға сәйкестендіру үшін `--identity-token-audience` реттеңіз.
- Шығарылым құбыры басқаруды тексеру үшін `artifacts/manifest.bundle.json`, `artifacts/manifest.sig` және `artifacts/proof.json` мұрағаттауы керек.
- Детерминистік үлгі артефактілері `fixtures/sorafs_manifest/ci_sample` ішінде өмір сүреді; Құбырды қайта есептемей, алтын манифесттер, бөліктік жоспарлар немесе JSON бумасы қажет болғанда оларды сынақтарға көшіріңіз.

## Арматураны тексеру

Осы жұмыс процесі үшін детерминистік артефактілер астында тұрады
`fixtures/sorafs_manifest/ci_sample`. Құбырлар жоғарыдағы және қадамдарды қайталай алады
олардың шығыстарын канондық файлдардан ажыратады, мысалы:

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

Бос айырмашылықтар құрастыруды растайды байт-бірдей манифесттер, жоспарлар және
қолтаңба топтамалары. Толық ақпаратты `fixtures/sorafs_manifest/ci_sample/README.md` қараңыз
каталогтар тізімі және түсірілген жазбалардан үлгілеу туралы кеңестер
қорытындылар.