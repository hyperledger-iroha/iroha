---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: промежуточный манифест-плейбук
Название: دليل مانيفست الـstaging
Sidebar_label: دليل مانيفست الـstaging
описание: قائمة تحقق لتمكين ملف chunker المصادق عليه برلمانياً على نشرات Torii الخاصة Постановка.
---

:::примечание
Был установлен `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Для этого используйте Docusaurus и используйте Markdown для получения дополнительной информации. Создан Сфинкс.
:::

## نظرة عامة

Он был убит Джоном Чанкером в 2007 году в 18NT00000006X. Он организовал постановку спектакля "Старый мир". يفترض أن ميثاق حوكمة SoraFS تم التصديق عليه وأن الـ светильники المعتمدة متاحة داخل المستودع.

## 1. Настройки

1. Матчи предстоящих матчей:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Установите флажок Torii для подключения (отсутствие): `/var/lib/iroha/admission/sorafs`.
3. Вызовите Torii для обнаружения открытия:

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

## 2. نشر أظرف القبول

1. Установите флажок для подключения к сети `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Установите флажок Torii (зарегистрируйтесь в SIGHUP для проверки работоспособности системы). الفورية).
3. راقب السجلات لرسائل القبول:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Открытие нового открытия

1. Чтобы просмотреть рекламу поставщика услуг (بايتات Norito), выполните следующие действия:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Результаты открытия, сделанные в ходе исследования, посвященного исследованию:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Он был установлен на `profile_aliases` и `"sorafs.sf1@1.0.0"` на сервере.

## 4. Создание манифеста и плана

1. Манифест اجلب بيانات الوصفية (токен потока يتطلب إذا كان القبول مفعّلًا):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Загрузить JSON-файл:
   - Вместо `chunk_profile_handle` или `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` находится в режиме ожидания.
   - أن `chunk_digests_blake3` تتطابق مع الـ светильники المعاد توليدها.

## 5. Свободное время

- Сообщение от Prometheus в случае необходимости:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Он выступил в роли режиссера в постановке спектакля "Старый мир" и "Санкт-Петербург". Произошло отключение электроэнергии в штатном режиме.

## 6. Устранение неполадок

1. Создайте URL-адрес манифеста, созданного для проверки.
2. Выполните развертывание Nexus для последующего обновления.
3. Выполните процедуру проверки подлинности (Раздел 4 в `chunker_registry_rollout_checklist.md`). المصلحة.

الحفاظ على هذا الدليل محدثًا يضمن أن كل إطلاق لـ chunker/admission يتبع نفس الخطوات Он организовал постановку «Игры».