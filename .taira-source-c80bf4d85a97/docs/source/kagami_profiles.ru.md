---
lang: ru
direction: ltr
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-27T18:39:03.379028+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami Профили Iroha3

Kagami поставляется с предустановками для сетей Iroha 3, чтобы операторы могли штамповать детерминированные
Genesis проявляется без манипулирования ручками каждой сети.

- Профили: `iroha3-dev` (цепочка `iroha3-dev.local`, коллекторы k=1 r=1, начальное число VRF, полученное из идентификатора цепочки, когда выбран NPoS), `iroha3-taira` (цепочка `iroha3-taira`, коллекторы k=3 r=3, требуется `--vrf-seed-hex`, когда выбран NPoS), `iroha3-nexus` (цепочка `iroha3-nexus`, коллекторы k=5 r=3, требуется `--vrf-seed-hex` при выборе NPoS).
- Консенсус: сети профиля Sora (Nexus + пространства данных) требуют NPoS и запрещают поэтапное переключение; разрешенные развертывания Iroha3 должны выполняться без профиля Sora.
- Поколение: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`. Используйте `--consensus-mode npos` для Nexus; `--vrf-seed-hex` действителен только для NPoS (требуется для taira/nexus). Kagami закрепляет DA/RBC на линии Iroha3 и выдает сводку (цепочка, коллекторы, DA/RBC, начальное значение VRF, отпечаток пальца).
- Проверка: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` воспроизводит ожидания профиля (идентификатор цепочки, DA/RBC, коллекторы, покрытие PoP, консенсусный отпечаток). Укажите `--vrf-seed-hex` только при проверке манифеста NPoS для taira/nexus.
- Примеры пакетов: предварительно созданные пакеты находятся под `defaults/kagami/iroha3-{dev,taira,nexus}/` (genesis.json, config.toml, docker-compose.yml,verify.txt, README). Выполните регенерацию с помощью `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]`.
- Mochi: `mochi`/`mochi-genesis` принимает `--genesis-profile <profile>` и `--vrf-seed-hex <hex>` (только NPoS), пересылает их в Kagami и печатает ту же сводку Kagami в стандартный вывод/stderr при использовании профиля.

Пакеты встраивают точки BLS вместе с записями топологии, поэтому `kagami verify` работает успешно.
из коробки; настройте доверенные узлы/порты в конфигурациях по мере необходимости для локального
дым бежит.