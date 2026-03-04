---
id: incident-runbooks
lang: kk
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Incident Runbooks & Rollback Drills
sidebar_label: Incident Runbooks
description: Response guides for failed portal deployments, SoraFS replication degradation, analytics outages, and the quarterly rehearsal cadence required by DOCS-9.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Мақсат

Жол картасының **DOCS-9** тармағында әрекет етуге болатын кітаптар мен репетиция жоспары қажет.
Портал операторлары жөнелту сәтсіздіктерін болжаусыз қалпына келтіре алады. Бұл жазба
сигналы жоғары үш оқиғаны қамтиды — сәтсіз орналастыру, репликация
деградация және аналитикалық үзілістер — тоқсан сайынғы жаттығуларды құжаттайды
бүркеншік аттың кері қайтарылуын дәлелдеу және синтетикалық тексеру әлі де соңына дейін жұмыс істейді.

### Қатысты материал

- [`devportal/deploy-guide`](./deploy-guide) — орау, қол қою және бүркеншік ат
  жылжыту жұмыс процесі.
- [`devportal/observability`](./observability) — шығарылым тегтері, аналитика және
  төменде сілтеме жасалған зондтар.
- `docs/source/sorafs_node_client_protocol.md`
  және [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — регистрлік телеметрия және эскалация шектері.
- `docs/portal/scripts/sorafs-pin-release.sh` және `npm run probe:*` көмекшілері
  барлық тексеру парақтарында сілтеме жасалған.

### Ортақ телеметрия және құралдар

| Сигнал / Құрал | Мақсаты |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (кездескен/жіберілген/күтуде) | Шағылыстыру тоқтауларын және SLA бұзылуларын анықтайды. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Триаж үшін кешіктіру тереңдігі мен аяқталу кідірісін анықтайды. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Нашар орналастырудан кейін жиі болатын шлюз жағындағы сәтсіздіктерді көрсетеді. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Шкафтарды шығаратын және кері қайтаруды растайтын синтетикалық зондтар. |
| `npm run check:links` | Сынған байланыс қақпасы; әрбір жұмсартудан кейін қолданылады. |
| `sorafs_cli manifest submit … --alias-*` (`scripts/sorafs-pin-release.sh` арқылы оралған) | Бүркеншік аттарды жылжыту/қайтару механизмі. |
| `Docs Portal Publishing` Grafana тақтасы (`dashboards/grafana/docs_portal.json`) | Бас тарту/бүркеншік ат/TLS/репликация телеметриясын біріктіреді. PagerDuty ескертулері дәлел үшін осы панельдерге сілтеме жасайды. |

## Runbook — Сәтсіз орналастыру немесе нашар артефакт

### Триггер шарттары

- Алдын ала қарау/өндіріс зондтары сәтсіз аяқталды (`npm run probe:portal -- --expect-release=…`).
- Grafana ескертулері `torii_sorafs_gateway_refusals_total` немесе
  `torii_sorafs_manifest_submit_total{status="error"}` шығарылымнан кейін.
- Қолмен QA бұзылған маршруттарды немесе Try-It прокси қателерін бірден кейін хабарлайды
  бүркеншік атпен жылжыту.

### Дереу оқшаулау

1. **Орналастыруларды тоқтату:** CI құбырын `DEPLOY_FREEZE=1` (GitHub) арқылы белгілеңіз
   жұмыс үрдісін енгізу) немесе Дженкинс тапсырмасын кідіртіңіз, сондықтан ешқандай қосымша артефактілер сөнбейді.
2. **Артефактілерді түсіру:** сәтсіз құрастырудың `build/checksums.sha256` нұсқасын жүктеп алыңыз,
   `portal.manifest*.{json,to,bundle,sig}` және кері қайтару мүмкін болатындай зонд шығысы
   нақты дайджесттерге сілтеме жасайды.
3. **Мүдделі тараптарды хабардар ету:** SRE сақтау орны, Docs/DevRel жетекші және басқару
   хабардар болу үшін кезекші (әсіресе `docs.sora` әсер еткенде).

### Кері қайтару процедурасы

1. Соңғы белгілі-жақсы (LKG) манифестін анықтаңыз. Өндірістік жұмыс процесі сақталады
   оларды `artifacts/devportal/<release>/sorafs/portal.manifest.to` астында.
2. Жеткізу көмекшісімен манифестке бүркеншік атты қайта байланыстырыңыз:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Қайтару қорытындысын оқиға билетіне LKG және бірге жазыңыз
   сәтсіз манифест дайджесттері.

### Валидация

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature …` және `sorafs_cli proof verify …`
   (орналастыру нұсқаулығын қараңыз) қайта көтерілген манифесттің сәйкестігін растау үшін
   мұрағатталған CAR.
4. `npm run probe:tryit-proxy` Try-It кезеңдік проксиінің қайтып келгеніне көз жеткізу үшін.

### Оқиғадан кейінгі

1. Түпкі себеп түсінілгеннен кейін ғана орналастыру құбырын қайта қосыңыз.
2. Толтыру [`devportal/deploy-guide`](./deploy-guide) "Сабақтар"
   жаңа таңбалары бар жазбалар, егер бар болса.
3. Сәтсіз сынақ жинағы үшін файл ақаулары (зонд, сілтеме тексерушісі және т.б.).

## Runbook — Репликацияның нашарлауы

### Триггер шарттары

- Ескерту: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(қосынды(torii_sorafs_replication_sla_total{нәтиже=~"кездескен|өтінбеген"}), 1) <
  10 минут үшін 0,95`.
- `torii_sorafs_replication_backlog_total > 10` 10 минутқа (қараңыз
  `pin-registry-ops.md`).
- Басқару шығарылымнан кейін бүркеншік аттың қол жетімділігінің баяу екенін хабарлайды.

### Триаж

1. Растау үшін [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) бақылау тақталарын тексеріңіз
   артта қалудың сақтау класына немесе провайдер паркіне локализацияланғанын.
2. Torii журналдарын `sorafs_registry::submit_manifest` ескертулеріне қарсы тексеріңіз.
   жіберулердің өздері сәтсіз екенін анықтау.
3. `sorafs_cli manifest status --manifest …` (тізімдер
   әр провайдерге қайталау нәтижелері).

### Жеңілдету

1. Көшіру саны жоғары (`--pin-min-replicas 7`) манифестті қайта шығарыңыз.
   `scripts/sorafs-pin-release.sh`, сондықтан жоспарлаушы жүктемені үлкенірек таратады
   провайдер жинағы. Жаңа манифест дайджестін оқиға журналына жазыңыз.
2. Егер кешіктіру бір провайдерге байланысты болса, оны мына арқылы уақытша өшіріңіз
   репликацияны жоспарлаушы (`pin-registry-ops.md` құжатталған) және жаңасын жіберіңіз
   манифест басқа провайдерлерді бүркеншік атты жаңартуға мәжбүр етеді.
3. Бүркеншік аттың жаңалығы репликация паритетінен маңыздырақ болғанда, қайта байланыстырыңыз
   сахналанған жылы манифестке бүркеншік атын (`docs-preview`), содан кейін жариялаңыз
   SRE артта қалуды жойғаннан кейін кейінгі манифест.

### Қалпына келтіру және жабу

1. `torii_sorafs_replication_sla_total{outcome="missed"}` мониторын қамтамасыз ету үшін
   үстірттерді санау.
2. `sorafs_cli manifest status` шығысын әрбір көшірменің бар екендігінің дәлелі ретінде түсіріңіз
   сәйкестікке қайтады.
3. Келесі қадамдармен өлгеннен кейінгі репликацияның артта қалу тізімін файлдаңыз немесе жаңартыңыз
   (провайдердің масштабтауы, chunker баптауы және т.б.).

## Runbook — Аналитика немесе телеметрия үзілуі

### Триггер шарттары

- `npm run probe:portal` сәтті болды, бірақ бақылау тақталары қабылдауды тоқтатады
  >15 минут ішінде `AnalyticsTracker` оқиғалары.
- Құпиялықты шолу тоқтатылған оқиғалардың күтпеген ұлғаюын белгілейді.
- `npm run probe:tryit-proxy` `/probe/analytics` жолдарында сәтсіз аяқталды.

### Жауап

1. Құрастыру уақыты кірістерін тексеріңіз: `DOCS_ANALYTICS_ENDPOINT` және
   Сәтсіз шығарылым артефактіндегі `DOCS_ANALYTICS_SAMPLE_RATE` (`build/release.json`).
2. `DOCS_ANALYTICS_ENDPOINT` бағытын көрсетіп `npm run probe:portal` қайта іске қосыңыз.
   трекер әлі де пайдалы жүктемелерді шығаратынын растау үшін кезеңдік коллектор.
3. Коллекторлар жұмыс істемей тұрса, `DOCS_ANALYTICS_ENDPOINT=""` орнатып, қайта құрыңыз.
   трекердің қысқа тұйықталуы; оқиғаның хронологиясына тоқтау терезесін жазыңыз.
4. `scripts/check-links.mjs` қозғалыссыз саусақ іздерін растаңыз `checksums.sha256`
   (талдау үзілістері сайт картасын тексеруді *бұғатпауы керек).
5. Коллектор қалпына келгеннен кейін, орындау үшін `npm run test:widgets` іске қосыңыз
   аналитикалық көмекші бірлігі қайта жариялау алдында сынақтар.

### Оқиғадан кейінгі

1. [`devportal/observability`](./observability) кез келген жаңа коллектормен жаңартыңыз
   шектеулер немесе іріктеу талаптары.
2. Кез келген аналитикалық деректер тасталса немесе сыртқа өңделсе, файлды басқару туралы хабарлама
   саясат.

## Тоқсан сайынғы төзімділік жаттығулары

Екі жаттығуды да **әр тоқсанның бірінші сейсенбісінде** орындаңыз (қаңтар/сәуір/шілде/қазан)
немесе кез келген ірі инфрақұрылымдық өзгерістерден кейін бірден. астында артефактілерді сақтаңыз
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Бұрғылау | Қадамдар | Дәлелдер |
| ----- | ----- | -------- |
| Бүркеншік аттың кері репетициясы | 1. Ең соңғы өндіріс манифестін пайдаланып, "Сәтсіз орналастыру" кері қайтаруды қайталаңыз.<br/>2. Зондтар өткеннен кейін өндіріске қайта байланыстырыңыз.<br/>3. `portal.manifest.submit.summary.json` жазыңыз және бұрғылау қалтасына тексеру журналдары. | `rollback.submit.json`, зонд шығысы және жаттығудың босату тегін. |
| Синтетикалық валидация аудиті | 1. `npm run probe:portal` және `npm run probe:tryit-proxy` параметрлерін өндіру мен кезеңге қарсы іске қосыңыз.<br/>2. `npm run check:links` іске қосыңыз және `build/link-report.json` мұрағатыңыз.<br/>3. Зерттеудің сәттілігін растайтын Grafana панельдерінің скриншоттарын/экспорттарын тіркеңіз. | Манифест саусақ ізіне сілтеме жасайтын зонд журналдары + `link-report.json`. |

Өткізіп алған жаттығуларды Docs/DevRel менеджеріне және SRE басқару шолуына жеткізіңіз,
өйткені жол картасы екі бүркеншік аттың детерминирленген, тоқсан сайынғы дәлелдерін талап етеді
кері қайтару және портал зондтары сау болып қалады.

## ПейжерКезекшілік және шақыру бойынша үйлестіру

- PagerDuty қызметі **Docs Portal Publishing** мына жерден жасалған ескертулерге иелік етеді.
  `dashboards/grafana/docs_portal.json`. `DocsPortal/GatewayRefusals` ережелері,
  `DocsPortal/AliasCache` және `DocsPortal/TLSExpiry` Docs/DevRel бетінде
  қосымша ретінде SRE сақтау орны бар негізгі.
- Беттелгенде `DOCS_RELEASE_TAG` қосыңыз, зардап шеккендердің скриншоттарын қосыңыз
  Grafana панельдері және оқиға ескертпелеріндегі сілтеме зонд/байланысты тексеру шығысы
  жұмсарту басталады.
- Жеңілдетілгеннен кейін (кері қайтару немесе қайта орналастыру), `npm run probe:portal` қайта іске қосыңыз,
  `npm run check:links` және көрсеткіштерді көрсететін жаңа Grafana суретін түсіріңіз
  табалдырықтарға қайтады. Барлық дәлелдерді PagerDuty оқиғасына дейін тіркеңіз
  оны шешу.
- Егер екі ескерту бір уақытта қосылса (мысалы, TLS мерзімінің аяқталуы плюс кешігу), триаж
  алдымен бас тартады (жариялауды тоқтату), кері қайтару процедурасын орындаңыз, содан кейін өшіріңіз
  Көпірдегі SRE сақтау орны бар TLS/артта қалған элементтер.