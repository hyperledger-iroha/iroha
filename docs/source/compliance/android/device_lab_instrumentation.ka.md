---
lang: ka
direction: ltr
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2025-12-29T18:16:35.924058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Device Lab Instrumentation Hooks (AND6)

ეს მითითება ხურავს საგზაო რუქის მოქმედებას „დარჩენილი მოწყობილობა-ლაბორატორიის ეტაპი /
ინსტრუმენტული კაკვები AND6 დაწყების წინ“. იგი განმარტავს, თუ როგორ არის დაცული ყველა
მოწყობილობა-ლაბორატორიის სლოტმა უნდა დაიჭიროს ტელემეტრია, რიგი და ატესტაციის არტეფაქტები
AND6 შესაბამისობის საკონტროლო სია, მტკიცებულებების ჟურნალი და მმართველობის პაკეტები იზიარებენ იგივეს
დეტერმინისტული სამუშაო პროცესი. დააკავშირეთ ეს შენიშვნა დაჯავშნის პროცედურასთან
(`device_lab_reservation.md`) და რეპეტიციების დაგეგმვისას წარუმატებლობის რბოლა.

## მიზნები და სფერო

- **დეტერმინისტული მტკიცებულება** - ყველა ინსტრუმენტული გამომავალი მუშაობს
  `artifacts/android/device_lab/<slot-id>/` SHA-256-თან ერთად ასახავს აუდიტორებს
  შეუძლია განასხვავოს პაკეტები ზონდების ხელახლა გაშვების გარეშე.
- **Script-first workflow** – ხელახლა გამოიყენეთ არსებული დამხმარეები
  (`ci/run_android_telemetry_chaos_prep.sh`,
  `scripts/android_keystore_attestation.sh`, `scripts/android_override_tool.sh`)
  შეკვეთილი adb ბრძანებების ნაცვლად.
- **შემოწმების სიები სინქრონიზებული რჩება** – ყოველი გაშვება მიმართავს ამ დოკუმენტს
  AND6 შესაბამისობის საკონტროლო სია და აერთებს არტეფაქტებს
  `docs/source/compliance/android/evidence_log.csv`.

## არტეფაქტის განლაგება

1. აირჩიეთ უნიკალური სლოტის იდენტიფიკატორი, რომელიც ემთხვევა ჯავშნის ბილეთს, მაგ.
   `2026-05-12-slot-a`.
2. დათესეთ სტანდარტული დირექტორიები:

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. შეინახეთ ყველა ბრძანების ჟურნალი შესატყვისი საქაღალდეში (მაგ.
   `telemetry/status.ndjson`, `attestation/pixel8pro.log`).
4. SHA-256-ის გადაღება ვლინდება სლოტის დახურვის შემდეგ:

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## ინსტრუმენტული მატრიცა

| ნაკადი | ბრძანებ(ებ)ი | გამომავალი ადგილმდებარეობა | შენიშვნები |
|------|------------|----------------|-------|
| ტელემეტრიის რედაქცია + სტატუსის ნაკრები | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | გაუშვით სლოტის დასაწყისში და ბოლოს; მიამაგრეთ CLI stdout `status.log`-ზე. |
| მომლოდინე რიგი + ქაოსის მოსამზადებელი | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256` | სარკეების სცენარიD `readiness/labs/telemetry_lab_01.md`-დან; გააფართოვეთ env var სლოტში არსებული ყველა მოწყობილობისთვის. |
| უგულებელყოფა ledger digest | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | საჭიროა მაშინაც კი, როცა ზედმეტად არ არის აქტიური; დაამტკიცეთ ნულოვანი მდგომარეობა. |
| StrongBox / TEE ატესტაცია | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | გაიმეორეთ თითოეული რეზერვირებული მოწყობილობისთვის (შეადარეთ სახელები `android_strongbox_device_matrix.md`-ში). |
| CI აღკაზმულობა ატესტაციის რეგრესია | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | იღებს იგივე მტკიცებულებებს, რასაც CI ატვირთავს; ჩართეთ სიმეტრიისთვის ხელით გაშვებებში. |
| Lint / დამოკიდებულების საბაზისო | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | გაუშვით ერთხელ გაყინვის ფანჯარაში; მონიშნეთ შეჯამება შესაბამისობის პაკეტებში. |

## სტანდარტული სლოტის პროცედურა1. **ფრენის წინ (T-24h)** – დაადასტურეთ, რომ ჯავშნის ბილეთი მიუთითებს ამ
   დოკუმენტი, განაახლეთ მოწყობილობის მატრიცის ჩანაწერი და დათესეთ არტეფაქტის ფესვი.
2. **სლოტის დროს**
   - ჯერ გაუშვით ტელემეტრიის ნაკრები + რიგის ექსპორტის ბრძანებები. საშვი
     `--note <ticket>`-დან `ci/run_android_telemetry_chaos_prep.sh`-მდე, ასე რომ, ჟურნალი
     მიუთითებს ინციდენტის ID.
   - გააქტიურეთ ატესტაციის სკრიპტები თითო მოწყობილობაზე. როდესაც აღკაზმულობა წარმოქმნის ა
     `.zip`, დააკოპირეთ იგი არტეფაქტის ფესვში და ჩაწერეთ Git SHA, რომელიც დაბეჭდილია აქ
     სცენარის დასასრული.
   - შეასრულეთ `make android-lint` გადაცილებული შემაჯამებელი ბილიკით, თუნდაც CI
     უკვე გაიქცა; აუდიტორები ელიან თითო სლოტის ჟურნალს.
3. ** გაშვების შემდგომი **
   - შექმენით `sha256sum.txt` და `README.md` (უფასო ფორმის შენიშვნები) სლოტში
     საქაღალდე, რომელიც აჯამებს შესრულებულ ბრძანებებს.
   - მიამაგრეთ რიგი `docs/source/compliance/android/evidence_log.csv`-ზე
     სლოტის ID, ჰეშის მანიფესტის გზა, Buildkite მითითებები (ასეთის არსებობის შემთხვევაში) და უახლესი
     მოწყობილობა-ლაბორატორიის სიმძლავრის პროცენტი დაჯავშნის კალენდრის ექსპორტიდან.
   - დააკავშირეთ სლოტის საქაღალდე `_android-device-lab` ბილეთში, AND6
     საკონტროლო სია და `docs/source/android_support_playbook.md` გამოშვების ანგარიში.

## წარუმატებლობის დამუშავება და ესკალაცია

- თუ რომელიმე ბრძანება ვერ მოხერხდა, გადაიღეთ stderr გამომავალი `logs/` ქვეშ და მიჰყევით
  ესკალაციის კიბე `device_lab_reservation.md` §6.
- რიგის ან ტელემეტრიის ხარვეზები დაუყოვნებლივ უნდა აღინიშნოს ზედმეტობის სტატუსი
  `docs/source/sdk/android/telemetry_override_log.md` და მიუთითეთ სლოტის ID
  ასე რომ, მმართველობას შეუძლია კვალის მიკვლევა.
- საატესტაციო რეგრესიები უნდა დაფიქსირდეს
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  მოწყობილობის გაუმართავი სერიებით და ზემოთ ჩაწერილი შეკვრის ბილიკებით.

## მოხსენების ჩამონათვალი

სლოტის დასრულებამდე მონიშვნამდე, დარწმუნდით, რომ განახლებულია შემდეგი მითითებები:

- `docs/source/compliance/android/and6_compliance_checklist.md` — მონიშნეთ
  დაასრულეთ ინსტრუმენტების მწკრივი და გაითვალისწინეთ სლოტის ID.
- `docs/source/compliance/android/evidence_log.csv` — ჩანაწერის დამატება/განახლება
  სლოტის ჰეში და სიმძლავრის კითხვა.
- `_android-device-lab` ბილეთი — მიამაგრეთ არტეფაქტის ბმულები და Buildkite ვაკანსიის ID.
- `status.md` — ჩართეთ მოკლე შენიშვნა Android-ის შემდეგ მზადყოფნის დაიჯესტში, ასე რომ
  საგზაო რუქის მკითხველებმა იციან, რომელმა სლოტმა შექმნა უახლესი მტკიცებულებები.

ამ პროცესის შემდეგ ინახავს AND6-ის "მოწყობილობა-ლაბორატორია + ინსტრუმენტული კაკვები"
მაილსტონი აუდიტორულია და ხელს უშლის ხელით განსხვავებას დაჯავშნას, შესრულებას შორის,
და ანგარიში.