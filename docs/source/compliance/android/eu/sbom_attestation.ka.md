---
lang: ka
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

# SBOM და წარმოშობის ატესტაცია — Android SDK

| ველი | ღირებულება |
|-------|-------|
| ფარგლები | Android SDK (`java/iroha_android`) + აპლიკაციების ნიმუში (`examples/android/*`) |
| სამუშაო პროცესის მფლობელი | გამოშვების ინჟინერია (ალექსეი მოროზოვი) |
| ბოლო დადასტურებული | 2026-02-11 (Buildkite `android-sdk-release#4821`) |

## 1. თაობის სამუშაო პროცესი

გაუშვით დამხმარე სკრიპტი (დამატებულია AND6 ავტომატიზაციისთვის):

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

სკრიპტი ასრულებს შემდეგს:

1. ასრულებს `ci/run_android_tests.sh` და `scripts/check_android_samples.sh`.
2. გამოიძახებს Gradle wrapper-ს `examples/android/` ქვეშ CycloneDX SBOM-ების შესაქმნელად
   `:android-sdk`, `:operator-console` და `:retail-wallet` მოწოდებულთან ერთად
   `-PversionName`.
3. აკოპირებს თითოეულ SBOM-ს `artifacts/android/sbom/<sdk-version>/`-ში კანონიკური სახელებით
   (`iroha-android.cyclonedx.json` და ა.შ.).

## 2. წარმოშობა და ხელმოწერა

იგივე სკრიპტი ხელს აწერს ყველა SBOM-ს `cosign sign-blob --bundle <file>.sigstore --yes`-ით
და ასხივებს `checksums.txt` (SHA-256) დანიშნულების დირექტორიაში. დააყენეთ `COSIGN`
გარემოს ცვლადი, თუ ორობითი ცხოვრობს `$PATH` გარეთ. სცენარის დასრულების შემდეგ,
ჩაწერეთ პაკეტის/შემოწმების ჯამის ბილიკები პლუს Buildkite run id-ში
`docs/source/compliance/android/evidence_log.csv`.

## 3. შემოწმება

გამოქვეყნებული SBOM-ის დასადასტურებლად:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

შეადარეთ გამომავალი SHA `checksums.txt`-ში ჩამოთვლილ მნიშვნელობას. მიმომხილველები ასევე განასხვავებენ SBOM-ს წინა გამოშვებისგან, რათა დარწმუნდნენ, რომ დამოკიდებულების დელტა არის მიზანმიმართული.

## 4. მტკიცებულების სურათი (2026-02-11)

| კომპონენტი | SBOM | SHA-256 | Sigstore პაკეტი |
|-----------|------|---------|----------------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` პაკეტი ინახება SBOM-თან |
| ოპერატორის კონსოლის ნიმუში | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| საცალო საფულის ნიმუში | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(ჰეშები აღბეჭდილი Buildkite-დან `android-sdk-release#4821`; რეპროდუცირება ზედა გადამოწმების ბრძანების მეშვეობით.)*

## 5. გამორჩეული ნამუშევარი

- SBOM + cosign ნაბიჯების ავტომატიზაცია გაშვების მილსადენში GA-მდე.
- ასახეთ SBOM-ები საჯარო არტეფაქტის თაიგულში, როგორც კი AND6 მონიშნავს საკონტროლო სიის დასრულებას.
- კოორდინაცია გაუწიეთ Docs-ს SBOM-ის ჩამოტვირთვის მდებარეობების დასაკავშირებლად პარტნიორის გამოშვების ჩანაწერებიდან.