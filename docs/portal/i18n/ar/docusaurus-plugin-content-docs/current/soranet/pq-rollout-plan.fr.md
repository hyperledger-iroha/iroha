---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة الطرح PQ
العنوان: Playbook de Deploiement post-quantique SNNet-16G
Sidebar_label: خطة النشر PQ
الوصف: دليل التشغيل لتعزيز المصافحة الهجينة X25519+ML-KEM من SoraNet الكناري الافتراضي على المرحلات والعملاء ومجموعات SDK.
---

:::ملاحظة المصدر الكنسي
:::

يقوم SNNet-16G بإنهاء النشر اللاحق لنقل SoraNet. تتيح المقابض `rollout_phase` لمشغلي التنسيق تعزيزًا محددًا لمتطلبات الحماية المرحلة A مقابل الغطاء الرئيسي المرحلة B ووضعية PQ الصارمة المرحلة C بدون تعديل JSON/TOML الخام لكل سطح.

غطاء كتاب اللعب Ce:

- تعريفات المرحلة ومقابض التكوين الجديدة (`sorafs.gateway.rollout_phase`، `sorafs.rollout_phase`) للكابلات في قاعدة التعليمات البرمجية (`crates/iroha_config/src/parameters/actual.rs:2230`، `crates/iroha/src/config/user.rs:251`).
- تعيين علامات SDK وCLI حتى يتمكن كل عميل من متابعة الطرح.
- مراقبة جدولة ترحيل الكناري/العميل بالإضافة إلى لوحات التحكم في الحوكمة التي تفتح الترويج (`dashboards/grafana/soranet_pq_ratchet.json`).
- خطافات التراجع وتشير إلى دليل التدريب على الحرائق ([PQ دليل السقاطة](./pq-ratchet-runbook.md)).

## كارت المراحل| `rollout_phase` | شريط إخفاء الهوية فعال | التأثير الافتراضي | نوع الاستخدام |
|-----------------|-------------------------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (المرحلة أ) | قم بالتمرير إلى دائرة حماية PQ حتى يتم إعادة تشغيل السفينة. | Baseline وعرض لأول مرة لمدة نصف ساعة على الكناري. |
| `ramp` | `anon-majority-pq` (المرحلة ب) | تفضيل التحديد على المرحلات PQ من أجل >= طبقتين من الغطاء؛ المرحلات الكلاسيكية تبقى احتياطية. | مرحلات الكناري المنطقة الاسمية. تبديل معاينة SDK. |
| `default` | `anon-strict-pq` (المرحلة ج) | قم بفرض دوائر PQ فريدة من نوعها وقم بتشغيل إنذارات الرجوع إلى إصدار أقدم. | اكتمل الترويج النهائي بعد انتهاء عملية القياس عن بعد وحوكمة تسجيل الخروج. |

إذا كان السطح محددًا أيضًا `anonymity_policy` بشكل صريح، فسوف يتجاوز المرحلة الخاصة بهذا المكون. قم بتأجيل الشريط بوضوح للحفاظ على القيمة `rollout_phase` حتى يتمكن المشغلون من الحفاظ على بيئة مناسبة والسماح للعملاء بالبقاء.

## مرجع التكوين

### المنسق (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

يقوم مُحمل المُنسق بإعادة تشغيل الشريط الاحتياطي في وقت التشغيل (`crates/sorafs_orchestrator/src/lib.rs:2229`) والكشف عبر `sorafs_orchestrator_policy_events_total` و`sorafs_orchestrator_pq_ratio_*`. ابحث عن `docs/examples/sorafs_rollout_stage_b.toml` و`docs/examples/sorafs_rollout_stage_c.toml` من أجل تطبيق المقتطفات.

### عميل الصدأ / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
````iroha::Client` قم بتسجيل صيانة المرحلة التحليلية (`crates/iroha/src/client.rs:2315`) حتى تتمكن الأوامر المساعدة (على سبيل المثال `iroha_cli app sorafs fetch`) من الإبلاغ عن المرحلة الفعلية من جوانب سياسة عدم الكشف بشكل افتراضي.

## الأتمتة

يقوم اثنان من المساعدين `cargo xtask` بأتمتة إنشاء الجدول الزمني والتقاط العناصر.

1. ** إنشاء الجدول الزمني الإقليمي **

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   تقبل الواجبات اللواحق `s` أو `m` أو `h` أو `d`. يتم الأمر باستخدام `artifacts/soranet_pq_rollout_plan.json` واستئناف Markdown (`artifacts/soranet_pq_rollout_plan.md`) للانضمام إلى طلب التغيير.

2. **التقاط القطع الأثرية من الحفر مع التوقيعات**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   أمر بنسخ الملفات الواردة في `artifacts/soranet_pq_rollout/<timestamp>_<label>/`، وحساب ملخص BLAKE3 لكل قطعة أثرية وكتابة `rollout_capture.json` مع البيانات الوصفية بالإضافة إلى توقيع Ed25519 على الحمولة. استخدم المفتاح الخاص الذي يشير إلى دقائق التدريب على الحريق حتى تتمكن الإدارة من التحقق بسرعة من التقاطها.

## Matrice des flags SDK & CLI| السطح | الكناري (المرحلة أ) | المنحدر (المرحلة ب) | الافتراضي (المرحلة ج) |
|---------|------------------|----------------|-------------------|
| جلب `sorafs_cli` | `--anonymity-policy stage-a` أو إعادة النظر في المرحلة | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| التكوين المنسق JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| تكوين عميل الصدأ (`iroha.toml`) | `rollout_phase = "canary"` (افتراضي) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` الأوامر الموقعة | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| جافا/أندرويد `GatewayFetchOptions` | `setRolloutPhase("canary")`، الخيار `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`، الخيار `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`، الخيار `.ANON_STRICT_PQ` |
| مساعدين منسق جافا سكريبت | `rolloutPhase: "canary"` أو `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| بايثون `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| سويفت `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

يتم تعيين جميع أدوات تبديل SDK على أساس تحليل المرحلة باستخدام المُنسق (`crates/sorafs_orchestrator/src/lib.rs:365`)، لفرز عمليات النشر متعددة اللغات في خطوة القفل مع المرحلة التي تم تكوينها.

## قائمة التحقق من جدولة الكناري

1. **التجربة المبدئية (T ناقص أسبوعين)**- تأكد من أن معدل البنيان في المرحلة A هو أقل من 1% على الشاشتين السابقتين وأن غطاء PQ أكبر من = 70% في المنطقة (`sorafs_orchestrator_pq_candidate_ratio`).
   - مخطط فتحة الحكم الذي يوافق على نافذة الكناري.
   - قم بالتمرير إلى `sorafs.gateway.rollout_phase = "ramp"` في التدريج (محرر JSON المنسق وإعادة النشر) وخط أنابيب الترويج الجاف.

2. **تتابع الكناري (يوم T)**

   - قم بترقية منطقة ما إلى الوقت المحدد `rollout_phase = "ramp"` sur l'orchestrator وبيانات مرحلات المشاركين.
   - مراقب "أحداث السياسة لكل نتيجة" و"معدل انقطاع التيار الكهربائي" في لوحة المعلومات PQ Ratchet (التي تتضمن الحفاظ على طرح اللوحة) مرتين مع TTL في ذاكرة التخزين المؤقت الحارسة.
   - التقاط اللقطات `sorafs_cli guard-directory fetch` مقدمًا وبعد ذلك لمخزون المراجعة.

3. ** كناري العميل/SDK (T بالإضافة إلى أسبوع واحد)**

   - أساسي `rollout_phase = "ramp"` في إعدادات العميل أو تمرير التجاوزات `stage-b` لمجموعات SDK المصممة.
   - التقاط اختلافات القياس عن بعد (`sorafs_orchestrator_policy_events_total` Groupe par `client_id` و`region`) وربطها بسجل حادثة التشغيل.

4. **العرض الترويجي الافتراضي (T بالإضافة إلى 3 أسابيع)**

   - مرة واحدة في الحوكمة الصحيحة، المنسق الأساسي وتكوينات العميل على `rollout_phase = "default"` وقم بإدارة قائمة التحقق من الاستعداد الموقعة في عناصر الإصدار.

## إدارة القائمة المرجعية والأدلة| تغيير المرحلة | بوابة الترقية | حزمة الأدلة | لوحات المعلومات والتنبيهات |
|--------------|----------------|---------------------------------|-----|
| كناري -> المنحدر *(معاينة المرحلة ب)* | معدل انقطاع التيار الكهربائي المرحلة A = 0.7 تقدم المنطقة الاسمية، التحقق من تذكرة Argon2 p95  الافتراضي *(فرض المرحلة C)* | النسخ الاحتياطي للقياس عن بعد SN16 لمدة 30 يومًا، `sn16_handshake_downgrade_total` في خط الأساس، `sorafs_orchestrator_brownouts_total` صفر خلال عميل الكناري، وتجربة الوكيل لتبديل السجل. | النسخ `sorafs_cli proxy set-mode --mode gateway|direct`، الفرز `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`، السجل `sorafs_cli guard-directory verify`، وعلامة الحزمة `cargo xtask soranet-rollout-capture --label default`. | لوحة Meme PQ Ratchet بالإضافة إلى اللوحات SN16 التي تعمل على تقليل مستوى المستندات في `docs/source/sorafs_orchestrator_rollout.md` و`dashboards/grafana/soranet_privacy_metrics.json`. || خفض رتبة الطوارئ / الاستعداد للتراجع | قم بإلغاء القفل عند قيام أجهزة الكمبيوتر بالرجوع إلى إصدار أقدم، أو صدى التحقق من دليل الحماية، أو تسجيل المخزن المؤقت `/policy/proxy-toggle` لأحداث الرجوع إلى إصدار سابق. | قائمة التحقق `docs/source/ops/soranet_transport_rollback.md` والسجلات `sorafs_cli guard-directory import` / `guard-cache prune` و`cargo xtask soranet-rollout-capture --label rollback` وتذاكر الحوادث وقوالب الإشعارات. | `dashboards/grafana/soranet_pq_ratchet.json`، `dashboards/grafana/soranet_privacy_metrics.json` وحزم التنبيهات الثنائية (`dashboards/alerts/soranet_handshake_rules.yml`، `dashboards/alerts/soranet_privacy_rules.yml`). |

- يتم إنشاء كل قطعة أثرية من `artifacts/soranet_pq_rollout/<timestamp>_<label>/` مع `rollout_capture.json` بحيث تحتوي حزم الإدارة على لوحة النتائج والآثار والأدوات والخلاصات.
- قم بإرفاق الملخصات SHA256 من الرسوم المسبقة (دقائق PDF، حزمة التقاط، لقطات حماية) إلى دقائق الترويج حتى تكون الموافقات البرلمانية فعالة وممتعة دون الوصول إلى التدريج العنقودي.
- قم بالرجوع إلى خطة القياس عن بعد في تذكرة الترويج للتأكد من أن `docs/source/soranet/snnet16_telemetry_plan.md` يبقي المصدر القانوني لمفردات الرجوع إلى إصدار أقدم ومتابعة التنبيهات.

## افتقد لوحات المعلومات والقياس عن بعد يوميًا

`dashboards/grafana/soranet_pq_ratchet.json` يتضمن صيانة لوحة تعليق توضيحي "خطة الطرح" التي تعيد النظر في دليل التشغيل هذا وتكشف عن المرحلة الفعلية حتى تكون مراجعات الحوكمة المؤكدة هي المرحلة النشطة. قم بمزامنة وصف اللوحة مع التطورات المستقبلية لمقابض التكوين.للتنبيه، تأكد من أن القواعد الموجودة تستخدم التصنيف `stage` لتحديد المراحل الكنارية والتخفيض الافتراضي للعتبات السياسية المنفصلة (`dashboards/alerts/soranet_handshake_rules.yml`).

## خطافات التراجع

### الافتراضي -> المنحدر (المرحلة ج -> المرحلة ب)

1. قم بإرجاع المُنسق باستخدام `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (وقم بإعادة تشغيل المرحلة الثانية على تكوينات SDK) حتى تتمكن المرحلة B من إعادة تشغيل الأسطول بالكامل.
2. فرض العملاء على ملف تعريف النقل على `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`، والتقاط النسخ حتى يكون سير عمل الإصلاح `/policy/proxy-toggle` قابلاً للتدقيق.
3. قم بتنفيذ `cargo xtask soranet-rollout-capture --label rollback-default` لأرشفة مجلدات الحماية المختلفة، وإخراج الأداة المساعدة ولقطات الشاشة إلى لوحات المعلومات مثل `artifacts/soranet_pq_rollout/`.

### المنحدر -> الكناري (المرحلة ب -> المرحلة أ)

1. قم باستيراد لقطة حماية الدليل قبل الترويج باستخدام `sorafs_cli guard-directory import --guard-directory guards.json` ثم أعد الاتصال بـ `sorafs_cli guard-directory verify` حتى تتضمن حزمة التخفيض التجزئات.
2. قم بتعريف `rollout_phase = "canary"` (أو تجاوز مع `anonymity_policy stage-a`) على المُنسق وتكوينات العميل، ثم قم بإعادة تشغيل مثقاب السقاطة PQ من [PQratchet runbook](./pq-ratchet-runbook.md) للتحقق من خفض مستوى خط الأنابيب.
3. قم بإرفاق لقطات الشاشة التي تم تحديثها بواسطة جهاز القياس عن بعد PQ Ratchet وSN16 بالإضافة إلى إدارة نتائج التنبيهات في سجل الحوادث قبل إدارة الإشعارات.

### الدرابزين رابيلز- قم بالرجوع إلى `docs/source/ops/soranet_transport_rollback.md` في كل تخفيض للرتبة وقم بتسجيل جميع عمليات التخفيف المؤقتة مثل العنصر `TODO:` في أداة تعقب الطرح للمتابعة.
- Gardez `dashboards/alerts/soranet_handshake_rules.yml` and `dashboards/alerts/soranet_privacy_rules.yml` sous couverture `promtool test rules` avant and apres un rollback to toute derive d'alerts soit documentee with the Capture package.