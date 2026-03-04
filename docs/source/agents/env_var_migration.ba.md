---
lang: ba
direction: ltr
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9ce6010594e495116c1397b984000d1ee5d45d064294eca046f8dc762fa73b6
source_last_modified: "2026-01-05T09:28:11.999442+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Env → Конфигурация миграцияһы Трекер

Был трекер дөйөмләштерә производство-йөҙөндә мөхит-үҙгәрмәй торған переключатель өҫтөндә .
`docs/source/agents/env_var_inventory.{json,md}` һәм тәғәйенләнгән миграцияға тиклем
юл `iroha_config` (йәки асыҡ dev/һынау-тик scoping).


Иҫкәрмә: `ci/check_env_config_surface.sh` хәҙер яңы **производство** env ҡасан уңышһыҙлыҡҡа осрай
shims `AGENTS_BASE_REF` менән сағыштырғанда күренә, әгәр `ENV_CONFIG_GUARD_ALLOW=1` булмаһа.
йыйылма; документ ниәтләп өҫтәүҙәр бында ҡулланыу алдынан өҫтөнлөк.

## тамамланған миграциялар- **IVM ABI опт-аут ** — `IVM_ALLOW_NON_V1_ABI` сығарыу; компилятор хәҙер кире ҡаға
  v1 булмаған АБИ-лар шартһыҙ рәүештә хата юлын һаҡлаусы берәмек һынауы менән.
- **IVM отладка баннер env chim** — `IVM_SUPPRESS_BANNER` env out-out;
  баннер баҫтырыу программалы ҡоротҡос аша ҡала.
- **IVM кэш/размерлау** — епләнгән кэш/гурист/ГПУ размерлау аша
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`,
  `accel.max_gpus`) һәм эшләү ваҡыты env shims. Хужалар хәҙер шылтырата
  `ivm::ivm_cache::configure_limits` һәм `ivm::zk::set_prover_threads`, һынауҙар ҡулланыу
  `CacheLimitsGuard` урынына env өҫтөнлөк итә.
- **Ситаждың сират тамыры** — `connect.queue.root` өҫтәлгән (поручка:
  `~/.iroha/connect`) клиентҡа конфиг һәм уны CLI һәм 2
  JS диагностикаһы. JS ярҙамсылары конфиг (йәки асыҡ `rootDir`) һәм 1990 й.
  тик `IROHA_CONNECT_QUEUE_ROOT` тик dev/һынауҙа `allowEnvOverride` аша ғына хөрмәт;
  ҡалыптар документ ручка шулай операторҙар инде кәрәкмәй env өҫтөнлөк.
- **Izanami селтәр оп-ин** — `allow_net` CLI/config флагы өсөн асыҡ өҫтәлде.
  Izanami хаос инструменты; йүгерә хәҙер `allow_net=true`/`--allow-net` һәм
- **IVM баннер гудок** — `IROHA_BEEP` env shim менән алмаштырылған конфиг-двигателдәр
  `ivm.banner.{show,beep}` X toggles (подлубливый: дөрөҫ/дөрөҫ). Стартап баннеры/ гудок
  проводка хәҙер конфигурацияны тик производствола ғына уҡый; dev/тест төҙөү һаман да хөрмәт
  env ҡул менән өҫтөнлөклө переключатель.
- **ДА катушка өҫтөнлөк (һынауҙар ғына)** — `IROHA_DA_SPOOL_DIR` өҫтөнлөк хәҙер.
  `cfg(test)` ярҙамсылары артында кәртәләп алынған; етештереү коды һәр ваҡыт катушка сығанаҡтары
  конфигурациянан юл.
- **Крипто интринсика** — `IROHA_DISABLE_SM_INTRINSICS` / алмаштырылған /
  `IROHA_ENABLE_SM_INTRINSICS` конфигурация менән эшләгән
  `crypto.sm_intrinsics` сәйәсәте (`auto`/`force-enable`/`force-disable`) һәм
  `IROHA_SM_OPENSSL_PREVIEW` һаҡсыһын алып ташлай. Хужалар сәйәсәтте 2011 йылда ҡуллана.
  стартап, эскәмйәләр/тестар `CRYPTO_SM_INTRINSICS` аша ҡабул итеүе мөмкин, ә OpenSSL
  алдан ҡарау хәҙер тик конфиг флагын хөрмәт итә.
  Izanami инде `--allow-net`/персистенный конфиг талап итә, ә һынауҙар хәҙер таяна.
  тип ручка түгел, ә тирә-яҡ мөхит env toggles.
- **FastPQ GPU көйләү** — `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}` өҫтәлгән
  config knobs (defaults: `None`/`None`/`false`/`false`/`false`) and thread them through CLI parsing
  `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` шашкалары хәҙер үҙен dev/һынау fallbacks һәм
  бер тапҡыр конфигурация йөкләмәләрен иғтибарға алмай (хатта конфиг уларҙы ҡуйылмаған ҡалдырғанда); docs/инвентарь булды
  Яңыртылған флаг миграция.【краттар/ро, src/main.rs:2609】【крат/ироха_ядро/src/src/fastpq/lane.109】【крат/тиҙ_провер/src/overrides.rs.:11】
  (`IVM_DECODE_TRACE`, `IVM_DEBUG_WSV`, `IVM_DEBUG_COMPACT`, `IVM_DEBUG_INVALID`,
  `IVM_DEBUG_REGALLOC`, `IVM_DEBUG_METAL_ENUM`, `IVM_DEBUG_METAL_SELFTEST`,
  `IVM_FORCE_METAL_ENUM`, `IVM_FORCE_METAL_SELFTEST_FAIL`, `IVM_FORCE_CUDA_SELFTEST_FAIL`,
  `IVM_DISABLE_METAL`, `IVM_DISABLE_CUDA`) хәҙер ҡапҡалы артында отладка/тест төҙөү аша уртаҡ .
  ярҙамсы шулай етештереү бинарҙары уларҙы иғтибарға алмай, шул уҡ ваҡытта урындағы диагностика өсөн ручкалар һаҡлана. Env
  инвентарь регенерацияланған, тип сағылдырыу өсөн dev/һынау-тик даирәһе.- * *FASTPQ ҡоролмаларын яңыртыу** — `FASTPQ_UPDATE_FIXTURES` хәҙер FASTPQ интеграцияһында ғына күренә
  һынауҙар; етештереү сығанаҡтары инде уҡымай env toggle һәм инвентарь сағылдыра һынау-тик
  масштаб.
- **Инвентаризация яңыртыу + даирәһен асыҡлау** — Энв инвентаризация инструменттары хәҙер `build.rs` файлдарын тегтар тип билдәләй.
  төҙөү даирәһе һәм тректар `#[cfg(test)]`/интеграция жгут модулдәре шулай һынау-тик toggles (мәҫәлән,
  `IROHA_TEST_*`, `IROHA_RUN_IGNORED`) һәм CUDA төҙөү флагтары етештереү һанынан ситтә күренә.
  Инвентаризация регенерацияланған декабрь 07, 2025 (518 refs / 144 vars) һаҡлау өсөн env-config һаҡсыһы diff йәшел.
- **P2P топологияһы env shim релиз һаҡсыһы** — `IROHA_P2P_TOPOLOGY_UPDATE_MS` хәҙер детерминистик тыуҙыра
  стартап хатаһы сығарыу төҙөү (иҫкәртергә-тик отладка/тест) шулай етештереү төйөндәре тик таяна
  `network.peer_gossip_period_ms`. 1990 йылдарҙа был йүнәлештәге эштәрҙең инвентаризацияһы һаҡсыны сағылдырыу өсөн яңыртылған һәм
  яңыртылған классификатор хәҙер scops `cfg!`-һаҡланған переключатель отладка/тест булараҡ.

## Юғары өҫтөнлөклө миграциялар (етештереү юлдары)

- _Бер ниндәй ҙә (инвентарь менән яңыртылған cfg!/отладка асыҡлау; env-config һаҡсыһы йәшел һуң P2P shim ҡатыу)._

## Дев/тест-тик ҡоршау өсөн ҡоймаға

- Ағымдағы һепертке (07 декабрь, 2025): төҙөү-тик CUDA флагтары (`IVM_CUDA_*`) `build` һәм .
  йүнлек өҙөктәре (`IROHA_TEST_*`, `IROHA_RUN_IGNORED`, `IROHA_SKIP_BIND_CHECKS`) хәҙер 1000 й.
  `test`/`debug` инвентаризацияла (шул иҫәптән `cfg!`- һаҡсылы шашкалар). Өҫтәмә ҡойма кәрәкмәй;
  киләсәктә өҫтәүҙәрҙе һаҡлау артында `cfg(test)`/эскәмйә-тик ярҙамсылары менән TODO маркерҙары ҡасан shims ваҡытлыса.

## Төҙөү ваҡытында envs (сығып, нисек-был)

- Йөк/функция envs (`CARGO_*`, `OUT_DIR` X, `DOCS_RS`, `PROFILE`, `CUDA_HOME`,
  `CUDA_PATH`, `JSONSTAGE1_CUDA_ARCH`, `FASTPQ_SKIP_GPU_BUILD`, һ.б.)
  төҙөү-сценарий борсолоуҙары һәм экст-коп өсөн йөрөү ваҡыты конфиг миграцияһы.

## Киләһе ғәмәлдәр

1) Run `make check-env-config-surface`X конфиг-ер өҫтө яңыртыуҙарҙан һуң яңы етештереү env shims тотоу өсөн
   иртә һәм подсистема хужалары/ЭТА.  
2) Яңыртыу инвентаризация (`make check-env-config-surface`) һәр һыпыртыуҙан һуң шулай
   трекер ҡала менән тура килтереп, яңы ҡоршау һәм env-config һаҡсыһы дифф ҡала шау-шыуһыҙ.