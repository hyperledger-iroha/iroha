---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-05T09:28:11.908615+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Quickstart

Был практик етәксе аша үтә детерминистик SF-1 chunker профиле,
асыҡ ҡул ҡуйыу, һәм күп провайдерҙар ағымы ағымы, улар нигеҙендә I18NT0000000002X
һаҡлау торбаһы. Парлы уны [массор торба тәрән һыу инеү] (manifest-pipeline.md)
дизайн яҙмалары һәм CLI флагы һылтанма материалы өсөн.

## Алдан шарттар

- Руст инструменттар слет (`rustup update`), эш урыны урындағы клонланған.
- Һорау: [OpenSSL-генерацияланған Ed25519 клавиатура] (I18NU0000010X)
  ҡул ҡуйыу өсөн манифесттар.
- 18-се һанлы ≥ ≥ Nod

`export RUST_LOG=info` комплекты, шул уҡ ваҡытта эксперимент үткәреү өсөн ярҙамсы CLI хәбәрҙәре.

## 1. Детерминистик ҡорамалдарҙы яңыртыу

Канонлы SF-1 векторҙарын регенерациялау. Команда шулай уҡ ҡул ҡуйҙы.
асыҡ конверттар ҡасан I18NI000000014X тәьмин ителә; ҡулланыу I18NI000000015X
урындағы үҫеш ваҡытында ғына.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Сығыштар:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- I18NI000000018X (әгәр ҡул ҡуйылған)
- I18NI000000019X

## 2. Фейым йөк һәм планды тикшерергә

`sorafs_chunker` ҡулланыу өсөн үҙ ирке менән файл йәки архив өлөшө:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Төп баҫыуҙар:

- `profile` / `break_mask` – `sorafs.sf1@1.0.0` параметрҙарын раҫлай.
- `chunks[]` – офсет, оҙонлоҡтар, һәм өлөш BLAKE3 үҙләштереүгә заказ бирелгән.

Ҙурыраҡ ҡорамалдар өсөн, пропускной способный регрессия йүгерә, потоковый һәм стриминг һәм
партия киҫәкләү синхронлаштырыуҙа ҡала:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Төҙөү һәм ҡул ҡуйырға манифест

Ҡайһы бер өлөшө планын урап, псевдоним, һәм идара итеү ҡултамғалары ҡулланыу өсөн манифест
`sorafs-manifest-stub`. Түбәндәге командала бер файл файҙалы йөк күрһәтелә; үтергә
каталог юлы ағас ҡаплау өсөн (CLI уны лексикографик йөрөй).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

I18NI000000026X өсөн:

- I18NI000000027X – SHA3 офсеттар/оҙонлоғо, тап килә.
  өлөшсә ҡоролмалары.
- `manifest.manifest_blake3` – BLAKE3 дисвертта ҡул ҡуйылған.
- `chunk_fetch_specs[]` – оркестрсылар өсөн инструкция алыуға заказ бирҙе.

Ысын ҡултамғалар менән тәьмин итергә әҙер булғанда, I18NI000000030X һәм I18NI000000031X өҫтәргә
аргументтары. Команда һәр Ed25519 ҡултамғаһын раҫлай, яҙғансы,
конверт.

## 4. Күп провайдер эҙләү моделләштереү

Ҡулланыу өсөн разработчик CLI алыу өсөн реплей өлөшө планы ҡаршы бер йәки бер нисә .
провайдерҙар. Был CI төтөн анализдары һәм оркестрҙа прототиплаштырыу өсөн идеаль.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Ҡабул итеүҙәр:

- `payload_digest_hex` асыҡ отчетҡа тап килергә тейеш.
- `provider_reports[]` ер өҫтө уңышы/уңышһыҙлыҡтар һаны бер провайдер.
- нуль булмаған I18NI000000034X айырып күрһәтә артҡа баҫым көйләүҙәре.
- Pass I18NI0000000035X, эшләү өсөн планлаштырылған провайдерҙар һанын сикләү өсөн
  һәм CI моделләштереүҙәрен төп кандидаттарға йүнәлтелгән тотоу.
- I18NI0000000036X Xunk per-punk ретия иҫәбен үтәй (3) шулай итеп, һеҙ
  оркестратор регрессияларын тиҙ генә инъекциялау етешһеҙлектәрен өҫтөн ҡуя ала.

Өҫтәү I18NI0000000037X һәм I18NI0000000038X
тиҙ ҡасан реконструкцияланған файҙалы йөк тайпыла манифест.

## 5. Киләһе аҙымдар

- **Идара итеү интеграцияһы** – торба асыҡ һеңдерергә һәм
  I18NI0000000039X совет эш ағымына, шулай итеп, булавка реестры ала
  реклама доступность.
- **Зинчастотный һөйләшеү** – консультация [I18NI000000040X] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  яңы профилдәрҙе теркәгәнсе. Автоматлаштырыу канон тотҡаларын өҫтөн күрергә тейеш
  (`namespace.name@semver`) һанлы идентификаторҙар өҫтөндә.
- **CI автоматлаштырыу** – өҫтәге командаларҙы өҫтәп, торбалар сығарыу өсөн шулай docs,
  ҡоролмалары, һәм артефакттар детерминистик манифесттар менән бер рәттән баҫтырып сығара
  метамағлүмәттәр.