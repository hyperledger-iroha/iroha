---
lang: ka
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf9773ecd75fc31ee89da58a3c5eda846b910eb6e131f1e042b565892e028f16
source_last_modified: "2025-12-29T18:16:35.062011+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SDK Binding & Fixture Governance

WP1-E საგზაო რუკაზე მოუწოდებს „დოკუმენტებს/საკინძებს“, როგორც კანონიკურ ადგილს შესანახად
ენათაშორისი სავალდებულო მდგომარეობა. ეს დოკუმენტი აღრიცხავს სავალდებულო ინვენტარს,
რეგენერაციის ბრძანებები, დრიფტის მცველები და მტკიცებულებების მდებარეობები GPU პარიტეტის შესაბამისად
კარიბჭეებს (WP1-E/F/G) და cross-SDK cadence საბჭოს აქვს ერთი მითითება.

## საერთო ფარები
- **კანონიკური სათამაშო წიგნი:** `docs/source/norito_binding_regen_playbook.md` წერია
  როტაციის პოლიტიკა, მოსალოდნელი მტკიცებულებები და ესკალაციის სამუშაო პროცესი Android-ისთვის,
  სვიფტი, პითონი და მომავალი აკინძები.
- **Norito სქემის პარიტეტი:** `scripts/check_norito_bindings_sync.py` (გამოძახებული მეშვეობით
  `scripts/check_norito_bindings_sync.sh` და კარიბჭე CI მიერ
  `ci/check_norito_bindings_sync.sh`) ბლოკავს აშენებას Rust, Java ან Python
  სქემის არტეფაქტების დრიფტი.
- **Cadence watchdog:** `scripts/check_fixture_cadence.py` კითხულობს
  `artifacts/*_fixture_regen_state.json` ფაილებს და ახორციელებს სამ/პარას (Android,
  Python) და Wed (Swift) ფანჯრები, ამიტომ საგზაო რუქის კარიბჭეებს აქვთ აუდიტორული დროის ანაბეჭდები.

## სავალდებულო მატრიცა

| სავალდებულო | შესვლის პუნქტები | Fixture / regen ბრძანება | დრიფტის მცველები | მტკიცებულება |
|---------|--------------------------------------|------------|---------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (სურვილისამებრ `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| პითონი | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## სავალდებულო დეტალები

### Android (Java)
Android SDK მუშაობს `java/iroha_android/`-ის ქვეშ და მოიხმარს კანონიკურ Norito-ს
`scripts/android_fixture_regen.sh`-ის მიერ წარმოებული მოწყობილობები. რომ დამხმარე ექსპორტს
ახალი `.norito` blobs Rust ინსტრუმენტთა ჯაჭვიდან, განახლებები
`artifacts/android_fixture_regen_state.json` და აღრიცხავს კადენციის მეტამონაცემებს, რომლებიც
`scripts/check_fixture_cadence.py` და მართვის დაფები მოიხმარს. დრიფტი არის
აღმოჩენილია `scripts/check_android_fixtures.py`-ის მიერ (ასევე ჩართული
`ci/check_android_fixtures.sh`) და `java/iroha_android/run_tests.sh`, რომელიც
ავარჯიშებს JNI აკინძებს, WorkManager რიგის განმეორებას და StrongBox ჩანაცვლებებს.
როტაციის მტკიცებულებები, წარუმატებლობის შენიშვნები და ხელახალი ჩანაწერები ცოცხალია
`artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
`IrohaSwift/` ასახავს იგივე Norito დატვირთვას `scripts/swift_fixture_regen.sh`-ის მეშვეობით.
სკრიპტი ჩაწერს ბრუნვის მფლობელს, კადენციის ლეიბლს და წყაროს (`live` vs `archive`)
შიგნით `artifacts/swift_fixture_regen_state.json` და აწვდის მეტამონაცემებს
cadence checker. `scripts/swift_fixture_archive.py` საშუალებას აძლევს შემნახველებს გადაყლაპონ
ჟანგით წარმოქმნილი არქივები; `scripts/check_swift_fixtures.py` და
`ci/check_swift_fixtures.sh` აღასრულებს ბაიტის დონის პარიტეტს პლუს SLA ასაკობრივი ლიმიტები, ხოლო
`scripts/swift_fixture_regen.sh` მხარს უჭერს `SWIFT_FIXTURE_EVENT_TRIGGER` სახელმძღვანელოს
როტაციები. ესკალაციის სამუშაო პროცესი, KPI-ები და დაფები დოკუმენტირებულია
`docs/source/swift_parity_triage.md` და cadence ბრიფები ქვეშ
`docs/source/sdk/swift/`.

### პითონი
Python კლიენტი (`python/iroha_python/`) იზიარებს Android-ის პროგრამებს. სირბილი
`scripts/python_fixture_regen.sh` იზიდავს უახლეს `.norito` დატვირთვას, განაახლებს
`python/iroha_python/tests/fixtures/` და გამოსცემს კადენციის მეტამონაცემებს
`artifacts/python_fixture_regen_state.json` ერთხელ პირველი საგზაო რუქის როტაცია
დატყვევებულია. `scripts/check_python_fixtures.py` და
`python/iroha_python/scripts/run_checks.sh` კარიბჭის pytest, mypy, ruff და მოწყობილობა
პარიტეტი ადგილობრივად და CI-ში. ბოლო-ბოლო დოკუმენტები (`docs/source/sdk/python/…`) და
დამაკავშირებელი რეგენის სათამაშო წიგნში აღწერილია, თუ როგორ უნდა კოორდინირდეს ბრუნვა Android-თან
მფლობელები.

### JavaScript
`javascript/iroha_js/` არ ეყრდნობა ადგილობრივ `.norito` ფაილებს, მაგრამ WP1-E ტრეკებს
მისი გამოშვების მტკიცებულება, ამიტომ GPU CI ზოლები მემკვიდრეობით იღებენ სრულ წარმოშობას. ყოველი გამოშვება
იჭერს წარმომავლობას `npm run release:provenance`-ის საშუალებით (მხარდაჭერით
`javascript/iroha_js/scripts/record-release-provenance.mjs`), ქმნის და ხელს აწერს
SBOM პაკეტები `scripts/js_sbom_provenance.sh`-ით, აწარმოებს ხელმოწერილი დადგმის მშრალ გაშვებას
(`scripts/js_signed_staging.sh`) და ამოწმებს რეესტრის არტეფაქტს
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. შედეგად მიღებული მეტამონაცემები
მიწები `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/` ქვეშ,
`artifacts/js/sbom/` და `artifacts/js/verification/`, რომლებიც უზრუნველყოფენ დეტერმინისტულ
მტკიცებულება საგზაო რუქის JS5/JS6 და WP1-F საორიენტაციო გაშვებისთვის. გამომცემლობა წიგნში
`docs/source/sdk/js/` აკავშირებს ავტომატიზაციას.