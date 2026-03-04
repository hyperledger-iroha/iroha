---
id: address-checksum-runbook
lang: ka
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Account Address Checksum Incident Runbook
sidebar_label: Checksum incidents
description: Operational response for IH58 (preferred) / compressed (`sora`, second-best) checksum failures (ADDR-7).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::შენიშვნა კანონიკური წყარო
ეს გვერდი ასახავს `docs/source/sns/address_checksum_failure_runbook.md`-ს. განახლება
ჯერ წყაროს ფაილი, შემდეგ კი ამ ასლის სინქრონიზაცია.
:::

საკონტროლო ჯამის წარუმატებლობა ჩნდება `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`) მასშტაბით
Torii, SDK-ები და საფულე/ექსპლორერი კლიენტები. ADDR-6/ADDR-7 საგზაო რუქის ელემენტები ახლა
მოითხოვეთ ოპერატორებისგან, რომ მიჰყვეს ამ წიგნს, როდესაც საკონტროლო ჯამის სიგნალიზაცია ან მხარდაჭერა
ბილეთების ცეცხლი.

## როდის უნდა გაუშვათ თამაში

- **გაფრთხილებები:** `AddressInvalidRatioSlo` (განსაზღვრულია
  `dashboards/alerts/address_ingest_rules.yml`) მოგზაურობები და ანოტაციების სია
  `reason="ERR_CHECKSUM_MISMATCH"`.
- ** ფიქსაციის დრიფტი: ** `account_address_fixture_status` Prometheus ტექსტური ფაილი ან
  Grafana დაფა იტყობინება საკონტროლო ჯამის შეუსაბამობის შესახებ ნებისმიერი SDK ასლისთვის.
- ** მხარდაჭერის გამწვავებები:** Wallet/explorer/SDK გუნდები მოჰყავთ შემოწმების ჯამის შეცდომებს, IME
  კორუფცია, ან ბუფერში სკანირება, რომელიც აღარ დეკოდირდება.
- **ხელით დაკვირვება:** Torii ჟურნალები აჩვენებს განმეორებით `address_parse_error=checksum_mismatch`
  წარმოების საბოლოო წერტილებისთვის.

თუ ინციდენტი კონკრეტულად Local-8/Local-12 შეჯახებას ეხება, მიჰყევით
სამაგიეროდ `AddressLocal8Resurgence` ან `AddressLocal12Collision` სათამაშო წიგნები.

## მტკიცებულებათა ჩამონათვალი

| მტკიცებულება | ბრძანება / მდებარეობა | შენიშვნები |
|----------|------------------|-------|
| Grafana სნეპშოტი | `dashboards/grafana/address_ingest.json` | დააფიქსირეთ არასწორი მიზეზების ავარია და დაზიანებული საბოლოო წერტილები. |
| გაფრთხილების დატვირთვა | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | ჩართეთ კონტექსტური ლეიბლები და დროის ანაბეჭდები. |
| ფიქსაციის ჯანმრთელობა | `artifacts/account_fixture/address_fixture.prom` + Grafana | ადასტურებს, გადავიდა თუ არა SDK ასლები `fixtures/account/address_vectors.json`-დან. |
| PromQL შეკითხვა | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | CSV-ის ექსპორტი ინციდენტის დოკუმენტისთვის. |
| ჟურნალები | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` (ან ჟურნალის აგრეგაცია) | გააზიარეთ PII გაზიარებამდე. |
| მოწყობილობების შემოწმება | `cargo xtask address-vectors --verify` | ადასტურებს, რომ კანონიკური გენერატორი და ერთგული JSON თანხმდებიან. |
| SDK პარიტეტის შემოწმება | `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` | გაშვება ყოველი SDK-ისთვის, რომელიც მოხსენებულია გაფრთხილებებში/ბილეთებში. |
| ბუფერში/IME საღი აზრი | `iroha tools address inspect <literal>` | ამოიცნობს დამალულ სიმბოლოებს ან IME-ს გადაწერას; მიუთითეთ `address_display_guidelines.md`. |

## მყისიერი პასუხი

1. აღიარეთ გაფრთხილება, დააკავშირეთ Grafana სნეპშოტები + PromQL გამომავალი ინციდენტში
   თემა და შენიშვნა შეეხო Torii კონტექსტს.
2. გაყინეთ მანიფესტის აქციები / SDK ავრცელებს მისამართების ანალიზს.
3. შეინახეთ დაფის სნეპშოტები და გენერირებული Prometheus ტექსტური ფაილის არტეფაქტები
   ინციდენტის საქაღალდე (`docs/source/sns/incidents/YYYY-MM/<ticket>/`).
4. ამოიღეთ ჟურნალის ნიმუშები, რომლებიც აჩვენებს `checksum_mismatch` დატვირთვას.
5. აცნობეთ SDK-ის მფლობელებს (`#sdk-parity`) ნიმუშების დატვირთვით, რათა მათ შეძლონ ტრიაჟირება.

## ძირეული მიზეზის იზოლაცია

### მოწყობილობა ან გენერატორის დრიფტი

- ხელახლა გაუშვით `cargo xtask address-vectors --verify`; რეგენერაცია, თუ ის ვერ მოხერხდა.
- შეასრულეთ `ci/account_fixture_metrics.sh` (ან ინდივიდუალური
  `scripts/account_fixture_helper.py check`) თითოეული SDK-სთვის შეფუთული დასადასტურებლად
  სეზონი ემთხვევა კანონიკურ JSON-ს.

### კლიენტის შიფრები / IME რეგრესია

- შეამოწმეთ მომხმარებლის მიერ მოწოდებული ლიტერალები `iroha tools address inspect`-ის საშუალებით ნულოვანი სიგანის საპოვნელად
  უერთდება, კანას კონვერტაციები ან შეკვეცილი დატვირთვები.
- გადაამოწმეთ საფულე/გამომძიებელი მიედინება
  `docs/source/sns/address_display_guidelines.md` (ორმაგი ასლის სამიზნეები, გაფრთხილებები,
  QR დამხმარეები) იმის უზრუნველსაყოფად, რომ ისინი იცავენ დამტკიცებულ UX-ს.

### მანიფესტის ან რეესტრის პრობლემები

- მიჰყევით `address_manifest_ops.md`-ს უახლესი მანიფესტების ნაკრების ხელახლა დასადასტურებლად და
  უზრუნველყოს, რომ არ გამოჩნდეს ლოკალური-8 სელექტორი.
  ჩნდება დატვირთვებში.

### მავნე ან არასწორი ტრაფიკი

- დაანგრიეთ შეურაცხმყოფელი IP-ები/აპების ID-ები Torii ჟურნალებისა და `torii_http_requests_total`-ის მეშვეობით.
- შეინახეთ მინიმუმ 24 საათის ჟურნალები უსაფრთხოების/მმართველობის შემდგომი დაკვირვებისთვის.

## შერბილება და აღდგენა

| სცენარი | მოქმედებები |
|----------|---------|
| ფიქსურის დრიფტი | განაახლეთ `fixtures/account/address_vectors.json`, ხელახლა გაუშვით `cargo xtask address-vectors --verify`, განაახლეთ SDK პაკეტები და მიამაგრეთ `address_fixture.prom` სნეპშოტები ბილეთზე. |
| SDK/კლიენტის რეგრესია | ფაილის პრობლემები კანონიკურ მოწყობილობაზე + `iroha tools address inspect` გამომავალზე და კარიბჭის გამოშვებებზე მითითებით SDK პარიტეტის CI-ის მიღმა (მაგ., `ci/check_address_normalize.sh`). |
| მავნე წარდგინებები | შეზღუდეთ ან დაბლოკეთ შეურაცხმყოფელი პრინციპები, გადადით მმართველობამდე, თუ საჭიროა საფლავის ქვის ამომრჩეველები. |

მას შემდეგ, რაც შემარბილებელი ღონისძიებები ჩამოყალიბდა, ხელახლა გაუშვით ზემოთ მოცემული PromQL მოთხოვნა დასადასტურებლად
`ERR_CHECKSUM_MISMATCH` რჩება ნულზე (გარდა `/tests/*`) სულ მცირე
ინციდენტის შეფასებამდე 30 წუთით ადრე.

## დახურვა

1. დაარქივეთ Grafana სნეპშოტები, PromQL CSV, ჟურნალის ამონაწერები და `address_fixture.prom`.
2. განაახლეთ `status.md` (ADDR განყოფილება) პლუს საგზაო რუქის მწკრივი, თუ ინსტრუმენტები/docs
   შეიცვალა.
3. შეიტანეთ ინციდენტის შემდგომი შენიშვნები `docs/source/sns/incidents/` ქვეშ ახალი გაკვეთილების დროს
   აღმოცენდება.
4. დარწმუნდით, რომ SDK-ის გამოშვების შენიშვნებში მითითებულია გამშვები ჯამის შესწორებები, როდესაც ეს შესაძლებელია.
5. დაადასტურეთ, რომ გაფრთხილება რჩება მწვანე 24 საათის განმავლობაში და მოწყობილობების შემოწმება მანამდე რჩება მწვანე
   გადაწყვეტა.