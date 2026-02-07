---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b80573de9799c783b62fe4babb553de4dd0778b028cd6d6ad58eb3094f7284eb
source_last_modified: "2026-01-04T08:19:26.497389+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Provider Advert Rollout Plan"
translator: machine-google-reviewed
---

> ადაპტირებულია [`docs/source/sorafs/provider_advert_rollout.md`]-დან (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# SoraFS პროვაიდერის რეკლამის გავრცელების გეგმა

ეს გეგმა კოორდინაციას უწევს ნებადართული პროვაიდერის რეკლამების შეწყვეტას
სრულად მართული `ProviderAdvertV1` ზედაპირი, რომელიც საჭიროა მრავალ წყაროს ნაჭრისთვის
მოძიება. იგი ფოკუსირებულია სამ მიწოდებაზე:

- **ოპერატორის სახელმძღვანელო.** ნაბიჯ-ნაბიჯ მოქმედებები შენახვის პროვაიდერებმა უნდა შეასრულონ
  სანამ ყოველი ჭიშკარი შემობრუნდება.
- **ტელემეტრიის დაფარვა.** დაფები და გაფრთხილებები, რომლებსაც იყენებს Observability და Ops
  იმის დასადასტურებლად, რომ ქსელი იღებს მხოლოდ შესაბამის რეკლამებს.
გაშვება შეესაბამება SF-2b/2c ეტაპებს [SoraFS მიგრაციაში
საგზაო რუკა](./migration-roadmap) და იღებს დაშვების პოლიტიკას
[პროვაიდერის დაშვების პოლიტიკა] (./provider-admission-policy) უკვე შედის
ეფექტი.

## მიმდინარე მოთხოვნები

SoraFS იღებს მხოლოდ მართვის კონვერტირებულ `ProviderAdvertV1` დატვირთვას. The
მიღებისას შემდეგი მოთხოვნები სრულდება:

- `profile_id=sorafs.sf1@1.0.0` კანონიკური `profile_aliases`-ით.
- `chunk_range_fetch` შესაძლებლობების დატვირთვა უნდა იყოს ჩართული მრავალ წყაროსთვის
  მოძიება.
- `signature_strict=true` რეკლამას დართული საბჭოს ხელმოწერებით
  კონვერტი.
- `allow_unknown_capabilities` ნებადართულია მხოლოდ GREASE წვრთნების დროს
  და უნდა იყოს შესული.

## ოპერატორის ჩამონათვალი

1. **ინვენტარის რეკლამები.** ჩამოთვალეთ ყველა გამოქვეყნებული განცხადება და ჩაწერეთ:
   - მმართველი კონვერტის გზა (`defaults/nexus/sorafs_admission/...` ან წარმოების ექვივალენტი).
   - რეკლამა `profile_id` და `profile_aliases`.
   - შესაძლებლობების სია (მოველით მინიმუმ `torii_gateway` და `chunk_range_fetch`).
   - `allow_unknown_capabilities` დროშა (აუცილებელია, როდესაც არსებობს გამყიდველის მიერ დაჯავშნილი TLVs).
2. **რეგენერაცია პროვაიდერის ხელსაწყოებით.**
   - აღადგინეთ დატვირთვა თქვენი პროვაიდერის რეკლამის გამომცემელთან, რაც უზრუნველყოფს:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` განსაზღვრული `max_span`-ით
     - `allow_unknown_capabilities=<true|false>` GREASE TLV-ების არსებობისას
   - გადამოწმება `/v1/sorafs/providers` და `sorafs_fetch` საშუალებით; გაფრთხილებები უცნობის შესახებ
     შესაძლებლობები უნდა იყოს ტრიაჟირებული.
3. **მრავალწყაროების მზადყოფნის დადასტურება.**
   - შეასრულეთ `sorafs_fetch` `--provider-advert=<path>`-ით; CLI ახლა მარცხდება
     როდესაც `chunk_range_fetch` აკლია და ბეჭდავს გაფრთხილებებს იგნორირებული უცნობისთვის
     შესაძლებლობები. გადაიღეთ JSON ანგარიში და დაარქივეთ იგი ოპერაციების ჟურნალებით.
4. **სცენის განახლება.**
   - გაგზავნეთ `ProviderAdmissionRenewalV1` კონვერტები მინიმუმ 30 დღით ადრე
     ვადის გასვლა. განახლებამ უნდა შეინარჩუნოს კანონიკური სახელური და შესაძლებლობების ნაკრები;
     უნდა შეიცვალოს მხოლოდ ფსონი, საბოლოო წერტილები ან მეტამონაცემები.
5. **დამოკიდებულ გუნდებთან კომუნიკაცია.**
   - SDK-ის მფლობელებმა უნდა გამოუშვან ვერსიები, რომლებიც აფრთხილებენ ოპერატორებს
     რეკლამები უარყოფილია.
   - DevRel აცხადებს თითოეულ ფაზაზე გადასვლას; მოიცავს დაფის ბმულებს და
     ბარიერის ლოგიკა ქვემოთ.
6. ** დააინსტალირეთ დაფები და გაფრთხილებები.**
   - შემოიტანეთ Grafana ექსპორტი და განათავსეთ იგი **SoraFS / პროვაიდერის ქვეშ
     გაშვება** დაფის UID `sorafs-provider-admission`-ით.
   - დარწმუნდით, რომ გაფრთხილების წესები მიუთითებს გაზიარებულ `sorafs-advert-rollout`-ზე
     შეტყობინებების არხი დადგმასა და წარმოებაში.

## ტელემეტრია და დაფები

შემდეგი მეტრიკა უკვე გამოქვეყნებულია `iroha_telemetry`-ის მეშვეობით:

- `torii_sorafs_admission_total{result,reason}` — ითვლები მიღებული, უარყოფილი,
  და გამაფრთხილებელი შედეგები. მიზეზები მოიცავს `missing_envelope`, `unknown_capability`,
  `stale` და `policy_violation`.

Grafana ექსპორტი: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
ფაილის იმპორტი საზიარო დაფების საცავში (`observability/dashboards`)
და განაახლეთ მხოლოდ მონაცემთა წყაროს UID გამოქვეყნებამდე.

დაფა აქვეყნებს Grafana საქაღალდეში **SoraFS / Provider Rollout**
სტაბილური UID `sorafs-provider-admission`. გაფრთხილების წესები
`sorafs-admission-warn` (გაფრთხილება) და `sorafs-admission-reject` (კრიტიკული) არის
წინასწარ კონფიგურირებული `sorafs-advert-rollout` შეტყობინებების პოლიტიკის გამოსაყენებლად; მორგება
ეს საკონტაქტო წერტილი, თუ დანიშნულების სია შეიცვლება, ვიდრე რედაქტირება
დაფა JSON.

რეკომენდებული Grafana პანელები:

| პანელი | შეკითხვა | შენიშვნები |
|-------|-------|-------|
| **მიღებების შედეგის მაჩვენებელი** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | დაწყობა დიაგრამა ვიზუალურად მიღება და გაფრთხილება წინააღმდეგ უარყოფა. გაფრთხილება, როდესაც გაფრთხილება > 0,05 * სულ (გაფრთხილება) ან უარყოფა > 0 (კრიტიკული). |
| **გაფრთხილების კოეფიციენტი** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | ერთხაზიანი დროის სერია, რომელიც კვებავს პეიჯერის ზღურბლს (5% გაფრთხილების სიჩქარე 15 წუთის განმავლობაში). |
| **უარყოფის მიზეზები** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | მართავს runbook ტრიაჟს; მიამაგრეთ ბმულები შემარბილებელი ნაბიჯების შესახებ. |
| **ვალების განახლება** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | მიუთითებს პროვაიდერებს, რომლებიც გამოტოვებენ განახლების ვადას; ჯვარედინი მითითება აღმოჩენის ქეში ჟურნალებთან. |

CLI არტეფაქტები მექანიკური დაფებისთვის:

- `sorafs_fetch --provider-metrics-out` წერს `failures`, `successes` და
  `disabled` მრიცხველი თითო პროვაიდერზე. იმპორტი ad-hoc დაფებში მონიტორინგისთვის
  ორკესტრი მშრალ აწარმოებს წარმოების პროვაიდერების შეცვლამდე.
- JSON ანგარიშის `chunk_retry_rate` და `provider_failure_rate` ველები
  ხაზგასმით აღვნიშნოთ ჩამორჩენის ან შემორჩენილი დატვირთვის სიმპტომები, რომლებიც ხშირად წინ უსწრებს მიღებას
  უარყოფები.

### Grafana დაფის განლაგება

Observability აქვეყნებს სპეციალურ დაფას — **SoraFS პროვაიდერის მიღება
Rollout ** (`sorafs-provider-admission`) — ქვეშ **SoraFS / პროვაიდერის Rollout **
შემდეგი კანონიკური პანელის ID-ებით:

- პანელი 1 — *მიღების შედეგის მაჩვენებელი* (დაწყობილი ტერიტორია, ერთეული „ops/წთ“).
- პანელი 2 — *გამაფრთხილებელი თანაფარდობა* (ერთ სერია), რომელიც ასხივებს გამოხატვას
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- პანელი 3 — *უარყოფის მიზეზები* (დროის სერია დაჯგუფებული `reason`-ის მიხედვით), დალაგებულია მიხედვით
  `rate(...[5m])`.
- პანელი 4 — * განაახლეთ დავალიანება* (stats), შეკითხვის ასახვა ზემოთ ცხრილში და
  ანოტირებულია მიგრაციის წიგნიდან ამოღებული რეკლამის განახლების ვადები.

დააკოპირეთ (ან შექმენით) JSON ჩონჩხი ინფრასტრუქტურის დაფების რეპოში
`observability/dashboards/sorafs_provider_admission.json`, შემდეგ განაახლეთ მხოლოდ
მონაცემთა წყარო UID; პანელის პირადობის მოწმობები და გაფრთხილების წესები მითითებულია runbook-ებით
ქვემოთ, ამიტომ მოერიდეთ მათ ხელახლა დანომრვას ამ დოკუმენტაციის გადახედვის გარეშე.

მოხერხებულობისთვის საცავი ახლა აგზავნის საცნობარო დაფის განმარტებას
`docs/source/grafana_sorafs_admission.json`; დააკოპირეთ იგი თქვენს Grafana საქაღალდეში, თუ
თქვენ გჭირდებათ საწყისი წერტილი ადგილობრივი ტესტირებისთვის.

### Prometheus გაფრთხილების წესები

დაამატეთ შემდეგი წესების ჯგუფი `observability/prometheus/sorafs_admission.rules.yml`-ს
(შექმენით ფაილი, თუ ეს არის SoraFS წესების პირველი ჯგუფი) და ჩართეთ იგი
თქვენი Prometheus კონფიგურაცია. შეცვალეთ `<pagerduty>` რეალური მარშრუტით
ეტიკეტი თქვენი გამოძახების როტაციისთვის.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

გაუშვით `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
ცვლილებების დაყენებამდე, რათა დარწმუნდეთ, რომ სინტაქსი გადის `promtool check rules`.

## მიღების შედეგები

- აკლია `chunk_range_fetch` შესაძლებლობა → უარყოფა `reason="missing_capability"`-ით.
- უცნობი შესაძლებლობების TLV-ები `allow_unknown_capabilities=true`-ის გარეშე → უარყოფა
  `reason="unknown_capability"`.
- `signature_strict=false` → უარყოფა (დაჯავშნილია იზოლირებული დიაგნოსტიკისთვის).
- ვადაგასული `refresh_deadline` → უარყოფა.

## კომუნიკაცია და ინციდენტების მართვა

- ** ყოველკვირეული სტატუსის გამგზავნი.** DevRel ავრცელებს მიღების მოკლე მიმოხილვას
  მეტრიკა, გამოჩენილი გაფრთხილებები და მომავალი ვადები.
- **შემთხვევის რეაგირება.** თუ `reject` გააფრთხილებს ხანძარს, მოწვეული ინჟინრები:
  1. მიიღეთ შეურაცხმყოფელი რეკლამა Torii აღმოჩენის მეშვეობით (`/v1/sorafs/providers`).
  2. ხელახლა გაუშვით რეკლამის ვალიდაცია პროვაიდერის მილსადენში და შეადარეთ
     `/v1/sorafs/providers` შეცდომის რეპროდუცირებისთვის.
  3. კოორდინაცია გაუწიეთ პროვაიდერს რეკლამის როტაციისთვის მომდევნო განახლებამდე
     ვადა.
- **ცვლილება იყინება.** შესაძლებლობების სქემა არ ცვლის მიწას R1/R2-ის დროს, გარდა იმ შემთხვევისა
  განლაგების კომიტეტი ხელს აწერს; GREASE-ის გამოცდები უნდა დაინიშნოს ამ პერიოდში
  ყოველკვირეული ტექნიკური ფანჯარა და შესული ხართ მიგრაციის წიგნში.

## ცნობები

- [SoraFS კვანძის/კლიენტის პროტოკოლი](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [პროვაიდერის დაშვების პოლიტიკა] (./provider-admission-policy)
- [მიგრაციის საგზაო რუკა](./migration-roadmap)
- [Provader Advert Multi-Source Extensions] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)