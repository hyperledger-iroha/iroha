---
lang: hy
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 676798a4cf7c3e7737a0f80640f3f268a2f625f92afdd359ac528881d2aeb046
source_last_modified: "2025-12-29T18:16:35.060950+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Փաստաթղթերի ավտոմատացման բազային (AND5)

Ճանապարհային քարտեզի AND5 կետը պահանջում է փաստաթղթեր, տեղայնացում և հրապարակում
ավտոմատացումը պետք է ստուգվի մինչև AND6 (CI & Compliance) սկսելը: Այս թղթապանակը
արձանագրում է հրամանները, արտեֆակտները և ապացույցների դասավորությունը, որոնք հղում են կատարում AND5/AND6-ին,
արտացոլելով պատկերված պլանները
`docs/source/sdk/android/developer_experience_plan.md` և
`docs/source/sdk/android/parity_dashboard_plan.md`.

## Խողովակաշարեր և հրամաններ

| Առաջադրանք | Հրաման(ներ) | Սպասվող արտեֆակտներ | Ծանոթագրություններ |
|------|---------------------------------|-------|
| Տեղայնացման անավարտ համաժամեցում | `python3 scripts/sync_docs_i18n.py` (ըստ ցանկության անցնել `--lang <code>` մեկ վազքի համար) | Մատյան ֆայլը, որը պահվում է `docs/automation/android/i18n/<timestamp>-sync.log`-ում, գումարած թարգմանված կոճակը պարտավորվում է | Պահում է `docs/i18n/manifest.json`-ը թարգմանված կոճղերի հետ համաժամեցված; տեղեկամատյանը գրանցում է հպված լեզուների կոդերը և ելակետում գրանցված git commit-ը: |
| Norito հարմարանք + հավասարության ստուգում | `ci/check_android_fixtures.sh` (փաթաթում է `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`) | Պատճենեք ստեղծված ամփոփագիրը JSON-ը `docs/automation/android/parity/<stamp>-summary.json` | Ստուգում է `java/iroha_android/src/test/resources` օգտակար բեռները, մանիֆեստի հեշերը և ստորագրված սարքերի երկարությունը: Կցեք ամփոփագիրը `artifacts/android/fixture_runs/`-ի տակ կադենսային ապացույցների կողքին: |
| Նմուշի մանիֆեստի և հրապարակման ապացույց | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (փորձարկումներ + SBOM + ծագում) | Ծագման փաթեթի մետատվյալները գումարած `docs/source/sdk/android/samples/`-ից ստացված `sample_manifest.json`-ը, որը պահվում է `docs/automation/android/samples/<version>/`-ում | Կապում է AND5 նմուշային հավելվածները և թողարկում ավտոմատացումը՝ ֆիքսեք ստեղծված մանիֆեստը, SBOM հեշը և ծագման մատյանը բետա վերանայման համար: |
| Պարիտետի վահանակի հոսք | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json`, որին հաջորդում է `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | Պատճենեք `metrics.prom` լուսանկարը կամ Grafana արտահանվող JSON-ը `docs/automation/android/parity/<stamp>-metrics.prom`-ում | Սնուցում է վահանակի պլանը, որպեսզի AND5/AND7 կառավարումը կարողանա ստուգել անվավեր ներկայացումների հաշվիչները և հեռաչափության ընդունումը: |

## Ապացույցների հավաքում

1. **Ամեն ինչի ժամանակի դրոշմակնիք։** Անվանեք ֆայլերը՝ օգտագործելով UTC ժամադրոշմները
   (`YYYYMMDDTHHMMSSZ`), ուստի հավասարության վահանակներ, կառավարման արձանագրություններ և հրապարակված
   փաստաթղթերը կարող են դետերմինիստական կերպով հղում կատարել նույն գործարկմանը:
2. **Հղումը կատարում է:** Յուրաքանչյուր գրանցամատյան պետք է ներառի գործարկման git commit հեշը:
   գումարած ցանկացած համապատասխան կոնֆիգուրացիա (օրինակ՝ `ANDROID_PARITY_PIPELINE_METADATA`):
   Երբ գաղտնիությունը պահանջում է խմբագրում, ներառեք նշում և հղում դեպի անվտանգ պահոց:
3. **Արխիվացրեք նվազագույն համատեքստը։** Մենք ստուգում ենք միայն կառուցվածքային ամփոփագրերը (JSON,
   `.prom`, `.log`): Ծանր արտեֆակտները (APK փաթեթներ, սքրինշոթներ) պետք է մնան
   `artifacts/` կամ օբյեկտի պահեստավորում՝ գրանցամատյանում գրանցված ստորագրված հեշով:
4. **Թարմացրեք կարգավիճակի գրառումները։** Երբ `status.md`-ում առաջանում են AND5 հանգուցային կետեր, մեջբերում
   համապատասխան ֆայլը (օրինակ՝ `docs/automation/android/parity/20260324T010203Z-summary.json`)
   այնպես որ աուդիտորները կարող են հետևել ելակետին՝ առանց CI տեղեկամատյանները քերելու:

Այս դասավորությանը հետևելը բավարարում է «փաստաթղթերի/ավտոմատացման բազային գծերը, որոնք հասանելի են
աուդիտ» նախապայման, որը AND6-ը մեջբերում և պահպանում է Android փաստաթղթերի ծրագիրը
հրապարակված պլանների հետ համընթաց։