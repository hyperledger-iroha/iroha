---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 45850b658b5ae8cfb1ab2541e226e3674c66e32ae7b377d1d50ddd2ef168a4f6
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: staging-manifest-playbook
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Канонический источник
:::

## Обзор

Этот плейбук описывает включение профиля chunker, ратифицированного парламентом, на staging-развертывании Torii перед продвижением изменений в прод. Предполагается, что устав управления SoraFS ратифицирован, а канонические fixtures доступны в репозитории.

## 1. Предварительные условия

1. Синхронизируйте канонические fixtures и подписи:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Подготовьте каталог admission envelopes, который Torii читает при старте (пример пути): `/var/lib/iroha/admission/sorafs`.
3. Убедитесь, что конфиг Torii включает discovery cache и enforcement admission:

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

## 2. Публикация admission envelopes

1. Скопируйте утвержденные provider admission envelopes в каталог, указанный в `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Перезапустите Torii (или отправьте SIGHUP, если вы обернули загрузчик hot reload).
3. Следите за логами admission:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Проверка распространения discovery

1. Опубликуйте подписанный provider advert payload (байты Norito), сформированный вашим pipeline провайдера:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Запросите endpoint discovery и убедитесь, что advert отображается с каноническими aliases:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Убедитесь, что `profile_aliases` содержит `"sorafs.sf1@1.0.0"` первым элементом.

## 4. Проверка endpoints manifest и plan

1. Получите metadata manifest (требуется stream token, если admission enforced):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Проверьте JSON-вывод и убедитесь, что:
   - `chunk_profile_handle` равен `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` совпадает с отчетом детерминизма.
   - `chunk_digests_blake3` совпадают с регенерированными fixtures.

## 5. Проверки телеметрии

- Убедитесь, что Prometheus публикует новые метрики профиля:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Дашборды должны показывать staging-провайдера под ожидаемым alias и держать счетчики brownout на нуле, пока профиль активен.

## 6. Готовность к rollout

1. Сформируйте короткий отчет с URL-адресами, ID manifest и snapshot телеметрии.
2. Поделитесь отчетом в канале Nexus rollout вместе с запланированным окном активации в продакшене.
3. Переходите к продакшен-чеклисту (Section 4 в `chunker_registry_rollout_checklist.md`) после согласования со стейкхолдерами.

Поддержание этого плейбука в актуальном состоянии гарантирует, что каждый rollout chunker/admission следует одним и тем же детерминированным шагам между staging и production.
