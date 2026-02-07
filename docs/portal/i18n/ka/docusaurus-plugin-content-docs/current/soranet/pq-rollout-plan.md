---
id: pq-rollout-plan
lang: ka
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNNet-16G Post-Quantum Rollout Playbook
sidebar_label: PQ Rollout Plan
description: Operational guide for promoting the SoraNet hybrid X25519+ML-KEM handshake from canary to default across relays, clients, and SDKs.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::შენიშვნა კანონიკური წყარო
:::

SNNet-16G დაასრულებს SoraNet ტრანსპორტის პოსტკვანტურ გავრცელებას. `rollout_phase` სახელურები საშუალებას აძლევს ოპერატორებს კოორდინირება გაუწიონ დეტერმინისტულ დაწინაურებას არსებული A ეტაპის დაცვის მოთხოვნიდან B ეტაპის უმრავლესობის გაშუქებამდე და C ეტაპის მკაცრი PQ პოზა ყველა ზედაპირისთვის დაუმუშავებელი JSON/TOML რედაქტირების გარეშე.

ეს სათამაშო წიგნი მოიცავს:

- ფაზის განმარტებები და ახალი კონფიგურაციის ღილაკები (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) ჩართული კოდის ბაზაში (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- SDK და CLI დროშის რუქა, რათა ყველა კლიენტმა შეძლოს აკონტროლოს გაშვება.
- სარელეო/კლიენტის კანარის განრიგის მოლოდინები, პლუს მმართველობის დაფები, რომლებიც კარიბჭის რეკლამას წარმოადგენს (`dashboards/grafana/soranet_pq_ratchet.json`).
- გადაბრუნებული კაკვები და მითითებები ცეცხლსასროლი ბურღვის წიგნზე ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## ფაზის რუკა

| `rollout_phase` | ეფექტური ანონიმურობის ეტაპი | ნაგულისხმევი ეფექტი | ტიპიური გამოყენება |
|--------------------------------------------|--------------|-------------|
| `canary` | `anon-guard-pq` (სტადია A) | საჭიროა მინიმუმ ერთი PQ მცველი თითო წრეზე, სანამ ფლოტი ათბობს. | საწყისი და ადრეული კანარის კვირები. |
| `ramp` | `anon-majority-pq` (სტადია B) | მიკერძოებული შერჩევა PQ რელეების მიმართ >= დაფარვის ორი მესამედით; კლასიკური რელეები რჩება სანაცვლოდ. | რეგიონის მიხედვით რელე კანარები; SDK გადახედვის გადართვა. |
| `default` | `anon-strict-pq` (სტადია C) | განახორციელეთ მხოლოდ PQ სქემები და გამკაცრეთ სიგნალიზაციის შემცირების სიგნალიზაცია. | საბოლოო აქცია ტელემეტრიისა და მმართველობის ხელმოწერის დასრულების შემდეგ. |

თუ ზედაპირი ასევე ადგენს გამოკვეთილ `anonymity_policy`-ს, ის არღვევს ამ კომპონენტის ფაზას. ექსპლიციტური ეტაპის გამოტოვება ახლა გადადის `rollout_phase` მნიშვნელობამდე, ასე რომ ოპერატორებს შეუძლიათ გადაატრიალონ ფაზა ერთხელ ყოველ გარემოში და კლიენტებს მისცენ მემკვიდრეობით.

## კონფიგურაციის მითითება

### ორკესტრი (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

ორკესტრატორი ჩამტვირთავი წყვეტს სარეზერვო სტადიას გაშვების დროს (`crates/sorafs_orchestrator/src/lib.rs:2229`) და ასახავს მას `sorafs_orchestrator_policy_events_total` და `sorafs_orchestrator_pq_ratio_*` მეშვეობით. იხილეთ `docs/examples/sorafs_rollout_stage_b.toml` და `docs/examples/sorafs_rollout_stage_c.toml` გამოსაყენებლად მზა ფრაგმენტებისთვის.

### Rust კლიენტი / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` ახლა ჩაწერს გაანალიზებულ ფაზას (`crates/iroha/src/client.rs:2315`), ასე რომ დამხმარე ბრძანებებს (მაგალითად, `iroha_cli app sorafs fetch`) შეუძლიათ მიმდინარე ფაზის მოხსენება ნაგულისხმევი ანონიმურობის პოლიტიკასთან ერთად.

## ავტომატიზაცია

ორი `cargo xtask` დამხმარე ავტომატიზირებს გრაფიკის გენერირებას და არტეფაქტის აღებას.

1. **შექმენით რეგიონალური განრიგი**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   ხანგრძლივობა იღებს `s`, `m`, `h`, ან `d` სუფიქსებს. ბრძანება გამოსცემს `artifacts/soranet_pq_rollout_plan.json` და Markdown-ის შეჯამებას (`artifacts/soranet_pq_rollout_plan.md`), რომელთა გაგზავნა შესაძლებელია ცვლილების მოთხოვნით.

2. **საბურღი არტეფაქტების დაჭერა ხელმოწერებით **

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   ბრძანება აკოპირებს მოწოდებულ ფაილებს `artifacts/soranet_pq_rollout/<timestamp>_<label>/`-ში, ითვლის BLAKE3-ის დაჯესტებს თითოეული არტეფაქტისთვის და წერს `rollout_capture.json` მეტამონაცემებს პლუს Ed25519 ხელმოწერას დატვირთვაზე. გამოიყენეთ იგივე პირადი გასაღები, რომელიც ხელს აწერს ხანძარსაწინააღმდეგო წვრთნების წუთს, რათა მმართველობამ შეძლოს დაჭერის სწრაფად დადასტურება.

## SDK & CLI დროშის მატრიცა

| ზედაპირი | კანარის (სტადია A) | Ramp (სტადია B) | ნაგულისხმევი (სტადია C) |
|---------|-----------------|---------------|-----------------|
| `sorafs_cli` მოტანა | `--anonymity-policy stage-a` ან დაეყრდნო ფაზას | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| ორკესტრატორის კონფიგურაცია JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust კლიენტის კონფიგურაცია (`iroha.toml`) | `rollout_phase = "canary"` (ნაგულისხმევი) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` ხელმოწერილი ბრძანებები | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, სურვილისამებრ `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, სურვილისამებრ `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, სურვილისამებრ `.ANON_STRICT_PQ` |
| JavaScript ორკესტრატორის დამხმარეები | `rolloutPhase: "canary"` ან `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| პითონი `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

ყველა SDK გადართავს რუკას იმავე სცენის პარსერზე, რომელსაც იყენებს ორკესტრატორი (`crates/sorafs_orchestrator/src/lib.rs:365`), ასე რომ შერეული ენების განლაგება დარჩება დაბლოკვის ეტაპზე კონფიგურირებული ფაზის მიხედვით.

## კანარის განრიგის საკონტროლო სია

1. **წინასწარი გაფრენა (T მინუს 2 კვირა)**

- დაადასტურეთ A სტადიის შეფერხების მაჩვენებელი <1% წინა ორ კვირაში და PQ დაფარვა >=70% თითო რეგიონში (`sorafs_orchestrator_pq_candidate_ratio`).
   - დაგეგმეთ მმართველობის განხილვის სლოტი, რომელიც ამტკიცებს კანარის ფანჯარას.
   - განაახლეთ `sorafs.gateway.rollout_phase = "ramp"` ინსცენირებაში (დაარედაქტირეთ ორკესტრი JSON და ხელახლა განათავსეთ) და გაატარეთ სარეკლამო მილსადენი.

2. **რელე კანარა (T დღე)**

   - მოაწყეთ ერთი რეგიონის პოპულარიზაცია ორკესტრატორზე `rollout_phase = "ramp"` დაყენებით და მონაწილე რელე მანიფესტებით.
   - დააკვირდით "პოლიტიკის მოვლენებს თითო შედეგზე" და "Brownout Rate"-ს PQ Ratchet-ის დაფაზე (რომელიც ახლა აღჭურვილია გაშვების პანელით) ორჯერ მეტი დამცავი ქეშით TTL.
   - ამოიღეთ `sorafs_cli guard-directory fetch` კადრები აუდიტის შესანახად გაშვებამდე და შემდეგ.

3. **კლიენტი/SDK canary (T პლუს 1 კვირა)**

   - გადაატრიალეთ `rollout_phase = "ramp"` კლიენტის კონფიგურაციებში ან გაიარეთ `stage-b` უგულებელყოფა დანიშნული SDK კოჰორტებისთვის.
   - გადაიღეთ ტელემეტრიული განსხვავებები (`sorafs_orchestrator_policy_events_total` დაჯგუფებული `client_id` და `region`) და მიამაგრეთ ისინი ინციდენტების ჩანაწერში.

4. **ნაგულისხმევი აქცია (T პლუს 3 კვირა)**

   - როგორც კი მმართველობა გამორთულია, გადართეთ ორკესტრის და კლიენტის კონფიგურაციები `rollout_phase = "default"`-ზე და გადაატრიალეთ ხელმოწერილი მზადყოფნის საკონტროლო სია გამოშვების არტეფაქტებში.

## მმართველობა და მტკიცებულებათა ჩამონათვალი

| ფაზის შეცვლა | სარეკლამო კარიბჭე | მტკიცებულებათა ნაკრები | დაფები და გაფრთხილებები |
|--------------|---------------|----------------|--------------------|
| Canary → Ramp *(B ეტაპის გადახედვა)* | სტადია-A შეფერხების მაჩვენებელი <1% ბოლო 14 დღის განმავლობაში, `sorafs_orchestrator_pq_candidate_ratio` ≥ 0,7 თითო დაწინაურებულ რეგიონში, Argon2 ბილეთის დადასტურების p95 < 50 ms და დაჯავშნული აქციის მართვის სლოტი. | `cargo xtask soranet-rollout-plan` JSON/Markdown წყვილი, დაწყვილებული `sorafs_cli guard-directory fetch` სნეპშოტები (ადრე/შემდეგ), ხელმოწერილი `cargo xtask soranet-rollout-capture --label canary` პაკეტი და კანარის წუთების მითითება [PQ ratchet runbook](I180000000). | `dashboards/grafana/soranet_pq_ratchet.json` (პოლიტიკის მოვლენები + ბრუნვის სიხშირე), `dashboards/grafana/soranet_privacy_metrics.json` (SN16 დაქვეითების კოეფიციენტი), ტელემეტრიის მითითებები `docs/source/soranet/snnet16_telemetry_plan.md`-ში. |
| Ramp → ნაგულისხმევი *(C ეტაპის აღსრულება)* | 30-დღიანი SN16 ტელემეტრიის დამწვრობა დაფიქსირდა, `sn16_handshake_downgrade_total` ბინა საწყის ეტაპზე, `sorafs_orchestrator_brownouts_total` ნული კლიენტის კანარის დროს და პროქსის გადართვის რეპეტიცია შესულია. | `sorafs_cli proxy set-mode --mode gateway|direct` ტრანსკრიპტი, `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` გამომავალი, `sorafs_cli guard-directory verify` ჟურნალი და ხელმოწერილი `cargo xtask soranet-rollout-capture --label default` პაკეტი. | იგივე PQ Ratchet დაფა პლუს SN16 დაქვეითების პანელები დოკუმენტირებული `docs/source/sorafs_orchestrator_rollout.md`-ში და `dashboards/grafana/soranet_privacy_metrics.json`-ში. |
| გადაუდებელი დაქვეითება/დაბრუნების მზადყოფნა | ამოქმედდება, როდესაც დაქვეითების მრიცხველები იზრდება, დამცავი დირექტორიის დადასტურება ვერ ხერხდება, ან `/policy/proxy-toggle` ბუფერული ჩანაწერები განაგრძობს შემცირების მოვლენებს. | საკონტროლო სია `docs/source/ops/soranet_transport_rollback.md`, `sorafs_cli guard-directory import` / `guard-cache prune` ჟურნალებიდან, `cargo xtask soranet-rollout-capture --label rollback`, ინციდენტის ბილეთები და შეტყობინებების შაბლონები. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` და ორივე გაფრთხილების პაკეტი (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- შეინახეთ ყველა არტეფაქტი `artifacts/soranet_pq_rollout/<timestamp>_<label>/`-ის ქვეშ, გენერირებული `rollout_capture.json`-ით, რათა მართვის პაკეტები შეიცავდეს ანგარიშების დაფას, პრომო ინსტრუმენტის კვალს და დისჯესტებს.
- მიამაგრეთ ატვირთული მტკიცებულებების SHA256 დისჯესტები (წუთები PDF, გადაღების ნაკრები, დაცვის კადრები) აქციის წუთებს, რათა პარლამენტის დამტკიცებები განმეორდეს დადგმის კლასტერზე წვდომის გარეშე.
- მიუთითეთ ტელემეტრიის გეგმა სარეკლამო ბილეთში, რათა დაამტკიცოთ, რომ `docs/source/soranet/snnet16_telemetry_plan.md` რჩება ლექსიკის დაქვეითების და გაფრთხილების ზღვრების კანონიკურ წყაროდ.

## დაფის და ტელემეტრიის განახლებები

`dashboards/grafana/soranet_pq_ratchet.json` ახლა მიეწოდება "გავრცელების გეგმის" ანოტაციის პანელს, რომელიც აკავშირებს ამ სახელმძღვანელოს და ასახავს მიმდინარე ფაზას, რათა მმართველობის მიმოხილვამ დაადასტუროს რომელი ეტაპია აქტიური. შეინახეთ პანელის აღწერა სინქრონიზებული კონფიგურაციის ღილაკების მომავალ ცვლილებებთან.

გაფრთხილებისთვის, დარწმუნდით, რომ არსებული წესები გამოიყენებს `stage` იარლიყს, რათა კანარის და ნაგულისხმევი ფაზები გამოიწვიოს ცალკეული პოლიტიკის ზღვრები (`dashboards/alerts/soranet_handshake_rules.yml`).

## დაბრუნების კაკვები

### ნაგულისხმევი → რემპი (სტადია C → ეტაპი B)

1. დააქვეითეთ ორკესტრატორი `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp`-ით (და ასახეთ იგივე ფაზა SDK კონფიგურაციებში), რათა B ეტაპი განახლდეს ფლოტის მასშტაბით.
2. აიძულეთ კლიენტები უსაფრთხო სატრანსპორტო პროფილში `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`-ის მეშვეობით, აღბეჭდეთ ტრანსკრიპტი, რათა `/policy/proxy-toggle` რემედიაციის სამუშაო ნაკადი დარჩეს აუდიტის ქვეშ.
3. გაუშვით `cargo xtask soranet-rollout-capture --label rollback-default` დამცავი დირექტორიის განსხვავებების არქივისთვის, პრომო ინსტრუმენტის გამომავალი და დაფის ეკრანის ანაბეჭდებისთვის `artifacts/soranet_pq_rollout/` ქვეშ.

### პანდუსი → კანარი (სტადია B → ეტაპი A)

1. შემოიტანეთ დაცვის დირექტორიის სნეპშოტი, რომელიც გადაღებულია დაწინაურებამდე `sorafs_cli guard-directory import --guard-directory guards.json`-ით და ხელახლა გაუშვით `sorafs_cli guard-directory verify`, რათა დაქვეითების პაკეტი შეიცავდეს ჰეშებს.
2. დააყენეთ `rollout_phase = "canary"` (ან გადაახვიეთ `anonymity_policy stage-a`-ით) ორკესტრატორისა და კლიენტის კონფიგურაციებზე, შემდეგ კვლავ დაუკარით PQ საბურღი საბურღი [PQ ratchet runbook] (./pq-ratchet-runbook.md) დაქვეითებული მილსადენის დასამტკიცებლად.
3. მიამაგრეთ განახლებული PQ Ratchet და SN16 ტელემეტრიის ეკრანის ანაბეჭდები პლუს გაფრთხილების შედეგები ინციდენტების ჟურნალში, სანამ აცნობებთ მმართველობას.

### გვარდიის შეხსენებები- მიუთითეთ `docs/source/ops/soranet_transport_rollback.md`, როდესაც ხდება დაქვეითება და დაარეგისტრირეთ ნებისმიერი დროებითი შემარბილებელი საშუალება, როგორც `TODO:` ელემენტი გაშვების ტრეკერში შემდგომი მუშაობისთვის.
- შეინახეთ `dashboards/alerts/soranet_handshake_rules.yml` და `dashboards/alerts/soranet_privacy_rules.yml` `promtool test rules` დაფარვის ქვეშ უკან დაბრუნებამდე და მის შემდეგ, რათა გაფრთხილების დრიფტი დოკუმენტირებული იყოს გადაღების პაკეტთან ერთად.