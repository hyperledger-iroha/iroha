---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b80573de9799c783b62fe4babb553de4dd0778b028cd6d6ad58eb3094f7284eb
source_last_modified: "2026-01-04T08:19:26.497389+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Provider Advert Rollout Plan"
translator: machine-google-reviewed
---

> [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) нұсқасынан бейімделген.

№ SoraFS провайдерінің жарнамасын шығару жоспары

Бұл жоспар рұқсат етілген провайдер жарнамаларынан үзіндіні үйлестіреді
көп көзді бөлікке қажет толық басқарылатын `ProviderAdvertV1` беті
іздеу. Ол үш жеткізілімге назар аударады:

- **Оператор нұсқаулығы.** Қадамдық әрекеттерді сақтау провайдерлері орындауы керек
  әрбір қақпа аударылмас бұрын.
- **Телеметриялық қамту.** Бақылау мүмкіндігі және Операциялар пайдаланатын бақылау тақталары мен ескертулер
  растау үшін желі тек сәйкес жарнамаларды қабылдайды.
Шығарылым [SoraFS көшіруіндегі SF-2b/2c кезеңдерімен сәйкес келеді.
жол картасы](./migration-roadmap) және қабылдау саясатын қабылдайды
[провайдердің қабылдау саясаты](./provider-admission-policy) әлдеқашан енгізілген
әсері.

## Ағымдағы талаптар

SoraFS тек басқару конверті бар `ProviderAdvertV1` пайдалы жүктемелерін қабылдайды. The
қабылдау кезінде келесі талаптар орындалады:

- `profile_id=sorafs.sf1@1.0.0` канондық `profile_aliases` бар.
- `chunk_range_fetch` мүмкіндігінің пайдалы жүктемелері көп көзге қосылуы керек
  іздеу.
- `signature_strict=true` хабарландыруға кеңестік қолтаңбасы бар
  конверт.
- `allow_unknown_capabilities` тек МАЙ ЖАУҒА айқын жаттығулар кезінде рұқсат етіледі.
  және тіркелуі керек.

## Операторды тексеру тізімі

1. **Инвентарлық жарнамалар.** Әрбір жарияланған хабарландыруларды тізімдеңіз және жазыңыз:
   - Басқарушы конверт жолы (`defaults/nexus/sorafs_admission/...` немесе өндіріс баламасы).
   - Жарнама `profile_id` және `profile_aliases`.
   - Мүмкіндіктер тізімі (кем дегенде `torii_gateway` және `chunk_range_fetch` күтіңіз).
   - `allow_unknown_capabilities` жалаушасы (жеткізушілер сақтаған TLV бар кезде қажет).
2. **Провайдер құралдарымен қалпына келтіріңіз.**
   - Провайдеріңіздің жарнама шығарушысымен пайдалы жүктемені қайта құрыңыз, осылайша:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` анықталған `max_span`
     - GREASE TLV бар кезде `allow_unknown_capabilities=<true|false>`
   - `/v2/sorafs/providers` және `sorafs_fetch` арқылы растау; белгісіз туралы ескертулер
     мүмкіндіктерін сынау керек.
3. **Көп дереккөздің дайындығын тексеру.**
   - `sorafs_fetch` `--provider-advert=<path>` көмегімен орындаңыз; CLI енді сәтсіз аяқталды
     `chunk_range_fetch` жоқ болғанда және еленбеген белгісіз үшін ескертулерді басып шығарады
     мүмкіндіктері. JSON есебін түсіріп, оны операциялар журналдарымен мұрағаттаңыз.
4. **Кезеңдік жаңартулар.**
   - `ProviderAdmissionRenewalV1` конверттерін кем дегенде 30 күн бұрын жіберіңіз
     жарамдылық мерзімі. Жаңартулар канондық дескриптор мен мүмкіндіктер жинағын сақтауы керек;
     тек үлес, соңғы нүктелер немесе метадеректер өзгеруі керек.
5. **Тәуелді топтармен сөйлесіңіз.**
   - SDK иелері операторларға ескертулер беретін нұсқаларды шығаруы керек
     жарнамалар қабылданбайды.
   - DevRel әрбір фазалық ауысуды хабарлайды; бақылау тақтасының сілтемелерін және
     төменгі шекті логика.
6. **Бақылау тақталары мен ескертулерді орнатыңыз.**
   - Grafana экспортын импорттаңыз және оны **SoraFS / Провайдер астында орналастырыңыз
     UID `sorafs-provider-admission` бақылау тақтасы бар шығару**.
   - Ескерту ережелерінің ортақ `sorafs-advert-rollout` нұсқаулығын қамтамасыз етіңіз
     қойылым мен өндірістегі хабарландыру арнасы.

## Телеметрия және бақылау тақталары

Келесі көрсеткіштер `iroha_telemetry` арқылы әлдеқашан ашылған:

- `torii_sorafs_admission_total{result,reason}` — қабылданған, қабылданбаған сандар,
  және ескерту нәтижелері. Себептерге `missing_envelope`, `unknown_capability`,
  `stale`, және `policy_violation`.

Grafana экспорттау: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Файлды ортақ бақылау тақталарының репозиторийіне импорттау (`observability/dashboards`)
және жариялау алдында тек UID деректер көзін жаңартыңыз.

Басқарма Grafana **SoraFS / Provider Rollout** қалтасының астында жариялайды.
тұрақты UID `sorafs-provider-admission`. Ескерту ережелері
`sorafs-admission-warn` (ескерту) және `sorafs-admission-reject` (сыни)
`sorafs-advert-rollout` хабарландыру саясатын пайдалану үшін алдын ала конфигурацияланған; реттеу
тағайындау тізімі өзгертудің орнына өзгерсе, сол байланыс нүктесі
бақылау тақтасы JSON.

Ұсынылатын Grafana панельдері:

| Панель | Сұрау | Ескертпелер |
|-------|-------|-------|
| **Қабылдау нәтижесінің көрсеткіші** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Қабылдау және ескерту және қабылдамауды визуализациялау үшін стек диаграммасы. Ескерту > 0,05 * жалпы (ескерту) немесе қабылдамау > 0 (сыни). |
| **Ескерту коэффициенті** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Пейджер шегін беретін бір жолды уақыт сериялары (5% ескерту жылдамдығы 15 минут). |
| **Қабылдамау себептері** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Drives runbook триажы; жұмсарту қадамдарына сілтемелер тіркеңіз. |
| **Қарызды жаңарту** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Жаңарту мерзімін өткізіп алған провайдерлерді көрсетеді; табу кэш журналдарымен айқас сілтеме. |

Қолмен басқару тақталарына арналған CLI артефактілері:

- `sorafs_fetch --provider-metrics-out` `failures`, `successes` және
  Әр провайдерге `disabled` есептегіштері. Бақылау үшін арнайы бақылау тақталарына импорттаңыз
  өндірістік провайдерлерді ауыстыру алдында оркестрдің құрғақ жұмысы.
- JSON есебінің `chunk_retry_rate` және `provider_failure_rate` өрістері
  қабылдауға дейін жиі болатын дроссельдік немесе ескірген пайдалы жүктеме белгілерін бөлектеңіз
  бас тартулар.

### Grafana бақылау тақтасының орналасуы

Observability арнайы тақтаны жариялайды — **SoraFS Провайдерді қабылдау
Rollout** (`sorafs-provider-admission`) — **SoraFS / Provider Rollout** астында
келесі канондық панель идентификаторларымен:

- 1-панель — *Қабылдау нәтижесінің көрсеткіші* (қабатталған аумақ, «опс/мин» бірлігі).
- Панель 2 — *Ескерту коэффициенті* (бір қатар), өрнекті шығаратын
  `сома(ставка(torii_sorafs_admission_total{нәтиже="ескерту"}[5м]) /
   сома(ставка(torii_sorafs_admission_total[5м]))`.
- Панель 3 — *Қабылдамау себептері* (`reason` бойынша топтастырылған уақыт қатарлары), сұрыпталған
  `rate(...[5m])`.
- 4-панель — *Қарызды жаңарту* (стат), жоғарыдағы кестедегі сұрауды қайталау және
  тасымалдау кітабынан алынған хабарландыруды жаңарту мерзімдерімен түсіндіріледі.

Инфрақұрылым бақылау тақталарындағы реподағы JSON қаңқасын көшіріңіз (немесе жасаңыз).
`observability/dashboards/sorafs_provider_admission.json`, содан кейін ғана жаңартыңыз
деректер көзі UID; панель идентификаторлары мен ескерту ережелеріне runbooks сілтеме жасайды
төменде, сондықтан осы құжаттаманы қайта қарамай, оларды қайта нөмірлеуден аулақ болыңыз.

Ыңғайлы болу үшін репозиторий қазір анықтамалық бақылау тақтасының анықтамасын мына мекенжайға жібереді
`docs/source/grafana_sorafs_admission.json`; егер болса, оны Grafana қалтасына көшіріңіз
сізге жергілікті тестілеу үшін бастапқы нүкте қажет.

### Prometheus ескерту ережелері

Келесі ережелер тобын `observability/prometheus/sorafs_admission.rules.yml` ішіне қосыңыз
(егер бұл бірінші SoraFS ереже тобы болса, файлды жасаңыз) және оны мына жерден қосыңыз
Prometheus конфигурацияңыз. `<pagerduty>` мәнін нақты бағыттаумен ауыстырыңыз
шақыру бойынша айналымға арналған белгі.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml` іске қосыңыз
синтаксис `promtool check rules` өтуін қамтамасыз ету үшін өзгертулерді баспас бұрын.

## Қабылдау нәтижелері

- `chunk_range_fetch` мүмкіндігі жоқ → `reason="missing_capability"` арқылы қабылдамау.
- `allow_unknown_capabilities=true` жоқ белгісіз TLV мүмкіндігі → қабылдамау
  `reason="unknown_capability"`.
- `signature_strict=false` → қабылдамау (оқшауланған диагностика үшін сақталған).
- Мерзімі өткен `refresh_deadline` → қабылдамау.

## Байланыс және оқиғаны өңдеу

- **Апта сайынғы мәртебе жіберуші.** DevRel қабылдаудың қысқаша мазмұнын таратады
  көрсеткіштер, орындалмаған ескертулер және алдағы мерзімдер.
- **Оқиғаға жауап беру.** Егер `reject` өрт туралы ескертсе, шақыру бойынша инженерлер:
  1. Torii табу (`/v2/sorafs/providers`) арқылы қорлайтын жарнаманы алыңыз.
  2. Провайдер құбырында жарнаманы тексеруді қайта іске қосыңыз және онымен салыстырыңыз
     Қатені шығару үшін `/v2/sorafs/providers`.
  3. Келесі жаңартуға дейін жарнаманы айналдыру үшін провайдермен келісіңіз
     мерзімі.
- **Өзгеріс қатып қалады.** R1/R2 кезінде ешбір мүмкіндік схемасы жерді өзгертпейді
  қабылдау комиссиясы қол қояды; GREASE сынақтары кезінде жоспарланған болуы керек
  апта сайынғы техникалық қызмет көрсету терезесі және көшіру кітабына енгізілген.

## Анықтамалар

- [SoraFS Түйін/Клиент протоколы](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Провайдердің қабылдау саясаты](./provider-admission-policy)
- [Көші-қон жол картасы](./migration-roadmap)
- [Провайдер жарнамасының көп көзді кеңейтімдері](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)