---
lang: ka
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6120b882618e0f9b6113948d3b12d97e0152a5fc5d4350681ba30aaf114e99d3
source_last_modified: "2026-01-22T14:45:01.354580+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-default-lane-quickstart
title: Default lane quickstart (NX-5)
sidebar_label: Default Lane Quickstart
description: Configure and verify the Nexus default lane fallback so Torii and SDKs can omit lane_id in public lanes.
translator: machine-google-reviewed
---

:::შენიშვნა კანონიკური წყარო
ეს გვერდი ასახავს `docs/source/quickstart/default_lane.md`-ს. შეინახეთ ორივე ეგზემპლარი
გასწორებულია მანამ, სანამ ლოკალიზაციის გაწმენდა არ მოხვდება პორტალში.
:::

# ნაგულისხმევი ხაზის სწრაფი დაწყება (NX-5)

> **საგზაო რუკის კონტექსტი:** NX-5 — ნაგულისხმევი საჯარო ზოლის ინტეგრაცია. გაშვების დრო ახლა
> ამჟღავნებს `nexus.routing_policy.default_lane` სარეზერვო სისტემას, ამიტომ Torii REST/gRPC
> ბოლო წერტილებს და ყველა SDK-ს შეუძლია უსაფრთხოდ გამოტოვოს `lane_id`, როდესაც ტრაფიკი ეკუთვნის
> კანონიკურ საზოგადოებრივ შესახვევზე. ეს გზამკვლევი ათვალიერებს ოპერატორებს კონფიგურაციის პროცესში
> კატალოგი, `/status`-ში დაბრუნების გადამოწმება და კლიენტის განხორციელება
> ქცევა ბოლომდე.

## წინაპირობები

- Sora/Nexus build `irohad` (იმართება `irohad --sora --config ...`-ით).
- წვდომა კონფიგურაციის საცავზე, რათა შეძლოთ `nexus.*` სექციების რედაქტირება.
- `iroha_cli` კონფიგურირებულია სამიზნე კლასტერთან სასაუბროდ.
- `curl`/`jq` (ან ექვივალენტი) Torii `/status` დატვირთვის შესამოწმებლად.

## 1. აღწერეთ ზოლის და მონაცემთა სივრცის კატალოგი

გამოაცხადეთ ზოლები და მონაცემთა სივრცეები, რომლებიც უნდა არსებობდეს ქსელში. ფრაგმენტი
ქვემოთ (მოჭრილი `defaults/nexus/config.toml`-დან) აღრიცხავს სამ საჯარო ზოლს
პლუს მონაცემთა სივრცის შესატყვისი მეტსახელები:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

თითოეული `index` უნდა იყოს უნიკალური და მომიჯნავე. მონაცემთა სივრცის ID არის 64-ბიტიანი მნიშვნელობები;
ზემოთ მოყვანილი მაგალითები სიცხადისთვის იყენებენ იგივე ციფრულ მნიშვნელობებს, როგორც ზოლის ინდექსებს.

## 2. დააყენეთ მარშრუტიზაციის ნაგულისხმევი პარამეტრები და არჩევითი უგულებელყოფა

`nexus.routing_policy` განყოფილება აკონტროლებს უკანა ზოლს და გაძლევთ საშუალებას
კონკრეტული ინსტრუქციების ან ანგარიშის პრეფიქსებისთვის მარშრუტიზაციის უგულებელყოფა. თუ წესი არ არის
ემთხვევა, დამგეგმავი მარშრუტებს ტრანზაქციას კონფიგურირებულ `default_lane`-ზე
და `default_dataspace`. როუტერის ლოგიკა ცხოვრობს
`crates/iroha_core/src/queue/router.rs` და მიმართავს პოლიტიკას გამჭვირვალედ
Torii REST/gRPC ზედაპირები.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

როდესაც მოგვიანებით დაამატებთ ახალ ხაზებს, ჯერ განაახლეთ კატალოგი, შემდეგ გააგრძელეთ მარშრუტიზაცია
წესები. სარეზერვო ზოლი კვლავ უნდა მიუთითებდეს საჯარო ზოლზე, რომელიც უჭირავს

## 3. ჩატვირთეთ კვანძი გამოყენებული პოლიტიკით

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

კვანძი აღრიცხავს მიღებულ მარშრუტიზაციის პოლიტიკას გაშვების დროს. ვალიდაციის ნებისმიერი შეცდომა
(გამოტოვებული ინდექსები, დუბლირებული მეტსახელები, მონაცემთა სივრცის არასწორი ID) გამოჩნდება ადრე
იწყება ჭორები.

## 4. დაადასტურეთ ზოლის მართვის მდგომარეობა

მას შემდეგ, რაც კვანძი ონლაინ რეჟიმშია, გამოიყენეთ CLI დამხმარე, რათა დაადასტუროთ ნაგულისხმევი ხაზი
დალუქული (მანიფესტი დატვირთული) და მზად არის მოძრაობისთვის. შემაჯამებელი ხედი ბეჭდავს ერთ რიგს
თითო ზოლზე:

```bash
iroha_cli app nexus lane-report --summary
```

გამოსავლის მაგალითი:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

თუ ნაგულისხმევი ზოლი აჩვენებს `sealed`, მიჰყევით ზოლის მართვის წიგნს მანამდე
გარე ტრაფიკის დაშვება. `--fail-on-sealed` დროშა მოსახერხებელია CI-სთვის.

## 5. შეამოწმეთ Torii სტატუსის დატვირთვა

`/status` პასუხი ასახავს როგორც მარშრუტიზაციის პოლიტიკას, ასევე თითო ზოლის განრიგს
სნეპშოტი. გამოიყენეთ `curl`/`jq` კონფიგურირებული ნაგულისხმევი პარამეტრების დასადასტურებლად და ამის შესამოწმებლად
უკანა ხაზი აწარმოებს ტელემეტრიას:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

ნიმუშის გამომავალი:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

ცოცხალი გრაფიკის მრიცხველების შესამოწმებლად `0` ზოლისთვის:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

ეს ადასტურებს, რომ TEU სნეპშოტი, მეტამონაცემების მეტამონაცემები და მანიფესტის დროშები შეესაბამება
კონფიგურაციით. იგივე დატვირთვა არის ის, რასაც Grafana პანელები იყენებენ
შესახვევის დაფის დაფა.

## 6. განახორციელეთ კლიენტის ნაგულისხმევი პარამეტრები

- **Rust/CLI.** `iroha_cli` და Rust კლიენტის ყუთი გამოტოვებს `lane_id` ველს
  როცა არ გაივლი `--lane-id` / `LaneSelector`. ამიტომ რიგის როუტერი
  უბრუნდება `default_lane`-ს. გამოიყენეთ აშკარა `--lane-id`/`--dataspace-id` დროშები
  მხოლოდ არანაგულისხმევი ზოლის დამიზნებისას.
- **JS/Swift/Android.** უახლესი SDK გამოშვებები განიხილება `laneId`/`lane_id` როგორც სურვილისამებრ
  და დაუბრუნდით `/status`-ის მიერ რეკლამირებულ მნიშვნელობას. შეინახეთ მარშრუტიზაციის პოლიტიკა
  სინქრონიზაცია ინსცენირებასა და წარმოებაში, რათა მობილური აპებს არ დასჭირდეთ გადაუდებელი დახმარება
  რეკონფიგურაციები.
- **Pipeline/SSE ტესტები.** ტრანზაქციის მოვლენის ფილტრები მიიღება
  `tx_lane_id == <u32>` პრედიკატები (იხ. `docs/source/pipeline.md`). გამოწერა
  `/v2/pipeline/events/transactions` ამ ფილტრით იმის დასამტკიცებლად, რომ წერები გაგზავნილია
  მკაფიო ზოლის გარეშე ჩამოსვლა სარეზერვო ზოლის id-ის ქვეშ.

## 7. დაკვირვებადობა და მართვის კაკვები

- `/status` ასევე აქვეყნებს `nexus_lane_governance_sealed_total` და
  `nexus_lane_governance_sealed_aliases`, რათა Alertmanager-მა გააფრთხილოს, როცა ა
  ჩიხი კარგავს თავის მანიფესტს. შეინახეთ ეს გაფრთხილებები ჩართული დევნეტებისთვისაც კი.
- გრაფიკის ტელემეტრიის რუკა და ზოლის მართვის დაფა
  (`dashboards/grafana/nexus_lanes.json`) ველით მეტსახელის/სლაგ ველებს
  კატალოგი. თუ თქვენ გადაარქმევთ ალიასს, ხელახლა მონიშნეთ შესაბამისი Kura დირექტორიები ასე
  აუდიტორები ინარჩუნებენ დეტერმინისტულ ბილიკებს (თვალთვალის ქვეშ NX-1).
- ნაგულისხმევი ზოლების პარლამენტის დამტკიცება უნდა მოიცავდეს უკან დაბრუნების გეგმას. ჩანაწერი
  მანიფესტ ჰეშის და მმართველობის მტკიცებულება ამ სწრაფი დაწყებასთან ერთად თქვენს
  ოპერატორის runbook, ასე რომ, მომავალი როტაციები არ გამოიცნობს საჭირო მდგომარეობას.

როგორც კი ეს შემოწმებები გაივლის, შეგიძლიათ მიიჩნიოთ `nexus.routing_policy.default_lane`, როგორც
კოდის ბილიკები ქსელში.