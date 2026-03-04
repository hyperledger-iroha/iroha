---
lang: ba
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-12-29T18:16:35.951567+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docker Төҙөүсе һүрәте

Был контейнер `Dockerfile.build` билдәләнә һәм бөтә инструменттар өйөмдәре .
CI һәм урындағы релиз төҙөү өсөн кәрәкле бәйлелектәр. Һүрәт хәҙер йүгерә
тамырһыҙ ҡулланыусы ғәҙәттәгесә, шуға күрә Git операциялары архи Linux менән эшләүен дауам итә’s .
`libgit2` пакеты глобаль `safe.directory` оборламына мөрәжәғәт итмәйенсә.

## аргументтар төҙөү

- `BUILDER_USER` – контейнер эсендә булдырылған логин исеме (по умолчанию: `iroha`).
- `BUILDER_UID` – һанлы ҡулланыусы id (поручка: `1000`).
- `BUILDER_GID` – беренсел төркөм id (порупное: `1000` X).

Ҡасан һеҙ монтаж эш урыны һеҙҙең хост, үткәреү тап килгән UID/GID ҡиммәттәре шулай
генерацияланған артефакттар яҙыулы булып ҡала:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
``` X

Ҡораллы селтәр каталогы (`/usr/local/rustup`, `/usr/local/cargo`, Iroha)
милкендә мил
функциональ бер тапҡыр һауыт тамыр өҫтөнлөктәрен төшөрә.

## Йүгереп төҙөй

Һеҙҙең эш урынын `/workspace` тиклем беркетергә (контейнер `WORKDIR`) ҡасан саҡырыу
һүрәт. Миҫал:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

Һүрәттә `docker` төркөмө ағзалығы шул тиклем оялы Docker командалары һаҡлана (мәҫәлән.
`docker buildx bake`) CI эш ағымдары өсөн мөмкин булып ҡала, улар хост PID монтажлау .
һәм розетка. Төркөм карталарын көйләү кәрәк булғанда һеҙҙең мөхит.

## Iroha 2 ҡаршы Iroha 3 артефакт

Эш урыны хәҙер бәрелештәрҙән ҡотолоу өсөн айырым бинарҙарҙы сығарыу һыҙығына сығара:
`iroha3`/`iroha3d` (поручка) һәм `iroha2` X/Docker (Iroha 2). Ярҙамсыларҙы ҡулланыу өсөн
теләк парын етештерә:

- `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3` `make build` өсөн Iroha 3
- `make build-i2` (йәки `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`) Iroha өсөн 2

Һайлаусы функциялар йыйылмаһы булавкалары (`telemetry` + `schema-endpoint` плюс .
линия-специфик `build-i{2,3}` флагы) шулай Iroha 2 төҙөү осраҡлы рәүештә йыйып ала алмай
Iroha 3-өсөн генә ғәҙәттәгесә.

`scripts/build_release_bundle.sh` аша төҙөлгән өйөмдәр дөрөҫ бинар һайлау
автоматик рәүештә `--profile` X йәки `iroha3` `iroha3` тип аталған.