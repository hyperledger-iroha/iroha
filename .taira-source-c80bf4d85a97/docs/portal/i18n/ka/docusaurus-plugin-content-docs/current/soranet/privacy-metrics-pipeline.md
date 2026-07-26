---
id: privacy-metrics-pipeline
lang: ka
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet Privacy Metrics Pipeline (SNNet-8)
sidebar_label: Privacy Metrics Pipeline
description: Privacy-preserving telemetry collection for SoraNet relays and orchestrators.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::შენიშვნა კანონიკური წყარო
:::

# SoraNet Privacy Metrics Pipeline

SNNet-8 წარმოგიდგენთ კონფიდენციალურობის მცოდნე ტელემეტრიულ ზედაპირს სარელეო მუშაობისთვის. The
რელე ახლა აგროვებს ხელის ჩამორთმევას და წრიულ მოვლენებს წუთების ზომის თაიგულებში და
ექსპორტს ახორციელებს მხოლოდ უხეში Prometheus მრიცხველებით, ინდივიდუალური სქემების შენარჩუნებით
unlinkable, ხოლო ოპერატორებს აძლევს ქმედით ხილვადობას.

## აგრეგატორის მიმოხილვა

- Runtime განხორციელება მოქმედებს `tools/soranet-relay/src/privacy.rs`-ში როგორც
  `PrivacyAggregator`.
- თაიგულები იკვრება კედლის საათის წუთით (`bucket_secs`, ნაგულისხმევი 60 წამი) და
  ინახება შემოსაზღვრულ რგოლში (`max_completed_buckets`, ნაგულისხმევი 120). კოლექციონერი
  აქციები ინარჩუნებენ საკუთარ შეზღუდულ ნარჩენებს (`max_share_lag_buckets`, ნაგულისხმევი 12)
  ასე რომ, შემორჩენილი Prio-ს ფანჯრები გარეცხილია როგორც ჩახშობილი თაიგულები, ვიდრე გაჟონვა
  მეხსიერების ან ნიღბის ჩარჩენილი კოლექციონერები.
- `RelayConfig::privacy` პირდაპირ რუკებს ასახავს `PrivacyConfig`-ში, ავლენს ტუნინგს
  სახელურები (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`,
  `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`,
  `expected_shares`). წარმოების გაშვების დრო ინარჩუნებს ნაგულისხმევს, ხოლო SNNet-8a
  შემოაქვს უსაფრთხო აგრეგაციის ზღვრები.
- Runtime მოდულები ჩაწერენ მოვლენებს აკრეფილი დამხმარეების საშუალებით:
  `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`,
  `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`,
  `record_verified_bytes` და `record_gar_category`.

## სარელეო ადმინისტრატორის საბოლოო წერტილი

ოპერატორებს შეუძლიათ გამოიკითხონ რელეს ადმინისტრატორის მსმენელი ნედლეული დაკვირვებისთვის
`GET /privacy/events`. ბოლო წერტილი აბრუნებს ახალი ხაზებით გამოყოფილ JSON-ს
(`application/x-ndjson`), რომელიც შეიცავს `SoranetPrivacyEventV1` სარკისებურ დატვირთვას
შიდა `PrivacyEventBuffer`-დან. ბუფერი ინახავს უახლეს მოვლენებს
`privacy.event_buffer_capacity` ჩანაწერებში (ნაგულისხმევი 4096) და იშლება
წაიკითხეთ, ამიტომ საფხეკები საკმარისად ხშირად უნდა გამოკითხონ, რათა თავიდან აიცილონ ხარვეზები. მოვლენები მოიცავს
იგივე ხელის ჩამორთმევა, დროსელი, დამოწმებული გამტარობა, აქტიური წრე და GAR სიგნალები
რომელიც კვებავს Prometheus მრიცხველებს, რაც საშუალებას აძლევს ქვედა დინების კოლექციონერებს დაარქივონ
კონფიდენციალურობისთვის უსაფრთხო პურის ნამსხვრევები ან შესანახი უსაფრთხო აგრეგაციის სამუშაო ნაკადები.

## სარელეო კონფიგურაცია

ოპერატორები არეგულირებენ კონფიდენციალურობის ტელემეტრიის კადენციებს სარელეო კონფიგურაციის ფაილში
`privacy` განყოფილება:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

ველის ნაგულისხმევი პარამეტრები ემთხვევა SNNet-8 სპეციფიკაციას და დამოწმებულია დატვირთვის დროს:

| ველი | აღწერა | ნაგულისხმევი |
|-------|-------------|---------|
| `bucket_secs` | თითოეული აგრეგაციის ფანჯრის სიგანე (წამი). | `60` |
| `min_handshakes` | კონტრიბუტორების მინიმალური რაოდენობა, სანამ bucket შეძლებს მრიცხველების გამოშვებას. | `12` |
| `flush_delay_buckets` | დასრულებული თაიგულები უნდა დაველოდოთ გარეცხვის მცდელობას. | `1` |
| `force_flush_buckets` | მაქსიმალური ასაკი, სანამ ჩვენ გამოვყოფთ ჩახშობილ ვედროს. | `6` |
| `max_completed_buckets` | შენახული თაიგულის ნარჩენი (აფერხებს შეუზღუდავ მეხსიერებას). | `120` |
| `max_share_lag_buckets` | კოლექტორის აქციების შენახვის ფანჯარა ჩახშობამდე. | `12` |
| `expected_shares` | Prio კოლექციონერის აქციები საჭიროა გაერთიანებამდე. | `2` |
| `event_buffer_capacity` | NDJSON ღონისძიების ჩანაწერი ადმინისტრატორის ნაკადისთვის. | `4096` |

დააყენეთ `force_flush_buckets` უფრო დაბალი ვიდრე `flush_delay_buckets`, ნულოვანი
ზღურბლები, ან შეკავების მცველის გამორთვა ახლა ვერ ხერხდება ვალიდაციის თავიდან აცილება
განლაგება, რომელიც გაჟონავს თითო სარელეო ტელემეტრია.

`event_buffer_capacity` ლიმიტი ასევე ზღუდავს `/admin/privacy/events`-ს, რაც უზრუნველყოფს
საფხეკები განუსაზღვრელი ვადით ვერ ჩამორჩებიან.

## Prio კოლექციონერი აქციები

SNNet-8a ავრცელებს ორმაგ კოლექციონერებს, რომლებიც ასხივებენ საიდუმლოებით გაზიარებულ Prio თაიგულებს. The
ორკესტრი ახლა აანალიზებს `/privacy/events` NDJSON ნაკადს ორივესთვის
`SoranetPrivacyEventV1` ჩანაწერები და `SoranetPrivacyPrioShareV1` გაზიარებები,
მათი გადაგზავნა `SoranetSecureAggregator::ingest_prio_share`-ში. თაიგულები ასხივებენ
ერთხელ `PrivacyBucketConfig::expected_shares` შენატანები ჩამოვა, ასახული
სარელეო ქცევა. აქციები დამოწმებულია თაიგულების გასწორებისა და ჰისტოგრამის ფორმისთვის
`SoranetPrivacyBucketMetricsV1`-ში გაერთიანებამდე. თუ კომბინირებული
ხელის ჩამორთმევის რაოდენობა მცირდება `min_contributors`-ზე ქვემოთ, ვედრო ექსპორტირებულია როგორც
`suppressed`, სარელეო აგრეგატორის ქცევის ასახვა. დათრგუნული
Windows ახლა ასხივებს `suppression_reason` ეტიკეტს, რათა ოპერატორებმა განასხვავონ
`insufficient_contributors`, `collector_suppressed` შორის,
`collector_window_elapsed` და `forced_flush_window_elapsed` სცენარი, როდესაც
ტელემეტრიული ხარვეზების დიაგნოსტიკა. `collector_window_elapsed` მიზეზიც ირთვება
როდესაც Prio-ს აქციები აგრძელებს `max_share_lag_buckets`-ს და ქმნის ჩარჩენილ კოლექციონერებს
ხილული აკუმულატორების მეხსიერებაში დატოვების გარეშე.

## Torii გადაყლაპვის საბოლოო წერტილები

Torii ახლა ასახავს ორ ტელემეტრიით დახურულ HTTP ბოლო წერტილს, ასე რომ, რელეები და კოლექტორები
შეუძლია დაკვირვების გაგზავნა შეკვეთილი ტრანსპორტის ჩაშენების გარეშე:

- `POST /v1/soranet/privacy/event` იღებს ა
  `RecordSoranetPrivacyEventDto` დატვირთვა. სხეული ახვევს ა
  `SoranetPrivacyEventV1` პლუს სურვილისამებრ `source` ლეიბლი. Torii ადასტურებს
  მოთხოვნა აქტიური ტელემეტრიის პროფილის წინააღმდეგ, ჩაიწერს მოვლენას და პასუხობს
  HTTP `202 Accepted`-თან ერთად Norito JSON კონვერტით, რომელიც შეიცავს
  გამოთვლილი თაიგულის ფანჯარა (`bucket_start_unix`, `bucket_duration_secs`) და
  სარელეო რეჟიმი.
- `POST /v1/soranet/privacy/share` იღებს `RecordSoranetPrivacyShareDto`-ს
  ტვირთამწეობა. კორპუსს აქვს `SoranetPrivacyPrioShareV1` და სურვილისამებრ
  `forwarded_by` მინიშნება, რომ ოპერატორებმა შეძლონ კოლექტორების ნაკადების შემოწმება. წარმატებული
  წარდგენები აბრუნებს HTTP `202 Accepted` Norito JSON კონვერტით, რომელიც აჯამებს
  კოლექციონერი, თაიგულის ფანჯარა და ჩახშობის მინიშნება; ვალიდაციის წარუმატებლობები რუკაზე
  ტელემეტრიული `Conversion` პასუხი დეტერმინისტული შეცდომის დამუშავების შესანარჩუნებლად
  კოლექციონერებს შორის. ორკესტრატორის ღონისძიების ციკლი ახლა ასხივებს ამ აქციებს
  გამოკითხვის რელეები, ინარჩუნებს Torii-ის Prio აკუმულატორის სინქრონიზაციას სარელეო თაიგულებთან.

ორივე ბოლო წერტილი პატივს სცემს ტელემეტრიის პროფილს: ისინი ასხივებენ `503 სერვისს
მიუწვდომელია` როდესაც მეტრიკა გამორთულია. კლიენტებს შეუძლიათ გაგზავნონ ან Norito ორობითი
(`application/x.norito`) ან Norito JSON (`application/x.norito+json`) სხეულები;
სერვერი ავტომატურად აწარმოებს მოლაპარაკებას ფორმატზე სტანდარტული Torii-ის საშუალებით
ექსტრაქტორები.

## Prometheus მეტრიკა

თითოეული ექსპორტირებული ვედრო ატარებს `mode` (`entry`, `middle`, `exit`) და
`bucket_start` ეტიკეტები. გამოიყოფა შემდეგი მეტრიკული ოჯახები:

| მეტრული | აღწერა |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | ხელის ჩამორთმევის ტაქსონომია `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`-ით. |
| `soranet_privacy_throttles_total{scope}` | დროსელის მრიცხველები `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`-ით. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | გაერთიანებული გაგრილების ხანგძლივობა, რომელიც ხელს უწყობს შენელებული ხელის ჩამორთმევას. |
| `soranet_privacy_verified_bytes_total` | დამოწმებული გამტარუნარიანობა დაბრმავებული გაზომვის მტკიცებულებებიდან. |
| `soranet_privacy_active_circuits_{avg,max}` | საშუალო და პიკური აქტიური სქემები თითო თაიგულზე. |
| `soranet_privacy_rtt_millis{percentile}` | RTT პროცენტული შეფასებები (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | ჰეშირებული მმართველობის სამოქმედო ანგარიშის მრიცხველები ჩართულია კატეგორიის შეჯამების მიხედვით. |
| `soranet_privacy_bucket_suppressed` | თაიგულები შეჩერებულია, რადგან კონტრიბუტორის ბარიერი არ დაკმაყოფილდა. |
| `soranet_privacy_pending_collectors{mode}` | კოლექტორის წილი აკუმულატორების მოლოდინში კომბინაცია, დაჯგუფებული სარელეო რეჟიმის მიხედვით. |
| `soranet_privacy_suppression_total{reason}` | ჩახშობილია თაიგულების მრიცხველები `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}`-ით, რათა დაფებმა შეძლონ კონფიდენციალურობის ხარვეზების მინიჭება. |
| `soranet_privacy_snapshot_suppression_ratio` | ბოლო გადინების ჩახშობა/დაწურული თანაფარდობა (0–1), სასარგებლოა გაფრთხილებული ბიუჯეტისთვის. |
| `soranet_privacy_last_poll_unixtime` | უახლესი წარმატებული გამოკითხვის UNIX დროის ანაბეჭდი (ამოძრავებს კოლექციონერის უმოქმედობის გაფრთხილებას). |
| `soranet_privacy_collector_enabled` | ლიანდაგი, რომელიც გადატრიალდება `0`-ზე, როდესაც კონფიდენციალურობის შემგროვებელი გამორთულია ან ვერ ჩაირთვება (ამოძრავებს კოლექციონერის გამორთული გაფრთხილებას). |
| `soranet_privacy_poll_errors_total{provider}` | კენჭისყრის წარუმატებლობები დაჯგუფებული სარელეო ფსევდონიმების მიხედვით (გადაშიფვრის შეცდომებზე ზრდა, HTTP წარუმატებლობა ან სტატუსის მოულოდნელი კოდები). |

თაიგულები დაკვირვების გარეშე რჩება ჩუმად, დაფის დაფების გარეშე
ნულოვანი ფანჯრების დამზადება.

## ოპერატიული სახელმძღვანელო

1. **Dashboards** – ზემოთ მოცემული მეტრიკის დიაგრამა დაჯგუფებული `mode` და `window_start` მიხედვით.
   მონიშნეთ დაკარგული ფანჯრები ზედაპირული კოლექტორის ან სარელეო პრობლემებისთვის. გამოყენება
   `soranet_privacy_suppression_total{reason}` კონტრიბუტორის გასარჩევად
   ხარვეზები კოლექციონერზე ორიენტირებული ჩახშობის შედეგად ხარვეზების ტრიაჟის დროს. Grafana
   აქტივი ახლა აგზავნის სპეციალურ **"ჩახშობის მიზეზები (5 მ)"** პანელი, რომელიც იკვებება ამ
   მრიცხველები პლუს **"Suppressed Bucket %"** სტატისტიკა, რომელიც გამოითვლება
   `sum(soranet_privacy_bucket_suppressed) / count(...)` თითო შერჩევით ასე
   ოპერატორებს შეუძლიათ შეამჩნიონ ბიუჯეტის დარღვევები ერთი შეხედვით. ** კოლექციონერის წილი
   Backlog** სერია (`soranet_privacy_pending_collectors`) და **Snapshot
   ჩახშობის კოეფიციენტი** სტატისტიკა ხაზს უსვამს ჩარჩენილ კოლექტორებს და ბიუჯეტის დრიფტს დროს
   ავტომატური გაშვებები.
2. **გაფრთხილება** – მართეთ სიგნალიზაცია კონფიდენციალურობისთვის დაცული მრიცხველებიდან: PoW უარყოფის მწვერვალები,
   გაგრილების სიხშირე, RTT დრიფტი და ტევადობის უარყოფა. რადგან მრიცხველები არიან
   მონოტონური თითოეულ თაიგულში, განაკვეთზე დაფუძნებული პირდაპირი წესები კარგად მუშაობს.
3. **ინციდენტის რეაგირება** – პირველ რიგში დაეყრდნოთ გაერთიანებულ მონაცემებს. უფრო ღრმა გამართვისას
   აუცილებელია, მოითხოვეთ რელეები თაიგულის კადრების გასამეორებლად ან დაბრმავებული შესამოწმებლად
   საზომი მტკიცებულებები, ნაცვლად ნედლი სატრანსპორტო ჟურნალის მოსავლის.
4. **შეკავება** – გადაფხეკით ხშირად, რათა თავიდან აიცილოთ გადაჭარბება
   `max_completed_buckets`. ექსპორტიორებმა უნდა განიხილონ Prometheus გამომავალი, როგორც
   კანონიკური წყარო და ჩამოაგდეთ ადგილობრივი თაიგულები გადაგზავნის შემდეგ.

## ჩახშობის ანალიტიკა და ავტომატური გაშვებებიSNNet-8-ის მიღება ემყარება იმის დემონსტრირებას, რომ ავტომატური კოლექტორები დარჩებიან
ჯანსაღი და ეს ჩახშობა რჩება პოლიტიკის ფარგლებში (≤10% თაიგულების თითო
რელე ნებისმიერი 30 წუთიანი ფანჯარაში). ინსტრუმენტები საჭირო იყო ამ კარიბჭის დასაკმაყოფილებლად ახლა
გემები ხესთან ერთად; ოპერატორებმა ის უნდა ჩაერთონ თავიანთ ყოველკვირეულ რიტუალებში. ახალი
Grafana ჩახშობის პანელები ასახავს ქვემოთ მოცემულ PromQL სნიპეტებს, რაც იძლევა გამოძახების საშუალებას
გუნდების პირდაპირი ხილვადობა მანამ, სანამ მათ არ დასჭირდებათ ხელით შეკითხვებზე დაბრუნება.

### PromQL რეცეპტები ჩახშობის მიმოხილვისთვის

ოპერატორებს ხელთ უნდა ჰქონდეთ შემდეგი PromQL დამხმარეები; ორივე მითითებულია
გაზიარებულ Grafana დაფაში (`dashboards/grafana/soranet_privacy_metrics.json`)
და Alertmanager წესები:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

გამოიყენეთ გამომავალი თანაფარდობა, რათა დაადასტუროთ, რომ **“Suppressed Bucket %”** სტატისტიკა რჩება ქვემოთ
პოლიტიკის ბიუჯეტი; შეაერთეთ მავთულის დეტექტორი Alertmanager-ში სწრაფი გამოხმაურებისთვის
როდესაც კონტრიბუტორის რაოდენობა მოულოდნელად ეცემა.

### ოფლაინ თაიგულის ანგარიში CLI

სამუშაო სივრცე აჩვენებს `cargo xtask soranet-privacy-report` ერთჯერადი NDJSON-ისთვის
იჭერს. მიუთითეთ ის ერთ ან რამდენიმე სარელეო ადმინისტრატორის ექსპორტზე:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

დამხმარე გადასცემს გადაღებას `SoranetSecureAggregator`-ის მეშვეობით, ბეჭდავს ა
ჩახშობის შეჯამება stdout-ზე და სურვილისამებრ წერს სტრუქტურირებულ JSON ანგარიშს
`--json-out <path|->`-ის საშუალებით. იგი პატივს სცემს იმავე სახელურებს, როგორც ცოცხალი კოლექციონერი
(`--bucket-secs`, `--min-contributors`, `--expected-shares` და ა.შ.), გაქირავება
ოპერატორები იმეორებენ ისტორიულ გადაღებებს სხვადასხვა ზღურბლზე ტრიაჟის დროს
საკითხი. მიამაგრეთ JSON Grafana ეკრანის სურათებთან ერთად, რათა SNNet-8
ჩახშობის ანალიტიკური კარიბჭე რჩება აუდიტორული.

### პირველი ავტომატური გაშვების საკონტროლო სია

მმართველობა კვლავ მოითხოვს იმის მტკიცებას, რომ პირველი ავტომატიზაციის გაშვება დააკმაყოფილა
ჩახშობის ბიუჯეტი. დამხმარე ახლა იღებს `--max-suppression-ratio <0-1>` ასე
CI ან ოპერატორებს შეუძლიათ სწრაფად მარცხი, როდესაც ჩახშობილი თაიგულები აღემატება დაშვებულს
ფანჯარა (ნაგულისხმევი 10%) ან როდესაც ჯერ არ არის თაიგულები. რეკომენდებული ნაკადი:

1. NDJSON-ის ექსპორტი რელეს ადმინისტრატორის ბოლო წერტილიდან
   `/v1/soranet/privacy/event|share` ნაკადი შევიდა
   `artifacts/sorafs_privacy/<relay>.ndjson`.
2. გაუშვით დამხმარე პოლიტიკის ბიუჯეტით:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   ბრძანება ბეჭდავს დაკვირვებულ თანაფარდობას და გამოდის ნულიდან, როდესაც ბიუჯეტი არის
   გადააჭარბა **ან**, როდესაც თაიგულები მზად არ არის, რაც მიუთითებს იმაზე, რომ ტელემეტრია არ არის
   ჯერ კიდევ წარმოებული გასაშვებად. ცოცხალი მეტრიკა უნდა იყოს ნაჩვენები
   `soranet_privacy_pending_collectors` დრენაჟი ნულისკენ და
   `soranet_privacy_snapshot_suppression_ratio` რჩება იმავე ბიუჯეტის ფარგლებში
   ხოლო გაშვება სრულდება.
3. დაარქივეთ JSON გამომავალი და CLI ჟურნალი SNNet-8 მტკიცებულების ნაკრებით მანამდე
   სატრანსპორტო ნაგულისხმევის გადახვევა, რათა მიმომხილველებმა შეძლონ ზუსტი არტეფაქტების გამეორება.

## შემდეგი ნაბიჯები (SNNet-8a)

- ორმაგი Prio კოლექციონერების ინტეგრირება, მათი წილი გადაყლაპვისას
  მუშაობის დრო ისე, რომ რელეები და კოლექტორები ასხივებენ თანმიმდევრულ `SoranetPrivacyBucketMetricsV1`
  ტვირთამწეობა. *(შესრულებულია - იხილეთ `ingest_privacy_payload` in
  `crates/sorafs_orchestrator/src/lib.rs` და თანმხლები ტესტები.)*
- გამოაქვეყნეთ საერთო Prometheus დაფა JSON და გაფრთხილების წესები, რომელიც მოიცავს
  ჩახშობის ხარვეზები, კოლექციონერის ჯანმრთელობა და ანონიმურობის გაუარესება. *(შესრულებულია - იხ
  `dashboards/grafana/soranet_privacy_metrics.json`,
  `dashboards/alerts/soranet_privacy_rules.yml`,
  `dashboards/alerts/soranet_policy_rules.yml` და ვალიდაციის მოწყობილობები.)*
- შექმენით დიფერენციალური კონფიდენციალურობის კალიბრაციის არტეფაქტები, რომლებიც აღწერილია აქ
  `privacy_metrics_dp.md`, რეპროდუცირებადი ნოუთბუქების და მართვის ჩათვლით
  ამუშავებს. *(შესრულებულია - რვეული + არტეფაქტები, გენერირებული
  `scripts/telemetry/run_privacy_dp.py`; CI შეფუთვა
  `scripts/telemetry/run_privacy_dp_notebook.sh` ახორციელებს ნოუთბუქის მეშვეობით
  `.github/workflows/release-pipeline.yml` სამუშაო პროცესი; მმართველობის დაიჯესტი შეტანილია
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

მიმდინარე გამოცემა აწვდის SNNet-8 საფუძველს: განმსაზღვრელი,
კონფიდენციალურობისთვის უსაფრთხო ტელემეტრია, რომელიც პირდაპირ ხვდება არსებულ Prometheus საფხეხებში
და დაფები. არსებობს დიფერენციალური კონფიდენციალურობის კალიბრაციის არტეფაქტები
გამოშვების მილსადენის სამუშაო მიმდინარეობა ინარჩუნებს ნოუთბუქის გამომავალს და დარჩენილს
სამუშაო ფოკუსირებულია პირველი ავტომატური გაშვების მონიტორინგზე, პლუს ჩახშობის გაფართოებაზე
გაფრთხილების ანალიტიკა.