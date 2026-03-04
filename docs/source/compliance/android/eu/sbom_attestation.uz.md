---
lang: uz
direction: ltr
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-05T09:28:12.002687+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM & Provenance Attestation — Android SDK

| Maydon | Qiymat |
|-------|-------|
| Qo'llash doirasi | Android SDK (`java/iroha_android`) + namunaviy ilovalar (`examples/android/*`) |
| Ish oqimi egasi | Chiqarish muhandisligi (Aleksey Morozov) |
| Oxirgi tasdiqlangan | 2026-02-11 (Buildkite `android-sdk-release#4821`) |

## 1. Generation Workflow

Yordamchi skriptni ishga tushiring (AND6 avtomatlashtirish uchun qo'shilgan):

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

Skript quyidagi amallarni bajaradi:

1. `ci/run_android_tests.sh` va `scripts/check_android_samples.sh` ni bajaradi.
2. CycloneDX SBOM larini yaratish uchun `examples/android/` ostida Gradle o'ramini chaqiradi.
   `:android-sdk`, `:operator-console` va `:retail-wallet` yetkazib berilgan
   `-PversionName`.
3. Har bir SBOMni kanonik nomlar bilan `artifacts/android/sbom/<sdk-version>/` ga nusxalaydi
   (`iroha-android.cyclonedx.json` va boshqalar).

## 2. Kelib chiqishi va imzolanishi

Xuddi shu skript har bir SBOMni `cosign sign-blob --bundle <file>.sigstore --yes` bilan imzolaydi
va maqsadli katalogda `checksums.txt` (SHA-256) chiqaradi. `COSIGN` ni o'rnating
agar ikkilik `$PATH` dan tashqarida yashasa, muhit o'zgaruvchisi. Skript tugagandan so'ng,
to'plam/cheksum yo'llarini va Buildkite ishga tushirish identifikatorini yozib oling
`docs/source/compliance/android/evidence_log.csv`.

## 3. Tekshirish

Chop etilgan SBOMni tekshirish uchun:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

SHA chiqishini `checksums.txt` da keltirilgan qiymat bilan solishtiring. Sharhchilar, shuningdek, qaramlik deltalari qasddan bo'lishini ta'minlash uchun SBOMni oldingi versiyadan farq qiladi.

## 4. Dalillar surati (2026-02-11)

| Komponent | SBOM | SHA-256 | Sigstore to'plami |
|----------|------|---------|-----------------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` to'plami SBOM | yonida saqlangan
| Operator konsoli namunasi | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| Chakana hamyon namunasi | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Buildkite-dan olingan xeshlar `android-sdk-release#4821`-da ishlaydi; yuqoridagi tasdiqlash buyrug'i orqali ko'paytiring.)*

## 5. Ajoyib ish

- GA dan oldin bo'shatish quvuridagi SBOM + kosign qadamlarini avtomatlashtirish.
- AND6 nazorat roʻyxati tugallanganini bildirgandan soʻng, SBOMlarni ommaviy artefakt paqiriga aks ettiring.
- SBOM yuklab olish joylarini hamkorlarga tegishli relizlar qaydlaridan bog'lash uchun Docs bilan muvofiqlashtiring.