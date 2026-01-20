---
lang: ru
direction: ltr
source: docs/source/runbooks/address_manifest_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb5d84c6939c186ebb4cd1b622e5ab66872349f5c177191c940a9e9fd63d1a17
source_last_modified: "2025-12-14T09:53:36.233782+00:00"
translation_last_reviewed: 2025-12-28
---

# Ранбук операций с манифестом адресов (ADDR-7c)

Этот ранбук операционализирует пункт дорожной карты **ADDR-7c**, описывая, как
проверять, публиковать и выводить из обращения записи в манифесте аккаунтов/алиасов
Sora Nexus. Он дополняет технический контракт в
[`docs/account_structure.md`](../../account_structure.md) §4 и ожидания по
телеметрии, зафиксированные в `dashboards/grafana/address_ingest.json`.

## 1. Область и входные данные

| Вход | Источник | Примечания |
|-------|--------|-------|
| Подписанный пакет манифеста (`manifest.json`, `manifest.sigstore`, `checksums.sha256`, `notes.md`) | Pin SoraFS (`sorafs://address-manifests/<CID>/`) и HTTPS‑зеркало | Пакеты формируются автоматизацией релизов; сохраняйте структуру каталогов при зеркалировании. |
| Digest + последовательность предыдущего манифеста | Предыдущий пакет (тот же шаблон пути) | Нужно для доказательства монотонности/неизменяемости. |
| Доступ к телеметрии | Dashboard Grafana `address_ingest` + Alertmanager | Нужен для контроля вывода Local‑8 и всплесков неверных адресов. |
| Инструменты | `cosign`, `shasum`, `b3sum` (или `python3 -m blake3`), `jq`, CLI `iroha`, `scripts/account_fixture_helper.py` | Установите перед запуском чек‑листа. |

## 2. Структура артефактов

Каждый пакет следует структуре ниже; не переименовывайте файлы при копировании
между окружениями.

```
address-manifest-<REVISION>/
├── manifest.json              # canonical JSON (UTF-8, newline-terminated)
├── manifest.sigstore          # Sigstore bundle from `cosign sign-blob`
├── checksums.sha256           # one-line SHA-256 sum for each artifact
└── notes.md                   # change log (reason codes, tickets, owners)
```

Поля заголовка `manifest.json`:

| Поле | Описание |
|-------|-------------|
| `version` | Версия схемы (сейчас `1`). |
| `sequence` | Монотонный номер ревизии; должен увеличиваться ровно на единицу. |
| `generated_ms` | UTC‑временная метка публикации (миллисекунды с эпохи). |
| `ttl_hours` | Максимальный срок кэша, который могут соблюдать Torii/SDK (по умолчанию 24). |
| `previous_digest` | BLAKE3 тела предыдущего манифеста (hex). |
| `entries` | Упорядоченный массив записей (`global_domain`, `local_alias` или `tombstone`). |

## 3. Процедура проверки

1. **Скачать пакет.**

   ```bash
   export REV=2025-04-12
   sorafs_cli fetch --id sorafs://address-manifests/${REV} --out artifacts/address_manifest_${REV}
   cd artifacts/address_manifest_${REV}
   ```

2. **Проверка чек‑сумм.**

   ```bash
   shasum -a 256 -c checksums.sha256
   ```

   Все файлы должны вернуть `OK`; несоответствия трактуйте как подмену.

3. **Верификация Sigstore.**

   ```bash
   cosign verify-blob \
     --bundle manifest.sigstore \
     --certificate-identity-regexp 'governance\.sora\.nexus/addr-manifest' \
     --certificate-oidc-issuer https://accounts.google.com \
     manifest.json
   ```

4. **Доказательство неизменяемости.** Сравните `sequence` и `previous_digest` с
   архивным манифестом:

   ```bash
   jq '.sequence, .previous_digest' manifest.json
   b3sum -l 256 ../address-manifest_<prev>/manifest.json
   ```

   Выведенный digest должен совпасть с `previous_digest`. Разрывы последовательности
   недопустимы; при нарушении переиздайте манифест.

5. **Соблюдение TTL.** Убедитесь, что `generated_ms + ttl_hours` покрывает
   плановые окна деплоя; иначе governance должна переопубликовать до истечения кэша.

6. **Проверка записей.**
   - Записи `global_domain` ДОЛЖНЫ включать `{ "domain": "example", "chain": "sora:nexus:global", "selector": "global" }`.
   - Записи `local_alias` ДОЛЖНЫ содержать 12‑байтный digest, сформированный Norm v1
     (проверьте `iroha address convert <address-or-account_id> --format json --expect-prefix 753`;
     JSON‑сводка отражает домен через `input_domain`, а `--append-domain` воспроизводит кодировку как `<ih58>@<domain>` для манифестов).
   - Записи `tombstone` ДОЛЖНЫ ссылаться на точный selector, подлежащий выводу,
     и содержать поля `reason_code`, `ticket`, `replaces_sequence`.

7. **Паритет fixtures.** Перегенерируйте канонические векторы и убедитесь, что
   таблица Local‑digest не изменилась неожиданно:

   ```bash
   cargo xtask address-vectors
   python3 scripts/account_fixture_helper.py check --quiet
   ```

8. **Автоматизированный guardrail.** Запустите проверку манифеста для повторной
   валидации пакета end‑to‑end (схема заголовка, форма записей, checksums и
   связка previous‑digest):

   ```bash
   cargo xtask address-manifest verify \
     --bundle artifacts/address-manifest_2025-05-12 \
     --previous artifacts/address-manifest_2025-04-30
   ```

   Параметр `--previous` указывает на непосредственно предыдущий пакет, чтобы
   инструмент подтвердил монотонность `sequence` и пересчитал BLAKE3‑доказательство
   `previous_digest`. Команда быстро падает при расхождении checksum или при
   отсутствии обязательных полей у `tombstone`, поэтому прикладывайте вывод к
   тикету изменений до запроса подписей.

## 4. Поток изменений alias и tombstone

1. **Предложить изменение.** Создайте тикет governance с кодом причины
   (`LOCAL8_RETIREMENT`, `DOMAIN_REASSIGNED` и т. д.) и затронутыми selectors.
2. **Вывести канонические payload.** Для каждого alias выполните:

   ```bash
   iroha address convert snx1...@wonderland --expect-prefix 753 --format json > /tmp/alias.json
   jq '.canonical_hex, .input_domain' /tmp/alias.json
   ```

3. **Черновик записи манифеста.** Добавьте JSON‑запись вида:

   ```json
   {
     "type": "tombstone",
     "selector": { "kind": "local", "digest_hex": "b18fe9c1abbac45b3e38fc5d" },
     "reason_code": "LOCAL8_RETIREMENT",
     "ticket": "ADDR-7c-2025-04-12",
     "replaces_sequence": 36
   }
   ```

   При замене Local alias на Global добавьте и `tombstone`, и последующую запись
   `global_domain` с дискриминантом Nexus.

4. **Валидация пакета.** Повторите шаги проверки выше для чернового манифеста
   перед запросом подписей.
5. **Публикация и мониторинг.** После подписи governance следуйте §3 и держите
   `true` на прод‑кластерах, когда метрики подтверждают нулевое использование Local‑8.
   Меняйте флаг на `false` только на dev/test кластерах при необходимости доп. времени на soak.

## 5. Мониторинг и rollback

- Dashboards: `dashboards/grafana/address_ingest.json` (панели для
  `torii_address_invalid_total{endpoint,reason}`,
  `torii_address_local8_total{endpoint}`,
  `torii_address_collision_total{endpoint,kind="local12_digest"}`, и
  `torii_address_collision_domain_total{endpoint,domain}`) должны оставаться
  зелёными 30 дней перед постоянным блокированием трафика Local‑8/Local‑12.
- Доказательство gate: экспортируйте 30‑дневный диапазон Prometheus для
  `torii_address_local8_total` и `torii_address_collision_total` (например,
  `promtool query range --output=json ...`) и выполните
  `cargo xtask address-local8-gate --input <file> --json-out artifacts/address_gate.json`;
  приложите JSON + вывод CLI к тикетам rollout, чтобы governance увидела окно
  покрытия и подтвердила неизменность счётчиков.
- Оповещения (см. `dashboards/alerts/address_ingest_rules.yml`):
  - `AddressLocal8Resurgence` — пейджинг при любом новом инкременте Local‑8.
    Считайте блокером релиза, остановите strict‑mode rollout и временно выставьте
    Верните флаг в `true`, когда телеметрия чистая.
  - `AddressLocal12Collision` — срабатывает, когда две метки Local‑12 хешируются
    в один digest. Приостановите промо манифеста, запустите
    `scripts/address_local_toolkit.sh` для проверки маппинга digest и
    согласуйте с governance Nexus до переиздания записи.
  - `AddressInvalidRatioSlo` — предупреждает, когда доля неверных IH58/сжатых
    отправок (за исключением Local‑8/strict‑mode отказов) превышает 0,1 % SLO
    на протяжении 10 минут. Проверьте `torii_address_invalid_total` по контексту/причине
    и согласуйте с SDK‑командой до повторного включения strict‑mode.
- Логи: сохраните строки `manifest_refresh` Torii и номер governance‑тикета в `notes.md`.
- Rollback: переопубликуйте предыдущий пакет (те же файлы, тикет с пометкой rollback)
  до устранения проблемы, затем верните `true`.

## 6. Ссылки

- [`docs/account_structure.md`](../../account_structure.md) §§4–4.1 (контракт).
- [`scripts/account_fixture_helper.py`](../../../scripts/account_fixture_helper.py) (синхронизация fixtures).
- [`fixtures/account/address_vectors.json`](../../../fixtures/account/address_vectors.json) (канонические digests).
- [`dashboards/grafana/address_ingest.json`](../../../dashboards/grafana/address_ingest.json) (телеметрия).
