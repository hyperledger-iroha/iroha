---
id: privacy-metrics-pipeline
lang: kk
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet Privacy Metrics Pipeline (SNNet-8)
sidebar_label: Privacy Metrics Pipeline
description: Privacy-preserving telemetry collection for SoraNet relays and orchestrators.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ескерту Канондық дереккөз
:::

# SoraNet құпиялылық метрикасының құбыры

SNNet-8 реле жұмыс уақыты үшін құпиялылықты ескеретін телеметрия бетін ұсынады. The
реле енді қол алысу мен тізбек оқиғаларын минуттық өлшемдегі шелектерге және
жеке тізбектерді сақтай отырып, тек ірі Prometheus есептегіштерін экспорттайды
операторларға әрекет ету мүмкіндігін бере отырып, байланыстыру мүмкін емес.

## Агрегаторға шолу

- Орындау уақытын іске асыру `tools/soranet-relay/src/privacy.rs` ретінде өмір сүреді
  `PrivacyAggregator`.
- Шелектер қабырға сағатының минутына (`bucket_secs`, әдепкі 60 секунд) кілттенеді және
  шектелген сақинада сақталады (`max_completed_buckets`, әдепкі 120). Коллекционер
  акциялар өздерінің шектелген кешігуін сақтайды (`max_share_lag_buckets`, әдепкі 12)
  сондықтан ескі Prio терезелері ағып кетпей, басылған шелек ретінде жуылады
  жады немесе кептеліп қалған коллекторларды маскирлеу.
- `RelayConfig::privacy` тура `PrivacyConfig` форматына сәйкестендіріп, баптауды көрсетеді
  тұтқалар (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`,
  `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`,
  `expected_shares`). Өндірістің орындалу уақыты SNnet-8a кезінде әдепкі мәндерді сақтайды
  қауіпсіз жинақтау шектерін енгізеді.
- Орындау модульдері терілген көмекшілер арқылы оқиғаларды жазады:
  `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`,
  `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`,
  `record_verified_bytes`, және `record_gar_category`.

## Relay Admin Endpoint

Операторлар арқылы өңделмеген бақылаулар үшін реленің әкімші тыңдаушысы сауалнама жүргізе алады
`GET /privacy/events`. Соңғы нүкте жаңа жолмен бөлінген JSON қайтарады
(`application/x-ndjson`) шағылыстырылған `SoranetPrivacyEventV1` пайдалы жүктемелері бар
ішкі `PrivacyEventBuffer`. Буфер ең жаңа оқиғаларды сақтайды
`privacy.event_buffer_capacity` жазбаларына (әдепкі 4096) және су төгілген
оқыңыз, сондықтан қырғыштар бос орындарды болдырмас үшін жиі сұрау керек. Оқиғалар қамтиды
бірдей қол алысу, дроссель, тексерілген өткізу қабілеттілігі, белсенді тізбек және GAR сигналдары
бұл Prometheus есептегіштеріне қуат беріп, төменгі коллекторларға мұрағаттауға мүмкіндік береді
құпиялылық үшін қауіпсіз нан үгінділері немесе арнаның қауіпсіз біріктіру жұмыс процестері.

## Реле конфигурациясы

Операторлар арқылы реле конфигурация файлындағы құпиялылық телеметриялық каденцияларды реттейді
`privacy` бөлімі:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

Өрістің әдепкі мәндері SNNet-8 спецификациясына сәйкес келеді және жүктеу уақытында тексеріледі:

| Өріс | Сипаттама | Әдепкі |
|-------|-------------|---------|
| `bucket_secs` | Әрбір жинақтау терезесінің ені (секунд). | `60` |
| `min_handshakes` | Шелек есептегіштерді шығара алмас бұрын ең аз салымшылар саны. | `12` |
| `flush_delay_buckets` | Жууды бастамас бұрын күтуге болатын шелектерді аяқтады. | `1` |
| `force_flush_buckets` | Біз басылған шелек шығарғанға дейінгі ең үлкен жас. | `6` |
| `max_completed_buckets` | Сақталған шелек кеші (шексіз жадты болдырмайды). | `120` |
| `max_share_lag_buckets` | Өшіру алдында коллекторлық акцияларды сақтау терезесі. | `12` |
| `expected_shares` | Біріктірмес бұрын коллекторлық акциялар қажет. | `2` |
| `event_buffer_capacity` | Әкімші ағыны үшін NDJSON оқиғасының артта қалуы. | `4096` |

`force_flush_buckets` параметрі `flush_delay_buckets` мәнінен төмен,
шекті мәндер немесе сақтау қорғаушысын өшіру енді болдырмау үшін тексеруді орындамайды
әрбір релелік телеметрияны ағызатын орналастырулар.

`event_buffer_capacity` шегі `/admin/privacy/events`-ті де шектеп,
қырғыштар шексіз артта қалуы мүмкін емес.

## Prio коллекторының акциялары

SNNet-8a құпия ортақ Prio шелектерін шығаратын қос коллекторларды орналастырады. The
оркестр енді екеуі үшін де `/privacy/events` NDJSON ағынын талдайды
`SoranetPrivacyEventV1` жазбалары және `SoranetPrivacyPrioShareV1` акциялары,
оларды `SoranetSecureAggregator::ingest_prio_share` ішіне қайта жіберу. Шелектер шығарады
`PrivacyBucketConfig::expected_shares` жарналары келгенде, шағылыстырады
релелік мінез-құлық. Акциялар шелек туралау және гистограмма пішіні үшін тексеріледі
`SoranetPrivacyBucketMetricsV1` біріктіру алдында. Егер біріктірілген болса
қол алысу саны `min_contributors` төмен болса, шелек экспортталады
`suppressed`, релелік агрегатордың әрекетін көрсетеді. Басылған
Windows енді операторлар ажырата алатындай етіп `suppression_reason` белгісін шығарады.
`insufficient_contributors`, `collector_suppressed`,
`collector_window_elapsed` және `forced_flush_window_elapsed` сценарийлері
телеметриялық олқылықтарды диагностикалау. `collector_window_elapsed` себебі де өртенеді
Prio акциялары `max_share_lag_buckets`-тен өткенде, коллекторлар кептеледі
жадында ескірген аккумуляторларды қалдырмай көрінеді.

## Torii Қабылдаудың соңғы нүктелері

Torii енді екі телеметриялы HTTP соңғы нүктелерін көрсетеді, сондықтан реле мен коллекторлар
тапсырысты тасымалдауды ендірусіз бақылауларды жібере алады:

- `POST /v2/soranet/privacy/event` a қабылдайды
  `RecordSoranetPrivacyEventDto` пайдалы жүктеме. Денені орау а
  `SoranetPrivacyEventV1` плюс қосымша `source` белгісі. Torii растайды
  белсенді телеметрия профиліне қарсы сұрау, оқиғаны жазады және жауап береді
  бар Norito JSON конвертімен бірге HTTP `202 Accepted`
  есептелген шелек терезесі (`bucket_start_unix`, `bucket_duration_secs`) және
  релелік режим.
- `POST /v2/soranet/privacy/share` `RecordSoranetPrivacyShareDto` қабылдайды
  пайдалы жүк. Денеде `SoranetPrivacyPrioShareV1` және қосымшасы бар
  `forwarded_by` кеңесі операторлар коллектор ағындарын тексере алады. Сәтті
  Жіберулер қорытындылайтын Norito JSON конверті бар HTTP `202 Accepted` қайтарады
  коллектор, шелек терезесі және басу туралы анықтама; валидация қателерінің картасы
  детерминирленген қатені өңдеуді сақтау үшін `Conversion` телеметрия жауабы
  коллекторлар бойынша. Оркестрдің оқиғалар циклі енді осы үлестерді өзі сияқты шығарады
  Torii Prio аккумуляторын релелік шелектермен синхрондауды сақтай отырып, сауалнама релесі.

Екі соңғы нүкте де телеметрия профилін құрметтейді: олар `503 қызметін шығарады
Көрсеткіштер өшірілгенде қолжетімді емес`. Клиенттер екілік Norito жібере алады
(`application/x.norito`) немесе Norito JSON (`application/x.norito+json`) денелері;
сервер стандартты Torii арқылы пішімді автоматты түрде келіседі
экстракторлар.

## Prometheus метрика

Әрбір экспортталған шелек `mode` (`entry`, `middle`, `exit`) және
`bucket_start` жапсырмалары. Келесі метрикалық отбасылар шығарылады:

| метрикалық | Сипаттама |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` көмегімен қол алысу таксономиясы. |
| `soranet_privacy_throttles_total{scope}` | `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` бар дроссель есептегіштері. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Жиынтық салқындату ұзақтығы қысқартылған қол алысуларға байланысты. |
| `soranet_privacy_verified_bytes_total` | Соқыр өлшеу дәлелдерінен расталған өткізу қабілеттілігі. |
| `soranet_privacy_active_circuits_{avg,max}` | Бір шелектегі орташа және пик белсенді тізбектер. |
| `soranet_privacy_rtt_millis{percentile}` | RTT пайыздық бағалаулары (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Санат дайджесті арқылы кілттелген басқару әрекеті есебі есептегіштері. |
| `soranet_privacy_bucket_suppressed` | Салымшы шегі орындалмағандықтан, шелектер ұсталды. |
| `soranet_privacy_pending_collectors{mode}` | Реле режимі бойынша топтастырылған жинақтаушы біріктіру күтілуде. |
| `soranet_privacy_suppression_total{reason}` | `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` бар шелек есептегіштері басылған, сондықтан бақылау тақталары құпиялылық олқылықтарын көрсете алады. |
| `soranet_privacy_snapshot_suppression_ratio` | Соңғы ағынның басылған/тазаланған қатынасы (0–1), ескерту бюджеттері үшін пайдалы. |
| `soranet_privacy_last_poll_unixtime` | Ең соңғы сәтті сауалнаманың UNIX уақыт белгісі (коллектордың бос тұру ескертуін басқарады). |
| `soranet_privacy_collector_enabled` | Құпиялылық коллекторы өшірілгенде немесе іске қосылмаған кезде `0` түріне ауысатын көрсеткіш (коллектор өшірілген ескертуді басқарады). |
| `soranet_privacy_poll_errors_total{provider}` | Реле бүркеншік аты бойынша топтастырылған сұрау сәтсіздіктері (декод қателеріндегі өсулер, HTTP қателері немесе күтпеген күй кодтары). |

Бақылаусыз шелектер үнсіз қалады, бақылау тақталарын ұқыпты ұстайды
нөлдік толтырылған терезелерді жасау.

## Операциялық нұсқаулық

1. **Бақылау тақталары** – `mode` және `window_start` арқылы топтастырылған жоғарыдағы көрсеткіштерді диаграммалаңыз.
   Коллектор немесе реле мәселелерін шешу үшін жетіспейтін терезелерді бөлектеңіз. Қолдану
   `soranet_privacy_suppression_total{reason}` салымшыны ажырату үшін
   саңылауларды тригалау кезінде коллектормен басқарылатын басудан туындайтын кемшіліктер. Grafana
   актив енді осылар беретін арнайы **«Басу себептері (5м)»** панелін жібереді.
   есептегіштер плюс есептейтін **“Басылған шелек %”** статистикасы
   Таңдау үшін `sum(soranet_privacy_bucket_suppressed) / count(...)`, сондықтан
   операторлар бюджеттік бұзушылықтарды бір қарағанда байқай алады. **Коллектордың үлесі
   Backlog** сериясы (`soranet_privacy_pending_collectors`) және **Snapshot
   Басу коэффициенті** статистикасы кезінде кептеліп қалған коллекторлар мен бюджеттің ауытқуын көрсетеді
   автоматтандырылған жүгірулер.
2. **Ескерту** – құпиялылық үшін қауіпсіз есептегіштерден дабылдарды басқарады: PoW қабылдамау ұштары,
   салқындату жиілігі, RTT дрейфі және сыйымдылықты қабылдамау. Өйткені есептегіштер
   әр шелектегі монотонды, тарифке негізделген қарапайым ережелер жақсы жұмыс істейді.
3. **Оқиғаға жауап** – алдымен жинақталған деректерге сүйеніңіз. Тереңірек жөндеу кезінде
   қажет болса, шелек суреттерін қайталау немесе соқырларды тексеру үшін релелер сұраңыз
   шикі трафик журналдарын жинаудың орнына өлшеу дәлелдері.
4. **Ұстау** – асып кетпеу үшін жиі қырыңыз
   `max_completed_buckets`. Экспорттаушылар Prometheus шығысын келесідей қарастыруы керек
   канондық дереккөз және қайта жіберілгеннен кейін жергілікті шелектерді тастаңыз.

## Басу талдаулары және автоматтандырылған іске қосуларSNNet-8 қабылдау автоматтандырылған коллекторлардың қалатынын көрсетуге байланысты
сау және бұл басу саясат шегінде қалады (әрбір шелектердің ≤10%
кез келген 30 минуттық терезе арқылы өту). Қазір бұл қақпаны қанағаттандыру үшін құрал қажет
ағашы бар кемелер; операторлар оны апта сайынғы рәсімдеріне қосуы керек. Жаңа
Grafana басу панелдері төмендегі PromQL үзінділерін көрсетеді, қоңырау кезінде
командалар қолмен сұрауларға қайта оралмас бұрын тікелей көріну мүмкіндігін береді.

### Басуды тексеруге арналған PromQL рецепттері

Операторлар келесі PromQL көмекшілерін қолында ұстауы керек; екеуіне сілтеме жасалған
ортақ Grafana бақылау тақтасында (`dashboards/grafana/soranet_privacy_metrics.json`)
және Alertmanager ережелері:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

**“Басылған шелек %”** статистикасының төменде қалуын растау үшін қатынас шығысын пайдаланыңыз
саяси бюджет; жылдам кері байланыс үшін тік детекторды Alertmanager жүйесіне өткізіңіз
салымшылардың саны күтпеген жерден төмендегенде.

### Офлайн шелек есебі CLI

Жұмыс кеңістігі бір реттік NDJSON үшін `cargo xtask soranet-privacy-report` көрсетеді
түсіреді. Оны бір немесе бірнеше релелік әкімші экспортына көрсетіңіз:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

Көмекші түсіруді `SoranetSecureAggregator` арқылы жібереді, a басып шығарады
stdout жүйесіне басуды қорытындылайды және міндетті түрде құрылымдық JSON есебін жазады
`--json-out <path|->` арқылы. Ол тірі коллектор сияқты бірдей тұтқаларды құрметтейді
(`--bucket-secs`, `--min-contributors`, `--expected-shares`, т.б.), жалға беру
операторлар триаждау кезінде әртүрлі шектерде тарихи түсірулерді қайталайды
мәселе. JSON файлын Grafana скриншоттарымен бірге тіркеңіз, сонда SNNet-8
басуды талдау қақпасы тексерілетін болып қалады.

### Бірінші автоматтандырылған іске қосуды тексеру тізімі

Басқару әлі де бірінші автоматтандырудың орындалғанын дәлелдеуді талап етеді
шектеу бюджеті. Көмекші енді `--max-suppression-ratio <0-1>` қабылдайды
Басылған шелектер рұқсат етілген мәннен асып кеткенде, CI немесе операторлар тез істен шығуы мүмкін
терезе (әдепкі 10%) немесе әлі ешқандай шелек болмаған кезде. Ұсынылатын ағын:

1. NDJSON реле әкімшісінің соңғы нүктелерінен және оркестратордан экспорттаңыз
   `/v2/soranet/privacy/event|share` ағыны
   `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Саясат бюджетімен көмекшіні іске қосыңыз:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   Пәрмен байқалған қатынасты басып шығарады және бюджет болған кезде нөлден тыс шығады
   ешбір шелек дайын болмаған кезде **немесе** асып кетті, бұл телеметрияның дайын емес екенін білдіреді
   әлі жүгіру үшін шығарылған. Тікелей көрсеткіштер көрсетілуі керек
   `soranet_privacy_pending_collectors` нөлге қарай ағызу және
   `soranet_privacy_snapshot_suppression_ratio` бір бюджетте қалады
   іске қосу орындалып жатқанда.
3. JSON шығысын және CLI журналын SNNet-8 дәлелдер жинағымен бұрын мұрағаттаңыз
   рецензенттер нақты артефактілерді қайталай алатындай етіп тасымалдаудың әдепкі параметрін аударыңыз.

## Келесі қадамдар (SNNet-8a)

- Қос Prio коллекторларын біріктіріп, олардың үлесін қосуды қосыңыз
  жұмыс уақыты, сондықтан релелер мен коллекторлар дәйекті `SoranetPrivacyBucketMetricsV1` шығарады
  пайдалы жүктемелер. *(Орындалды — `ingest_privacy_payload` қараңыз
  `crates/sorafs_orchestrator/src/lib.rs` және ілеспе сынақтар.)*
- Ортақ Prometheus бақылау тақтасын JSON және ескерту ережелерін жариялаңыз
  басу саңылаулары, коллектордың денсаулығы және анонимдік өшуі. *(Орындалды - қараңыз
  `dashboards/grafana/soranet_privacy_metrics.json`,
  `dashboards/alerts/soranet_privacy_rules.yml`,
  `dashboards/alerts/soranet_policy_rules.yml` және тексеру құрылғылары.)*
- бөлімінде сипатталған дифференциалды-құпиялық калибрлеу артефактілерін жасаңыз
  `privacy_metrics_dp.md`, оның ішінде қайталанатын жазу кітапшалары мен басқару
  қорытады. *(Орындалды — жазу кітапшасы + жасаған артефактілер
  `scripts/telemetry/run_privacy_dp.py`; CI қаптамасы
  `scripts/telemetry/run_privacy_dp_notebook.sh` жазу кітапшасын арқылы орындайды
  `.github/workflows/release-pipeline.yml` жұмыс процесі; басқару дайджесті енгізілді
  `docs/source/status/soranet_privacy_dp_digest.md`.)*

Ағымдағы шығарылым SNNet-8 негізін береді: детерминистикалық,
Қолданыстағы Prometheus қырғыштарына тікелей кіретін құпиялылық үшін қауіпсіз телеметрия
және бақылау тақталары. Құпиялылықты дифференциалды калибрлеу артефактілері бар
шығару құбырының жұмыс процесі ноутбук шығыстарын жаңа, ал қалғандарын сақтайды
жұмыс бірінші автоматтандырылған іске қосуды бақылауға және басуды ұзартуға бағытталған
ескерту талдауы.