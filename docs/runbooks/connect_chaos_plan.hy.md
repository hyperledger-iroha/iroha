---
lang: hy
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-12-29T18:16:35.913527+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Միացրեք քաոսի և սխալների փորձի պլանը (IOS3 / IOS7)

Այս գրքույկը սահմանում է կրկնվող քաոսի վարժությունները, որոնք բավարարում են IOS3/IOS7-ին
ճանապարհային քարտեզի գործողություն _«պլանավորել համատեղ քաոսի փորձը»_ (`roadmap.md:1527`): Զուգակցել այն
Միացման նախադիտման աշխատագիրքը (`docs/runbooks/connect_session_preview_runbook.md`)
խաչաձեւ SDK ցուցադրություններ բեմադրելիս:

## Նպատակներ և հաջողության չափանիշներ
- Կիրառեք ընդհանուր Connect-ի կրկնակի փորձի/հետաձգման քաղաքականությունը, անցանց հերթերի սահմանափակումները և
  Հեռաչափություն արտահանողներ վերահսկվող անսարքությունների ներքո՝ առանց արտադրության ծածկագրի մուտացիայի:
- Վերցրեք դետերմինիստական արտեֆակտներ (`iroha connect queue inspect` ելք,
  `connect.*` չափման նկարներ, Swift/Android/JS SDK տեղեկամատյաններ), որպեսզի կառավարումը կարողանա
  աուդիտ յուրաքանչյուր վարժանք:
- Ապացուցեք, որ դրամապանակները և dApps-ը հարգում են կազմաձևերի փոփոխությունները (դրսևորվող շեղումներ, աղ
  ռոտացիա, ատեստավորման ձախողումներ)՝ երեսապատելով կանոնական `ConnectError`
  կատեգորիայի և ռեդակցիայի համար անվտանգ հեռաչափության իրադարձություններ:

## Նախադրյալներ
1. **Շրջակա միջավայրի բեռնախցիկ**
   - Սկսեք Torii ցուցադրական փաթեթը՝ `scripts/ios_demo/start.sh --telemetry-profile full`:
   - Գործարկեք առնվազն մեկ SDK նմուշ (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`):
2. **Գործիքավորում**
   - Միացնել SDK ախտորոշումը (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     `ConnectSessionDiagnostics` Swift-ում; `ConnectQueueJournal` + `ConnectQueueJournalTests`
     համարժեքներ Android/JS-ում):
   - Համոզվեք, որ CLI `iroha connect queue inspect --sid <sid> --metrics`-ը լուծում է
     SDK-ի կողմից արտադրված հերթի ուղին (`~/.iroha/connect/<sid>/state.json` և
     `metrics.ndjson`):
   - Լարային հեռաչափության արտահանողներ, որպեսզի տեսանելի լինեն հետևյալ ժամանակային շարքերը
     Grafana և `scripts/swift_status_export.py telemetry`-ի միջոցով՝ `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **Ապացույցների թղթապանակներ** – ստեղծեք `artifacts/connect-chaos/<date>/` և պահեք.
   - չմշակված տեղեկամատյաններ (`*.log`), չափման նկարներ (`*.json`), վահանակի արտահանում
     (`*.png`), CLI ելքեր և PagerDuty ID-ներ:

## Սցենարների մատրիցա| ID | Սխալ | Ներարկման քայլեր | Ակնկալվող ազդանշաններ | Ապացույցներ |
|----|-------|-----------------|-----------------|----------|
| C1 | WebSocket-ի անջատում և վերամիացում | Փաթաթեք `/v1/connect/ws`-ը վստահված անձի հետևում (օրինակ՝ `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) կամ ժամանակավորապես արգելափակեք ծառայությունը (`kubectl scale deploy/torii --replicas=0` ≤60-ի համար): Ստիպեք դրամապանակին շարունակել շրջանակներ ուղարկել, որպեսզի անցանց հերթերը լցվեն: | `connect.reconnects_total` ավելանում է, `connect.resume_latency_ms` աճում է, բայց մնում է - Վահանակի ծանոթագրություն անջատման պատուհանի համար:- Նմուշի մատյանից հատված վերամիացում + արտահոսքի հաղորդագրություններով: |
| C2 | Անցանց հերթի գերհեռացում / TTL ժամկետի ավարտ | Կարկատել նմուշը՝ հերթի սահմանները կրճատելու համար (Swift. instantiate `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` `ConnectSessionDiagnostics` ներսում; Android/JS-ն օգտագործում է համապատասխան կոնստրուկտորներ): Կասեցրեք դրամապանակը ≥2× `retentionInterval`-ի համար, քանի դեռ dApp-ը շարունակում է հերթագրել հարցումները: | `connect.queue_dropped_total{reason="overflow"}` և `{reason="ttl"}` աճ, `connect.queue_depth` սարահարթեր նոր սահմանում, SDK-ների մակերես `ConnectError.QueueOverflow(limit: 4)` (կամ `.QueueExpired`): `iroha connect queue inspect`-ը ցույց է տալիս `state=Overflow`-ը `warn/drop` ջրանիշերով 100%: | - Չափիչ հաշվիչների սքրինշոթ:- CLI JSON ելքի արտահոսք գրավող:- Swift/Android մատյանի հատված, որը պարունակում է `ConnectError` տողը: |
| C3 | Ակնհայտ դրեյֆ / ընդունելության մերժում | Փակեք դրամապանակներին սպասարկվող Connect մանիֆեստը (օրինակ՝ փոփոխեք `docs/connect_swift_ios.md` նմուշի մանիֆեստը կամ սկսեք Torii-ը՝ `--connect-manifest-path`-ով, որը ցույց է տալիս `chain_id` կամ `permissions` պատճենը, որտեղ `permissions` է): Ստացեք dApp-ի խնդրանքի հաստատում և համոզվեք, որ դրամապանակը մերժվում է քաղաքականության միջոցով: | Torii-ը վերադարձնում է `HTTP 409` `manifest_mismatch`-ով `/v1/connect/session`-ի համար, SDK-ներն արտանետում են `ConnectError.Authorization.manifestMismatch(manifestVersion)`, հեռաչափությունը բարձրացնում է `connect.manifest_mismatch_total`-ը, իսկ SDK-ները թողնում են `connect.manifest_mismatch_total`, (`state=Idle`): | - Torii մատյանից քաղվածք, որը ցույց է տալիս անհամապատասխանության հայտնաբերումը:- բացահայտված սխալի SDK-ի սքրինշոթը:- Չափման պատկեր, որը ցույց է տալիս, որ թեստի ընթացքում հերթագրված շրջանակներ չկան: |
| C4 | Բանալին պտտվող / աղի տարբերակի բախում | Պտտեք Connect salt-ը կամ AEAD ստեղնը նիստի կեսին: Մշակողի կույտերում վերագործարկեք Torii-ը `CONNECT_SALT_VERSION=$((old+1))`-ով (արտահայտում է Android-ի խմբագրման աղի թեստը `docs/source/sdk/android/telemetry_schema_diff.md`-ում): Պահպանեք դրամապանակը ցանցից դուրս, մինչև աղի ռոտացիան ավարտվի, ապա վերսկսեք: | Վերսկսման առաջին փորձը ձախողվում է `ConnectError.Authorization.invalidSalt`-ով, հերթերը լցվում են (dApp-ը թողնում է քեշավորված շրջանակները `salt_version_mismatch` պատճառաբանությամբ), հեռաչափությունը թողարկում է `android.telemetry.redaction.salt_version` (Android) և `swift.connect.session_event{event="salt_rotation"}`: Երկրորդ աշխատաշրջանը SID-ի թարմացումից հետո: | - Վահանակի ծանոթագրություն աղի դարաշրջանից առաջ/հետո:- Անվավեր աղի սխալ և հետագա հաջողություն պարունակող մատյաններ:- `iroha connect queue inspect` ելք, որը ցույց է տալիս `state=Stalled`, որին հաջորդում է թարմ `state=Active`: || C5 | Ատեստավորում / StrongBox ձախողում | Android դրամապանակներում կարգավորեք `ConnectApproval`՝ ներառելով `attachments[]` + StrongBox ատեստավորումը: Օգտագործեք ատեստավորման զրահը (`scripts/android_keystore_attestation.sh` `--inject-failure strongbox-simulated`-ի հետ) կամ վնասեք ատեստավորման JSON-ը նախքան dApp-ին հանձնելը: | DApp-ը մերժում է հաստատումը `ConnectError.Authorization.invalidAttestation`-ով, Torii-ը գրանցում է ձախողման պատճառը, արտահանողները հարվածում են `connect.attestation_failed_total`-ին, իսկ հերթը մաքրում է վիրավորական մուտքը: Swift/JS dApps-ը գրանցում է սխալը, մինչդեռ նիստը կենդանի է պահում: | - Կառուցեք մատյանը ներարկված ձախողման ID-ով:- SDK-ի սխալների գրանցամատյան + հեռաչափության հաշվիչի գրանցում:- Ապացուցում է, որ հերթը հեռացրեց վատ շրջանակը (`recordsRemoved > 0`): |

## Սցենարի մանրամասները

### C1 — WebSocket-ի անջատում և վերամիացում
1. Փաթաթեք Torii վստահված անձի հետևում (toxiproxy, Envoy կամ `kubectl port-forward`), որպեսզի
   դուք կարող եք միացնել հասանելիությունը՝ առանց ամբողջ հանգույցը սպանելու:
2. Գործարկել 45 վրկ անջատում.
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. Դիտեք հեռաչափության վահանակները և «scripts/swift_status_export.py» հեռաչափությունը
   --json-out artifacts/connect-chaos//c1_metrics.json`:
4. Անջատումից անմիջապես հետո թափել հերթի վիճակը.
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. Հաջողություն = վերամիացման մեկ փորձ, սահմանափակ հերթի աճ և ավտոմատ
   արտահոսք վստահված անձի վերականգնումից հետո:

### C2 — Անցանց հերթի գերհոսք / TTL ժամկետի ավարտ
1. Նեղացնել հերթերի շեմերը տեղական շենքերում.
   - Swift. թարմացրեք `ConnectQueueJournal` սկզբնավորիչը ձեր նմուշի ներսում
     (օրինակ՝ `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     անցնել `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`.
   - Android/JS. փոխանցել համարժեք կազմաձևման օբյեկտը կառուցելիս
     `ConnectQueueJournal`.
2. Կասեցրեք դրամապանակը (սիմուլյատորի ֆոն կամ սարքի ինքնաթիռի ռեժիմ) ≥60 վրկ.
   մինչ dApp-ը թողարկում է `ConnectClient.requestSignature(...)` զանգեր:
3. Օգտագործեք `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) կամ JS
   ախտորոշիչ օգնական՝ ապացույցների փաթեթը արտահանելու համար (`state.json`, `journal/*.to`,
   `metrics.ndjson`):
4. Հաջողություն = հեղեղումների հաշվիչների ավելացում, SDK մակերեսներ `ConnectError.QueueOverflow`
   մեկ անգամ, իսկ դրամապանակը վերսկսելուց հետո հերթը վերականգնվում է:

### C3 — Ակնհայտ դրեյֆ / ընդունելության մերժում
1. Կատարեք ընդունելության մանիֆեստի պատճենը, օրինակ.
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. Գործարկեք Torii `--connect-manifest-path /tmp/manifest_drift.json`-ով (կամ
   թարմացնել docker compose/k8s կազմաձևը փորվածքի համար):
3. Փորձեք սեանս սկսել դրամապանակից; սպասել HTTP 409:
4. Վերցրեք Torii + SDK տեղեկամատյանները գումարած `connect.manifest_mismatch_total`-ից
   հեռաչափության վահանակ:
5. Հաջողություն = մերժում առանց հերթի աճի, գումարած դրամապանակը ցուցադրում է ընդհանուրը
   տաքսոնոմիայի սխալ (`ConnectError.Authorization.manifestMismatch`):### C4 - բանալիների պտույտ / աղի բախում
1. Ձայնագրեք ընթացիկ աղի տարբերակը հեռաչափությունից.
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Վերագործարկեք Torii-ը նոր աղով (`CONNECT_SALT_VERSION=$((OLD+1))` կամ թարմացրեք
   կազմաձևման քարտեզ): Պահպանեք դրամապանակն անցանց, մինչև վերագործարկման ավարտը:
3. Վերսկսել դրամապանակը; առաջին ռեզյումեն պետք է ձախողվի անվավեր աղի սխալով
   և `connect.queue_dropped_total{reason="salt_version_mismatch"}` ավելացումներ:
4. Ստիպեք հավելվածին թողնել քեշավորված շրջանակները՝ ջնջելով նիստերի գրացուցակը
   (`rm -rf ~/.iroha/connect/<sid>` կամ հարթակի հատուկ քեշը մաքրվում է), ապա
   վերսկսել նիստը թարմ նշաններով:
5. Հաջողություն = հեռաչափությունը ցույց է տալիս աղի բախումը, ռեզյումեի անվավեր իրադարձությունը գրանցվում է
   մեկ անգամ, իսկ հաջորդ նիստը հաջողվում է առանց ձեռքի միջամտության:

### C5 — Ատեստավորում / StrongBox ձախողում
1. Ստեղծեք ատեստավորման փաթեթ՝ օգտագործելով `scripts/android_keystore_attestation.sh`
   (սահմանեք `--inject-failure strongbox-simulated`՝ ստորագրության բիթը շրջելու համար):
2. Թող դրամապանակը միացնի այս փաթեթը իր `ConnectApproval` API-ի միջոցով; dApp-ը
   պետք է վավերացնի և մերժի օգտակար բեռը:
3. Ստուգեք հեռաչափությունը (`connect.attestation_failed_total`, Swift/Android միջադեպ
   չափումներ) և ապահովել, որ հերթը բաց թողեց թունավորված մուտքը:
4. Հաջողություն = մերժումը մեկուսացված է վատ հաստատումից, հերթերը մնում են առողջ,
   իսկ ատեստավորման մատյանը պահվում է փորված ապացույցների հետ միասին:

## Ապացույցների ստուգաթերթ
- `artifacts/connect-chaos/<date>/c*_metrics.json` արտահանում է
  `scripts/swift_status_export.py telemetry`.
- CLI ելքեր (`c*_queue.txt`) `iroha connect queue inspect`-ից:
- SDK + Torii տեղեկամատյանները ժամանակի դրոշմանիշներով և SID հեշերով:
- Վահանակի սքրինշոթներ՝ ծանոթագրություններով յուրաքանչյուր սցենարի համար:
- PagerDuty / միջադեպի ID-ներ, եթե Sev1/2 ահազանգերը գործարկվեն:

Ամբողջական մատրիցը եռամսյակը մեկ անգամ լրացնելը բավարարում է ճանապարհային քարտեզի դարպասը և
ցույց է տալիս, որ Swift/Android/JS Connect իրականացումները վճռականորեն են արձագանքում
ամենաբարձր ռիսկի ձախողման ռեժիմներում: