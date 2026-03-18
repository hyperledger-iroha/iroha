---
lang: ka
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS metrics & onboarding kit
description: Dashboard, pricing, and automation artifacts referenced by roadmap item SN-8.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SNS Metrics & Inboarding Kit

საგზაო რუკის პუნქტი **SN-8** აერთიანებს ორ დაპირებას:

1. გამოაქვეყნეთ დაფები, რომლებიც ასახავს რეგისტრაციას, განახლებას, ARPU-ს, დავებს და
   გაყინეთ ფანჯრები `.sora`, `.nexus` და `.dao`.
2. გაგზავნეთ საბორტო ნაკრები, რათა რეგისტრატორებმა და სტიუარდებმა შეძლონ DNS-ის, ფასების და ა.შ.
   API-ები თანმიმდევრულად, სანამ რაიმე სუფიქსი გამოვა.

ეს გვერდი ასახავს წყაროს ვერსიას
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
ასე რომ, გარე რეცენზენტებს შეუძლიათ იგივე პროცედურის შესრულება.

## 1. მეტრიკული პაკეტი

### Grafana დაფის და პორტალის ჩაშენება

- იმპორტი `dashboards/grafana/sns_suffix_analytics.json` Grafana-ში (ან სხვა
  ანალიტიკის ჰოსტი) სტანდარტული API-ის მეშვეობით:

```bash
curl -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${GRAFANA_TOKEN}" \
     -X POST https://grafana.sora.net/api/dashboards/db \
     --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- იგივე JSON უზრუნველყოფს ამ პორტალის გვერდის iframe-ს (იხ. **SNS KPI Dashboard**).
  ყოველთვის, როცა დაფაზე შეხვალთ, გაუშვით
  `npm run build && npm run serve-verified-preview` შიგნით `docs/portal`
  დაადასტურეთ როგორც Grafana, ასევე ჩაშენებული დარჩენა სინქრონიზებული.

### პანელები და მტკიცებულებები

| პანელი | მეტრიკა | მმართველობის მტკიცებულება |
|-------|---------|--------------------|
| რეგისტრაცია და განახლება | `sns_registrar_status_total` (წარმატება + განახლების გადამწყვეტი ეტიკეტები) | თითო სუფიქსის გამტარუნარიანობა + SLA თვალთვალი. |
| ARPU / წმინდა ერთეული | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | ფინანსებს შეუძლია შეესაბამებოდეს რეგისტრატორის მანიფესტებს შემოსავალს. |
| დავები და ყინვები | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | აჩვენებს აქტიურ გაყინვას, საარბიტრაჟო კადენციას და მეურვის დატვირთვას. |
| SLA/შეცდომის განაკვეთები | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | ხაზს უსვამს API რეგრესიებს, სანამ ისინი გავლენას მოახდენენ მომხმარებლებზე. |
| მასობრივი მანიფესტის ტრეკერი | `sns_bulk_release_manifest_total`, გადახდის მეტრიკა `manifest_id` ეტიკეტებით | აკავშირებს CSV წვეთებს ანგარიშსწორების ბილეთებთან. |

PDF/CSV-ის ექსპორტი Grafana-დან (ან ჩაშენებული iframe) ყოველთვიური KPI-ის განმავლობაში
განიხილავს და დაურთოს შესაბამის დანართს ქვემოთ
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. სტიუარდებმა ასევე დაიჭირეს SHA-256
ექსპორტირებული პაკეტის `docs/source/sns/reports/` ქვეშ (მაგალითად,
`steward_scorecard_2026q1.md`), რათა აუდიტმა შეძლოს მტკიცებულების ბილიკის ხელახალი თამაში.

### დანართის ავტომატიზაცია

შექმენით დანართის ფაილები პირდაპირ დაფის ექსპორტიდან, რათა მიმომხილველებმა მიიღონ ა
თანმიმდევრული დაიჯესტი:

```bash
cargo xtask sns-annex \
  --suffix .sora \
  --cycle 2026-03 \
  --dashboard dashboards/grafana/sns_suffix_analytics.json \
  --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json \
  --output docs/source/sns/reports/.sora/2026-03.md \
  --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md \
  --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- დამხმარე ჰაშებს ექსპორტს, იღებს UID/ტეგების/პანელის რაოდენობას და წერს
  მარკდაუნის დანართი `docs/source/sns/reports/.<suffix>/<cycle>.md` ქვეშ (იხ
  `.sora/2026-03` ნიმუში ჩადენილი ამ დოკუმენტთან ერთად).
- `--dashboard-artifact` აკოპირებს ექსპორტს
  `artifacts/sns/regulatory/<suffix>/<cycle>/` ამიტომ დანართში მითითებულია
  კანონიკური მტკიცებულების გზა; გამოიყენეთ `--dashboard-label` მხოლოდ მაშინ, როცა უნდა მიუთითოთ
  ჯგუფურ არქივში.
- `--regulatory-entry` მიუთითებს მმართველ მემორანდუმზე. დამხმარე ჩანართები (ან
  ცვლის) `KPI Dashboard Annex` ბლოკი, რომელიც ჩაწერს დანართის გზას, დაფა
  არტეფაქტი, დაიჯესტი და დროის შტამპი, ასე რომ მტკიცებულებები სინქრონიზებული რჩება ხელახალი გაშვების შემდეგ.
- `--portal-entry` ინახავს Docusaurus ასლს (`docs/portal/docs/sns/regulatory/*.md`)
  გასწორებულია ისე, რომ მიმომხილველებს არ სჭირდებათ ცალკეული დანართების შეჯამების ხელით განსხვავება.
- თუ გამოტოვებთ `--regulatory-entry`/`--portal-entry`, მიამაგრეთ გენერირებული ფაილი
  შენიშვნები ხელით და მაინც ატვირთეთ PDF/CSV კადრები, რომლებიც გადაღებულია Grafana-დან.
- განმეორებადი ექსპორტისთვის, ჩამოთვალეთ სუფიქსი/ციკლის წყვილები
  `docs/source/sns/regulatory/annex_jobs.json` და გაუშვით
  `python3 scripts/run_sns_annex_jobs.py --verbose`. დამხმარე დადის ყოველ შესასვლელში,
  აკოპირებს დაფის ექსპორტს (ნაგულისხმევად `dashboards/grafana/sns_suffix_analytics.json`
  როდესაც დაუზუსტებელია) და განაახლებს დანართის ბლოკს თითოეული მარეგულირებელი ორგანოს შიგნით (და,
  როდესაც ხელმისაწვდომია, პორტალი) მემორანდუმი ერთი პასით.
- გაუშვით `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (ან `make check-sns-annex`), რათა დაამტკიცოთ, რომ ვაკანსიების სია დალაგებულია/მოშლილი რჩება, თითოეული მემორანდუმი ატარებს შესატყვისი `sns-annex` მარკერის და დანართის ნაკერი არსებობს. დამხმარე წერს `artifacts/sns/annex_schedule_summary.json` ლოკალის/ჰაშის შეჯამების გვერდით, რომლებიც გამოიყენება მართვის პაკეტებში.
ეს წაშლის მექანიკური კოპირების/ჩასმის ნაბიჯებს და ინარჩუნებს SN-8 დანართის მტკიცებულებებს თანმიმდევრულად
დაცვის განრიგი, მარკერი და ლოკალიზაციის დრიფტი CI-ში.

## 2. საბორტო ნაკრების კომპონენტები

### სუფიქსის გაყვანილობა

- რეესტრის სქემა + შერჩევის წესები:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  და [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md).
- DNS ჩონჩხის დამხმარე:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  სარეპეტიციო ნაკადით დატყვევებული
  [Gateway/DNS runbook] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md).
- რეგისტრატორის ყოველი გაშვებისთვის, შეიტანეთ მოკლე შენიშვნა ქვემოთ
  `docs/source/sns/reports/` აჯამებს სელექტორის ნიმუშებს, GAR მტკიცებულებებს და DNS ჰეშებს.

### ფასების თაღლითური ცხრილი

| ეტიკეტის სიგრძე | საბაზისო გადასახადი (USD ეკვივა) |
|--------------|--------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6–9 | $12 |
| 10+ | $8 |

სუფიქსის კოეფიციენტები: `.sora` = 1.0×, `.nexus` = 0.8×, `.dao` = 1.3×.  
ვადის მულტიპლიკატორები: 2-წლიანი -5%, 5-წლიანი -12%; მადლის ფანჯარა = 30 დღე, გამოსყიდვა
= 60 დღე (20% საკომისიო, მინიმუმ $5, მაქსიმუმ $200). ჩაწერეთ შეთანხმებული გადახრები
რეგისტრატორის ბილეთი.

### პრემიუმ აუქციონები განახლებების წინააღმდეგ

1. **პრემიუმ აუზი** — დალუქული შეთავაზების ვალდებულება/გამოვლენა (SN-3). აკონტროლეთ შეთავაზებები
   `sns_premium_commit_total` და გამოაქვეყნეთ მანიფესტი ქვეშ
   `docs/source/sns/reports/`.
2. **ჰოლანდიის ხელახლა გახსნა** — შეღავათი + გამოსყიდვის ვადის ამოწურვის შემდეგ დაიწყეთ 7-დღიანი ჰოლანდიური გაყიდვა
   10×ზე, რომელიც იშლება 15% დღეში. ლეიბლი გამოიხატება `manifest_id`-ით ასე რომ
   დაფაზე შეუძლია პროგრესი.
3. **განახლებები** — მონიტორი `sns_registrar_status_total{resolver="renewal"}` და
   გადაიღეთ ავტორიზებული განახლების საკონტროლო სია (შეტყობინებები, SLA, გადახდის რელსები)
   რეგისტრატორის ბილეთის შიგნით.

### დეველოპერის API და ავტომატიზაცია

- API კონტრაქტები: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- ნაყარი დამხმარე და CSV სქემა:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- ბრძანების მაგალითი:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --ndjson artifacts/sns/releases/2026q2/requests.ndjson \
  --submission-log artifacts/sns/releases/2026q2/submissions.log \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token
```

შეიყვანეთ manifest ID (`--submission-log` გამომავალი) KPI დაფის ფილტრში
ასე რომ, ფინანსებს შეუძლია შემოსავლების პანელების შეჯერება თითო გამოშვებაზე.

### მტკიცებულებათა ნაკრები

1. რეგისტრატორის ბილეთი კონტაქტებით, სუფიქსის ფარგლებით და გადახდის რელსებით.
2. DNS/გამხსნელის მტკიცებულება (zonefile ჩონჩხები + GAR მტკიცებულებები).
3. ფასების სამუშაო ფურცელი + მმართველობის მიერ დამტკიცებული ნებისმიერი უგულებელყოფა.
4. API/CLI კვამლის ტესტის არტეფაქტები (`curl` ნიმუშები, CLI ტრანსკრიპტები).
5. KPI დაფის სკრინშოტი + CSV ექსპორტი, თან ერთვის ყოველთვიურ დანართს.

## 3. გაუშვით საკონტროლო სია

| ნაბიჯი | მფლობელი | არტეფაქტი |
|------|-------|----------|
| დაფა შემოტანილია | პროდუქტის ანალიტიკა | Grafana API პასუხი + დაფის UID |
| პორტალის ჩაშენება დადასტურებულია | Docs/DevRel | `npm run build` ჟურნალები + გადახედვის ეკრანის სურათი |
| DNS-ის რეპეტიცია დასრულდა | ქსელი/ოპერაციები | `sns_zonefile_skeleton.py` გამომავალი + runbook ჟურნალი |
| რეგისტრატორის ავტომატიზაციის მშრალი გაშვება | რეგისტრატორი Eng | `sns_bulk_onboard.py` წარდგენის ჟურნალი |
| წარდგენილია მმართველობის მტკიცებულებები | მმართველობის საბჭო | დანართის ბმული + ექსპორტირებული დაფის SHA-256 |

შეავსეთ ჩამონათვალი რეგისტრატორის ან სუფიქსის გააქტიურებამდე. ხელმოწერილი
Bundle ასუფთავებს SN-8 საგზაო რუქის კარიბჭეს და აუდიტორებს აძლევს ერთ მითითებას როდის
ბაზარზე გაშვების მიმოხილვა.