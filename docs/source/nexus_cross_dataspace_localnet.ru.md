<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus Межпространственное доказательство локальной сети

Этот модуль Runbook выполняет доказательство интеграции Nexus, которое:

- загружает четырехранговую локальную сеть с двумя ограниченными частными пространствами данных (`ds1`, `ds2`),
- маршрутизирует трафик учетной записи в каждое пространство данных,
- создает актив в каждом пространстве данных,
- выполняет расчет атомарного свопа между пространствами данных в обоих направлениях,
- подтверждает семантику отката, отправляя недофинансированную часть и проверяя, что балансы остаются неизменными.

Канонический тест:
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## Быстрый бег

Используйте скрипт-оболочку из корня репозитория:

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

Поведение по умолчанию:

- запускает только контрольный тест между пространствами данных,
- комплекты `NORITO_SKIP_BINDINGS_SYNC=1`,
- комплекты `IROHA_TEST_SKIP_BUILD=1`,
- использует `--test-threads=1`,
- проходит `--nocapture`.

## Полезные опции

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` хранит временные одноранговые каталоги (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) для криминалистической экспертизы.
- `--all-nexus` запускает `mod nexus::` (полное подмножество интеграции Nexus), а не только контрольный тест.

## CI-шлюз

CI-помощник:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

Сделать цель:

```bash
make check-nexus-cross-dataspace
```

Этот вентиль выполняет оболочку детерминированного доказательства и завершает задание неудачно, если атомарное
сценарий обмена регрессирует.

## Эквивалентные команды вручную

Целевое контрольное испытание:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

Полное подмножество Nexus:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## Ожидаемые подтверждающие сигналы- Тест пройден.
- Одно ожидаемое предупреждение появляется для намеренно неудачной части расчета с недостаточным финансированием:
  `settlement leg requires 10000 but only ... is available`.
- Окончательные утверждения баланса успешны после:
  - успешный форвардный своп,
  - успешный обратный обмен,
  - неудавшийся недофинансированный своп (откат неизменившихся остатков).

## Текущий снимок проверки

По состоянию на **19 февраля 2026 г.** этот рабочий процесс выполнялся следующим образом:

- целевой тест: `1 passed; 0 failed`,
- полное подмножество Nexus: `24 passed; 0 failed`.