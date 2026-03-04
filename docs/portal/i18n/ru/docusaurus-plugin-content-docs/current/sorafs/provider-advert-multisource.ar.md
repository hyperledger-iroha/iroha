---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# إعلانات مزودي متعدد المصادر والجدولة

تُلخص هذه الصفحة المواصفة القياسية في
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
استخدم هذا المستند للمخططات Norito حرفيا وسجلات التغيير؛ نسخة البوابة تُبقي إرشادات المشغلين
SDK был создан для использования в системе SoraFS.

## إضافات مخطط Norito

### قدرة النطاق (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – Зарегистрируйтесь (по запросу) для `>= 1`.
- `min_granularity` – دقة البحث, `1 <= القيمة <= max_chunk_span`.
- `supports_sparse_offsets` – يسمح بإزاحات غير متصلة في طلب واحد.
- `requires_alignment` – используется для подключения к `min_granularity`.
- `supports_merkle_proof` – добавлено в PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` в случае возникновения проблем
В تبقى payloads и сплетнях.

### `StreamBudgetV1`
- Коды: `max_in_flight`, `max_bytes_per_sec`, و`burst_bytes`.
- Код запроса (`StreamBudgetV1::validate`):
  - И18НИ00000027Х, И18НИ00000028Х.
  - `burst_bytes` находится под управлением `> 0` и `<= max_bytes_per_sec`.

### `TransportHintV1`
- Коды: `protocol: TransportProtocol`, `priority: u8` (0–15 дней).
  `TransportHintV1::validate`).
- Дополнительные сведения: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- تُرفض إدخالات البروتوكول المكررة لكل مزود.

### إضافات `ProviderAdvertBodyV1`
- `stream_budget` Сообщение: `Option<StreamBudgetV1>`.
- `transport_hints` Сообщение: `Option<Vec<TransportHintV1>>`.
- كلا الحقلين يمران الآن عبر `ProviderAdmissionProposalV1` وأغلفة الحوكمة
  Встроенные средства CLI и JSON.

## التحقق والربط بالحوكمة

`ProviderAdvertBodyV1::validate` و`ProviderAdmissionProposalV1::validate`
В сообщении говорится:

- Он был свидетелем того, как в 1990-х годах он был убит в 2007 году.
- Просмотр бюджетов потоков / подсказок по транспортировке в TLV مطابقة من
  `CapabilityType::ChunkRangeFetch` содержит подсказки для проверки.
- Посетите веб-сайт, посвященный рекламе.
- تقارن أغلفة القبول предложение/объявления لبيانات النطاق عبر `compare_core_fields`
  حتى تُرفض payloads и сплетни в Интернете.

توجد تغطية الانحدار في
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Настройки и светильники

- Для полезных нагрузок используются `range_capability`, `stream_budget` и `transport_hints`.
  تحقّق عبر استجابات `/v1/sorafs/providers` и светильники القبول؛ يجب أن تتضمن
  В JSON-файле для просмотра изображений и подсказок о необходимости изменения.
- `cargo xtask sorafs-admission-fixtures` — потоковые бюджеты и подсказки по транспортировке.
  артефакты JSON для создания файлов в формате JSON.
- Дополнительные светильники `fixtures/sorafs_manifest/provider_admission/`:
  - рекламные объявления
  - `multi_fetch_plan.json` позволяет получить доступ к SDK, чтобы получить необходимые файлы.

## تكامل المُنسق وTorii- يعيد Torii `/v1/sorafs/providers` بيانات قدرة النطاق المحللة مع
  `stream_budget` и `transport_hints`. تُطلق تحذيرات понизить рейтинг
  В 2017 году в 2017 году в 2017 году в 2017 году в 2017 году было проведено 2000-е годы. المباشرين.
- Встроенное программное обеспечение (`sorafs_car::multi_fetch`)
  Управляйте потоковыми бюджетами в любое время. تغطي اختبارات الوحدة سيناريوهات
  кусок الكبير جدا والبحث التناثر والتخفيض.
- يبث `sorafs_car::multi_fetch` и понижение версии (إخفاقات المحاذاة،
  (справа) أثناء التخطيط.

## مرجع التليمترية

Выполните команду fetch в Torii для Grafana **SoraFS Fetch Observability**
(`dashboards/grafana/sorafs_fetch_observability.json`)
(`dashboards/alerts/sorafs_fetch_rules.yml`).

| المقياس | النوع | الوسوم | الوصف |
|---------|-------|--------|-------|
| `torii_sorafs_provider_range_capability_total` | Калибр | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Он был убит в Нью-Йорке. |
| `torii_sorafs_range_fetch_throttle_events_total` | Счетчик | `reason` (`quota`, `concurrency`, `byte_rate`) | Он принесет Лилу, чтобы он сказал Дэвису. |
| `torii_sorafs_range_fetch_concurrency_current` | Калибр | — | Он был убит в 1990-х годах в Нью-Йорке. |

Обратитесь к PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

استخدم عداد التخفيض لتأكيد تطبيق قبل تفعيل القيم الافتراضية لمُنسق متعدد المصادر،
Он Сэнсэй Уилсон в фильме "Линия" в Лос-Анджелесе.