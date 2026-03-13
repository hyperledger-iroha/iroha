---
lang: ka
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2025-12-29T18:16:35.928660+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC Security Controls Checklist — Android SDK

| ველი | ღირებულება |
|-------|-------|
| ვერსია | 0.1 (2026-02-12) |
| ფარგლები | Android SDK + ოპერატორის ინსტრუმენტები, რომლებიც გამოიყენება იაპონიის ფინანსურ განლაგებაში |
| მფლობელები | შესაბამისობა და სამართლებრივი (Daniel Park), Android პროგრამის წამყვანი |

## საკონტროლო მატრიცა

| FISC კონტროლი | განხორციელების დეტალები | მტკიცებულებები / ცნობები | სტატუსი |
|--------------|----------------------|--------------------|--------|
| **სისტემის კონფიგურაციის მთლიანობა** | `ClientConfig` ახორციელებს მანიფესტის ჰეშინგს, სქემის ვალიდაციას და მხოლოდ წაკითხვადი გაშვების წვდომას. კონფიგურაციის ხელახალი ჩატვირთვის წარუმატებლობა ასხივებს `android.telemetry.config.reload` მოვლენებს, რომლებიც დოკუმენტირებულია runbook-ში. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ განხორციელებული |
| **წვდომის კონტროლი და ავტორიზაცია ** | SDK პატივს სცემს Torii TLS პოლიტიკას და `/v2/pipeline` ხელმოწერილ მოთხოვნებს; ოპერატორის სამუშაო ნაკადების მითითება მხარდაჭერა Playbook §4–5 ესკალაციისთვის პლუს გადაფარვის კარიბჭე ხელმოწერილი Norito არტეფაქტების საშუალებით. | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (გადალახეთ სამუშაო პროცესი). | ✅ განხორციელებული |
| **კრიპტოგრაფიული გასაღების მართვა** | StrongBox-ის სასურველი პროვაიდერები, ატესტაციის ვალიდაცია და მოწყობილობის მატრიცის დაფარვა უზრუნველყოფს KMS შესაბამისობას. ატესტაციის აღკაზმულობის შედეგები დაარქივებულია `artifacts/android/attestation/` ქვეშ და თვალყურს ადევნებს მზადყოფნის მატრიცას. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ განხორციელებული |
| ** შესვლა, მონიტორინგი და შეკავება ** | ტელემეტრიის რედაქციის პოლიტიკა ჰეშირებს სენსიტიურ მონაცემებს, ანაწილებს მოწყობილობის ატრიბუტებს და აიძულებს შენარჩუნებას (7/30/90/365-დღიანი ფანჯრები). მხარდაჭერა Playbook §8 აღწერს დაფის ზღურბლებს; `telemetry_override_log.md`-ში ჩაწერილი გადაფარვები. | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ განხორციელებული |
| **ოპერაციები და ცვლილებების მართვა ** | GA ამოღების პროცედურა (მხარდაჭერის Playbook §7.2) პლუს `status.md` განაახლებს ტრეკის გამოშვების მზადყოფნას. გამოშვების მტკიცებულება (SBOM, Sigstore პაკეტები) დაკავშირებულია `docs/source/compliance/android/eu/sbom_attestation.md`-ით. | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ განხორციელებული |
| ** ინციდენტზე რეაგირება და მოხსენება ** | Playbook განსაზღვრავს სიმძიმის მატრიცას, SLA პასუხის ფანჯრებს და შესაბამისობის შეტყობინების ნაბიჯებს; ტელემეტრიის უგულებელყოფა + ქაოსის რეპეტიციები უზრუნველყოფს რეპროდუცირებას პილოტების წინაშე. | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ განხორციელებული |
| **მონაცემთა რეზიდენტობა/ლოკალიზაცია** | ტელემეტრიული კოლექტორები JP განლაგებისთვის მუშაობს დამტკიცებულ ტოკიოს რეგიონში; StrongBox ატესტაციის პაკეტები ინახება რეგიონში და მითითებულია პარტნიორის ბილეთებიდან. ლოკალიზაციის გეგმა უზრუნველყოფს დოკუმენტების ხელმისაწვდომობას იაპონურ ენაზე ბეტამდე (AND5). | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 მიმდინარეობს (მიმდინარეობს ლოკალიზაცია) |

## მიმომხილველი შენიშვნები

- შეამოწმეთ მოწყობილობა-მატრიცის ჩანაწერები Galaxy S23/S24-ისთვის რეგულირებადი პარტნიორის ჩართვამდე (იხილეთ მზადყოფნის დოკუმენტის რიგები `s23-strongbox-a`, `s24-strongbox-a`).
- დარწმუნდით, რომ ტელემეტრიის კოლექტორები JP განლაგებაში ახორციელებენ იგივე შეკავების/გადალახვის ლოგიკას, რომელიც განსაზღვრულია DPIA-ში (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`).
- მიიღეთ დადასტურება გარე აუდიტორებისგან მას შემდეგ, რაც საბანკო პარტნიორები განიხილავენ ამ სიას.