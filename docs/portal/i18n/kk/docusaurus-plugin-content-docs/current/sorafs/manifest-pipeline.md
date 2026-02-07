---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Чунинг → Манифест құбыры

Жылдам іске қосудың бұл серіктесі өңделмеген күйге айналатын соңғы құбырды қадағалайды
SoraFS Pin тізіліміне қолайлы Norito манифестеріне байттар. Мазмұны
бейімделген [`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
канондық спецификация мен өзгертулер журналы үшін сол құжатты қараңыз.

## 1. Детерминирленген кесінді

SoraFS SF-1 (`sorafs.sf1@1.0.0`) профилін пайдаланады: FastCDC шабыттандырылған прокат
64КБ ең аз бөлік өлшемі бар хэш, 256КБ мақсатты, максимум 512КБ және
`0x0000ffff` сыну маскасы. Профиль тіркелген
`sorafs_manifest::chunker_registry`.

### Тот басатын көмекшілер

- `sorafs_car::CarBuildPlan::single_file` – Бөлшектердің ығысуын, ұзындығын және
  BLAKE3 CAR метадеректерін дайындау кезінде дайджест жасайды.
- `sorafs_car::ChunkStore` – Пайдалы жүктемелерді тасымалдайды, метадеректерді сақтайды және
  64KiB / 4KiB Retrievability Proof-of-Retrievability (PoR) сынама ағашын шығарады.
- `sorafs_chunker::chunk_bytes_with_digests` – Екі CLI артындағы кітапхана көмекшісі.

### CLI құралдары

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON құрамында реттелген ығысулар, ұзындықтар және бөлік дайджесттері бар. табыңыз
манифесттерді немесе оркестрді алу спецификацияларын құру кезінде жоспарлаңыз.

### PoR куәгерлері

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` және
`--por-sample=<count>`, сондықтан аудиторлар детерминирленген куәгерлер жиынын сұрай алады. Жұп
JSON жазу үшін `--por-proof-out` немесе `--por-sample-out` бар жалаушалар.

## 2. Манифестті орау

`ManifestBuilder` кесінді метадеректерін басқару тіркемелерімен біріктіреді:

- Root CID (dag-cbor) және CAR міндеттемелері.
- Бүркеншік аттың дәлелдері және провайдердің мүмкіндігі туралы шағымдар.
- Кеңес қолтаңбалары және қосымша метадеректер (мысалы, құрастыру идентификаторлары).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Маңызды нәтижелер:

- `payload.manifest` – Norito кодталған манифест байттары.
- `payload.report.json` – Адам/автомат бойынша оқылатын қысқаша, соның ішінде
  `chunk_fetch_specs`, `payload_digest_hex`, CAR дайджесттері және бүркеншік ат метадеректері.
- `payload.manifest_signatures.json` – BLAKE3 манифесті бар конверт
  дайджест, бөлшектік жоспар SHA3 дайджест және сұрыпталған Ed25519 қолтаңбалары.

Сыртқы конверттермен қамтамасыз етілген хатқалталарды тексеру үшін `--manifest-signatures-in` пайдаланыңыз
қол қоюшылар оларды кері жазбастан бұрын және `--chunker-profile-id` немесе
Тізбе таңдауын құлыптау үшін `--chunker-profile=<handle>`.

## 3. Жариялау және бекіту

1. **Басқаруды ұсыну** – Манифест дайджест пен қолтаңбаны қамтамасыз етіңіз
   конвертті кеңеске жіберіңіз, осылайша түйреуішті қабылдауға болады. Сыртқы аудиторлар қажет
   манифест дайджестімен қатар SHA3 дайджестінің бөлшек жоспарын сақтаңыз.
2. ** Пайдалы жүктемелерді бекіту** – сілтеме жасалған CAR мұрағатын (және қосымша CAR индексін) жүктеп салу
   PIN тізілімінің манифестінде. Манифест пен CAR ортақ екеніне көз жеткізіңіз
   бірдей түбірлік CID.
3. **Телеметрияны жазу** – JSON есебін, PoR куәгерлерін және кез келген алуды сақтау
   шығару артефактілеріндегі көрсеткіштер. Бұл жазбалар оператордың бақылау тақталарын және
   үлкен пайдалы жүктемелерді жүктеп алмай, мәселелерді қайта жасауға көмектеседі.

## 4. Көп провайдерді алу симуляциясы

`жүк тасымалдау -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` әр провайдерге параллелизмді арттырады (жоғарыдағы `#4`).
- `@<weight>` жоспарлау қиғаштығын баптайды; әдепкі бойынша 1.
- `--max-peers=<n>` іске қосу үшін жоспарланған провайдерлердің санын шектейді
  ашылу қалағанынан көп кандидаттар береді.
- `--expect-payload-digest` және `--expect-payload-len` үнсіздіктен қорғайды
  сыбайлас жемқорлық.
- `--provider-advert=name=advert.to` провайдердің мүмкіндіктерін бұрын тексереді
  оларды модельдеуде пайдалану.
- `--retry-budget=<n>` әрбір бөлікті қайталау санын қайта анықтайды (әдепкі: 3), сондықтан CI
  сәтсіздік сценарийлерін сынау кезінде регрессияларды жылдамырақ көрсете алады.

`fetch_report.json` жинақталған көрсеткіштерді көрсетеді (`chunk_retry_total`,
`provider_failure_rate`, т.б.) CI бекітулері мен бақылауға жарамды.

## 5. Тізілімді жаңарту және басқару

Жаңа chunker профильдерін ұсынғанда:

1. `sorafs_manifest::chunker_registry_data` ішіндегі дескриптордың авторы.
2. `docs/source/sorafs/chunker_registry.md` және қатысты жарғыларды жаңарту.
3. Арматураларды қалпына келтіріңіз (`export_vectors`) және қол қойылған манифесттерді түсіріңіз.
4. Жарғыға сәйкестік туралы есепті басқару қолдарымен тапсырыңыз.

Автоматтандыру канондық тұтқаларды (`namespace.name@semver`) және құлауды таңдауы керек