---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: промежуточный манифест-плейбук
title: SoraFS книга промежуточного манифеста
Sidebar_label: SoraFS книга промежуточного манифеста
описание: Torii کی промежуточное развертывание پر Утвержденный парламентом профиль chunker فعال کرنے کے لیے контрольный список۔
---

:::примечание
:::

## Обзор

Развертывание playbook staging Torii پر Утвержденный парламентом профиль chunker میں продвигать کرنے سے پہلے تصدیق ہو سکے۔ یہ فرض کرتا ہے کہ SoraFS ратификация устава управления ہو چکا ہے اور канонический репозиторий светильников میں موجود ہیں۔

## 1. Предварительные условия

1. Канонические светильники и синхронизация подписей:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Входные конверты и каталог, где находится Torii, где находится запуск (пример пути): `/var/lib/iroha/admission/sorafs`.
3. Установите кэш обнаружения конфигурации Torii и принудительное допуск, чтобы включить функцию:

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

## 2. На входных конвертах публикуется کریں

1. Конверты одобренного поставщика услуг: `torii.sorafs.discovery.admission.envelopes_dir` — каталог, копия:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii перезапустите (если есть горячая перезагрузка загрузчика или обертка для SIGHUP).
3. Сообщения о входе в журналы хвоста:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Проверка распространения обнаружения

1. Конвейер провайдера содержит подписанную полезную нагрузку объявления провайдера (Norito байт) или сообщение:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Запрос конечной точки обнаружения для подтверждения или объявления канонических псевдонимов или для подтверждения:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Введите `profile_aliases` и `"sorafs.sf1@1.0.0"` для входа в систему.

## 4. Упражнение по составлению плана конечных точек.

1. Получение метаданных манифеста (принудительный допуск к потоку или токен потока):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Проверка вывода JSON и проверка правильности:
   - `chunk_profile_handle`, `sorafs.sf1@1.0.0` ہو۔
   - Отчет о детерминизме `manifest_digest_hex` سے соответствует کرے۔
   - `chunk_digests_blake3` регенерированные светильники سے align ہوں۔

## 5. Проверка телеметрии

- Если у вас есть Prometheus, метрики профиля показывают:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Панели мониторинга, предполагаемый псевдоним промежуточного поставщика, или профиль, или счетчики отключений, или счетчики отключений, или счетчики отключений.

## 6. Готовность к развертыванию

1. URL-адреса, идентификатор манифеста, а также моментальный снимок телеметрии.
2. Установите канал развертывания Nexus и запланируйте окно активации производства.
3. Заинтересованные стороны должны подписать контрольный список производственного процесса (раздел 4 в `chunker_registry_rollout_checklist.md`).

В учебном пособии можно найти блокировщик/развертывание допуска, промежуточное производство и детерминированные шаги. چلتا ہے۔