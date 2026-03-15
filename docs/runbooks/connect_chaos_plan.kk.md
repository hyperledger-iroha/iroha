---
lang: kk
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

# Хаос пен ақауларды қайталау жоспарын қосыңыз (IOS3 / IOS7)

Бұл оқулық IOS3/IOS7 талаптарын қанағаттандыратын қайталанатын хаос жаттығуларын анықтайды
жол картасы әрекеті _“бірлескен хаос репетициясын жоспарлау”_ (`roadmap.md:1527`). Онымен жұптаңыз
Connect алдын ала қарау кітабы (`docs/runbooks/connect_session_preview_runbook.md`)
кросс SDK демонстрацияларын қою кезінде.

## Мақсаттар мен табыс критерийлері
- Ортақ қосылымды қайталау/өшіру саясатын, желіден тыс кезек шектеулерін және
  телеметрия экспорттаушылары өндірістік кодты мутациясыз бақыланатын ақаулар.
- Детерминирленген артефактілерді түсіру (`iroha connect queue inspect` шығысы,
  `connect.*` метрикасының суреті, Swift/Android/JS SDK журналдары), сондықтан басқару
  әрбір жаттығуды тексеру.
- Әмияндар мен dApps конфигурация өзгерістерін (манифест дрейфтері, тұз
  айналу, аттестаттау ақаулары) канондық `ConnectError` бетін жабу арқылы
  санат және редакциялау қауіпсіз телеметрия оқиғалары.

## Алғышарттар
1. **Қоршаған ортаны жүктеу жолағы**
   - Torii демонстрациясын бастаңыз: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - Кем дегенде бір SDK үлгісін іске қосыңыз (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. **Аспаптар**
   - SDK диагностикасын қосу (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     Swift ішіндегі `ConnectSessionDiagnostics`; `ConnectQueueJournal` + `ConnectQueueJournalTests`
     Android/JS жүйесіндегі эквиваленттер).
   - CLI `iroha connect queue inspect --sid <sid> --metrics` шешілетініне көз жеткізіңіз
     SDK арқылы жасалған кезек жолы (`~/.iroha/connect/<sid>/state.json` және
     `metrics.ndjson`).
   - Сымды телеметрия экспорттаушылары, осылайша келесі уақыт қатарлары ішінде көрінеді
     Grafana және `scripts/swift_status_export.py telemetry` арқылы: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **Дәлелдер қалталары** – `artifacts/connect-chaos/<date>/` жасаңыз және сақтаңыз:
   - өңделмеген журналдар (`*.log`), метриканың суреті (`*.json`), бақылау тақтасының экспорттары
     (`*.png`), CLI шығыстары және PagerDuty идентификаторлары.

## Сценарий матрицасы| ID | Қате | Инъекция қадамдары | Күтілетін сигналдар | Дәлелдер |
|----|-------|-----------------|------------------|----------|
| C1 | WebSocket үзілуі және қайта қосылу | `/v2/connect/ws` проксиінің артына ораңыз (мысалы, `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) немесе қызметті уақытша блоктаңыз (≤60 секунд үшін `kubectl scale deploy/torii --replicas=0`). Офлайн кезектерді толтыру үшін әмиянды кадрларды жіберуді жалғастыруға мәжбүрлеңіз. | `connect.reconnects_total` өседі, `connect.resume_latency_ms` өседі, бірақ - Өшіру терезесіне арналған бақылау тақтасының аннотациясы.- Қайта қосу + суды төгу хабарлары бар үлгі журнал үзіндісі. |
| C2 | Офлайн кезек толып кету / TTL мерзімі | Кезек шектеулерін қысқарту үшін үлгіні түзетіңіз (Swift: `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` ішінде `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` үлгісін жасаңыз; Android/JS сәйкес конструкторларды пайдаланады). dApp сұрауларды кезекке қоюды жалғастырған кезде әмиянды ≥2× `retentionInterval` уақытқа тоқтатыңыз. | `connect.queue_dropped_total{reason="overflow"}` және `{reason="ttl"}` қадамы, `connect.queue_depth` үстірттері жаңа шекте, SDK беті `ConnectError.QueueOverflow(limit: 4)` (немесе `.QueueExpired`). `iroha connect queue inspect` `state=Overflow` `warn/drop` су белгілерімен 100% көрсетеді. | - метрикалық есептегіштердің скриншоты.- толып кетуді түсіретін CLI JSON шығысы.- `ConnectError` жолын қамтитын Swift/Android журнал үзіндісі. |
| C3 | Манифест дрейфі / қабылдаудан бас тарту | Әмияндарға қызмет көрсететін Connect манифестімен бұрмалау (мысалы, `docs/connect_swift_ios.md` үлгісі манифестін өзгерту немесе `--connect-manifest-path` көшірмесінде `--connect-manifest-path` арқылы `chain_id` немесе Grafana Torii бастаңыз). dApp сұрауын мақұлдаңыз және әмиянның саясат арқылы қабылданбағанына көз жеткізіңіз. | Torii `/v2/connect/session` үшін `manifest_mismatch` бар `HTTP 409` қайтарады, SDK `ConnectError.Authorization.manifestMismatch(manifestVersion)` шығарады, телеметрия `connect.manifest_mismatch_total` жоғарылайды және emp қалдырады (`state=Idle`). | - Сәйкессіздікті анықтауды көрсететін Torii журнал үзіндісі.- Қатенің SDK скриншоты.- Сынақ кезінде кезекте тұрған кадрлардың жоқтығын дәлелдейтін метрикалық сурет. |
| C4 | Кілттің айналуы / тұзды нұсқадағы соққы | Қосылу тұзын немесе AEAD пернесін сеанс ортасында бұраңыз. Әзірлеуші ​​стектерде Torii жүйесін `CONNECT_SALT_VERSION=$((old+1))` көмегімен қайта іске қосыңыз (`docs/source/sdk/android/telemetry_schema_diff.md` жүйесіндегі Android түзеткіш тұзының сынағы қайталанады). Тұз айналымы аяқталғанша әмиянды желіден тыс ұстаңыз, содан кейін жалғастырыңыз. | Бірінші жалғастыру әрекеті `ConnectError.Authorization.invalidSalt`, кезектерді тазарту (dApp `salt_version_mismatch` себебімен кэштелген кадрларды түсіреді), телеметрия `android.telemetry.redaction.salt_version` (Android) және `swift.connect.session_event{event="salt_rotation"}` шығарады. SID жаңартуынан кейінгі екінші сеанс сәтті аяқталды. | - Тұз дәуіріне дейінгі/кейінгі бақылау тақтасының аннотациясы.- Жарамсыз тұз қатесі және кейінгі сәтті қамтитын журналдар.- `iroha connect queue inspect` шығысы `state=Stalled`, содан кейін жаңа `state=Active` көрсетеді. || C5 | Аттестация / StrongBox қатесі | Android әмияндарында `ConnectApproval` параметрін `attachments[]` + StrongBox аттестациясын қосу үшін теңшеңіз. dApp қолданбасына бермес бұрын аттестаттау құралын (`scripts/android_keystore_attestation.sh` `--inject-failure strongbox-simulated` бар) пайдаланыңыз немесе JSON аттестациясын бұрмалаңыз. | DApp `ConnectError.Authorization.invalidAttestation` көмегімен бекітуді қабылдамайды, Torii сәтсіздік себебін журналға жазады, экспорттаушылар `connect.attestation_failed_total` соққысын жасайды және кезек бұзылған жазбаны тазартады. Swift/JS dApps сеансты тірі қалдыру кезінде қатені тіркейді. | - Инъекцияланған қате идентификаторы бар арна журналы.- SDK қате журналы + телеметрия есептегішін түсіру.- Кезектің нашар кадрды алып тастағанының дәлелі (`recordsRemoved > 0`). |

## Сценарий мәліметтері

### C1 — WebSocket үзілуі және қайта қосылу
1. Torii проксиінің (токсипрокси, өкіл немесе `kubectl port-forward`) артына ораңыз.
   сіз бүкіл түйінді өлтірмей қолжетімділікті ауыстыра аласыз.
2. 45 секундтық өшіруді іске қосыңыз:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. Телеметрияның бақылау тақталарын және `scripts/swift_status_export.py телеметриясын бақылаңыз.
   --json-out artefacts/connect-chaos//c1_metrics.json`.
4. Үзілістен кейін бірден қоқыс кезегі күйі:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. Сәттілік = бір рет қайта қосу әрекеті, шектелген кезек өсуі және автоматты
   прокси қалпына келтірілгеннен кейін ағызыңыз.

### C2 — Офлайн кезектегі толып кету / TTL мерзімі
1. Жергілікті құрылыстардағы кезек шегін қысқарту:
   - Swift: үлгідегі `ConnectQueueJournal` инициализаторын жаңартыңыз
     (мысалы, `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` өту.
   - Android/JS: құрастыру кезінде баламалы конфигурация нысанын өткізіңіз
     `ConnectQueueJournal`.
2. Әмиянды (симулятордың фоны немесе құрылғының ұшақ режимі) ≥60 секундқа уақытша тоқтатыңыз.
   dApp `ConnectClient.requestSignature(...)` қоңырауларын шығарған кезде.
3. `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) немесе JS пайдаланыңыз
   дәлелдер бумасын экспорттауға арналған диагностикалық көмекші (`state.json`, `journal/*.to`,
   `metrics.ndjson`).
4. Сәттілік = есептегіштердің толып кетуі, SDK беттері `ConnectError.QueueOverflow`
   бір рет, ал әмиян қалпына келгеннен кейін кезек қалпына келеді.

### C3 — Манифесттік дрейф / қабылдаудан бас тарту
1. Қабылдау манифестінің көшірмесін жасаңыз, мысалы:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. Torii іске қосыңыз `--connect-manifest-path /tmp/manifest_drift.json` (немесе
   бұрғылау үшін докер құрастыру/k8s конфигурациясын жаңартыңыз).
3. Сеансты әмияннан бастау әрекеті; HTTP 409 күтіңіз.
4. Torii + SDK журналдарын және `connect.manifest_mismatch_total` файлын мына жерден түсіріңіз.
   телеметрия бақылау тақтасы.
5. Сәттілік = кезек өсусіз бас тарту, сонымен қатар әмиян ортақ көрсетеді
   таксономия қатесі (`ConnectError.Authorization.manifestMismatch`).### C4 — Перненің айналуы / тұзды бұдыр
1. Телеметриядан ағымдағы тұз нұсқасын жазыңыз:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Torii құрылғысын жаңа тұзбен қайта іске қосыңыз (`CONNECT_SALT_VERSION=$((OLD+1))` немесе
   конфигурация картасы). Қайта іске қосу аяқталғанша әмиянды желіден тыс ұстаңыз.
3. Әмиянды жалғастыру; бірінші түйіндеме жарамсыз-тұз қателігімен сәтсіздікке ұшырауы керек
   және `connect.queue_dropped_total{reason="salt_version_mismatch"}` қадамдары.
4. Сеанс каталогын жою арқылы қолданбаны кэштелген кадрларды тастауға мәжбүр етіңіз
   (`rm -rf ~/.iroha/connect/<sid>` немесе платформаға арналған кэш тазалау), содан кейін
   сеансты жаңа белгілермен қайта бастаңыз.
5. Сәттілік = телеметрия тұзды соққыны көрсетеді, жарамсыз түйіндеме оқиғасы тіркеледі
   бір рет, ал келесі сессия қолмен араласусыз сәтті аяқталады.

### C5 — Аттестация / StrongBox қатесі
1. `scripts/android_keystore_attestation.sh` арқылы аттестаттау бумасын жасаңыз
   (қолтаңба битін аудару үшін `--inject-failure strongbox-simulated` орнатыңыз).
2. Әмиянға осы топтаманы `ConnectApproval` API арқылы тіркеңіз; dApp
   пайдалы жүктемені растау және қабылдамау керек.
3. Телеметрияны тексеріңіз (`connect.attestation_failed_total`, Swift/Android оқиғасы
   метрика) және кезек уланған жазбаны тастағанына көз жеткізіңіз.
4. Сәттілік = қабылдамау жаман мақұлдаумен оқшауланады, кезектер сау болады,
   және аттестаттау журналы бұрғылау дәлелдерімен бірге сақталады.

## Дәлелдерді тексеру тізімі
- `artifacts/connect-chaos/<date>/c*_metrics.json` келесіден экспорттайды
  `scripts/swift_status_export.py telemetry`.
- CLI шығыстары (`c*_queue.txt`) `iroha connect queue inspect`.
- Уақыт белгілері мен SID хэштері бар SDK + Torii журналдары.
- Әрбір сценарий үшін аннотациялары бар бақылау тақтасының скриншоттары.
- Sev1/2 ескертулері іске қосылған болса, PagerDuty / оқиға идентификаторлары.

Толық матрицаны тоқсанына бір рет толтыру жол картасының қақпасын қанағаттандырады және
Swift/Android/JS Connect іске асырулары айқын жауап беретінін көрсетеді
ең жоғары қателік режимдері бойынша.