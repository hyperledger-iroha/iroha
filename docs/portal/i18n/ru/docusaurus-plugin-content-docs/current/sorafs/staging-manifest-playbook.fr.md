---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: промежуточный манифест-плейбук
Название: Сборник манифестов в постановке
Sidebar_label: Сборник манифестов в постановке
описание: Контрольный список для активации блока профиля, утвержденного парламентом по развертываниям Torii de staging.
---

:::note Источник канонический
Эта страница отражена `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Получите копию Docusaurus и уцените ее, как полную копию набора Sphinx.
:::

## ансамбль

В этой книге описана активация блока профиля, ратифицированная парламентом по развертыванию Torii для постановки перед продвижением изменений в производстве. Я полагаю, что Хартия управления SoraFS ратифицирована и что канонические светильники доступны на складе.

## 1. Предварительные условия

1. Синхронизируйте канонические светильники и подписи:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Подготовьте репертуар конвертов для приема Torii лиры или демарраж (образец): `/var/lib/iroha/admission/sorafs`.
3. Убедитесь, что конфигурация Torii активна, обнаружение кэша и приложение доступа:

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. Публикация входных конвертов

1. Скопируйте одобренные конверты для приема в справочный репертуар по номеру `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Redémarrez Torii (вы отправляете SIGHUP, если вы инкапсулировали загрузчик с перезарядкой).
3. Используйте журналы для сообщений о допуске:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Проверка обнаружения распространения

1. Поместите подпись полезной нагрузки в объявлении Fournisseur (октеты Norito) по вашему конвейеру Fournisseur:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Опросите обнаружение конечной точки и подтвердите, что рекламное устройство имеет канонические псевдонимы:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Уверяем вас, что `profile_aliases` включает `"sorafs.sf1@1.0.0"` на первом входе.

## 4. Проверка манифеста и плана конечных точек

1. Восстановите метадоны манифеста (необходим маркер потока, если есть приложение):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Проверьте сортировку JSON и проверьте:
   - `chunk_profile_handle` есть `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` соответствует взаимосвязи детерминизма.
   - `chunk_digests_blake3` выровнен с обычными светильниками.

## 5. Проверка телеметрии

- Подтвердите, что Prometheus выставляет новые показатели профиля:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Панели мониторинга служат поставщиком услуг по организации мероприятий под псевдонимом посещающих и наблюдают за компьютерами по отключению электроэнергии до нуля, пока профиль активен.

## 6. Подготовка к развертыванию

1. Запишите судебный протокол с URL-адресами, идентификатором манифеста и снимком телеметрии.
2. Установите связь в канале развертывания Nexus с запланированным отверстием для активации.
3. Пройдите контрольный список производства (раздел 4 в `chunker_registry_rollout_checklist.md`) для того, чтобы стороны не подписали соглашение.

Поддерживайте эту книгу в течение дня, гарантируя, что каждая часть развертывания/допуска подходит для мемов, определяющихся между постановкой и производством.