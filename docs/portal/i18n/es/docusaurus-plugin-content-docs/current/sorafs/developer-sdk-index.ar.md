---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: índice-sdk-desarrollador
título: SDK de SoraFS
sidebar_label: SDK gratuito
descripción: مقتطفات خاصة بكل لغة لدمج آرتيفاكتات SoraFS.
---

:::nota المصدر المعتمد
Utilice el código `docs/source/sorafs/developer/sdk/index.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

Para obtener más información, consulte las instrucciones del fabricante SoraFS.
Para instalar Rust en el SDK de Rust] (./developer-sdk-rust.md).

## مساعدات اللغات- **Python** — `sorafs_multi_fetch_local` (اختبارات دخان للمُنسِّق المحلي) y
  `sorafs_gateway_fetch` (تمارين E2E للبوابة) يقبلان الآن `telemetry_region` اختياريًا
  Nombre del producto `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` y `"direct-only"`)
  CLI. Utilice el servidor Proxy QUIC para instalar `sorafs_gateway_fetch`.
  `local_proxy_manifest` حتى تتمكن الاختبارات من تمرير paquete de confianza إلى محولات المتصفح.
- **JavaScript** — يعكس `sorafsMultiFetchLocal` مساعد Python y بايتات الحمولة y ملخصات
  Dispositivos proxy `sorafsGatewayFetch`, Torii y servidores proxy
  Utilice la CLI para acceder a los archivos/dispositivos de conexión.
- **Rust** — يمكن للخدمات تضمين المُجدول مباشرةً عبر `sorafs_car::multi_fetch`؛ راجع
  [مقتطفات Rust SDK](./developer-sdk-rust.md) Para el flujo de prueba y el flujo de prueba.
- **Android** — يعيد `HttpClientTransport.sorafsGatewayFetch(…)` استخدام مُنفّذ HTTP الخاص
  Hay Torii y hay `GatewayFetchOptions`. ادمجه مع
  `ClientConfig.Builder#setSorafsGatewayUri` Y تلميح رفع PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) عندما يجب أن تلتزم الرفوعات
  بمسارات PQ فقط.

## marcador de مفاتيح والسياسات

Utiliza Python (`sorafs_multi_fetch_local`) y JavaScript
(`sorafsMultiFetchLocal`) Marcador de pantalla الواعية بالتليمترية التي يستخدمها CLI:- تمكّن الثنائيات الإنتاجية marcador افتراضيًا؛ Fuente `use_scoreboard=True`
  (أو وفّر إدخالات `telemetry`) عند إعادة تشغيل accesorios حتى يستخلص المساعد ترتيب
  المزوّدين الموزون من بيانات anuncios y لقطات التليمترية الحديثة.
- اضبط `return_scoreboard=True` لتلقي الأوزان المحسوبة مع إيصالات الـ chunk حتى تتمكن
  سجلات CI من التقاط التشخيصات.
- Para el hogar `deny_providers` y `boost_providers` para el hogar y el hogar.
  `priority_delta` عندما يختار المُجدول المزوّدين.
- حافظ على الوضع الافتراضي `"soranet-first"` ما لم تكن تجهّز لخفض المستوى؛ قدّم
  `"direct-only"` فقط عندما يتعين على منطقة امتثال تجنّب المرحلات أو عند تدريب
  Utilice SNNet-5a y `"soranet-strict"` para obtener archivos PQ exclusivos.
- تعرض مساعدات البوابة أيضًا `scoreboardOutPath` y `scoreboardNowUnixSecs`. اضبط
  `scoreboardOutPath` Tablero de marcador de pantalla táctil (يعكس علم CLI `--scoreboard-out`)
  El software `cargo xtask sorafs-adoption-check` se encuentra en el SDK y en el software.
  `scoreboardNowUnixSecs` Accesorios para accesorios de iluminación `assume_now` Accesorios para accesorios
  وصفية قابلة لإعادة الإنتاج. في مساعد JavaScript يمكنك أيضًا ضبط
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`؛ وعند حذف الملصق
  Utilice `region:<telemetryRegion>` (el respaldo es `sdk:js`). Aplicación Python
  `telemetry_source="sdk:python"` كلما حفظ لوحة marcador ويُبقي البيانات الوصفية
  الضمنية معطّلة.

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```