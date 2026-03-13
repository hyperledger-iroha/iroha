---
lang: ka
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 20b155bf2418ccdfb4981e52af44816bed8fc256ba8e54a78f6b9b320450b8fc
source_last_modified: "2026-01-22T14:35:36.747633+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-ops
title: Pin Registry Operations
sidebar_label: Pin Registry Operations
description: Monitor and triage the SoraFS pin registry and replication SLA metrics.
translator: machine-google-reviewed
---

:::შენიშვნა კანონიკური წყარო
:::

## მიმოხილვა

ამ წიგნში დოკუმენტირებულია SoraFS პინის რეესტრის მონიტორინგი და ტრიაჟირება და მისი რეპლიკაციის სერვისის დონის ხელშეკრულებები (SLA). მეტრიკა წარმოიშვა `iroha_torii`-დან და ექსპორტირებულია Prometheus-ის მეშვეობით `torii_sorafs_*` სახელთა სივრცის ქვეშ. Torii იკვლევს რეესტრის მდგომარეობას 30 წამის ინტერვალზე ფონზე, ასე რომ, დაფები რჩება აქტუალური მაშინაც კი, როცა არცერთი ოპერატორი არ ამოწმებს `/v2/sorafs/pin/*` ბოლო წერტილებს. შემოიტანეთ კურირებული დაფა (`docs/source/grafana_sorafs_pin_registry.json`) მზა გამოსაყენებლად Grafana განლაგებისთვის, რომელიც პირდაპირ ასახავს ქვემოთ მოცემულ სექციებს.

## მეტრიკული მითითება

| მეტრული | ეტიკეტები | აღწერა |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | ჯაჭვზე მანიფესტის ინვენტარი სასიცოცხლო ციკლის მდგომარეობის მიხედვით. |
| `torii_sorafs_registry_aliases_total` | — | რეესტრში ჩაწერილი აქტიური მანიფესტის მეტსახელების რაოდენობა. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | რეპლიკაციის შეკვეთის ჩამორჩენა სეგმენტირებული სტატუსის მიხედვით. |
| `torii_sorafs_replication_backlog_total` | — | მოხერხებულობის ლიანდაგი სარკისებური `pending` შეკვეთებს. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | SLA აღრიცხვა: `met` ითვლის დასრულებულ შეკვეთებს ვადაში, `missed` აგროვებს დაგვიანებულ დასრულებებს + ვადის გასვლას, `pending` ასახავს დაუსრულებელ შეკვეთებს. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | სრული დასრულების შეყოვნება (ეპოქები გაცემასა და დასრულებას შორის). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | მომლოდინე შეკვეთის სქელი ფანჯრები (ბოლო ვადა გამოკლებული ეპოქა). |

ყველა ლიანდაგი გადატვირთულია ყოველი სნეპშოტის ამოღებისას, ამიტომ საინფორმაციო დაფები უნდა აიღონ `1m` კადენცია ან უფრო სწრაფად.

## Grafana დაფა

დაფა JSON მიეწოდება შვიდი პანელით, რომლებიც ფარავს ოპერატორის სამუშაო პროცესებს. მოთხოვნები ჩამოთვლილია ქვემოთ სწრაფი მითითებისთვის, თუ გსურთ შეკვეთილი სქემების შექმნა.

1. **მანიფესტის სასიცოცხლო ციკლი** – `torii_sorafs_registry_manifests_total` (დაჯგუფებულია `status`-ით).
2. **Alias ​​კატალოგის ტენდენცია** – `torii_sorafs_registry_aliases_total`.
3. **შეკვეთის რიგი სტატუსის მიხედვით** – `torii_sorafs_registry_orders_total` (დაჯგუფებულია `status`-ით).
4. **ბექლოგი vs ვადაგასული შეკვეთები** – აერთიანებს `torii_sorafs_replication_backlog_total` და `torii_sorafs_registry_orders_total{status="expired"}` ზედაპირის გაჯერებამდე.
5. **SLA წარმატების კოეფიციენტი** –

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latency vs ვადა slack** – გადაფარვა `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` და `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. გამოიყენეთ Grafana ტრანსფორმაციები `min_over_time` ნახვების დასამატებლად, როდესაც გჭირდებათ აბსოლუტური მოდუნებული იატაკი, მაგალითად:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **გამოტოვებული შეკვეთები (1 სთ კურსი)** –

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## გაფრთხილების ზღურბლები

- **SLA წარმატება < 0,95 15 წთ**
  - ბარიერი: `sum(torii_sorafs_replication_sla_total{outcome="met"}) / clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95`
  - მოქმედება: გვერდი SRE; რეპლიკაციის დასაბრუნებელი ტრიაჟის დაწყება.
- **მოლოდინში 10-ზე ზემოთ **
  - ბარიერი: `torii_sorafs_replication_backlog_total > 10` შენარჩუნებულია 10 წუთის განმავლობაში
  - მოქმედება: შეამოწმეთ პროვაიდერის ხელმისაწვდომობა და Torii სიმძლავრის გრაფიკი.
- **ვადაგასული შეკვეთები > 0**
  - ბარიერი: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - ქმედება: შეამოწმეთ მმართველობის მანიფესტები, რათა დაადასტუროთ პროვაიდერის ჩაქრობა.
- ** დასრულება p95 > ბოლო ვადის შენელება **
  - ბარიერი: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - მოქმედება: შეამოწმეთ, რომ პროვაიდერები ასრულებენ ვალდებულებებს ვადამდე; განიხილეთ გადანაწილების გაცემა.

### მაგალითი Prometheus წესები

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## Triage Workflow

1. ** მიზეზის იდენტიფიცირება **
   - თუ SLA გამოტოვებს მწვერვალს, ხოლო ნარჩენები დაბალია, ფოკუსირება მოახდინეთ პროვაიდერის მუშაობაზე (PoR წარუმატებლობა, დაგვიანებული დასრულებები).
   - თუ ნარჩენები იზრდება სტაბილური გამოტოვებით, შეამოწმეთ დაშვება (`/v2/sorafs/pin/*`), რათა დაადასტუროთ მანიფესტები, რომლებიც ელოდება საბჭოს დამტკიცებას.
2. **პროვაიდერის სტატუსის შემოწმება **
   - გაუშვით `iroha app sorafs providers list` და შეამოწმეთ, რომ რეკლამირებული შესაძლებლობები ემთხვევა რეპლიკაციის მოთხოვნებს.
   - შეამოწმეთ `torii_sorafs_capacity_*` ლიანდაგები, რათა დაადასტუროთ მოწოდებული GiB და PoR წარმატება.
3. ** ხელახლა მინიჭება რეპლიკაცია **
   - გასცეთ ახალი შეკვეთები `sorafs_manifest_stub capacity replication-order`-ის საშუალებით, როდესაც უკმარისობა (`stat="avg"`) დაეცემა 5 ეპოქამდე (მანიფესტი/მანქანის შეფუთვა იყენებს `iroha app sorafs toolkit pack`).
   - შეატყობინეთ მმართველობას, თუ მეტსახელებს არ გააჩნიათ აქტიური მანიფესტის კავშირები (`torii_sorafs_registry_aliases_total` მოულოდნელად იკლებს).
4. **დოკუმენტის შედეგი **
   - ჩაიწერეთ ინციდენტის ჩანაწერები SoraFS ოპერაციების ჟურნალში დროის ნიშანებით და მანიფესტში ზემოქმედების ქვეშ მყოფი დიჯესტებით.
   - განაახლეთ ეს წიგნაკი, თუ დაინერგება წარუმატებლობის ახალი რეჟიმები ან დაფები.

## გავრცელების გეგმა

მიჰყევით ამ ეტაპობრივ პროცედურას წარმოებაში ალიასის ქეშის პოლიტიკის ჩართვის ან გამკაცრებისას:

1. ** მოამზადეთ კონფიგურაცია **
   - განაახლეთ `torii.sorafs_alias_cache` `iroha_config`-ში (მომხმარებელი → ფაქტობრივი) შეთანხმებული TTL-ებით და გრეის ფანჯრებით: `positive_ttl`, `refresh_window`, I18NI0000008NI200, I18NI0000008NI2001 `revocation_ttl`, `rotation_max_age`, `successor_grace` და `governance_grace`. ნაგულისხმევი ემთხვევა პოლიტიკას `docs/source/sorafs_alias_policy.md`-ში.
   - SDK-ებისთვის, გაანაწილეთ იგივე მნიშვნელობები მათი კონფიგურაციის ფენების მეშვეობით (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` Rust / NAPI / Python აკინძებში) ისე, რომ კლიენტის აღსრულება ემთხვევა კარიბჭეს.
2. **მშრალი გაშვება დადგმაში**
   - განათავსეთ კონფიგურაციის ცვლილება დადგმულ კლასტერში, რომელიც ასახავს წარმოების ტოპოლოგიას.
   - გაუშვით `cargo xtask sorafs-pin-fixtures`, რათა დაადასტუროთ კანონიკური მეტსახელის მოწყობილობები ჯერ კიდევ გაშიფრული და ორმხრივი; ნებისმიერი შეუსაბამობა გულისხმობს მანიფესტის დრეიფს, რომელიც პირველ რიგში უნდა მოგვარდეს.
   - განახორციელეთ `/v2/sorafs/pin/{digest}` და `/v2/sorafs/aliases` საბოლოო წერტილები სინთეზური მტკიცებულებებით, რომლებიც ფარავს ახალ, განახლების ფანჯარას, ვადაგასულობას და ვადაგასულობას. გადაამოწმეთ HTTP სტატუსის კოდები, სათაურები (`Sora-Proof-Status`, `Retry-After`, `Warning`) და JSON სხეულის ველები ამ runbook-თან.
3. ** ჩართვა წარმოებაში **
   - გააფართოვეთ ახალი კონფიგურაცია სტანდარტული ცვლილების ფანჯრის მეშვეობით. გამოიყენეთ იგი ჯერ Torii-ზე, შემდეგ გადატვირთეთ კარიბჭეები/SDK სერვისები, როგორც კი კვანძი დაადასტურებს ახალ პოლიტიკას ჟურნალებში.
   - შემოიტანეთ `docs/source/grafana_sorafs_pin_registry.json` Grafana-ში (ან განაახლეთ არსებული დაფები) და მიამაგრეთ მეტსახელის ქეშის განახლების პანელები NOC სამუშაო სივრცეში.
4. ** განლაგების შემდგომი დადასტურება **
   - მონიტორი `torii_sorafs_alias_cache_refresh_total` და `torii_sorafs_alias_cache_age_seconds` 30 წუთის განმავლობაში. მწვერვალები `error`/`expired` მოსახვევებში უნდა იყოს დაკავშირებული პოლიტიკის განახლების ფანჯრებთან; მოულოდნელი ზრდა ნიშნავს, რომ ოპერატორებმა უნდა შეამოწმონ მეტსახელის მტკიცებულებები და პროვაიდერის ჯანმრთელობა, სანამ გააგრძელებენ.
   - დაადასტურეთ, რომ კლიენტის მხარის ჟურნალები აჩვენებს იგივე პოლიტიკის გადაწყვეტილებებს (SDK-ები აღმოაჩენენ შეცდომებს, როდესაც მტკიცებულება მოძველებულია ან ვადაგასულია). კლიენტის გაფრთხილებების არარსებობა მიუთითებს არასწორ კონფიგურაციაზე.
5. **გამობრუნება**
   - თუ მეტსახელის გამოშვება ჩამორჩება და ფანჯრის განახლება ხშირად ტრიალებს, დროებით გააუქმეთ პოლიტიკა კონფიგურაციაში `refresh_window` და `positive_ttl` გაზრდით, შემდეგ გადაანაწილეთ. შეინახეთ `hard_expiry` ხელუხლებლად, ასე რომ ჭეშმარიტად მოძველებული მტკიცებულებები კვლავ უარყოფილი იქნება.
   - დაუბრუნდით წინა კონფიგურაციას წინა `iroha_config` სნეპშოტის აღდგენით, თუ ტელემეტრია აგრძელებს ამაღლებული `error` რაოდენობის ჩვენებას, შემდეგ გახსენით ინციდენტი, რათა თვალყური ადევნოთ მეტსახელის გენერირების შეფერხებებს.

## დაკავშირებული მასალები

- `docs/source/sorafs/pin_registry_plan.md` — განხორციელების საგზაო რუკა და მმართველობის კონტექსტი.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` — შენახვის მუშაკის ოპერაციები, ავსებს ამ რეესტრის სათამაშო წიგნს.