---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementación de múltiples fuentes
título: دليل إطلاق متعدد المصادر وإدراج المزوّدين في القائمة السوداء
sidebar_label: دليل إطلاق متعدد المصادر
descripción: قائمة تشغيل للعمليات المرحلية متعددة المصادر ومنع المزوّدين في حالات الطوارئ.
---

:::nota المصدر المعتمد
Utilice el código `docs/source/sorafs/runbooks/multi_source_rollout.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم سحب مجموعة التوثيق القديمة.
:::

## الغاية

يوجّه هذا الدليل فرق SRE والمهندسين المناوبين عبر مسارين حاسمين:

1. إطلاق المُنسِّق متعدد المصادر على موجات مضبوطة.
2. إدراج المزوّدين المسيئين في القائمة السوداء أو خفض أولويتهم دون زعزعة الجلسات القائمة.

يفترض أن حزمة الأوركسترة التي تسليمها ضمن SF-6 منشورة بالفعل (`sorafs_orchestrator`, and API لنطاق الشرائح في البوابة، مُصدّرات التليمترية).

> **راجع أيضًا:** يتعمق [دليل تشغيل المُنسِّق](./orchestrator-ops.md) في إجراءات كل تشغيل (التقاط لوحة النتائج، مفاتيح الإطلاق المرحلي، والرجوع للخلف). استخدم المرجعين معًا أثناء التغييرات الحية.

## 1. التحقق قبل التنفيذ

1. **تأكيد مدخلات الحوكمة.**
   - Haga clic en el botón de encendido `ProviderAdvertV1` para que el producto funcione correctamente. تحقّق عبر `/v2/sorafs/providers` y مع حقول القدرات المتوقعة.
   - يجب أن تكون لقطات التليمترية التي توفّر معدلات الكمون/الفشل أحدث من 15 دقيقة قبل كل تشغيل كناري.
2. **تهيئة الإعدادات.**
   - Establezca un código JSON en el código `iroha_config`:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```Utilice el código JSON (`max_providers`, y los archivos de configuración). استخدم الملف نفسه في puesta en escena/producción لتبقى الفوارق صغيرة.
3. **تمرين الـ accesorios القياسية.**
   - املأ متغيرات بيئة المانيفست/الرموز ونفّذ الجلب الحتمي:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     Aquí hay un resumen de datos (hexadecimal) y base64 que no funciona correctamente.
   - قارن `artifacts/canary.scoreboard.json` بالإصدار السابق. أي مزوّد جديد غير مؤهل أو انزياح وزن >10% يتطلب مراجعة.
4. **التأكد من توصيل التليمترية.**
   - Utilice Grafana para `docs/examples/sorafs_fetch_dashboard.json`. تأكد من ظهور مقاييس `sorafs_orchestrator_*` في puesta en escena قبل المتابعة.

## 2. إدراج المزوّدين في القائمة السوداء عند الطوارئ

اتبع هذا الإجراء عندما يقدم المزوّد شرائح تالفة، أو يتجاوز المهلات بشكل مستمر، أو يفشل في فحوص الامتثال.1. **توثيق الأدلة.**
   - صدّر أحدث ملخص buscar (مخرجات `--json-out`). سجّل فهارس الشرائح الفاشلة، وأسماء المزوّدين المستعارة، وعدم تطابق digest.
   - Utilice el conector `telemetry::sorafs.fetch.*`.
2. **تطبيق تجاوز فوري.**
   - علّم المزوّد كمُعاقب في لقطة التليمترية الموزعة على المُنسِّق (اضبط `penalty=true` أو اخفض `token_health` como `0`). سيستبعده بناء marcador التالي تلقائيًا.
   - Humo de humo desde `--deny-provider gw-alpha` hasta `sorafs_cli fetch`.
   - أعد نشر حزمة التليمترية/الإعداد المحدثة في البيئة المتأثرة (puesta en escena → canario → producción). دوّن التغيير في سجل الحادثة.
3. **التحقق من التجاوز.**
   - أعد تشغيل buscar للـ accesorio القياسي. تأكد أن marcador يضع المزوّد كغير مؤهل بسبب `policy_denied`.
   - Utilice `sorafs_orchestrator_provider_failures_total` para conectar el dispositivo a su dispositivo móvil.
4. **تصعيد الحظر طويل الأمد.**
   - إذا استمر الحظر لأكثر من 24 h، افتح تذكرة حوكمة لتدوير أو تعليق الخاص به. حتى تمر الموافقة، أبقِ قائمة المنع وقم بتحديث لقطات التليمترية كي لا يعود المزوّد إلى marcador.
5. **بروتوكول التراجع.**
   - لإعادة المزوّد، أزله من قائمة المنع، أعد النشر، والتقط لقطة marcador جديدة. أرفق التغيير بتقرير ما بعد الحادثة.

## 3. خطة الإطلاق المرحلية| المرحلة | النطاق | الإشارات المطلوبة | معايير Pasa/No pasa |
|---------|--------|-------------------|-----------------|
| **Laboratorio** | عنقود تكامل مخصص | Accesorios para cargas útiles CLI y accesorios para cargas útiles | تنجح كل الشرائح، تبقى عدادات فشل المزوّد عند 0، نسبة إعادة المحاولة < 5%. |
| **Puesta en escena** | puesta en escena de بيئة كاملة لطبقة التحكم | Teclado Grafana Monitor قواعد التنبيه في وضع solo advertencia | تعود `sorafs_orchestrator_active_fetches` إلى الصفر بعد كل تشغيل تجريبي؛ Aquí está el mensaje `warn/critical`. |
| **Canarias** | ≤10% del precio de venta | جهاز النداء (buscapersonas) صامت لكن التليمترية مراقبة لحظيًا | نسبة إعادة المحاولة < 10%, فشل المزوّدين محصور في نظراء معروفين بالضجيج، وهيستوغرام الكمون مطابق لخط أساس estadificación ±20%. |
| **الإتاحة العامة** | 100% libre | قواعد الـ buscapersonas فعّالة | La unidad `NoHealthyProviders` funciona las 24 horas y funciona con dispositivos SLA. |

لكل مرحلة:

1. Inserte el archivo JSON en el archivo `max_providers` y guarde el archivo.
2. Introduzca `sorafs_cli fetch` y configure el SDK para el dispositivo y el dispositivo.
3. التقط artefactos الخاصة بالـ marcador وresumen وأرفقها بسجل الإصدار.
4. راجع لوحات التليمترية مع مهندس المناوبة قبل الترقية إلى المرحلة التالية.

## 4. الرصد وربط الحوادث- **المقاييس:** تأكد من أن Alertmanager يراقب `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` e `sorafs_orchestrator_retries_total`. الارتفاع المفاجئ يعني عادة أن مزوّدًا يتدهور تحت الحمل.
- **السجلات:** وجّه أهداف `telemetry::sorafs.fetch.*` إلى مجمّع السجلات المشترك. أنشئ عمليات بحث محفوظة لـ `event=complete status=failed` لتسريع triaje.
- **لوحات النتائج:** احفظ كل artefacto من marcador في تخزين طويل الأمد. Esta es una configuración JSON que contiene archivos adjuntos y archivos adjuntos.
- **لوحات المتابعة:** انسخ لوحة Grafana المعتمدة (`docs/examples/sorafs_fetch_dashboard.json`) إلى مجلد الإنتاج مع قواعد التنبيه من `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. التواصل والتوثيق

- سجّل كل تغيير denegar/impulsar في سجل عمليات التشغيل مع الطابع الزمني، والمشغل، والسبب، والحادث المرتبط.
- أخطر فرق SDK عند تغير أوزان المزوّدين أو ميزانيات إعادة المحاولة لمواءمة توقعات جانب العميل.
- Utilice GA, `status.md` para conectar y desconectar el dispositivo.