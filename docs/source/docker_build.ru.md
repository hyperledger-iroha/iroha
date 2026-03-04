---
lang: ru
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-11-29T19:27:21.735343+00:00"
translation_last_reviewed: 2026-01-01
---

# Docker Builder Image

Этот контейнер определен в `Dockerfile.build` и включает все зависимости toolchain,
необходимые для CI и локальных release-сборок. Образ теперь по умолчанию запускается
от не-root пользователя, поэтому операции Git продолжают работать с пакетом `libgit2`
в Arch Linux без глобального обходного решения `safe.directory`.

## Аргументы build

- `BUILDER_USER` — имя логина, создаваемого внутри контейнера (по умолчанию: `iroha`).
- `BUILDER_UID` — числовой id пользователя (по умолчанию: `1000`).
- `BUILDER_GID` — id основной группы (по умолчанию: `1000`).

При монтировании workspace с хоста передавайте соответствующие значения UID/GID, чтобы
сгенерированные артефакты оставались доступными для записи:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

Каталоги toolchain (`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`) принадлежат
сконфигурированному пользователю, поэтому команды Cargo, rustup и Poetry остаются полностью
работоспособными после отказа контейнера от прав root.

## Запуск сборок

Подключайте workspace к `/workspace` (контейнерный `WORKDIR`) при запуске образа. Пример:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

Образ сохраняет членство в группе `docker`, поэтому вложенные команды Docker (например,
`docker buildx bake`) доступны для CI workflow, которые монтируют PID и сокет хоста.
При необходимости скорректируйте сопоставления групп для вашей среды.

## Артефакты Iroha 2 и Iroha 3

Workspace теперь выпускает отдельные бинарники для каждой линии релиза, чтобы избежать
коллизий: `iroha3`/`iroha3d` (по умолчанию) и `iroha2`/`iroha2d` (Iroha 2). Используйте
helper-скрипты, чтобы получить нужную пару:

- `make build` (или `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`) для Iroha 3
- `make build-i2` (или `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`) для Iroha 2

Селектор фиксирует наборы feature-флагов (`telemetry` + `schema-endpoint` плюс специфический
для линии флаг `build-i{2,3}`), чтобы сборки Iroha 2 не подхватили случайно дефолты Iroha 3.

Release bundles, собранные через `scripts/build_release_bundle.sh`, автоматически выбирают
правильные имена бинарников, когда `--profile` установлен в `iroha2` или `iroha3`.
