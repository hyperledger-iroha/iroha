---
lang: ba
direction: ltr
source: docs/source/ministry/emergency_canon_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afd5db8a761f8cc56dcd8f67f047f06b45c3211738fedb6dfff326d84b4e8a68
source_last_modified: "2026-01-22T14:45:02.098548+00:00"
translation_last_reviewed: 2026-02-07
title: Emergency Canon & TTL Policy
summary: Reference implementation notes for roadmap item MINFO-6a covering denylist tiers, TTL enforcement, and governance evidence requirements.
translator: machine-google-reviewed
---

# Ғәҙәттән тыш канон & ТТЛ сәйәсәте (МИНФО-6а)

Юл картаһы һылтанма: **МИНФО-6а — Ғәҙәттән тыш хәлдәр канон & TTL сәйәсәте**.

Был документ билдәләй, денилист ярус ҡағиҙәләре, TTL үтәү, һәм идара итеү бурыстары, хәҙер Torii һәм CLI ташый. Операторҙар был ҡағиҙәләрҙе яңы яҙмалар баҫтырыр алдынан йәки ғәҙәттән тыш хәлдәр канондарын саҡырыр алдынан үтәргә тейеш.

## Тиер аңлатмалары

| Тиер | Ғәҙәттәгесә TTL | Тәҙрәгеҙҙе ҡарағыҙ | Талаптар |
|-----|-------------|---------------|---------------|
| Стандарт | 180 көн (`torii.sorafs_gateway.denylist.standard_ttl`) | н/а | `issued_at` X-сығы булырға тейеш. `expires_at` X-ҙы төшөрөп ҡалдырырға мөмкин; Torii `issued_at + standard_ttl`-ҡа тиклем ғәҙәттәгесә, оҙонораҡ тәҙрәләрҙе кире ҡаға. |
| Ғәҙәттән тыш хәлдәр | 30 көн (`torii.sorafs_gateway.denylist.emergency_ttl`) | 7 көн (`torii.sorafs_gateway.denylist.emergency_review_window`) | Талап итә, буш булмаған `emergency_canon` ярлыҡ һылтанма алдан раҫланған канон (мәҫәлән, `csam-hotline`). `issued_at` + `expires_at` 30-көнлөк тәҙрә эсенә ергә төшөргә тейеш, ә тикшерелгән дәлилдәр автогенерацияланған срокты (`issued_at + review_window`) килтерергә тейеш. |
| Даими | Срогы юҡ | н/а | Ғәҙәттән тыш хәлдәр менән идара итеү ҡарарҙары өсөн һаҡлана. Яҙмалар буш булмаған `governance_reference` (тауыш id, манифест хеш һ.б.) килтерергә тейеш. `expires_at` X-тан кире ҡағыла. |

Ғәҙәттәгесә, `torii.sorafs_gateway.denylist.*` һәм `iroha_cli` аша конфигурациялана, Torii тиклем дөрөҫ булмаған яҙмаларҙы тотоу өсөн сиктәрҙе көҙгөләй, файлды ҡабаттан тейәү.

## Эш ағымы

**Әҙерләнгән метамағлүмәттәр:** `policy_tier`, `issued_at`, `expires_at` (ҡабул ителгәндә), һәм `emergency_canon`/`governance_reference` һәр JSON яҙмаһы эсендә үҙ эсенә ала. (`docs/source/sorafs_gateway_denylist_sample.json`).
2. **Урындағы кимәлдә раҫлау:** эшләй `iroha app sorafs gateway lint-denylist --path <denylist.json>` шулай CLI ярусҡа махсус TTLs һәм кәрәкле яландарҙы үтәй, файл йөкмәтелгән йәки ҡыҫтырылғанға тиклем.
3. **Башҡа дәлилдәр:** канон id йәки идара итеү һылтанмаһы беркетелгән цитироваться инеү GAR эше өйөмө (көнсығыш пакет, референдум минут, һ.б.) шулай аудиторҙар ҡарарҙы эҙләй ала.
4. **Авариялы ашығыс яҙмаларҙы ҡарап сығыу:** 30 көн эсендә авто-ваҡытлыса авария канондары. Операторҙар 7 көнлөк тәҙрә эсендә фактонан һуңғы тикшерелеүҙе тамамларға һәм һөҙөмтәне министрлыҡ трекерында/SoraFS дәлилдәр магазинында теркәргә тейеш.
5. **Ҡабаттан Torii:** бер тапҡыр раҫланған, `torii.sorafs_gateway.denylist.path` аша инвалид юлды йәйелдерергә һәм перезапускать/перемость Torii; йөрөү ваҡыты яҙмаларҙы ҡабул иткәнсе шул уҡ сиктәрҙе үтәй.

## Ҡораллы һәм Һылтанмалар

- `sorafs::gateway::denylist` (`crates/iroha_torii/src/sorafs/gateway/denylist.rs`) һәм тейәүсе йәшәй, хәҙер сәйәсәтте үтәмәү `torii.sorafs_gateway.denylist.*` индереүҙәрен анализлауҙа ярус метамағлүмәттәрен ҡуллана.
- CLI валидацияһы `GatewayDenylistRecord::validate` (`crates/iroha_cli/src/commands/sorafs.rs`X) эсендә эшләү ваҡыты семантикаһын көҙгөләй. Ҡайһы бер осраҡта, ҡасан TTLs конфигурацияланған тәҙрәнән артып китә йәки мотлаҡ канон/идара итеү һылтанмалары юҡ.
- Конфигурация ручкалары `torii.sorafs_gateway.denylist` (`crates/iroha_config/src/parameters/{defaults,actual.rs,user.rs}`) буйынса билдәләнә, шуға күрә операторҙар TTL-ды/ҡабатлау сроктарын үҙгәртә ала, әгәр идара итеү төрлө сиктәрҙе раҫлаһа.
- Йәмәғәт өлгөһө danilist (`docs/source/sorafs_gateway_denylist_sample.json`) хәҙер өс ярусты ла күрһәтә һәм яңы яҙмалар өсөн канон шаблон булараҡ ҡулланырға тейеш.Был ҡоршауҙар юл картаһы әйберен ҡәнәғәтләндерә **МИНФО-6а** ғәҙәттән тыш хәлдәр канон исемлеген кодификациялау, сикләнмәгән ТТЛ-дарҙы иҫкәртергә һәм даими блоктар өсөн асыҡ идара итеү дәлилдәрен мәжбүр итә.

## Реестр автоматлаштырыу & дәлилдәр экспорты

Ғәҙәттән тыш канон раҫлауҙары детерминистик реестр снимок һәм а етештерергә тейеш
дифф өйөмө алдынан Torii йөкләмәләр дениист. Ҡулланылған инструменттар аҫтында .
`xtask/src/sorafs.rs` плюс CI йүгән `ci/check_sorafs_gateway_denylist.sh`
бөтә эш ағымын ҡаплай.

### Канональ өйөм быуыны

1. Стаж сеймал яҙмалары (ғәҙәттә, идара итеү ҡаралған файл) эштә
   каталогы.
2. Canonicalize һәм JSON аша мөһөрләү аша:
   ```bash
   cargo xtask sorafs-gateway denylist pack \
     --input path/to/denylist.json \
     --out artifacts/ministry/denylist_registry/$(date +%Y%m%dT%H%M%SZ) \
     --label ministry-emergency \
     --force
   ```
   Команда ҡул ҡуйылған-дуҫ Norito өйөмөн сығара.
   конверт, һәм Меркл-тамыр текст файлы көтөлгән идара итеү рецензенттар.
   `artifacts/ministry/denylist_registry/` буйынса каталогты һаҡлау (йәки һеҙҙең
   һайланған дәлилдәр биҙрә) шулай `scripts/ministry/transparency_release.py` мөмкин
   һуңыраҡ `--artifact denylist_bundle=<path>` менән алырға.
3. Һаҡлау генерацияланған `checksums.sha256` менән бергә өйөм уны этәрергә
   тиклем SoraFS/GAR. CI’s `ci/check_sorafs_gateway_denylist.sh` күнекмәләр шул уҡ
   `pack` ярҙамсыһы ҡаршы өлгө инвалидисты гарантиялау өсөн инструменталь эштәр эшләй
   һәр релиз.

### Айырма + аудит өйөмө

1. Яңы өйөмд менән сағыштырырға ҡаршы алдағы производство снимок ҡулланып
   xtask diff ярҙамсы:
   ```bash
   cargo xtask sorafs-gateway denylist diff \
     --old artifacts/ministry/denylist_registry/2026-05-01/denylist_old.json \
     --new artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --report-json artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
   JSON отчетында бөтә өҫтәүҙәр/алып ташлау һәм көҙгө дәлилдәр .
   структураһы `MinistryDenylistChangeV1` тарафынан ҡулланыла (эшкә һылтанма буйынса
   `docs/source/sorafs_gateway_self_cert.md` һәм үтәү планы).
2. Беркетергә `denylist_diff.json` һәр канон үтенесе (ул иҫбатлай, күпме
   яҙмалар ҡағыла, ниндәй ярус үҙгәрҙе, һәм ниндәй дәлилдәр хеш карталары
   канон өйөм).
3. Ҡасан диффтар автоматик рәүештә генерацияланған (CI йәки торбалар сығарыу), экспорт
   `denylist_diff.json` юл аша `--artifact denylist_diff=<path>` шулай итеп, шулай
   асыҡлыҡ күрһәтә, уны санитария метрикаһы менән бер рәттән. Шул уҡ CI
   ярҙамсы ҡабул итә `--evidence-out <path>`, ул CLI резюме аҙымы һәм
   һөҙөмтәлә барлыҡҡа килгән JSON-ды һуңыраҡ баҫтырыу өсөн һоралған урынға күсерә.

### Баҫма & асыҡлыҡ1. Пакетты ташларға + дифф артефакттары квартал үтә күренмәлелек каталогы
   (`artifacts/ministry/transparency/<YYYY-Q>/denylist/`). Асыҡлыҡ
   ярҙам итеүсе уларҙы индерә ала, һуңынан уларҙы индерә ала:
   ```bash
   scripts/ministry/transparency_release.py \
     --quarter 2026-Q3 \
     --output-dir artifacts/ministry/transparency/2026-Q3 \
     --sanitized artifacts/ministry/transparency/2026-Q3/sanitized_metrics.json \
     --dp-report artifacts/ministry/transparency/2026-Q3/dp_report.json \
     --artifact denylist_bundle=artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --artifact denylist_diff=artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
2. Һылтанма генерацияланған өйөм/айырым квартал отчетында .
   (`docs/source/ministry/reports/<YYYY-Q>.md`) һәм шул уҡ юлдарҙы беркетергә
   GAR тауыш биреүҙең пакеты, шулай итеп, аудиторҙар реплей мөмкин дәлилдәр эҙҙәре 2019 йыл.
   эске CI. `ци/тикшереү_sorafs_gateaway_denilist.sh --дәлил-аут \.
   артефакттар/министрлыҡ/инстанист_регист//dinilist_evidence.json` хәҙер
   башҡарыу пакет/айырым/мәҡәл ҡоро-йүгереп (һөйләү `iroha_cli ҡушымта соралар шлюз
   дәлилдәр ` капот аҫтында) шулай автоматлаштырыу резюме менән бергә һаҡлана ала.
   канон өйөмдәре.
3. Баҫылып сыҡҡандан һуң, идара итеү файҙалы йөктө якорь аша .
   `cargo xtask ministry-transparency anchor`X (автоматик рәүештә тип атала.
   `transparency_release.py`, ҡасан `--governance-dir` X) шулай итеп, шулай
   денистист реестры дисциплинаһы үтә күренмәлелек менән бер үк ДАГ ағасында барлыҡҡа килә
   ебәрергә.

Был процесты үтәп, “реестр автоматлаштырыу һәм дәлилдәр экспорты” ябыла.
18NI000000069X-та саҡырылған айырма һәм һәр авария канонын тәьмин итә
ҡарар ҡабатланған артефакттар менән килә, JSON diffs, һәм асыҡлыҡ журналы
яҙмалар.

### TTL & Канон дәлилдәре ярҙамсыһы

Һуң өйөм/дифф пар етештерелә, йүгерергә CLI дәлилдәр ярҙамсыһы тотоу өсөн
TTL резюме һәм ғәҙәттән тыш хәлдәрҙе тикшергән сроктар, идара итеү талап итә:

```bash
iroha app sorafs gateway evidence \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --out artifacts/ministry/denylist_registry/2026-05-14/denylist_evidence.json \
  --label csam-canon-2026-05
```

Команда сығанаҡты хеш JSON, һәр яҙма раҫлай, һәм компакт сығара
резюме составында:

- `kind` һәм иң тәүге/һуңғыһы менән сәйәсәт ярусына дөйөм яҙмалар
  ваҡыт маркалары күҙәтелә.
- `emergency_reviews[]` исемлеген иҫәпләп, һәр авария каноны менән уның
  дескриптор, һөҙөмтәле срогы, макс рөхсәт TTL, һәм иҫәпләнгән
  `review_due_by` срогы.

Ҡушымта `denylist_evidence.json` менән бергә тығыҙ өйөм/дифф шулай аудиторҙар ала
раҫлау TTL үтәү, ҡабаттан йүгермәйенсә, CLI. CI эш урындары, улар инде генерация
өйөмдәр ярҙамсыға мөрәжәғәт итә ала һәм дәлилдәр артефактын баҫтырып сығара ала (мәҫәлән,
шылтыратыу `ci/check_sorafs_gateway_denylist.sh --evidence-out <path>`), тәьмин итеү
һәр канон эҙмә-эҙлекле резюме менән ерҙәрҙе һорай.

### Меркл реестры дәлилдәре

MINFO-6-ла индерелгән Merkle реестры операторҙарҙан баҫтырыуҙы талап итә.
тамыр һәм пер-перетатив дәлилдәр менән бер рәттән ТТЛ резюмеһы. Шунда уҡ йүгергәндән һуң
дәлилдәр ярҙамсыһы, Меркл артефакттарын ҡулға ала:

```bash
iroha app sorafs gateway merkle snapshot \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.to

iroha app sorafs gateway merkle proof \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --index 12 \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.to
```Снимок JSON BLAKE3 Меркл тамырын, япраҡ иҫәбен һәм һәр береһен теркәй,
дескриптор/хэш пары шулай GAR тауыштары аныҡ ағасҡа һылтанма яһай ала, тип хешировать.
`--norito-out` менән тәьмин итеү `.to` артефактын JSON менән бер рәттән һаҡлай, рөхсәт итеү
шлюздар туранан-тура Norito аша туранан-тура реестр яҙмаларын ҡырҡып ташламай
stdout. Torii йүнәлеш биттәрен сығара һәм бер туғандар өсөн теләһә ниндәй хеш
нуль нигеҙендә инеү индексы, уны ябай итеү өсөн беркетергә инклюзия иҫбатлау өсөн һәр .
ғәҙәттән тыш канон цитироваться GAR иҫтәлекле-был факультатив Norito күсермәһе һаҡлай дәлил
әҙер легаль бүленә. JSON һәм Norito артефакттарын һаҡлағыҙ.
ТТЛ резюме һәм дифф өйөмө шулай үтә күренмәлелек релиздары һәм идара итеү
якорь бер үк тамырға һылтанма яһай.