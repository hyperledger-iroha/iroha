---
lang: kk
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-12-29T18:16:35.951567+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docker құрастырушы кескіні

Бұл контейнер `Dockerfile.build` ішінде анықталған және барлық құралдар тізбегін жинақтайды
CI және жергілікті шығарылым құрастырулары үшін қажетті тәуелділіктер. Кескін енді a ретінде жұмыс істейді
әдепкі бойынша root емес пайдаланушы, сондықтан Git операциялары Arch Linux-пен жұмыс істей береді
`safe.directory` жаһандық шешімін қолданбай `libgit2` пакеті.

## Аргументтер құрастырыңыз

- `BUILDER_USER` – контейнер ішінде жасалған логин аты (әдепкі: `iroha`).
- `BUILDER_UID` – пайдаланушының сандық идентификаторы (әдепкі: `1000`).
- `BUILDER_GID` – негізгі топ идентификаторы (әдепкі: `1000`).

Жұмыс кеңістігін хосттан орнатқанда, сәйкес UID/GID мәндерін осылайша өткізіңіз
Жасалған артефакттар жазуға жарамды болып қалады:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

Құралдар тізбегі каталогтары (`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`)
конфигурацияланған пайдаланушыға тиесілі, сондықтан Cargo, rustup және Poetry пәрмендері толығымен қалады
контейнер түбірлік артықшылықтарды алып тастағаннан кейін жұмыс істейді.

## Құрылымдарды іске қосу

Жұмыс кеңістігін шақыру кезінде `/workspace` (контейнер `WORKDIR`) тіркеңіз.
сурет. Мысалы:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

Кескін `docker` топ мүшелігін сақтайды, сондықтан кірістірілген Docker пәрмендері (мысалы:
`docker buildx bake`) PID хостын орнататын CI жұмыс ағындары үшін қолжетімді болып қалады.
және розетка. Топтық салыстыруларды ортаңызға қажетінше реттеңіз.

## Iroha 2 және Iroha 3 артефакті

Жұмыс кеңістігі енді соқтығысуды болдырмау үшін әр шығару жолына бөлек екілік файлдарды шығарады:
`iroha3`/`iroha3d` (әдепкі) және `iroha2`/`iroha2d` (Iroha 2). Көмекшілерді пайдаланыңыз
қажетті жұпты жасаңыз:

- `make build` (немесе `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`) Iroha 3 үшін
- Iroha 2 үшін `make build-i2` (немесе `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`)

Селектор мүмкіндіктер жиынын бекітеді (`telemetry` + `schema-endpoint` плюс
желіге тән `build-i{2,3}` жалауы) сондықтан Iroha 2 құрастырулары кездейсоқ таңдай алмайды
Iroha 3-тек әдепкі.

`scripts/build_release_bundle.sh` арқылы жасалған шығарылым жинақтары дұрыс екілік файлды таңдайды
`--profile` параметрі `iroha2` немесе `iroha3` күйіне орнатылғанда автоматты түрде атаулар.