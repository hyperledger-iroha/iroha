---
lang: ka
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 676798a4cf7c3e7737a0f80640f3f268a2f625f92afdd359ac528881d2aeb046
source_last_modified: "2025-12-29T18:16:35.060950+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android დოკუმენტაციის ავტომატიზაციის საბაზისო ხაზი (AND5)

საგზაო რუკის პუნქტი AND5 საჭიროებს დოკუმენტაციას, ლოკალიზაციას და გამოქვეყნებას
ავტომატიზაცია უნდა იყოს შემოწმებული AND6 (CI და შესაბამისობა) დაწყებამდე. ეს საქაღალდე
ჩაწერს ბრძანებებს, არტეფაქტებს და მტკიცებულებების განლაგებას, რომლებიც AND5/AND6 მიუთითებს,
აღბეჭდილი გეგმების ასახვა
`docs/source/sdk/android/developer_experience_plan.md` და
`docs/source/sdk/android/parity_dashboard_plan.md`.

## მილსადენები და ბრძანებები

| ამოცანა | ბრძანებ(ებ)ი | მოსალოდნელი არტეფაქტები | შენიშვნები |
|------|------------|-------------------|------|
| ლოკალიზაციის stub sync | `python3 scripts/sync_docs_i18n.py` (სურვილისამებრ გაივლის `--lang <code>` პერსპექტივაში) | `docs/automation/android/i18n/<timestamp>-sync.log`-ში შენახული ჟურნალის ფაილი, პლუს ნათარგმნი ნაკერი, ჩადენილია | ინახავს `docs/i18n/manifest.json` სინქრონიზებულს თარგმნილ ნაკერებთან; ჟურნალი აფიქსირებს ენის კოდებს, რომლებიც შეეხო და git commit აღბეჭდილია საწყისში. |
| Norito სამაგრი + პარიტეტის შემოწმება | `ci/check_android_fixtures.sh` (ახვევს `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`) | დააკოპირეთ გენერირებული შემაჯამებელი JSON `docs/automation/android/parity/<stamp>-summary.json` | ამოწმებს `java/iroha_android/src/test/resources` დატვირთვას, მანიფესტის ჰეშებს და ხელმოწერილი მოწყობილობების სიგრძეს. მიამაგრეთ რეზიუმე კადენციის მტკიცებულებასთან ერთად `artifacts/android/fixture_runs/`-ში. |
| მანიფესტისა და გამოქვეყნების მტკიცებულების ნიმუში | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (აწარმოებს ტესტებს + SBOM + წარმოშობა) | წარმოშობის ნაკრების მეტამონაცემები პლუს `sample_manifest.json` `docs/source/sdk/android/samples/`-დან, რომელიც ინახება `docs/automation/android/samples/<version>/`-ში | აკავშირებს AND5 აპების ნიმუშებს და ათავისუფლებს ავტომატიზაციას - აიღეთ გენერირებული მანიფესტი, SBOM ჰეში და წარმოშობის ჟურნალი ბეტა მიმოხილვისთვის. |
| პარიტეტული დაფის არხი | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` მოჰყვება `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | დააკოპირეთ `metrics.prom` სნეპშოტი ან Grafana ექსპორტი JSON `docs/automation/android/parity/<stamp>-metrics.prom`-ში | კვებავს საინფორმაციო დაფის გეგმას, რათა AND5/AND7 მმართველობამ გადაამოწმოს არასწორი წარდგენის მრიცხველები და ტელემეტრიის მიღება. |

## მტკიცებულებების აღება

1. **ყველაფერზე დროის ანაბეჭდი.** დაასახელეთ ფაილები UTC დროის ანაბეჭდების გამოყენებით
   (`YYYYMMDDTHHMMSSZ`) ასე რომ, პარიტეტის საინფორმაციო დაფები, მმართველობის ოქმები და გამოქვეყნებული
   დოკუმენტებს შეუძლიათ განსაზღვრონ ერთი და იგივე გაშვება.
2. **მინიშნება commits.** თითოეული ჟურნალი უნდა შეიცავდეს გაშვების git commit ჰეშის
   პლუს ნებისმიერი შესაბამისი კონფიგურაცია (მაგ., `ANDROID_PARITY_PIPELINE_METADATA`).
   როდესაც კონფიდენციალურობა მოითხოვს რედაქტირებას, შეიტანეთ შენიშვნა და ბმული უსაფრთხო სარდაფთან.
3. ** დაარქივეთ მინიმალური კონტექსტი. ** ჩვენ ვამოწმებთ მხოლოდ სტრუქტურირებულ შეჯამებებს (JSON,
   `.prom`, `.log`). მძიმე არტეფაქტები (APK პაკეტები, ეკრანის ანაბეჭდები) უნდა დარჩეს
   `artifacts/` ან ობიექტის შენახვა ჟურნალში ჩაწერილი ხელმოწერილი ჰეშით.
4. ** განაახლეთ სტატუსის ჩანაწერები.** როდესაც AND5 ეტაპები წინ მიიწევს `status.md`-ში, ციტირება
   შესაბამისი ფაილი (მაგ., `docs/automation/android/parity/20260324T010203Z-summary.json`)
   ასე რომ, აუდიტორებს შეუძლიათ საბაზისო ხაზების მიკვლევა CI ჟურნალების გახეხვის გარეშე.

ამ განლაგების შემდეგ აკმაყოფილებს „დოკუმენტების/ავტომატიზაციის საბაზისო ხაზებს, რომლებიც ხელმისაწვდომია
აუდიტი” წინაპირობა, რომელიც AND6-ს მოჰყავს და ინახავს Android დოკუმენტაციის პროგრამას
გამოქვეყნებულ გეგმებთან ერთად.