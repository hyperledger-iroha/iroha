---
lang: kk
direction: ltr
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T14:35:37.691616+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Деректердің қолжетімділігін репликациялау саясаты (DA-4)

_Күйі: Орындалуда — Иелері: ЖТ негізгі протоколы / Сақтау тобы / SRE_

DA қабылдау құбыры енді детерминирленген сақтау мақсаттарын жүзеге асырады
`roadmap.md` (DA-4 жұмыс ағыны) ішінде сипатталған әрбір blob класы. Torii бас тартады
конфигурацияланғанға сәйкес келмейтін қоңырау шалушы қамтамасыз ететін сақтау конверттерін сақтау
әрбір валидатор/сақтау түйіні талап етілетінін сақтайтынына кепілдік беретін саясат
жіберуші ниетіне сүйенбестен дәуірлер мен көшірмелер саны.

## Әдепкі саясат

| Blob класы | Ыстық ұстау | Суық ұстау | Міндетті көшірмелер | Сақтау класы | Басқару тегі |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 сағат | 14 күн | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 сағат | 7 күн | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 сағат | 180 күн | 3 | `cold` | `da.governance` |
| _Әдепкі (барлық басқа сыныптар)_ | 6 сағат | 30 күн | 3 | `warm` | `da.default` |

Бұл мәндер `torii.da_ingest.replication_policy` ішіне ендірілген және қолданылады
барлық `/v1/da/ingest` жіберілімдері. Torii манифесттерді мәжбүрлі түрде қайта жазады
сақтау профилін көрсетеді және қоңырау шалушылар сәйкес келмейтін мәндерді берген кезде ескерту шығарады
операторлар ескірген SDK файлдарын анықтай алады.

### Taikai қолжетімділік сабақтары

Taikai маршруттау манифесттері (`taikai.trm` метадеректері) енді мынаны қамтиды
`availability_class` кеңес (`Hot`, `Warm` немесе `Cold`). Бар болғанда, Torii
сәйкес сақтау профилін `torii.da_ingest.replication_policy` ішінен таңдайды
пайдалы жүктемені бөлшектемес бұрын, оқиға операторларына белсенді емес деңгейді төмендетуге мүмкіндік береді
жаһандық саясат кестесін өңдеусіз орындау. Әдепкі параметрлер:

| Қол жетімділік класы | Ыстық ұстау | Суық ұстау | Міндетті көшірмелер | Сақтау класы | Басқару тегі |
|----------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 сағат | 14 күн | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 сағат | 30 күн | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 сағат | 180 күн | 3 | `cold` | `da.taikai.archive` |

Егер манифест `availability_class` жіберіп алса, қабылдау жолы қайтадан келесіге түседі.
`hot` профилі тікелей трансляциялар толық көшірме жиынын сақтайды. Операторлар жасай алады
жаңаны өңдеу арқылы осы мәндерді қайта анықтаңыз
`torii.da_ingest.replication_policy.taikai_availability` блогы конфигурацияда.

## Конфигурация

Саясат `torii.da_ingest.replication_policy` астында әрекет етеді және aны көрсетеді
*әдепкі* үлгі және әр сынып үшін қайта анықтаулар массиві. Класс идентификаторлары болып табылады
регистрді сезбейді және `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact` немесе басқару бекіткен кеңейтімдер үшін `custom:<u16>`.
Сақтау сыныптары `hot`, `warm` немесе `cold` қабылдайды.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Жоғарыда аталған әдепкі параметрлермен іске қосу үшін блокты түртпей қалдырыңыз. Тарту үшін а
сынып, сәйкес келетін қайта анықтауды жаңарту; жаңа сыныптар үшін базалық сызықты өзгерту,
өңдеу `default_retention`.Арнайы Taikai қолжетімділік сыныптарын реттеу үшін астына жазбаларды қосыңыз
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## Орындау семантикасы

- Torii пайдаланушы берген `RetentionPolicy` параметрін мәжбүрлі профильмен ауыстырады
  бөліну немесе айқын эмиссия алдында.
- Сәйкес келмейтін сақтау профилін жариялайтын алдын ала құрастырылған манифесттер қабылданбайды
  `400 schema mismatch` көмегімен ескірген клиенттер келісімшартты әлсірете алмайды.
- Әрбір қайта анықтау оқиғасы тіркеледі (`blob_class`, жіберілген және күтілетін саясат)
  шығару кезінде сәйкес келмейтін қоңырау шалушыларды анықтау.

Жаңартылған қақпаны `docs/source/da/ingest_plan.md` (тексеруді тексеру тізімі) қараңыз
сақтауды қамтамасыз етуді қамтиды.

## Қайта репликациялау жұмыс процесі (DA-4 бақылауы)

Сақтауды орындау тек бірінші қадам болып табылады. Мұны операторлар да дәлелдеу керек
тікелей манифесттер мен репликация тапсырыстары конфигурацияланған саясатқа сәйкес келеді
SoraFS сәйкес келмейтін блоктарды автоматты түрде қайталай алады.

1. **Дрейфті бақылаңыз.** Torii шығарады
   `overriding DA retention policy to match configured network baseline` кез келген уақытта
   қоңырау шалушы ескірген сақтау мәндерін жібереді. Бұл журналды жұптаңыз
   `torii_sorafs_replication_*` телеметриясы репликаның кемшіліктерін немесе кешіктірілгенін анықтау
   қайта орналастырулар.
2. **Тікелей көшірмелерге қарсы ниет айырмашылығы.** Жаңа аудит көмекшісін пайдаланыңыз:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Пәрмен берілгеннен `torii.da_ingest.replication_policy` жүктейді
   конфигурациялайды, әрбір манифестті (JSON немесе Norito) декодтайды және қосымша кез келгеніне сәйкес келеді
   `ReplicationOrderV1` манифест дайджесті бойынша пайдалы жүктемелер. Қорытынды екі жалаушамен белгіленеді
   шарттар:

   - `policy_mismatch` – манифестті сақтау профилі мәжбүрлі профильден алшақтайды
     саясат (Torii дұрыс конфигурацияланбаса, бұл ешқашан болмауы керек).
   - `replica_shortfall` – тірі репликация тәртібі келесіге қарағанда аз көшірмелерді сұрайды
     `RetentionPolicy.required_replicas` немесе оған қарағанда азырақ тапсырмалар береді
     мақсат.

   Нөлдік емес шығу күйі белсенді жетіспеушілікті көрсетеді, сондықтан CI/шақыру бойынша автоматтандыру
   бірден бетке алады. JSON есебін тіркеңіз
   Парламент дауысына арналған `docs/examples/da_manifest_review_template.md` пакеті.
3. **Қайта репликацияны іске қосу.** Аудит жетіспеушілік туралы есеп бергенде, жаңадан шығарыңыз.
   `ReplicationOrderV1` бөлімінде сипатталған басқару құралы арқылы
   `docs/source/sorafs/storage_capacity_marketplace.md` және аудитті қайта іске қосыңыз
   көшірме жинағы жинақталғанша. Төтенше жағдайларды қайта анықтау үшін CLI шығысын жұптаңыз
   `iroha app da prove-availability` көмегімен SRE бір дайджестке сілтеме жасай алады
   және PDP дәлелдері.

Регрессиялық қамту `integration_tests/tests/da/replication_policy.rs` ішінде өмір сүреді;
люкс сәйкес емес сақтау саясатын `/v1/da/ingest` жібереді және тексереді
алынған манифест қоңырау шалушының орнына мәжбүрленген профильді көрсетеді
ниет.

## Денсаулықты растайтын телеметрия және бақылау тақталары (DA-5 көпірі)

Жол картасының **DA-5** тармағы PDP/PoTR орындау нәтижелерінің аудитте болуын талап етеді.
нақты уақыт. `SorafsProofHealthAlert` оқиғалары енді арнайы жиынды басқарады
Prometheus көрсеткіштері:

- `torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
- `torii_sorafs_proof_health_pdp_failures{provider_id}`
- `torii_sorafs_proof_health_potr_breaches{provider_id}`
- `torii_sorafs_proof_health_penalty_nano{provider_id}`
- `torii_sorafs_proof_health_cooldown{provider_id}`
- `torii_sorafs_proof_health_window_end_epoch{provider_id}`

**SoraFS PDP және PoTR Health** Grafana тақтасы
(`dashboards/grafana/sorafs_pdp_potr_health.json`) енді сол сигналдарды көрсетеді:- *Триггер арқылы расталған денсаулық туралы ескертулер* триггер/айыппұл жалаушасы бойынша ескерту жылдамдығын көрсетеді.
  Taikai/CDN операторлары тек PDP, тек PoTR немесе қосарлы ескертулер екенін дәлелдей алады.
  ату.
- *Colddown режиміндегі провайдерлер* қазіргі уақытта а бойынша провайдерлердің нақты сомасын хабарлайды
  SorafsProofHealthAlert салқындату.
- *Proof Health Window Snapshot* PDP/PoTR есептегіштерін, айыппұл сомасын біріктіреді,
  салқындату жалаушасы және әр провайдер үшін ескерту терезесінің аяқталу дәуірі, сондықтан басқаруды тексерушілер
  кестені оқиға пакеттеріне тіркей алады.

Runbooks DA орындау дәлелдерін ұсынған кезде осы панельдерді байланыстыруы керек; олар
CLI proof-stream қателерін тікелей тізбектегі айыппұл метадеректеріне байланыстыру және
жол картасында көрсетілген бақылау мүмкіндігін қамтамасыз етіңіз.