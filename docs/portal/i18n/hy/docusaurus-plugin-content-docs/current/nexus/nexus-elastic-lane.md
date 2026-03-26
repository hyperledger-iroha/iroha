---
id: nexus-elastic-lane
lang: hy
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Elastic lane provisioning (NX-7)
sidebar_label: Elastic Lane Provisioning
description: Bootstrap workflow for creating Nexus lane manifests, catalog entries, and rollout evidence.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Կանոնական աղբյուր
Այս էջը արտացոլում է `docs/source/nexus_elastic_lane.md`: Պահպանեք երկու պատճենները հավասարեցված, մինչև թարգմանության մաքրումը տեղակայվի պորտալում:
:::

# Elastic Lane Provisioning Toolkit (NX-7)

> **Ճանապարհային քարտեզի կետ.** NX-7 — Առաձգական գոտիների ապահովման գործիքավորում  
> **Կարգավիճակ.** Գործիքավորումն ավարտված է — ստեղծում է մանիֆեստներ, կատալոգի հատվածներ, Norito օգտակար բեռներ, ծխի թեստեր,
> և բեռնվածության թեստի փաթեթի օգնականն այժմ կարում է անցքի հետաձգման դարպասը + ապացույցը ցույց է տալիս, որ վավերացնող է
> բեռնման գործարկումները կարող են հրապարակվել առանց պատվերով գրելու:

Այս ուղեցույցը օպերատորներին ուղեկցում է նոր `scripts/nexus_lane_bootstrap.sh` օգնականի միջոցով, որն ավտոմատացնում է
երթուղու մանիֆեստի ստեղծում, գծի/տվյալների տարածության կատալոգի հատվածներ և հրապարակման ապացույցներ: Նպատակը դարձնելն է
հեշտ է պտտել Nexus նոր ուղիները (հանրային կամ մասնավոր) առանց բազմաթիվ ֆայլեր ձեռքով խմբագրելու կամ
կատալոգի երկրաչափությունը ձեռքով վերահղում:

## 1. Նախադրյալներ

1. Գոտի կեղծանունների, տվյալների տարածության, վավերացնողների հավաքածուի, սխալների հանդուրժողականության (`f`) և կարգավորման քաղաքականության հաստատում:
2. Վերջնական վավերացնող ցուցակ (հաշվի ID-ներ) և պաշտպանված անունների ցանկ:
3. Մուտք գործեք հանգույցի կազմաձևման պահոց, որպեսզի կարողանաք կցել ստեղծված հատվածները:
4. Գոտի մանիֆեստի ռեեստրի ուղիները (տես `nexus.registry.manifest_directory` և
   `cache_directory`):
5. Հեռուստաչափական կոնտակտներ/PagerDuty բռնակներ գծի համար, որպեսզի զգուշացումները հնարավոր լինի միացնել գծի վրա
   գալիս է առցանց:

## 2. Ստեղծեք գծի արտեֆակտներ

Գործարկեք օգնականը պահեստի արմատից.

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Հիմնական դրոշներ.

- `--lane-id`-ը պետք է համապատասխանի `nexus.lane_catalog`-ի նոր մուտքի ցուցանիշին:
- `--dataspace-alias` և `--dataspace-id/hash` վերահսկում են տվյալների տարածության կատալոգի մուտքագրումը (կանխադրված է
  lane id, երբ բաց է թողնված):
- `--validator`-ը կարող է կրկնվել կամ ստացվել `--validators-file`-ից:
- `--route-instruction` / `--route-account` թողարկում են պատրաստի տեղադրման երթուղային կանոններ:
- `--metadata key=value` (կամ `--telemetry-contact/channel/runbook`) գրավել runbook կոնտակտները, որպեսզի
  վահանակները անմիջապես նշում են ճիշտ սեփականատերերը:
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` ավելացրեք գործարկման ժամանակի թարմացման կեռիկը մանիֆեստին
  երբ գոտին պահանջում է օպերատորի ընդլայնված հսկողություն:
- `--encode-space-directory`-ը ավտոմատ կերպով կանչում է `cargo xtask space-directory encode`-ը: Զուգակցել այն
  `--space-directory-out`, երբ ցանկանում եք կոդավորված `.to` ֆայլը լռելյայնից այլ տեղ:

Սցենարը արտադրում է երեք արտեֆակտ `--output-dir`-ի ներսում (կանխադրված է ընթացիկ գրացուցակում),
գումարած կամընտիր չորրորդը, երբ կոդավորումը միացված է.

1. `<slug>.manifest.json` — երթուղու մանիֆեստ, որը պարունակում է վավերացման քվորում, պաշտպանված անունների տարածքներ և
   կամընտիր գործարկման ժամանակի արդիականացման կեռիկի մետատվյալներ:
2. `<slug>.catalog.toml` — TOML հատված՝ `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]`,
   և ցանկացած պահանջվող երթուղային կանոններ: Համոզվեք, որ `fault_tolerance`-ը սահմանված է տվյալների տարածության մուտքագրման չափի վրա
   գոտի-ռելեի հանձնաժողով (`3f+1`):
3. `<slug>.summary.json` — աուդիտի ամփոփում, որը նկարագրում է երկրաչափությունը (սլագ, հատվածներ, մետատվյալներ) գումարած
   պահանջվող ներդրման քայլերը և ճշգրիտ `cargo xtask space-directory encode` հրամանը (տակ
   `space_directory_encode.command`): Կցեք այս JSON-ը մուտքի տոմսին՝ ապացույցների համար:
4. `<slug>.manifest.to` — արտանետվում է, երբ դրված է `--encode-space-directory`; պատրաստ է Torii-ի համար
   `iroha app space-directory manifest publish` հոսք.

Օգտագործեք `--dry-run`՝ JSON/հատվածները նախադիտելու համար՝ առանց ֆայլեր գրելու, և `--force`՝ վերագրանցելու համար
գոյություն ունեցող արտեֆակտներ.

## 3. Կիրառել փոփոխությունները

1. Պատճենեք JSON մանիֆեստը կազմաձևված `nexus.registry.manifest_directory`-ում (և քեշի մեջ
   գրացուցակը, եթե գրանցամատյանը արտացոլում է հեռավոր փաթեթներ): Գործարկեք ֆայլը, եթե մանիֆեստները տարբերակված են
   ձեր կոնֆիգուրացիայի ռեպո:
2. Կատալոգի հատվածը միացրեք `config/config.toml`-ին (կամ համապատասխան `config.d/*.toml`-ին): Ապահովել
   `nexus.lane_count`-ը առնվազն `lane_id + 1` է և թարմացրեք ցանկացած `nexus.routing_policy.rules`, որը
   պետք է ուղղել դեպի նոր գոտի:
3. Կոդավորեք (եթե բաց եք թողել `--encode-space-directory`) և հրապարակեք մանիֆեստը Տիեզերական գրացուցակում
   օգտագործելով ամփոփագրում նշված հրամանը (`space_directory_encode.command`): Սա առաջացնում է
   `.manifest.to` օգտակար բեռ Torii ակնկալում և գրանցում է ապացույցները աուդիտորների համար. ներկայացնել միջոցով
   `iroha app space-directory manifest publish`.
4. Գործարկեք `irohad --sora --config path/to/config.toml --trace-config` և արխիվացրեք հետքի արդյունքը
   թողարկման տոմսը: Սա ապացուցում է, որ նոր երկրաչափությունը համընկնում է գեներացված slug/kura հատվածներին:
5. Վերագործարկեք գծին հատկացված վավերացուցիչները, երբ մանիֆեստի/կատալոգի փոփոխությունները գործարկվեն: Պահել
   JSON-ի ամփոփագիրը ապագա աուդիտի տոմսում:

## 4. Ստեղծեք ռեեստրի բաշխման փաթեթ

Փաթեթավորեք ստեղծված մանիֆեստը և ծածկույթը, որպեսզի օպերատորները կարողանան առանց գոտիների կառավարման տվյալները բաշխել
կոնֆիգուրացիաների խմբագրում յուրաքանչյուր հոսթի վրա: Փաթեթի օգնականի պատճենները դրսևորվում են կանոնական դասավորության մեջ,
արտադրում է կամընտիր կառավարման կատալոգի ծածկույթ `nexus.registry.cache_directory`-ի համար և կարող է արտանետել
tarball անցանց փոխանցումների համար.

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Արդյունքներ:

1. `manifests/<slug>.manifest.json` — պատճենեք դրանք կազմաձևվածի մեջ
   `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` — ընկնել `nexus.registry.cache_directory`: Յուրաքանչյուր `--module`
   մուտքը դառնում է խցանվող մոդուլի սահմանում, որը հնարավորություն է տալիս կառավարման մոդուլների փոխանակում (NX-2)
   թարմացնելով քեշի ծածկույթը՝ `config.toml` խմբագրելու փոխարեն:
3. `summary.json` — ներառում է հեշեր, ծածկույթի մետատվյալներ և օպերատորի հրահանգներ:
4. Լրացուցիչ `registry_bundle.tar.*` — պատրաստ է SCP, S3 կամ արտեֆակտ հետագծերի համար:

Համաժամեցրեք ամբողջ գրացուցակը (կամ արխիվը) յուրաքանչյուր վավերացնողի հետ, հանեք օդային բաց հոսթերից և պատճենեք
մանիֆեստները + քեշը ծածկում են իրենց ռեեստրի ուղիները, նախքան Torii-ը վերագործարկելը:

## 5. Վալիդատորի ծխի թեստեր

Torii-ը վերագործարկվելուց հետո գործարկեք ծխի նոր օգնականը՝ ստուգելու գծի `manifest_ready=true` հաղորդումները,
Չափիչները ցույց են տալիս ակնկալվող գծերի քանակը, և կնքված չափիչը պարզ է: Երթուղիներ, որոնք պահանջում են մանիֆեստներ
պետք է բացահայտի ոչ դատարկ `manifest_path`; օգնականն այժմ ձախողվում է անմիջապես, երբ ճանապարհը բացակայում է
NX-7 տեղակայման յուրաքանչյուր գրառում ներառում է ստորագրված մանիֆեստի ապացույցներ.

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Ավելացրեք `--insecure` ինքնաստորագրված միջավայրերը փորձարկելիս: Սցենարը դուրս է գալիս ոչ զրոյից, եթե գոտին է
բացակայում է, կնքված է, կամ չափումները/հեռաչափությունը շեղվում են ակնկալվող արժեքներից: Օգտագործեք
`--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` և
`--max-headroom-events` կոճակներ՝ յուրաքանչյուր գծի բլոկի բարձրությունը/վերջնականությունը/հետախոտը/գլխի սենյակի հեռաչափությունը պահպանելու համար
ձեր գործառնական ծրարների մեջ և միացրեք դրանք `--max-slot-p95` / `--max-slot-p99`-ի հետ
(գումարած `--min-slot-samples`)՝ առանց օգնականից հեռանալու NX‑18 միջանցքի տևողության թիրախները կիրառելու համար:

Օդային բաց վավերացումների (կամ CI) համար դուք կարող եք վերարտադրել Torii ստացված պատասխանը՝ ուղիղ եթերի փոխարեն:
վերջնակետ:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`fixtures/nexus/lanes/`-ի տակ գրանցված սարքերը արտացոլում են արտեֆակտները, որոնք արտադրվել են բեռնախցիկի միջոցով
օգնական, որպեսզի նոր մանիֆեստները կարող են տեղադրվել առանց պատվերով գրելու: CI-ն իրականացնում է նույն հոսքը միջոցով
`ci/check_nexus_lane_smoke.sh` և `ci/check_nexus_lane_registry_bundle.sh`
(անունը՝ `make check-nexus-lanes`) ապացուցելու, որ NX-7 ծխի օգնականը համահունչ է մնում հրապարակվածին
ծանրաբեռնվածության ձևաչափը և ապահովելու համար, որ փաթեթների մարսողությունները/վերածումները մնում են վերարտադրելի:

Երբ գոտին վերանվանվում է, նկարեք `nexus.lane.topology` հեռաչափության իրադարձությունները (օրինակ՝
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) և ետ կերակրեք դրանք
ծխի օգնականը. `--telemetry-file/--from-telemetry` դրոշակն ընդունում է նոր տողով սահմանազատված գրանցամատյանը և
`--require-alias-migration old:new`-ը պնդում է, որ `alias_migrated` իրադարձությունը գրանցել է վերանվանումը.

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

`telemetry_alias_migrated.ndjson` սարքը միավորում է կանոնական վերանվանման նմուշը, որպեսզի CI-ն կարողանա ստուգել
հեռաչափության վերլուծության ուղին՝ առանց կենդանի հանգույցի հետ կապվելու:

## Վավերացնողի բեռնվածության թեստեր (NX-7 վկայություն)

Ճանապարհային քարտեզը **NX-7** պահանջում է, որ յուրաքանչյուր նոր գիծ ուղարկվի վերարտադրվող վավերացնող բեռնվածություն: Օգտագործեք
`scripts/nexus_lane_load_test.py` ծխի չեկերը, անցք տևողությամբ դարպասները և անցք կապոցը կարելու համար
դրսևորվում է մեկ արտեֆակտ հավաքածուի մեջ, որը կառավարումը կարող է վերարտադրել.

```bash
scripts/nexus_lane_load_test.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --slot-range 81200-81600 \
  --workload-seed NX7-PAYMENTS-2026Q2 \
  --require-alias-migration core:payments \
  --out-dir artifacts/nexus/load/payments-2026q2
```

Օգնականը կիրառում է նույն DA-ի քվորումը, oracle-ը, հաշվարկային բուֆերը, TEU-ը և օգտագործված միջանցքի տևողության դարպասները
ծխի օգնականի կողմից և գրում է `smoke.log`, `slot_summary.json`, բնիկ փաթեթի մանիֆեստ և
`load_test_manifest.json` ընտրված `--out-dir`-ի մեջ, որպեսզի բեռնվածության գործարկումները կարող են ուղղակիորեն կցվել
թողարկեք տոմսեր՝ առանց պատվերով գրելու:

## 6. Հեռուստաչափություն և կառավարման հետևում

- Թարմացրեք գոտիների վահանակները (`dashboards/grafana/nexus_lanes.json` և հարակից ծածկույթները)
  նոր երթուղու ID և մետատվյալներ: Ստեղծված մետատվյալների ստեղները (`contact`, `channel`, `runbook` և այլն) կազմում են.
  պարզ է նախապես լրացնել պիտակները:
- Wire PagerDuty/Alertmanager-ի կանոնները նոր գծի համար՝ նախքան ընդունելությունը միացնելը: `summary.json`
  հաջորդ քայլերի զանգվածը արտացոլում է ստուգաթերթը [Nexus գործողություններում] (./nexus-operations):
- Գրանցեք մանիֆեստի փաթեթը Տիեզերական գրացուցակում, երբ վավերացնողի հավաքածուն ակտիվանա: Օգտագործեք նույնը
  մանիֆեստ JSON-ը, որը ստեղծվել է օգնականի կողմից, ստորագրված՝ համաձայն կառավարման ուղեցույցի:
- Հետևեք [Sora Nexus օպերատորի միացում] (./nexus-operator-onboarding) ծխի թեստերի համար (FindNetworkStatus, Torii
  հասանելիություն) և վերը բերված արտեֆակտների հավաքածուի միջոցով գրավել ապացույցները:

## 7. Չոր վազքի օրինակ

Առանց ֆայլեր գրելու արտեֆակտները նախադիտելու համար.

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --dry-run
```

Հրամանը տպում է JSON ամփոփագիրը և TOML հատվածը stdout-ում՝ թույլ տալով արագ կրկնել
պլանավորում։

---

Լրացուցիչ համատեքստի համար տե՛ս.- [Nexus գործողություններ](./nexus-operations) — գործառնական ստուգաթերթի և հեռաչափության պահանջներ:
- [Sora Nexus օպերատորի ներբեռնում] (./nexus-operator-onboarding) – մանրամասն ներբեռնման հոսք, որը հղում է անում
  նոր օգնական.
- [Nexus գծի մոդել] (./nexus-lane-model) — երթուղու երկրաչափություն, սլագներ և պահեստավորման դասավորություն, որն օգտագործվում է գործիքի կողմից: