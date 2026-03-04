---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: промежуточный манифест-плейбук
Название: Сборник манифестов в постановке
Sidebar_label: Сборник манифестов в постановке
описание: Контрольный список для проверки размера файла, ратифицированного парламентом, на этапе Torii.
---

:::примечание Фуэнте каноника
Эта страница отражает `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Mantén ambas copyas sincronizadas.
:::

## Резюме

В этом сборнике пьес описывается, как выполнить задание, утвержденное Парламентом, и подготовить Torii к постановке перед продвижением производства. Предположим, что хартия губернатора SoraFS была ратифицирована и что канонические светильники доступны в репозитории.

## 1. Предварительные условия

1. Синхронизация канонических устройств и фирм:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Подготовьте директорию для доступа к Torii и прочтите начальный (рута примера): `/var/lib/iroha/admission/sorafs`.
3. Убедитесь, что конфигурация Torii позволяет использовать кэш обнаружения и приложение доступа:

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

## 2. Публикация о допуске

1. Скопируйте сведения о допуске к директории, ссылающейся на `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Reinicia Torii (отправьте SIGHUP, если включите загрузчик с загрузкой в источнике питания).
3. Просмотрите журналы сообщений о входе:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Поддержите пропаганду открытий

1. Публикация объявления поставщика полезной нагрузки (байты Norito), созданного для вашего конвейера поставщика:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Проконсультируйтесь с конечной точкой обнаружения и подтвердите, что реклама появляется под каноническими псевдонимами:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Убедитесь, что `profile_aliases` включает `"sorafs.sf1@1.0.0"` как первый раз.

## 4. Заблокируйте конечные точки манифеста и плана

1. Получите метаданные манифеста (требуется токен потока, если доступ активен):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Проверка и проверка JSON:
   - `chunk_profile_handle` - это `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` совпадают с отчетом о детерминизме.
   - `chunk_digests_blake3` доступен с восстановленными светильниками.

## 5. Проблемы телеметрии

- Подтвердите, что Prometheus показывает новые показатели эффективности:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Приборные панели должны быть проверены на этапе постановки под псевдонимом esperado и содержать контадоров по отключению электроэнергии в cero mientras el perfil esté activo.

## 6. Подготовка к развертыванию

1. Запишите отчет с URL-адресами, идентификатор манифеста и снимок телеметрии.
2. Сравните отчет с каналом развертывания соединения Nexus с запланированной активацией в производстве.
3. Продолжить контрольный список производства (Раздел 4 в `chunker_registry_rollout_checklist.md`) только потому, что части интересны этому прекрасному виду.

В этой книге-сценарии актуализируется то, что каждый раз развертывание фрагмента/доступа означает, что все мистические моменты определены между постановкой и производством.