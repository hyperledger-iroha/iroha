---
lang: ru
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2026-01-03T18:07:58.297179+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: staging-manifest-playbook-ru
slug: /sorafs/staging-manifest-playbook-ru
---

:::обратите внимание на канонический источник
Зеркала `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Обе копии должны быть согласованы между выпусками.
:::

## Обзор

В этом руководстве рассматривается включение профиля чанкера, утвержденного парламентом, в промежуточном развертывании Torii перед внедрением изменения в рабочую среду. Предполагается, что устав управления SoraFS ратифицирован и канонические настройки доступны в репозитории.

## 1. Предварительные условия

1. Синхронизируйте канонические фикстуры и подписи:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Подготовьте каталог конверта допуска, который Torii будет читать при запуске (пример пути): `/var/lib/iroha/admission/sorafs`.
3. Убедитесь, что конфигурация Torii включает кэш обнаружения и принудительное допуск:

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

1. Скопируйте утвержденные конверты приема поставщиков в каталог, указанный в `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Перезапустите Torii (или отправьте SIGHUP, если вы завернули загрузчик с перезагрузкой на лету).
3. Сохраните журналы сообщений о допуске:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Проверка распространения обнаружения

1. Опубликуйте подписанную полезную нагрузку объявления провайдера (Norito байт), созданную вашим
   конвейер провайдера:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Запросите конечную точку обнаружения и подтвердите, что реклама отображается с каноническими псевдонимами:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Убедитесь, что `profile_aliases` включает `"sorafs.sf1@1.0.0"` в качестве первой записи.

## 4. Манифест учений и конечные точки плана

1. Получите метаданные манифеста (требуется токен потока, если доступ обязателен):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Проверьте вывод JSON и убедитесь:
   - `chunk_profile_handle` — это `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` соответствует отчету о детерминизме.
   - `chunk_digests_blake3` совместить с регенерированными светильниками.

## 5. Проверка телеметрии

- Подтвердите, что Prometheus предоставляет новые показатели профиля:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- На информационных панелях промежуточный поставщик должен отображаться под ожидаемым псевдонимом, а счетчики отключений должны быть равны нулю, пока профиль активен.

## 6. Готовность к развертыванию

1. Создайте короткий отчет с URL-адресами, идентификатором манифеста и снимком телеметрии.
2. Опубликуйте отчет в канале развертывания Nexus рядом с запланированным окном производственной активации.
3. Перейдите к производственному контрольному списку (раздел 4 в `chunker_registry_rollout_checklist.md`) после того, как заинтересованные стороны подпишут его.

Постоянное обновление этого сборника правил гарантирует, что каждое развертывание чанка/допуска будет следовать одним и тем же детерминированным шагам на этапе подготовки и производства.
