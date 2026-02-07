---
lang: kk
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-05T09:28:11.997191+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Жылдам бастау

Бұл практикалық нұсқаулық детерминирленген SF-1 chunker профилі арқылы жүреді,
манифестке қол қою және SoraFS негізіндегі көп провайдерлерді алу ағыны
сақтау құбыры. Оны [манифест құбыр желісінің терең сүңгуімен](manifest-pipeline.md) жұптаңыз
дизайн жазбалары мен CLI жалауының анықтамалық материалы үшін.

## Алғышарттар

- Rust құралдар тізбегі (`rustup update`), жұмыс кеңістігі жергілікті түрде клондалған.
- Қосымша: [OpenSSL арқылы жасалған Ed25519 пернелер жұбы](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  манифесттерге қол қою үшін.
- Қосымша: Docusaurus порталын алдын ала қарауды жоспарласаңыз, Node.js ≥ 18.

Пайдалы CLI хабарларын шығару үшін тәжірибе жасау кезінде `export RUST_LOG=info` орнатыңыз.

## 1. Детерминирленген бекітпелерді жаңартыңыз

Канондық SF-1 түйіндеу векторларын қалпына келтіріңіз. Пәрмен де қолтаңбаны шығарады
`--signing-key` берілген кездегі манифест конверттері; `--allow-unsigned` пайдаланыңыз
жергілікті даму кезінде ғана.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Шығарулар:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (қол қойылған болса)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Пайдалы жүкті бөліп, жоспарды тексеріңіз

Ерікті файлды немесе мұрағатты бөлшектеу үшін `sorafs_chunker` пайдаланыңыз:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Негізгі өрістер:

- `profile` / `break_mask` – `sorafs.sf1@1.0.0` параметрлерін растайды.
- `chunks[]` – реттелген ығысулар, ұзындықтар және BLAKE3 дайджесттері.

Үлкенірек құрылғылар үшін ағынды және ағынды қамтамасыз ету үшін сынаққа негізделген регрессияны іске қосыңыз
пакеттік жинақтау синхронды күйде қалады:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Манифестті құрастырыңыз және оған қол қойыңыз

Бөлшек жоспарын, бүркеншік аттарды және басқару қолтаңбаларын пайдалану арқылы манифестке ораңыз
`sorafs-manifest-stub`. Төмендегі пәрмен бір файлдық пайдалы жүктемені көрсетеді; өту
ағашты бумалау үшін каталог жолы (CLI оны лексикографиялық түрде жүргізеді).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` шолу:

- `chunking.chunk_digest_sha3_256` – SHA3 ығысулар/ұзындықтар дайджесті, сәйкес келеді
  шанкер арматура.
- `manifest.manifest_blake3` – манифест конвертінде қол қойылған BLAKE3 дайджест.
- `chunk_fetch_specs[]` – оркестрлер үшін тапсырысты алу нұсқаулары.

Нақты қолтаңбаларды беруге дайын болғанда, `--signing-key` және `--signer` қосыңыз
аргументтер. Пәрмен жазу алдында әрбір Ed25519 қолтаңбасын тексереді
конверт.

## 4. Көп провайдерлерді іздеуді имитациялаңыз

Бөлшек жоспарын біреуіне немесе бірнешеуіне қарсы қайта ойнату үшін әзірлеушінің CLI алуын пайдаланыңыз
провайдерлер. Бұл CI түтін сынақтары мен оркестрдің прототипін жасау үшін өте қолайлы.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Бекітулер:

- `payload_digest_hex` манифест есебіне сәйкес келуі керек.
- `provider_reports[]` әр провайдердің сәтті/сәтсіздігін көрсетеді.
- Нөлдік емес `chunk_retry_total` кері қысымды реттеулерді ерекшелейді.
- Іске қосуға жоспарланған провайдерлердің санын шектеу үшін `--max-peers=<n>` өтіңіз
  және CI модельдеулерін негізгі үміткерлерге бағыттаңыз.
- `--retry-budget=<n>` әдепкі әр бөлікті қайталау санын (3) қайта анықтайды, осылайша сіз
  сәтсіздіктерді енгізу кезінде оркестр регрессияларын жылдамырақ көрсете алады.

Сәтсіз болу үшін `--expect-payload-digest=<hex>` және `--expect-payload-len=<bytes>` қосыңыз
қалпына келтірілген пайдалы жүктеме манифесттен ауытқыған кезде жылдам.

## 5. Келесі қадамдар

- **Басқару интеграциясы** – манифест дайджесті және құбыры
  `manifest_signatures.json` кеңес жұмыс процесіне енгізеді, осылайша Pin тізілімі мүмкін
  қолжетімділігін жарнамалау.
- **Тіркеу келіссөздері** – кеңес алыңыз [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  жаңа профильдерді тіркеуден бұрын. Автоматтандыру канондық тұтқаларды таңдауы керек
  (`namespace.name@semver`) сандық идентификаторлар үстінде.
- **CI automation** – жоғарыдағы пәрмендерді құжаттар үшін құбырларды шығару үшін қосыңыз,
  қондырғылар мен артефактілер қол қойылғанымен қатар детерминистік манифесттерді жариялайды
  метадеректер.