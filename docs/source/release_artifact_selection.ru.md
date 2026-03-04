---
lang: ru
direction: ltr
source: docs/source/release_artifact_selection.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3ea92fbfd7a44cd789ecf187e0edc0dcb33969d45836dd55af706424c66656b
source_last_modified: "2025-11-02T04:40:39.806222+00:00"
translation_last_reviewed: 2026-01-01
---

# Выбор релизных артефактов Iroha

Эта заметка поясняет, какие артефакты (bundles и образы контейнеров) операторам следует разворачивать для каждого профиля релиза.

## Профили

- **iroha2 (Self-hosted networks)** — конфигурация одной lane, соответствующая `defaults/genesis.json` и `defaults/client.toml`.
- **iroha3 (SORA Nexus)** — конфигурация Nexus с несколькими lanes, использующая шаблоны `defaults/nexus/*`.

## Bundles (бинарные)

Bundles создаются через `scripts/build_release_bundle.sh` с `--profile`, установленным в `iroha2` или `iroha3`.

Каждый tarball содержит:

- `bin/` — `irohad`, `iroha`, и `kagami`, собранные с профилем деплоя.
- `config/` — профильная конфигурация genesis/client (single vs. nexus). Bundles для Nexus включают `config.toml` с параметрами lanes и DA.
- `PROFILE.toml` — метаданные о профиле, конфиге, версии, commit, OS/arch, и наборе включенных features.
- Метаданные, записанные рядом с tarball:
  - `<profile>-<version>-<os>.tar.zst`
  - `<profile>-<version>-<os>.tar.zst.sha256`
  - `<profile>-<version>-<os>.tar.zst.sig` и `.pub` (если указан `--signing-key`)
  - `<profile>-<version>-manifest.json`, фиксирующий путь к tarball, hash и детали подписи

## Образы контейнеров

Образы контейнеров создаются через `scripts/build_release_image.sh` с теми же аргументами профиля/конфига.

Выходные файлы:

- `<profile>-<version>-<os>-image.tar`
- `<profile>-<version>-<os>-image.tar.sha256`
- Опциональная подпись/публичный ключ (`*.sig`/`*.pub`)
- `<profile>-<version>-image.json`, фиксирующий tag, ID образа, hash и метаданные подписи

## Выбор правильного артефакта

1. Определите поверхность деплоя:
   - **SORA Nexus / multi-lane** -> используйте bundle и image `iroha3`.
   - **Self-hosted single-lane** -> используйте артефакты `iroha2`.
   - Если сомневаетесь, запустите `scripts/select_release_profile.py --network <alias>` или `--chain-id <id>`; helper сопоставляет сети с нужным профилем по `release/network_profiles.toml`.
2. Скачайте нужный tarball и сопутствующие файлы manifest. Проверьте SHA256 и подпись перед распаковкой:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub        -signature iroha3-<version>-linux.tar.zst.sig        iroha3-<version>-linux.tar.zst
   ```
3. Извлеките bundle (`tar --use-compress-program=zstd -xf <tar>`) и поместите `bin/` в PATH деплоя. Применяйте локальные overrides конфигурации при необходимости.
4. Загрузите образ контейнера командой `docker load -i <profile>-<version>-<os>-image.tar`, если используете контейнерный деплой. Проверьте hash/подпись как выше перед загрузкой.

## Чеклист конфигурации Nexus

- `config/config.toml` должен включать секции `[nexus]`, `[nexus.lane_catalog]`, `[nexus.dataspace_catalog]`, и `[nexus.da]`.
- Убедитесь, что правила маршрутизации lanes соответствуют ожиданиям governance (`nexus.routing_policy`).
- Проверьте, что пороги DA (`nexus.da`) и параметры fusion (`nexus.fusion`) соответствуют настройкам, одобренным советом.

## Чеклист конфигурации single-lane

- `config/config.d` (если присутствует) должен содержать только single-lane overrides и не иметь секций `[nexus]`.
- Убедитесь, что `config/client.toml` указывает нужный Torii endpoint и список peers.
- Genesis должен сохранять канонические domains/assets для self-hosted сети.

## Краткая справка по инструментам

- `scripts/build_release_bundle.sh --help`
- `scripts/build_release_image.sh --help`
- `scripts/select_release_profile.py --list`
- `docs/source/sora_nexus_operator_onboarding.md` — полный onboarding поток для операторов data-space Sora Nexus после выбора артефактов.
