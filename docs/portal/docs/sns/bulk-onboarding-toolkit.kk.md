---
lang: kk
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a583af55cf8b4cf5070828bfb52146be88f92937c8d7887ab37a2056bf55ec9e
source_last_modified: "2026-01-22T16:26:46.515965+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
идентификатор: жаппай қосу құралдары жинағы
тақырыбы: SNS Bulk Onboarding Toolkit
sidebar_label: жаппай қосу құралдары жинағы
сипаттама: SN-3b тіркеушісі үшін CSV RegisterNameRequestV1 автоматтандыруы іске қосылады.
---

:::ескерту Канондық дереккөз
`docs/source/sns/bulk_onboarding_toolkit.md` айналары сыртқы операторлар көруі үшін
репозиторийді клондаусыз бірдей SN-3b нұсқауы.
:::

# SNS Bulk Onboarding Toolkit (SN-3b)

**Жол картасының анықтамасы:** SN-3b «Жаппай іске қосу құралдары»  
**Артефактілер:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Ірі тіркеушілер жиі жүздеген `.sora` немесе `.nexus` тіркеулерін алдын ала жасайды.
бірдей басқару рұқсаттарымен және есеп айырысу рельстерімен. JSON қолмен жасау
пайдалы жүктемелер немесе CLI қайта іске қосу масштабталмайды, сондықтан SN-3b детерминистикалық мәнді жібереді.
`RegisterNameRequestV1` құрылымдарын дайындайтын CSV - Norito құрастырушы
Torii немесе CLI. Көмекші алдыңғы қатардағы әрбір жолды тексереді, екеуін де шығарады
жинақталған манифест және қосымша жаңа жолмен бөлінген JSON және жібере алады
тексерулер үшін құрылымдық түбіртектерді жазу кезінде пайдалы жүктемелерді автоматты түрде береді.

## 1. CSV схемасы

Талдаушы келесі тақырып жолын қажет етеді (тәртіп икемді):

| Баған | Міндетті | Сипаттама |
|--------|----------|-------------|
| `label` | Иә | Сұралған белгі (аралас регистр қабылданады; құрал v1 және UTS-46 нормасына сәйкес қалыпқа келтіріледі). |
| `suffix_id` | Иә | Сандық жұрнақ идентификаторы (ондық немесе `0x` он алтылық). |
| `owner` | Иә | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | Иә | `1..=255` бүтін сан. |
| `payment_asset_id` | Иә | Есеп айырысу активі (мысалы, `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Иә | Активтің төл бірліктерін көрсететін таңбасыз бүтін сандар. |
| `settlement_tx` | Иә | JSON мәні немесе төлем транзакциясын немесе хэшті сипаттайтын әріптік жол. |
| `payment_payer` | Иә | Төлемге рұқсат берген AccountId. |
| `payment_signature` | Иә | JSON немесе басқарушы немесе қазынашылық қол қою дәлелін қамтитын әріптік жол. |
| `controllers` | Қосымша | Контроллер тіркелгі мекенжайларының нүктелі үтірмен немесе үтірмен бөлінген тізімі. Өткізілмесе, әдепкі `[owner]`. |
| `metadata` | Қосымша | Кірістірілген JSON немесе `@path/to/file.json` шешуші кеңестерді, TXT жазбаларын және т.б. қамтамасыз етеді. Әдепкі `{}`. |
| `governance` | Қосымша | Кірістірілген JSON немесе `@path` `GovernanceHookV1` белгісін көрсетеді. `--require-governance` осы бағанды ​​орындайды. |

Кез келген баған ұяшық мәніне `@` префиксін қою арқылы сыртқы файлға сілтеме жасай алады.
Жолдар CSV файлына қатысты шешіледі.

## 2. Көмекшіні іске қосу

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Негізгі опциялар:

- `--require-governance` басқару ілгегі жоқ жолдарды қабылдамайды (үшін пайдалы
  премиум аукциондар немесе резервтік тапсырмалар).
- `--default-controllers {owner,none}` контроллер ұяшықтарының бос болуын шешеді
  иесінің есептік жазбасына қайта оралыңыз.
- `--controllers-column`, `--metadata-column` және `--governance-column` атауын өзгерту
  жоғарғы экспортпен жұмыс істеу кезінде қосымша бағандар.

Сәтті болған кезде сценарий жинақталған манифест жазады:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "soraカタカナ...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"soraカタカナ...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"soraカタカナ...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

Егер `--ndjson` берілсе, әрбір `RegisterNameRequestV1` ретінде де жазылады.
бір жолды JSON құжаты, осылайша автоматтандырулар сұрауларды тікелей жібере алады
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Автоматтандырылған жіберулер

### 3.1 Torii РЕСТ режимі

`--submit-torii-url` плюс `--submit-token` немесе көрсетіңіз
`--submit-token-file` әрбір манифест жазбасын тікелей Torii ішіне басу үшін:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Көмекші сұрауға бір `POST /v1/sns/names` шығарады және оны тоқтатады
  бірінші HTTP қатесі. Жауаптар журнал жолына NDJSON ретінде қосылады
  жазбалар.
- `--poll-status` әр сұраудан кейін `/v1/sns/names/{namespace}/{literal}` қайта сұрайды
  жазба екенін растау үшін жіберу (`--poll-attempts` дейін, әдепкі 5)
  көрінетін. `--suffix-map` (`suffix_id` және `"suffix"` мәндерінің JSON) қамтамасыз етіңіз, осылайша
  құрал сұрауға арналған `{label}.{suffix}` литералдарын шығара алады.
- Баптау құрылғылары: `--submit-timeout`, `--poll-attempts` және `--poll-interval`.

### 3.2 iroha CLI режимі

Әрбір манифест жазбасын CLI арқылы бағыттау үшін екілік жолды беріңіз:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Контроллерлер `Account` жазбалары болуы керек (`controller_type.kind = "Account"`)
  себебі CLI қазіргі уақытта тек тіркелгіге негізделген контроллерлерді көрсетеді.
- Метадеректер мен басқару блоктары сұрау бойынша уақытша файлдарға жазылады және
  `iroha sns register --metadata-json ... --governance-json ...` мекенжайына жіберілді.
- CLI stdout және stderr плюс шығу кодтары журналға жазылады; нөлдік емес шығу кодтары тоқтатылады
  жүгіру.

Екі жіберу режимі тіркеушіні қарсы тексеру үшін бірге жұмыс істей алады (Torii және CLI).
орналастырулар немесе қалпына келтірулерді қайталау.

### 3.3 Беру түбіртектері

`--submission-log <path>` берілгенде, сценарий NDJSON жазбаларын қосады
түсіру:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Сәтті Torii жауаптары ішінен алынған құрылымдық өрістерді қамтиды
`NameRecordV1` немесе `RegisterNameResponseV1` (мысалы, `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) бақылау тақталары және басқару
есептер журналды еркін пішіндегі мәтінді тексермей-ақ талдай алады. Осы журналға тіркеңіз
қайталанатын дәлелдерге арналған манифестпен бірге тіркеуші билеттері.

## 4. Docs порталының шығарылымын автоматтандыру

CI және портал тапсырмалары оралатын `docs/portal/scripts/sns_bulk_release.sh` деп аталады
көмекші және `artifacts/sns/releases/<timestamp>/` астында артефактілерді сақтайды:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

Сценарий:

1. `registrations.manifest.json`, `registrations.ndjson` құрастырады және көшіреді
   бастапқы CSV шығарылым каталогына.
2. Манифестті Torii және/немесе CLI (конфигурацияланған кезде) арқылы жібереді, жазу
   `submissions.log` жоғарыдағы құрылымдық түбіртектері бар.
3. Шығарылымды сипаттайтын `summary.json` шығарады (жолдар, Torii URL, CLI жолы,
   уақыт белгісі) осылайша порталды автоматтандыру буманы артефакт қоймасына жүктеп сала алады.
4. Құрамында `metrics.prom` (`--metrics` арқылы қайта анықтау) шығарады
   Жалпы сұраулар үшін Prometheus форматындағы есептегіштер, жұрнақтарды бөлу,
   активтердің жалпы сомасы және ұсыну нәтижелері. Жиынтық JSON осы файлға сілтеме жасайды.

Жұмыс үрдістері шығарылым каталогын қазір бір артефакт ретінде мұрағаттайды
аудит үшін басқаруға қажеттінің барлығын қамтиды.

## 5. Телеметрия және бақылау тақталары

`sns_bulk_release.sh` арқылы жасалған көрсеткіштер файлы келесіні көрсетеді
сериясы:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom`-ті Prometheus бүйір вагонына беріңіз (мысалы, Promtail немесе
пакеттік импорттаушы) тіркеушілерді, басқарушыларды және басқару әріптестерін сәйкестендіру үшін
жаппай прогресс. Grafana тақтасы
`dashboards/grafana/sns_bulk_release.json` панельдермен бірдей деректерді көрсетеді
әр жұрнақ саны, төлем көлемі және жіберудің сәтті/сәтсіздігі коэффициенттері үшін.
Басқарма `release` арқылы сүзеді, осылайша аудиторлар бір CSV іске қосуына толықтай қарай алады.

## 6. Валидация және сәтсіздік режимдері

- **Белгілерді канонизациялау:** кірістер Python IDNA plus арқылы қалыпқа келтірілген
  кіші әріп және Norm v1 таңба сүзгілері. Жарамсыз белгілер кез келгеннен бұрын тез істен шығады
  желілік қоңыраулар.
- **Сандық қоршаулар:** суффикс идентификаторлары, қызмет мерзімі және баға ұсыныстары төмендеуі керек
  `u16` және `u8` шекараларында. Төлем өрістері ондық немесе он алтылық бүтін сандарды қабылдайды
  `i64::MAX` дейін.
- **Метадеректер немесе басқаруды талдау:** кірістірілген JSON тікелей талданады; файл
  сілтемелер CSV орнына қатысты шешіледі. Объекті емес метадеректер
  тексеру қатесін тудырады.
- **Контроллерлер:** бос ұяшықтар `--default-controllers` құрметіне ие. Нақты көрсетіңіз
  контроллер тізімдері (мысалы, `soraカタカナ...;soraカタカナ...`) иеленбейтіндерге өкілеттік беру кезінде
  актерлер.

Қателіктер мәтінмәндік жол нөмірлерімен хабарланады (мысалы
`error: row 12 term_years must be between 1 and 255`). Скрипт арқылы шығады
тексеру қателерінде `1` коды және CSV жолы жоқ кезде `2`.

## 7. Тестілеу және шығу

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` CSV талдауын қамтиды,
  NDJSON эмиссиясы, басқаруды орындау және CLI немесе Torii жіберу
  жолдар.
- Көмекші таза Python (қосымша тәуелділіктер жоқ) және кез келген жерде жұмыс істейді
  `python3` қол жетімді. Орындау тарихы CLI қатарында бақыланады
  репродуктивтіліктің негізгі репозиторийі.

Өндірістік іске қосулар үшін жасалған манифест пен NDJSON бумасын тіркеңіз
Тіркеуші билеті, сондықтан басқарушылар жіберілген нақты жүктерді қайталай алады
Torii дейін.