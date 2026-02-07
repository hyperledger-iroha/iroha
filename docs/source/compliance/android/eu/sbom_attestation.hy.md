---
lang: hy
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

# SBOM և ծագման հաստատում — Android SDK

| Դաշտային | Արժեք |
|-------|-------|
| Շրջանակ | Android SDK (`java/iroha_android`) + հավելվածների նմուշներ (`examples/android/*`) |
| Աշխատանքային հոսքի սեփականատեր | Release Engineering (Ալեքսեյ Մորոզով) |
| Վերջին ստուգված | 2026-02-11 (Buildkite `android-sdk-release#4821`) |

## 1. Սերնդի աշխատանքային հոսք

Գործարկեք օգնական սկրիպտը (ավելացվել է AND6 ավտոմատացման համար).

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

Սցենարը կատարում է հետևյալը.

1. Կատարում է `ci/run_android_tests.sh` և `scripts/check_android_samples.sh`:
2. Կանչում է Gradle փաթաթան `examples/android/` տակ՝ CycloneDX SBOM-ներ ստեղծելու համար
   `:android-sdk`, `:operator-console` և `:retail-wallet` մատակարարված
   `-PversionName`.
3. Յուրաքանչյուր SBOM-ը պատճենում է `artifacts/android/sbom/<sdk-version>/`-ում՝ կանոնական անուններով
   (`iroha-android.cyclonedx.json` և այլն):

## 2. Ծագում և ստորագրում

Նույն սցենարը ստորագրում է յուրաքանչյուր SBOM `cosign sign-blob --bundle <file>.sigstore --yes`-ով
և ուղարկում է `checksums.txt` (SHA-256) նպատակակետ գրացուցակում: Սահմանեք `COSIGN`
շրջակա միջավայրի փոփոխական, եթե երկուականն ապրում է `$PATH`-ից դուրս: Սցենարի ավարտից հետո,
գրանցեք փաթեթի/ստուգիչ գումարի ուղիները, գումարած Buildkite գործարկման ID-ն
`docs/source/compliance/android/evidence_log.csv`.

## 3. Ստուգում

Հրապարակված SBOM-ը ստուգելու համար՝

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

Համեմատեք ելքային SHA-ը `checksums.txt`-ում նշված արժեքի հետ: Վերանայողները նաև տարբերում են SBOM-ը նախորդ թողարկման համեմատ՝ ապահովելու համար, որ կախվածության դելտաները դիտավորյալ են:

## 4. Ապացույցի նկար (2026-02-11)

| Բաղադրիչ | SBOM | SHA-256 | Sigstore փաթեթ |
|-----------|------|---------|-----------------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` փաթեթը պահվում է SBOM-ի կողքին |
| Օպերատորի վահանակի նմուշ | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| Մանրածախ դրամապանակի նմուշ | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Հեշեր, որոնք վերցված են Buildkite-ի գործարկումից `android-sdk-release#4821`; վերարտադրել վերը նշված ստուգման հրամանի միջոցով:)*

## 5. Հիանալի աշխատանք

- Ավտոմատացրեք SBOM + համանշանային քայլերը թողարկման խողովակաշարի ներսում մինչև GA:
- Հայելային SBOM-ները հանրային արտեֆակտի դույլում, երբ AND6-ը նշում է ստուգաթերթի ամբողջականությունը:
- Համակարգեք Docs-ի հետ՝ SBOM-ի ներբեռնման վայրերը գործընկերոջ թողարկման նշումներից կապելու համար: