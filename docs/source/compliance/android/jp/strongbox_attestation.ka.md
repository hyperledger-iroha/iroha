---
lang: ka
direction: ltr
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2025-12-29T18:16:35.929201+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# StrongBox ატესტაციის მტკიცებულება — იაპონიის განლაგება

| ველი | ღირებულება |
|-------|-------|
| შეფასების ფანჯარა | 2026-02-10 – 2026-02-12 |
| არტეფაქტის მდებარეობა | `artifacts/android/attestation/<device-tag>/<date>/` (პაკეტის ფორმატი `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`-ზე) |
| Capture Tooling | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| რეცენზენტები | Hardware Lab წამყვანი, შესაბამისობა და იურიდიული (JP) |

## 1. გადაღების პროცედურა

1. StrongBox მატრიცაში ჩამოთვლილ თითოეულ მოწყობილობაზე შექმენით გამოწვევა და აიღეთ ატესტაციის ნაკრები:
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. ჩააბარეთ ნაკრების მეტამონაცემები (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) მტკიცებულების ხეზე.
3. გაუშვით CI დამხმარე, რათა ხელახლა გადაამოწმოთ ყველა პაკეტი ხაზგარეშე:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. მოწყობილობის შეჯამება (2026-02-12)

| მოწყობილობის ტეგი | მოდელი / StrongBox | Bundle Path | შედეგი | შენიშვნები |
|------------|------------------|------------|--------|------|
| `pixel6-strongbox-a` | Pixel 6 / Tensor G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ გავლილი (ტექნიკის მხარდაჭერით) | გამოწვევის შეზღუდვა, OS პაჩი 2025-03-05. |
| `pixel7-strongbox-a` | Pixel 7 / Tensor G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ ჩააბარა | პირველადი CI შესახვევის კანდიდატი; ტემპერატურა სპეც. |
| `pixel8pro-strongbox-a` | Pixel 8 Pro / Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ გაიარა (ხელახალი ტესტი) | USB-C კერა შეიცვალა; Buildkite `android-strongbox-attestation#221`-მა გადაიღო გამვლელი ნაკრები. |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ ჩააბარა | Knox ატესტაციის პროფილი იმპორტირებულია 2026-02-09. |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ ჩააბარა | Knox ატესტაციის პროფილი იმპორტირებულია; CI შესახვევი ახლა მწვანეა. |

მოწყობილობის ტეგები რუკაზე `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`.

## 3. რეცენზენტთა ჩამონათვალი

- [x] გადაამოწმეთ `result.json` აჩვენებს `strongbox_attestation: true`-ს და სერთიფიკატების ჯაჭვს სანდო root-ს.
- [x] დაადასტურეთ გამოწვევის ბაიტები ემთხვევა Buildkite გაშვებებს `android-strongbox-attestation#219` (საწყისი წმენდა) და `#221` (Pixel 8 Pro ხელახალი ტესტი + S24 გადაღება).
- [x] ხელახლა გაუშვით Pixel 8 Pro-ის გადაღება ტექნიკის შეკეთების შემდეგ (მფლობელი: Hardware Lab Lead, დასრულებული 2026-02-13).
- [x] დაასრულეთ Galaxy S24-ის გადაღება Knox-ის პროფილის დამტკიცების შემდეგ (მფლობელი: Device Lab Ops, დასრულებული 2026-02-13).

## 4. დისტრიბუცია

- მიამაგრეთ ეს შეჯამება და უახლესი ანგარიშის ტექსტური ფაილი პარტნიორთა შესაბამისობის პაკეტებს (FISC საკონტროლო სია §მონაცემთა რეზიდენტობა).
- რეგულატორის აუდიტებზე პასუხის გაცემის პაკეტების ბილიკები; არ გადასცეთ დაუმუშავებელი სერთიფიკატები დაშიფრული არხების გარეთ.

## 5. ჟურნალის შეცვლა

| თარიღი | შეცვლა | ავტორი |
|------|--------|--------|
| 2026-02-12 | JP პაკეტის საწყისი აღბეჭდვა + ანგარიში. | Device Lab Ops |