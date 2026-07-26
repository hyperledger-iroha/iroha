---
lang: hy
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9774dca76eff9ff13fcad9bf1fa7f084b95a987c392727cf0e6a74a4844e2b8e
source_last_modified: "2026-01-22T14:45:01.375247+00:00"
translation_last_reviewed: 2026-02-07
id: pq-rollout-plan
title: SNNet-16G Post-Quantum Rollout Playbook
sidebar_label: PQ Rollout Plan
description: Operational guide for promoting the SoraNet hybrid X25519+ML-KEM handshake from canary to default across relays, clients, and SDKs.
translator: machine-google-reviewed
---

:::note Կանոնական աղբյուր
:::

SNNet-16G-ն ավարտում է SoraNet տրանսպորտի հետքվանտային թողարկումը: `rollout_phase` կոճակները թույլ են տալիս օպերատորներին համակարգել A Stage պահակային պահանջներից դետերմինիստական ​​առաջխաղացումը մինչև B փուլի մեծամասնության ծածկույթը և C աստիճանի խիստ PQ կեցվածքը՝ առանց խմբագրելու հում JSON/TOML յուրաքանչյուր մակերեսի համար:

Այս խաղագիրքը ներառում է.

- Փուլային սահմանումները և նոր կազմաձևման կոճակները (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) միացված են կոդի բազայում (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`):
- SDK-ի և CLI դրոշի քարտեզագրում, որպեսզի յուրաքանչյուր հաճախորդ կարողանա հետևել տարածմանը:
- Ռելեների/հաճախորդի դեղձանիկների պլանավորման ակնկալիքները, գումարած կառավարման վահանակները, որոնք մուտք են գործում դարպասի առաջխաղացում (`dashboards/grafana/soranet_pq_ratchet.json`):
- Վերադարձի կեռիկներ և հղումներ դեպի կրակային փորվածքների տեղեկագիր ([PQ ratchet runbook](./pq-ratchet-runbook.md)):

## Փուլային քարտեզ

| `rollout_phase` | Արդյունավետ անանունության փուլ | Կանխադրված էֆեկտ | Տիպիկ օգտագործման |
|---------------------------------------------------------------|--------------|
| `canary` | `anon-guard-pq` (Փուլ Ա) | Պահանջվում է առնվազն մեկ PQ պահակ յուրաքանչյուր շղթայում, մինչ նավատորմը տաքանում է: | Ելակետային և վաղ դեղձանիկ շաբաթներ: |
| `ramp` | `anon-majority-pq` (Փուլ B) | Շեղումների ընտրություն դեպի PQ ռելեներ >= երկու երրորդի ծածկույթի համար; դասական ռելեները մնում են որպես հետադարձ: | Տարածաշրջան առ տարածաշրջան ռելե դեղձանիկներ; SDK-ի նախադիտման միացումներ: |
| `default` | `anon-strict-pq` (Փուլ Գ) | Կիրառեք միայն PQ սխեմաները և խստացրեք վարկանիշի իջեցման ազդանշանները: | Վերջնական առաջխաղացում՝ հեռաչափության և կառավարման ստորագրման ավարտից հետո: |

Եթե ​​մակերեսը սահմանում է նաև հստակ `anonymity_policy`, այն վերացնում է այդ բաղադրիչի փուլը: Հստակ փուլը բաց թողնելն այժմ տեղափոխվում է `rollout_phase` արժեք, որպեսզի օպերատորները կարողանան մեկ անգամ շրջել փուլը յուրաքանչյուր միջավայրում և թույլ տալ, որ հաճախորդները ժառանգեն այն:

## Կազմաձևման հղում

### Նվագախումբ (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Նվագախմբի բեռնիչը լուծում է հետադարձ փուլը գործարկման ժամանակ (`crates/sorafs_orchestrator/src/lib.rs:2229`) և այն ցուցադրում է `sorafs_orchestrator_policy_events_total` և `sorafs_orchestrator_pq_ratio_*` միջոցով: Տե՛ս `docs/examples/sorafs_rollout_stage_b.toml` և `docs/examples/sorafs_rollout_stage_c.toml`՝ կիրառման համար պատրաստ հատվածների համար:

### Rust հաճախորդ / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client`-ն այժմ գրանցում է վերլուծված փուլը (`crates/iroha/src/client.rs:2315`), այնպես որ օգնական հրամանները (օրինակ՝ `iroha_cli app sorafs fetch`) կարող են զեկուցել ընթացիկ փուլի մասին լռելյայն անանունության քաղաքականության հետ մեկտեղ:

## Ավտոմատացում

Երկու `cargo xtask` օգնականներ ավտոմատացնում են ժամանակացույցի ստեղծումը և արտեֆակտերի նկարահանումը:

1. **Ստեղծեք տարածաշրջանային ժամանակացույցը**

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

   Տևողությունները ընդունում են `s`, `m`, `h`, կամ `d` վերջածանցները: Հրամանը թողարկում է `artifacts/soranet_pq_rollout_plan.json` և Markdown ամփոփագիր (`artifacts/soranet_pq_rollout_plan.md`), որը կարող է առաքվել փոփոխության հարցումով:

2. **Գրավեք հորատման արտեֆակտները ստորագրություններով**

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

   Հրամանը մատակարարված ֆայլերը պատճենում է `artifacts/soranet_pq_rollout/<timestamp>_<label>/`-ում, հաշվարկում է BLAKE3-ի ամփոփումները յուրաքանչյուր արտեֆակտի համար և գրում `rollout_capture.json`, որը պարունակում է մետատվյալներ գումարած Ed25519 ստորագրությունը օգտակար բեռի վրա: Օգտագործեք նույն անձնական բանալին, որը ստորագրում է կրակի վարման արձանագրությունը, որպեսզի կառավարումը կարողանա արագ վավերացնել գրավումը:

## SDK & CLI դրոշի մատրիցա

| Մակերեւութային | Կանարյան (Ա փուլ) | Թեքահարթակ (Բ փուլ) | Կանխադրված (Փուլ Գ) |
|---------|-----------------|------------------------------------|
| `sorafs_cli` բերման | `--anonymity-policy stage-a` կամ ապավինել փուլին | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Նվագախմբի կազմաձևում JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust client config (`iroha.toml`) | `rollout_phase = "canary"` (լռելյայն) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` ստորագրված հրամաններ | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, ընտրովի `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, ընտրովի `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, ընտրովի `.ANON_STRICT_PQ` |
| JavaScript նվագախմբի օգնականներ | `rolloutPhase: "canary"` կամ `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Բոլոր SDK-ն փոխում է քարտեզը նվագախմբի կողմից օգտագործվող նույն բեմական վերլուծիչի վրա (`crates/sorafs_orchestrator/src/lib.rs:365`), այնպես որ խառը լեզվով տեղակայումները մնում են կազմաձևված փուլի կողպեքին:

## Կանարյան պլանավորման ստուգաթերթ

1. **Նախաթռիչք (T հանած 2 շաբաթ)**

- Հաստատեք Ա-ի բաշխման մակարդակը <1% նախորդ երկու շաբաթվա ընթացքում և PQ ծածկույթը >=70% մեկ տարածաշրջանում (`sorafs_orchestrator_pq_candidate_ratio`):
   - Պլանավորեք կառավարման վերանայման անցքը, որը հաստատում է դեղձանիկի պատուհանը:
   - Թարմացրեք `sorafs.gateway.rollout_phase = "ramp"`-ը բեմադրության մեջ (խմբագրել JSON նվագախմբին և վերաբաշխել) և չորացնել խթանման խողովակաշարը:

2. **Ռելե դեղձանիկ (T օր)**

   - Միանգամից խթանեք մեկ շրջան՝ նվագախմբի վրա դնելով `rollout_phase = "ramp"` և մասնակցող ռելեի մանիֆեստները:
   - Մշտադիտարկեք «Քաղաքական իրադարձությունները մեկ արդյունքի համար» և «Brownout Rate»-ը PQ Ratchet վահանակում (որն այժմ ցուցադրում է տեղադրման վահանակը) կրկնակի գերազանցող պահակային քեշի TTL-ով:
   - Կտրեք `sorafs_cli guard-directory fetch` նկարները աուդիտի պահեստավորման համար գործարկումից առաջ և հետո:

3. **Հաճախորդ/SDK դեղձանիկ (T գումարած 1 շաբաթ)**

   - Շրջեք `rollout_phase = "ramp"`-ը հաճախորդի կոնֆիգուրացիաներում կամ անցեք `stage-b` վերագրանցումներ՝ նշանակված SDK խմբերի համար:
   - Ձեռք բերեք հեռաչափության տարբերությունները (`sorafs_orchestrator_policy_events_total` խմբավորված ըստ `client_id`-ի և `region`-ի) և կցեք դրանք տեղադրման միջադեպերի մատյանին:

4. **Լռելյայն առաջխաղացում (T գումարած 3 շաբաթ)**

   - Երբ կառավարումն անջատվի, փոխարկեք և՛ նվագախմբի, և՛ հաճախորդի կազմաձևերը `rollout_phase = "default"`-ի և պտտեք ստորագրված պատրաստության ստուգաթերթը թողարկման արտեֆակտների մեջ:

## Կառավարում և ապացույցների ստուգաթերթ

| Փուլային փոփոխություն | Խթանման դարպաս | Ապացույցների փաթեթ | Վահանակներ և ահազանգեր |
|--------------|----------------------------------------------------------|
| Canary → Ramp *(Փուլ B նախադիտում)* | Փուլ-A բաշխման տոկոսադրույքը <1% վերջին 14 օրվա ընթացքում, `sorafs_orchestrator_pq_candidate_ratio` ≥ 0,7 յուրաքանչյուր խթանված տարածաշրջանի համար, Argon2 տոմսի ստուգման p95 < 50 ms, և ամրագրված առաջխաղացման կառավարման անցքը: | `cargo xtask soranet-rollout-plan` JSON/Markdown զույգ, զուգակցված `sorafs_cli guard-directory fetch` նկարներ (առաջ/հետո), ստորագրված `cargo xtask soranet-rollout-capture --label canary` փաթեթ և դեղձանիկ րոպեների հղումներ [PQ ratchet runbook] (I18NU050): | `dashboards/grafana/soranet_pq_ratchet.json` (Քաղաքական իրադարձություններ + անկման արագություն), `dashboards/grafana/soranet_privacy_metrics.json` (SN16 իջեցման գործակից), հեռաչափության հղումներ `docs/source/soranet/snnet16_telemetry_plan.md`-ում: |
| Թեքահարթակ → Կանխադրված *(Գ փուլի կատարում)* | 30-օրյա SN16 հեռաչափության այրումը, `sn16_handshake_downgrade_total` հարթ է ելակետում, `sorafs_orchestrator_brownouts_total` զրո հաճախորդի դեղձանիկի ժամանակ, և պրոքսի փոխարկիչի փորձը գրանցված է: | `sorafs_cli proxy set-mode --mode gateway|direct` տառադարձում, `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` ելք, `sorafs_cli guard-directory verify` մատյան և ստորագրված `cargo xtask soranet-rollout-capture --label default` փաթեթ: | Նույն PQ Ratchet board-ը գումարած SN16 իջեցման վահանակները, որոնք փաստաթղթավորված են `docs/source/sorafs_orchestrator_rollout.md` և `dashboards/grafana/soranet_privacy_metrics.json`-ում: |
| Արտակարգ իրավիճակների իջեցում / հետդարձի պատրաստություն | Գործարկվում է, երբ նվազեցման հաշվիչներն աճում են, պահակային գրացուցակի ստուգումը ձախողվում է, կամ `/policy/proxy-toggle` բուֆերային գրառումները պահպանում են վարկանիշի իջեցման իրադարձությունները: | Ստուգաթերթ `docs/source/ops/soranet_transport_rollback.md`, `sorafs_cli guard-directory import` / `guard-cache prune` տեղեկամատյաններից, `cargo xtask soranet-rollout-capture --label rollback`, միջադեպերի տոմսերից և ծանուցման ձևանմուշներից: | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` և երկու ազդանշանային փաթեթներ (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`): |

- Պահպանեք յուրաքանչյուր արտեֆակտ `artifacts/soranet_pq_rollout/<timestamp>_<label>/`-ի տակ՝ ստեղծված `rollout_capture.json`-ի հետ, որպեսզի կառավարման փաթեթները պարունակեն ցուցատախտակ, գովազդային գործիքների հետքեր և ամփոփումներ:
- Կցեք SHA256 վերբեռնված ապացույցների ամփոփումները (րոպե PDF, նկարահանման փաթեթ, պահակային նկարներ) առաջխաղացման արձանագրությանը, որպեսզի խորհրդարանի հաստատումները կարող են վերարտադրվել առանց բեմականացման կլաստերի մուտքի:
- Նշեք հեռաչափության պլանը խթանման տոմսում՝ ապացուցելու համար, որ `docs/source/soranet/snnet16_telemetry_plan.md`-ը մնում է բառապաշարների իջեցման և զգուշացման շեմերի կանոնական աղբյուրը:

## Վահանակի և հեռաչափության թարմացումներ

`dashboards/grafana/soranet_pq_ratchet.json`-ն այժմ առաքվում է «Տարադրման պլան» ծանոթագրությունների վահանակով, որը կապվում է այս գրքույկի հետ և ներկայացնում ընթացիկ փուլը, որպեսզի կառավարման վերանայումները կարողանան հաստատել, թե որ փուլն է ակտիվ: Պահպանեք վահանակի նկարագրությունը համաժամեցված կազմաձևման կոճակների հետագա փոփոխությունների հետ:

Զգուշացման համար համոզվեք, որ գոյություն ունեցող կանոնները օգտագործում են `stage` պիտակը, որպեսզի կանարյան և լռելյայն փուլերը գործարկեն առանձին քաղաքականության շեմեր (`dashboards/alerts/soranet_handshake_rules.yml`):

## Վերադարձ կեռիկներ

### Կանխադրված → թեքահարթակ (Փուլ C → Փուլ B)

1. Նվագախմբին իջեցրեք `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp`-ով (և արտացոլեք նույն փուլը SDK-ի կազմաձևերում), որպեսզի B փուլը վերսկսվի ամբողջ նավատորմի վրա:
2. Ստիպեք հաճախորդներին մտնել անվտանգ տրանսպորտային պրոֆիլ `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`-ի միջոցով՝ ֆիքսելով տառադարձումը, որպեսզի `/policy/proxy-toggle` վերականգնման աշխատանքային հոսքը մնա ստուգելի:
3. Գործարկեք `cargo xtask soranet-rollout-capture --label rollback-default`՝ պահակային գրացուցակի տարբերություններն արխիվացնելու համար, պրոմգործիքների ելքը և վահանակի սքրինշոթները `artifacts/soranet_pq_rollout/` տակ:

### թեքահարթակ → Կանարի (Փուլ B → Փուլ Ա)

1. Ներմուծեք `sorafs_cli guard-directory import --guard-directory guards.json`-ով նկարահանված պահակային գրացուցակի լուսանկարը, որը նկարահանվել է առաջխաղացումից առաջ և կրկնել `sorafs_cli guard-directory verify`, որպեսզի իջեցման փաթեթը ներառի հեշեր:
2. Տեղադրեք `rollout_phase = "canary"` (կամ փոխարինեք `anonymity_policy stage-a`-ով) նվագախմբի և հաճախորդի կոնֆիգուրացիաների վրա, այնուհետև վերարտադրեք PQ ցողունային հորատումը [PQ ratchet runbook]-ից (./pq-ratchet-runbook.md)՝ ապացուցելու նվազման խողովակաշարը:
3. Կցեք PQ Ratchet-ի և SN16 հեռաչափության թարմացված սքրինշոթները, գումարած ահազանգերի արդյունքները միջադեպերի մատյանում, նախքան ղեկավարությանը ծանուցելը:

### Պաշտպանիչի հիշեցումներ- Տեղեկացրեք `docs/source/ops/soranet_transport_rollback.md`-ին, երբ տեղի է ունենում իջեցում և գրանցեք ցանկացած ժամանակավոր մեղմացում որպես `TODO:` տարր՝ գործարկման հետագծում հետագա աշխատանքների համար:
- Պահպանեք `dashboards/alerts/soranet_handshake_rules.yml`-ը և `dashboards/alerts/soranet_privacy_rules.yml`-ը `promtool test rules` ծածկույթի տակ հետադարձից առաջ և հետո, որպեսզի զգոն շեղումը փաստաթղթավորվի գրավման փաթեթի կողքին: