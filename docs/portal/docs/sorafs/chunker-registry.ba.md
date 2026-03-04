---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d43c9ac18ba6b9e4e7941325b2ebdec672b01627747e3272927557faf82957af
source_last_modified: "2026-01-22T14:35:36.798331+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-registry
title: SoraFS Chunker Profile Registry
sidebar_label: Chunker Registry
description: Profile IDs, parameters, and negotiation plan for the SoraFS chunker registry.
translator: machine-google-reviewed
---

:::иҫкәртергә канонлы сығанаҡ
::: 1990 й.

## I18NT000000000X Чанкер профиле реестры (SF-2a)

SoraFS стека һөйләшеүҙәр аша chunking тәртибе аша бәләкәй, исемдәр киңлеге реестр.
Һәр профиль детерминистик CDC параметрҙарын, сетермик метамағлүмәттәрен һәм
көтөлгән матдә/мультикадек ҡулланылған манифест һәм CAR архивтарында.

Профиль авторҙары кәңәшләшергә тейеш
[`docs/source/sorafs/chunker_profile_authoring.md`] (./chunker-profile-authoring.md)
өсөн кәрәкле метамағлүмәттәр, раҫлау тикшерелгән исемлеге, һәм тәҡдим шаблонына тиклем .
яңы яҙмалар тапшырыу. Бер тапҡыр идара итеү үҙгәрештәрҙе раҫланы, эйәреп,
[реестр роллут тикшерелгән исемлек](I18NU000000011X) һәм
[съемник манифест плейбук] (./staging-manifest-playbook) пропагандалау өсөн
стадиялау һәм етештереү аша ҡорамалдар.

### Профилдәр

| Исем киңлеге | Исем | SemVer | Профиль идентификаторы | Мин (байт) | Маҡсат (байтс) | Макс (байте) | Битлек өҙөлә | Мультихаш | псевдоним | Иҫкәрмәләр |
|---------|-------|------------------------------------------------------------------------------------. -----|------------|-------------|--------------------|----------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0"]` | SF-1 ҡоролмаларында ҡулланылған канон профиле |

Реестр `sorafs_manifest::chunker_registry` («`chunker_registry_charter.md`] (./chunker-registry-charter.md) ярҙамында идара иткән кодта йәшәй). Һәр яҙма
I18NI000000024X менән::

* `namespace` – логик төркөмләү менән бәйле профилдәр (мәҫәлән, `sorafs`).
* `name` – кеше уҡый торған профиль ярлығы (I18NI000000028X, `sf1-fast`, ...).
* I18NI000000030X – параметр йыйылмаһы өсөн семантик версия телмәре.
* I18NI000000031X – ысын I18NI000000032X (мин/маҡсат/макс/битлек).
* I18NI000000033X – мультихаш ҡулланылғанда ҡулланылған өлөшләтә һеңдерелгән (`0x1f`
  SoraFS өсөн ғәҙәттәгесә).

I18NI000000035X аша манифест сериализациялана. Структурала яҙылған.
сей CDC менән бер рәттән реестр метамағлүмәттәре (исем киңлеге, исеме, сембель)
параметрҙары һәм өҫтә күрһәтелгән псевдонимдар исемлеге. Ҡулланыусылар тәүҙә тырышырға тейеш.
теркәү эҙләү I18NI00000000036X һәм кире төшөп рәт параметрҙары ҡасан
билдәһеҙ идентификаторҙар барлыҡҡа килә. Реестр устав ҡағиҙәләре канон тотҡаһы талап итә
(I18NI000000037X) I18NI0000000038X-та беренсе яҙма булып тора.

Реестрҙы инструменттарҙан тикшерергә, ярҙамсы CLI йүгерергә:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]

All of the CLI flags that write JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) accept `-` as the path, which streams the payload to stdout instead of
creating a file. This makes it easy to pipe the data into tooling while still keeping the
default behaviour of printing the main report.

To inspect a specific PoR witness, provide chunk/segment/leaf indices and
optionally persist the proof to disk:

```
$ йөк йүгерә -p sorafs_manifest --бин соралар_манифест_чунк_магазин -- ./docs.tar \
    --пор-иҫбатлау=0:0 --пор-иҫбатлаусы=япраҡ.;
```

You can select a profile by numeric id (`--profile-id=1`) or by registry handle
(`--profile=sorafs.sf1@1.0.0`); the handle form is convenient for scripts that
thread namespace/name/semver directly from governance metadata.

Use `--promote-profile=<handle>` to emit a JSON metadata block (including all
registered aliases) that can be pasted into `chunker_registry_data.rs` when
promoting a new default profile:

```
$ йөк йүгерә -p sorafs_manifest --бин сорафтар_манифест_чунк_магазин -- \.
    --протогы-профиль=сорафтар.sf1@1.0.0.
```

The main report (and optional proof file) include the root digest, the sampled
leaf bytes (hex-encoded), and the segment/chunk sibling digests so verifiers can
rehash the 64 KiB/4 KiB layers against the `por_root_hex` value.

To validate an existing proof against a payload, pass the path via
`--por-proof-verify` (the CLI adds `"por_proof_verified": true` when the witness
matches the computed root):

```
$ йөк йүгерә -p sorafs_manifest --бин соралар_манифест_чунк_магазин -- ./docs.tar \
    --пор-иҫбатлаусы=япраҡ.иҡтисад.json
```

For batch sampling, use `--por-sample=<count>` and optionally provide a seed/
output path. The CLI guarantees deterministic ordering (`splitmix64` seeded)
and will transparently truncate when the request exceeds the available leaves:

```
$ йөк йүгерә -p sorafs_manifest --бин соралар_манифест_чунк_магазин -- ./docs.tar \
    --пор-өлгө=8 --пор-өлгө-орлоҡ=0xfeedface --пор-өлгө-аут=пор.үлсәмдәр.json

Манифест стаб көҙгө шул уҡ мәғлүмәттәрҙе, был уңайлы, ҡасан сценарий I18NI000000039X һайлау торбалар. Ике өлөшө магазин CLIs шулай уҡ канон ручка формаһын ҡабул итә (`--profile=sorafs.sf1@1.0.0`), шуға күрә төҙөү сценарийҙары ҡотолорға мөмкин ҡаты кодлау һанлы идентификаторҙар:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "profile_id": 1,
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

I18NI0000000041X яланы (I18NI000000042X) матчта, нимә CLIs ҡабул итә аша .
`--profile=…`, был туранан-тура автоматлаштырыуға күсерергә хәүефһеҙ.

### һөйләшеүҙәр алып барыусыһы

Ҡапҡалар һәм клиенттар реклама ярҙам профилдәре аша провайдер реклама:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicit via registry)
    capabilities: [...]
}
```

Күп сығанаҡлы өлөш планлаштырыу `range` мөмкинлеге аша иғлан ителә. 1990 й.
CLI уны I18NI0000000045X менән ҡабул итә, унда опциональ һанлы .
суффикс провайдер өҫтөнлөк диапазоны-фетч конкурентлыҡ кодлай (мәҫәлән,
I18NI000000046X 64 ағымлы бюджетты рекламалай). Ҡасан төшөрөп ҡалдырылған, ҡулланыусылар
18NI000000047X дөйөм I18NI000000047X һиҙеүенә кире төшөп, рекламаның башҡа урындарында баҫылып сыҡҡан.

CAR мәғлүмәттәрен һорағанда, клиенттар I18NI000000048X баш исемлеген ебәрергә тейеш.
I18NI000000049X кортеждары өҫтөнлөк тәртибендә ярҙам итте:

```
Accept-Chunker: sorafs.sf1;version=1.0.0
``` X

Ҡапҡалар үҙ-ара ярҙам профилен һайлай (I18NI000000050X тиклем ғәҙәттәгесә)
һәм ҡарарҙы сағылдыра аша I18NI000000051X яуап башлығы. Манифесттары
һайланған профилде индереү шулай аҫҡы ағым төйөндәре раҫлай ала өлөшө макеты
HTTP һөйләшеүҙәренә таянмайынса.

###

* I18NI000000052X профиле карталары йәмәғәт ҡорамалдарына 2012 йылда.
  `fixtures/sorafs_chunker` һәм корпора теркәлгән 19 йәшкә тиклем теркәлгән.
  `fuzz/sorafs_chunker`. Аҙағына тиклем паритет ғәмәлгә ашырыла Rust, Go, һәм төйөн .
  аша бирелгән һынауҙар.
* I18NI000000055X раҫлауынса, дескриптор параметрҙары
  матч I18NI000000056X осраҡлы дивергенцияны һаҡлау өсөн.
* `iroha app sorafs toolkit pack` һәм I18NI0000000058X тарафынан етештерелгән манифесттар реестр метамағлүмәттәрен үҙ эсенә ала.