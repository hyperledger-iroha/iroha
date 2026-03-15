---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: промежуточный манифест-плейбук
Название: Плейбук манифеста для постановки
sidebar_label: Плейбук манифеста для постановки
описание: Чеклист для включения чанкера профиля, ратифицированного парламентом, на staging-развертываниях Torii.
---

:::note Канонический источник
:::

## Обзор

В этом плейбуке описано включение чанкера профиля, ратифицированного парламентом, на постановке-развертывании Torii перед продвижением изменений в прод. Предполагается, что устав управления SoraFS ратифицирован, а канонические приспособления доступны в репозиториях.

## 1. Предварительные условия

1. Синхронизируйте канонические приспособления и подключите:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Подготовьте каталог входных конвертов, который Torii читает при старте (пример пути): `/var/lib/iroha/admission/sorafs`.
3. Убедитесь, что конфиг Torii включает кэш обнаружения и принудительное допуск:

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

## 2. Публикация приемных конвертов

1. Скопируйте утвержденные конверты приема провайдера в каталоге, указанном в `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Перезапустите Torii (или отправьте SIGHUP, если вы обернули загрузчик с горячей перезагрузкой).
3. Следите за входом в систему:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Проверка обнаружения распространения

1. Опубликуйте подписанный провайдером рекламный контент (байты Norito), сформированный вашим конвейером провайдера:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Запросите обнаружение конечной точки и убедитесь, что реклама отображается с каноническими псевдонимами:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Убедитесь, что `profile_aliases` содержит первый элемент `"sorafs.sf1@1.0.0"`.

## 4. Проверка манифеста и плана конечных точек

1. Получите манифест метаданных (требуется токен потока, если принудительный доступ):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Проверьте JSON-вывод и убедитесь, что:
   - `chunk_profile_handle` вокруг `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` соответствует отчету детерминизма.
   - `chunk_digests_blake3` произошли с регенерированными светильниками.

## 5. Проверка телеметрии

- Убедитесь, что Prometheus публикует новые метрики профиля:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Дашборды должны показывать промежуточный провайдер под ожидаемым псевдонимом и вести счетчики затемнения на нуле, пока профиль активен.

## 6. Готовность к внедрению

1. Сформируйте краткий отчет с URL-адресами, манифестом идентификатора и снимком телеметрии.
2. Поделитесь отчетом в канале Nexus развертывания вместе с запланированным окном активации в продакшене.
3. Переходите к продакшен-чеклисту (Раздел 4 в `chunker_registry_rollout_checklist.md`) после согласования со стейкхолдерами.

Поддержание этого плейбука в актуальном состоянии гарантирует, что каждый блокировщик/прием развертывания следует одному и тем же определенным шагам между постановкой и производством.