---
lang: hy
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T14:35:37.510319+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Գաղտնի ակտիվների աուդիտ և գործառնությունների խաղագիրք, որը հղում է կատարում `roadmap.md:M4`-ին:

# Գաղտնի ակտիվների աուդիտ և գործառնությունների մատյան

Այս ուղեցույցը համախմբում է այն ապացույցները, որոնց վրա հենվում են աուդիտորները և օպերատորները
գաղտնի ակտիվների հոսքերը վավերացնելիս: Այն լրացնում է պտտման գրքույկը
(`docs/source/confidential_assets_rotation.md`) և տրամաչափման մատյան
(`docs/source/confidential_assets_calibration.md`):

## 1. Ընտրովի բացահայտում և իրադարձությունների հոսքեր

- Յուրաքանչյուր գաղտնի հրահանգ թողարկում է կառուցվածքային `ConfidentialEvent` ծանրաբեռնվածություն
  (`Shielded`, `Transferred`, `Unshielded`) նկարահանված
  `crates/iroha_data_model/src/events/data/events.rs:198` և սերիականացված է
  կատարողներ (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`):
  Ռեգրեսիայի հավաքածուն իրականացնում է կոնկրետ օգտակար բեռներ, որպեսզի աուդիտորները կարողանան ապավինել
  որոշիչ JSON դասավորություններ (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`):
- Torii-ը բացահայտում է այս իրադարձությունները ստանդարտ SSE/WebSocket խողովակաշարի միջոցով; աուդիտորներ
  բաժանորդագրվել՝ օգտագործելով `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`),
  ընտրովի շրջանակը մեկ ակտիվի սահմանմանը: CLI օրինակ.

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "rose#wonderland" } }'
  ```

- Քաղաքականության մետատվյալները և սպասվող անցումները հասանելի են միջոցով
  `GET /v1/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`), արտացոլված է Swift SDK-ով
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) և փաստաթղթավորված
  ինչպես գաղտնի ակտիվների դիզայնը, այնպես էլ SDK ուղեցույցները
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`):

## 2. Հեռաչափություն, վահանակներ և չափաբերման ապացույցներ

- Գործարկման ժամանակի չափման մակերևույթի ծառերի խորությունը, պարտավորությունների/սահմանների պատմությունը, արմատների հեռացումը
  հաշվիչներ և ստուգիչ-քեշ հարվածի հարաբերակցություններ
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`): Grafana վահանակներ են
  `dashboards/grafana/confidential_assets.json` առաքել համապատասխան վահանակները և
  ծանուցումներ՝ `docs/source/confidential_assets.md:401`-ում փաստաթղթավորված աշխատանքային հոսքով:
- Կալիբրացիոն աշխատանքներ (NS/op, gas/op, ns/gas) ստորագրված տեղեկամատյաններով
  `docs/source/confidential_assets_calibration.md`. Վերջին Apple Silicon
  NEON վազքը արխիվացված է
  `docs/source/confidential_assets_calibration_neon_20260428.log`, և նույնը
  մատյանում գրանցվում են SIMD-չեզոք և AVX2 պրոֆիլների ժամանակավոր հրաժարումները մինչև
  x86 հոսթերը միանում են առցանց:

## 3. Միջադեպի արձագանքման և օպերատորի առաջադրանքները

- Պտտման/արդիականացման ընթացակարգերը գործում են
  `docs/source/confidential_assets_rotation.md`, որը ներկայացնում է, թե ինչպես կարելի է բեմադրել նորը
  պարամետրերի փաթեթներ, ժամանակացույցի քաղաքականության թարմացումներ և տեղեկացնել դրամապանակներին/աուդիտորներին: Այն
  tracker (`docs/source/project_tracker/confidential_assets_phase_c.md`) ցուցակները
  runbook-ի սեփականատերերը և փորձի ակնկալիքները:
- Արտադրության փորձերի կամ վթարային պատուհանների համար օպերատորները կցում են ապացույցներ
  `status.md` գրառումները (օրինակ՝ բազմակողմանի փորձերի մատյան) և ներառում են.
  `curl` քաղաքականության անցումների ապացույց, Grafana նկարներ և համապատասխան իրադարձություն
  մարսում է, որպեսզի աուդիտորները կարողանան վերակառուցել անանուխ→ փոխանցել→ բացահայտել ժամանակացույցերը:

## 4. Արտաքին վերանայման կադենս

- Անվտանգության վերանայման շրջանակը. գաղտնի սխեմաներ, պարամետրերի գրանցամատյաններ, քաղաքականություն
  անցումներ և հեռաչափություն։ Այս փաստաթուղթը գումարած չափաբերման մատյանների ձևերը
  վաճառողներին ուղարկված ապացույցների փաթեթը. վերանայման ժամանակացույցը հետևվում է միջոցով
  M4 `docs/source/project_tracker/confidential_assets_phase_c.md`-ում:
- Օպերատորները պետք է թարմացնեն `status.md`-ը ցանկացած վաճառողի հայտնաբերման կամ հետագա գործողությունների վերաբերյալ
  գործողության տարրեր. Մինչև արտաքին վերանայման ավարտը, այս գրքույկը ծառայում է որպես
  գործառնական ելակետային աուդիտորները կարող են փորձարկել: