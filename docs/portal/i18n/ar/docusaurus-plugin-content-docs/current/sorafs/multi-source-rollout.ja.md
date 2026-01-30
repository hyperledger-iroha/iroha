---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/multi-source-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f053073356b6420f87142dc0bf9de0f57254de8bd06a56889d3fb135ede0ea37
source_last_modified: "2025-11-14T04:43:21.829481+00:00"
translation_last_reviewed: 2026-01-30
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/runbooks/multi_source_rollout.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم سحب مجموعة التوثيق القديمة.
:::

## الغاية

يوجّه هذا الدليل فرق SRE والمهندسين المناوبين عبر مسارين حاسمين:

1. إطلاق المُنسِّق متعدد المصادر على موجات مضبوطة.
2. إدراج المزوّدين المسيئين في القائمة السوداء أو خفض أولويتهم دون زعزعة الجلسات القائمة.

يفترض أن حزمة الأوركسترة التي تم تسليمها ضمن SF-6 منشورة بالفعل (`sorafs_orchestrator`, واجهة API لنطاق الشرائح في البوابة، مُصدّرات التليمترية).

> **راجع أيضًا:** يتعمق [دليل تشغيل المُنسِّق](./orchestrator-ops.md) في إجراءات كل تشغيل (التقاط لوحة النتائج، مفاتيح الإطلاق المرحلي، والرجوع للخلف). استخدم المرجعين معًا أثناء التغييرات الحية.

## 1. التحقق قبل التنفيذ

1. **تأكيد مدخلات الحوكمة.**
   - يجب على جميع المزوّدين المرشحين نشر أظرفة `ProviderAdvertV1` مع حمولة قدرات النطاق وميزانيات التدفق. تحقّق عبر `/v1/sorafs/providers` وقارن مع حقول القدرات المتوقعة.
   - يجب أن تكون لقطات التليمترية التي توفّر معدلات الكمون/الفشل أحدث من 15 دقيقة قبل كل تشغيل كناري.
2. **تهيئة الإعدادات.**
   - احفظ إعداد JSON للمُنسِّق داخل شجرة `iroha_config` المتعددة الطبقات:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     حدّث JSON بحدود الإطلاق (`max_providers`، وميزانيات إعادة المحاولة). استخدم الملف نفسه في staging/production لتبقى الفوارق صغيرة.
3. **تمرين الـ fixtures القياسية.**
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

     يجب أن تتضمن متغيرات البيئة digest حمولة المانيفست (hex) والرموز المرمّزة base64 لكل مزوّد مشارك في الكناري.
   - قارن `artifacts/canary.scoreboard.json` بالإصدار السابق. أي مزوّد جديد غير مؤهل أو انزياح وزن >10% يتطلب مراجعة.
4. **التأكد من توصيل التليمترية.**
   - افتح تصدير Grafana في `docs/examples/sorafs_fetch_dashboard.json`. تأكد من ظهور مقاييس `sorafs_orchestrator_*` في staging قبل المتابعة.

## 2. إدراج المزوّدين في القائمة السوداء عند الطوارئ

اتبع هذا الإجراء عندما يقدم المزوّد شرائح تالفة، أو يتجاوز المهلات بشكل مستمر، أو يفشل في فحوص الامتثال.

1. **توثيق الأدلة.**
   - صدّر أحدث ملخص fetch (مخرجات `--json-out`). سجّل فهارس الشرائح الفاشلة، وأسماء المزوّدين المستعارة، وعدم تطابق digest.
   - احفظ مقتطفات السجل ذات الصلة من أهداف `telemetry::sorafs.fetch.*`.
2. **تطبيق تجاوز فوري.**
   - علّم المزوّد كمُعاقب في لقطة التليمترية الموزعة على المُنسِّق (اضبط `penalty=true` أو اخفض `token_health` إلى `0`). سيستبعده بناء scoreboard التالي تلقائيًا.
   - لاختبارات smoke السريعة، مرر `--deny-provider gw-alpha` إلى `sorafs_cli fetch` لتمرين مسار الفشل دون انتظار انتشار التليمترية.
   - أعد نشر حزمة التليمترية/الإعداد المحدثة في البيئة المتأثرة (staging → canary → production). دوّن التغيير في سجل الحادثة.
3. **التحقق من التجاوز.**
   - أعد تشغيل fetch للـ fixture القياسي. تأكد أن scoreboard يضع المزوّد كغير مؤهل بسبب `policy_denied`.
   - افحص `sorafs_orchestrator_provider_failures_total` للتأكد من توقف العداد عن الزيادة للمزوّد الممنوع.
4. **تصعيد الحظر طويل الأمد.**
   - إذا استمر الحظر لأكثر من 24 h، افتح تذكرة حوكمة لتدوير أو تعليق advert الخاص به. حتى تمر الموافقة، أبقِ قائمة المنع وقم بتحديث لقطات التليمترية كي لا يعود المزوّد إلى scoreboard.
5. **بروتوكول التراجع.**
   - لإعادة المزوّد، أزله من قائمة المنع، أعد النشر، والتقط لقطة scoreboard جديدة. أرفق التغيير بتقرير ما بعد الحادثة.

## 3. خطة الإطلاق المرحلية

| المرحلة | النطاق | الإشارات المطلوبة | معايير Go/No-Go |
|---------|--------|-------------------|-----------------|
| **Lab** | عنقود تكامل مخصص | جلب يدوي عبر CLI ضد payloads fixtures | تنجح كل الشرائح، تبقى عدادات فشل المزوّد عند 0، نسبة إعادة المحاولة < 5%. |
| **Staging** | بيئة staging كاملة لطبقة التحكم | لوحة Grafana موصولة؛ قواعد التنبيه في وضع warning-only | تعود `sorafs_orchestrator_active_fetches` إلى الصفر بعد كل تشغيل تجريبي؛ لا توجد تنبيهات `warn/critical`. |
| **Canary** | ≤10% من حركة الإنتاج | جهاز النداء (pager) صامت لكن التليمترية مراقبة لحظيًا | نسبة إعادة المحاولة < 10%، فشل المزوّدين محصور في نظراء معروفين بالضجيج، وهيستوغرام الكمون مطابق لخط أساس staging ±20%. |
| **الإتاحة العامة** | 100% إطلاق | قواعد الـ pager فعّالة | صفر أخطاء `NoHealthyProviders` لمدة 24 h، نسبة إعادة المحاولة مستقرة، ولوحات SLA خضراء. |

لكل مرحلة:

1. حدّث JSON للمُنسِّق بقيم `max_providers` وميزانيات إعادة المحاولة المطلوبة.
2. نفّذ `sorafs_cli fetch` أو مجموعة اختبارات تكامل SDK مقابل الـ fixture القياسي ومانيفست ممثل للبيئة.
3. التقط artifacts الخاصة بالـ scoreboard وsummary وأرفقها بسجل الإصدار.
4. راجع لوحات التليمترية مع مهندس المناوبة قبل الترقية إلى المرحلة التالية.

## 4. الرصد وربط الحوادث

- **المقاييس:** تأكد من أن Alertmanager يراقب `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` و`sorafs_orchestrator_retries_total`. الارتفاع المفاجئ يعني عادة أن مزوّدًا يتدهور تحت الحمل.
- **السجلات:** وجّه أهداف `telemetry::sorafs.fetch.*` إلى مجمّع السجلات المشترك. أنشئ عمليات بحث محفوظة لـ `event=complete status=failed` لتسريع triage.
- **لوحات النتائج:** احفظ كل artifact من scoreboard في تخزين طويل الأمد. يعمل JSON كمسار أدلة لمراجعات الامتثال والرجوع المرحلي.
- **لوحات المتابعة:** انسخ لوحة Grafana المعتمدة (`docs/examples/sorafs_fetch_dashboard.json`) إلى مجلد الإنتاج مع قواعد التنبيه من `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. التواصل والتوثيق

- سجّل كل تغيير deny/boost في سجل عمليات التشغيل مع الطابع الزمني، والمشغل، والسبب، والحادث المرتبط.
- أخطر فرق SDK عند تغير أوزان المزوّدين أو ميزانيات إعادة المحاولة لمواءمة توقعات جانب العميل.
- بعد اكتمال GA، حدّث `status.md` بملخص الإطلاق وأرشِف مرجع هذا الدليل في ملاحظات الإصدار.
