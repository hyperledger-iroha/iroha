---
lang: ba
direction: ltr
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2025-12-29T18:16:35.972838+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% JDG-SDN аттестацияһы һәм әйләнеш

Был иҫкәрмә Серле мәғлүмәттәр төйөндәре (SDN) аттестацияһы өсөн үтәү моделен тота.
юрисдикция мәғлүмәттәре һаҡсыһы (JDG) ағымы ҡулланған.

## Ҡулға алыу форматында
- `JdgSdnCommitment` өлкәһен бәйләй (`JdgAttestationScope`), шифрланған
  файҙалы йөк хеш, һәм SDN асыҡ асҡыс. Сальдар — ҡултамғалар тип яҙылған
  (`SignatureOf<JdgSdnCommitmentSignable>`) домен-теглы файҙалы йөк өҫтөндә
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`.
- Структур валидация (`validate_basic`) үтәй:
  - `version == JDG_SDN_COMMITMENT_VERSION_V1`
  - дөрөҫ блок диапазондары
  - буш булмаған мөһөрҙәр
  - аттестацияға ҡаршы масштаб тигеҙлеге аша үткәндә
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- Дедубликация аттестация валитаторы тарафынан эшкәртелә (ҡушымта+түләү хеш
  үҙенсәлек) тотоп торған/ҡабатланған йөкләмәләрҙе иҫкәртергә.

## Реестр һәм ротация сәйәсәте
- SDN асҡыстары `JdgSdnRegistry`-та йәшәй, уларҙы `(Algorithm, public_key_bytes)` төймәһендә.
- `JdgSdnKeyRecord` активация бейеклеген, пенсияға сығыу бейеклеген теркәй, пенсияға сығыу, 2000 й.
  һәм опциональ ата-әсә асҡысы.
- Ротацион `JdgSdnRotationPolicy` менән идара итә (әлеге ваҡытта: `dual_publish_blocks`
  ҡаплап тәҙрә). Теркәү бала асҡыс яңырта ата-әсә пенсияға .
  `child.activation + dual_publish_blocks`, ҡоршау менән:
  - юғалған ата-әсәләр кире ҡағыла .
  - активациялар ҡәтғи рәүештә артырға тейеш .
  - рәхмәт тәҙрәһенән артып киткән ҡапланыуҙар кире ҡағыла
- Реестр ярҙамсылары ҡуйылған яҙмаларҙы (`record`, `keys`) статус өсөн сыға
  һәм API экспозицияһы.

## Валидация ағымы
- `JdgAttestation::validate_with_sdn_registry` структурын урап ала
  аттестация тикшерергә һәм SDN үтәү. `JdgSdnPolicy` ептәре:
  - `require_commitments`: PII/секрет файҙалы йөктәр өсөн булыуҙы үтәү
  - `rotation`: ата-әсә пенсияһын яңыртыуҙа ҡулланылған рәхмәт тәҙрәһе
- Һәр йөкләмә тикшерелә:
  - структур дөрөҫлөк + аттестация-киҫәк матчы
  - теркәлгән төп булыу
  - аттестлы блок диапазонын ҡаплаған әүҙем тәҙрә (пенсия сиктәре инде
    ике баҫҡыслы рәхмәтте үҙ эсенә ала)
  - домен-теглы йөкләмә органы өҫтөндә дөрөҫ мөһөр
- Тотороҡло хаталар оператор дәлилдәре өсөн индекс өҫтөндә:
  `MissingSdnCommitments`, `UnknownSdnKey`, `InactiveSdnKey`, `InvalidSeal`,
  йәки структур `Commitment`/`ScopeMismatch` етешһеҙлектәре.

## Оператор runbook
- **Провизия:** `activated_at` менән беренсе SDN төймәһен теркәй.
  беренсе йәшерен блок бейеклеге. Төп бармаҡ эҙен JDG операторҙарына баҫтырып сығарыу.
- **Ротат:** вариҫ асҡысын генерациялау, уны `rotation_parent` менән теркәү .
  ағымдағы асҡысҡа күрһәтеп, ата-әсәнең пенсияға тиң раҫлауы
  `child_activation + dual_publish_blocks`. Ҡабаттан герметизация файҙалы йөк йөкләмәләре менән
  ҡапланыу тәҙрәһе ваҡытында әүҙем асҡыс.
- **Аудит:** реестр снимоктарын фашлай (`record`, `keys`) аша Torii/статус
  ер өҫтө шулай аудиторҙар әүҙем асҡыс һәм пенсия тәҙрәләре раҫлай ала. Уяу
  әгәр ҙә аттестацияланған диапазон әүҙем тәҙрәнән ситтә төшә.
- **Һауыҡтырыу:** `UnknownSdnKey` → реестрҙы герметизация асҡысын үҙ эсенә ала;
  `InactiveSdnKey` → әүҙемләштереү бейеклеген әйләндерә йәки көйләү; `InvalidSeal` →
  ҡабаттан тығыҙлыҡ йөктәре һәм яңыртыу аттестацияһы.## Йүгереп йөрөүсе ярҙамсы
- `JdgSdnEnforcer` X (`crates/iroha_core/src/jurisdiction.rs`) сәйәсәтте пакеттар + .
  теркәү һәм раҫлай аттестациялар аша `validate_with_sdn_registry`.
- Реестрҙарҙы Torii өйөмдәренән тейәп була (ҡара:
  `JdgSdnEnforcer::from_reader`/`from_path`) йәки йыйылған
  `from_records`, был теркәү ваҡытында ротация ҡоршауҙары ҡулланыла.
- Операторҙар Norito өйөмөн Torii/статусҡа дәлилдәр итеп һаҡлай ала.
  өҫкө өлөшө, шул уҡ ваҡытта шул уҡ файҙалы йөк емләп, ҡабул итеү һәм ҡабул итеү һәм
  консенсус һаҡсылары. Бер глобаль үтәүсе стартапта инициализациялана ала.
  `init_enforcer_from_path`, һәм `enforcer()`/`registry_snapshot()`/`sdn_registry_status()` X.
  йәшәү сәйәсәтен фаш + төп яҙмалар өсөн статус/Torii өҫтө.

## Һынауҙар
- `crates/iroha_data_model/src/jurisdiction.rs`-та регрессия ҡаплауы:
  `sdn_registry_accepts_active_commitment`, `sdn_registry_rejects_unknown_key`,
  `sdn_registry_rejects_inactive_key`, `sdn_registry_rejects_bad_signature`,
  `JdgSdnCommitment`,
  `sdn_registry_rejects_overlap_beyond_policy`, ғәмәлдәге менән бер рәттән
  структур аттестация/SDN валидация һынауҙары.