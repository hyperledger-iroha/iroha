---
lang: ka
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-05T09:28:11.999717+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android გამოშვების ჩამონათვალი (AND6)

ამ საკონტროლო სიაში მოცემულია **AND6 — CI და შესაბამისობის გამკვრივება** კარიბჭეებიდან
`roadmap.md` (§პრიორიტეტი 5). ის ასწორებს Android SDK გამოშვებებს Rust-თან
გაათავისუფლეთ RFC მოლოდინები CI სამუშაოების, შესაბამისობის არტეფაქტების მართლწერით,
მოწყობილობა-ლაბორატორიული მტკიცებულებები და წარმოშობის პაკეტები, რომლებიც უნდა დაერთოს GA-მდე,
LTS, ან Hotfix მატარებელი წინ მიიწევს.

გამოიყენეთ ეს დოკუმენტი:

- `docs/source/android_support_playbook.md` — გამოშვების კალენდარი, SLA და
  ესკალაციის ხე.
- `docs/source/android_runbook.md` — ყოველდღიური ოპერაციული წიგნები.
- `docs/source/compliance/android/and6_compliance_checklist.md` — რეგულატორი
  არტეფაქტის ინვენტარი.
- `docs/source/release_dual_track_runbook.md` — ორმაგი ტრეკის გამოშვების მართვა.

## 1. სცენის კარიბჭე ერთი შეხედვით

| სცენა | საჭირო კარიბჭე | მტკიცებულება |
|-------|---------------|----------|
| **T−7 დღე (წინასწარ გაყინვა)** | ღამის `ci/run_android_tests.sh` მწვანე 14 დღე; `ci/check_android_fixtures.sh`, `ci/check_android_samples.sh` და `ci/check_android_docs_i18n.sh` გავლა; ლაქების/დამოკიდებულების სკანირება რიგშია. | Buildkite-ის დაფები, მოწყობილობების განსხვავების ანგარიში, სკრინშოტის სნეპშოტების ნიმუში. |
| **T−3 დღე (RC აქცია)** | მოწყობილობა-ლაბორატორიის ჯავშანი დადასტურებულია; StrongBox ატესტაციის CI run (`scripts/android_strongbox_attestation_ci.sh`); რობოელექტრო/ინსტრუმენტული კომპლექტები განხორციელებული დაგეგმილ აპარატურაზე; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` სუფთა. | მოწყობილობის მატრიცა CSV, ატესტაციის ნაკრების მანიფესტი, Gradle ანგარიშები დაარქივებულია `artifacts/android/lint/<version>/` ქვეშ. |
| **T−1 დღე (გასვლა/არ-გასვლა)** | ტელემეტრიის რედაქციის სტატუსის ნაკრები განახლებულია (`scripts/telemetry/check_redaction_status.py --write-cache`); შესაბამისობის არტეფაქტები განახლებულია `and6_compliance_checklist.md`-ზე; დასრულებული წარმოშობის რეპეტიცია (`scripts/android_sbom_provenance.sh --dry-run`). | `docs/source/compliance/android/evidence_log.csv`, ტელემეტრიის სტატუსი JSON, წარმოშობის მშრალი გაშვების ჟურნალი. |
| **T0 (GA/LTS cutover)** | `scripts/publish_android_sdk.sh --dry-run` დასრულებული; წარმოშობა + SBOM ხელმოწერილი; გამოშვების საკონტროლო სია ექსპორტირებული და მიმაგრებულია წასასვლელი/არ წასვლის წუთებზე; `ci/sdk_sorafs_orchestrator.sh` კვამლის სამუშაო მწვანე. | გამოუშვით RFC დანართები, Sigstore პაკეტები, შვილად აყვანის არტეფაქტები `artifacts/android/`-ის ქვეშ. |
| **T+1 დღე (შეწყვეტის შემდგომ)** | Hotfix მზადყოფნა დამოწმებულია (`scripts/publish_android_sdk.sh --validate-bundle`); დაფის განსხვავებები განხილულია (`ci/check_android_dashboard_parity.sh`); მტკიცებულების პაკეტი ატვირთულია `status.md`-ზე. | დაფის განსხვავებების ექსპორტი, ბმული `status.md` ჩანაწერზე, დაარქივებული გამოშვების პაკეტი. |

## 2. CI & Quality Gate Matrix| კარიბჭე | ბრძანებ(ებ)ი / სკრიპტი | შენიშვნები |
|------|-------------------|-------|
| ერთეული + ინტეგრაციის ტესტები | `ci/run_android_tests.sh` (ახვევს `ci/run_android_tests.sh`) | გამოსცემს `artifacts/android/tests/test-summary.json` + ტესტის ჟურნალს. მოიცავს Norito კოდეკს, რიგს, StrongBox სარეზერვო და Torii კლიენტის აღკაზმულობის ტესტებს. საჭიროა ღამით და მონიშვნამდე. |
| ფიქსურის პარიტეტი | `ci/check_android_fixtures.sh` (ახვევს `scripts/check_android_fixtures.py`) | უზრუნველყოფს რეგენერირებული Norito მოწყობილობების შესაბამისობას Rust-ის კანონიკურ კომპლექტთან; მიამაგრეთ JSON განსხვავება, როდესაც კარიბჭე ვერ ხერხდება. |
| აპლიკაციების ნიმუში | `ci/check_android_samples.sh` | აშენებს `examples/android/{operator-console,retail-wallet}` და ამოწმებს ლოკალიზებულ ეკრანის სურათებს `scripts/android_sample_localization.py`-ის საშუალებით. |
| Docs/I18N | `ci/check_android_docs_i18n.sh` | იცავს README + ლოკალიზებულ სწრაფ სტარტებს. ხელახლა გაშვება მას შემდეგ, რაც დოკუმენტის რედაქტირება ხდება გამოშვების ფილიალში. |
| დაფის პარიტეტი | `ci/check_android_dashboard_parity.sh` | ადასტურებს CI/ექსპორტირებული მეტრიკის შესაბამისობას Rust-ის კოლეგებთან; საჭიროა T+1 ვერიფიკაციის დროს. |
| SDK მიღების კვამლი | `ci/sdk_sorafs_orchestrator.sh` | ახორციელებს მრავალ წყაროს Sorafs-ის ორკესტრატორთან დაკავშირებას მიმდინარე SDK-ით. საჭიროა დადგმული არტეფაქტების ატვირთვამდე. |
| ატესტაციის შემოწმება | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | აერთიანებს StrongBox/TEE ატესტაციის პაკეტებს `artifacts/android/attestation/**`-ში; მიამაგრეთ რეზიუმე GA პაკეტებს. |
| მოწყობილობა-ლაბორატორიის სლოტის ვალიდაცია | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | ამოწმებს ხელსაწყოების პაკეტებს პაკეტების გასაშვებად მტკიცებულებების მიმაგრებამდე; CI მუშაობს ნიმუშის სლოტზე `fixtures/android/device_lab/slot-sample`-ში (ტელემეტრია/ატესტაცია/რიგი/ლოგები + `sha256sum.txt`). |

> **მინიშნება:** დაამატეთ ეს სამუშაოები `android-release` Buildkite მილსადენში, რათა
> გაყინვის კვირები ავტომატურად ხელახლა გაუშვით ყველა კარიბჭე გამოშვების ტოტის წვერით.

კონსოლიდირებული `.github/workflows/android-and6.yml` სამუშაო გადის ლინტზე,
სატესტო კომპლექტი, ატესტაცია-შეჯამება და მოწყობილობა-ლაბორატორიის სლოტის შემოწმება ყოველი PR/Push-ზე
Android წყაროების შეხება, მტკიცებულებების ატვირთვა `artifacts/android/{lint,tests,attestation,device_lab}/` ქვეშ.

## 3. Lint & Dependency Scans

გაუშვით `scripts/android_lint_checks.sh --version <semver>` რეპო ძირიდან. The
სკრიპტი ახორციელებს:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- ანგარიშები და დამოკიდებულების დაცვის შედეგები არქივდება ქვეშ
  `artifacts/android/lint/<label>/` და `latest/` სიმბლინკი გამოსაშვებად
  მილსადენები.
- ლაქის აღმოჩენის წარუმატებლობა მოითხოვს ან გამოსწორებას ან ჩანაწერს გამოშვებაში
  მიღებული რისკის დოკუმენტირება RFC (დამტკიცებულია Release Engineering + პროგრამის მიერ
  ტყვია).
- `dependencyGuardBaseline` აღადგენს დამოკიდებულების საკეტს; მიამაგრეთ განსხვავება
  go/no-go პაკეტზე.

## 4. Device Lab & StrongBox დაფარვა

1. დაჯავშნეთ Pixel + Galaxy მოწყობილობები მოყვანილი სიმძლავრის ტრეკერის გამოყენებით
   `docs/source/compliance/android/device_lab_contingency.md`. ბლოკავს გამოშვებებს
   თუ ` ატესტაციის ანგარიშის განახლებისთვის.
3. გაუშვით ინსტრუმენტული მატრიცა (მოწყობილობაში დააკონკრეტეთ კომპლექტი/ABI სია
   ტრეკერი). დააფიქსირეთ წარუმატებლობები ინციდენტების ჟურნალში, თუნდაც განმეორებითი მცდელობები წარმატებული იყოს.
4. შეიტანეთ ბილეთი, თუ საჭიროა Firebase Test Lab-ზე დაბრუნება; ბილეთის დაკავშირება
   ქვემოთ მოცემულ საკონტროლო სიაში.

## 5. შესაბამისობისა და ტელემეტრიის არტეფაქტები- მიჰყევით `docs/source/compliance/android/and6_compliance_checklist.md` ევროკავშირისთვის
  და JP წარდგინებები. განაახლეთ `docs/source/compliance/android/evidence_log.csv`
  ჰეშებით + Buildkite სამუშაო URL-ებით.
- განაახლეთ ტელემეტრიული რედაქციის მტკიცებულებები მეშვეობით
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`.
  შეინახეთ მიღებული JSON ქვეშ
  `artifacts/android/telemetry/<version>/status.json`.
- ჩაწერეთ სქემის განსხვავების გამომავალი
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  Rust-ის ექსპორტიორებთან თანასწორობის დასამტკიცებლად.

## 6. წარმოშობა, SBOM და გამოცემა

1. მშრალად გაუშვით გამოქვეყნების მილსადენი:

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. შექმენით SBOM + Sigstore წარმოშობა:

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. მიამაგრეთ `artifacts/android/provenance/<semver>/manifest.json` და ხელმოწერილი
   `checksums.sha256` RFC გამოშვებამდე.
4. Maven-ის რეალურ საცავში დაწინაურებისას ხელახლა გაუშვით
   `scripts/publish_android_sdk.sh` `--dry-run`-ის გარეშე, გადაიღეთ კონსოლი
   შედით სისტემაში და ატვირთეთ მიღებული არტეფაქტები `artifacts/android/maven/<semver>`-ზე.

## 7. წარდგენის პაკეტის შაბლონი

ყოველი GA/LTS/hotfix გამოშვება უნდა შეიცავდეს:

1. **შესრულებული საკონტროლო სია** — დააკოპირეთ ამ ფაილის ცხრილი, მონიშნეთ თითოეული ელემენტი და ბმული
   არტეფაქტების მხარდასაჭერად (Buildkite run, ჟურნალები, doc diffs).
2. **მოწყობილობის ლაბორატორიის მტკიცებულება** — ატესტაციის ანგარიშის შეჯამება, დაჯავშნის ჟურნალი და
   ნებისმიერი საგანგებო გააქტიურება.
3. **ტელემეტრიის პაკეტი** — რედაქციის სტატუსი JSON, სქემის განსხვავება, ბმული
   `docs/source/sdk/android/telemetry_redaction.md` განახლებები (ასეთის არსებობის შემთხვევაში).
4. **შესაბამისობის არტეფაქტები** — ჩანაწერები დამატებულია/განახლებულია შესაბამისობის საქაღალდეში
   პლუს განახლებული მტკიცებულების ჟურნალი CSV.
5. **წარმოშობის ნაკრები ** — SBOM, Sigstore ხელმოწერა და `checksums.sha256`.
6. **გამოცემის შეჯამება** — ერთგვერდიანი მიმოხილვა დართულია `status.md` შეჯამებაზე
   ზემოთ (თარიღი, ვერსია, ნებისმიერი მოხსნილი კარიბჭის ხაზგასმა).

შეინახეთ პაკეტი `artifacts/android/releases/<version>/` ქვეშ და მიმართეთ მას
`status.md`-ში და RFC გამოშვებაში.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` ავტომატურად
  კოპირებს უახლეს ლინტ არქივს (`artifacts/android/lint/latest`) და
  შესაბამისობის მტკიცებულება შედით `artifacts/android/releases/<version>/`-ში, ასე რომ
  წარდგენის პაკეტს ყოველთვის აქვს კანონიკური მდებარეობა.

---

**შეხსენება:** განაახლეთ ეს სია, როდესაც ახალი CI სამუშაოები, შესაბამისობის არტეფაქტები,
ან ტელემეტრიის მოთხოვნები ემატება. საგზაო რუკის პუნქტი AND6 ღია რჩება მანამ
საკონტროლო სია და მასთან დაკავშირებული ავტომატიზაცია სტაბილურია ზედიზედ ორი გამოშვებისთვის
მატარებლები.