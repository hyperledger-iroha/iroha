---
id: pq-rollout-plan
lang: kk
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNNet-16G Post-Quantum Rollout Playbook
sidebar_label: PQ Rollout Plan
description: Operational guide for promoting the SoraNet hybrid X25519+ML-KEM handshake from canary to default across relays, clients, and SDKs.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ескерту Канондық дереккөз
:::

SNNet-16G SoraNet тасымалдау үшін кванттық шығаруды аяқтайды. `rollout_phase` тұтқалары операторларға бар A кезеңінің қорғау талабынан B сатысының көпшілік қамтуына және C кезеңінің қатаң PQ күйіне дейін әр бет үшін өңделмеген JSON/TOML өңдеусіз детерминирленген жылжытуды үйлестіруге мүмкіндік береді.

Бұл ойын кітабы мыналарды қамтиды:

- Фазалық анықтамалар және жаңа конфигурация тұтқалары (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) код базасына (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`) жалғанған.
- SDK және CLI жалауын салыстыру, осылайша әрбір клиент шығарылымды бақылай алады.
- Реле/клиенттің канариялық жоспарлау күтулері, сонымен қатар жылжытуды қамтамасыз ететін басқару тақталары (`dashboards/grafana/soranet_pq_ratchet.json`).
- Кері ілмектер және өртке қарсы бұрғылау кітабына сілтемелер ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Фазалық карта

| `rollout_phase` | Анонимділіктің тиімді кезеңі | Әдепкі әсер | Әдеттегі қолдану |
|----------------|--------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (А кезеңі) | Флот қызған кезде бір тізбекке кемінде бір PQ қорғаушысы қажет. | Негізгі және ерте канар апталары. |
| `ramp` | `anon-majority-pq` (В кезеңі) | >= 2/3 қамту үшін PQ релелеріне бейімділік таңдау; классикалық релелер резерв ретінде қалады. | Аймақ бойынша эстафеталық канареялар; SDK алдын ала қарау ауыстырады. |
| `default` | `anon-strict-pq` (С кезеңі) | Тек PQ сұлбаларын орындаңыз және төмендетілген дабылдарды қатайтыңыз. | Телеметрия мен басқаруға қол қою аяқталғаннан кейін соңғы жылжыту. |

Егер бет сонымен қатар айқын `anonymity_policy` орнатса, ол сол құрамдас үшін фазаны қайта анықтайды. Айқын кезеңді өткізіп жіберу енді `rollout_phase` мәніне ауыстырылады, осылайша операторлар фазаны әр ортаға бір рет аударып, клиенттерге оны мұраға алуға мүмкіндік береді.

## Конфигурация анықтамасы

### Оркестр (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Оркестр жүктеушісі қалпына келтіру кезеңін орындау уақытында шешеді (`crates/sorafs_orchestrator/src/lib.rs:2229`) және оны `sorafs_orchestrator_policy_events_total` және `sorafs_orchestrator_pq_ratio_*` арқылы көрсетеді. Қолдануға дайын үзінділер үшін `docs/examples/sorafs_rollout_stage_b.toml` және `docs/examples/sorafs_rollout_stage_c.toml` қараңыз.

### Тот клиенті / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` енді талданған кезеңді (`crates/iroha/src/client.rs:2315`) жазады, осылайша көмекші пәрмендер (мысалы, `iroha_cli app sorafs fetch`) әдепкі анонимділік саясатымен қатар ағымдағы кезең туралы есеп бере алады.

## Автоматтандыру

Екі `cargo xtask` көмекшісі кесте құруды және артефакті түсіруді автоматтандырады.

1. **Аймақтық кестені жасаңыз**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   Ұзақтықтар `s`, `m`, `h` немесе `d` жұрнақтарын қабылдайды. Пәрмен өзгерту сұрауымен бірге жеткізілуі мүмкін `artifacts/soranet_pq_rollout_plan.json` және Markdown жиынын (`artifacts/soranet_pq_rollout_plan.md`) шығарады.

2. **Бұрғылау артефактілерін қолтаңбаларымен түсіріңіз**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   Пәрмен берілген файлдарды `artifacts/soranet_pq_rollout/<timestamp>_<label>/` ішіне көшіреді, әрбір артефакт үшін BLAKE3 дайджесттерін есептейді және пайдалы жүктеменің үстіне метадеректер мен Ed25519 қолтаңбасын қамтитын `rollout_capture.json` жазады. Басқару түсіруді жылдам тексеруі үшін өрт сөндіру жаттығуларының минуттарына қол қоятын бірдей жеке кілтті пайдаланыңыз.

## SDK және CLI жалауша матрицасы

| Беттік | Канария (А кезеңі) | Рамп (В кезеңі) | Әдепкі (С кезеңі) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` алу | `--anonymity-policy stage-a` немесе фазаға сүйеніңіз | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Оркестр конфигурациясы JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust клиент конфигурациясы (`iroha.toml`) | `rollout_phase = "canary"` (әдепкі) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` қол қойылған командалар | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, таңдау бойынша `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, міндетті түрде `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, міндетті түрде `.ANON_STRICT_PQ` |
| JavaScript оркестрінің көмекшілері | `rolloutPhase: "canary"` немесе `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Барлық SDK картаны оркестрмен (`crates/sorafs_orchestrator/src/lib.rs:365`) пайдаланатын бір кезең талдаушысына ауыстырады, сондықтан аралас тілді орналастырулар конфигурацияланған кезеңмен құлыптау қадамында қалады.

## Канариялық жоспарлауды бақылау тізімі

1. **Алдын ала ұшу (Т минус 2 апта)**

- Алдыңғы екі аптадағы өшіп қалу жылдамдығын <1% және аймаққа PQ қамту >=70% (`sorafs_orchestrator_pq_candidate_ratio`) кезеңін растаңыз.
   - Канар терезесін бекітетін басқаруды шолу ұяшығын жоспарлаңыз.
   - Кезеңде `sorafs.gateway.rollout_phase = "ramp"` жаңартыңыз (JSON оркестрін өңдеңіз және қайта орналастырыңыз) және жылжыту құбырын құрғатыңыз.

2. **Эстафеталық канарея (Т күні)**

   - Оркестрде және қатысушы эстафета манифестінде `rollout_phase = "ramp"` орнату арқылы бір уақытта бір аймақты жылжытыңыз.
   - TTL қорғау кэшінің екі есе көп болуы үшін PQ Ratchet бақылау тақтасындағы (қазір шығару тақтасы бар) "Нәтижеге қатысты саясат оқиғалары" мен "Қою жылдамдығын" бақылаңыз.
   - `sorafs_cli guard-directory fetch` суретін тексеруді сақтау үшін іске қосу алдында және одан кейін кесіңіз.

3. **Клиент/SDK канариясы (Т плюс 1 апта)**

   - `rollout_phase = "ramp"` параметрін клиент конфигурацияларында аударыңыз немесе тағайындалған SDK когорттары үшін `stage-b` қайта анықтауларын өткізіңіз.
   - Телеметриялық айырмашылықтарды түсіріңіз (`sorafs_orchestrator_policy_events_total` `client_id` және `region` арқылы топтастырылған) және оларды шығару оқиғалары журналына тіркеңіз.

4. **Әдепкі жарнама (T плюс 3 апта)**

   - Басқаруды өшіргеннен кейін, оркестр мен клиент теңшелімдерін `rollout_phase = "default"` параметріне ауыстырыңыз және қол қойылған дайындықты тексеру тізімін шығарылым артефактілеріне айналдырыңыз.

## Басқару және дәлелдемелерді тексеру тізімі

| Фазалық өзгеріс | Жарнама қақпасы | Дәлелдер жинағы | Бақылау тақталары және ескертулер |
|-----------------------|----------------|-----------------|---------------------|
| Канария → Рамп *(В кезеңін алдын ала қарау)* | A сатысы - кейінгі 14 күн ішінде қайталану жылдамдығы <1%, алға жылжыған аймақ үшін `sorafs_orchestrator_pq_candidate_ratio` ≥ 0,7, Argon2 билеті p95 < 50 мс растайды және брондалған жарнамаға арналған басқару ұяшығы. | `cargo xtask soranet-rollout-plan` JSON/Markdown жұбы, жұптастырылған `sorafs_cli guard-directory fetch` суреттері (бұрын/кейін), қол қойылған `cargo xtask soranet-rollout-capture --label canary` жинағы және [PQ ratchet runbook](I180000000) сілтемесі бар канар минуттары. | `dashboards/grafana/soranet_pq_ratchet.json` (Саясат оқиғалары + Brownout Rate), `dashboards/grafana/soranet_privacy_metrics.json` (SN16 төмендету коэффициенті), `docs/source/soranet/snnet16_telemetry_plan.md` ішіндегі телеметрия сілтемелері. |
| Рамп → Әдепкі *(С сатысы)* | 30 күндік SN16 телеметриясының күйіп кетуі, бастапқыда `sn16_handshake_downgrade_total` тегіс, клиенттің канарейі кезінде `sorafs_orchestrator_brownouts_total` нөл және прокси ауыстырып-қосқышының репетициясы тіркелді. | `sorafs_cli proxy set-mode --mode gateway|direct` транскрипциясы, `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` шығысы, `sorafs_cli guard-directory verify` журналы және қол қойылған `cargo xtask soranet-rollout-capture --label default` бумасы. | Бірдей PQ Ratchet тақтасы және `docs/source/sorafs_orchestrator_rollout.md` және `dashboards/grafana/soranet_privacy_metrics.json` құжаттарында құжатталған SN16 төмендету панельдері. |
| Төтенше жағдайды төмендету / кері қайтару дайындығы | Төменгі деңгей санауыштары жоғарылағанда, қорғау каталогын тексеру сәтсіз болғанда немесе `/policy/proxy-toggle` буфері тұрақты төмендету оқиғаларын жазғанда іске қосылады. | `docs/source/ops/soranet_transport_rollback.md`, `sorafs_cli guard-directory import` / `guard-cache prune` журналдары, `cargo xtask soranet-rollout-capture --label rollback`, оқиға билеттері және хабарландыру үлгілерінен алынған бақылау тізімі. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` және екі ескерту бумалары (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Әрбір артефактты `artifacts/soranet_pq_rollout/<timestamp>_<label>/` астында жасалған `rollout_capture.json` көмегімен сақтаңыз, осылайша басқару пакеттерінде көрсеткіштер тақтасы, промқұралдар іздері және дайджесттер болады.
- Жүктеп салынған дәлелдердің SHA256 дайджесттерін (минуттар PDF, түсіру жинағы, күзет суреттері) жарнамалық хаттамаларға тіркеңіз, осылайша Парламент мақұлдауларын сахналық кластерге қол жеткізбестен қайта ойнатуға болады.
- `docs/source/soranet/snnet16_telemetry_plan.md` деңгейін төмендету сөздіктері мен ескерту шектерінің канондық көзі болып қала беретінін дәлелдеу үшін жарнамалық билеттегі телеметрия жоспарына сілтеме жасаңыз.

## Бақылау тақтасы және телеметрия жаңартулары

`dashboards/grafana/soranet_pq_ratchet.json` енді осы оқу кітабына қайта сілтеме жасайтын және басқару шолулары қай кезеңнің белсенді екенін растауы үшін ағымдағы кезеңді көрсететін "Шығарылым жоспары" аннотация тақтасымен бірге жеткізіледі. Панель сипаттамасын конфигурациялау түймелерінің болашақ өзгерістерімен синхрондаңыз.

Ескерту үшін бар ережелердің `stage` белгісін пайдаланғанына көз жеткізіңіз, осылайша канариялық және әдепкі фазалар бөлек саясат шектерін (`dashboards/alerts/soranet_handshake_rules.yml`) іске қосады.

## Қайтару ілмектері

### Әдепкі → Рамп (С кезеңі → B кезеңі)

1. `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` көмегімен оркестрдің дәрежесін төмендетіңіз (және SDK конфигурацияларында бірдей фазаны көрсетіңіз), осылайша B кезеңі бүкіл флот бойынша жалғасады.
2. `/policy/proxy-toggle` түзету жұмыс процесі тексерілетін болып қалуы үшін транскриптті түсіріп, `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` арқылы клиенттерді қауіпсіз тасымалдау профиліне мәжбүрлеңіз.
3. `artifacts/soranet_pq_rollout/` астында қорғау каталогының айырмашылықтарын, промқұрал шығысын және бақылау тақтасының скриншоттарын мұрағаттау үшін `cargo xtask soranet-rollout-capture --label rollback-default` іске қосыңыз.

### Рампа → Канария (В кезеңі → A кезеңі)

1. `sorafs_cli guard-directory import --guard-directory guards.json` көмегімен жылжыту алдында түсірілген қорғаушы каталогының суретін импорттаңыз және төмендету пакеті хэштерді қамтитындай `sorafs_cli guard-directory verify` қайта іске қосыңыз.
2. Оркестр және клиент конфигурацияларында `rollout_phase = "canary"` (немесе `anonymity_policy stage-a` арқылы қайта анықтау) орнатыңыз, содан кейін төмендетілген құбырды дәлелдеу үшін [PQ ratchet runbook](./pq-ratchet-runbook.md) ішінен PQ бұрғылау бұрғысын қайта ойнатыңыз.
3. Жаңартылған PQ Ratchet және SN16 телеметрия скриншоттарын және ескерту нәтижелерін басқаруды хабардар етпес бұрын оқиға журналына тіркеңіз.

### Қорғау туралы еске салғыштар- Төмендету орын алған сайын `docs/source/ops/soranet_transport_rollback.md` сілтемесі және кез келген уақытша жұмсартуды кейінгі жұмыс үшін шығару трекерінде `TODO:` элементі ретінде тіркеңіз.
- `dashboards/alerts/soranet_handshake_rules.yml` және `dashboards/alerts/soranet_privacy_rules.yml` кері қайтару алдында және одан кейін `promtool test rules` қамту астында сақтаңыз, осылайша ескертулер дрейфі түсіру жинағымен бірге құжатталған.