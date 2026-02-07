---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة الطرح PQ
العنوان: Plan de despliegue poscuantico SNNet-16G
Sidebar_label: خطة النشر PQ
الوصف: دليل التشغيل لتعزيز المصافحة الهجينة X25519+ML-KEM من SoraNet من الكناري يحتوي على الإعدادات الافتراضية والمرحلات والعملاء ومجموعات SDK.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/soranet/pq_rollout_plan.md`. Manten ambas copias syncronizadas.
:::

SNNet-16G يكمل الرحلة اللاحقة لنقل SoraNet. تسمح عناصر التحكم `rollout_phase` للمشغلين بتنسيق عرض ترويجي محدد من متطلبات الحماية الفعلية للمرحلة A حتى يتم تغطية الجزء الأكبر من المرحلة B ووضع PQ المحدود للمرحلة C بدون تحرير JSON/TOML بشكل أساسي لكل سطح.

هذا اللعب المكعب:

- تعريفات المراحل ومقابض التكوين الجديدة (`sorafs.gateway.rollout_phase`، `sorafs.rollout_phase`) الموصولة بقاعدة التعليمات البرمجية (`crates/iroha_config/src/parameters/actual.rs:2230`، `crates/iroha/src/config/user.rs:251`).
- خريطة علامات SDK وCLI حتى يتمكن كل عميل من متابعة الطرح.
- توقعات جدولة ترحيل الكناري/العميل مع لوحات التحكم التي ستفتح الترويج (`dashboards/grafana/soranet_pq_ratchet.json`).
- خطافات التراجع والمراجع لدليل التدريب على الحريق ([PQ دليل السقاطة](./pq-ratchet-runbook.md)).

## مابا دي فاسيز| `rollout_phase` | Etapa de anonymato efectiva | تأثير العيب | استخدام تيبيكو |
|-----------------|-------------------------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (المرحلة أ) | تتطلب على الأقل حارس PQ من أجل دائرة بينما تسخن السفينة. | خط الأساس والأسبوع الأول من الكناري. |
| `ramp` | `anon-majority-pq` (المرحلة ب) | يتم تشغيل التحديد بواسطة مرحلات PQ para >= dos tercios de cobertura؛ ينقل الكلاسيكو الدائم كبديل احتياطي. | جزر الكناري حسب منطقة المرحلات؛ تبديل المعاينة في SDK. |
| `default` | `anon-strict-pq` (المرحلة ج) | يتم تطبيق الدوائر فقط PQ وتحمل إنذارات الرجوع إلى إصدار أقدم. | الترويج النهائي بمجرد اكتمال القياس عن بعد وتوقيع الإدارة. |

إذا تم تحديد سطح أيضًا `anonymity_policy` بشكل صريح، فسيتم تجاوز الخطوة الخاصة بهذا المكون. قم بحذف الخطوة بوضوح الآن لتغيير قيمة `rollout_phase` حتى يتمكن المشغلون من تغيير الخطوة مرة واحدة فقط من خلالهم ويتركون العملاء هنا.

## مرجع التكوين

### المنسق (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

يقوم مُحمل المُنسق بإرجاع المرحلة الاحتياطية في وقت التشغيل (`crates/sorafs_orchestrator/src/lib.rs:2229`) والعرض عبر `sorafs_orchestrator_policy_events_total` و`sorafs_orchestrator_pq_ratio_*`. الإصدار `docs/examples/sorafs_rollout_stage_b.toml` و`docs/examples/sorafs_rollout_stage_c.toml` لقوائم المقتطفات لتطبيقها.

### عميل الصدأ / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
````iroha::Client` الآن قم بتسجيل تحليل الحالة (`crates/iroha/src/client.rs:2315`) حتى يتمكن المساعدون (على سبيل المثال `iroha_cli app sorafs fetch`) من الإبلاغ عن الحالة الفعلية جنبًا إلى جنب مع السياسة المجهولة بسبب الخلل.

## الأتمتة

يعمل هذان المساعدان `cargo xtask` على أتمتة إنشاء الجدول الزمني والتقاط العناصر.

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

   هذه المتانة صحيحة `s`، `m`، `h` أو `d`. يصدر الأمر `artifacts/soranet_pq_rollout_plan.json` واستئناف في Markdown (`artifacts/soranet_pq_rollout_plan.md`) الذي يمكنك إرساله بطلب التغيير.

2. **التقاط قطع أثرية من الحفر باستخدام شركة**

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

   الأمر ينسخ الملفات الواردة في `artifacts/soranet_pq_rollout/<timestamp>_<label>/`، ويحسب BLAKE3 لكل قطعة أثرية ويكتب `rollout_capture.json` مع بيانات تعريفية مع شركة Ed25519 حول الحمولة. استخدم نفس المفتاح الخاص الذي يثبت دقائق التدريب على الحريق حتى تتمكن من التحكم في صحة الالتقاط بسرعة.

## Matriz de flags de SDK y CLI| سطحية | الكناري (المرحلة أ) | المنحدر (المرحلة ب) | الافتراضي (المرحلة ج) |
|---------|------------------|----------------|-------------------|
| جلب `sorafs_cli` | `--anonymity-policy stage-a` أو قم بالمصادقة على الفور | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| التكوين المنسق JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| تكوين عميل الصدأ (`iroha.toml`) | `rollout_phase = "canary"` (افتراضي) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` الأوامر الموقعة | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| جافا/أندرويد `GatewayFetchOptions` | `setRolloutPhase("canary")`، اختياري `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`، اختياري `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`، اختياري `.ANON_STRICT_PQ` |
| مساعدين منسق جافا سكريبت | `rolloutPhase: "canary"` أو `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| بايثون `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| سويفت `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

تشير جميع مفاتيح تبديل SDK إلى التحليل نفسه للخطوات المستخدمة من قبل المُنسق (`crates/sorafs_orchestrator/src/lib.rs:365`)، حتى يتم الحفاظ على اللغات المتعددة التي يتم تشغيلها في خطوة ثابتة مع الإعداد المسبق.

## قائمة التحقق من جدولة الكناري

1. **التجربة المبدئية (T ناقص أسبوعين)**- تأكد من أن درجة البنيان في المرحلة A تصل إلى =70% في المنطقة (`sorafs_orchestrator_pq_candidate_ratio`).
   - برمجة فتحة التحكم التي تفتح نافذة الكناري.
   - تحديث `sorafs.gateway.rollout_phase = "ramp"` والتدريج (تحرير JSON للمنسق وإعادة النشر) والتشغيل الجاف لخط أنابيب الترويج.

2. **تتابع الكناري (يوم T)**

   - ترقية منطقة ما من خلال تكوين `rollout_phase = "ramp"` والمنسق وبيانات ترحيل المشاركين.
   - مراقبة "أحداث السياسة لكل نتيجة" و"معدل انقطاع التيار الكهربائي" في لوحة المعلومات PQ Ratchet (التي تتضمن الآن لوحة الطرح) أثناء حماية ذاكرة التخزين المؤقت المزدوجة TTL.
   - قم بقص لقطات `sorafs_cli guard-directory fetch` قبل وبعد التشغيل لتخزين السمع.

3. ** كناري العميل/SDK (T بالإضافة إلى أسبوع واحد)**

   - قم بالتبديل إلى `rollout_phase = "ramp"` في تكوينات العميل أو تجاوز `stage-b` لمجموعات SDK المخصصة.
   - التقاط اختلافات القياس عن بعد (`sorafs_orchestrator_policy_events_total` المجمعة بواسطة `client_id` و`region`) وإلحاق جميع أحداث بدء التشغيل.

4. **العرض الترويجي الافتراضي (T بالإضافة إلى 3 أسابيع)**

   - بمجرد إدارة الشركة، قم بتغيير المنسق مثل تكوينات العميل إلى `rollout_phase = "default"` وقم بتدوير قائمة التحقق من الاستعداد للتأكد من عناصر الإصدار.

## القائمة المرجعية للحوكمة والأدلة| تغيير المسار | بوابة الترويج | حزمة الأدلة | لوحات المعلومات والتنبيهات |
|--------------|----------------|---------------------------------|-----|
| كناري -> المنحدر *(معاينة المرحلة ب)* | مدة انقطاع المرحلة A = 0.7 لكل منطقة معززة، التحقق من تذكرة Argon2 p95  الافتراضي *(فرض المرحلة C)* | النسخ الاحتياطي للقياس عن بعد SN16 لمدة 30 يومًا كاملة، و`sn16_handshake_downgrade_total` مخطط في الأساس، و`sorafs_orchestrator_brownouts_total` في الصفر خلال كناري العميل، وتجربة الوكيل لتبديل التسجيل. | النسخ من `sorafs_cli proxy set-mode --mode gateway|direct`، خرج من `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`، سجل من `sorafs_cli guard-directory verify`، وحزمة ثابتة `cargo xtask soranet-rollout-capture --label default`. | نفس الشيء الذي يعرض PQ Ratchet هو ألواح خفض مستوى SN16 الموثقة في `docs/source/sorafs_orchestrator_rollout.md` و`dashboards/grafana/soranet_privacy_metrics.json`. || خفض رتبة الطوارئ / الاستعداد للتراجع | إذا تم تنشيط أجهزة خفض المستوى الفرعية، فسيفشل التحقق من دليل الحماية أو المخزن المؤقت `/policy/proxy-toggle` في تسجيل أحداث خفض مستوى الدعم الداعمة. | قائمة مرجعية لـ `docs/source/ops/soranet_transport_rollback.md`، وسجلات `sorafs_cli guard-directory import` / `guard-cache prune`، و`cargo xtask soranet-rollout-capture --label rollback`، وتذاكر الأحداث، ونماذج الإشعارات. | `dashboards/grafana/soranet_pq_ratchet.json`، `dashboards/grafana/soranet_privacy_metrics.json` وحزم التنبيهات المتعددة (`dashboards/alerts/soranet_handshake_rules.yml`، `dashboards/alerts/soranet_privacy_rules.yml`). |

- قم بحماية كل قطعة أثرية من `artifacts/soranet_pq_rollout/<timestamp>_<label>/` مع `rollout_capture.json` التي تم إنشاؤها حتى تحتوي حزم الإدارة على لوحة النتائج ونتائج الترويج والخلاصات.
- ملخص SHA256 من الأدلة الفرعية (دقائق PDF، حزمة الالتقاط، لقطات الحراسة) ودقائق الترويج حتى تتمكن موافقة البرلمان من إعادة إنتاجها دون الوصول إلى مجموعة العرض المسرحي.
- قم بالرجوع إلى خطة القياس عن بعد في تذكرة الترويج للتأكد من أن `docs/source/soranet/snnet16_telemetry_plan.md` سيستمر في إمداد المفردات الأساسية بالتخفيض ومظلات التنبيه.

## تحديث لوحات المعلومات والقياس عن بعد

يتضمن `dashboards/grafana/soranet_pq_ratchet.json` الآن لوحة تعليقات توضيحية بعنوان "خطة الطرح" والتي تكمل دليل التشغيل هذا وتعرض العملية الفعلية حتى تؤكد مراجعات الإدارة أنه تم تنشيطها. احتفظ بوصف اللوحة المتزامنة مع التغييرات المستقبلية في مقابض التكوين.للتنبيه، تأكد من استخدام القواعد الموجودة للرمز `stage` حتى تتباعد المراحل الكنارية والافتراضية عن الحدود السياسية المنفصلة (`dashboards/alerts/soranet_handshake_rules.yml`).

## خطافات التراجع

### الافتراضي -> المنحدر (المرحلة ج -> المرحلة ب)

1. قم باستعادة المنسق مع `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (وعكس نفس الشيء في تكوينات SDK) حتى تتمكن المرحلة B من التحرك إلى جميع الأسطول.
2. قم بدعم العملاء من خلال ملف النقل الآمن عبر `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`، والتقط النسخ حتى يكون سير عمل المعالجة `/policy/proxy-toggle` جاهزًا للتدقيق.
3. قم بتشغيل `cargo xtask soranet-rollout-capture --label rollback-default` لحفظ اختلافات دليل الحماية، وإخراج أداة promtool ولقطات شاشة لوحات المعلومات أسفل `artifacts/soranet_pq_rollout/`.

### المنحدر -> الكناري (المرحلة ب -> المرحلة أ)

1. قم باستيراد لقطة الدليل التي تم التقاطها قبل الترويج باستخدام `sorafs_cli guard-directory import --guard-directory guards.json` وأعد تشغيل `sorafs_cli guard-directory verify` حتى تتضمن حزمة العرض التجزئات.
2. قم بضبط `rollout_phase = "canary"` (أو تجاوز `anonymity_policy stage-a`) في تكوينات المنسق والعميل، ثم قم بتكرار مثقاب السقاطة PQ من [PQ Rachet Runbook](./pq-ratchet-runbook.md) لاختبار خط الأنابيب المنخفض.
3. قم بإضافة لقطات شاشة تم تحديثها من PQ Ratchet والقياس عن بعد SN16 بالإضافة إلى نتائج التنبيهات لجميع سجل الأحداث قبل إخطار الإدارة.

### مسجلات الدرابزين- مرجع `docs/source/ops/soranet_transport_rollback.md` عندما يتم إجراء عرض توضيحي وتسجيل أي تخفيف مؤقت مثل عنصر `TODO:` في أداة تعقب الطرح للعمل اللاحق.
- قم بصيانة `dashboards/alerts/soranet_handshake_rules.yml` و`dashboards/alerts/soranet_privacy_rules.yml` من خلال تغطية `promtool test rules` مسبقًا وبعد التراجع حتى يتم توثيق انجراف التنبيهات جنبًا إلى جنب مع حزمة الالتقاط.