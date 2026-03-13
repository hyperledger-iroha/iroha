---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: промежуточный манифест-плейбук
Название: Пособие по постановке манифеста
Sidebar_label: Сборник манифестов при простановке
описание: Контрольный список для проверки или выполнения блока, утвержденного парламентом при развертывании Torii на этапе подготовки.
---

:::примечание Fonte canonica
Эта страница написана `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Мантенья представился как копиас синхронизадас.
:::

## Визао гераль

Эта книга действий предназначена как привычный или неэффективный блокировщик, ратифицированный парламентом в развертывании Torii для постановки перед продвижением к производству. Мы предполагаем, что хартия управления от SoraFS ратифицирована и что канонические светильники не доступны в хранилище.

## 1. Предварительные условия

1. Синхронизируйте канонические и ассинатурные светильники:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Подготовьте папку с входными конвертами для Torii при запуске (пример): `/var/lib/iroha/admission/sorafs`.
3. Гарантия, что конфигурация Torii позволяет использовать кэш обнаружения и принудительное допуск:

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

## 2. Приемные конверты Publicar

1. Скопируйте конверты приема поставщика, утвержденные для справочной информации по `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Reinicie или Torii (вы завидуете SIGHUP, когда вы голосуете или загрузчик с горячей перезагрузкой).
3. Сопровождайте журналы сообщений о входе:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Поддержите пропаганду открытий

1. Публикация или объявление поставщика полезной нагрузки (байты Norito) для создания конвейера:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. Проконсультируйтесь с конечной точкой обнаружения и подтвердите, что объявление появляется под каноническими псевдонимами:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   Гарантия, что `profile_aliases` включает `"sorafs.sf1@1.0.0"` в качестве первого входа.

## 4. Выполнение конечных точек манифеста и плана

1. Создайте манифест метаданных (принудительный доступ к токену потока exige):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Проверка и проверка JSON:
   - `chunk_profile_handle` и `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` соответствует отношению детерминизма.
   - `chunk_digests_blake3` alinham с восстановленными светильниками.

## 5. Проверка телеметрии

- Подтвердите, что Prometheus показывает, как новые метрики выполняют фильтр:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Панели мониторинга предназначены для провайдеров промежуточных событий или псевдонимов esperado и manter os contadores de lowout em нулевой результат или perfil estiver ativo.

## 6. Готовность к развертыванию

1. Запишите краткие URL-адреса, идентификатор манифеста и снимок телеметрии.
2. Сравните канал развертывания Nexus с каналом запуска в производстве.
3. Введите контрольный список производства (Раздел 4 в `chunker_registry_rollout_checklist.md`), когда это будет подтверждено заинтересованными сторонами.

Эта интерактивная книга действий гарантирует, что каждое развертывание фрагмента/допуска будет выполнено в соответствии с определенными проходами между постановкой и производством.