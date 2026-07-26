---
lang: hy
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 370733f000ffa7022cab18931c2697031a225d2ce3ae83382896c0d61f9fe6e2
source_last_modified: "2026-01-05T09:28:11.912569+00:00"
translation_last_reviewed: 2026-02-07
id: pq-ratchet-runbook
title: SoraNet PQ Ratchet Fire Drill
sidebar_label: PQ Ratchet Runbook
description: On-call rehearsal steps for promoting or demoting the staged PQ anonymity policy with deterministic telemetry validation.
translator: machine-google-reviewed
---

:::note Կանոնական աղբյուր
:::

## Նպատակը

Այս գրքույկը առաջնորդում է SoraNet-ի հետքվանտային (PQ) անանունության քաղաքականության բեմականացված կրակային հորատման հաջորդականությունը: Օպերատորները կրկնում են և՛ առաջխաղացումը (Փուլ A -> Փուլ B -> Փուլ C), և՛ վերահսկվող իջեցումը դեպի B/A փուլ, երբ PQ մատակարարումը նվազում է: Հորատումը վավերացնում է հեռաչափական կեռիկները (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) և հավաքում է արտեֆակտներ միջադեպերի փորձերի մատյանում:

## Նախադրյալներ

- Վերջին `sorafs_orchestrator` երկուական հնարավորությունների կշռումով (կատարել `docs/source/soranet/reports/pq_ratchet_validation.md`-ում ցուցադրված հորատման հղումը կամ դրանից հետո):
- Մուտք դեպի Prometheus/Grafana `dashboards/grafana/soranet_pq_ratchet.json` սպասարկող կույտ:
- Անվանական պահակային գրացուցակի լուսանկար: Վերցրեք և հաստատեք պատճենը նախքան վարումը.

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Եթե աղբյուրի գրացուցակը հրապարակում է միայն JSON, ապա պտտման օգնականները գործարկելուց առաջ այն նորից կոդավորեք Norito երկուական տարբերակով՝ `soranet-directory build`-ով:

- Վերցրեք մետատվյալները և թողարկողի ռոտացիայի նախնական փուլային արտեֆակտները CLI-ով.

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Փոփոխել պատուհանը, որը հաստատվել է ցանցային և դիտորդական հերթապահ թիմերի կողմից:

## Խթանման քայլեր

1. **Փուլային աուդիտ**

   Գրանցեք մեկնարկային փուլը.

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Սպասեք `anon-guard-pq` առաջխաղացմանը:

2. **Առաջարկեք B փուլ (մեծամասնության PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Սպասեք >=5 րոպե, մինչև մանիֆեստները թարմացվեն:
   - Grafana-ում (`SoraNet PQ Ratchet Drill` վահանակ) հաստատեք «Քաղաքական իրադարձություններ» վահանակը ցույց է տալիս `outcome=met` `stage=anon-majority-pq`-ի համար:
   - Լուսանկարեք սքրինշոթ կամ վահանակ JSON և կցեք այն միջադեպերի մատյանին:

3. **Առաջարկեք C փուլ (խիստ PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Ստուգեք `sorafs_orchestrator_pq_ratio_*` հիստոգրամների միտումը դեպի 1.0:
   - Հաստատեք, որ շագանակագույն հաշվիչը մնում է հարթ; հակառակ դեպքում հետևեք իջեցման քայլերին:

|

1. **Սինթետիկ PQ դեֆիցիտ առաջացնել **

   Անջատեք PQ ռելեները խաղահրապարակի միջավայրում՝ կրճատելով պահակային գրացուցակը միայն դասական գրառումների վրա, այնուհետև վերաբեռնեք նվագախմբի քեշը.

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Դիտարկեք շեղման հեռաչափությունը**

   - Վահանակ. «Brownout Rate» վահանակը բարձրանում է 0-ից:
   - PromQL՝ `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch`-ը պետք է զեկուցի `anonymity_outcome="brownout"` `anonymity_reason="missing_majority_pq"`-ի հետ:

3. **Նվազեցում B փուլ / փուլ A **

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Եթե PQ մատակարարումը դեռևս անբավարար է, իջեցրեք `anon-guard-pq`: Զորավարժությունն ավարտվում է այն բանից հետո, երբ լուծվեն բրոնխային հաշվիչներ, և առաջխաղացումները կարող են կրկին կիրառվել:

4. **Վերականգնել պահակային գրացուցակը**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Հեռամետրիա և արտեֆակտներ

- **Վահանակ՝** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus ահազանգեր.** ապահովել, որ `sorafs_orchestrator_policy_events_total` խափանման ազդանշանը մնա կազմաձևված SLO-ից ցածր (<5% ցանկացած 10 րոպեանոց պատուհանում):
- **Միջադեպերի մատյան.** կցեք նկարահանված հեռաչափության հատվածները և օպերատորի նշումները `docs/examples/soranet_pq_ratchet_fire_drill.log`-ին:
- **Ստորագրված նկարում.** օգտագործեք `cargo xtask soranet-rollout-capture`՝ հորատման գրանցամատյանը և ցուցատախտակը `artifacts/soranet_pq_rollout/<timestamp>/`-ում պատճենելու համար, հաշվարկեք BLAKE3 բովանդակությունը և ստացեք ստորագրված `rollout_capture.json`:

Օրինակ՝

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Ստեղծված մետատվյալները և ստորագրությունը կցեք կառավարման փաթեթին:

## Վերադարձ

Եթե փորվածքը բացահայտի PQ-ի իրական դեֆիցիտը, մնացեք Ա փուլում, տեղեկացրեք Ցանցային TL-ին և կցեք հավաքագրված չափումները, ինչպես նաև պահակային գրացուցակի տարբերությունները միջադեպերի հետագծողին: Օգտագործեք պահակային գրացուցակի արտահանումը, որը գրավել է ավելի վաղ՝ նորմալ ծառայությունը վերականգնելու համար:

:::tip Ռեգրեսիայի ծածկույթ
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics`-ն ապահովում է սինթետիկ վավերացում, որն ապահովում է այս փորվածքը:
:::