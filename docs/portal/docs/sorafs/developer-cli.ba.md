---
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c9acf8a8d9c298ad2fe95e5480942441aa790346f90c5b5e1a8c1ff638c5e73
source_last_modified: "2026-01-22T16:26:46.522695+00:00"
translation_last_reviewed: 2026-02-07
id: developer-cli
title: SoraFS CLI Cookbook
sidebar_label: CLI Cookbook
description: Task-focused walkthrough of the consolidated `sorafs_cli` surface.
translator: machine-google-reviewed
---

:::иҫкәртергә канонлы сығанаҡ
::: 1990 й.

Консолидацияланған I18NI000000013X өҫтө (`sorafs_car` йәшник менән тәьмин ителә.
I18NI000000015X функцияһы өҫтөндә эшләй) I18NT0000000003X әҙерләү өсөн кәрәкле һәр аҙымды фашлай.
артефакттары. Был аш-һыу китабын ҡулланып, туранан-тура дөйөм эш ағымына һикерергә; уны пар менән .
оператив контекст өсөн асыҡ торба һәм оркестр йүнһеҙлеге.

## Пакет файҙалы йөкләмәләр

Ҡулланыу `car pack` етештереү өсөн детерминистик CAR архивтары һәм өлөшө пландары. 1990 й.
командаһы автоматик рәүештә SF-1 чанкерын һайлай, әгәр ҙә тотҡа бирелмәһә.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Ғәҙәттәгесә, чанкер ручкаһы: I18NI000000017X.
- Каталог индереүҙәр лексикографик тәртиптә йөрөйҙәр, шуға күрә чектар тотороҡло ҡала
  платформалар буйынса.
- JSON йомғаҡлауы файҙалы йөктәрҙе үҙ эсенә ала, пер-версия метамағлүмәттәре, һәм тамыр .
  CID реестр һәм оркестр менән танылған.

## Конструкция манифесттары

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- I18NI0000000018X варианттары картаһы туранан-тура I18NI000000019X яландарында 2012 йылда.
  `sorafs_manifest::ManifestBuilder`.
- I18NI000000021X тәьмин итеү, ҡасан һеҙ теләйһегеҙ, CLI SHA3 өлөшөн ҡабаттан иҫәпләү өсөн
  тапшырыу алдынан үҙләштереү; юғиһә ул ҡабаттан файҙалана үҙләштереү һеңдерелгән .
  һығымта.
- JSON сығарыу I18NT0000000001X файҙалы йөкләмәһен көҙгөләй, туранан-тура диффтар ваҡытында 2012 йыл.
  рецензиялар.

## Ҡулланма оҙон ғүмерле асҡыстарһыҙ күренә

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
``` X

- 1-се жетондарҙы, тирә-яҡ мөхит үҙгәртеүселәрен йәки файлға нигеҙләнгән сығанаҡтарҙы ҡабул итеү.
- Провенанс метамағлүмәттәрен өҫтәй (`token_source`, `token_hash_hex`, өлөшләтә һеңдерергә)
  сеймал JWT һаҡһыҙ, әгәр `--include-token=true`.
- CI-ла яҡшы эшләй: GitHub Actions I18NT00000000006X менән берләштерә
  `--identity-token-provider=github-actions`.

## I18NT000000004X-ҡа тапшырыла

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority i105... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- I18NT000000002X псевдонимы өсөн декодлауҙы башҡара һәм улар тап килә.
  I18NT000000005X тиклем POSTing алдынан асыҡ һеңдерергә.
- SHA3 өлөшө үҙләштереү планы тап килмәү һөжүмдәренә юл ҡуймау.
- Яуап резюмелары HTTP статусын, башлыҡтарын һәм реестр файҙалы йөктәрен тота.
  һуңыраҡ аудит.

## CAR йөкмәткеһен һәм дәлилдәрен раҫлау

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- PoR ағасын яңынан төҙөй һәм файҙалы йөктәрҙе үҙләштереүҙе асыҡ резюме менән сағыштыра.
- Ҡаҙанлыҡтарҙы иҫәпләй һәм идентификаторҙар репликацияға дәлилдәр тапшырғанда талап ителә
  идара итеүгә тиклем.

## Стромлы дәлилдәр телеметрия

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v2/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- Һәр ағымлы дәлилдәр өсөн NDJSON әйберҙәрен сығара (өҙөүсе реплей менән өҙөлә
  `--emit-events=false`).
- Уңыштар/уңышһыҙлыҡтар һаны агрегаттары, латентлыҡ гистограммалары, һәм 1990 йылда уңышһыҙлыҡтар өлгөләре.
  Йыйынтыҡ JSON шулай приборҙар таҡталары һөҙөмтәләрен график төҙөй ала, бүрәнәләр ҡырҡып.
- нульдән тыш сыға, ҡасан шлюз тураһында хәбәр итә етешһеҙлектәр йәки урындағы PoR тикшерергә
  (`--por-root-hex` аша) дәлилдәрҙе кире ҡаға. Сиктәрҙе көйләү менән .
  18NI000000028X һәм репетиция йүгерә өсөн `--max-verification-failures`.
- Бөгөн PoR-ға ярҙам итә; PDP һәм PoTR ҡабаттан ҡулланыу бер үк конверт бер тапҡыр SF-13/SF-14
  ил.
- I18NI000000030X рендерланған резюме, метамағлүмәттәр яҙа (ваҡыт тамғаһы,
  CLI версияһы, шлюз URL, асыҡ һеңдерергә), һәм күсермәһе манифест .
  тәьмин ителгән каталог шулай идара итеү пакеттары архивлау мөмкин дәлил-ағым
  йүгереүҙе ҡабатламайынса дәлилдәр.

## Өҫтәмә һылтанмалар

- `docs/source/sorafs_cli.md` — тулы флаг документацияһы.
- I18NI000000032X — иҫбатлау телеметрия схемаһы һәм I18NT000000000X
  приборҙар таҡтаһы шаблон.
- I18NI000000033X — тәрән һыу инеү өҫтөндә чанкинг, манифест
  композиция, һәм CAR менән эш итеү.