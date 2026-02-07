---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : déploiement multi-source
titre : دليل إطلاق متعدد المصادر وإدراج المزوّدين في القائمة السوداء
sidebar_label : دليل إطلاق متعدد المصادر
description: قائمة تشغيل للعمليات المرحلية متعددة المصادر ومنع المزوّدين في حالات الطوارئ.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/runbooks/multi_source_rollout.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم سحب مجموعة التوثيق القديمة.
:::

## الغاية

يوجّه هذا الدليل فرق SRE والمهندسين المناوبين عبر مسارين حاسمين:

1. إطلاق المُنسِّق متعدد المصادر على موجات مضبوطة.
2. إدراج المزوّدين المسيئين في القائمة السوداء أو خفض أولويتهم دون زعزعة الجلسات القائمة.

Vous pouvez également utiliser le SF-6 pour la mise en œuvre (`sorafs_orchestrator`, et l'API pour la mise en œuvre في البوابة، مُصدّرات التليمترية).

> **راجع أيضًا:** يتعمق [دليل تشغيل المُنسِّق](./orchestrator-ops.md) في إجراءات كل تشغيل (التقاط لوحة النتائج، مفاتيح الإطلاق المرحلي، والرجوع للخلف). استخدم المرجعين معًا أثناء التغييرات الحية.

## 1. التحقق قبل التنفيذ

1. **تأكيد مدخلات الحوكمة.**
   - يجب على جميع المزوّدين المرشحين نشر أظرفة `ProviderAdvertV1` مع حمولة قدرات النطاق وميزانيات التدفق. تحقّق عبر `/v1/sorafs/providers` وقارن مع حقول القدرات المتوقعة.
   - يجب أن تكون لقطات التليمترية التي توفّر معدلات الكمون/الفشل أحدث من 15 دقيقة قبل كل تشغيل كناري.
2. ** تهيئة الإعدادات.**
   - Utilisez JSON pour utiliser le code `iroha_config` :

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```JSON est utilisé pour la lecture (`max_providers` et la lecture est effectuée). استخدم الملف نفسه في mise en scène/production لتبقى الفوارق صغيرة.
3. ** تمرين الـ calendriers القياسية.**
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
   - قارن `artifacts/canary.scoreboard.json` pour la prise en charge. أي مزوّد جديد غير مؤهل أو انزياح وزن >10% يتطلب مراجعة.
4. **التأكد من توصيل التليمترية.**
   - Utilisez Grafana pour `docs/examples/sorafs_fetch_dashboard.json`. Il s'agit d'un fichier `sorafs_orchestrator_*` pour la mise en scène de la fonction de mise en scène.

## 2. إدراج المزوّدين في القائمة السوداء عند الطوارئ

اتبع هذا الإجراء عندما يقدم المزوّد شرائح تالفة، أو يتجاوز المهلات بشكل مستمر، أو يفشل في فحوص الامتثال.1. **توثيق الأدلة.**
   - Vous pouvez récupérer la récupération (`--json-out`). سجّل فهارس الشرائح الفاشلة، وأسماء المزوّدين المستعارة، وعدم تطابق digest.
   - احفظ مقتطفات السجل ذات الصلة من أهداف `telemetry::sorafs.fetch.*`.
2. **تطبيق تجاوز فوري.**
   - علّم المزوّد كمُعاقب في لقطة التليمترية الموزعة على المُنسِّق (اضبط `penalty=true` أو اخفض `token_health` ou `0`). سيستبعده بناء tableau de bord التالي تلقائيًا.
   - لاختبارات smoke السريعة، مرر `--deny-provider gw-alpha` إلى `sorafs_cli fetch` لتمرين مسار الفشل دون انتظار انتشار التليمترية.
   - أعد نشر حزمة التليمترية/الإعداد المحدثة في البيئة المتأثرة (mise en scène → canari → production). دوّن التغيير في سجل الحادثة.
3. **التحقق من التجاوز.**
   - أعد تشغيل récupérer le luminaire القياسي. Vous pouvez utiliser le tableau de bord comme `policy_denied`.
   - افحص `sorafs_orchestrator_provider_failures_total` للتأكد من توقف العداد عن الزيادة للمزوّد الممنوع.
4. **تصعيد الحظر طويل الأمد.**
   - La diffusion en continu 24 heures sur 24 est effectuée sous forme de publicité et de publicité. Vous avez besoin de plus de détails sur le tableau de bord.
5. **بروتوكول التراجع.**
   - لإعادة المزوّد، أزله من قائمة المنع، أعد النشر، والتقط لقطة tableau de bord جديدة. أرفق التغيير بتقرير ما بعد الحادثة.

## 3. خطة الإطلاق المرحلية| المرحلة | النطاق | الإشارات المطلوبة | Lire Go/No-Go |
|---------|--------|---------|-----------------|
| **Labo** | عنقود تكامل مخصص | جلب يدوي عبر CLI ضد appareils de charges utiles | تنجح كل الشرائح، تبقى عدادات فشل المزوّد عند 0, نسبة إعادة المحاولة < 5%. |
| **Mise en scène** | بيئة mise en scène كاملة لطبقة التحكم | لوحة Grafana موصولة؛ قواعد التنبيه في وضع avertissement uniquement | تعود `sorafs_orchestrator_active_fetches` إلى الصفر بعد كل تشغيل تجريبي؛ Pour cela, vous devez utiliser `warn/critical`. |
| **Canari** | ≤10% pour les achats | جهاز النداء (pager) صامت لكن التليمترية مراقبة لحظيًا | نسبة إعادة المحاولة < 10%، فشل المزوّدين محصور في نظراء معروفين بالضجيج، وهيستوغرام الكمون مطابق لخط أساس mise en scène ±20%. |
| **الإتاحة العامة** | 100% إطلاق | قواعد الـ pager فعّالة | Le `NoHealthyProviders` est valable 24 h et est compatible avec le SLA. |

لكل مرحلة:

1. Utilisez JSON pour utiliser `max_providers` et utilisez le code source.
2. Utilisez `sorafs_cli fetch` pour installer le SDK sur le luminaire et les appareils.
3. Les artefacts du tableau de bord et le résumé du tableau de bord.
4. راجع لوحات التليمترية مع مهندس المناوبة قبل الترقية إلى المرحلة التالية.

## 4. الرصد وربط الحوادث- **المقاييس:** تأكد من أن Alertmanager يراقب `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` et `sorafs_orchestrator_retries_total`. الارتفاع المفاجئ يعني عادة أن مزوّدًا يتدهور تحت الحمل.
- **السجلات:** وجّه أهداف `telemetry::sorafs.fetch.*` إلى مجمّع السجلات المشترك. Il s'agit d'un outil de triage `event=complete status=failed`.
- **لوحات النتائج:** احفظ كل artefact من scoreboard في تخزين طويل الأمد. JSON est également utilisé pour les fichiers et les vidéos.
- **لوحات المتابعة:** انسخ لوحة Grafana المعتمدة (`docs/examples/sorafs_fetch_dashboard.json`) إلى مجلد الإنتاج مع قواعد التنبيه من `docs/examples/sorafs_fetch_alerts.yaml`.

## 5. التواصل والتوثيق

- سجّل كل تغيير deny/boost في سجل عمليات التشغيل مع الطابع الزمني، والمشغل، والسبب، والحادث المرتبط.
- Le SDK est également compatible avec les périphériques USB et les périphériques USB.
- بعد اكتمال GA, حدّث `status.md` بملخص الإطلاق وأرشِف مرجع هذا الدليل في ملاحظات الإصدار.