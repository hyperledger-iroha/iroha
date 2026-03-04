---
lang: az
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

| Sahə | Dəyər |
|-------|-------|
| Əhatə dairəsi | Android SDK (`java/iroha_android`) + nümunə proqramlar (`examples/android/*`) |
| İş axınının sahibi | Release Engineering (Aleksey Morozov) |
| Son Doğrulanmış | 11-02-2026 (Buildkite `android-sdk-release#4821`) |

## 1. Nəsil iş axını

Köməkçi skripti işə salın (AND6 avtomatlaşdırılması üçün əlavə edilib):

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

Skript aşağıdakıları yerinə yetirir:

1. `ci/run_android_tests.sh` və `scripts/check_android_samples.sh`-i yerinə yetirir.
2. CycloneDX SBOM-larını yaratmaq üçün `examples/android/` altında Gradle sarğısını işə salır.
   `:android-sdk`, `:operator-console` və `:retail-wallet` təchiz edilmiş
   `-PversionName`.
3. Hər bir SBOM-u kanonik adlarla `artifacts/android/sbom/<sdk-version>/`-ə kopyalayır
   (`iroha-android.cyclonedx.json` və s.).

## 2. Mənşə və İmza

Eyni skript hər SBOM-u `cosign sign-blob --bundle <file>.sigstore --yes` ilə imzalayır
və təyinat kataloqunda `checksums.txt` (SHA-256) yayır. `COSIGN` seçin
ikili `$PATH` xaricində yaşayırsa mühit dəyişəni. After the script finishes,
paket/yoxlama yollarını və Buildkite run id-ni qeyd edin
`docs/source/compliance/android/evidence_log.csv`.

## 3. Doğrulama

Dərc edilmiş SBOM-u yoxlamaq üçün:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

SHA çıxışını `checksums.txt`-də sadalanan dəyərlə müqayisə edin. Rəyçilər həmçinin asılılıq deltalarının qəsdən olmasını təmin etmək üçün SBOM-u əvvəlki buraxılışdan fərqləndirirlər.

## 4. Sübut Snapshot (2026-02-11)

| Komponent | SBOM | SHA-256 | Sigstore Paketi |
|----------|------|---------|-----------------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` paketi SBOM | yanında saxlanılır
| Operator konsolu nümunəsi | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| Pərakəndə pul kisəsi nümunəsi | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Buildkite-dən alınmış heşlər `android-sdk-release#4821` ilə işləyir; yuxarıdakı doğrulama əmri ilə çoxaldın.)*

## 5. Görkəmli iş

- GA-dan əvvəl buraxılış boru kəməri daxilində SBOM + kosign addımlarını avtomatlaşdırın.
- AND6 yoxlama siyahısının tamamlandığını qeyd etdikdən sonra SBOM-ları ictimai artefakt kovasına əks etdirin.
- SBOM yükləmə yerlərini tərəfdaşlarla əlaqəli buraxılış qeydlərindən əlaqələndirmək üçün Sənədlərlə əlaqələndirin.