---
lang: kk
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ead8c13470d6e8766d8f161cdd9443ef29c72d3f87bd7aac27f179c3e1c98fb
source_last_modified: "2026-01-22T16:26:46.498151+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-elastic-lane
title: Elastic lane provisioning (NX-7)
sidebar_label: Elastic Lane Provisioning
description: Bootstrap workflow for creating Nexus lane manifests, catalog entries, and rollout evidence.
translator: machine-google-reviewed
---

:::ескерту Канондық дереккөз
Бұл бет `docs/source/nexus_elastic_lane.md` көрсетеді. Аударма сыпырып порталға түскенше екі көшірмені де туралап ұстаңыз.
:::

# Эластикалық жолақты қамтамасыз ету құралдары жинағы (NX-7)

> **Жол картасының тармағы:** NX-7 — Серпімді жолақты қамтамасыз ету құралдары  
> **Күй:** Құралдар аяқталды — манифесттерді, каталог үзінділерін, Norito пайдалы жүктемелерін, түтін сынақтарын жасайды,
> және жүктеу сынағы бумасының көмекшісі енді ұяшықтардың кешігуін тігеді + дәлелдемелер осылайша валидаторды көрсетеді
> жүктеуді орындау сценарийінсіз жариялануы мүмкін.

Бұл нұсқаулық операторларды автоматтандыратын жаңа `scripts/nexus_lane_bootstrap.sh` көмекшісі арқылы көрсетеді.
жолақ манифестінің генерациясы, жолақ/деректер кеңістігі каталогының үзінділері және шығару дәлелдері. Мақсат – жасау
бірнеше файлдарды қолмен өңдеусіз немесе жаңа Nexus жолақтарын (қоғамдық немесе жеке) айналдыру оңай
каталог геометриясын қолмен қайта шығару.

## 1. Пререквизиттер

1. Жолақ бүркеншік аты, деректер кеңістігі, валидатор жинағы, қатеге төзімділік (`f`) және есеп айырысу саясаты үшін басқаруды бекіту.
2. Аяқталған валидатор тізімі (есептік жазба идентификаторлары) және қорғалған аттар кеңістігі тізімі.
3. Жасалған үзінділерді қосу үшін түйін конфигурациясының репозиторийіне қатынасыңыз.
4. Жолақ манифест тізіліміне арналған жолдар (`nexus.registry.manifest_directory` және қараңыз.
   `cache_directory`).
5. Жолаққа арналған телеметриялық контактілер/PagerDuty тұтқалары
   желіге келеді.

## 2. Жолақ артефактілерін жасаңыз

Көмекшіні репозиторий түбірінен іске қосыңыз:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Негізгі жалаушалар:

- `--lane-id` жаңа жазбаның `nexus.lane_catalog` индексіне сәйкес келуі керек.
- `--dataspace-alias` және `--dataspace-id/hash` деректер кеңістігінің каталог жазбасын басқарады (әдепкі бойынша
  жіберілген кезде жолақ идентификаторы).
- `--validator` қайталануы немесе `--validators-file` сайтынан алынуы мүмкін.
- `--route-instruction` / `--route-account` қоюға дайын маршруттау ережелерін шығарады.
- `--metadata key=value` (немесе `--telemetry-contact/channel/runbook`) runbook контактілерін түсіру үшін
  бақылау тақталары бірден құқық иелерін тізімдейді.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` манифестке орындау уақытын жаңарту ілгегін қосыңыз
  жолақ оператордың кеңейтілген басқару элементтерін қажет еткенде.
- `--encode-space-directory` `cargo xtask space-directory encode` автоматты түрде шақырады. Онымен жұптаңыз
  `--space-directory-out` кодталған `.to` файлын әдепкіден басқа жерде қаласаңыз.

Сценарий `--output-dir` ішінде үш артефакт жасайды (ағымдық каталогтың әдепкілері),
плюс кодтау қосылғанда қосымша төртінші:

1. `<slug>.manifest.json` — валидатор кворумы, қорғалған аттар кеңістігі және бар жолақ манифесті
   қосымша орындау уақытын жаңарту ілмек метадеректері.
2. `<slug>.catalog.toml` — `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` бар TOML үзіндісі,
   және кез келген сұралған маршруттау ережелері. `fault_tolerance` деректер кеңістігі жазбасында өлшемге орнатылғанына көз жеткізіңіз
   жолақты-релейлік комитет (`3f+1`).
3. `<slug>.summary.json` — геометрияны (слаг, сегменттер, метадеректер) сипаттайтын аудиторлық қорытынды
   қажетті шығару қадамдары және дәл `cargo xtask space-directory encode` пәрмені (
   `space_directory_encode.command`). Дәлел үшін осы JSON-ды борттық билетке тіркеңіз.
4. `<slug>.manifest.to` — `--encode-space-directory` орнатылған кезде шығарылады; Torii үшін дайын
   `iroha app space-directory manifest publish` ағыны.

JSON/ үзінділерін файлдарды жазбай алдын ала қарау үшін `--dry-run`, ал қайта жазу үшін `--force` пайдаланыңыз.
бар артефактілер.

## 3. Өзгерістерді қолданыңыз

1. Манифест JSON файлын конфигурацияланған `nexus.registry.manifest_directory` (және кэшке) көшіріңіз
   каталог, егер тізілім қашықтағы бумаларды көрсететін болса). Манифесттер нұсқада болса, файлды орындаңыз
   сіздің конфигурация репо.
2. Каталог үзіндісін `config/config.toml` (немесе сәйкес `config.d/*.toml`) қосыңыз. Қамтамасыз ету
   `nexus.lane_count` - кем дегенде `lane_id + 1` және кез келген `nexus.routing_policy.rules` жаңартыңыз
   жаңа жолды көрсету керек.
3. Кодтаңыз (егер `--encode-space-directory` өткізіп жіберсеңіз) және манифестті кеңістік каталогына жариялаңыз
   қорытындыда түсірілген пәрменді пайдалану (`space_directory_encode.command`). Бұл шығарады
   `.manifest.to` пайдалы жүктеме Torii аудиторлар үшін дәлелдемелерді күтеді және жазады; арқылы жіберу
   `iroha app space-directory manifest publish`.
4. `irohad --sora --config path/to/config.toml --trace-config` іске қосыңыз және бақылау шығысын мұрағаттаңыз
   шығару билеті. Бұл жаңа геометрияның жасалған слаг/кура сегменттеріне сәйкес келетінін дәлелдейді.
5. Манифест/каталог өзгерістері енгізілгеннен кейін жолаққа тағайындалған валидаторларды қайта іске қосыңыз. Сақтау
   болашақ аудиттерге арналған билеттегі жиынтық JSON.

## 4. Тізілім тарату бумасын құрастырыңыз

Операторлар жолақты басқару деректерін онсыз тарата алатындай етіп жасалған манифест пен қабаттасуды бумалаңыз
әрбір хосттағы конфигурацияларды өңдеу. Топтаманың көмекшісі манифесттерді канондық орналасуға көшіреді,
`nexus.registry.cache_directory` үшін қосымша басқару каталогын жасайды және
офлайн аударымдарға арналған tarball:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Шығарулар:

1. `manifests/<slug>.manifest.json` — оларды конфигурацияланғанға көшіріңіз
   `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` — `nexus.registry.cache_directory` ішіне түсіру. Әрбір `--module`
   жазба басқару модулін ауыстыруға (NX-2) мүмкіндік беретін қосылатын модуль анықтамасына айналады.
   `config.toml` өңдеудің орнына кэш қабатын жаңарту.
3. `summary.json` — хэштерді, қабаттасу метадеректерін және оператор нұсқауларын қамтиды.
4. Қосымша `registry_bundle.tar.*` — SCP, S3 немесе артефакт трекерлеріне дайын.

Бүкіл каталогты (немесе мұрағатты) әрбір валидаторға синхрондаңыз, ауа саңылаулары бар хосттарға шығарып алыңыз және көшіріңіз
Torii қайта іске қоспас бұрын манифесттер + кэш қабаты олардың тізілім жолдарына.

## 5. Валидатор түтінінің сынақтары

Torii қайта іске қосылғаннан кейін `manifest_ready=true` жолақ есептерін тексеру үшін жаңа түтін көмекшісін іске қосыңыз,
метрикалар күтілетін жолақ санын көрсетеді және мөрленген өлшегіш анық. Манифесттерді қажет ететін жолақтар
бос емес `manifest_path` көрсетуі керек; Көмекші енді жол жоқ болған кезде бірден істен шығады
әрбір NX-7 орналастыру жазбасы қол қойылған манифест дәлелдерін қамтиды:

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

Өздігінен қол қойылған орталарды сынау кезінде `--insecure` қосыңыз. Жолақ болса, сценарий нөлден тыс шығады
жоқ, мөрленген немесе күтілетін мәндерден метрика/телеметрия ауытқуы. пайдаланыңыз
`--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog`, және
`--max-headroom-events` тұтқалары әр жолақты блоктың биіктігін/соңғылығын/артында қалдыруды/басты бөлменің телеметриясын сақтау үшін
операциялық конвертте және оларды `--max-slot-p95` / `--max-slot-p99` арқылы жұптаңыз.
(плюс `--min-slot-samples`) көмекшіден шықпай-ақ NX‑18 ұяшығының ұзақтығы мақсаттарын орындау үшін.

Ауа саңылаулары бар валидациялар үшін (немесе CI) сіз тікелей эфирді ұрудың орнына түсірілген Torii жауабын қайта ойната аласыз.
соңғы нүкте:

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

`fixtures/nexus/lanes/` астында жазылған қондырғылар жүктеу белдігімен жасалған артефактілерді көрсетеді.
көмекші, сондықтан жаңа манифесттерді арнайы сценарийсіз сызуға болады. CI арқылы бірдей ағынды жүзеге асырады
`ci/check_nexus_lane_smoke.sh` және `ci/check_nexus_lane_registry_bundle.sh`
(бүркеншік ат: `make check-nexus-lanes`) NX-7 түтін көмекшісі жарияланған құжатпен сәйкес келетінін дәлелдеу үшін
пайдалы жүктеме пішімі және бума дайджесттері/қабаттамасы қайталану мүмкіндігін қамтамасыз ету үшін.

Жолақ атауы өзгертілгенде, `nexus.lane.topology` телеметрия оқиғаларын түсіріңіз (мысалы,
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) және оларды кері жіберіңіз
түтін көмекшісі. `--telemetry-file/--from-telemetry` жалаушасы жаңа жолмен бөлінген журналды қабылдайды және
`--require-alias-migration old:new` `alias_migrated` оқиғасы атын өзгертуді жазғанын растайды:

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

`telemetry_alias_migrated.ndjson` құрылғысы CI тексере алатындай атын өзгерту үлгісін жинақтайды.
тірі түйінмен байланыссыз телеметрияны талдау жолын.

## Валидаторды жүктеу сынақтары (NX-7 дәлелі)

Жол картасы **NX-7** қайталанатын валидатор жүктемесін жіберу үшін әрбір жаңа жолақты талап етеді. Қолдану
`scripts/nexus_lane_load_test.py` түтін тексерулерін, саңылау ұзақтығы қақпаларын және ұяшық бумасын тігу үшін
басқару қайталай алатын бір артефакт жиынтығына айналады:

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

Көмекші бірдей DA кворумын, oracle, есеп айырысу буферін, TEU және пайдаланылатын ұяшық ұзақтығы қақпаларын қамтамасыз етеді
түтін көмекшісі арқылы және `smoke.log`, `slot_summary.json`, ұяшықтардың манифесті және
`load_test_manifest.json` таңдалған `--out-dir` ішіне жүктеп алуды тікелей қосуға болады.
тапсырыс сценарийінсіз сатылым билеттері.

№# 6. Телеметрия және басқаруды бақылау

- Жолақ бақылау тақталарын (`dashboards/grafana/nexus_lanes.json` және қатысты қабаттасулар) жаңартыңыз.
  жаңа жол идентификаторы және метадеректер. Жасалған метадеректер кілттері (`contact`, `channel`, `runbook`, т.б.) жасайды.
  жапсырмаларды алдын ала толтыру оңай.
- Қабылдауды қосу алдында жаңа жолақ үшін Wire PagerDuty/Alertmanager ережелерін қосыңыз. `summary.json`
  келесі қадамдар жиымы [Nexus операциялар](./nexus-operations) ішіндегі бақылау тізімін көрсетеді.
- Валидатор жинағы белсенді болған кезде манифест бумасын Space Directory ішінде тіркеңіз. Дәл солай пайдаланыңыз
  басқару жұмыс кітабына сәйкес қол қойылған көмекші арқылы жасалған JSON манифесті.
- Түтін сынақтары (FindNetworkStatus, Torii) үшін [Sora Nexus операторын қосу](./nexus-operator-onboarding) бақылаңыз
  қол жетімділік) және жоғарыда жасалған артефакт жиынтығымен дәлелдемелерді түсіріңіз.

## 7. Құрғақ іске қосу мысалы

Артефактілерді файлдарды жазбай алдын ала қарау үшін:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --dry-run
```

Пәрмен JSON қорытындысын және TOML үзіндісін stdout файлына басып шығарады, бұл уақыт ішінде жылдам итерацияға мүмкіндік береді.
жоспарлау.

---

Қосымша мәтінмәнді қараңыз:- [Nexus операциялар](./nexus-operations) — операциялық бақылау парағы және телеметрия талаптары.
- [Sora Nexus операторын қосу](./nexus-operator-onboarding) — мыналарға сілтеме жасайтын егжей-тегжейлі қосу ағыны
  жаңа көмекші.
- [Nexus жолақ үлгісі](./nexus-lane-model) — құрал пайдаланатын жолақ геометриясы, шламдар және сақтау орналасуы.