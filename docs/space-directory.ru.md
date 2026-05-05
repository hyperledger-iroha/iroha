---
lang: ru
direction: ltr
source: docs/space-directory.md
status: complete
translator: manual
source_hash: 922c3f5794c0a665150637d138f8010859d9ccfe8ea2156a1d95ea8bc7c97ac7
source_last_modified: "2025-11-12T12:40:39.146222+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Русский перевод docs/space-directory.md (Space Directory Operator Playbook) -->

# Плейбук оператора Space Directory

В этом плейбуке описано, как создавать, публиковать, аудировать и ротировать записи
**Space Directory** для dataspace’ов Nexus. Он дополняет архитектурные заметки в
`docs/source/nexus.md` и план онбординга CBDC
(`docs/source/cbdc_lane_playbook.md`), предоставляя практические процедуры, fixtures и
шаблоны по управлению.

> **Область.** Space Directory выступает каноническим реестром manifests для dataspaces,
> UAID‑политик (Universal Account ID) и audit‑trail’а, на который опираются регуляторы.
> Хотя базовый контракт всё ещё активно дорабатывается (NX‑15), fixtures и процессы,
> описанные ниже, уже готовы к интеграции в инструменты и интеграционные тесты.

## 1. Базовые понятия

| Термин | Описание | Ссылки |
|--------|----------|--------|
| Dataspace | Контекст исполнения/lane, запускающий набор контрактов, одобренный в governance. | `docs/source/nexus.md`, `crates/iroha_data_model/src/nexus/mod.rs` |
| UAID | `UniversalAccountId` (blake2b‑32 hash), используемый для закрепления cross‑dataspace‑разрешений. | `crates/iroha_data_model/src/nexus/manifest.rs` |
| Capability Manifest | `AssetPermissionManifest`, описывающий детерминированные правила allow/deny для пары UAID/dataspace (deny имеет приоритет). | Fixture `fixtures/space_directory/capability/*.manifest.json` |
| Dataspace Profile | Метаданные по governance + DA, публикуемые вместе с manifests, чтобы операторы могли восстановить набор валидаторов, списки разрешённой композиции и audit‑хуки. | Fixture `fixtures/space_directory/profile/cbdc_lane_profile.json` |
| SpaceDirectoryEvent | Norito‑кодированные события, эмитируемые при активации/истечении/отзыве manifests. | `crates/iroha_data_model/src/events/data/space_directory.rs` |

## 2. Жизненный цикл manifest’а

Space Directory реализует **управление жизненным циклом на основе epochs**. Каждый
изменение порождает подписанный manifest‑bundle плюс событие:

| Событие | Триггер | Требуемые действия |
|---------|---------|--------------------|
| `ManifestActivated` | Новый manifest достигает `activation_epoch`. | Распространить bundle, обновить кэши, заархивировать решение governance. |
| `ManifestExpired` | `expiry_epoch` прошёл без продления. | Уведомить операторов, очистить UAID‑handles, подготовить manifest‑замену. |
| `ManifestRevoked` | Аварийное решение deny‑wins до истечения срока. | Немедленно отозвать UAID, выпустить отчёт об инциденте, запланировать повторный обзор в governance. |

Подписчики используют `DataEventFilter::SpaceDirectory` для слежения за конкретными
dataspace’ами или UAID’ами. Пример фильтра (Rust):

```rust
use iroha_data_model::events::data::filters::SpaceDirectoryEventFilter;

let filter = SpaceDirectoryEventFilter::new()
    .for_dataspace(11u32.into())
    .for_uaid("uaid:0f4d…ab11".parse().unwrap());
```

## 3. Рабочий процесс оператора

| Фаза | Ответственные | Шаги | Артефакты |
|------|---------------|------|-----------|
| Draft | Владелец dataspace | Клонировать fixture, отредактировать права/gov, выполнить `cargo test -p iroha_data_model nexus::manifest`. | Diff в Git, лог тестов. |
| Review | Governance WG | Валидировать JSON manifest’а + Norito‑bytes, подписать журнал решения. | Подписанный протокол, хеш manifest’а (BLAKE3 + Norito `.to`). |
| Publish | Lane ops | Отправить через CLI (`iroha app space-directory manifest publish`) с Norito‑payload’ом `.to` или «сырым» JSON **или** сделать POST на `/v1/space-directory/manifests` с JSON‑manifest’ом + опциональной reason; проверить ответ Torii, зафиксировать `SpaceDirectoryEvent`. | Квитанция CLI/Torii, лог событий. |
| Expire | Lane ops / Governance | Выполнить `iroha app space-directory manifest expire` (UAID, dataspace, epoch) при достижении конца срока; проверить `SpaceDirectoryEvent::ManifestExpired`, заархивировать доказательства очищения bindings. | Вывод CLI, лог событий. |
| Revoke | Governance + Lane ops | Выполнить `iroha app space-directory manifest revoke` (UAID, dataspace, epoch, reason) **или** POST `/v1/space-directory/manifests/revoke` с тем же payload в Torii, проверить `SpaceDirectoryEvent::ManifestRevoked`, обновить комплект доказательств. | Квитанция CLI/Torii, лог событий, заметка в тикете. |
| Monitor | SRE/Compliance | Отслеживать телеметрию + audit‑логи, настраивать алерты по revocation/expiry. | Скриншот Grafana, архив логов. |
| Rotate/Revoke | Lane ops + Governance | Подготовить manifest‑замену (новый epoch), провести tabletop, открыть инцидент (при revoke). | Тикет на ротацию, пост‑мортем. |

Все артефакты rollout’а хранятся под
`artifacts/nexus/<dataspace>/<timestamp>/` с manifest’ом checksums, чтобы выполнение
регуляторных запросов на доказательства было тривиальным.

## 4. Шаблон manifest’а и fixtures

Используйте подготовленные fixtures как каноническую ссылку на схему. Пример опта‑CBDC
(`fixtures/space_directory/capability/cbdc_wholesale.manifest.json`) содержит записи и
allow, и deny:

```json
{
  "version": 1,
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "issued_ms": 1762723200000,
  "activation_epoch": 4097,
  "expiry_epoch": 4600,
  "entries": [
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.transfer",
        "method": "transfer",
        "asset": "CBDC#centralbank",
        "role": "Initiator"
      },
      "effect": {
        "Allow": {
          "max_amount": "500000000",
          "window": "PerDay"
        }
      },
      "notes": "Wholesale transfer allowance (per UAID, per day)."
    },
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.kit",
        "method": "withdraw"
      },
      "effect": {
        "Deny": {
          "reason": "Withdrawals disabled for this UAID."
        }
      },
      "notes": "Deny wins over any preceding allowance."
    }
  ]
}
```

Типичная deny‑wins‑правило для онбординга CBDC могло бы выглядеть так:

```json
{
  "scope": {
    "dataspace": 11,
    "program": "cbdc.transfer",
    "method": "transfer",
    "asset": "CBDC#centralbank"
  },
  "effect": {
    "Deny": { "reason": "sanctions/watchlist match" }
  }
}
```

После публикации комбинация `uaid` + `dataspace` становится первичным ключом реестра
разрешений. Последующие manifests связываются через `activation_epoch`, и реестр
гарантирует, что последняя активная версия всегда соблюдает семантику «deny выигрывает».

## 5. API Torii

### Публикация manifest’а

Операторы могут публиковать manifests напрямую через Torii, не используя CLI.

```
POST /v1/space-directory/manifests
```

| Поле | Тип | Описание |
|------|-----|----------|
| `authority` | `AccountId` | Аккаунт, подписывающий транзакцию публикации. |
| `private_key` | `ExposedPrivateKey` | Закодированный в base64 приватный ключ, которым Torii подписывает от имени `authority`. |
| `manifest` | `SpaceDirectoryManifest` | Полный manifest (JSON), который будет закодирован в Norito на стороне сервера. |
| `reason` | `Option<String>` | Необязательное сообщение для audit‑trail, сохраняемое вместе с жизненным циклом. |

Пример JSON‑тела:

```jsonc
{
  "authority": "<i105-account-id>",
  "private_key": "ed25519:CiC7…",
  "manifest": {
    "version": 1,
    "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
    "dataspace": 11,
    "issued_ms": 1762723200000,
    "activation_epoch": 4097,
    "entries": [
      {
        "scope": {
          "dataspace": 11,
          "program": "cbdc.transfer",
          "method": "transfer",
          "asset": "CBDC#centralbank"
        },
        "effect": {
          "Allow": { "max_amount": "500000000", "window": "PerDay" }
        }
      }
    ]
  },
  "reason": "CBDC onboarding wave 4"
}
```

Torii возвращает `202 Accepted`, как только транзакция попадает в очередь. После
выполнения блока генерируется `SpaceDirectoryEvent::ManifestActivated` (с учётом
`activation_epoch`), bindings автоматически перестраиваются, а endpoint инвентаризации
manifest’ов начинает отражать новый payload. Контроль доступа соответствует остальным
write‑API Space Directory (gates по CIDR/token’у/API‑fee‑policy).

### API для отзыва manifest’а

Аварийные revocation больше не требуют запуска CLI: операторы могут отправить POST прямо
в Torii, поставив в очередь каноническую инструкцию
`RevokeSpaceDirectoryManifest`. Аккаунт‑отправитель должен иметь
`CanPublishSpaceDirectoryManifest { dataspace }`, аналогично CLI‑workflow.

```
POST /v1/space-directory/manifests/revoke
```

| Поле | Тип | Описание |
|------|-----|----------|
| `authority` | `AccountId` | Аккаунт, подписывающий транзакцию revoke. |
| `private_key` | `ExposedPrivateKey` | Приватный ключ в base64, которым Torii подписывает от имени `authority`. |
| `uaid` | `String` | Литерал UAID (`uaid:<hex>` или 64‑символьный hex‑digest, LSB=1). |
| `dataspace` | `u64` | Идентификатор dataspace, где размещён manifest. |
| `revoked_epoch` | `u64` | Epoch (включительно), с которого revocation должен вступить в силу. |
| `reason` | `Option<String>` | Необязательное аудиторское сообщение, сохраняемое вместе с данными жизненного цикла. |

Пример JSON‑тела:

```jsonc
{
  "authority": "<i105-account-id>",
  "private_key": "ed25519:CiC7…",
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "revoked_epoch": 9216,
  "reason": "Fraud investigation #NX-16-R05"
}
```

Torii возвращает `202 Accepted` после постановки транзакции в очередь. После выполнения
блока вы получите `SpaceDirectoryEvent::ManifestRevoked`, mapping `uaid_dataspaces`
будет автоматически перестроен, а `/portfolio` и инвентарь manifest’ов начнут
немедленно показывать состояние «revoked». CIDR/fee‑policy‑gates совпадают с read‑API.

## 6. Шаблон профиля dataspace’а

Профили содержат всё необходимое новому валидатору до подключения. Fixture
`profile/cbdc_lane_profile.json` документирует:

- Эмитента/кворум governance (`i105...` + ID тикета с evidence).
- Набор валидаторов + кворум и защищённые namespaces (`cbdc`, `gov`).
- DA‑профиль (класс A, список подтверждающих лиц, период ротации).
- ID группы composability и whitelist, связывающий UAID’ы с capability‑manifest’ами.
- Audit‑хуки (список событий, схема логов, сервис PagerDuty).

Используйте JSON как стартовую точку для новых dataspaces и адаптируйте пути в
whitelist под соответствующие manifests.

## 7. Публикация и ротация

1. **Кодирование UAID.** Вычислите blake2b‑32‑digest и добавьте префикс `uaid:`:

   ```bash
   python3 - <<'PY'
   import hashlib, binascii
   seed = bytes.fromhex("0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11")
   print("uaid:" + hashlib.blake2b(seed, digest_size=32).hexdigest())
   PY
   ```

2. **Кодирование Norito‑payload’а.**

   ```bash
   cargo xtask space-directory encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

   Или воспользуйтесь CLI‑helper’ом (он создаст и `.to`‑файл, и `.hash` с
   BLAKE3‑256‑digest’ом):

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

3. **Публикация через Torii.**

   ```bash
   # Если manifest уже закодирован в Norito:
   iroha app space-directory manifest publish \
     --uaid uaid:0f4d…ab11 \
     --dataspace 11 \
     --payload artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

   Либо используйте HTTP‑API, описанное выше. В любом случае следует архивировать:
   - исходный JSON manifest’а;
   - Norito‑payload `.to` и BLAKE3‑хеш;
   - ответ Torii и событие `SpaceDirectoryEvent`.

4. **Ротация или отзыв.**

   - Для плановой ротации: подготовьте manifest‑замену с будущим `activation_epoch`,
     выполните tabletop‑учения и согласуйте активацию с операторами всех затронутых
     dataspaces.
   - Для аварийного revoke: следуйте плейбуку Revocation (фаза “Revoke” в таблице
     workflow), укажите ID инцидента в `reason` и сохраните соответствующие логи и
     snapshots.

При соблюдении этого плейбука Space Directory предоставляет каноничный, проверяемый и
ориентированный на регуляторов реестр того, как права и возможности выдаются, ограничиваются
и отзываются во всех dataspaces Nexus.
