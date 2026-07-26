---
lang: kk
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6120b882618e0f9b6113948d3b12d97e0152a5fc5d4350681ba30aaf114e99d3
source_last_modified: "2026-01-22T14:45:01.354580+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-default-lane-quickstart
title: Default lane quickstart (NX-5)
sidebar_label: Default Lane Quickstart
description: Configure and verify the Nexus default lane fallback so Torii and SDKs can omit lane_id in public lanes.
translator: machine-google-reviewed
---

:::ескерту Канондық дереккөз
Бұл бет `docs/source/quickstart/default_lane.md` көрсетеді. Екі көшірмені де сақтаңыз
локализация сыпырып порталға түскенше тураланады.
:::

# Әдепкі жолақты жылдам іске қосу (NX-5)

> **Жол картасының мәтінмәні:** NX-5 — әдепкі қоғамдық жолақ интеграциясы. Қазір орындалу уақыты
> `nexus.routing_policy.default_lane` резервін көрсетеді, сондықтан Torii REST/gRPC
> соңғы нүктелер және әрбір SDK трафик тиесілі болған кезде `lane_id`-ті қауіпсіз түрде өткізіп жіберуі мүмкін
> канондық қоғамдық жолақта. Бұл нұсқаулық операторларды конфигурациялау арқылы көрсетеді
> каталог, `/status` ішіндегі қалпына келтіруді тексеру және клиентті орындау
> мінез-құлық соңына дейін.

## Алғышарттар

- `irohad` Sora/Nexus құрастырмасы (`irohad --sora --config ...` арқылы жұмыс істейді).
- `nexus.*` бөлімдерін өңдеуге болатын конфигурация репозиторийіне кіру.
- `iroha_cli` мақсатты кластермен сөйлесу үшін конфигурацияланған.
- `curl`/`jq` (немесе баламасы) Torii `/status` пайдалы жүктемесін тексеру үшін.

## 1. Жолақ және деректер кеңістігі каталогын сипаттаңыз

Желіде болуы керек жолақтар мен деректер кеңістігін жариялаңыз. Үзінді
төменде (`defaults/nexus/config.toml` кесілген) үш жалпыға ортақ жолақты тіркейді
плюс сәйкес деректер кеңістігінің бүркеншік аттары:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

Әрбір `index` бірегей және сабақтас болуы керек. Деректер кеңістігінің идентификаторлары 64 биттік мәндер;
жоғарыдағы мысалдар анықтық үшін жолақ индекстері сияқты бірдей сандық мәндерді пайдаланады.

## 2. Маршрутизацияның әдепкі мәндерін және қосымша қайта анықтауларды орнатыңыз

`nexus.routing_policy` бөлімі қалпына келтіру жолағын басқарады және сізге мүмкіндік береді
арнайы нұсқаулар немесе тіркелгі префикстері үшін маршруттауды қайта анықтау. Ереже болмаса
сәйкес келсе, жоспарлаушы транзакцияны конфигурацияланған `default_lane` бағытына бағыттайды
және `default_dataspace`. Маршрутизатордың логикасы тұрады
`crates/iroha_core/src/queue/router.rs` және саясатты ашық түрде қолданады
Torii REST/gRPC беттері.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

Кейінірек жаңа жолдарды қосқанда, алдымен каталогты жаңартыңыз, содан кейін бағыттауды кеңейтіңіз
ережелер. Артқы жолақ ұстап тұрған жалпыға ортақ жолаққа бағытталуын жалғастыруы керек

## 3. Қолданылған саясатпен түйінді жүктеңіз

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

Түйін іске қосу кезінде алынған маршруттау саясатын тіркейді. Кез келген тексеру қателері
(жоғалған индекстер, қайталанатын бүркеншік аттар, жарамсыз деректер кеңістігі идентификаторлары) бұрын пайда болады
өсек басталады.

## 4. Жолақты басқару күйін растаңыз

Түйін желіге қосылғаннан кейін әдепкі жолақ екенін тексеру үшін CLI көмекшісін пайдаланыңыз
мөрленген (манифест жүктелген) және қозғалысқа дайын. Жиынтық көрініс бір жолды басып шығарады
бір жолақ бойынша:

```bash
iroha_cli app nexus lane-report --summary
```

Мысал шығару:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

Әдепкі жолақ `sealed` көрсетсе, жолақты басқарудың нұсқаулығын орындаңыз.
сыртқы трафикке мүмкіндік береді. `--fail-on-sealed` жалауы CI үшін ыңғайлы.

## 5. Torii күйінің пайдалы жүктемелерін тексеріңіз

`/status` жауабы маршруттау саясатын да, әр жолақты жоспарлаушыны да көрсетеді.
сурет. Конфигурацияланған әдепкі мәндерді растау және оны тексеру үшін `curl`/`jq` пайдаланыңыз.
резервтік жолақ телеметрияны шығарады:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Шығару үлгісі:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

`0` жолағы үшін тірі жоспарлаушы есептегіштерін тексеру үшін:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Бұл TEU суретінің, бүркеншік ат метадеректерінің және манифест жалауларының тураланатынын растайды
конфигурациясымен. Дәл осындай пайдалы жүктеме Grafana панельдері үшін пайдаланылады
жолақты қабылдау бақылау тақтасы.

## 6. Клиенттің әдепкі мәндерін орындаңыз

- **Rust/CLI.** `iroha_cli` және Rust клиент жәшігінде `lane_id` өрісі жоқ.
  `--lane-id` / `LaneSelector` өтпеген кезде. Сондықтан кезек маршрутизаторы
  `default_lane` мәніне қайта түседі. Айқын `--lane-id`/`--dataspace-id` жалаушаларын пайдаланыңыз
  тек әдепкі емес жолаққа бағытталған кезде.
- **JS/Swift/Android.** Соңғы SDK шығарылымдары `laneId`/`lane_id` қосымша ретінде қарастырылады.
  және `/status` жариялаған мәнге қайта оралыңыз. Маршруттау саясатын сақтаңыз
  мобильді қолданбалар төтенше жағдайды қажет етпейтіндіктен, кезең мен өндіріс бойынша синхрондаңыз
  қайта конфигурациялау.
- **Құбыр/SSE сынақтары.** Транзакция оқиғасының сүзгілері қабылдайды
  `tx_lane_id == <u32>` предикаттары (`docs/source/pipeline.md` қараңыз). Жазылу
  `/v1/pipeline/events/transactions` осы сүзгі арқылы жіберілген жазуды дәлелдеу үшін
  анық жолақсыз кері жолақ идентификаторының астына келеді.

## 7. Бақылау және басқару ілмектері

- `/status` сонымен қатар `nexus_lane_governance_sealed_total` және
  `nexus_lane_governance_sealed_aliases`, сондықтан Alertmanager кез келген уақытта ескертеді a
  жолақ өзінің манифестін жоғалтады. Бұл ескертулерді тіпті devnets үшін қосулы ұстаңыз.
- Жоспарлағыштың телеметриялық картасы және жолақты басқару бақылау тақтасы
  (`dashboards/grafana/nexus_lanes.json`) келесіден бүркеншік ат/слаг өрістерін күтеді
  каталог. Бүркеншік атты өзгертсеңіз, сәйкес Kura каталогтарын осылай қайта белгілеңіз
  аудиторлар детерминирленген жолдарды сақтайды (NX-1 бойынша қадағаланады).
- Парламенттің әдепкі жолақтарға рұқсат беруі кері қайтару жоспарын қамтуы керек. Жазба
  манифест хэш және басқару дәлелдері осы жылдам бастаумен қатар сіздің
  Runbook операторы болашақ айналымдар қажетті күйді таппайды.

Бұл тексерулерден өткеннен кейін `nexus.routing_policy.default_lane` ретінде қарастыруға болады
желідегі код жолдары.