---
lang: ba
direction: ltr
source: docs/source/contract_deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f2b1d7d027d715eac5a3ca8be29dea8f0e76013e948947a4de66108ac561f34
source_last_modified: "2026-01-22T14:58:53.689594+00:00"
translation_last_reviewed: 2026-02-07
title: Contract Deployment (.to) — API & Workflow
translator: machine-google-reviewed
---

Статус: Torii, CLI һәм төп ҡабул итеү һынауҙары тарафынан тормошҡа ашырылған һәм ғәмәлгә ашырыла (2025 йылдың ноябре).

## Обзор

- IVM байтекодты (`.to`) йыйылмаһы Torii-ға тапшырып йәки сығарыу юлы менән төҙөгән.
  `RegisterSmartContractCode`/`RegisterSmartContractBytes` инструкциялары
  туранан-тура.
- Төйөндәр `code_hash` һәм канонлы АБИ хешын урындағы кимәлдә ҡабаттан иҫәпләү; тап килмәүе
  детерминистик яҡтан кире ҡағыу.
- Һаҡланған артефакттар `contract_manifests` һәм 1990 й.
  `contract_code` реестрҙары. Манифесттар хештар ғына һылтанмалар һәм бәләкәй булып ҡала;
  код байттары `code_hash` тарафынан клавиша.
- Һаҡланған исемдәр киңлектәрендә идара итеү тәҡдимен талап итә ала, ә һуңынан
  таратыу ҡабул ителә. Ҡабул итеү юлы тәҡдим файҙалы йөк һәм өҫкә ҡарай һәм
  үтәй `(namespace, contract_id, code_hash, abi_hash)` тигеҙлек ҡасан
  исемдәр киңлеге һаҡлана.

## Һаҡланған артефакттар & Һаҡлау

- `RegisterSmartContractCode` вставкалар/өҫтөндә бирелгән манифест өсөн бирелгән
  `code_hash`. Ҡасан шул уҡ хеш инде бар, ул яңы менән алмаштырыла
  беленергә.
- `RegisterSmartContractBytes` 1990 йылдарҙа төҙөлгән программаны һаҡлай.
  `contract_code[code_hash]`. Әгәр байт өсөн хеш инде бар, улар тура килергә тейеш
  теүәл; төрлө байттар инвариант хоҡуҡ боҙоуҙы күтәрә.
- Код күләме `max_contract_code_bytes` ҡулланыусы параметры менән ҡапланған
  (ғәҙәти 16 МиБ). Уны `SetParameter(Custom)` транзакцияһы менән өҫтөнлөк бирегеҙ.
  ҙурыраҡ артефакттарҙы теркәү.
- Һаҡлау сикһеҙ: манифест һәм код асыҡтан-асыҡ тиклем ҡала
  киләсәктә идара итеү эш ағымында алынды. ТТЛ йәки автоматик ГК юҡ.

## Ҡабул итеү торбаһы

- Валитатор IVM башын анализлай, `version_major == 1` X, чектарҙы үтәй.
  `abi_version == 1`. Билдәһеҙ версиялар шунда уҡ кире ҡаға; эшләү ваҡыты юҡ
  переключатель.
- Ҡасан манифест инде `code_hash` өсөн бар, раҫлау тәьмин итә
  һаҡлаған `code_hash`/`abi_hash` тигеҙ иҫәпләнгән ҡиммәттәргә тиң тапшырылған
  программаһы. Тураш түгел, `Manifest{Code,Abi}HashMismatch` хаталары етештерә.
- Һаҡланған исемдәр киңлектәренә йүнәлтелгән операциялар метамағлүмәт асҡыстарын үҙ эсенә алырға тейеш
  `gov_namespace` һәм `gov_contract_id`. Ҡабул итеү юлы уларҙы сағыштыра .
  ҡаршы ҡабул ителгән `DeployContract` тәҡдимдәре; әгәр ҙә бер ниндәй ҙә тап килгән тәҡдим бар, был
  операцияһы `NotPermitted` менән кире ҡағыла.

## Torii ос нөктәләре (функцияһы `app_api`)- `POST /v1/contracts/deploy`
  - Һорау органы: `DeployContractDto` X (ҡара: Баҫыу реквизиттары өсөн `docs/source/torii_contracts_api.md`).
  - Torii base64 файҙалы йөктө декодлай, ике хеш-шашын иҫәпләй, манифест төҙөй,
    һәм `RegisterSmartContractCode` плюс тапшыра
    `RegisterSmartContractBytes` ҡултамғалы операцияла
    шылтыратыусы.
  - Яуап: `{ ok, code_hash_hex, abi_hash_hex }`.
  - Хаталар: дөрөҫ булмаған base64, ярҙамһыҙ ABI версияһы, рөхсәт юҡ
    (`CanRegisterSmartContractCode`), ҙурлыҡтағы ҡапҡас артып китә, ​​идара итеү ҡапҡаһы.
- Torii.
  - `RegisterContractCodeDto` ҡабул итеү (власть, шәхси асҡыс, асыҡ) һәм тик тапшыра
    `RegisterSmartContractCode`. Ҡулланыу ҡасан манифестар айырым ҡуйыла 2013 йылдан .
    байткод.
- `POST /v1/contracts/instance`
  - ҡабул итеү `DeployAndActivateInstanceDto` (власть, шәхси асҡыс, исемдәр киңлеге/контракт_id, `code_b64`, опциональ манифест өҫтөнлөктәре) һәм таратыу + атомлы әүҙемләштереү.
- `POST /v1/contracts/instance/activate`
  - Ҡабул итеү `ActivateInstanceDto` (власть, шәхси асҡыс, исемдәр киңлеге, cout_id, `code_hash`) һәм тик активация инструкцияһын тапшыра.
- `GET /v1/contracts/code/{code_hash}`
  - Ҡайтарыу `{ manifest: { code_hash, abi_hash } }`.
    Өҫтәмә асыҡ ҡырҙар эске һаҡланған, әммә бында төшөрөп ҡалдырылған өсөн
    тотороҡло API.
- `GET /v1/contracts/code-bytes/{code_hash}`
  - `{ code_b64 }` ҡайтарыу менән һаҡланған `.to` һүрәте base64 тип кодланған.

Бөтә контракт тормош циклы остары бағышланған бағышланған таратыусы сикләүсе аша конфигурацияланған
`torii.deploy_rate_per_origin_per_sec` (секундына жетондар) һәм
`torii.deploy_burst_per_origin` X (бурст токендары). Ғәҙәттәгесә 4 req/s менән ярсыҡ менән 2012.
8 өсөн һәр токен/асҡыс алынған `X-API-Token`, алыҫ IP, йәки ос нөктәһе кәңәштәре.
Ышаныслы операторҙар өсөн сикләүсене өҙөү өсөн `null`-ға тиклем йәки яланға ҡуйығыҙ. Ҡасан .
сикләүсе янғындар, Torii өҫтәүҙәр
`torii_contract_throttled_total{endpoint="code|deploy|instance|activate"}` телеметрия счетчигы һәм
ҡайтара HTTP 429; теләһә ниндәй обработчик хатаһы өҫтәүҙәр
`torii_contract_errors_total{endpoint=…}` иҫкәрткән өсөн.

## Идара итеү интеграцияһы & һаҡланған исемдәр киңлеге- `gov_protected_namespaces` (JSON массив исемдәр киңлеге
  ҡылдар) ҡабул итеү ҡапҡаһы мөмкинлек бирә. Torii ярҙамсыларҙы фашлай.
  `/v1/gov/protected-namespaces` һәм CLI уларҙы көҙгө аша
  `iroha_cli app gov protected set` / `iroha_cli app gov protected get`.
- `ProposeDeployContract` менән булдырылған тәҡдимдәр (йәки Torii X
  `/v1/gov/proposals/deploy-contract` ос нөктәһе) тотоу
  `(namespace, contract_id, code_hash, abi_hash, abi_version)`.
- Бер тапҡыр референдум үтә, `EnactReferendum` тәҡдимде билдәләй һәм
  ҡабул итеү ҡабул итәсәк таратыу, улар тап килгән метамағлүмәттәр һәм код йөрөтә.
- Транзакциялар Torii һәм
  `gov_contract_id=an identifier` (һәм `contract_namespace` / ҡуйырға тейеш /
  `contract_id` өсөн шылтыратыу-ваҡыт бәйләү). CLI ярҙамсылары был халыҡты тултыра
  автоматик рәүештә үткәндә `--namespace`/`--contract-id`.
- Ҡасан һаҡланған исемдәр киңлеге өҫтөндә эшләй, сиратҡа ҡабул итеү тырышлыҡтарын кире ҡаға.
  ғәмәлдәге `contract_id` X-ты икенсе исемдәр киңлегенә ҡабаттан бәйләү; ҡабул ителгән ҡулланыу
  тәҡдим йәки пенсияға элекке мотлаҡ башҡа урындарҙы йәйелдерер алдынан.
- Әгәр ҙә һыҙат манифестында бер өҫтәге валитатор кворумы ҡуйһа, үҙ эсенә ала
  `gov_manifest_approvers` (JSON массив валитатор иҫәбенә идентификаторҙар) шулай сират иҫәпләй ала
  өҫтәмә раҫлауҙар менән бер рәттән транзакция органы. Лейнс та кире ҡаға.
  метамағлүмәттәр, һылтанмалар исемдәр киңлектәрендә булмаған манифест&#8217;s
  `protected_namespaces` комплекты.

## CLI ярҙамсылары

- `iroha_cli app contracts deploy --authority <id> --private-key <hex> --code-file <path>`
  Torii тапшырыу запросын тапшыра (осоуҙа хештарҙы иҫәпләү).
- `iroha_cli app contracts deploy-activate --authority <id> --private-key <hex> --namespace <ns> --contract-id <id> --code-file <path>`
  манифест төҙөй (ялған асҡыс менән ҡултамға), байт + манифест,
  һәм бер транзакцияла `(namespace, contract_id)` бәйләүен әүҙемләштереү. Файҙаланыу
  `--dry-run` компьютер хештарын һәм инструкция һанын баҫтырыу өсөн
  тапшырыу, һәм `--manifest-out` ҡул ҡуйылған манифест JSON ҡотҡарыу өсөн.
- `iroha_cli app contracts manifest build --code-file <path> [--sign-with <hex>]` иҫәпләүҙәр
  Torii өсөн `.to` X һәм теләк буйынса манифестҡа ҡул ҡуя,
  JSON йәки яҙыу `--out` тип баҫтырыу.
- `iroha_cli app contracts simulate --authority <id> --private-key <hex> --code-file <path> --gas-limit <u64>`
  офлайн виртуаль пропуск эшләй һәм хәбәр итә ABI/хэш метамағлүмәттәр плюс сиратлы ИСИ
  (һанаҡ һәм инструкция ids) селтәргә теймәйенсә. Беркетергә
  `--namespace/--contract-id` көҙгө шылтыратыу-ваҡыт метамағлүмәттәр.
- Torii Torii аша манифест алып килә.
  һәм теләк буйынса уны дискҡа яҙа.
- `iroha_cli app contracts code get --code-hash <hex> --out <path>` скачивание
  һаҡланған Torii һүрәте.
- `iroha_cli app contracts instances --namespace <ns> [--table]`X исемлеге әүҙемләштерелгән
  контракт инстанциялары (төп + метамағлүмәттәр менән идара ителә).
- Идара итеү ярҙамсылары (`iroha_cli app gov deploy propose`, `iroha_cli app gov enact`,
  `iroha_cli app gov protected set/get` X) һаҡланған исем-шәрифе эш ағымын оркестрлаштыра һәм
  аудит өсөн JSON артефакттарын фашлай.

## Һынау & ҡаплау

- `crates/iroha_core/tests/contract_code_bytes.rs` ҡаплау коды буйынса берәмек һынауҙары
  һаҡлау, идемпотенция, һәм ҙурлыҡтағы ҡапҡас.
- `crates/iroha_core/tests/gov_enact_deploy.rs` раҫлай аша манифест индереү .
  ҡабул итеү, һәм `crates/iroha_core/tests/gov_protected_gate.rs` күнекмәләр
  һаҡланған-исемдәре ҡабул итеү аҙағынан аҙағына тиклем.
- Torii маршруттары үтенес/яуап блогы һынауҙары инә, ә CLI командалары бар.
  интеграция һынауҙары тәьмин итеү JSON түңәрәк-сәйәхәттәр тотороҡло ҡала.

Һылтанма `docs/source/governance_api.md` өсөн ентекле референдум файҙалы йөкләмәләр һәм
бюллетень эш ағымы.