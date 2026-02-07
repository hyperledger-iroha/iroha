---
lang: uz
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

# SoraFS CI oshxona kitobi

Ushbu parcha `docs/source/sorafs_ci_templates.md` ko'rsatmalarini aks ettiradi va
imzolash, tekshirish va isbot tekshiruvlarini a ga qanday kiritish kerakligini ko'rsatadi
bitta GitHub Actions ishi.

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

## Eslatmalar

- `sorafs_cli` yuguruvchida mavjud bo'lishi kerak (masalan, ushbu bosqichlardan oldin `cargo install --path crates/sorafs_car --features cli`).
- Ish jarayoni aniq OIDC auditoriyasini taqdim etishi kerak (bu erda `sorafs`); Fulcio siyosatingizga mos kelish uchun `--identity-token-audience` sozlang.
- Chiqarish quvuri boshqaruvni tekshirish uchun `artifacts/manifest.bundle.json`, `artifacts/manifest.sig` va `artifacts/proof.json` arxivi kerak.
- Deterministik namunaviy artefaktlar `fixtures/sorafs_manifest/ci_sample` da yashaydi; Agar sizga oltin manifestlar, bo'lak rejalar kerak bo'lsa yoki quvur liniyasini qayta hisoblamasdan JSON to'plami kerak bo'lganda ularni testlarga ko'chiring.

## Armaturani tekshirish

Ushbu ish oqimi uchun deterministik artefaktlar ostida yashaydi
`fixtures/sorafs_manifest/ci_sample`. Quvurlar yuqoridagi va qadamlarni takrorlashi mumkin
ularning natijalarini kanonik fayllardan farqlang, masalan:

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

Bo'sh farqlar qurilishni tasdiqlaydi bayt-bir xil manifestlar, rejalar va
imzo to'plamlari. Toʻliq maʼlumot uchun `fixtures/sorafs_manifest/ci_sample/README.md` ga qarang
katalog ro'yxati va qo'lga olingan reliz eslatmalarini andoza qilish bo'yicha maslahatlar
xulosalar.