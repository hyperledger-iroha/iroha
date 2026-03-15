---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de nó
título: خطة تنفيذ عقدة SoraFS
sidebar_label: Nome da barra lateral
description: Você pode usar o SF-3 para obter mais informações sobre o que fazer com o SF-3. Obrigado.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/sorafs_node_plan.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف وثائق Sphinx القديمة.
:::

A caixa SF-3 é feita de caixa de papelão `sorafs-node` ou caixa Iroha/Torii ou mais SoraFS. استخدم هذه الخطة بجانب [دليل تخزين العقدة](node-storage.md) , e [سياسة قبول O nome](provider-admission-policy.md) e o [nome do arquivo](storage-capacity-marketplace.md) não são válidos.

## النطاق المستهدف (المرحلة M1)

1. **تكامل مخزن القطع.** تغليف `sorafs_car::ChunkStore` بواجهة خلفية دائمة تخزن بايتات القطع وملفات manifesto وأشجار PoR em qualquer lugar do mundo.
2. **نقاط نهاية البوابة.** توفير نقاط نهاية HTTP para Norito para pin وجلب القطع وأخذ عينات PoR O problema é o Torii.
3. **توصيل الإعدادات.** إضافة بنية إعداد `SoraFsStorage` (مفتاح التفعيل, السعة, المجلدات, حدود Nome do produto: `iroha_config` e `iroha_core` e `iroha_torii`.
4. **الحصص/الجدولة.** فرض حدود القرص/التوازي التي يحددها المشغل ووضع الطلبات sem contrapressão.
5. **التليمترية.** إصدار مقاييس/سجلات لنجاح pin وزمن جلب القطع واستغلال السعة ونتائج عينات PoR.

## تفصيل العمل

### A. بنية الـ crate والوحدات

| المهمة | المالك | الملاحظات |
|------|--------|-----------|
| O `crates/sorafs_node` é o seguinte: `config` e `store` e `gateway` e `scheduler` e `telemetry`. | فريق التخزين | Verifique se o dispositivo está conectado com Torii. |
| Use `StorageConfig` como `SoraFsStorage` (usuário → real → padrões). | فريق التخزين / Config WG | Verifique se o Norito/`iroha_config` está instalado. |
| Use o `NodeHandle` para Torii para conectar pinos/buscas. | فريق التخزين | Limpe o local e o local onde estiver. |

### B. مخزن قطع دائم

| المهمة | المالك | الملاحظات |
|------|--------|-----------|
| بناء واجهة خلفية على القرص تغلف `sorafs_car::ChunkStore` é o manifesto do arquivo (`sled`/`sqlite`). | فريق التخزين | Nome de usuário: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| O valor de PoR é maior (64 KiB/4 KiB) usando `ChunkStore::sample_leaves`. | فريق التخزين | يدعم إعادة التشغيل; Não se preocupe. |
| تنفيذ إعادة فحص السلامة عند البدء (إعادة تجزئة manifesta e pins غير المكتملة). | فريق التخزين | O Torii está danificado. |

### C. نقاط نهاية البوابة| نقطة النهاية | السلوك | المهام |
|------------|---------|-------|
| `POST /sorafs/pin` | O `PinProposalV1` é o manifesto do CID e o manifesto do CID. | Você pode usar o chunker وفرض الحصص وبث البيانات عبر مخزن القطع. |
| `GET /sorafs/chunks/{cid}` + gama de produtos | Verifique o valor do produto `Content-Chunker` e verifique o valor do produto. | استخدام المجدول مع ميزانيات البث (ربطها بقدرات النطاق SF-2d). |
| `POST /sorafs/por/sample` | تنفيذ أخذ عينات PoR لملف manifest وإرجاع حزمة إثبات. | Você pode usar o Norito JSON para obter o arquivo Norito. |
| `GET /sorafs/telemetry` | Exemplo: السعة ونجاح PoR وعدّادات أخطاء fetch. | توفير البيانات للوحة المراقبة/المشغلين. |

تقوم الوصلات في وقت التشغيل بتمرير تفاعلات PoR عبر `sorafs_node::por`, حيث يسجل المتتبع كل `PorChallengeV1` e `PorProofV1` e `AuditVerdictV1` são usados para `CapacityMeter`. Torii مخصص.【crates/sorafs_node/src/scheduler.rs#L147】

O que fazer:

- Você pode usar Axum como Torii para usar `norito::json`.
- Verifique se o Norito é compatível (`PinResultV1` e `FetchErrorV1` e `FetchErrorV1`).

- ✅ أصبح المسار `/v2/sorafs/por/ingestion/{manifest_digest_hex}` يعرض عمق الـ backlog وأقدم época/prazo وأحدث طوابع النجاح/الفشل لكل مزود، `sorafs_node::NodeHandle::por_ingestion_status`, e Torii ou `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` 【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. المجدول وفرض الحصص

| المهمة | التفاصيل |
|------|----------|
| حصة القرص | تتبع البايتات على القرص؛ Os pinos de fixação são do tipo `max_capacity_bytes`. Certifique-se de que não há nenhum problema com isso. |
| Buscar buscar | A placa de vídeo (`max_parallel_fetches`) pode ser usada para usar o SF-2d. |
| Pinos de aço | تحديد عدد مهام الإدخال المعلقة؛ Verifique se o Norito está danificado. |
| وتيرة PoR | Você pode usar o `por_sample_interval_secs`. |

### E. التليمترية والسجلات

Nome (Prometheus):

-`sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (`result`)
-`torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
-`torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
-`torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

السجلات / الأحداث:

- Verifique Norito para remover o problema (`StorageTelemetryV1`).
- تنبيهات عند تجاوز الاستغلال 90% أو عندما تتخطى سلسلة إخفاقات PoR العتبة.

### F. استراتيجية الاختبارات

1. **اختبارات وحدات.** ديمومة مخزن القطع, حسابات الحصة, ثوابت المجدول (انظر `crates/sorafs_node/src/scheduler.rs`).
2. **Configurações de segurança** (`crates/sorafs_node/tests`). دورة pin → fetch, الاستعادة بعد إعادة التشغيل, رفض الحصص, والتحقق من إثباتات أخذ عينات PoR.
3. **Escolha Torii.** Use Torii para obter acesso HTTP e HTTP. `assert_cmd`.
4. **خارطة طريق الفوضى.** تدريبات مستقبلية تحاكي نفاد القرص, بطء IO, وإزالة الموفّرين.

## التبعيات- سياسة قبول SF-2b — التأكد من أن العقد تتحقق من أظرف القبول قبل الإعلان.
- سوق السعة SF-2c — ربط التليمترية بإعلانات السعة.
- امتدادات anúncio para SF-2d — استهلاك قدرة النطاق + ميزانيات البث عند توفرها.

## معايير إغلاق المرحلة

- `cargo run -p sorafs_node --example pin_fetch` يعمل مع fixtures محلية.
- O Torii é o `--features sorafs-storage` e deve ser removido.
- تحديث الوثائق ([دليل تخزين العقدة](node-storage.md)) مع افتراضيات الإعداد وأمثلة CLI; O runbook está disponível.
- ظهور التليمترية في لوحات staging وضبط التنبيهات لتشبع السعة وإخفاقات PoR.

## مخرجات الوثائق والعمليات

- تحديث [مرجع تخزين العقدة](node-storage.md) مع افتراضيات الإعداد, استخدام CLI, وخطوات الاستكشاف.
- إبقاء [runbook عمليات العقدة](node-operations.md) متوافقا مع التنفيذ مع تطور SF-3.
- A API da API `/sorafs/*` é usada para configurar o OpenAPI Produto Torii.